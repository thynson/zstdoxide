//! FSE sequence decoding and checked repeat-offset execution.

use alloc::alloc::{Allocator, Global};

use crate::bitstream::ReverseBits;
use crate::constants::{LL_BASE, LL_BITS, LL_DEFAULT, ML_BASE, ML_BITS, ML_DEFAULT, OF_DEFAULT};
use crate::cursor::ByteCursor;
use crate::dictionary::{DictionaryEntropy, DictionaryEntropyView};
use crate::fse::{FseTable, FseTableView};
use crate::history::SequenceOutput;
use crate::{Error, Result};

pub(crate) struct SequenceState<A: Allocator = Global> {
    allocator: A,
    literal_lengths: Option<FseTable<A>>,
    offsets: Option<FseTable<A>>,
    match_lengths: Option<FseTable<A>>,
    repeat: [usize; 3],
}

impl SequenceState<Global> {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "compatibility convenience for direct sequence callers"
        )
    )]
    pub(crate) fn new() -> Self {
        Self::new_in(Global)
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "compatibility convenience for direct sequence callers"
        )
    )]
    pub(crate) fn with_repeat(repeat: [usize; 3]) -> Self {
        Self::with_repeat_in(repeat, Global)
    }
}

impl<A: Allocator> SequenceState<A> {
    pub(crate) fn retained_capacity(&self) -> usize {
        [&self.literal_lengths, &self.offsets, &self.match_lengths]
            .into_iter()
            .filter_map(Option::as_ref)
            .map(FseTable::retained_capacity)
            .sum()
    }

    /// Retain this allocator without allocating table payloads or cloning it.
    pub(crate) fn new_in(allocator: A) -> Self {
        Self::with_repeat_in([1, 4, 8], allocator)
    }

    pub(crate) fn with_repeat_in(repeat: [usize; 3], allocator: A) -> Self {
        Self {
            allocator,
            literal_lengths: None,
            offsets: None,
            match_lengths: None,
            repeat,
        }
    }

    /// Release frame overrides and restore repeats while retaining the allocator.
    /// This also clears partially published tables after a failed decode; it
    /// does not roll back output that was already appended to the caller's sink.
    pub(crate) fn reset(&mut self, repeat: [usize; 3]) {
        self.literal_lengths = None;
        self.offsets = None;
        self.match_lengths = None;
        self.repeat = repeat;
    }
}

impl<A: Allocator + Clone> SequenceState<A> {
    /// Decode and execute one sequence section, including trailing literals.
    ///
    /// Corresponds to `ZSTD_decodeSeqHeaders`, `ZSTD_decodeSequence`, and
    /// `ZSTD_execSequence`, with indices replacing C pointer state.
    pub(crate) fn decode<O: SequenceOutput>(
        &mut self,
        input: &[u8],
        literals: &[u8],
        history: &mut O,
        max_block: usize,
    ) -> Result<()> {
        self.decode_with_dictionary_view(input, literals, history, max_block, None)
    }

    /// Use immutable prepared tables until a block overrides each distribution.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "compatibility convenience for direct sequence callers"
        )
    )]
    pub(crate) fn decode_with_dictionary<O: SequenceOutput, B: Allocator>(
        &mut self,
        input: &[u8],
        literals: &[u8],
        history: &mut O,
        max_block: usize,
        initial: Option<&DictionaryEntropy<B>>,
    ) -> Result<()> {
        self.decode_with_dictionary_view(
            input,
            literals,
            history,
            max_block,
            initial.map(DictionaryEntropy::view),
        )
    }

    /// Borrow prepared tables independently of their allocator until overridden.
    /// Header tables publish in LL/OF/ML order, matching the existing decoder:
    /// a later allocation or input error may retain earlier replacements, and
    /// output/repeats may already have advanced. The frame driver must discard
    /// failed output and reset the state before a new frame. This is not the
    /// encoder candidate's transactional commit/discard contract.
    pub(crate) fn decode_with_dictionary_view<O: SequenceOutput>(
        &mut self,
        input: &[u8],
        literals: &[u8],
        history: &mut O,
        max_block: usize,
        initial: Option<DictionaryEntropyView<'_>>,
    ) -> Result<()> {
        let mut cursor = ByteCursor::new(input);
        let count = match cursor.byte()? {
            byte @ 0..=127 => usize::from(byte),
            byte @ 128..=254 => (usize::from(byte - 128) << 8) + usize::from(cursor.byte()?),
            _ => cursor.little_endian(2)? as usize + 0x7f00,
        };
        if literals.len() > max_block || count > max_block / 3 {
            return Err(Error::CorruptData);
        }
        if count == 0 {
            if !cursor.remaining().is_empty() {
                return Err(Error::CorruptData);
            }
            return history.append(literals);
        }
        let modes = cursor.byte()?;
        if modes & 3 != 0 {
            return Err(Error::CorruptData);
        }
        read_table(
            &mut cursor,
            modes >> 6,
            &mut self.literal_lengths,
            initial.map(|tables| tables.literal_lengths),
            (&LL_DEFAULT, 6),
            (35, 9),
            &self.allocator,
        )?;
        read_table(
            &mut cursor,
            (modes >> 4) & 3,
            &mut self.offsets,
            initial.map(|tables| tables.offsets),
            (&OF_DEFAULT, 5),
            (31, 8),
            &self.allocator,
        )?;
        read_table(
            &mut cursor,
            (modes >> 2) & 3,
            &mut self.match_lengths,
            initial.map(|tables| tables.match_lengths),
            (&ML_DEFAULT, 6),
            (52, 9),
            &self.allocator,
        )?;
        let ll = self
            .literal_lengths
            .as_ref()
            .map(FseTable::view)
            .or(initial.map(|tables| tables.literal_lengths))
            .ok_or(Error::CorruptData)?;
        let of = self
            .offsets
            .as_ref()
            .map(FseTable::view)
            .or(initial.map(|tables| tables.offsets))
            .ok_or(Error::CorruptData)?;
        let ml = self
            .match_lengths
            .as_ref()
            .map(FseTable::view)
            .or(initial.map(|tables| tables.match_lengths))
            .ok_or(Error::CorruptData)?;
        let mut bits = ReverseBits::new(cursor.remaining())?;
        let mut literal_state = bits.read_bits(ll.table_log())? as usize;
        let mut offset_state = bits.read_bits(of.table_log())? as usize;
        let mut match_state = bits.read_bits(ml.table_log())? as usize;
        let mut literal_position = 0;
        let block_start = history.len();
        for index in 0..count {
            let literal_entry = ll.entry(literal_state)?;
            let offset_entry = of.entry(offset_state)?;
            let match_entry = ml.entry(match_state)?;
            let literal_code = usize::from(literal_entry.symbol);
            let match_code = usize::from(match_entry.symbol);
            if literal_code >= LL_BASE.len()
                || match_code >= ML_BASE.len()
                || offset_entry.symbol > 31
            {
                return Err(Error::CorruptData);
            }
            let offset_value =
                (1_u32 << offset_entry.symbol) + bits.read_bits(offset_entry.symbol)?;
            let match_length =
                (ML_BASE[match_code] + bits.read_bits(ML_BITS[match_code])?) as usize;
            let literal_length =
                (LL_BASE[literal_code] + bits.read_bits(LL_BITS[literal_code])?) as usize;
            let end = literal_position + literal_length;
            let pending = literals
                .get(literal_position..end)
                .ok_or(Error::CorruptData)?;
            let generated = history.len() - block_start;
            if literal_length + match_length > max_block - generated {
                return Err(Error::CorruptData);
            }
            let offset = resolve_offset(&mut self.repeat, offset_value as usize, literal_length)?;
            history.append(pending)?;
            history.copy_match(offset, match_length)?;
            literal_position = end;
            if index + 1 < count {
                literal_state =
                    usize::from(literal_entry.base) + bits.read_bits(literal_entry.bits)? as usize;
                match_state =
                    usize::from(match_entry.base) + bits.read_bits(match_entry.bits)? as usize;
                offset_state =
                    usize::from(offset_entry.base) + bits.read_bits(offset_entry.bits)? as usize;
            }
        }
        if bits.remaining() != 0 {
            return Err(Error::CorruptData);
        }
        let trailing = &literals[literal_position..];
        if trailing.len() > max_block - (history.len() - block_start) {
            return Err(Error::CorruptData);
        }
        history.append(trailing)
    }
}

fn read_table<A: Allocator + Clone>(
    cursor: &mut ByteCursor<'_>,
    mode: u8,
    previous: &mut Option<FseTable<A>>,
    initial: Option<FseTableView<'_>>,
    default: (&[i16], u8),
    limits: (usize, u8),
    allocator: &A,
) -> Result<()> {
    let (max_symbol, max_log) = limits;
    let table = match mode {
        0 => FseTable::from_normalized_in(default.0, default.1, allocator.clone())?,
        1 => {
            let symbol = cursor.byte()?;
            if usize::from(symbol) > max_symbol {
                return Err(Error::CorruptData);
            }
            FseTable::rle_in(symbol, allocator.clone())
        }
        2 => {
            let (table, consumed) =
                FseTable::read_in(cursor.remaining(), max_symbol, max_log, allocator.clone())?;
            cursor.take(consumed)?;
            table
        }
        3 => {
            return previous
                .as_ref()
                .map(FseTable::view)
                .or(initial)
                .map(|_| ())
                .ok_or(Error::CorruptData);
        }
        _ => return Err(Error::CorruptData),
    };
    *previous = Some(table);
    Ok(())
}

fn resolve_offset(repeat: &mut [usize; 3], value: usize, literal_length: usize) -> Result<usize> {
    if value > 3 {
        let offset = value - 3;
        *repeat = [offset, repeat[0], repeat[1]];
        return Ok(offset);
    }
    let index = value - 1 + usize::from(literal_length == 0);
    let offset = match index {
        0..=2 => repeat[index],
        3 => repeat[0]
            .checked_sub(1)
            .filter(|&n| n != 0)
            .ok_or(Error::CorruptData)?,
        _ => return Err(Error::CorruptData),
    };
    match index {
        0 => {}
        1 => repeat.swap(0, 1),
        _ => *repeat = [offset, repeat[0], repeat[1]],
    }
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::History;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn tracking() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig::default()).unwrap()
    }

    fn prepared<A: Allocator + Clone>(allocator: A) -> DictionaryEntropy<A> {
        DictionaryEntropy {
            huffman: crate::huffman::HuffmanTable::read_in(&[128, 0x10], allocator.clone())
                .unwrap()
                .0,
            offsets: FseTable::rle_in(0, allocator.clone()),
            literal_lengths: FseTable::rle_in(0, allocator.clone()),
            match_lengths: FseTable::rle_in(0, allocator),
            repeat: [8, 4, 1],
        }
    }

    #[test]
    fn custom_sequence_state_borrows_dictionary_domains_and_replaces_one_channel() {
        let dictionary_owner = tracking().into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let entropy = prepared(&dictionary_owner);
        let dictionary_before = dictionary_monitor.snapshot();
        assert!(dictionary_monitor.fail_next());
        let owner = tracking();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let mut state = SequenceState::with_repeat_in(entropy.repeat, owner);
        let mut output = History::new(32);
        output.start_frame(32);
        output.append(b"abcdefgh").unwrap();
        assert!(monitor.fail_next());
        state
            .decode_with_dictionary(&[1, 0xfc, 1], b"", &mut output, 32, Some(&entropy))
            .unwrap();
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(state.retained_capacity(), 0);
        assert_eq!(state.repeat, [4, 8, 1]);
        state
            .decode_with_dictionary_view(
                &[1, 0xdc, 2, 4],
                b"",
                &mut output,
                32,
                Some(entropy.view()),
            )
            .unwrap();
        state
            .decode_with_dictionary_view(&[1, 0xfc, 4], b"", &mut output, 32, Some(entropy.view()))
            .unwrap();
        assert!(state.literal_lengths.is_none());
        assert!(state.match_lengths.is_none());
        assert_eq!(state.offsets.as_ref().unwrap().entry(0).unwrap().symbol, 2);
        assert_eq!(monitor.snapshot().attempts, 0);
        assert_eq!(monitor.snapshot().identities_created, 2); // one fresh RLE owner
        assert_eq!(
            dictionary_monitor.snapshot().attempts,
            dictionary_before.attempts
        );
        assert_eq!(dictionary_monitor.snapshot().identities_created, 1);
        assert_eq!(output.into_bytes(), b"abcdefghefggggggg");
        state.reset([1, 4, 8]);
        assert_eq!(state.allocator.identity(), identity);
        assert_eq!(state.repeat, [1, 4, 8]);
        assert!(state.offsets.is_none());
        // Table reads did not consume either allocator's pending failure.
        assert!(matches!(
            FseTable::from_normalized_in(&[16, 16], 5, &state.allocator),
            Err(Error::AllocationFailed)
        ));
        assert!(matches!(
            FseTable::from_normalized_in(&[16, 16], 5, &dictionary_owner),
            Err(Error::AllocationFailed)
        ));
        drop(state);
        monitor.assert_released();
        drop(entropy);
        dictionary_monitor.assert_released();
    }

    #[test]
    fn custom_sequence_state_preserves_partial_table_publication_at_each_failure() {
        let counts = crate::entropy_encode::write_normalized_counts(&[16, 16], 5).unwrap();
        let mut compressed = alloc::vec![1, 0xa8];
        for _ in 0..3 {
            compressed.extend_from_slice(&counts);
        }
        compressed.push(1); // complete headers, insufficient sequence state bits
        for (section, logs) in [(&[1, 0, 1][..], [6, 5, 6]), (&compressed[..], [5, 5, 5])] {
            for failing in 1..=3 {
                let owner = tracking();
                let identity = owner.identity();
                let monitor = owner.monitor();
                let mut state = SequenceState::new_in(owner);
                let mut seed = History::new(32);
                seed.start_frame(32);
                state
                    .decode(&[1, 0x54, 3, 2, 0, 6], b"abc!", &mut seed, 32)
                    .unwrap();
                let repeat = state.repeat;
                assert_eq!(monitor.snapshot().attempts, 0);
                monitor.set_fail_at(NonZeroU64::new(failing));
                let mut output = History::new(32);
                output.start_frame(32);
                assert_eq!(
                    state.decode(section, b"", &mut output, 32),
                    Err(Error::AllocationFailed)
                );
                assert_eq!(output.len(), 0);
                assert_eq!(state.repeat, repeat);
                for (index, table) in [
                    state.literal_lengths.as_ref(),
                    state.offsets.as_ref(),
                    state.match_lengths.as_ref(),
                ]
                .into_iter()
                .enumerate()
                {
                    let table = table.unwrap();
                    if index < failing as usize - 1 {
                        assert_eq!(table.table_log(), logs[index]);
                    } else {
                        assert_eq!(table.table_log(), 0);
                        assert_eq!(table.entry(0).unwrap().symbol, [3, 2, 0][index]);
                    }
                }
                let snapshot = monitor.snapshot();
                assert_eq!(snapshot.attempts, failing);
                assert_eq!(snapshot.live_blocks, failing - 1);
                monitor.clear_failure();
                state.reset([1, 4, 8]);
                assert_eq!(state.allocator.identity(), identity);
                assert_eq!(state.retained_capacity(), 0);
                assert_eq!(monitor.snapshot().live_blocks, 0);
                // A complete header still publishes all three tables before a
                // later bitstream failure, preserving the decoder's old contract.
                assert!(state.decode(section, b"", &mut output, 32).is_err());
                assert_eq!(monitor.snapshot().live_blocks, 3);
                assert_eq!(state.repeat, [1, 4, 8]);
                state.reset([1, 4, 8]);
                state
                    .decode(&[1, 0x54, 3, 2, 0, 6], b"abc!", &mut output, 32)
                    .unwrap();
                assert_eq!(output.into_bytes(), b"abcabc!");
                drop(state);
                monitor.assert_released();
            }
        }
    }

    #[test]
    fn custom_sequence_state_keeps_prepared_singleton_widths_with_non_clone_borrow() {
        let mut entropy = prepared(Global);
        entropy.offsets = FseTable::read_dictionary(&[0xf0, 0x03], 31, 8).unwrap().0;
        entropy.literal_lengths = FseTable::read_dictionary(&[0xf0, 0x03], 35, 9).unwrap().0;
        entropy.match_lengths = FseTable::read_dictionary(&[0xf0, 0x03], 52, 9).unwrap().0;
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let mut state = SequenceState::with_repeat_in(entropy.repeat, &owner);
        let mut output = History::new(32);
        output.start_frame(32);
        output.append(b"abcdefgh").unwrap();
        assert!(monitor.fail_next());
        state
            .decode_with_dictionary_view(
                &[1, 0xfc, 0, 128],
                b"",
                &mut output,
                32,
                Some(entropy.view()),
            )
            .unwrap();
        assert_eq!(output.into_bytes(), b"abcdefghefg");
        assert_eq!(state.repeat, [4, 8, 1]);
        assert_eq!(state.retained_capacity(), 0);
        assert_eq!(monitor.snapshot().attempts, 0);
        assert_eq!(monitor.snapshot().identities_created, 1);
        state.reset(entropy.repeat);
        assert_eq!(state.allocator.identity(), owner.identity());
        drop(state);
        monitor.assert_released();
    }

    #[test]
    fn custom_sequence_sink_failure_retains_documented_repeat_and_partial_output() {
        for failing in 1..=2 {
            let owner = tracking();
            let monitor = owner.monitor();
            let mut state = SequenceState::new_in(owner);
            let output_owner = tracking().into_non_clone();
            let output_monitor = output_owner.monitor();
            output_monitor.set_fail_at(NonZeroU64::new(failing));
            let mut output = History::new_in(32, &output_owner);
            output.start_frame(32);
            let section = [1, 0x54, 3, 2, 4, 6];
            assert_eq!(
                state.decode(&section, b"abc!", &mut output, 32),
                Err(Error::AllocationFailed)
            );
            assert_eq!(state.repeat, [3, 1, 4]);
            assert_eq!(
                output.frame_bytes(),
                if failing == 1 { &b""[..] } else { &b"abc"[..] }
            );
            assert_eq!(monitor.snapshot().attempts, 0);
            assert_eq!(output_monitor.snapshot().attempts, failing);
            drop(output);
            output_monitor.assert_released();
            state.reset([1, 4, 8]);
            let mut output = History::new_in(32, &output_owner);
            output.start_frame(32);
            state.decode(&section, b"abc!", &mut output, 32).unwrap();
            assert_eq!(output.into_bytes().as_slice(), b"abcabcabca!");
            output_monitor.assert_released();
            drop(state);
            monitor.assert_released();
        }
    }

    #[test]
    fn custom_sequence_state_reset_can_own_a_non_clone_allocator_without_decoding() {
        let owner = tracking().into_non_clone();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let mut state = SequenceState::new_in(owner);
        assert_eq!(state.repeat, [1, 4, 8]);
        state.reset([8, 4, 1]);
        assert_eq!(state.allocator.identity(), identity);
        assert_eq!(state.repeat, [8, 4, 1]);
        assert_eq!(monitor.snapshot().attempts, 0);
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(state);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
        monitor.assert_released();
    }

    #[test]
    fn custom_sequence_validation_and_zero_sequences_do_not_construct_tables() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let mut state = SequenceState::new_in(&owner);
        let mut output = History::new(32);
        output.start_frame(32);
        assert!(monitor.fail_next());
        state.decode(&[0], b"abc", &mut output, 32).unwrap();
        for input in [
            &[][..],
            &[0, 0],
            &[1],
            &[1, 0x57],
            &[1, 0xfc, 1],
            &[1, 0x54, 36, 0, 0, 1],
        ] {
            assert!(state.decode(input, b"", &mut output, 32).is_err());
        }
        assert_eq!(output.into_bytes(), b"abc");
        assert_eq!(state.retained_capacity(), 0);
        assert_eq!(state.repeat, [1, 4, 8]);
        assert_eq!(monitor.snapshot().attempts, 0);
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(state);
        monitor.assert_released();
    }

    #[test]
    fn prepared_tables_and_repcodes_are_borrowed_until_each_table_is_replaced() {
        let entropy = DictionaryEntropy {
            huffman: crate::huffman::HuffmanTable::read(&[128, 0x10]).unwrap().0,
            offsets: FseTable::rle(0),
            literal_lengths: FseTable::rle(0),
            match_lengths: FseTable::rle(0),
            repeat: [8, 4, 1],
        };
        let mut state = SequenceState::with_repeat(entropy.repeat);
        let mut output = History::new(32);
        output.start_frame(32);
        output.append(b"abcdefgh").unwrap();
        state
            .decode_with_dictionary(&[1, 0xfc, 1], b"", &mut output, 32, Some(&entropy))
            .unwrap();
        assert!(state.offsets.is_none());
        assert_eq!(state.repeat, [4, 8, 1]);
        // Replace only OF with an explicit distance-one code, retaining LL/ML.
        state
            .decode_with_dictionary(&[1, 0xdc, 2, 4], b"", &mut output, 32, Some(&entropy))
            .unwrap();
        state
            .decode_with_dictionary(&[1, 0xfc, 4], b"", &mut output, 32, Some(&entropy))
            .unwrap();
        assert!(state.offsets.is_some());
        assert!(state.literal_lengths.is_none());
        assert!(state.match_lengths.is_none());
        assert_eq!(entropy.repeat, [8, 4, 1]);
        assert_eq!(output.into_bytes(), b"abcdefghefggggggg");
    }

    #[test]
    fn executes_rle_symbols_and_overlapping_matches() {
        let mut output = History::new(32);
        output.start_frame(32);
        SequenceState::new()
            .decode(&[1, 0x54, 3, 2, 0, 6], b"abc!", &mut output, 32)
            .unwrap();
        assert_eq!(output.into_bytes(), b"abcabc!");
        let mut output = History::new(32);
        output.start_frame(32);
        SequenceState::new()
            .decode(&[2, 0x54, 1, 2, 0, 16], b"ab", &mut output, 32)
            .unwrap();
        assert_eq!(output.into_bytes(), b"aaaabbbb");
    }

    #[test]
    fn repeat_offsets_follow_specification_examples() {
        let mut repeat = [1, 4, 8];
        for (value, literals, expected) in [
            (1114, 11, [1111, 1, 4]),
            (1, 22, [1111, 1, 4]),
            (2225, 22, [2222, 1111, 1]),
            (1114, 111, [1111, 2222, 1111]),
            (3336, 33, [3333, 1111, 2222]),
            (2, 22, [1111, 3333, 2222]),
            (3, 33, [2222, 1111, 3333]),
            (3, 0, [2221, 2222, 1111]),
            (1, 0, [2222, 2221, 1111]),
        ] {
            resolve_offset(&mut repeat, value, literals).unwrap();
            assert_eq!(repeat, expected);
        }
        assert_eq!(
            resolve_offset(&mut [1, 4, 8], 3, 0),
            Err(Error::CorruptData)
        );
    }

    #[test]
    fn rejects_invalid_modes_extra_bits_and_missing_history() {
        for input in [
            &[1, 0xfc, 1][..],
            &[0, 0][..],
            &[1, 0x57][..],
            &[1, 0x54, 0, 2, 0, 6][..],
            &[1, 0x54, 3, 2, 0, 14][..],
        ] {
            let mut output = History::new(32);
            output.start_frame(32);
            assert!(
                SequenceState::new()
                    .decode(input, b"abc", &mut output, 32)
                    .is_err()
            );
        }
    }
}
