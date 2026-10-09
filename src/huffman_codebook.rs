//! Checked canonical Huffman codes paired with exact serialized descriptions.
//!
//! Codewords are plain initialized values. The codebook owns its fixed alphabet
//! inline; it stores neither source pointers nor references into decoding tables.

use alloc::alloc::{Allocator, Global};
use alloc::vec::Vec;

use crate::bitwriter::BitWriter;
use crate::frame::MAX_BLOCK_SIZE;
use crate::huffman::HuffmanTable;
use crate::{Error, Result};

const SYMBOLS: usize = 256;
const DICTIONARY_MAX_LOG: u8 = 12;

/// An ordinary numeric codeword, shared by every future accessor backend.
/// A zero bit count is the internal representation of an absent symbol.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Codeword {
    pub(crate) value: u16,
    pub(crate) bits: u8,
}

/// A complete canonical prefix code with explicit absent-symbol entries.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HuffmanCodebook {
    entries: [Codeword; SYMBOLS],
    table_log: u8,
}

impl HuffmanCodebook {
    /// Assign codes by ascending weight and then ascending symbol.
    ///
    /// Corresponds to the canonical assignment in `HUF_readCTable` and
    /// `HUF_buildCTableFromTree`. Zero lengths represent absent symbols; the
    /// nonzero lengths must have complete Kraft mass and at least two symbols.
    /// The explicit limit distinguishes new inline trees (eleven) from reused
    /// prepared dictionary trees (twelve). Construction allocates no heap storage.
    pub(crate) fn from_lengths(lengths: &[u8; SYMBOLS], maximum_log: u8) -> Result<Self> {
        if !(1..=DICTIONARY_MAX_LOG).contains(&maximum_log) {
            return Err(Error::InvalidParameter);
        }
        let table_log = *lengths.iter().max().ok_or(Error::InvalidParameter)?;
        if table_log == 0 || table_log > maximum_log {
            return Err(Error::InvalidParameter);
        }
        let table_size = 1_u32 << table_log;
        let mut mass = 0_u32;
        let mut present = 0;
        for &bits in lengths {
            if bits != 0 {
                // table_log is the validated maximum, so the shift is 0..12.
                mass = mass
                    .checked_add(1 << (table_log - bits))
                    .filter(|&value| value <= table_size)
                    .ok_or(Error::InvalidParameter)?;
                present += 1;
            }
        }
        if present < 2 || mass != table_size {
            return Err(Error::InvalidParameter);
        }

        let mut entries = [Codeword::default(); SYMBOLS];
        let mut rank_start = 0_u32;
        for bits in (1..=table_log).rev() {
            let copies = 1_u32 << (table_log - bits);
            for (symbol, &length) in lengths.iter().enumerate() {
                if length != bits {
                    continue;
                }
                if !rank_start.is_multiple_of(copies) {
                    return Err(Error::InvalidParameter);
                }
                let value = rank_start / copies;
                if value >= 1 << bits {
                    return Err(Error::InvalidParameter);
                }
                entries[symbol] = Codeword {
                    // value is below 2^bits, and bits is at most twelve.
                    value: value as u16,
                    bits,
                };
                rank_start = rank_start
                    .checked_add(copies)
                    .filter(|&value| value <= table_size)
                    .ok_or(Error::InvalidParameter)?;
            }
        }
        if rank_start != table_size {
            return Err(Error::InvalidParameter);
        }
        Ok(Self { entries, table_log })
    }

    /// Copy the canonical relation from a validated immutable decoding table.
    /// No borrow survives the call; dictionary tables may retain twelve-bit codes.
    pub(crate) fn from_decoding_table<A: Allocator>(table: &HuffmanTable<A>) -> Result<Self> {
        Self::from_lengths(&table.code_lengths(), DICTIONARY_MAX_LOG)
    }

    #[cfg(test)]
    pub(crate) fn table_log(&self) -> u8 {
        self.table_log
    }

    /// Return only a present symbol's code. Missing symbols cannot emit zero bits.
    pub(crate) fn code(&self, symbol: u8) -> Result<Codeword> {
        let index = usize::from(symbol);
        if index >= self.entries.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-huffman-codebook"))]
        let entry = self.entries[index];
        #[cfg(feature = "unsafe-huffman-codebook")]
        // SAFETY: The runtime guard proves index is within the initialized
        // 256-entry array; converting a u8 also bounds it independently. The
        // array owns properly aligned Codeword values and cannot be resized.
        // The shared self borrow keeps that storage alive and prevents mutation
        // for the entire access. get_unchecked therefore returns a valid shared
        // reference within the array, and dereferencing copies an initialized
        // Codeword without moving ownership or creating a mutable alias. No
        // reference or pointer escapes this block. Symbol presence is checked
        // afterward identically in both backends, never assumed from the input.
        let entry = unsafe { *self.entries.get_unchecked(index) };
        if entry.bits == 0 {
            Err(Error::CorruptData)
        } else {
            Ok(entry)
        }
    }

    /// Test table reuse eligibility without modifying the codebook or input.
    pub(crate) fn covers(&self, input: &[u8]) -> bool {
        input.iter().all(|&symbol| self.code(symbol).is_ok())
    }

    /// Encode one reverse-consumed stream and its terminal marker.
    ///
    /// Corresponds to `HUF_compress1X_usingCTable`. Coverage is checked before
    /// changing the caller-owned writer, so an absent symbol preserves its
    /// contents and open/closed state. On valid input, reset retains the writer's
    /// allocation; subsequent allocation failure may leave partial scratch
    /// output. Numeric codewords are appended little-endian without reversing
    /// their individual bits. Empty input still produces the marker byte `1`.
    pub(crate) fn encode_stream(&self, input: &[u8], writer: &mut BitWriter) -> Result<()> {
        if !self.covers(input) {
            return Err(Error::CorruptData);
        }
        writer.reset();
        for &symbol in input.iter().rev() {
            let code = self.code(symbol)?;
            writer.write(u32::from(code.value), code.bits)?;
        }
        writer.finish()
    }

    /// Encode four independently terminated streams after their jump table.
    ///
    /// Corresponds to `HUF_compress4X_usingCTable`. The format requires at least
    /// six literals; the block limit bounds each stream's length even for reused
    /// twelve-bit dictionary codes. Invalid sizes or absent symbols leave both
    /// caller-owned buffers unchanged. Successful reservation precedes reset;
    /// a later writer allocation failure may leave partial scratch output.
    pub(crate) fn encode_four_streams(
        &self,
        input: &[u8],
        writer: &mut BitWriter,
        output: &mut Vec<u8>,
    ) -> Result<()> {
        self.encode_four_streams_bounded(input, writer, output, MAX_BLOCK_SIZE)
    }

    /// Standalone entropy streams follow the upstream fuzz target's 256 KiB
    /// source bound. Frame literal sections continue to use the smaller limit.
    /// Actual quarter sizes, including markers, must fit the u16 jump fields.
    #[cfg(test)]
    pub(crate) fn encode_standalone_four_streams(
        &self,
        input: &[u8],
        writer: &mut BitWriter,
        output: &mut Vec<u8>,
    ) -> Result<()> {
        self.encode_four_streams_bounded(input, writer, output, 256 * 1024)
    }

    fn encode_four_streams_bounded(
        &self,
        input: &[u8],
        writer: &mut BitWriter,
        output: &mut Vec<u8>,
        source_limit: usize,
    ) -> Result<()> {
        if !(6..=source_limit).contains(&input.len()) {
            return Err(Error::InvalidParameter);
        }
        let quarter = input.len().div_ceil(4);
        let third_end = quarter.checked_mul(3).ok_or(Error::InvalidParameter)?;
        if third_end > input.len() {
            return Err(Error::InvalidParameter);
        }
        let ends = [quarter, quarter * 2, third_end, input.len()];
        let mut start = 0;
        let mut encoded_size = 6_usize;
        for (index, &end) in ends.iter().enumerate() {
            let bits = input[start..end]
                .iter()
                .try_fold(1_usize, |bits, &symbol| {
                    bits.checked_add(usize::from(self.code(symbol)?.bits))
                        .ok_or(Error::InvalidParameter)
                })?;
            let size = bits.div_ceil(8);
            if index < 3 && size > usize::from(u16::MAX) {
                return Err(Error::InvalidParameter);
            }
            encoded_size = encoded_size
                .checked_add(size)
                .ok_or(Error::InvalidParameter)?;
            start = end;
        }
        // Validate every symbol and exact jump before changing either buffer.
        // A large source using short codes may fit even when table_log's
        // worst-case bound exceeds u16, so that bound cannot reject a stream.
        // try_reserve's argument is additional to the current length. Retaining
        // an already larger output allocation must not request it a second time.
        output
            .try_reserve(encoded_size.saturating_sub(output.len()))
            .map_err(|_| Error::AllocationFailed)?;
        output.clear();
        output.resize(6, 0);
        let mut start = 0;
        for (stream_index, end) in ends.into_iter().enumerate() {
            self.encode_stream(&input[start..end], writer)?;
            let encoded = writer.as_bytes();
            if stream_index < 3 {
                let length = u16::try_from(encoded.len()).map_err(|_| Error::CorruptData)?;
                if length == 0 {
                    return Err(Error::CorruptData);
                }
                let jump = stream_index * 2;
                output[jump..jump + 2].copy_from_slice(&length.to_le_bytes());
            }
            output.extend_from_slice(encoded);
            start = end;
        }
        Ok(())
    }
}

/// A new inline table and the exact bytes that establish its codes for decoding.
/// Private fields prevent replacement of one half without revalidating the pair.
#[derive(Debug)]
pub(crate) struct BuiltLiteralTable<A: Allocator = Global> {
    codebook: HuffmanCodebook,
    description: Vec<u8, A>,
}

impl<A: Allocator> BuiltLiteralTable<A> {
    /// Validate and own one complete inline description, with no trailing bytes.
    ///
    /// Reconstructing from the selected description keeps fallback trees paired
    /// with their actual codes. `HuffmanTable::read` enforces the inline limit of
    /// eleven bits and performs fallible setup allocation. Dictionary reuse uses
    /// `HuffmanCodebook::from_decoding_table` instead of publishing a new header.
    /// Validation borrows the retained allocator temporarily; its scratch drops
    /// before the description moves, so even a non-Clone owner is supported.
    pub(crate) fn from_description(description: Vec<u8, A>) -> Result<Self> {
        let codebook = {
            let (table, consumed) = HuffmanTable::read_in(&description, description.allocator())?;
            if consumed != description.len() {
                return Err(Error::CorruptData);
            }
            HuffmanCodebook::from_decoding_table(&table)?
        };
        Ok(Self {
            codebook,
            description,
        })
    }

    pub(crate) fn codebook(&self) -> &HuffmanCodebook {
        &self.codebook
    }

    pub(crate) fn description(&self) -> &[u8] {
        &self.description
    }

    /// Transfer the validated codes after their matching description has been
    /// copied into a selected literal section. The setup bytes can be released.
    pub(crate) fn into_codebook(self) -> HuffmanCodebook {
        self.codebook
    }
}

/// A standalone description and its exact codes, permitting twelve-bit trees.
/// This separate type cannot be supplied where an inline literal table is needed.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct BuiltStandaloneTable<A: Allocator = Global> {
    codebook: HuffmanCodebook,
    description: Vec<u8, A>,
}

#[cfg(test)]
impl<A: Allocator> BuiltStandaloneTable<A> {
    /// Own exactly one checked standalone description with no trailing bytes.
    /// The dictionary reader shares standalone Huffman's twelve-bit tree limit;
    /// the inline `BuiltLiteralTable` constructor keeps its eleven-bit limit.
    /// The validation table borrows the description's allocator and is dropped
    /// before ownership moves, preserving the exact owner without cloning it.
    pub(crate) fn from_description(description: Vec<u8, A>) -> Result<Self> {
        let codebook = {
            let (table, consumed) =
                HuffmanTable::read_dictionary_in(&description, description.allocator())?;
            if consumed != description.len() {
                return Err(Error::CorruptData);
            }
            HuffmanCodebook::from_decoding_table(&table)?
        };
        Ok(Self {
            codebook,
            description,
        })
    }

    pub(crate) fn codebook(&self) -> &HuffmanCodebook {
        &self.codebook
    }

    pub(crate) fn description(&self) -> &[u8] {
        &self.description
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, NonCloneAllocator, TrackingAllocator};

    fn owned_description(
        bytes: &[u8],
    ) -> (
        Vec<u8, NonCloneAllocator>,
        zstdoxide_test_allocator::AllocationMonitor,
    ) {
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut description = Vec::new_in(owner);
        description.try_reserve_exact(bytes.len()).unwrap();
        for &byte in bytes {
            description.push(byte);
        }
        (description, monitor)
    }

    #[test]
    fn custom_paired_descriptions_validate_with_borrowed_non_clone_owners() {
        for bytes in [&[128, 0x10][..], &[4, 0x10, 0x3f, 3, 4][..]] {
            let (description, monitor) = owned_description(bytes);
            let pointer = description.as_ptr();
            let identity = description.allocator().identity();
            let paired = BuiltLiteralTable::from_description(description).unwrap();
            assert_eq!(paired.description(), bytes);
            assert_eq!(paired.description.as_ptr(), pointer);
            assert_eq!(paired.description.allocator().identity(), identity);
            let (global, _) = HuffmanTable::read(bytes).unwrap();
            assert_eq!(
                paired.codebook(),
                &HuffmanCodebook::from_decoding_table(&global).unwrap()
            );
            assert_eq!(monitor.snapshot().identities_created, 1);
            // Validation scratch is gone; only the original description remains.
            assert_eq!(monitor.snapshot().live_blocks, 1);
            let codes = paired.into_codebook();
            monitor.assert_released();
            assert_eq!(
                codes,
                HuffmanCodebook::from_decoding_table(&global).unwrap()
            );
        }
        let twelve = [139, 0x11, 0x23, 0x45, 0x67, 0x89, 0xab];
        let (description, monitor) = owned_description(&twelve);
        let pointer = description.as_ptr();
        let paired = BuiltStandaloneTable::from_description(description).unwrap();
        assert_eq!(paired.description.as_ptr(), pointer);
        assert_eq!(paired.description(), twelve);
        assert_eq!(paired.codebook().table_log(), 12);
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(paired);
        monitor.assert_released();
    }

    #[test]
    fn custom_paired_validation_releases_description_and_scratch_at_every_failure() {
        fn validate<A: Allocator>(description: Vec<u8, A>, standalone: bool) -> Result<()> {
            if standalone {
                BuiltStandaloneTable::from_description(description).map(drop)
            } else {
                BuiltLiteralTable::from_description(description).map(drop)
            }
        }
        for (bytes, standalone) in [
            (&[128, 0x10][..], false),
            (&[4, 0x10, 0x3f, 3, 4][..], false),
            (&[139, 0x11, 0x23, 0x45, 0x67, 0x89, 0xab][..], true),
        ] {
            let (description, monitor) = owned_description(bytes);
            let before = monitor.snapshot().attempts;
            validate(description, standalone).unwrap();
            let requests = monitor.snapshot().attempts - before;
            assert!(requests >= 2);
            monitor.assert_released();
            for ordinal in 1..=requests {
                let (description, monitor) = owned_description(bytes);
                let before = monitor.snapshot().attempts;
                monitor.set_fail_at(NonZeroU64::new(before + ordinal));
                assert_eq!(
                    validate(description, standalone),
                    Err(Error::AllocationFailed)
                );
                assert_eq!(monitor.snapshot().attempts, before + ordinal);
                assert_eq!(monitor.snapshot().identities_created, 1);
                monitor.assert_released();
            }
        }
    }

    #[test]
    fn custom_paired_descriptions_reject_trailing_malformed_and_inline_twelve_bit_trees() {
        for bytes in [
            &[][..],
            &[128][..],
            &[128, 0][..],
            &[128, 0x10, 99][..],
            &[4, 0x10, 0x3f, 3, 0][..],
        ] {
            let (description, monitor) = owned_description(bytes);
            assert!(BuiltLiteralTable::from_description(description).is_err());
            monitor.assert_released();
            let (description, monitor) = owned_description(bytes);
            assert!(BuiltStandaloneTable::from_description(description).is_err());
            monitor.assert_released();
        }
        let (description, monitor) = owned_description(&[139, 0x11, 0x23, 0x45, 0x67, 0x89, 0xab]);
        assert!(matches!(
            BuiltLiteralTable::from_description(description),
            Err(Error::CorruptData)
        ));
        monitor.assert_released();
    }

    #[test]
    fn standalone_pair_accepts_twelve_bits_without_relaxing_inline_validation() {
        let dictionary =
            include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
        let (_, consumed) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        let description = &dictionary[8..8 + consumed];
        let built = BuiltStandaloneTable::from_description(Vec::from(description)).unwrap();
        assert_eq!(built.description(), description);
        assert_eq!(built.codebook().table_log(), 12);
        assert!(BuiltLiteralTable::from_description(Vec::from(description)).is_err());
        let mut trailing = Vec::from(description);
        trailing.push(0);
        assert!(BuiltStandaloneTable::from_description(trailing).is_err());
        for truncated in [
            &[][..],
            &description[..1],
            &description[..description.len() - 1],
        ] {
            assert!(BuiltStandaloneTable::from_description(Vec::from(truncated)).is_err());
        }
        let inline = [128, 0x10];
        let ordinary = BuiltLiteralTable::from_description(Vec::from(inline)).unwrap();
        let standalone = BuiltStandaloneTable::from_description(Vec::from(inline)).unwrap();
        assert_eq!(ordinary.codebook(), standalone.codebook());
        assert_eq!(ordinary.description(), standalone.description());
    }

    #[test]
    fn canonical_codes_match_the_format_example_and_symbol_order() {
        let mut lengths = [0; SYMBOLS];
        lengths[..6].copy_from_slice(&[1, 2, 3, 0, 4, 4]);
        let codes = HuffmanCodebook::from_lengths(&lengths, 11).unwrap();
        assert_eq!(codes.table_log(), 4);
        for (symbol, value, bits) in [(0, 1, 1), (1, 1, 2), (2, 1, 3), (4, 0, 4), (5, 1, 4)] {
            assert_eq!(codes.code(symbol), Ok(Codeword { value, bits }));
        }
        assert_eq!(codes.code(3), Err(Error::CorruptData));
        assert_eq!(codes.code(255), Err(Error::CorruptData));

        let flat = HuffmanCodebook::from_lengths(&[8; SYMBOLS], 11).unwrap();
        for symbol in 0..=255 {
            assert_eq!(
                flat.code(symbol),
                Ok(Codeword {
                    value: u16::from(symbol),
                    bits: 8
                })
            );
        }
    }

    #[test]
    fn sparse_codebooks_reject_absent_symbols_and_cover_the_extreme_indices() {
        let mut lengths = [0; SYMBOLS];
        lengths[0] = 1;
        lengths[255] = 1;
        let codes = HuffmanCodebook::from_lengths(&lengths, 1).unwrap();
        assert_eq!(codes.code(0), Ok(Codeword { value: 0, bits: 1 }));
        assert_eq!(codes.code(255), Ok(Codeword { value: 1, bits: 1 }));
        assert!(codes.covers(&[]));
        assert!(codes.covers(&[0, 255, 255, 0]));
        for symbol in 1..255 {
            assert_eq!(codes.code(symbol), Err(Error::CorruptData));
            assert!(!codes.covers(&[0, symbol, 255]));
        }
    }

    #[test]
    fn single_stream_matches_the_two_symbol_example_and_empty_marker() {
        let mut lengths = [0; SYMBOLS];
        lengths[..2].fill(1);
        let codes = HuffmanCodebook::from_lengths(&lengths, 1).unwrap();
        let mut writer = BitWriter::new();
        codes.encode_stream(&[0, 1, 1, 0], &mut writer).unwrap();
        assert_eq!(writer.as_bytes(), &[0x16]);
        assert_eq!(writer.write(0, 0), Err(Error::CorruptData));
        let capacity = writer.retained_capacity();
        codes.encode_stream(&[], &mut writer).unwrap();
        assert_eq!(writer.as_bytes(), &[1]);
        assert_eq!(writer.retained_capacity(), capacity);
    }

    #[test]
    fn single_stream_missing_symbols_preserve_pending_and_closed_writer_state() {
        let mut lengths = [0; SYMBOLS];
        lengths[0] = 1;
        lengths[255] = 1;
        let codes = HuffmanCodebook::from_lengths(&lengths, 1).unwrap();
        let mut writer = BitWriter::new();
        writer.write(0x52, 8).unwrap();
        writer.write(5, 3).unwrap();
        for input in [&[1, 0, 255][..], &[0, 1, 255], &[0, 255, 254]] {
            assert_eq!(
                codes.encode_stream(input, &mut writer),
                Err(Error::CorruptData)
            );
            assert_eq!(writer.as_bytes(), &[0x52]);
        }
        // A following write observes the original three pending bits; checking
        // as_bytes alone would not detect a reset of that partial reservoir.
        writer.write(3, 5).unwrap();
        writer.finish().unwrap();
        assert_eq!(writer.as_bytes(), &[0x52, 0x1d, 1]);
        assert_eq!(
            codes.encode_stream(&[255, 1, 0], &mut writer),
            Err(Error::CorruptData)
        );
        assert_eq!(writer.as_bytes(), &[0x52, 0x1d, 1]);
        assert_eq!(writer.write(0, 0), Err(Error::CorruptData));
    }

    #[test]
    fn single_stream_byte_width_codes_have_exact_reverse_order_at_size_boundaries() {
        let codes = HuffmanCodebook::from_lengths(&[8; SYMBOLS], 11).unwrap();
        let mut writer = BitWriter::new();
        for length in [0, 1, 7, 8, 9, 15, 16, 17, 255, 256, 257, 1023, 1024, 4095] {
            let source: Vec<_> = (0..length).map(|index| index as u8).collect();
            codes.encode_stream(&source, &mut writer).unwrap();
            let mut expected: Vec<_> = source.iter().rev().copied().collect();
            expected.push(1);
            assert_eq!(writer.as_bytes(), expected, "source length {length}");
        }
    }

    #[test]
    fn single_stream_dictionary_twelve_bit_codes_round_trip_every_present_symbol_at_the_end() {
        let dictionary =
            include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
        let (table, _) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        let codes = HuffmanCodebook::from_decoding_table(&table).unwrap();
        assert_eq!(codes.table_log(), 12);
        let present: Vec<_> = (0..=255)
            .filter(|&symbol| codes.code(symbol).is_ok())
            .collect();
        let mut writer = BitWriter::new();
        for &last in &present {
            for prefix_length in 0..8 {
                let mut source = present[..prefix_length].to_vec();
                source.push(last);
                codes.encode_stream(&source, &mut writer).unwrap();
                let bits: usize = source
                    .iter()
                    .map(|&symbol| usize::from(codes.code(symbol).unwrap().bits))
                    .sum();
                assert_eq!(writer.as_bytes().len(), (bits + 1).div_ceil(8));
                assert_eq!(
                    table
                        .decode_stream(writer.as_bytes(), source.len())
                        .unwrap(),
                    source,
                    "last symbol {last}, prefix length {prefix_length}"
                );
            }
        }
    }

    #[test]
    fn four_streams_six_and_nine_literals_include_an_empty_final_marker() {
        let (table, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        let codes = HuffmanCodebook::from_decoding_table(&table).unwrap();
        let mut writer = BitWriter::new();
        let mut output = Vec::new();
        for (source, expected) in [
            (&[0, 1, 0, 1, 0, 1][..], &[1, 0, 1, 0, 1, 0, 5, 5, 5, 1][..]),
            (
                &[0, 1, 0, 1, 0, 1, 0, 1, 0],
                &[1, 0, 1, 0, 1, 0, 10, 13, 10, 1],
            ),
        ] {
            codes
                .encode_four_streams(source, &mut writer, &mut output)
                .unwrap();
            assert_eq!(output, expected);
            assert_eq!(writer.as_bytes(), &[1]);
            assert_eq!(
                crate::huffman::decode_streams(&table, &output, source.len(), true).unwrap(),
                source
            );
        }
    }

    #[test]
    fn four_streams_invalid_sizes_and_symbols_preserve_both_buffers() {
        let mut lengths = [0; SYMBOLS];
        lengths[0] = 1;
        lengths[255] = 1;
        let codes = HuffmanCodebook::from_lengths(&lengths, 1).unwrap();
        let mut writer = BitWriter::new();
        writer.write(0x52, 8).unwrap();
        writer.write(5, 3).unwrap();
        let mut output = vec![88, 99];
        let capacity = output.capacity();
        for length in [0, 1, 2, 3, 4, 5, MAX_BLOCK_SIZE + 1] {
            assert_eq!(
                codes.encode_four_streams(&vec![0; length], &mut writer, &mut output),
                Err(Error::InvalidParameter)
            );
            assert_eq!(output, [88, 99]);
            assert_eq!(output.capacity(), capacity);
            assert_eq!(writer.as_bytes(), &[0x52]);
        }
        for absent_at in 0..9 {
            let mut source = [0; 9];
            source[absent_at] = 1;
            assert_eq!(
                codes.encode_four_streams(&source, &mut writer, &mut output),
                Err(Error::CorruptData)
            );
            assert_eq!(output, [88, 99]);
            assert_eq!(writer.as_bytes(), &[0x52]);
        }
        writer.write(3, 5).unwrap();
        writer.finish().unwrap();
        assert_eq!(writer.as_bytes(), &[0x52, 0x1d, 1]);
    }

    #[test]
    fn four_stream_jump_lengths_and_order_cover_literal_size_boundaries() {
        let codes = HuffmanCodebook::from_lengths(&[8; SYMBOLS], 11).unwrap();
        let mut writer = BitWriter::new();
        let mut output = Vec::new();
        for length in [
            6,
            7,
            8,
            9,
            10,
            11,
            12,
            13,
            1023,
            1024,
            16_383,
            16_384,
            MAX_BLOCK_SIZE - 1,
            MAX_BLOCK_SIZE,
        ] {
            let source: Vec<_> = (0..length).map(|index| index as u8).collect();
            codes
                .encode_four_streams(&source, &mut writer, &mut output)
                .unwrap();
            assert_eq!(output.len(), length + 10);
            let quarter = length.div_ceil(4);
            for jump in output[..6].as_chunks::<2>().0 {
                assert_eq!(
                    usize::from(u16::from_le_bytes([jump[0], jump[1]])),
                    quarter + 1
                );
            }
            let mut encoded_start = 6;
            for (stream, original_start) in (0..4).zip([0, quarter, quarter * 2, quarter * 3]) {
                let original_end = if stream == 3 {
                    length
                } else {
                    original_start + quarter
                };
                let count = original_end - original_start;
                let encoded_end = encoded_start + count;
                let recovered: Vec<_> = output[encoded_start..encoded_end]
                    .iter()
                    .rev()
                    .copied()
                    .collect();
                assert_eq!(recovered, source[original_start..original_end]);
                assert_eq!(output[encoded_end], 1);
                encoded_start = encoded_end + 1;
            }
            assert_eq!(encoded_start, output.len());
        }
    }

    #[test]
    fn four_stream_dictionary_maximum_width_respects_jump_bounds_and_reuses_capacity() {
        let dictionary =
            include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
        let (table, _) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        let codes = HuffmanCodebook::from_decoding_table(&table).unwrap();
        assert_eq!(codes.code(0), Ok(Codeword { value: 0, bits: 12 }));
        let source = vec![0; MAX_BLOCK_SIZE];
        let mut writer = BitWriter::new();
        let mut output = Vec::new();
        codes
            .encode_four_streams(&source, &mut writer, &mut output)
            .unwrap();
        assert_eq!(output.len(), 196_618);
        assert_eq!(&output[..6], &[1, 192, 1, 192, 1, 192]);
        assert_eq!(
            crate::huffman::decode_streams(&table, &output, source.len(), true).unwrap(),
            source
        );
        let capacities = (writer.retained_capacity(), output.capacity());
        codes
            .encode_four_streams(&source[..6], &mut writer, &mut output)
            .unwrap();
        assert_eq!(writer.as_bytes(), &[1]);
        assert_eq!((writer.retained_capacity(), output.capacity()), capacities);
        assert_eq!(output.len(), 19);
    }

    #[test]
    fn standalone_streams_use_their_own_source_limit_and_exact_jump_boundaries() {
        let codes = HuffmanCodebook::from_lengths(&[8; 256], 12).unwrap();
        let mut writer = BitWriter::new();
        let mut output = Vec::new();
        let source = vec![0; 4 * 65_534];
        codes
            .encode_standalone_four_streams(&source, &mut writer, &mut output)
            .unwrap();
        assert_eq!(&output[..6], &[255; 6]);
        assert_eq!(output.len(), 6 + 4 * 65_535);
        let saved = output.clone();
        let saved_stream = writer.as_bytes().to_vec();
        for length in [4 * 65_534 + 1, 256 * 1024, 256 * 1024 + 1] {
            assert_eq!(
                codes.encode_standalone_four_streams(&vec![0; length], &mut writer, &mut output),
                Err(Error::InvalidParameter)
            );
            assert_eq!(output, saved);
            assert_eq!(writer.as_bytes(), saved_stream);
        }
        assert_eq!(
            codes.encode_four_streams(&source, &mut writer, &mut output),
            Err(Error::InvalidParameter)
        );
    }

    #[test]
    fn standalone_log_twelve_streams_accept_short_codes_and_reject_actual_jump_overflow() {
        let dictionary =
            include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
        let (table, _) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        let codes = HuffmanCodebook::from_decoding_table(&table).unwrap();
        let symbol = (0..=255_u8)
            .filter(|&symbol| codes.code(symbol).is_ok())
            .min_by_key(|&symbol| codes.code(symbol).unwrap().bits)
            .unwrap();
        let bits = usize::from(codes.code(symbol).unwrap().bits);
        assert!(bits < 8);
        let mut writer = BitWriter::new();
        let mut output = Vec::new();
        let source = vec![symbol; 256 * 1024];
        codes
            .encode_standalone_four_streams(&source, &mut writer, &mut output)
            .unwrap();
        assert_eq!(output.len(), 6 + 4 * (65_536 * bits + 1).div_ceil(8));
        assert_eq!(
            crate::huffman::decode_streams(&table, &output, source.len(), true).unwrap(),
            source
        );
        let saved = output.clone();
        let stream = writer.as_bytes().to_vec();
        assert_eq!(
            codes.encode_standalone_four_streams(&vec![0; source.len()], &mut writer, &mut output),
            Err(Error::InvalidParameter)
        );
        assert_eq!(output, saved);
        assert_eq!(writer.as_bytes(), stream);
        let long_codes = vec![0; source.len()];
        codes.encode_stream(&long_codes, &mut writer).unwrap();
        assert_eq!(writer.as_bytes().len(), 393_217);
        assert_eq!(
            crate::huffman::decode_streams(&table, writer.as_bytes(), long_codes.len(), false)
                .unwrap(),
            long_codes
        );
    }

    #[test]
    fn invalid_lengths_limits_and_incomplete_or_excess_mass_are_rejected() {
        for limit in [0, 13, u8::MAX] {
            assert_eq!(
                HuffmanCodebook::from_lengths(&[8; SYMBOLS], limit).unwrap_err(),
                Error::InvalidParameter
            );
        }
        for prefix in [
            &[][..],
            &[1][..],
            &[2, 2][..],
            &[1, 1, 1][..],
            &[13, 13][..],
            &[255, 1][..],
        ] {
            let mut lengths = [0; SYMBOLS];
            lengths[..prefix.len()].copy_from_slice(prefix);
            assert_eq!(
                HuffmanCodebook::from_lengths(&lengths, 12).unwrap_err(),
                Error::InvalidParameter
            );
        }
        let mut deepest = [0; SYMBOLS];
        deepest[..11].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        deepest[11..13].fill(12);
        assert_eq!(
            HuffmanCodebook::from_lengths(&deepest, 11).unwrap_err(),
            Error::InvalidParameter
        );
        assert_eq!(
            HuffmanCodebook::from_lengths(&deepest, 12)
                .unwrap()
                .table_log(),
            12
        );
    }

    #[test]
    fn direct_and_fse_descriptions_reconstruct_exact_sparse_codes() {
        // The format's weights [4,3,2,0,1] leave symbol five's weight implicit.
        let paired = BuiltLiteralTable::from_description(vec![132, 0x43, 0x20, 0x10]).unwrap();
        let mut lengths = [0; SYMBOLS];
        lengths[..6].copy_from_slice(&[1, 2, 3, 0, 4, 4]);
        assert_eq!(
            paired.codebook(),
            &HuffmanCodebook::from_lengths(&lengths, 11).unwrap()
        );
        assert_eq!(paired.description(), &[132, 0x43, 0x20, 0x10]);

        // Independent existing decoder vector: FSE weights [0,1], inferred1.
        let paired = BuiltLiteralTable::from_description(vec![4, 0x10, 0x3f, 3, 4]).unwrap();
        assert_eq!(paired.codebook().code(0), Err(Error::CorruptData));
        assert_eq!(
            paired.codebook().code(1),
            Ok(Codeword { value: 0, bits: 1 })
        );
        assert_eq!(
            paired.codebook().code(2),
            Ok(Codeword { value: 1, bits: 1 })
        );
    }

    #[test]
    fn paired_descriptions_reject_truncation_trailing_bytes_and_invalid_weights() {
        let valid = [132, 0x43, 0x20, 0x10];
        for end in 0..valid.len() {
            assert!(BuiltLiteralTable::from_description(valid[..end].to_vec()).is_err());
        }
        let mut trailing = valid.to_vec();
        trailing.push(0);
        assert_eq!(
            BuiltLiteralTable::from_description(trailing).unwrap_err(),
            Error::CorruptData
        );
        for bytes in [vec![0], vec![128, 0], vec![128, 0xf0]] {
            assert!(BuiltLiteralTable::from_description(bytes).is_err());
        }
    }

    #[test]
    fn official_dictionary_codes_preserve_twelve_bits_and_missing_symbols() {
        let dictionary =
            include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
        let (table, consumed) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        // Independently verified with v1.5.7 HUF_readStats/HUF_readCTable.
        assert_eq!(consumed, 72);
        let codes = HuffmanCodebook::from_decoding_table(&table).unwrap();
        assert_eq!(codes.table_log(), 12);
        assert_eq!(codes.code(0), Ok(Codeword { value: 0, bits: 12 }));
        let missing = [
            192, 193, 242, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254,
        ];
        for symbol in 0..=255 {
            assert_eq!(codes.code(symbol).is_err(), missing.contains(&symbol));
        }
        assert_eq!(
            BuiltLiteralTable::from_description(dictionary[8..80].to_vec()).unwrap_err(),
            Error::CorruptData
        );
    }

    #[test]
    fn learned_table_pairing_uses_the_exact_published_description() {
        let mut observed = [0; SYMBOLS];
        for trial in 0..16 {
            observed[(trial * 17) % SYMBOLS] = 1 << (trial * 3);
            let paired = crate::huffman_encode::build_literal_table(&observed).unwrap();
            let serialized = crate::huffman_encode::serialize_literal_table(&observed).unwrap();
            assert_eq!(paired.description(), serialized);
            let (table, consumed) = HuffmanTable::read(paired.description()).unwrap();
            assert_eq!(consumed, paired.description().len());
            assert_eq!(
                paired.codebook(),
                &HuffmanCodebook::from_decoding_table(&table).unwrap()
            );
            assert!(paired.codebook().covers(&(0..=255).collect::<Vec<_>>()));
        }
    }

    #[test]
    fn paired_fallback_uses_selected_codes_instead_of_discarded_learned_lengths() {
        let mut observed = [0; SYMBOLS];
        observed[0] = 1 << 40;
        let learned = crate::huffman_encode::literal_lengths(&observed).unwrap();
        let selected = crate::huffman_encode::near_flat_lengths(&observed);
        assert_ne!(selected, learned);
        // Explicitly supply the serializer's fallback selection. This checks
        // pairing without assuming a particular histogram reaches its 128-byte
        // cutoff or changing that production cutoff to force the branch.
        let weights = crate::huffman_encode::explicit_weights(&selected).unwrap();
        let payload = crate::entropy_encode::encode_weights(&weights).unwrap();
        assert!((1..128).contains(&payload.len()));
        let mut description = vec![payload.len() as u8];
        description.extend_from_slice(&payload);
        let paired = BuiltLiteralTable::from_description(description).unwrap();
        assert_eq!(
            paired.codebook().code(0),
            Ok(Codeword {
                value: 127,
                bits: 7
            })
        );
        assert_eq!(
            paired.codebook().code(254),
            Ok(Codeword { value: 0, bits: 9 })
        );
        assert_eq!(
            paired.codebook().code(255),
            Ok(Codeword { value: 1, bits: 9 })
        );
        assert_eq!(
            paired.codebook(),
            &HuffmanCodebook::from_lengths(&selected, 11).unwrap()
        );
        assert_ne!(
            paired.codebook(),
            &HuffmanCodebook::from_lengths(&learned, 11).unwrap()
        );
    }
}
