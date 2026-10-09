//! Indexed, single-block parsing for the pinned no-dictionary Fast algorithm.
//!
//! The source and reusable owners are borrowed only for this call. Sampling
//! changes search history; returned repeat values remain uncommitted candidates.
//! Corresponds to `ZSTD_compressBlock_fast_noDict_generic` in zstd v1.5.7.
#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::{alloc::Allocator, ops::Range};

use crate::frame::MAX_BLOCK_SIZE;
use crate::match_key::MatchKey;
use crate::match_search::SearchKeyLayout;
use crate::matchfinder::MatchTable;
use crate::sequence_context::{LogicalSequence, SequenceWithOffset};
use crate::sequence_plan::OffsetIntent;
use crate::{Error, Result};

/// Checked source extent and effective settings, without an allocation owner.
pub(crate) struct FastBlock<'a> {
    input: &'a [u8],
    start: usize,
    end: usize,
    prefix_floor: usize,
    layout: SearchKeyLayout,
    window_size: usize,
    initial_step: usize,
}

impl<'a> FastBlock<'a> {
    /// Admit one direct parser call. Ordinary frames handle blocks below seven
    /// bytes before reaching this boundary, as does the reference dispatcher.
    pub(crate) fn new(
        input: &'a [u8],
        range: Range<usize>,
        prefix_floor: usize,
        layout: SearchKeyLayout,
        window_size: usize,
        initial_step: usize,
    ) -> Result<Self> {
        let length = range
            .end
            .checked_sub(range.start)
            .ok_or(Error::InvalidParameter)?;
        if range.end > input.len()
            || prefix_floor > range.start
            || !(1024..=1_048_576).contains(&window_size)
            || !window_size.is_power_of_two()
            || !(7..=MAX_BLOCK_SIZE.min(window_size)).contains(&length)
            || !(2..=131_073).contains(&initial_step)
            || SearchKeyLayout::for_fast(layout.hash_log(), layout.key_bytes())? != layout
        {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            input,
            start: range.start,
            end: range.end,
            prefix_floor,
            layout,
            window_size,
            initial_step,
        })
    }

    fn key(&self, position: usize) -> Result<MatchKey> {
        self.input
            .get(position..self.end)
            .and_then(|bytes| self.layout.read_key(bytes))
            .ok_or(Error::CorruptData)
    }

    fn four(&self, position: usize) -> Option<&[u8]> {
        let end = position.checked_add(4)?;
        if end > self.end {
            return None;
        }
        self.input.get(position..end)
    }

    fn repeated_match(&self, position: usize, distance: usize) -> bool {
        let Some(previous) = position.checked_sub(distance) else {
            return false;
        };
        distance != 0
            && distance <= self.window_size
            && previous >= self.prefix_floor
            && self
                .four(position)
                .zip(self.four(previous))
                .is_some_and(|(a, b)| a == b)
    }

    fn ordinary_match(
        &self,
        position: usize,
        previous: Option<usize>,
        floor: usize,
    ) -> Option<usize> {
        let previous = previous?;
        if previous >= position || previous < floor {
            return None;
        }
        self.four(position)
            .zip(self.four(previous))
            .filter(|(a, b)| a == b)
            .map(|_| previous)
    }

    fn count(&self, position: usize, previous: usize, mut length: usize) -> Result<usize> {
        if previous >= position
            || length > self.end.checked_sub(position).ok_or(Error::CorruptData)?
        {
            return Err(Error::CorruptData);
        }
        while length < self.end - position
            && self.input[position + length] == self.input[previous + length]
        {
            length += 1;
        }
        Ok(length)
    }
}

/// Raw parser cells and complete wire repeats have deliberately distinct roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FastParseResult {
    pub(crate) parser_repeats: [u32; 3],
    pub(crate) wire_repeats: [u32; 3],
    pub(crate) trailing_literals: usize,
}

/// A match plus the sampled position retained for post-match table insertion.
struct Found {
    position: usize,
    previous: usize,
    initial_length: usize,
    sampled_position: usize,
    intent: OffsetIntent,
}

fn advance(position: usize, amount: usize) -> Result<usize> {
    position.checked_add(amount).ok_or(Error::ResourceLimit)
}

/// Preserve cached reads and delayed writes across both positions of each pair.
fn search<A: Allocator>(
    block: &FastBlock<'_>,
    table: &mut MatchTable<A>,
    position: usize,
    anchor: usize,
    repeat: usize,
    floor: usize,
    limit: usize,
) -> Result<Option<Found>> {
    let mut step = block.initial_step;
    let mut next_step = advance(position, 128)?;
    let mut first = position;
    let mut second = advance(first, 1)?;
    let mut next_first = advance(first, step)?;
    let mut next_second = advance(next_first, 1)?;
    if next_second >= limit {
        return Ok(None);
    }
    let mut first_key = block.key(first)?;
    let mut second_key = block.key(second)?;
    let mut candidate = table.lookup(first_key)?;
    loop {
        let repeated = block.repeated_match(next_first, repeat);
        table.store(first_key, first)?;
        if repeated {
            let previous = next_first - repeat;
            // Entry repeat admission and the positive pair gap normally prove
            // both preceding bytes exist. Keep the indexed proof explicit.
            let backward = usize::from(
                next_first > anchor
                    && previous > block.prefix_floor
                    && block.input[next_first - 1] == block.input[previous - 1],
            );
            table.store(second_key, second)?;
            return Ok(Some(Found {
                position: next_first - backward,
                previous: previous - backward,
                initial_length: 4 + backward,
                sampled_position: first,
                intent: OffsetIntent::Repeat1,
            }));
        }
        if let Some(previous) = block.ordinary_match(first, candidate, floor) {
            table.store(second_key, second)?;
            return Ok(Some(Found {
                position: first,
                previous,
                initial_length: 4,
                sampled_position: first,
                intent: OffsetIntent::Explicit,
            }));
        }

        candidate = table.lookup(second_key)?;
        first_key = second_key;
        second_key = block.key(next_first)?;
        first = second;
        second = next_first;
        next_first = next_second;
        table.store(first_key, first)?;
        if let Some(previous) = block.ordinary_match(first, candidate, floor) {
            if step <= 4 {
                table.store(second_key, second)?;
            }
            return Ok(Some(Found {
                position: first,
                previous,
                initial_length: 4,
                sampled_position: first,
                intent: OffsetIntent::Explicit,
            }));
        }

        candidate = table.lookup(second_key)?;
        first_key = second_key;
        second_key = block.key(next_first)?;
        first = second;
        second = next_first;
        next_first = advance(first, step)?;
        next_second = advance(second, step)?;
        // These pair coordinates use the previous gap. The increased gap is
        // used by the following pair, matching the pinned sampling schedule.
        if next_first >= next_step {
            step = advance(step, 1)?;
            next_step = advance(next_step, 128)?;
        }
        if next_second >= limit {
            return Ok(None);
        }
    }
}

struct ParseOutput<'a, L: Allocator, S: Allocator> {
    literals: &'a mut Vec<u8, L>,
    sequences: &'a mut Vec<SequenceWithOffset, S>,
    anchor: usize,
    wire: [u32; 3],
}

impl<L: Allocator, S: Allocator> ParseOutput<'_, L, S> {
    fn emit(
        &mut self,
        block: &FastBlock<'_>,
        position: usize,
        previous: usize,
        length: usize,
        intent: OffsetIntent,
    ) -> Result<()> {
        let literal_length = position
            .checked_sub(self.anchor)
            .ok_or(Error::CorruptData)?;
        let distance = position.checked_sub(previous).ok_or(Error::CorruptData)?;
        if distance == 0
            || distance > block.window_size
            || previous < block.prefix_floor
            || length < 4
            || length > block.end.checked_sub(position).ok_or(Error::CorruptData)?
            || self.sequences.len() >= (block.end - block.start) / 4
        {
            return Err(Error::CorruptData);
        }
        let sequence = LogicalSequence {
            literal_length: u32::try_from(literal_length).map_err(|_| Error::ResourceLimit)?,
            match_length: u32::try_from(length).map_err(|_| Error::ResourceLimit)?,
            distance: u32::try_from(distance).map_err(|_| Error::ResourceLimit)?,
        };
        let resolved = intent.resolve(sequence.literal_length, sequence.distance, self.wire)?;
        // Disjoint literal ranges total at most the block size. Each sequence
        // consumes at least four source bytes, so both owners' initial reserves
        // cover these Copy pushes without another allocation or allocator call.
        for &byte in &block.input[self.anchor..position] {
            self.literals.push(byte);
        }
        self.sequences.push(SequenceWithOffset {
            sequence,
            offset: intent,
        });
        self.wire = resolved.next_repeat;
        self.anchor = position + length;
        Ok(())
    }
}

/// Parse without resetting sampled history or publishing entropy/repeat state.
///
/// Source/view, layout and repeat validation precede reservations. Both reserves
/// precede clearing contents or changing the table. A failed reserve can retain
/// increased capacity in the other owner, but preserves both logical outputs
/// and table. Later private/table errors may leave partial scratch and samples;
/// discard the failed parse and reset before supplying a different source.
/// All literals, including the final suffix, are returned in `literals`.
pub(crate) fn parse_block<A: Allocator, L: Allocator, S: Allocator>(
    block: FastBlock<'_>,
    table: &mut MatchTable<A>,
    initial_repeats: [u32; 3],
    literals: &mut Vec<u8, L>,
    sequences: &mut Vec<SequenceWithOffset, S>,
) -> Result<FastParseResult> {
    if table.layout() != block.layout {
        return Err(Error::InvalidParameter);
    }
    if initial_repeats.contains(&0) {
        return Err(Error::CorruptData);
    }
    let mut first_repeat = usize::try_from(initial_repeats[0]).map_err(|_| Error::ResourceLimit)?;
    let mut second_repeat =
        usize::try_from(initial_repeats[1]).map_err(|_| Error::ResourceLimit)?;
    let length = block.end - block.start;
    if literals.capacity() < length {
        literals
            .try_reserve_exact(length - literals.len())
            .map_err(|_| Error::AllocationFailed)?;
    }
    if sequences.capacity() < length / 4 {
        sequences
            .try_reserve_exact(length / 4 - sequences.len())
            .map_err(|_| Error::AllocationFailed)?;
    }
    literals.clear();
    sequences.clear();
    let mut output = ParseOutput {
        literals,
        sequences,
        anchor: block.start,
        wire: initial_repeats,
    };
    let floor = block
        .prefix_floor
        .max(block.end.saturating_sub(block.window_size));
    let mut position = advance(block.start, usize::from(block.start == floor))?;
    let repeat_floor = block
        .prefix_floor
        .max(position.saturating_sub(block.window_size));
    let max_repeat = position - repeat_floor;
    let saved_first = if first_repeat > max_repeat {
        let saved = first_repeat;
        first_repeat = 0;
        saved
    } else {
        0
    };
    let mut saved_second = if second_repeat > max_repeat {
        let saved = second_repeat;
        second_repeat = 0;
        saved
    } else {
        0
    };

    // A seven-byte block has no search positions. Avoid representing C's
    // physically padded iend-8 pointer as a negative Rust source index.
    if length >= 8 {
        let limit = block.end - 8;
        while let Some(mut found) = search(
            &block,
            table,
            position,
            output.anchor,
            first_repeat,
            floor,
            limit,
        )? {
            if found.intent == OffsetIntent::Explicit {
                second_repeat = first_repeat;
                first_repeat = found.position - found.previous;
                while found.position > output.anchor
                    && found.previous > floor
                    && block.input[found.position - 1] == block.input[found.previous - 1]
                {
                    found.position -= 1;
                    found.previous -= 1;
                    found.initial_length += 1;
                }
            }
            let matched = block.count(found.position, found.previous, found.initial_length)?;
            output.emit(
                &block,
                found.position,
                found.previous,
                matched,
                found.intent,
            )?;
            position = output.anchor;
            if position <= limit {
                let sampled = advance(found.sampled_position, 2)?;
                table.store(block.key(sampled)?, sampled)?;
                table.store(block.key(position - 2)?, position - 2)?;
                while position <= limit && block.repeated_match(position, second_repeat) {
                    let previous = position - second_repeat;
                    let matched = block.count(position, previous, 4)?;
                    core::mem::swap(&mut first_repeat, &mut second_repeat);
                    table.store(block.key(position)?, position)?;
                    output.emit(&block, position, previous, matched, OffsetIntent::Repeat1)?;
                    position = output.anchor;
                }
            }
        }
    }
    if saved_first != 0 && first_repeat != 0 {
        saved_second = saved_first;
    }
    let parser_repeats = [
        u32::try_from(if first_repeat == 0 {
            saved_first
        } else {
            first_repeat
        })
        .map_err(|_| Error::ResourceLimit)?,
        u32::try_from(if second_repeat == 0 {
            saved_second
        } else {
            second_repeat
        })
        .map_err(|_| Error::ResourceLimit)?,
        initial_repeats[2],
    ];
    let trailing_literals = block.end - output.anchor;
    for &byte in &block.input[output.anchor..block.end] {
        output.literals.push(byte);
    }
    Ok(FastParseResult {
        parser_repeats,
        wire_repeats: output.wire,
        trailing_literals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{alloc::Global, vec};
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn layout() -> SearchKeyLayout {
        SearchKeyLayout::for_fast(6, 4).unwrap()
    }

    fn block(input: &[u8], start: usize) -> FastBlock<'_> {
        FastBlock::new(input, start..input.len(), 0, layout(), 1024, 2).unwrap()
    }

    fn seed<A: Allocator>(table: &mut MatchTable<A>, input: &[u8], position: usize) {
        let key = table.layout().read_key(&input[position..]).unwrap();
        table.store(key, position).unwrap();
    }

    fn replay(
        input: &[u8],
        start: usize,
        literals: &[u8],
        sequences: &[SequenceWithOffset],
        initial: [u32; 3],
        result: FastParseResult,
    ) {
        let mut decoded = input[..start].to_vec();
        let mut cursor = 0;
        let mut repeats = initial;
        for record in sequences {
            let s = record.sequence;
            let ll = s.literal_length as usize;
            decoded.extend_from_slice(&literals[cursor..cursor + ll]);
            cursor += ll;
            let transition = record
                .offset
                .resolve(s.literal_length, s.distance, repeats)
                .unwrap();
            repeats = transition.next_repeat;
            for _ in 0..s.match_length {
                decoded.push(decoded[decoded.len() - s.distance as usize]);
            }
        }
        assert_eq!(literals.len() - cursor, result.trailing_literals);
        decoded.extend_from_slice(&literals[cursor..]);
        assert_eq!(decoded, input);
        assert_eq!(repeats, result.wire_repeats);
    }

    #[test]
    fn all_frozen_direct_cases_match_sequences_literals_repeats_and_complete_tables() {
        let cases = crate::fast_oracle_fixture::cases().unwrap();
        assert_eq!(cases.len(), 142);
        for case in cases {
            let layout = SearchKeyLayout::for_fast(case.hash_log, case.min_match.max(4)).unwrap();
            let mut table = MatchTable::with_layout_in(layout, Global).unwrap();
            for &position in &case.seeds {
                seed(&mut table, case.source, position as usize);
            }
            let mut expected_table = vec![0_u32; layout.bucket_count()];
            for &[index, position] in &case.table_before {
                expected_table[index as usize] = position;
            }
            for &[index, previous, position] in &case.table_changes {
                assert_eq!(expected_table[index as usize], previous);
                expected_table[index as usize] = position;
            }
            let view = FastBlock::new(
                case.source,
                case.prefix..case.source.len(),
                case.prefix_floor,
                layout,
                1 << case.window_log,
                (case.target_length as usize).max(1) + 1,
            )
            .unwrap();
            let mut literals = Vec::new();
            let mut sequences = Vec::new();
            let result = parse_block(
                view,
                &mut table,
                case.initial_repeats,
                &mut literals,
                &mut sequences,
            )
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.name));
            assert_eq!(sequences.len(), case.sequences.len(), "{}", case.name);
            let mut repeats = case.initial_repeats;
            for (actual, expected) in sequences.iter().zip(&case.sequences) {
                let s = actual.sequence;
                let transition = actual
                    .offset
                    .resolve(s.literal_length, s.distance, repeats)
                    .unwrap();
                assert_eq!(
                    [
                        s.literal_length,
                        s.match_length,
                        s.distance,
                        transition.offset_value
                    ],
                    [
                        expected.literal_length,
                        expected.match_length,
                        expected.distance,
                        expected.offset_value
                    ],
                    "{}",
                    case.name
                );
                repeats = transition.next_repeat;
            }
            assert_eq!(result.parser_repeats, case.parser_repeats, "{}", case.name);
            assert_eq!(result.wire_repeats, case.wire_repeats, "{}", case.name);
            assert_eq!(
                result.trailing_literals, case.trailing_literals,
                "{}",
                case.name
            );
            assert_eq!(
                &literals[..literals.len() - result.trailing_literals],
                case.literals,
                "{}",
                case.name
            );
            assert_eq!(
                &literals[literals.len() - result.trailing_literals..],
                &case.source[case.source.len() - result.trailing_literals..],
                "{}",
                case.name
            );
            for (index, &position) in table.test_positions().iter().enumerate() {
                let raw = if position == usize::MAX {
                    0
                } else {
                    u32::try_from(position).unwrap() + 2
                };
                assert_eq!(raw, expected_table[index], "{} bucket {index}", case.name);
            }
            replay(
                case.source,
                case.prefix,
                &literals,
                &sequences,
                case.initial_repeats,
                result,
            );
        }
    }

    #[test]
    fn first_probe_reads_the_seed_before_a_colliding_store_and_restores_disabled_repeats() {
        let input = [b'x'; 96];
        let mut table = MatchTable::with_layout_in(layout(), Global).unwrap();
        seed(&mut table, &input, 0);
        let mut literals = Vec::new();
        let mut sequences = Vec::new();
        let initial = [257, 263, 269];
        let result = parse_block(
            block(&input, 32),
            &mut table,
            initial,
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(
            sequences,
            [SequenceWithOffset {
                sequence: LogicalSequence {
                    literal_length: 0,
                    match_length: 64,
                    distance: 32
                },
                offset: OffsetIntent::Explicit
            }]
        );
        assert_eq!(result.parser_repeats, [32, 257, 269]);
        assert_eq!(result.wire_repeats, [32, 257, 263]);
        replay(&input, 32, &literals, &sequences, initial, result);
    }

    #[test]
    fn overlapping_repeats_and_seven_byte_cleanup_preserve_wire_state() {
        for length in [7, 8, 9, 12, 16, 64] {
            let input = vec![b'a'; length];
            let mut table = MatchTable::with_layout_in(layout(), Global).unwrap();
            let mut literals = Vec::new();
            let mut sequences = Vec::new();
            let result = parse_block(
                block(&input, 0),
                &mut table,
                [1, 4, 8],
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            replay(&input, 0, &literals, &sequences, [1, 4, 8], result);
            if length < 12 {
                assert!(sequences.is_empty());
                assert_eq!(literals, input);
                assert!(table.test_positions().iter().all(|&p| p == usize::MAX));
            }
            if length == 64 {
                assert!(sequences.iter().any(|s| s.sequence.distance == 1));
            }
        }
    }

    #[test]
    fn a_block_view_never_consumes_initialized_future_source_bytes() {
        let mut expected = None;
        for future in *b"aq" {
            let mut input = [future; 128];
            input[..64].fill(b'a');
            let mut table = MatchTable::with_layout_in(layout(), Global).unwrap();
            let mut literals = Vec::new();
            let mut sequences = Vec::new();
            let view = FastBlock::new(&input, 0..64, 0, layout(), 1024, 2).unwrap();
            let result =
                parse_block(view, &mut table, [1, 4, 8], &mut literals, &mut sequences).unwrap();
            replay(&input[..64], 0, &literals, &sequences, [1, 4, 8], result);
            assert!(
                table
                    .test_positions()
                    .iter()
                    .all(|&p| p == usize::MAX || p + 4 <= 64)
            );
            let observed = (literals, sequences, result, table.test_positions().to_vec());
            if let Some(expected) = &expected {
                assert_eq!(&observed, expected);
            } else {
                expected = Some(observed);
            }
        }
    }

    #[test]
    fn range_shape_and_scalar_extremes_are_checked_without_source_access() {
        let input = [0; 64];
        for (start, end, floor) in [
            (0, 6, 0),
            (8, 7, 0),
            (0, 65, 0),
            (0, 7, 1),
            (usize::MAX, usize::MAX, 0),
            (1, usize::MAX, 0),
        ] {
            assert!(matches!(
                FastBlock::new(&input, start..end, floor, layout(), 1024, 2),
                Err(Error::InvalidParameter)
            ));
        }
        for window in [0, 1, 1023, 1025, 1_048_577, usize::MAX] {
            assert!(matches!(
                FastBlock::new(&input, 0..64, 0, layout(), window, 2),
                Err(Error::InvalidParameter)
            ));
        }
        for step in [0, 1, 131_074, usize::MAX] {
            assert!(matches!(
                FastBlock::new(&input, 0..64, 0, layout(), 1024, step),
                Err(Error::InvalidParameter)
            ));
        }
        let wide = SearchKeyLayout::new(10, 8).unwrap();
        assert!(matches!(
            FastBlock::new(&input, 0..64, 0, wide, 1024, 2),
            Err(Error::InvalidParameter)
        ));
        assert_eq!(advance(usize::MAX, 1), Err(Error::ResourceLimit));
        assert_eq!(advance(usize::MAX - 128, 128), Ok(usize::MAX));
    }

    #[test]
    fn unusable_seed_positions_never_become_source_reads_or_matches() {
        let input = [b'x'; 64];
        let mut expected = None;
        for position in [64, usize::MAX - 1, usize::MAX] {
            let mut table = MatchTable::with_layout_in(layout(), Global).unwrap();
            table
                .store(layout().read_key(&input).unwrap(), position)
                .unwrap();
            let mut literals = Vec::new();
            let mut sequences = Vec::new();
            let result = parse_block(
                block(&input, 0),
                &mut table,
                [257, 263, 269],
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            replay(&input, 0, &literals, &sequences, [257, 263, 269], result);
            let observed = (literals, sequences, result, table.test_positions().to_vec());
            if let Some(expected) = &expected {
                assert_eq!(&observed, expected);
            } else {
                expected = Some(observed);
            }
        }
    }

    fn old_sequence() -> SequenceWithOffset {
        SequenceWithOffset {
            sequence: LogicalSequence {
                literal_length: 0,
                match_length: 4,
                distance: 1,
            },
            offset: OffsetIntent::Explicit,
        }
    }

    #[test]
    fn invalid_snapshot_or_layout_preserves_armed_owners_and_logical_buffers() {
        let literal_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let sequence_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let lm = literal_owner.monitor();
        let sm = sequence_owner.monitor();
        let mut literals = Vec::new_in(literal_owner);
        literals.try_reserve_exact(1).unwrap();
        literals.push(7);
        let mut sequences = Vec::new_in(sequence_owner);
        sequences.try_reserve_exact(1).unwrap();
        sequences.push(old_sequence());
        let input = [b'x'; 64];
        let mut table = MatchTable::with_layout_in(layout(), Global).unwrap();
        seed(&mut table, &input, 0);
        let before_table = table.test_positions().to_vec();
        assert!(lm.fail_next());
        assert!(sm.fail_next());
        let before_l = lm.snapshot();
        let before_s = sm.snapshot();
        assert_eq!(
            parse_block(
                block(&input, 0),
                &mut table,
                [0, 4, 8],
                &mut literals,
                &mut sequences
            ),
            Err(Error::CorruptData)
        );
        let view = FastBlock::new(
            &input,
            0..64,
            0,
            SearchKeyLayout::for_fast(7, 4).unwrap(),
            1024,
            2,
        )
        .unwrap();
        assert_eq!(
            parse_block(view, &mut table, [1, 4, 8], &mut literals, &mut sequences),
            Err(Error::InvalidParameter)
        );
        assert_eq!(lm.snapshot(), before_l);
        assert_eq!(sm.snapshot(), before_s);
        assert_eq!(literals.as_slice(), [7]);
        assert_eq!(sequences.as_slice(), [old_sequence()]);
        assert_eq!(table.test_positions(), before_table);
        drop(literals);
        drop(sequences);
        lm.assert_released();
        sm.assert_released();
    }

    #[test]
    fn both_output_reservation_failures_preserve_contents_then_retry_with_exact_owners() {
        for fail_literals in [true, false] {
            let table_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let literal_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let sequence_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let tm = table_owner.monitor();
            let lm = literal_owner.monitor();
            let sm = sequence_owner.monitor();
            let input = [b'x'; 96];
            let mut table = MatchTable::with_layout_in(layout(), table_owner).unwrap();
            seed(&mut table, &input, 0);
            let mut literals = Vec::new_in(literal_owner);
            literals.try_reserve_exact(3).unwrap();
            for b in [3, 2, 1] {
                literals.push(b);
            }
            let mut sequences = Vec::new_in(sequence_owner);
            sequences.try_reserve_exact(1).unwrap();
            sequences.push(old_sequence());
            let before_table = table.test_positions().to_vec();
            let table_snapshot = tm.snapshot();
            assert!(if fail_literals {
                lm.fail_next()
            } else {
                sm.fail_next()
            });
            assert_eq!(
                parse_block(
                    block(&input, 32),
                    &mut table,
                    [257, 263, 269],
                    &mut literals,
                    &mut sequences
                ),
                Err(Error::AllocationFailed)
            );
            assert_eq!(literals.as_slice(), [3, 2, 1]);
            assert_eq!(sequences.as_slice(), [old_sequence()]);
            assert_eq!(table.test_positions(), before_table);
            assert_eq!(tm.snapshot(), table_snapshot);
            assert_eq!(lm.snapshot().failures + sm.snapshot().failures, 1);
            lm.clear_failure();
            sm.clear_failure();
            let result = parse_block(
                block(&input, 32),
                &mut table,
                [257, 263, 269],
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            replay(&input, 32, &literals, &sequences, [257, 263, 269], result);
            assert_eq!(result.parser_repeats, [32, 257, 269]);
            drop(table);
            drop(literals);
            drop(sequences);
            for monitor in [tm, lm, sm] {
                monitor.assert_released();
                assert_eq!(
                    monitor.snapshot().owners_dropped,
                    monitor.snapshot().identities_created
                );
            }
        }
    }

    #[test]
    fn borrowed_nonclone_owners_reuse_capacity_without_requests_after_reset() {
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let input = [b'a'; 96];
        let mut table = MatchTable::with_layout_in(layout(), &owner).unwrap();
        let mut literals = Vec::new_in(&owner);
        let mut sequences = Vec::new_in(&owner);
        let first = parse_block(
            block(&input, 0),
            &mut table,
            [1, 4, 8],
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        let expected = (
            literals.to_vec(),
            sequences.to_vec(),
            table.test_positions().to_vec(),
        );
        table.reset();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        let second = parse_block(
            block(&input, 0),
            &mut table,
            [1, 4, 8],
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(literals.as_slice(), expected.0);
        assert_eq!(sequences.as_slice(), expected.1);
        assert_eq!(table.test_positions(), expected.2);
        assert_eq!(monitor.snapshot(), before);
        drop(table);
        drop(literals);
        drop(sequences);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(owner);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
}
