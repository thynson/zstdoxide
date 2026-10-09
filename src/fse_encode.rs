//! Checked inverse FSE transitions for sequence encoding.
//!
//! The state/symbol relation corresponds to `FSE_buildCTable_wksp`,
//! `FSE_initCState2`, and `FSE_encodeSymbol` in facebook/zstd v1.5.7. This
//! implementation owns typed rows instead of adopting C's packed table layout.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::fse::{FseEntry, FseTable, FseTableView};
use crate::{Error, Result};

const UNAVAILABLE: u16 = u16::MAX;

/// A copied inverse relation: decoding `state` yields the requested symbol,
/// consumes `bits` bits with `value`, then reaches the requested successor.
/// This initialized scalar record needs no separate unsafe representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FseEncodingTransition {
    pub(crate) state: u16,
    pub(crate) value: u16,
    pub(crate) bits: u8,
}

impl FseEncodingTransition {
    const UNAVAILABLE: Self = Self {
        state: UNAVAILABLE,
        value: 0,
        bits: 0,
    };
}

/// An immutable owned inverse table, or one allocation-free inline RLE symbol.
///
/// Each present dense row covers all `state_count` successor states exactly
/// once. Absent rows and initial states are explicitly unavailable. Logical
/// links are indices into initialized owned storage; no source borrow escapes
/// construction. A normalized singleton retains its original nonzero log.
/// `unsafe-fse-encoder` changes only checked dense-row access, independently
/// of the decoder's `unsafe-fse` feature. Scalar RLE uses the shared safe path.
#[derive(Debug)]
pub(crate) struct FseEncodingTable<A: Allocator = Global> {
    transitions: Vec<FseEncodingTransition, A>,
    initial: Vec<u16, A>,
    repeated: Option<u8>,
    state_count: usize,
    table_log: u8,
}

impl FseEncodingTable<Global> {
    /// Build an ordinary normalized table with the decoder's mass and alphabet
    /// validation. Single-symbol normalized dictionary tables enter through
    /// `from_decoding`; an inline RLE table enters through `rle`.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Global convenience retained for direct inverse-table callers"
        )
    )]
    pub(crate) fn new(counts: &[i16], table_log: u8) -> Result<Self> {
        Self::new_in(counts, table_log, Global)
    }

    /// Invert an immutable validated decoding table without retaining a borrow.
    /// The first decoding state for each symbol is its terminal initialization,
    /// preserving the previous predefined encoder's exact initial selection.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Global convenience retained for direct inverse-table callers"
        )
    )]
    pub(crate) fn from_decoding(decoding: &FseTable) -> Result<Self> {
        Self::from_decoding_in(decoding, Global)
    }

    /// Construct the wire's inline RLE mode, which has no state or payload bits.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Global convenience constructor retained during allocator migration"
        )
    )]
    pub(crate) const fn rle(symbol: u8) -> Self {
        Self {
            transitions: Vec::new(),
            initial: Vec::new(),
            repeated: Some(symbol),
            state_count: 1,
            table_log: 0,
        }
    }

    #[cfg(test)]
    fn from_relation(table_log: u8, entry_at: impl Fn(usize) -> Result<FseEntry>) -> Result<Self> {
        Self::from_relation_in(table_log, entry_at, Global)
    }
}

impl<A: Allocator> FseEncodingTable<A> {
    pub(crate) fn table_log(&self) -> u8 {
        self.table_log
    }

    /// Highest present symbol; dense construction trims absent trailing rows.
    pub(crate) fn max_symbol(&self) -> u8 {
        self.repeated
            .unwrap_or_else(|| (self.initial.len() - 1) as u8)
    }

    pub(crate) fn retained_capacity(&self) -> usize {
        self.transitions.capacity() * core::mem::size_of::<FseEncodingTransition>()
            + self.initial.capacity() * core::mem::size_of::<u16>()
    }

    pub(crate) fn initial(&self, symbol: usize) -> Result<u16> {
        if let Some(repeated) = self.repeated {
            return if symbol == usize::from(repeated) {
                Ok(0)
            } else {
                Err(Error::CorruptData)
            };
        }
        if symbol >= self.initial.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-fse-encoder"))]
        let state = self.initial[symbol];
        #[cfg(feature = "unsafe-fse-encoder")]
        // SAFETY: the runtime check proves symbol < initial.len(). Every
        // element was initialized by reserved pushes during construction.
        // The Vec owns the live, properly aligned u16 allocation throughout
        // this shared borrow; no mutation or reallocation can overlap it.
        // Copying this scalar neither creates aliases nor extends a reference.
        let state = unsafe { *self.initial.get_unchecked(symbol) };
        if state == UNAVAILABLE {
            Err(Error::CorruptData)
        } else {
            Ok(state)
        }
    }

    pub(crate) fn transition(&self, symbol: usize, next: u16) -> Result<FseEncodingTransition> {
        // This also rejects absent rows, including zero-initialized holes in
        // the alphabet, before any flattened transition access is attempted.
        self.initial(symbol)?;
        if usize::from(next) >= self.state_count {
            return Err(Error::CorruptData);
        }
        if self.repeated.is_some() {
            return Ok(FseEncodingTransition {
                state: 0,
                value: 0,
                bits: 0,
            });
        }
        let index = symbol
            .checked_mul(self.state_count)
            .and_then(|row| row.checked_add(usize::from(next)))
            .ok_or(Error::CorruptData)?;
        if index >= self.transitions.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-fse-encoder"))]
        let transition = self.transitions[index];
        #[cfg(feature = "unsafe-fse-encoder")]
        // SAFETY: symbol availability and successor bounds were checked, the
        // flattened index uses checked arithmetic, and the final runtime
        // check proves index < transitions.len(). Construction initialized
        // every slot, including explicit unavailable records. Vec owns valid,
        // properly aligned storage for this shared borrow, which prevents
        // overlapping mutation/reallocation. The copied scalar record carries
        // no references, pointers, or ownership out of that allocation.
        let transition = unsafe { *self.transitions.get_unchecked(index) };
        if transition.state == UNAVAILABLE {
            Err(Error::CorruptData)
        } else {
            Ok(transition)
        }
    }
}

impl<A: Allocator + Clone> FseEncodingTable<A> {
    /// Build fresh decoder scratch and inverse owners with this allocator type.
    /// Allocator clones may be non-equivalent: every Vec allocates and releases
    /// only through the exact value it retains. Ordinary Clone may itself panic
    /// or allocate; only returned reservation errors map to AllocationFailed.
    pub(crate) fn new_in(counts: &[i16], table_log: u8, allocator: A) -> Result<Self> {
        let decoding = FseTable::from_normalized_in(counts, table_log, allocator.clone())?;
        Self::from_decoding_in(&decoding, allocator)
    }

    /// Invert a validated table from an independent allocation domain. No source
    /// borrow or allocation moves into the result; failure leaves it unchanged.
    pub(crate) fn from_decoding_in<B: Allocator>(
        decoding: &FseTable<B>,
        allocator: A,
    ) -> Result<Self> {
        Self::from_decoding_view_in(decoding.view(), allocator)
    }

    /// Copy an allocator-independent validated view into fresh owned storage.
    /// No source reference or allocation enters the result, so the source may
    /// be dropped after construction. Existing inverse validation and errors apply.
    pub(crate) fn from_decoding_view_in(decoding: FseTableView<'_>, allocator: A) -> Result<Self> {
        Self::from_relation_in(
            decoding.table_log(),
            |state| decoding.entry(state),
            allocator,
        )
    }

    /// Retain two empty typed owners without allocating an inline RLE payload.
    pub(crate) fn rle_in(symbol: u8, allocator: A) -> Self {
        Self {
            transitions: Vec::new_in(allocator.clone()),
            initial: Vec::new_in(allocator),
            repeated: Some(symbol),
            state_count: 1,
            table_log: 0,
        }
    }

    /// The relation callback reads the same immutable table on both passes.
    /// Keeping this checked setup separate permits malformed-relation tests
    /// without exposing unvalidated constructors on the decoding table.
    fn from_relation_in(
        table_log: u8,
        entry_at: impl Fn(usize) -> Result<FseEntry>,
        allocator: A,
    ) -> Result<Self> {
        if table_log == 0 {
            let entry = entry_at(0)?;
            if entry.bits != 0 || entry.base != 0 {
                return Err(Error::CorruptData);
            }
            return Ok(Self::rle_in(entry.symbol, allocator));
        }
        if !(5..=12).contains(&table_log) {
            return Err(Error::CorruptData);
        }
        let state_count = 1_usize
            .checked_shl(u32::from(table_log))
            .ok_or(Error::ResourceLimit)?;
        let mut first_states = [UNAVAILABLE; 256];
        let mut symbol_count = 0;
        for state in 0..state_count {
            let entry = entry_at(state)?;
            validate_range(entry, table_log, state_count)?;
            let symbol = usize::from(entry.symbol);
            symbol_count = symbol_count.max(symbol + 1);
            if first_states[symbol] == UNAVAILABLE {
                first_states[symbol] = u16::try_from(state).map_err(|_| Error::ResourceLimit)?;
            }
        }
        let transition_count = symbol_count
            .checked_mul(state_count)
            .ok_or(Error::ResourceLimit)?;
        transition_count
            .checked_mul(core::mem::size_of::<FseEncodingTransition>())
            .ok_or(Error::ResourceLimit)?;
        let mut transitions = Vec::new_in(allocator.clone());
        transitions
            .try_reserve_exact(transition_count)
            .map_err(|_| Error::AllocationFailed)?;
        // Reservation bounds every initialization push; allocator identity never
        // changes and no infallible bulk resize can request another allocation.
        for _ in 0..transition_count {
            transitions.push(FseEncodingTransition::UNAVAILABLE);
        }
        let mut initial = Vec::new_in(allocator);
        initial
            .try_reserve_exact(symbol_count)
            .map_err(|_| Error::AllocationFailed)?;
        for &state in &first_states[..symbol_count] {
            initial.push(state);
        }

        for state in 0..state_count {
            let entry = entry_at(state)?;
            let width = validate_range(entry, table_log, state_count)?;
            let symbol = usize::from(entry.symbol);
            let row = symbol
                .checked_mul(state_count)
                .ok_or(Error::ResourceLimit)?;
            for value in 0..width {
                let next = usize::from(entry.base)
                    .checked_add(value)
                    .ok_or(Error::CorruptData)?;
                let index = row.checked_add(next).ok_or(Error::ResourceLimit)?;
                let slot = transitions.get_mut(index).ok_or(Error::CorruptData)?;
                if slot.state != UNAVAILABLE {
                    return Err(Error::CorruptData);
                }
                *slot = FseEncodingTransition {
                    state: u16::try_from(state).map_err(|_| Error::ResourceLimit)?,
                    value: u16::try_from(value).map_err(|_| Error::ResourceLimit)?,
                    bits: entry.bits,
                };
            }
        }
        for (symbol, &initial_state) in initial.iter().enumerate() {
            let row = &transitions[symbol * state_count..(symbol + 1) * state_count];
            if initial_state != UNAVAILABLE
                && row.iter().any(|transition| transition.state == UNAVAILABLE)
            {
                return Err(Error::CorruptData);
            }
        }
        Ok(Self {
            transitions,
            initial,
            repeated: None,
            state_count,
            table_log,
        })
    }
}

fn validate_range(entry: FseEntry, table_log: u8, state_count: usize) -> Result<usize> {
    if entry.bits > table_log {
        return Err(Error::CorruptData);
    }
    let width = 1_usize
        .checked_shl(u32::from(entry.bits))
        .ok_or(Error::CorruptData)?;
    let end = usize::from(entry.base)
        .checked_add(width)
        .ok_or(Error::CorruptData)?;
    if end > state_count {
        return Err(Error::CorruptData);
    }
    Ok(width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{LL_DEFAULT, ML_DEFAULT, OF_DEFAULT};
    use crate::entropy_encode::write_normalized_counts;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 8,
            fail_at: None,
        })
        .unwrap()
    }

    #[test]
    fn allocator_inverse_owns_distinct_clones_and_outlives_independent_decoder() {
        let source_allocator = allocator().into_non_clone();
        let source_monitor = source_allocator.monitor();
        let decoding = FseTable::from_normalized_in(&[16, 0, 16], 5, source_allocator).unwrap();
        let entries: Vec<_> = (0..32)
            .map(|state| decoding.entry(state).unwrap())
            .collect();
        let destination = allocator();
        let destination_monitor = destination.monitor();
        let destination_id = destination.identity();
        let inverse = FseEncodingTable::from_decoding_in(&decoding, destination).unwrap();
        assert_eq!(inverse.initial.allocator().identity(), destination_id);
        assert_ne!(inverse.transitions.allocator().identity(), destination_id);
        assert_eq!(source_monitor.snapshot().live_blocks, 1);
        assert_eq!(destination_monitor.snapshot().live_blocks, 2);
        drop(decoding);
        source_monitor.assert_released();
        for symbol in [0, 2] {
            for next in 0..32 {
                let transition = inverse.transition(symbol, next).unwrap();
                let decoded = entries[usize::from(transition.state)];
                assert_eq!(usize::from(decoded.symbol), symbol);
                assert_eq!(decoded.bits, transition.bits);
                assert_eq!(decoded.base + transition.value, next);
            }
        }
        assert_eq!(inverse.initial(1), Err(Error::CorruptData));
        drop(inverse);
        destination_monitor.assert_released();
    }

    #[test]
    fn allocator_borrowed_nonclone_preserves_rle_singletons_and_safe_lookup_errors() {
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        let repeated = FseEncodingTable::rle_in(7, &allocator);
        assert_eq!(monitor.snapshot().attempts, 0);
        assert_eq!(repeated.transition(7, 0).unwrap().bits, 0);
        assert_eq!(repeated.initial(6), Err(Error::CorruptData));
        assert_eq!(repeated.transition(7, 1), Err(Error::CorruptData));
        let header = write_normalized_counts(&[0, 32], 5).unwrap();
        let decoder = FseTable::read_dictionary(&header, 1, 5).unwrap().0;
        let singleton = FseEncodingTable::from_decoding_in(&decoder, &allocator).unwrap();
        assert_eq!(singleton.table_log(), 5);
        for next in 0..32 {
            assert_eq!(
                singleton.transition(1, next).unwrap(),
                FseEncodingTransition {
                    state: next,
                    value: 0,
                    bits: 0
                }
            );
        }
        assert_eq!(singleton.initial(usize::MAX), Err(Error::CorruptData));
        assert_eq!(singleton.transition(1, u16::MAX), Err(Error::CorruptData));
        drop(singleton);
        drop(repeated);
        monitor.assert_released();
    }

    #[test]
    fn allocator_inverse_failures_release_scratch_and_preserve_source_table() {
        let source = FseTable::from_normalized(&[16, 0, 16], 5).unwrap();
        let before: Vec<_> = (0..32).map(|state| source.entry(state).unwrap()).collect();
        for use_source in [false, true] {
            let baseline = allocator();
            let monitor = baseline.monitor();
            let table = if use_source {
                FseEncodingTable::from_decoding_in(&source, baseline).unwrap()
            } else {
                FseEncodingTable::new_in(&[16, 0, 16], 5, baseline).unwrap()
            };
            let attempts = monitor.snapshot().attempts;
            assert_eq!(attempts, if use_source { 2 } else { 3 });
            drop(table);
            monitor.assert_released();
            for ordinal in 1..=attempts {
                let allocator = TrackingAllocator::new(AllocatorConfig {
                    max_live_allocations: 8,
                    fail_at: NonZeroU64::new(ordinal),
                })
                .unwrap();
                let monitor = allocator.monitor();
                let result = if use_source {
                    FseEncodingTable::from_decoding_in(&source, allocator)
                } else {
                    FseEncodingTable::new_in(&[16, 0, 16], 5, allocator)
                };
                assert!(matches!(result, Err(Error::AllocationFailed)));
                assert_eq!(monitor.snapshot().attempts, ordinal);
                for (state, &entry) in before.iter().enumerate() {
                    assert_eq!(source.entry(state).unwrap(), entry);
                }
                monitor.assert_released();
            }
        }
        let invalid = allocator();
        let monitor = invalid.monitor();
        assert!(matches!(
            FseEncodingTable::new_in(&[31], 5, invalid),
            Err(Error::CorruptData)
        ));
        assert_eq!(monitor.snapshot().attempts, 0);
        monitor.assert_released();
    }

    fn verify_inverse(decoding: &FseTable) {
        let encoding = FseEncodingTable::from_decoding(decoding).unwrap();
        let state_count = 1_usize << decoding.table_log();
        assert_eq!(encoding.table_log(), decoding.table_log());
        for symbol in 0..=255 {
            let first = (0..state_count)
                .find(|&state| usize::from(decoding.entry(state).unwrap().symbol) == symbol);
            if let Some(first) = first {
                assert_eq!(encoding.initial(symbol).unwrap(), first as u16);
                for next in 0..state_count {
                    let inverse = encoding.transition(symbol, next as u16).unwrap();
                    let entry = decoding.entry(usize::from(inverse.state)).unwrap();
                    assert_eq!(usize::from(entry.symbol), symbol);
                    assert_eq!(entry.bits, inverse.bits);
                    assert!(usize::from(inverse.value) < 1 << inverse.bits);
                    assert_eq!(usize::from(entry.base + inverse.value), next);
                }
            } else {
                assert_eq!(encoding.initial(symbol), Err(Error::CorruptData));
                for next in 0..state_count {
                    assert_eq!(
                        encoding.transition(symbol, next as u16),
                        Err(Error::CorruptData)
                    );
                }
            }
            assert_eq!(
                encoding.transition(symbol, state_count as u16),
                Err(Error::CorruptData)
            );
        }
        assert_eq!(encoding.initial(usize::MAX), Err(Error::CorruptData));
        assert_eq!(encoding.transition(usize::MAX, 0), Err(Error::CorruptData));
    }

    #[test]
    fn predefined_tables_preserve_initial_states_and_every_inverse_relation() {
        for (counts, log) in [
            (&LL_DEFAULT[..], 6),
            (&ML_DEFAULT[..], 6),
            (&OF_DEFAULT[..], 5),
        ] {
            let decoding = FseTable::from_normalized(counts, log).unwrap();
            verify_inverse(&decoding);
            let encoding = FseEncodingTable::new(counts, log).unwrap();
            assert_eq!(encoding.max_symbol() as usize, counts.len() - 1);
            assert!(encoding.retained_capacity() > 0);
            for symbol in 0..counts.len() {
                let initial = encoding.initial(symbol).unwrap();
                assert_eq!(
                    decoding.entry(usize::from(initial)).unwrap().symbol as usize,
                    symbol
                );
            }
        }
    }

    #[test]
    fn support_holes_and_all_supported_logs_have_complete_inverse_rows() {
        for log in 5..=12 {
            let counts = [(1_i16 << log) - 2, 0, -1, 1];
            verify_inverse(&FseTable::from_normalized(&counts, log).unwrap());
        }
        verify_inverse(&FseTable::from_normalized(&[1; 256], 8).unwrap());
    }

    #[test]
    fn lookup_bounds_reject_extreme_indices_without_reading_absent_rows() {
        let decoding = FseTable::from_normalized(&[16, 0, 0, 16], 5).unwrap();
        let encoding = FseEncodingTable::from_decoding(&decoding).unwrap();
        for symbol in [0, 3] {
            assert!(encoding.initial(symbol).is_ok());
            for next in [0, 15, 31] {
                let inverse = encoding.transition(symbol, next).unwrap();
                let entry = decoding.entry(usize::from(inverse.state)).unwrap();
                assert_eq!(usize::from(entry.symbol), symbol);
                assert_eq!(entry.base + inverse.value, next);
            }
            for next in [32, 33, u16::MAX] {
                assert_eq!(encoding.transition(symbol, next), Err(Error::CorruptData));
            }
        }
        for symbol in [1, 2, 4, 255, 256, usize::MAX] {
            assert_eq!(encoding.initial(symbol), Err(Error::CorruptData));
            for next in [0, 31, u16::MAX] {
                assert_eq!(encoding.transition(symbol, next), Err(Error::CorruptData));
            }
        }
    }

    #[test]
    fn inline_rle_accepts_only_its_symbol_and_zero_successor_without_allocation() {
        for symbol in [0, 1, 255] {
            let table = FseEncodingTable::rle(symbol);
            assert_eq!(table.table_log(), 0);
            assert_eq!(table.max_symbol(), symbol);
            assert_eq!(table.retained_capacity(), 0);
            assert_eq!(table.initial(usize::from(symbol)), Ok(0));
            assert_eq!(
                table.transition(usize::from(symbol), 0),
                Ok(FseEncodingTransition {
                    state: 0,
                    value: 0,
                    bits: 0
                })
            );
            assert_eq!(
                table.transition(usize::from(symbol), 1),
                Err(Error::CorruptData)
            );
            assert_eq!(table.initial(256), Err(Error::CorruptData));
            verify_inverse(&FseTable::rle(symbol));
        }
    }

    #[test]
    fn dictionary_singletons_preserve_their_nonzero_initial_state_width() {
        for log in 5..=12 {
            let header = write_normalized_counts(&[0, 1_i16 << log], log).unwrap();
            let (decoding, consumed) = FseTable::read_dictionary(&header, 1, log).unwrap();
            assert_eq!(consumed, header.len());
            let encoding = FseEncodingTable::from_decoding(&decoding).unwrap();
            assert_eq!(encoding.table_log(), log);
            assert_eq!(encoding.initial(1), Ok(0));
            assert!(encoding.retained_capacity() > 0);
            for next in 0..1 << log {
                assert_eq!(
                    encoding.transition(1, next).unwrap(),
                    FseEncodingTransition {
                        state: next,
                        value: 0,
                        bits: 0
                    }
                );
            }
            assert_eq!(encoding.initial(0), Err(Error::CorruptData));
            assert_eq!(encoding.transition(0, 0), Err(Error::CorruptData));
        }
    }

    #[test]
    fn malformed_distributions_and_decoding_relations_are_rejected() {
        for (counts, log) in [
            (&[][..], 5),
            (&[32][..], 5),
            (&[-2, 34][..], 5),
            (&[16, 15][..], 5),
            (&[16, 17][..], 5),
            (&[16, 16][..], 4),
            (&[16, 16][..], 13),
            (&[1; 257][..], 8),
        ] {
            assert!(matches!(
                FseEncodingTable::new(counts, log),
                Err(Error::CorruptData)
            ));
        }
        for entry in [
            FseEntry {
                symbol: 0,
                base: 0,
                bits: 6,
            },
            FseEntry {
                symbol: 0,
                base: 32,
                bits: 0,
            },
            FseEntry {
                symbol: 0,
                base: u16::MAX,
                bits: 1,
            },
            // Each row assignment is individually bounded, but collisions
            // make the relation ambiguous and must fail during inversion.
            FseEntry {
                symbol: 0,
                base: 0,
                bits: 5,
            },
        ] {
            assert!(matches!(
                FseEncodingTable::from_relation(5, |_| Ok(entry)),
                Err(Error::CorruptData)
            ));
        }
        // A noncolliding row for symbol zero misses successor 31; symbol one
        // independently covers all states. Publication must reject the hole.
        assert!(matches!(
            FseEncodingTable::from_relation(5, |state| Ok(if state == 31 {
                FseEntry {
                    symbol: 1,
                    base: 0,
                    bits: 5,
                }
            } else {
                FseEntry {
                    symbol: 0,
                    base: state as u16,
                    bits: 0,
                }
            })),
            Err(Error::CorruptData)
        ));
        for entry in [
            FseEntry {
                symbol: 0,
                base: 1,
                bits: 0,
            },
            FseEntry {
                symbol: 0,
                base: 0,
                bits: 1,
            },
        ] {
            assert!(matches!(
                FseEncodingTable::from_relation(0, |_| Ok(entry)),
                Err(Error::CorruptData)
            ));
        }
    }
}
