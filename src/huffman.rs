//! Checked Zstandard Huffman table construction and literals decoding.
//!
//! The format and ordering follow facebook/zstd v1.5.7. This implementation
//! stores positions as indices and uses fallible owned allocations.

use alloc::alloc::{Allocator, Global};
use alloc::vec::Vec;

use crate::bitstream::ReverseBits;
use crate::fse::decode_weights_in;
use crate::{Error, Result};

const MAX_TABLE_LOG: u8 = 12;
const MAX_SYMBOLS: usize = 256;

/// A plain initialized value; both table backends share its representation.
#[derive(Clone, Copy, Debug)]
struct DecodeEntry {
    symbol: u8,
    bits: u8,
}

/// An immutable, complete prefix lookup table owned by a frame decoder.
#[derive(Debug)]
pub(crate) struct HuffmanTable<A: Allocator = Global> {
    entries: Vec<DecodeEntry, A>,
    table_log: u8,
}

impl HuffmanTable<Global> {
    /// Read an inline description using the global allocator.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Global convenience for direct tests and benchmarks"
        )
    )]
    pub(crate) fn read(input: &[u8]) -> Result<(Self, usize)> {
        Self::read_in(input, Global)
    }

    /// Read a prepared dictionary's description using the global allocator.
    pub(crate) fn read_dictionary(input: &[u8]) -> Result<(Self, usize)> {
        Self::read_dictionary_in(input, Global)
    }
}

impl<A: Allocator> HuffmanTable<A> {
    pub(crate) fn retained_capacity(&self) -> usize {
        self.entries.capacity() * core::mem::size_of::<DecodeEntry>()
    }

    /// Copy validated per-symbol lengths for checked encoding-table setup.
    /// Every lookup entry for one symbol has the same bit count; absent symbols
    /// remain zero. No reference into this decoding table escapes the call.
    pub(crate) fn code_lengths(&self) -> [u8; MAX_SYMBOLS] {
        let mut lengths = [0; MAX_SYMBOLS];
        for entry in &self.entries {
            lengths[usize::from(entry.symbol)] = entry.bits;
        }
        lengths
    }

    /// Copy initialized entries into a fresh owner without assuming allocator
    /// equivalence. Failure leaves this immutable source table untouched.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "allocator-aware contexts will copy prepared tables"
        )
    )]
    pub(crate) fn try_clone_in<B: Allocator>(&self, allocator: B) -> Result<HuffmanTable<B>> {
        let mut entries = Vec::new_in(allocator);
        entries
            .try_reserve_exact(self.entries.len())
            .map_err(|_| Error::AllocationFailed)?;
        for &entry in &self.entries {
            entries.push(entry);
        }
        Ok(HuffmanTable {
            entries,
            table_log: self.table_log,
        })
    }
}

impl<A: Allocator + Clone> HuffmanTable<A> {
    /// Read weights and build a complete decoding table.
    ///
    /// Corresponds to upstream `HUF_readStats` and `HUF_readDTableX1_wksp`.
    /// The returned count includes the one-byte weights header.
    /// The retained table and temporary weights use independent owners from
    /// this allocator family. Clones allocate fresh blocks; existing storage
    /// never moves between allocator values. Borrowed non-Clone allocators work
    /// because the reference itself implements Clone.
    pub(crate) fn read_in(input: &[u8], allocator: A) -> Result<(Self, usize)> {
        Self::read_with_limit(input, 11, allocator)
    }

    /// Prepared dictionaries produced by the reference may use a twelve-bit
    /// table (including the official HTTP dictionary). Ordinary literal trees
    /// retain the frame decoder's eleven-bit construction limit.
    pub(crate) fn read_dictionary_in(input: &[u8], allocator: A) -> Result<(Self, usize)> {
        Self::read_with_limit(input, MAX_TABLE_LOG, allocator)
    }

    fn read_with_limit(input: &[u8], maximum_log: u8, allocator: A) -> Result<(Self, usize)> {
        let header = usize::from(*input.first().ok_or(Error::UnexpectedEof)?);
        let (mut weights, consumed) = if header >= 128 {
            let count = header - 127;
            let encoded_len = count.div_ceil(2);
            let packed = input.get(1..1 + encoded_len).ok_or(Error::UnexpectedEof)?;
            let mut weights = Vec::new_in(allocator.clone());
            weights
                .try_reserve_exact(count + 1)
                .map_err(|_| Error::AllocationFailed)?;
            for index in 0..count {
                let packed_byte = packed[index / 2];
                weights.push(if index % 2 == 0 {
                    packed_byte >> 4
                } else {
                    packed_byte & 15
                });
            }
            (weights, encoded_len + 1)
        } else {
            let encoded = input.get(1..1 + header).ok_or(Error::UnexpectedEof)?;
            (decode_weights_in(encoded, allocator.clone())?, header + 1)
        };
        if weights.is_empty() || weights.len() >= MAX_SYMBOLS {
            return Err(Error::CorruptData);
        }

        let mut weight_total = 0usize;
        let mut rank_counts = [0usize; MAX_TABLE_LOG as usize + 1];
        for &weight in &weights {
            if weight > maximum_log {
                return Err(Error::CorruptData);
            }
            rank_counts[usize::from(weight)] += 1;
            if weight != 0 {
                weight_total += 1usize << (weight - 1);
            }
        }
        if weight_total == 0 {
            return Err(Error::CorruptData);
        }
        let table_log = (usize::BITS - weight_total.leading_zeros()) as u8;
        if table_log > maximum_log {
            return Err(Error::CorruptData);
        }
        let table_size = 1usize << table_log;
        let remainder = table_size - weight_total;
        if !remainder.is_power_of_two() {
            return Err(Error::CorruptData);
        }
        let final_weight = (remainder.trailing_zeros() + 1) as u8;
        rank_counts[usize::from(final_weight)] += 1;
        if rank_counts[1] < 2 || rank_counts[1] % 2 != 0 {
            return Err(Error::CorruptData);
        }
        weights
            .try_reserve(1)
            .map_err(|_| Error::AllocationFailed)?;
        weights.push(final_weight);

        let mut entries = Vec::new_in(allocator);
        entries
            .try_reserve_exact(table_size)
            .map_err(|_| Error::AllocationFailed)?;
        for weight in 1..=table_log {
            let copies = 1usize << (weight - 1);
            let bits = table_log + 1 - weight;
            for (symbol, &symbol_weight) in weights.iter().enumerate() {
                if symbol_weight == weight {
                    let entry = DecodeEntry {
                        symbol: symbol as u8,
                        bits,
                    };
                    for _ in 0..copies {
                        entries.push(entry);
                    }
                }
            }
        }
        if entries.len() != table_size {
            return Err(Error::CorruptData);
        }
        Ok((Self { entries, table_log }, consumed))
    }
}

impl<A: Allocator> HuffmanTable<A> {
    /// Checked access has the same contract in both table implementations.
    #[inline]
    fn lookup(&self, index: usize) -> Result<DecodeEntry> {
        #[cfg(not(feature = "unsafe-huffman"))]
        {
            self.entries.get(index).copied().ok_or(Error::CorruptData)
        }
        #[cfg(feature = "unsafe-huffman")]
        {
            if index >= self.entries.len() {
                return Err(Error::CorruptData);
            }
            // SAFETY: The runtime check proves index is inside the initialized
            // Vec allocation. Its pointer is non-null and aligned for DecodeEntry,
            // and add(index) stays within that allocation. The shared self borrow
            // keeps storage and its exact allocator alive and prevents mutation
            // for the load. DecodeEntry
            // is Copy, so reading it neither moves ownership nor creates aliases
            // permitting mutation. No pointer or reference escapes this block.
            Ok(unsafe { *self.entries.as_ptr().add(index) })
        }
    }

    /// Decode exactly `output_len` symbols from one terminated reverse stream.
    ///
    /// Corresponds to upstream `HUF_decompress1X1_usingDTable_internal_body`.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Global convenience for direct tests and benchmarks"
        )
    )]
    pub(crate) fn decode_stream(&self, input: &[u8], output_len: usize) -> Result<Vec<u8>> {
        self.decode_stream_in(input, output_len, Global)
    }

    /// Decode with an independent output allocator; no table allocation or
    /// allocator clone is needed, and no borrow of this table enters the result.
    pub(crate) fn decode_stream_in<B: Allocator>(
        &self,
        input: &[u8],
        output_len: usize,
        allocator: B,
    ) -> Result<Vec<u8, B>> {
        let mut output = Vec::new_in(allocator);
        output
            .try_reserve_exact(output_len)
            .map_err(|_| Error::AllocationFailed)?;
        self.append_stream(input, output_len, &mut output)?;
        Ok(output)
    }

    /// Decode into an already reserved output allocation.
    fn append_stream<B: Allocator>(
        &self,
        input: &[u8],
        output_len: usize,
        output: &mut Vec<u8, B>,
    ) -> Result<()> {
        let mut bits = ReverseBits::new(input)?;
        for _ in 0..output_len {
            let available = bits.remaining().min(usize::from(self.table_log)) as u8;
            // A short final code is repeated across all possible suffixes in the
            // table. Zero-extension chooses one equivalent entry without reading
            // past the beginning of the encoded stream.
            let prefix = bits.peek_bits(available)?;
            let index = (prefix << (self.table_log - available)) as usize;
            let entry = self.lookup(index)?;
            if usize::from(entry.bits) > bits.remaining() {
                return Err(Error::CorruptData);
            }
            bits.read_bits(entry.bits)?;
            output.push(entry.symbol);
        }
        if bits.remaining() != 0 {
            return Err(Error::CorruptData);
        }
        Ok(())
    }
}

/// Decode one literals section, preserving the preceding table on failure.
///
/// Corresponds to upstream `ZSTD_decodeLiteralsBlock`. The returned byte count
/// includes the literals header, tree description, jump table, and all streams.
pub(crate) fn decode_literals(
    input: &[u8],
    previous: &mut Option<HuffmanTable>,
    max_output: usize,
) -> Result<(Vec<u8>, usize)> {
    decode_literals_in(input, previous, max_output, Global)
}

/// Decode literals with custom owned output and replacement-table storage.
pub(crate) fn decode_literals_in<A: Allocator + Clone>(
    input: &[u8],
    previous: &mut Option<HuffmanTable<A>>,
    max_output: usize,
    allocator: A,
) -> Result<(Vec<u8, A>, usize)> {
    decode_literals_with_dictionary_in::<A, Global>(input, previous, max_output, None, allocator)
}

/// A prepared dictionary supplies the initial table until this frame replaces
/// it. Borrowed immutable tables never become self-references in frame state.
pub(crate) fn decode_literals_with_dictionary(
    input: &[u8],
    previous: &mut Option<HuffmanTable>,
    max_output: usize,
    initial: Option<&HuffmanTable>,
) -> Result<(Vec<u8>, usize)> {
    decode_literals_with_dictionary_in(input, previous, max_output, initial, Global)
}

/// Prepared tables may use a different allocator from output and frame-local
/// state. A replacement table is published only after every stream succeeds.
/// Raw/RLE sections and failures leave the previous table unchanged.
pub(crate) fn decode_literals_with_dictionary_in<A: Allocator + Clone, B: Allocator>(
    input: &[u8],
    previous: &mut Option<HuffmanTable<A>>,
    max_output: usize,
    initial: Option<&HuffmanTable<B>>,
    allocator: A,
) -> Result<(Vec<u8, A>, usize)> {
    let first = *input.first().ok_or(Error::UnexpectedEof)?;
    let kind = first & 3;
    let size_format = (first >> 2) & 3;
    if kind <= 1 {
        let header_len = match size_format {
            0 | 2 => 1,
            1 => 2,
            3 => 3,
            _ => unreachable!(),
        };
        let header = read_header(input, header_len)?;
        let regenerated = (header >> if header_len == 1 { 3 } else { 4 }) as usize;
        validate_literals_size(regenerated, max_output)?;
        let encoded_len = if kind == 0 { regenerated } else { 1 };
        let encoded = input
            .get(header_len..header_len + encoded_len)
            .ok_or(Error::UnexpectedEof)?;
        let mut output = Vec::new_in(allocator);
        output
            .try_reserve_exact(regenerated)
            .map_err(|_| Error::AllocationFailed)?;
        if kind == 0 {
            for &byte in encoded {
                output.push(byte);
            }
        } else {
            for _ in 0..regenerated {
                output.push(encoded[0]);
            }
        }
        return Ok((output, header_len + encoded_len));
    }

    let (header_len, size_bits) = match size_format {
        0 | 1 => (3usize, 10u8),
        2 => (4, 14),
        3 => (5, 18),
        _ => unreachable!(),
    };
    let header = read_header(input, header_len)?;
    let regenerated = ((header >> 4) & ((1u64 << size_bits) - 1)) as usize;
    let compressed = (header >> (4 + size_bits)) as usize;
    validate_literals_size(regenerated, max_output)?;
    let four_streams = size_format != 0;
    if four_streams && regenerated < 6 {
        return Err(Error::CorruptData);
    }
    let encoded = input
        .get(header_len..header_len + compressed)
        .ok_or(Error::UnexpectedEof)?;
    let output = if kind == 2 {
        let (table, table_size) = HuffmanTable::read_in(encoded, allocator.clone())?;
        let streams = encoded.get(table_size..).ok_or(Error::UnexpectedEof)?;
        let output = decode_streams_in(&table, streams, regenerated, four_streams, allocator)?;
        *previous = Some(table);
        output
    } else {
        // The two tables can have different allocator types. Select explicitly
        // rather than coercing either owner or copying prepared storage.
        if let Some(table) = previous.as_ref() {
            decode_streams_in(table, encoded, regenerated, four_streams, allocator)?
        } else if let Some(table) = initial {
            decode_streams_in(table, encoded, regenerated, four_streams, allocator)?
        } else {
            return Err(Error::CorruptData);
        }
    };
    Ok((output, header_len + compressed))
}

fn read_header(input: &[u8], bytes: usize) -> Result<u64> {
    let encoded = input.get(..bytes).ok_or(Error::UnexpectedEof)?;
    let mut header = 0u64;
    for (index, &byte) in encoded.iter().enumerate() {
        header |= u64::from(byte) << (index * 8);
    }
    Ok(header)
}

fn validate_literals_size(regenerated: usize, limit: usize) -> Result<()> {
    // A literals section belongs to a block, whose format maximum is 128 KiB.
    if regenerated > 128 * 1024 {
        return Err(Error::CorruptData);
    }
    if regenerated > limit {
        return Err(Error::ResourceLimit);
    }
    Ok(())
}

/// Decode independent quarter-sized streams using checked jump-table ranges.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Global convenience for direct entropy tests")
)]
pub(crate) fn decode_streams<B: Allocator>(
    table: &HuffmanTable<B>,
    input: &[u8],
    output_len: usize,
    four_streams: bool,
) -> Result<Vec<u8>> {
    decode_streams_in(table, input, output_len, four_streams, Global)
}

/// Decode into one output allocation independent of the borrowed table owner.
pub(crate) fn decode_streams_in<A: Allocator, B: Allocator>(
    table: &HuffmanTable<B>,
    input: &[u8],
    output_len: usize,
    four_streams: bool,
    allocator: A,
) -> Result<Vec<u8, A>> {
    if !four_streams {
        return table.decode_stream_in(input, output_len, allocator);
    }
    if input.len() < 10 || output_len < 6 {
        return Err(Error::CorruptData);
    }
    let segment_len = output_len.div_ceil(4);
    let final_len = output_len
        .checked_sub(3 * segment_len)
        .ok_or(Error::CorruptData)?;
    let lengths = [
        usize::from(u16::from_le_bytes([input[0], input[1]])),
        usize::from(u16::from_le_bytes([input[2], input[3]])),
        usize::from(u16::from_le_bytes([input[4], input[5]])),
    ];
    let final_start = 6 + lengths.iter().sum::<usize>();
    if final_start >= input.len() || lengths.contains(&0) {
        return Err(Error::CorruptData);
    }
    let mut output = Vec::new_in(allocator);
    output
        .try_reserve_exact(output_len)
        .map_err(|_| Error::AllocationFailed)?;
    let mut start = 6;
    for length in lengths {
        let end = start + length;
        let stream = input.get(start..end).ok_or(Error::CorruptData)?;
        table.append_stream(stream, segment_len, &mut output)?;
        start = end;
    }
    let stream = input.get(start..).ok_or(Error::CorruptData)?;
    table.append_stream(stream, final_len, &mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn tracking() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig::default()).unwrap()
    }

    fn same_entries<A: Allocator, B: Allocator>(left: &HuffmanTable<A>, right: &HuffmanTable<B>) {
        assert_eq!(left.table_log, right.table_log);
        assert_eq!(left.code_lengths(), right.code_lengths());
        assert_eq!(left.entries.len(), right.entries.len());
        for index in 0..left.entries.len() {
            let a = left.lookup(index).unwrap();
            let b = right.lookup(index).unwrap();
            assert_eq!((a.symbol, a.bits), (b.symbol, b.bits));
        }
        assert!(left.lookup(left.entries.len()).is_err());
        assert!(left.lookup(usize::MAX).is_err());
    }

    #[test]
    fn custom_tables_and_fallible_copies_keep_distinct_non_clone_owners() {
        for description in [&[128, 0x10][..], &[4, 0x10, 0x3f, 3, 4][..]] {
            let root = tracking();
            let monitor = root.monitor();
            let table_owner = root.clone();
            let table_id = table_owner.identity();
            let (table, consumed) = HuffmanTable::read_in(description, table_owner).unwrap();
            assert_eq!(consumed, description.len());
            assert_eq!(table.entries.allocator().identity(), table_id);
            let (global, _) = HuffmanTable::read(description).unwrap();
            same_entries(&table, &global);
            let copy_owner = root.clone().into_non_clone();
            let copy_id = copy_owner.identity();
            assert_ne!(copy_id, table_id);
            let copy = table.try_clone_in(copy_owner).unwrap();
            same_entries(&copy, &table);
            assert_eq!(copy.entries.allocator().identity(), copy_id);
            assert_ne!(copy.entries.as_ptr(), table.entries.as_ptr());
            drop(root);
            drop(table);
            assert_eq!(
                copy.decode_stream(&[0x16], 4).unwrap(),
                global.decode_stream(&[0x16], 4).unwrap()
            );
            drop(copy);
            monitor.assert_released();
        }

        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let (table, _) = HuffmanTable::read_in(&[128, 0x10], &owner).unwrap();
        assert_eq!(table.entries.allocator().identity(), owner.identity());
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(table);
        drop(owner);
        monitor.assert_released();
    }

    #[test]
    fn custom_table_construction_and_copy_release_every_failed_request() {
        for (description, dictionary) in [
            (&[128, 0x10][..], false),
            (&[4, 0x10, 0x3f, 3, 4][..], false),
            (&[139, 0x11, 0x23, 0x45, 0x67, 0x89, 0xab][..], true),
        ] {
            let owner = tracking();
            let monitor = owner.monitor();
            let (table, consumed) = if dictionary {
                HuffmanTable::read_dictionary_in(description, owner)
            } else {
                HuffmanTable::read_in(description, owner)
            }
            .unwrap();
            assert_eq!(consumed, description.len());
            let attempts = monitor.snapshot().attempts;
            assert!(attempts >= 2);
            drop(table);
            monitor.assert_released();
            for ordinal in 1..=attempts {
                let owner = TrackingAllocator::new(AllocatorConfig {
                    fail_at: NonZeroU64::new(ordinal),
                    ..AllocatorConfig::default()
                })
                .unwrap();
                let monitor = owner.monitor();
                let result = if dictionary {
                    HuffmanTable::read_dictionary_in(description, owner)
                } else {
                    HuffmanTable::read_in(description, owner)
                };
                assert!(
                    matches!(result, Err(Error::AllocationFailed)),
                    "request {ordinal}"
                );
                assert_eq!(monitor.snapshot().attempts, ordinal);
                drop(result);
                monitor.assert_released();
            }
        }
        let (source, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        assert!(monitor.fail_next());
        assert!(matches!(
            source.try_clone_in(owner),
            Err(Error::AllocationFailed)
        ));
        assert_eq!(source.decode_stream(&[0x16], 4).unwrap(), [0, 1, 1, 0]);
        monitor.assert_released();
    }

    #[test]
    fn custom_stream_outputs_are_owned_independently_and_fail_fallibly() {
        let (table, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        for (encoded, length, four) in [
            (&[0x16][..], 4, false),
            (&[1, 0, 1, 0, 1, 0, 5, 6, 4, 7][..], 8, true),
            (&[1, 0, 1, 0, 1, 0, 5, 6, 4, 1][..], 6, true),
        ] {
            let expected = decode_streams(&table, encoded, length, four).unwrap();
            let owner = tracking().into_non_clone();
            let identity = owner.identity();
            let monitor = owner.monitor();
            assert!(monitor.fail_next());
            assert!(matches!(
                decode_streams_in(&table, encoded, length, four, &owner),
                Err(Error::AllocationFailed)
            ));
            assert_eq!(monitor.snapshot().live_blocks, 0);
            let output = decode_streams_in(&table, encoded, length, four, owner).unwrap();
            assert_eq!(output.allocator().identity(), identity);
            assert_eq!(output.as_slice(), expected);
            assert_eq!(monitor.snapshot().attempts, 2);
            assert_eq!(monitor.snapshot().identities_created, 1);
            drop(output);
            monitor.assert_released();
        }
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let output = table.decode_stream_in(&[1], 0, owner).unwrap();
        assert!(output.is_empty());
        assert_eq!(monitor.snapshot().attempts, 0);
        drop(table);
        drop(output);
        monitor.assert_released();
    }

    #[test]
    fn custom_raw_and_rle_literals_use_only_the_output_owner() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        for size in [0usize, 1, 31, 32, 255, 4095, 4096, 131072] {
            let (header_len, format, shift) = if size < 32 {
                (1, 0u32, 3)
            } else if size < 4096 {
                (2, 4, 4)
            } else {
                (3, 12, 4)
            };
            for kind in 0..=1 {
                let header = ((size as u32) << shift) | format | kind;
                let mut encoded = header.to_le_bytes()[..header_len].to_vec();
                encoded.resize(encoded.len() + if kind == 0 { size } else { 1 }, b'x');
                let before = monitor.snapshot();
                let mut previous = None;
                let (output, consumed) =
                    decode_literals_in(&encoded, &mut previous, size, &owner).unwrap();
                assert_eq!(consumed, encoded.len());
                assert_eq!(output.len(), size);
                assert!(output.iter().all(|&byte| byte == b'x'));
                assert_eq!(output.allocator().identity(), owner.identity());
                assert!(previous.is_none());
                assert_eq!(
                    monitor.snapshot().attempts - before.attempts,
                    u64::from(size != 0)
                );
                drop(output);
                monitor.assert_released();
            }
        }
        assert_eq!(monitor.snapshot().identities_created, 1);
    }

    #[test]
    fn custom_literal_allocation_failures_do_not_publish_replacement_tables() {
        let cases = [
            compressed_literals(&[128, 0x10, 0x16], 4, 0, 2),
            compressed_literals(&[4, 0x10, 0x3f, 3, 4, 0x16], 4, 0, 2),
            compressed_literals(&[128, 0x10, 1, 0, 1, 0, 1, 0, 5, 6, 4, 7], 8, 1, 2),
            compressed_literals(&[0x16], 4, 0, 3),
            alloc::vec![4 << 3, 0, 1, 1, 0],
            alloc::vec![(4 << 3) | 1, 7],
        ];
        for encoded in &cases {
            let baseline_owner = tracking().into_non_clone();
            let baseline_monitor = baseline_owner.monitor();
            let mut previous = Some(
                HuffmanTable::read_in(&[128, 0x10], &baseline_owner)
                    .unwrap()
                    .0,
            );
            let before = baseline_monitor.snapshot().attempts;
            let output = decode_literals_in(encoded, &mut previous, 8, &baseline_owner)
                .unwrap()
                .0;
            let requests = baseline_monitor.snapshot().attempts - before;
            assert!(requests > 0);
            drop(output);
            drop(previous);
            baseline_monitor.assert_released();
            for ordinal in 1..=requests {
                let owner = tracking().into_non_clone();
                let monitor = owner.monitor();
                let mut previous = Some(HuffmanTable::read_in(&[128, 0x10], &owner).unwrap().0);
                let pointer = previous.as_ref().unwrap().entries.as_ptr();
                let before = monitor.snapshot();
                monitor.set_fail_at(NonZeroU64::new(before.attempts + ordinal));
                assert!(matches!(
                    decode_literals_in(encoded, &mut previous, 8, &owner),
                    Err(Error::AllocationFailed)
                ));
                assert_eq!(previous.as_ref().unwrap().entries.as_ptr(), pointer);
                assert_eq!(
                    previous
                        .as_ref()
                        .unwrap()
                        .decode_stream(&[0x16], 4)
                        .unwrap(),
                    [0, 1, 1, 0]
                );
                let after = monitor.snapshot();
                assert_eq!(after.attempts, before.attempts + ordinal);
                assert_eq!(after.live_blocks, before.live_blocks);
                assert_eq!(after.live_bytes, before.live_bytes);
                monitor.clear_failure();
                decode_literals_in(encoded, &mut previous, 8, &owner).unwrap();
                drop(previous);
                monitor.assert_released();
            }
        }
    }

    #[test]
    fn custom_dictionary_table_and_frame_storage_have_independent_allocator_types() {
        let (global, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        let dictionary_owner = tracking().into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let initial = global.try_clone_in(dictionary_owner).unwrap();
        let dictionary_before = dictionary_monitor.snapshot();
        let owner = tracking();
        let monitor = owner.monitor();
        let mut previous = None;
        let repeated = compressed_literals(&[0x16], 4, 0, 3);
        let decoded =
            decode_literals_with_dictionary_in(&repeated, &mut previous, 4, Some(&initial), &owner)
                .unwrap()
                .0;
        assert_eq!(decoded.as_slice(), [0, 1, 1, 0]);
        assert!(previous.is_none());
        drop(decoded);
        let replacement = compressed_literals(&[4, 0x10, 0x3f, 3, 4, 0x16], 4, 0, 2);
        decode_literals_with_dictionary_in(&replacement, &mut previous, 4, Some(&initial), &owner)
            .unwrap();
        let decoded =
            decode_literals_with_dictionary_in(&repeated, &mut previous, 4, Some(&initial), &owner)
                .unwrap()
                .0;
        assert_eq!(decoded.as_slice(), [1, 2, 2, 1]);
        drop(decoded);
        assert_eq!(
            dictionary_monitor.snapshot().attempts,
            dictionary_before.attempts
        );
        drop(previous);
        monitor.assert_released();
        assert_eq!(initial.decode_stream(&[0x16], 4).unwrap(), [0, 1, 1, 0]);
        drop(initial);
        dictionary_monitor.assert_released();
    }

    #[test]
    fn custom_allocators_decode_all_thirty_official_one_and_four_stream_fixtures() {
        let mut records = crate::cursor::ByteCursor::new(include_bytes!(
            "../tests/upstream/entropy-data/huffman.bin"
        ));
        assert_eq!(records.take(8).unwrap(), b"ZHUF0001");
        assert_eq!(records.little_endian(4).unwrap(), 30);
        let mut weight_modes = [false; 2];
        for case in 0..30 {
            let length = records.little_endian(4).unwrap() as usize;
            let description_len = records.little_endian(4).unwrap() as usize;
            let single_len = records.little_endian(4).unwrap() as usize;
            let four_len = records.little_endian(4).unwrap() as usize;
            let expected = records.take(length).unwrap();
            let description = records.take(description_len).unwrap();
            let single = records.take(single_len).unwrap();
            let four = records.take(four_len).unwrap();
            weight_modes[usize::from(description[0] >= 128)] = true;
            let owner = tracking().into_non_clone();
            let monitor = owner.monitor();
            let (table, consumed) = HuffmanTable::read_in(description, &owner).unwrap();
            assert_eq!(consumed, description.len());
            let (global, _) = HuffmanTable::read(description).unwrap();
            same_entries(&table, &global);
            for (encoded, four_streams) in [(single, false), (four, true)] {
                let output =
                    decode_streams_in(&table, encoded, length, four_streams, &owner).unwrap();
                assert_eq!(
                    output.as_slice(),
                    expected,
                    "case {case}, four={four_streams}"
                );
                drop(output);
                assert!(
                    decode_streams_in(
                        &table,
                        &encoded[..encoded.len() - 1],
                        length,
                        four_streams,
                        &owner
                    )
                    .is_err()
                );
            }
            assert!(table.decode_stream_in(single, length - 1, &owner).is_err());
            assert!(table.decode_stream_in(single, length + 1, &owner).is_err());
            for end in 0..description.len() {
                assert!(HuffmanTable::read_in(&description[..end], &owner).is_err());
            }
            let mut payload = description.to_vec();
            payload.extend_from_slice(four);
            let section = compressed_literals(&payload, length, 3, 2);
            let mut previous = None;
            let (output, consumed) =
                decode_literals_in(&section, &mut previous, length, &owner).unwrap();
            assert_eq!(output.as_slice(), expected);
            assert_eq!(consumed, section.len());
            drop(output);
            let repeated = compressed_literals(four, length, 3, 3);
            let output = decode_literals_in(&repeated, &mut previous, length, &owner)
                .unwrap()
                .0;
            assert_eq!(output.as_slice(), expected);
            drop(output);
            drop(previous);
            drop(table);
            assert_eq!(monitor.snapshot().identities_created, 1);
            monitor.assert_released();
        }
        assert_eq!(weight_modes, [true, true]);
        assert!(records.remaining().is_empty());
    }

    #[test]
    fn custom_malformed_tables_and_four_stream_bounds_release_all_scratch() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        for description in [
            &[][..],
            &[128][..],
            &[128, 0][..],
            &[128, 0xc0][..],
            &[130, 0x22, 0x10][..],
            &[4, 0x10, 0x3f, 3, 0][..],
            &[139, 0x11, 0x23, 0x45, 0x67, 0x89, 0xab][..],
        ] {
            assert!(HuffmanTable::read_in(description, &owner).is_err());
            monitor.assert_released();
        }
        let (table, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        let before = monitor.snapshot();
        for encoded in [
            &[][..],
            &[1; 9][..],
            &[255, 255, 255, 255, 255, 255, 1, 1, 1, 1][..],
            &[0, 0, 1, 0, 1, 0, 1, 1, 1, 1][..],
        ] {
            assert!(decode_streams_in(&table, encoded, 8, true, &owner).is_err());
        }
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        let four = [1, 0, 1, 0, 1, 0, 5, 6, 4, 7];
        assert!(matches!(
            decode_streams_in(&table, &four, 5, true, &owner),
            Err(Error::CorruptData)
        ));
        assert!(matches!(
            decode_streams_in(&table, &four, usize::MAX, true, &owner),
            Err(Error::AllocationFailed)
        ));
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        monitor.assert_released();
    }

    #[test]
    fn direct_weights_and_two_symbol_stream() {
        let (table, consumed) = HuffmanTable::read(&[128, 0x10, 99]).unwrap();
        assert_eq!(consumed, 2);
        assert_eq!(table.decode_stream(&[0x16], 4).unwrap(), [0, 1, 1, 0]);
    }

    #[test]
    fn fse_weights_build_a_huffman_table() {
        // An FSE distribution [16, 16] yields explicit weights [0, 1].
        // The implicit final weight is 1, so only byte values 1 and 2 occur.
        let (table, consumed) = HuffmanTable::read(&[4, 0x10, 0x3f, 3, 4]).unwrap();
        assert_eq!(consumed, 5);
        assert_eq!(table.decode_stream(&[0x16], 4).unwrap(), [1, 2, 2, 1]);
    }

    #[test]
    fn mixed_lengths_decode_short_final_code() {
        // Weights [1, 1, 2, 3] yield codes 000, 001, 01, 1.
        let (table, _) = HuffmanTable::read(&[130, 0x11, 0x20]).unwrap();
        assert_eq!(table.decode_stream(&[0x0b, 0x02], 4).unwrap(), [0, 1, 2, 3]);
        assert_eq!(table.decode_stream(&[0x03], 1).unwrap(), [3]);
    }

    #[test]
    fn maximum_eleven_bit_tree_decodes_long_and_short_codes() {
        // Explicit weights [1, 1, 2, ..., 10] imply final weight 11.
        // Symbol 0 has eleven zero bits; symbol 11 has the one-bit code 1.
        let (table, consumed) =
            HuffmanTable::read(&[138, 0x11, 0x23, 0x45, 0x67, 0x89, 0xa0]).unwrap();
        assert_eq!(consumed, 7);
        assert_eq!(table.table_log, 11);
        assert_eq!(table.entries.len(), 2048);
        assert_eq!(table.decode_stream(&[0x01, 0x10], 2).unwrap(), [0, 11]);
        assert_eq!(table.decode_stream(&[0, 0x08], 1).unwrap(), [0]);
        assert_eq!(table.decode_stream(&[3], 1).unwrap(), [11]);
        assert!(table.decode_stream(&[0, 0x04], 1).is_err());
        assert!(table.lookup(2048).is_err());
    }

    #[test]
    fn malformed_tree_fixtures_reject_excess_depth_and_missing_fse_marker() {
        // Every explicit weight is individually legal, but their distribution
        // would require a forbidden twelve-bit tree and implicit weight 12.
        let too_deep = [139, 0x11, 0x23, 0x45, 0x67, 0x89, 0xab];
        assert!(matches!(
            HuffmanTable::read(&too_deep),
            Err(Error::CorruptData)
        ));
        // The FSE normalized distribution is complete, but its stream has no
        // terminating marker. A previous table must not hide a bad new table.
        let broken_fse = [4, 0x10, 0x3f, 3, 0];
        assert!(HuffmanTable::read(&broken_fse).is_err());
        let (old, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        let mut previous = Some(old);
        let malformed = compressed_literals(&broken_fse, 2, 0, 2);
        assert!(decode_literals(&malformed, &mut previous, 2).is_err());
        assert_eq!(
            previous.unwrap().decode_stream(&[0x16], 4).unwrap(),
            [0, 1, 1, 0]
        );
        // A four-stream jump table cannot describe ranges beyond its payload,
        // even when each individual encoded length fits its u16 field.
        let huge_jumps = [128, 0x10, 255, 255, 255, 255, 255, 255, 1, 1, 1, 1];
        let malformed = compressed_literals(&huge_jumps, 8, 1, 2);
        assert!(decode_literals(&malformed, &mut None, 8).is_err());
    }

    #[test]
    fn invalid_weights_and_truncation_are_rejected() {
        for input in [
            &[][..],
            &[128][..],
            &[128, 0][..],
            &[128, 0xc0][..],
            &[130, 0x22, 0x10][..],
        ] {
            assert!(HuffmanTable::read(input).is_err(), "input: {input:?}");
        }
    }

    #[test]
    fn stream_must_end_at_declared_symbol_count() {
        let (table, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        assert!(table.decode_stream(&[0x16], 3).is_err());
        assert!(table.decode_stream(&[0x16], 5).is_err());
        assert!(table.decode_stream(&[0], 1).is_err());
        assert!(table.decode_stream(&[], 0).is_err());
        assert_eq!(table.decode_stream(&[1], 0).unwrap(), []);
        assert!(table.lookup(2).is_err());
    }

    fn compressed_literals(payload: &[u8], output_len: usize, format: u8, kind: u8) -> Vec<u8> {
        let size_bits = match format {
            0 | 1 => 10,
            2 => 14,
            3 => 18,
            _ => panic!("bad test format"),
        };
        let header_len = match format {
            0 | 1 => 3,
            2 => 4,
            3 => 5,
            _ => unreachable!(),
        };
        let header = u64::from(kind)
            | (u64::from(format) << 2)
            | ((output_len as u64) << 4)
            | ((payload.len() as u64) << (4 + size_bits));
        let mut encoded = header.to_le_bytes()[..header_len].to_vec();
        encoded.extend_from_slice(payload);
        encoded
    }

    #[test]
    fn raw_and_rle_literals_cover_header_widths() {
        for size in [0usize, 1, 31, 32, 255, 4095, 4096, 131072] {
            let (header_len, format, shift) = if size < 32 {
                (1, 0u32, 3)
            } else if size < 4096 {
                (2, 4, 4)
            } else {
                (3, 12, 4)
            };
            for kind in 0..=1 {
                let header = ((size as u32) << shift) | format | kind;
                let mut encoded = header.to_le_bytes()[..header_len].to_vec();
                encoded.resize(encoded.len() + if kind == 0 { size } else { 1 }, b'x');
                let consumed = encoded.len();
                encoded.push(0xff); // The following sequences section is not consumed.
                let (decoded, read) = decode_literals(&encoded, &mut None, size).unwrap();
                assert_eq!(decoded.len(), size);
                assert!(decoded.iter().all(|&byte| byte == b'x'));
                assert_eq!(read, consumed);
            }
        }
    }

    #[test]
    fn compressed_and_treeless_literals_reuse_table() {
        let encoded = compressed_literals(&[128, 0x10, 0x16], 4, 0, 2);
        let mut previous = None;
        let (decoded, consumed) = decode_literals(&encoded, &mut previous, 4).unwrap();
        assert_eq!(decoded, [0, 1, 1, 0]);
        assert_eq!(consumed, encoded.len());
        let repeated = compressed_literals(&[0x19], 4, 0, 3);
        assert_eq!(
            decode_literals(&repeated, &mut previous, 4).unwrap().0,
            [1, 0, 0, 1]
        );
        assert!(decode_literals(&repeated, &mut None, 4).is_err());
        decode_literals(&[0], &mut previous, 0).unwrap();
        assert!(previous.is_some(), "raw literals must preserve the table");
        decode_literals(&[1, 0], &mut previous, 0).unwrap();
        assert!(previous.is_some(), "RLE literals must preserve the table");
    }

    #[test]
    fn prepared_literal_table_is_borrowed_without_populating_owned_frame_state() {
        let (initial, _) = HuffmanTable::read_dictionary(&[128, 0x10]).unwrap();
        let encoded = compressed_literals(&[0x19], 4, 0, 3);
        let mut previous = None;
        for _ in 0..2 {
            assert_eq!(
                decode_literals_with_dictionary(&encoded, &mut previous, 4, Some(&initial))
                    .unwrap()
                    .0,
                [1, 0, 0, 1]
            );
            assert!(previous.is_none());
        }
        let replacement = compressed_literals(&[128, 0x10, 0x16], 4, 0, 2);
        decode_literals_with_dictionary(&replacement, &mut previous, 4, Some(&initial)).unwrap();
        assert!(previous.is_some());
        assert_eq!(
            decode_literals(&encoded, &mut previous, 4).unwrap().0,
            [1, 0, 0, 1]
        );
    }

    #[test]
    fn four_stream_literals_cover_compressed_header_widths() {
        let payload = [128, 0x10, 1, 0, 1, 0, 1, 0, 5, 6, 4, 7];
        for format in 1..=3 {
            let encoded = compressed_literals(&payload, 8, format, 2);
            let (decoded, consumed) = decode_literals(&encoded, &mut None, 8).unwrap();
            assert_eq!(decoded, [0, 1, 1, 0, 0, 0, 1, 1]);
            assert_eq!(consumed, encoded.len());
        }
        // Six literals split as 2/2/2/0; the last stream has only its marker.
        let payload = [128, 0x10, 1, 0, 1, 0, 1, 0, 5, 6, 4, 1];
        let encoded = compressed_literals(&payload, 6, 1, 2);
        assert_eq!(
            decode_literals(&encoded, &mut None, 6).unwrap().0,
            [0, 1, 1, 0, 0, 0]
        );
    }

    #[test]
    fn literals_reject_limits_truncation_and_bad_jump_tables() {
        assert_eq!(
            decode_literals(&[0x10, 1, 2], &mut None, 1).unwrap_err(),
            Error::ResourceLimit
        );
        assert!(decode_literals(&[0x10, 1], &mut None, 2).is_err());
        assert!(decode_literals(&[1], &mut None, 0).is_err());
        let encoded = compressed_literals(&[128, 0x10, 0x16], 4, 0, 2);
        for end in 0..encoded.len() {
            assert!(decode_literals(&encoded[..end], &mut None, 4).is_err());
        }
        for first_length in [0, 5, 255] {
            let payload = [128, 0x10, first_length, 0, 1, 0, 1, 0, 5, 6, 4, 7];
            let encoded = compressed_literals(&payload, 8, 1, 2);
            assert!(decode_literals(&encoded, &mut None, 8).is_err());
        }
        let encoded = compressed_literals(&[128, 0x10], 5, 1, 2);
        assert!(decode_literals(&encoded, &mut None, 5).is_err());
        let invalid_size = ((131073u32 << 4) | 12).to_le_bytes();
        assert_eq!(
            decode_literals(&invalid_size[..3], &mut None, usize::MAX).unwrap_err(),
            Error::CorruptData
        );
    }

    #[test]
    fn failed_compressed_literals_preserve_previous_table() {
        let (table, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        let mut previous = Some(table);
        let broken = compressed_literals(&[130, 0x11, 0x20, 0], 1, 0, 2);
        assert!(decode_literals(&broken, &mut previous, 1).is_err());
        assert_eq!(
            previous.unwrap().decode_stream(&[0x16], 4).unwrap(),
            [0, 1, 1, 0]
        );
    }
}
