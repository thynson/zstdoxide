//! Finite State Entropy tables and normalized-frequency parsing.
//!
//! Wire constants and table construction follow facebook/zstd v1.5.7,
//! `lib/common/entropy_common.c` and `lib/common/fse_decompress.c`.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::bitstream::ReverseBits;
use crate::{Error, Result};

/// One initialized FSE transition; a plain value shared by both feature modes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FseEntry {
    pub(crate) symbol: u8,
    pub(crate) bits: u8,
    pub(crate) base: u16,
}

/// An owned transition table, or one inline RLE transition without allocation.
#[derive(Debug)]
pub(crate) struct FseTable<A: Allocator = Global> {
    entries: Vec<FseEntry, A>,
    repeated: Option<FseEntry>,
    table_log: u8,
}

/// An immutable validated transition view independent of its owner's allocator.
/// Its borrowed slice cannot outlive the table; inline RLE remains distinct from
/// a normalized singleton table with a nonzero state width.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FseTableView<'a> {
    entries: &'a [FseEntry],
    repeated: Option<FseEntry>,
    table_log: u8,
}

impl FseTable<Global> {
    pub(crate) fn from_normalized(counts: &[i16], table_log: u8) -> Result<Self> {
        Self::from_normalized_in(counts, table_log, Global)
    }

    pub(crate) fn read(data: &[u8], max_symbol: usize, max_log: u8) -> Result<(Self, usize)> {
        Self::read_in(data, max_symbol, max_log, Global)
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Global convenience retained for direct dictionary-table callers"
        )
    )]
    pub(crate) fn read_dictionary(
        data: &[u8],
        max_symbol: usize,
        max_log: u8,
    ) -> Result<(Self, usize)> {
        Self::read_dictionary_in(data, max_symbol, max_log, Global)
    }

    pub(crate) fn rle(symbol: u8) -> Self {
        Self::rle_in(symbol, Global)
    }
}

impl<A: Allocator> FseTable<A> {
    pub(crate) fn retained_capacity(&self) -> usize {
        self.entries.capacity() * core::mem::size_of::<FseEntry>()
    }

    /// Build a decoding table from a validated normalized distribution.
    ///
    /// Corresponds to upstream `FSE_buildDTable_internal`.
    pub(crate) fn from_normalized_in(counts: &[i16], table_log: u8, allocator: A) -> Result<Self> {
        Self::build_normalized_in(counts, table_log, false, allocator)
    }

    /// Build prepared dictionary transitions, which may have one symbol while
    /// retaining a nonzero state width. Ordinary weights/sequence constructors
    /// retain their existing multi-symbol validation boundary.
    fn build_normalized_in(
        counts: &[i16],
        table_log: u8,
        dictionary: bool,
        allocator: A,
    ) -> Result<Self> {
        if !(5..=12).contains(&table_log) || counts.is_empty() || counts.len() > 256 {
            return Err(Error::CorruptData);
        }
        let table_size = 1_usize << table_log;
        let mut total = 0_usize;
        let mut present = 0;
        for &count in counts {
            if count < -1 {
                return Err(Error::CorruptData);
            }
            total += usize::from(count.unsigned_abs());
            present += usize::from(count != 0);
        }
        if total != table_size || present == 0 || (present == 1 && !dictionary) {
            return Err(Error::CorruptData);
        }
        let mut entries = Vec::new_in(allocator);
        entries
            .try_reserve_exact(table_size)
            .map_err(|_| Error::AllocationFailed)?;
        // Each push stays within the successfully reserved capacity. Unlike a
        // bulk resize, this initialization cannot request another allocation.
        for _ in 0..table_size {
            entries.push(FseEntry::default());
        }
        let mut next_states = [0_u16; 256];
        let mut low_probability_start = table_size;
        for (symbol, &count) in counts.iter().enumerate() {
            if count == -1 {
                low_probability_start -= 1;
                entries[low_probability_start].symbol = symbol as u8;
                next_states[symbol] = 1;
            } else {
                next_states[symbol] = count as u16;
            }
        }
        let step = (table_size >> 1) + (table_size >> 3) + 3;
        let mask = table_size - 1;
        let mut position = 0;
        for (symbol, &count) in counts.iter().enumerate() {
            for _ in 0..count.max(0) {
                entries[position].symbol = symbol as u8;
                position = (position + step) & mask;
                while position >= low_probability_start {
                    position = (position + step) & mask;
                }
            }
        }
        if position != 0 {
            return Err(Error::CorruptData);
        }
        for entry in &mut entries {
            let next = next_states[usize::from(entry.symbol)];
            if next == 0 {
                return Err(Error::CorruptData);
            }
            next_states[usize::from(entry.symbol)] += 1;
            entry.bits = table_log - (15 - next.leading_zeros() as u8);
            entry.base = ((usize::from(next) << entry.bits) - table_size) as u16;
            if usize::from(entry.base) + (1_usize << entry.bits) > table_size {
                return Err(Error::CorruptData);
            }
        }
        Ok(Self {
            entries,
            repeated: None,
            table_log,
        })
    }

    /// Read a normalized-count header and return its rounded-up byte length.
    ///
    /// `max_symbol` is inclusive. Corresponds to upstream `FSE_readNCount`.
    pub(crate) fn read_in(
        data: &[u8],
        max_symbol: usize,
        max_log: u8,
        allocator: A,
    ) -> Result<(Self, usize)> {
        let parsed = read_normalized_counts(data, max_symbol, max_log)?;
        let table = Self::from_normalized_in(
            &parsed.counts[..parsed.symbols],
            parsed.table_log,
            allocator,
        )?;
        Ok((table, parsed.bytes_read))
    }

    /// Read a formatted dictionary distribution, including a single-symbol
    /// table with its original state width. Corresponds to `ZSTD_loadDEntropy`.
    pub(crate) fn read_dictionary_in(
        data: &[u8],
        max_symbol: usize,
        max_log: u8,
        allocator: A,
    ) -> Result<(Self, usize)> {
        let parsed = read_normalized_counts(data, max_symbol, max_log)?;
        let table = Self::build_normalized_in(
            &parsed.counts[..parsed.symbols],
            parsed.table_log,
            true,
            allocator,
        )?;
        Ok((table, parsed.bytes_read))
    }

    /// Keep the allocator with the owner without allocating an RLE payload.
    pub(crate) fn rle_in(symbol: u8, allocator: A) -> Self {
        Self {
            entries: Vec::new_in(allocator),
            repeated: Some(FseEntry {
                symbol,
                bits: 0,
                base: 0,
            }),
            table_log: 0,
        }
    }

    /// Copy validated transitions into fresh storage owned by `allocator`.
    ///
    /// The source and its allocator remain unchanged on success or failure.
    /// Neither allocator needs to be Clone or equivalent to the other. Inline
    /// RLE copies remain allocation-free and preserve their zero-width state.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Allocator-aware contexts adopt table copies in a later checkpoint"
        )
    )]
    pub(crate) fn try_clone_in<B: Allocator>(&self, allocator: B) -> Result<FseTable<B>> {
        let mut entries = Vec::new_in(allocator);
        entries
            .try_reserve_exact(self.entries.len())
            .map_err(|_| Error::AllocationFailed)?;
        // The exact length was reserved before any initialized entry is copied;
        // each Copy push stays within that capacity and cannot allocate again.
        for &entry in &self.entries {
            entries.push(entry);
        }
        Ok(FseTable {
            entries,
            repeated: self.repeated,
            table_log: self.table_log,
        })
    }

    /// Borrow validated entries without allocating, copying, or cloning an owner.
    pub(crate) fn view(&self) -> FseTableView<'_> {
        FseTableView {
            entries: &self.entries,
            repeated: self.repeated,
            table_log: self.table_log,
        }
    }

    pub(crate) fn table_log(&self) -> u8 {
        self.view().table_log()
    }

    pub(crate) fn entry(&self, state: usize) -> Result<FseEntry> {
        self.view().entry(state)
    }
}

impl FseTableView<'_> {
    pub(crate) fn table_log(&self) -> u8 {
        self.table_log
    }

    pub(crate) fn entry(&self, state: usize) -> Result<FseEntry> {
        if let Some(entry) = self.repeated {
            return if state == 0 {
                Ok(entry)
            } else {
                Err(Error::CorruptData)
            };
        }
        if state >= self.entries.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-fse"))]
        let entry = self.entries[state];
        #[cfg(feature = "unsafe-fse")]
        // SAFETY: the runtime check above proves `state < entries.len()`.
        // The private view constructor borrows only initialized entries from
        // a validated table. The slice guarantees their alignment and lifetime,
        // keeping the owning table and its allocator borrowed while used.
        // Copying through a shared reference neither mutates the entries nor
        // extends any borrow; no pointer or reference escapes this access.
        let entry = unsafe { *self.entries.get_unchecked(state) };
        Ok(entry)
    }
}

/// Validated normalized-count metadata, independent of a decoding table.
pub(crate) struct NormalizedCounts {
    pub(crate) counts: [i16; 256],
    pub(crate) symbols: usize,
    pub(crate) table_log: u8,
    pub(crate) bytes_read: usize,
}

/// Parse the exact outputs of upstream `FSE_readNCount`.
///
/// Single-symbol distributions are valid metadata; compressed decoding tables
/// still reject them through `FseTable::from_normalized`.
pub(crate) fn read_normalized_counts(
    data: &[u8],
    max_symbol: usize,
    max_log: u8,
) -> Result<NormalizedCounts> {
    if max_symbol > 255 || !(5..=12).contains(&max_log) {
        return Err(Error::CorruptData);
    }
    let mut bits = ForwardBits { data, position: 0 };
    let table_log = bits.read(4)? as u8 + 5;
    if table_log > max_log {
        return Err(Error::CorruptData);
    }
    let mut counts = [0_i16; 256];
    let mut remaining = (1_i32 << table_log) + 1;
    let mut threshold = 1_i32 << table_log;
    let mut width = table_log + 1;
    let mut symbol = 0;
    let mut previous_zero = false;
    while remaining > 1 {
        if previous_zero {
            loop {
                let repeat = bits.read(2)? as usize;
                symbol += repeat;
                if symbol > max_symbol {
                    return Err(Error::CorruptData);
                }
                if repeat != 3 {
                    break;
                }
            }
        }
        if symbol > max_symbol {
            return Err(Error::CorruptData);
        }
        let cutoff = 2 * threshold - 1 - remaining;
        let low = bits.read(width - 1)? as i32;
        let encoded = if low < cutoff {
            low
        } else {
            let full = low + ((bits.read(1)? as i32) << (width - 1));
            if full >= threshold {
                full - cutoff
            } else {
                full
            }
        };
        let count = encoded - 1;
        if count < -1 || count.abs() >= remaining {
            return Err(Error::CorruptData);
        }
        counts[symbol] = count as i16;
        symbol += 1;
        remaining -= count.abs();
        previous_zero = count == 0;
        while remaining > 1 && remaining < threshold {
            threshold >>= 1;
            width -= 1;
        }
    }
    Ok(NormalizedCounts {
        counts,
        symbols: symbol,
        table_log,
        bytes_read: bits.position.div_ceil(8),
    })
}

/// A setup-only forward bit cursor for normalized-frequency descriptions.
struct ForwardBits<'a> {
    data: &'a [u8],
    position: usize,
}

impl ForwardBits<'_> {
    fn read(&mut self, count: u8) -> Result<u32> {
        let end = self
            .position
            .checked_add(usize::from(count))
            .ok_or(Error::ResourceLimit)?;
        if end.div_ceil(8) > self.data.len() {
            return Err(Error::UnexpectedEof);
        }
        let mut value = 0;
        for offset in 0..usize::from(count) {
            let position = self.position + offset;
            #[cfg(not(feature = "unsafe-bitstream"))]
            let byte = self.data[position / 8];
            #[cfg(feature = "unsafe-bitstream")]
            // SAFETY: position is less than end, and ceil(end / 8) was checked
            // against data.len(), so position / 8 is inside this initialized
            // slice. The shared slice supplies validity and lifetime; u8 has
            // alignment one, and this copied read cannot violate aliasing.
            let byte = unsafe { *self.data.get_unchecked(position / 8) };
            value |= u32::from((byte >> (position % 8)) & 1) << offset;
        }
        self.position = end;
        Ok(value)
    }
}

/// Decode explicit Huffman weights from a two-state FSE stream.
///
/// Corresponds to upstream `FSE_decompress_usingDTable_generic`. The weight
/// stream has its own termination rule: after emitting a symbol whose state
/// update would exceed the bitstream, emit the other state's final symbol.
#[cfg(test)]
pub(crate) fn decode_weights(data: &[u8]) -> Result<Vec<u8>> {
    decode_weights_in(data, Global)
}

/// Decode weights with both scratch-table and output storage in the selected
/// allocator family. The clone allocates its own table; it never receives an
/// allocation owned by the original allocator. Borrowing a non-Clone allocator
/// also works because a shared reference is Clone and retains its identity.
pub(crate) fn decode_weights_in<A: Allocator + Clone>(
    data: &[u8],
    allocator: A,
) -> Result<Vec<u8, A>> {
    let (table, header_size) = FseTable::read_in(data, 255, 6, allocator.clone())?;
    let mut bits = ReverseBits::new(data.get(header_size..).ok_or(Error::UnexpectedEof)?)?;
    // Both initial states must be completely present, even for a terminal pair.
    let mut states = [
        bits.read_bits(table.table_log())? as usize,
        bits.read_bits(table.table_log())? as usize,
    ];
    let mut weights = Vec::new_in(allocator);
    weights
        .try_reserve_exact(255)
        .map_err(|_| Error::AllocationFailed)?;
    let mut current = 0;
    loop {
        let entry = table.entry(states[current])?;
        if weights.len() == 255 {
            return Err(Error::CorruptData);
        }
        weights.push(entry.symbol);
        if usize::from(entry.bits) > bits.remaining() {
            if weights.len() == 255 {
                return Err(Error::CorruptData);
            }
            weights.push(table.entry(states[current ^ 1])?.symbol);
            return Ok(weights);
        }
        states[current] = usize::from(entry.base) + bits.read_bits(entry.bits)? as usize;
        current ^= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 4,
            fail_at: None,
        })
        .unwrap()
    }

    fn assert_same_table<A: Allocator, B: Allocator>(left: &FseTable<A>, right: &FseTable<B>) {
        assert_eq!(left.table_log(), right.table_log());
        assert_eq!(left.repeated, right.repeated);
        assert_eq!(left.entries.len(), right.entries.len());
        for state in 0..1 << left.table_log() {
            assert_eq!(left.entry(state), right.entry(state), "state {state}");
        }
        for state in [1 << left.table_log(), usize::MAX] {
            assert_eq!(left.entry(state), Err(Error::CorruptData));
            assert_eq!(right.entry(state), Err(Error::CorruptData));
        }
    }

    struct FixtureBytes<'a>(&'a [u8]);

    impl<'a> FixtureBytes<'a> {
        fn take(&mut self, count: usize) -> &'a [u8] {
            let (bytes, rest) = self.0.split_at(count);
            self.0 = rest;
            bytes
        }

        fn number(&mut self) -> usize {
            u32::from_le_bytes(self.take(4).try_into().unwrap()) as usize
        }
    }

    #[test]
    fn table_views_preserve_rle_and_normalized_singleton_states_without_allocating() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let rle = FseTable::rle_in(1, &owner);
        let singleton = FseTable::build_normalized_in(&[0, 32], 5, true, &owner).unwrap();
        let ordinary = FseTable::from_normalized_in(&[16, 16], 5, &owner).unwrap();
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        let views = [rle.view(), singleton.view(), ordinary.view()];
        let copied = views;
        assert_eq!(copied[0].table_log(), 0);
        assert_eq!(copied[1].table_log(), 5);
        assert_eq!(copied[1].entry(31).unwrap().symbol, 1);
        assert!(copied[0].entry(1).is_err());
        for (table, view) in [&rle, &singleton, &ordinary].into_iter().zip(copied) {
            assert_eq!(view.table_log(), table.table_log());
            assert_eq!(view.entries.as_ptr(), table.entries.as_ptr());
            for state in 0..1_usize << view.table_log() {
                assert_eq!(view.entry(state), table.entry(state));
            }
            assert_eq!(
                view.entry(1_usize << view.table_log()),
                Err(Error::CorruptData)
            );
            assert_eq!(view.entry(usize::MAX), Err(Error::CorruptData));
        }
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        assert_eq!(monitor.snapshot().identities_created, 1);
        // The pending failure was not consumed by borrowing or reading views.
        assert!(matches!(
            ordinary.try_clone_in(&owner),
            Err(Error::AllocationFailed)
        ));
        assert_eq!(views[1].entry(31).unwrap().symbol, 1);
        drop((rle, singleton, ordinary));
        monitor.assert_released();
    }

    #[test]
    fn table_views_keep_every_supported_dense_state_and_extreme_bounds() {
        for log in 5..=12 {
            let count = 1_i16 << (log - 1);
            let table = FseTable::from_normalized(&[count, count], log).unwrap();
            let view = table.view();
            assert_eq!(view.table_log(), log);
            for state in 0..1_usize << log {
                let entry = view.entry(state).unwrap();
                assert_eq!(entry, table.entry(state).unwrap());
                assert!(entry.symbol < 2);
                assert!(usize::from(entry.base) + (1_usize << entry.bits) <= 1_usize << log);
            }
            for state in [1_usize << log, usize::MAX - 1, usize::MAX] {
                assert_eq!(view.entry(state), Err(Error::CorruptData));
            }
        }
    }

    #[test]
    fn custom_tables_match_global_transitions_at_every_state_width() {
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        for table_log in 5..=12 {
            let size = 1_i16 << table_log;
            for counts in [
                &[size / 2, size / 2][..],
                &[size - 2, -1, -1][..],
                &[0, size / 4, -1, size / 2, size / 4 - 1][..],
            ] {
                let global = FseTable::from_normalized(counts, table_log).unwrap();
                let custom = FseTable::from_normalized_in(counts, table_log, &allocator).unwrap();
                assert_same_table(&global, &custom);
                assert_eq!(
                    custom.retained_capacity(),
                    (1 << table_log) * core::mem::size_of::<FseEntry>()
                );
                drop(custom);
                monitor.assert_released();
            }
        }
        for counts in [&[-1; 32][..], &[0, 5, -1, 17, 9][..]] {
            let global = FseTable::from_normalized(counts, 5).unwrap();
            let custom = FseTable::from_normalized_in(counts, 5, &allocator).unwrap();
            assert_same_table(&global, &custom);
        }
        assert_eq!(monitor.snapshot().attempts, 26);
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().grow_calls, 0);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn all_official_count_headers_match_with_a_custom_allocator() {
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        let mut fixture = FixtureBytes(include_bytes!("../tests/upstream/entropy-data/fse.bin"));
        assert_eq!(fixture.take(8), b"ZFSE0001");
        assert_eq!(fixture.number(), 512);
        for _ in 0..512 {
            let table_log = fixture.take(1)[0];
            let maximum = usize::from(fixture.take(1)[0]);
            let size = usize::from(u16::from_le_bytes(fixture.take(2).try_into().unwrap()));
            let mut counts = [0_i16; 256];
            for count in &mut counts[..=maximum] {
                *count = i16::from_le_bytes(fixture.take(2).try_into().unwrap());
            }
            let header = fixture.take(size);
            let (global, consumed) = FseTable::read_dictionary(header, maximum, table_log).unwrap();
            let (custom, custom_consumed) =
                FseTable::read_dictionary_in(header, maximum, table_log, &allocator).unwrap();
            assert_eq!(consumed, size);
            assert_eq!(custom_consumed, consumed);
            assert_same_table(&global, &custom);
            if counts.iter().filter(|&&count| count != 0).count() >= 2 {
                let (ordinary, consumed) =
                    FseTable::read_in(header, maximum, table_log, &allocator).unwrap();
                let built =
                    FseTable::from_normalized_in(&counts[..=maximum], table_log, &allocator)
                        .unwrap();
                assert_eq!(consumed, size);
                assert_same_table(&ordinary, &custom);
                assert_same_table(&built, &custom);
            } else {
                assert_eq!(
                    FseTable::read_in(header, maximum, table_log, &allocator).err(),
                    Some(Error::CorruptData)
                );
            }
            drop(custom);
            monitor.assert_released();
        }
        assert!(fixture.0.is_empty());
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().grow_calls, 0);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn non_clone_allocator_is_owned_by_one_table_and_borrowed_by_weight_scratch() {
        let owned = allocator().into_non_clone();
        let owned_monitor = owned.monitor();
        let identity = owned.identity();
        let table = FseTable::from_normalized_in(&[16, 16], 5, owned).unwrap();
        assert_eq!(table.entries.allocator().identity(), identity);
        assert_same_table(&table, &FseTable::from_normalized(&[16, 16], 5).unwrap());
        assert_eq!(owned_monitor.snapshot().live_blocks, 1);
        drop(table);
        assert_eq!(owned_monitor.snapshot().identities_created, 1);
        assert_eq!(owned_monitor.snapshot().owners_dropped, 1);
        owned_monitor.assert_released();

        let borrowed = allocator().into_non_clone();
        let monitor = borrowed.monitor();
        let weights = decode_weights_in(&[0x10, 0x3f, 12, 16], &borrowed).unwrap();
        assert_eq!(weights.as_slice(), [0, 1, 0, 0]);
        assert_eq!(weights.allocator().identity(), borrowed.identity());
        assert_eq!(monitor.snapshot().attempts, 2);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(weights);
        monitor.assert_released();
        drop(borrowed);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
        monitor.assert_released();
    }

    #[test]
    fn weight_table_and_output_use_their_distinct_allocator_owners() {
        let allocator = allocator();
        let monitor = allocator.monitor();
        let original_identity = allocator.identity();
        let weights = decode_weights_in(&[0x10, 0x3f, 3, 4], allocator).unwrap();
        assert_eq!(weights.as_slice(), [0, 1]);
        assert_eq!(weights.allocator().identity(), original_identity);
        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.identities_created, 2);
        assert_eq!(snapshot.owners_dropped, 1);
        assert_eq!(snapshot.attempts, 2);
        assert_eq!(snapshot.successes, 2);
        assert_eq!(snapshot.peak_live_blocks, 2);
        assert_eq!(snapshot.live_blocks, 1);
        assert_eq!(snapshot.deallocate_calls, 1);
        assert_eq!(
            snapshot.requested_bytes,
            (32 * core::mem::size_of::<FseEntry>() + 255) as u128
        );
        assert_eq!(snapshot.grow_calls, 0);
        assert!(!snapshot.violations.any());
        drop(weights);
        assert_eq!(monitor.snapshot().owners_dropped, 2);
        monitor.assert_released();
    }

    #[test]
    fn either_weight_allocation_failure_releases_every_completed_owner() {
        for ordinal in 1..=2 {
            let allocator = TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 4,
                fail_at: NonZeroU64::new(ordinal),
            })
            .unwrap();
            let monitor = allocator.monitor();
            assert_eq!(
                decode_weights_in(&[0x10, 0x3f, 3, 4], allocator).err(),
                Some(Error::AllocationFailed)
            );
            let snapshot = monitor.snapshot();
            assert_eq!(snapshot.attempts, ordinal);
            assert_eq!(snapshot.failures, 1);
            assert_eq!(snapshot.successes, ordinal - 1);
            assert_eq!(snapshot.deallocate_calls, ordinal - 1);
            assert_eq!(snapshot.identities_created, 2);
            assert_eq!(snapshot.owners_dropped, 2);
            monitor.assert_released();
        }
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        assert!(monitor.fail_next());
        assert_eq!(
            FseTable::from_normalized_in(&[16, 16], 5, allocator).err(),
            Some(Error::AllocationFailed)
        );
        assert_eq!(monitor.snapshot().attempts, 1);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
        monitor.assert_released();
    }

    #[test]
    fn fallible_table_copy_keeps_each_allocation_with_its_original_owner() {
        let first = allocator();
        let monitor = first.monitor();
        let second = first.clone().into_non_clone();
        assert_ne!(first.identity(), second.identity());
        let original = FseTable::from_normalized_in(&[30, -1, -1], 5, first).unwrap();
        let copied = original.try_clone_in(second).unwrap();
        assert_ne!(original.entries.as_ptr(), copied.entries.as_ptr());
        assert_ne!(
            original.entries.allocator().identity(),
            copied.entries.allocator().identity()
        );
        assert_same_table(&original, &copied);
        assert_eq!(monitor.snapshot().live_blocks, 2);
        drop(original);
        assert_same_table(
            &copied,
            &FseTable::from_normalized(&[30, -1, -1], 5).unwrap(),
        );
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(copied);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 2);

        let custom = allocator().into_non_clone();
        let monitor = custom.monitor();
        let (singleton, _) = FseTable::read_dictionary(&[0xf0, 0x03], 255, 12).unwrap();
        let copied = singleton.try_clone_in(custom).unwrap();
        assert_same_table(&singleton, &copied);
        assert_eq!(copied.table_log(), 5);
        drop(copied);
        monitor.assert_released();
    }

    #[test]
    fn failed_table_copy_preserves_source_storage_and_transitions() {
        let allocator = allocator();
        let monitor = allocator.monitor();
        let destination = allocator.clone();
        let source = FseTable::from_normalized_in(&[0, 5, -1, 17, 9], 5, allocator).unwrap();
        let pointer = source.entries.as_ptr();
        let capacity = source.retained_capacity();
        assert!(monitor.fail_next());
        assert_eq!(
            source.try_clone_in(destination).err(),
            Some(Error::AllocationFailed)
        );
        assert_eq!(source.entries.as_ptr(), pointer);
        assert_eq!(source.retained_capacity(), capacity);
        assert_same_table(
            &source,
            &FseTable::from_normalized(&[0, 5, -1, 17, 9], 5).unwrap(),
        );
        assert_eq!(monitor.snapshot().attempts, 2);
        assert_eq!(monitor.snapshot().failures, 1);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
        drop(source);
        monitor.assert_released();
    }

    #[test]
    fn custom_rle_construction_and_copy_never_allocate() {
        let allocator = allocator();
        let monitor = allocator.monitor();
        let destination = allocator.clone().into_non_clone();
        assert!(monitor.fail_next());
        let original = FseTable::rle_in(255, allocator.into_non_clone());
        let copied = original.try_clone_in(destination).unwrap();
        assert_same_table(&original, &copied);
        assert_same_table(&original, &FseTable::rle(255));
        assert_eq!(copied.retained_capacity(), 0);
        assert_eq!(monitor.snapshot().allocate_calls, 0);
        assert_eq!(monitor.snapshot().attempts, 0);
        drop(original);
        drop(copied);
        assert_eq!(monitor.snapshot().owners_dropped, 2);
        monitor.assert_released();
    }

    #[test]
    fn malformed_custom_table_inputs_do_not_allocate() {
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        assert!(monitor.fail_next());
        for (counts, log) in [
            (&[32][..], 5),
            (&[17, 16][..], 5),
            (&[-2, 30][..], 5),
            (&[16, 16][..], 4),
            (&[][..], 5),
        ] {
            assert_eq!(
                FseTable::from_normalized_in(counts, log, &allocator).err(),
                FseTable::from_normalized(counts, log).err()
            );
        }
        for (data, maximum, log) in [
            (&[][..], 255, 6),
            (&[0x10][..], 1, 5),
            (&[0x10, 0x3f][..], 0, 5),
            (&[0x0f][..], 255, 6),
            (&[0xf0, 0x03][..], 255, 12),
        ] {
            assert_eq!(
                FseTable::read_in(data, maximum, log, &allocator).err(),
                FseTable::read(data, maximum, log).err()
            );
        }
        assert_eq!(
            FseTable::read_dictionary_in(&[0xf0], 255, 12, &allocator).err(),
            FseTable::read_dictionary(&[0xf0], 255, 12).err()
        );
        assert_eq!(monitor.snapshot().attempts, 0);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn malformed_custom_weight_streams_release_scratch_and_output() {
        let mut too_many = [0; 36];
        too_many[..2].copy_from_slice(&[0x10, 0x3f]);
        too_many[35] = 1;
        for input in [
            &[0x10, 0x3f][..],
            &[0x10, 0x3f, 0][..],
            &[0x10, 0x3f, 1][..],
            &[0x10, 0x3f, 0, 1][..],
            &too_many,
        ] {
            let allocator = allocator();
            let monitor = allocator.monitor();
            assert_eq!(
                decode_weights_in(input, allocator).err(),
                decode_weights(input).err()
            );
            assert!((1..=2).contains(&monitor.snapshot().attempts));
            assert_eq!(monitor.snapshot().identities_created, 2);
            assert_eq!(monitor.snapshot().owners_dropped, 2);
            monitor.assert_released();
        }
    }

    #[test]
    fn official_huffman_weight_payloads_match_with_custom_scratch() {
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        let mut fixture =
            FixtureBytes(include_bytes!("../tests/upstream/entropy-data/huffman.bin"));
        assert_eq!(fixture.take(8), b"ZHUF0001");
        assert_eq!(fixture.number(), 30);
        let mut compressed = 0;
        for _ in 0..30 {
            let source_size = fixture.number();
            let description_size = fixture.number();
            let single_size = fixture.number();
            let four_size = fixture.number();
            fixture.take(source_size);
            let description = fixture.take(description_size);
            fixture.take(single_size);
            fixture.take(four_size);
            // Direct weight descriptions contain no FSE table or weight stream.
            if description[0] < 128 {
                assert_eq!(description.len(), usize::from(description[0]) + 1);
                let global = decode_weights(&description[1..]).unwrap();
                let custom = decode_weights_in(&description[1..], &allocator).unwrap();
                assert_eq!(custom.as_slice(), global);
                drop(custom);
                monitor.assert_released();
                compressed += 1;
            }
        }
        assert!(fixture.0.is_empty());
        assert_eq!(compressed, 12);
        assert_eq!(monitor.snapshot().attempts, compressed * 2);
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn normalized_metadata_preserves_single_symbol_and_consumed_size() {
        for data in [&[0xf0, 0x03][..], &[0xf0, 0x03, 0xff, 0xff][..]] {
            let parsed = read_normalized_counts(data, 255, 12).unwrap();
            assert_eq!(parsed.table_log, 5);
            assert_eq!(parsed.symbols, 1);
            assert_eq!(parsed.counts[0], 32);
            assert!(parsed.counts[1..].iter().all(|&count| count == 0));
            assert_eq!(parsed.bytes_read, 2);
        }
        assert!(read_normalized_counts(&[0xf0], 255, 12).is_err());
        assert!(FseTable::read(&[0xf0, 0x03], 255, 12).is_err());
    }

    #[test]
    fn reads_balanced_distribution_header() {
        let (table, consumed) = FseTable::read(&[0x10, 0x3f], 1, 5).unwrap();
        assert_eq!(consumed, 2);
        assert_eq!(table.table_log(), 5);
        let reference = FseTable::from_normalized(&[16, 16], 5).unwrap();
        for state in 0..32 {
            assert_eq!(table.entry(state), reference.entry(state));
            assert_eq!(table.entry(state).unwrap().bits, 1);
        }
    }

    #[test]
    fn reads_zero_run_with_continuation() {
        // 16, 0, three repeated zeroes, then 16. Both two-bit run codes
        // (continuation 3 and terminator 0) are required by the wire format.
        let (table, consumed) = FseTable::read(&[0x10, 0x63, 0x3e], 5, 5).unwrap();
        assert_eq!(consumed, 3);
        let reference = FseTable::from_normalized(&[16, 0, 0, 0, 0, 16], 5).unwrap();
        for state in 0..32 {
            assert_eq!(table.entry(state), reference.entry(state));
        }
        assert!(FseTable::read(&[0x10, 0x63, 0x3e], 4, 5).is_err());
    }

    #[test]
    fn low_probability_symbols_fill_end_of_table() {
        let table = FseTable::from_normalized(&[30, -1, -1], 5).unwrap();
        assert_eq!(
            table.entry(31).unwrap(),
            FseEntry {
                symbol: 1,
                bits: 5,
                base: 0
            }
        );
        assert_eq!(
            table.entry(30).unwrap(),
            FseEntry {
                symbol: 2,
                bits: 5,
                base: 0
            }
        );
        for state in 0..30 {
            assert_eq!(table.entry(state).unwrap().symbol, 0);
        }
    }

    #[test]
    fn all_low_probability_symbols_are_supported() {
        let table = FseTable::from_normalized(&[-1; 32], 5).unwrap();
        for state in 0..32 {
            assert_eq!(
                table.entry(state).unwrap(),
                FseEntry {
                    symbol: 31 - state as u8,
                    bits: 5,
                    base: 0
                }
            );
        }
    }

    #[test]
    fn every_symbols_transitions_partition_the_state_space() {
        let counts = [0, 5, -1, 17, 9];
        let table = FseTable::from_normalized(&counts, 5).unwrap();
        for (symbol, &count) in counts.iter().enumerate() {
            let mut covered = [false; 32];
            let mut occurrences = 0;
            for state in 0..32 {
                let entry = table.entry(state).unwrap();
                if usize::from(entry.symbol) == symbol {
                    occurrences += 1;
                    let start = usize::from(entry.base);
                    let end = start + (1 << entry.bits);
                    for next in &mut covered[start..end] {
                        assert!(!*next);
                        *next = true;
                    }
                }
            }
            assert_eq!(occurrences, count.unsigned_abs() as usize);
            assert!(covered.iter().all(|&covered| covered == (count != 0)));
        }
    }

    #[test]
    fn weight_stream_emits_terminal_pair() {
        // The header describes counts [16, 16]. Initial states 0 and 3
        // select symbols 0 and 1, with no transition bits before the marker.
        assert_eq!(decode_weights(&[0x10, 0x3f, 3, 4]).unwrap(), [0, 1]);
        // Two zero transition bits precede the initial states. Both states
        // become zero before the final pair is emitted.
        assert_eq!(decode_weights(&[0x10, 0x3f, 12, 16]).unwrap(), [0, 1, 0, 0]);
    }

    #[test]
    fn weight_stream_rejects_missing_initial_states_and_marker() {
        for input in [
            &[0x10, 0x3f][..],
            &[0x10, 0x3f, 0][..],
            &[0x10, 0x3f, 1][..],
            &[0x10, 0x3f, 0, 1][..],
        ] {
            assert!(decode_weights(input).is_err(), "input: {input:?}");
        }
    }

    #[test]
    fn weight_output_is_bounded_at_255_symbols() {
        // Each transition consumes one bit. The two terminal symbols make
        // this 254-bit transition payload exceed the weight alphabet limit.
        let mut data = alloc::vec![0x10, 0x3f];
        data.extend_from_slice(&[0; 33]);
        data.push(1);
        assert_eq!(decode_weights(&data), Err(Error::CorruptData));
    }

    #[test]
    fn rejects_invalid_tables_and_states() {
        assert!(FseTable::from_normalized(&[32], 5).is_err());
        assert!(FseTable::from_normalized(&[17, 16], 5).is_err());
        assert!(FseTable::from_normalized(&[-2, 30], 5).is_err());
        assert!(FseTable::from_normalized(&[16, 16], 4).is_err());
        let table = FseTable::rle(7);
        assert_eq!(table.table_log(), 0);
        assert_eq!(
            table.entry(0),
            Ok(FseEntry {
                symbol: 7,
                bits: 0,
                base: 0
            })
        );
        assert_eq!(table.entry(1), Err(Error::CorruptData));
        assert!(FseTable::read(&[], 255, 6).is_err());
        assert!(FseTable::read(&[0x10], 1, 5).is_err());
        assert!(FseTable::read(&[0x10, 0x3f], 0, 5).is_err());
        assert!(FseTable::read(&[0x0f], 255, 6).is_err());
    }
}
