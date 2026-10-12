//! Indexed parsing for the pinned no-dictionary DoubleFast algorithm.
//!
//! Search history is mutated independently of candidate wire repeats. All source
//! positions are checked indices, and both table owners keep their own allocator.
//! Corresponds to `ZSTD_compressBlock_doubleFast_noDict_generic` in zstd v1.5.7.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "DoubleFast frame integration follows")
)]

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::double_fast_block::{DoubleFastBlock, DoubleFastCursor};
use crate::double_fast_tables::DoubleFastTables;
use crate::match_key::MatchKey;
use crate::sequence_context::{LogicalSequence, SequenceWithOffset};
use crate::sequence_plan::OffsetIntent;
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DoubleFastParseResult {
    pub(crate) parser_repeats: [u32; 3],
    pub(crate) wire_repeats: [u32; 3],
    pub(crate) trailing_literals: usize,
}

/// Source-verified match, retaining the sampled position before any extension.
struct Found {
    position: usize,
    previous: usize,
    length: usize,
    sampled: usize,
    intent: OffsetIntent,
}

fn equal(block: &DoubleFastBlock<'_>, first: usize, second: usize, width: usize) -> bool {
    let view = |position: usize| {
        let end = position.checked_add(width)?;
        if end > block.range().end {
            return None;
        }
        block.source().get(position..end)
    };
    view(first).zip(view(second)).is_some_and(|(a, b)| a == b)
}

fn repeated(block: &DoubleFastBlock<'_>, position: usize, distance: usize) -> Option<usize> {
    let previous = position.checked_sub(distance)?;
    (distance != 0
        && distance <= block.window_size()
        && previous >= block.prefix_floor()
        && equal(block, position, previous, 4))
    .then_some(previous)
}

fn candidate(
    block: &DoubleFastBlock<'_>,
    position: usize,
    previous: Option<usize>,
    width: usize,
    strict: bool,
) -> Option<usize> {
    let previous = previous?;
    let floor = block.ordinary_floor();
    (previous < position
        && previous >= floor
        && (!strict || previous != floor)
        && equal(block, position, previous, width))
    .then_some(previous)
}

fn count(
    block: &DoubleFastBlock<'_>,
    position: usize,
    previous: usize,
    mut length: usize,
) -> Result<usize> {
    let available = block
        .range()
        .end
        .checked_sub(position)
        .ok_or(Error::CorruptData)?;
    if previous >= position || length > available {
        return Err(Error::CorruptData);
    }
    // previous < position and length < end-position prove both indices lie
    // within the validated block-end extent, without relying on input contents.
    while length < available
        && block.source()[position + length] == block.source()[previous + length]
    {
        length += 1;
    }
    Ok(length)
}

fn extend_backward(block: &DoubleFastBlock<'_>, anchor: usize, found: &mut Found) {
    while found.position > anchor
        && found.previous > block.ordinary_floor()
        && block.source()[found.position - 1] == block.source()[found.previous - 1]
    {
        found.position -= 1;
        found.previous -= 1;
        found.length += 1;
    }
}

fn explicit(
    block: &DoubleFastBlock<'_>,
    anchor: usize,
    cursor: DoubleFastCursor,
    position: usize,
    previous: usize,
    length: usize,
) -> Found {
    let mut found = Found {
        position,
        previous,
        length,
        sampled: cursor.current(),
        intent: OffsetIntent::Explicit,
    };
    extend_backward(block, anchor, &mut found);
    found
}

/// Retain the current long candidate across table stores and the next probe.
/// A later long lookup must see the current store when its bucket collides.
fn search<L: Allocator, S: Allocator>(
    block: &DoubleFastBlock<'_>,
    tables: &mut DoubleFastTables<L, S>,
    position: usize,
    anchor: usize,
    repeat: usize,
) -> Result<Option<Found>> {
    let Some(mut cursor) = block.search_from(position)? else {
        return Ok(None);
    };
    let mut long_key = block.long_key(cursor.current())?;
    let mut long_candidate = tables.long.lookup(long_key)?;
    loop {
        let current = cursor.current();
        let short_key = block.short_key(current)?;
        let short_candidate = tables.short.lookup(short_key)?;
        tables.short.store(short_key, current)?;
        tables.long.store(long_key, current)?;

        // The repeat probe is current+1, even when the next ordinary probe's
        // sampling gap has grown. Cursor bounds prove this index is in range.
        let repeat_position = current.checked_add(1).ok_or(Error::ResourceLimit)?;
        if let Some(previous) = repeated(block, repeat_position, repeat) {
            return Ok(Some(Found {
                position: repeat_position,
                previous,
                length: count(block, repeat_position, previous, 4)?,
                sampled: current,
                intent: OffsetIntent::Repeat1,
            }));
        }

        let next_key = block.long_key(cursor.next())?;
        if let Some(previous) = candidate(block, current, long_candidate, 8, false) {
            let found = explicit(
                block,
                anchor,
                cursor,
                current,
                previous,
                count(block, current, previous, 8)?,
            );
            store_deferred(tables, cursor, next_key)?;
            return Ok(Some(found));
        }
        let next_candidate = tables.long.lookup(next_key)?;
        if let Some(previous) = candidate(block, current, short_candidate, 4, false) {
            let mut position = current;
            let mut previous = previous;
            let mut length = count(block, position, previous, 4)?;
            if let Some(later) = candidate(block, cursor.next(), next_candidate, 8, true) {
                let later_length = count(block, cursor.next(), later, 8)?;
                // Ties retain the short match. Do not subtract the sampling gap
                // from the later match length when applying this preference.
                if later_length > length {
                    position = cursor.next();
                    previous = later;
                    length = later_length;
                }
            }
            let found = explicit(block, anchor, cursor, position, previous, length);
            store_deferred(tables, cursor, next_key)?;
            return Ok(Some(found));
        }
        let Some(next) = cursor.advance()? else {
            return Ok(None);
        };
        cursor = next;
        long_key = next_key;
        long_candidate = next_candidate;
    }
}

fn store_deferred<L: Allocator, S: Allocator>(
    tables: &mut DoubleFastTables<L, S>,
    cursor: DoubleFastCursor,
    key: MatchKey,
) -> Result<()> {
    // Every match consumes at least four bytes. A gap below four keeps this
    // sampled next position strictly before the match end, including catch-up.
    if cursor.step() < 4 {
        tables.long.store(key, cursor.next())?;
    }
    Ok(())
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
        block: &DoubleFastBlock<'_>,
        position: usize,
        previous: usize,
        length: usize,
        intent: OffsetIntent,
    ) -> Result<()> {
        let literal_length = position
            .checked_sub(self.anchor)
            .ok_or(Error::CorruptData)?;
        let distance = position.checked_sub(previous).ok_or(Error::CorruptData)?;
        let range = block.range();
        if distance == 0
            || distance > block.window_size()
            || previous < block.prefix_floor()
            || length < 4
            || length > range.end.checked_sub(position).ok_or(Error::CorruptData)?
            || self.sequences.len() >= (range.end - range.start) / 4
        {
            return Err(Error::CorruptData);
        }
        let sequence = LogicalSequence {
            literal_length: u32::try_from(literal_length).map_err(|_| Error::ResourceLimit)?,
            match_length: u32::try_from(length).map_err(|_| Error::ResourceLimit)?,
            distance: u32::try_from(distance).map_err(|_| Error::ResourceLimit)?,
        };
        let transition = intent.resolve(sequence.literal_length, sequence.distance, self.wire)?;
        // Literal extents are disjoint, and each sequence consumes >=4 bytes.
        // Both initial reservations therefore cover all pushes without growth.
        self.literals
            .extend_from_slice(&block.source()[self.anchor..position]);
        self.sequences.push(SequenceWithOffset {
            sequence,
            offset: intent,
        });
        self.anchor = position + length;
        self.wire = transition.next_repeat;
        Ok(())
    }
}

/// Parse a checked block without resetting either table or publishing repeats.
///
/// Layout/repeat checks and both fallible output reserves precede all logical
/// mutation. A failed second reserve may retain increased literal capacity but
/// preserves both output contents and tables. Later errors may leave partial
/// sampling and scratch; a persistent caller must poison/discard that state.
/// Returned literals include the trailing suffix, while raw parser repeats and
/// complete wire repeats deliberately remain separate candidate values.
pub(crate) fn parse_block<A: Allocator, B: Allocator, L: Allocator, S: Allocator>(
    block: DoubleFastBlock<'_>,
    tables: &mut DoubleFastTables<A, B>,
    initial_repeats: [u32; 3],
    literals: &mut Vec<u8, L>,
    sequences: &mut Vec<SequenceWithOffset, S>,
) -> Result<DoubleFastParseResult> {
    if tables.layouts() != block.layouts() {
        return Err(Error::InvalidParameter);
    }
    if initial_repeats.contains(&0) {
        return Err(Error::CorruptData);
    }
    let mut first_repeat = usize::try_from(initial_repeats[0]).map_err(|_| Error::ResourceLimit)?;
    let mut second_repeat =
        usize::try_from(initial_repeats[1]).map_err(|_| Error::ResourceLimit)?;
    let range = block.range();
    let length = range.end - range.start;
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
        anchor: range.start,
        wire: initial_repeats,
    };
    let mut position = block.initial_position();
    let max_repeat = position - block.repeat_floor();
    let saved_first = if first_repeat > max_repeat {
        core::mem::replace(&mut first_repeat, 0)
    } else {
        0
    };
    let mut saved_second = if second_repeat > max_repeat {
        core::mem::replace(&mut second_repeat, 0)
    } else {
        0
    };
    while let Some(found) = search(&block, tables, position, output.anchor, first_repeat)? {
        if found.intent == OffsetIntent::Explicit {
            second_repeat = first_repeat;
            first_repeat = found.position - found.previous;
        }
        output.emit(
            &block,
            found.position,
            found.previous,
            found.length,
            found.intent,
        )?;
        position = output.anchor;
        if range
            .end
            .checked_sub(8)
            .is_some_and(|limit| position <= limit)
        {
            let sampled = found.sampled.checked_add(2).ok_or(Error::ResourceLimit)?;
            let before_two = position.checked_sub(2).ok_or(Error::CorruptData)?;
            let before_one = position.checked_sub(1).ok_or(Error::CorruptData)?;
            tables.long.store(block.long_key(sampled)?, sampled)?;
            tables.long.store(block.long_key(before_two)?, before_two)?;
            tables.short.store(block.short_key(sampled)?, sampled)?;
            tables
                .short
                .store(block.short_key(before_one)?, before_one)?;
            while range
                .end
                .checked_sub(8)
                .is_some_and(|limit| position <= limit)
            {
                let Some(previous) = repeated(&block, position, second_repeat) else {
                    break;
                };
                let length = count(&block, position, previous, 4)?;
                core::mem::swap(&mut first_repeat, &mut second_repeat);
                tables.short.store(block.short_key(position)?, position)?;
                tables.long.store(block.long_key(position)?, position)?;
                output.emit(&block, position, previous, length, OffsetIntent::Repeat1)?;
                position = output.anchor;
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
    let trailing_literals = range.end - output.anchor;
    output
        .literals
        .extend_from_slice(&block.source()[output.anchor..range.end]);
    Ok(DoubleFastParseResult {
        parser_repeats,
        wire_repeats: output.wire,
        trailing_literals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::double_fast_oracle_fixture::{OracleCase, cases};
    use crate::match_search::SearchKeyLayout;
    use crate::matchfinder::MatchTable;
    use alloc::alloc::Global;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn block<'a>(case: &OracleCase<'_>, source: &'a [u8]) -> DoubleFastBlock<'a> {
        DoubleFastBlock::new(
            source,
            case.prefix..case.source.len(),
            case.prefix_floor,
            SearchKeyLayout::for_double_fast_long(case.long_log).unwrap(),
            SearchKeyLayout::for_fast(case.short_log, case.min_match.max(4)).unwrap(),
            1 << case.window_log,
        )
        .unwrap()
    }

    fn seed<A: Allocator>(table: &mut MatchTable<A>, source: &[u8], seeds: &[u32]) {
        for &position in seeds {
            let position = position as usize;
            let key = table.layout().read_key(&source[position..]).unwrap();
            table.store(key, position).unwrap();
        }
    }

    fn raw_cells<A: Allocator>(table: &MatchTable<A>) -> Vec<u32> {
        table
            .test_positions()
            .iter()
            .map(|&position| {
                if position == usize::MAX {
                    0
                } else {
                    u32::try_from(position).unwrap().checked_add(2).unwrap()
                }
            })
            .collect()
    }

    fn compare(
        case: &OracleCase<'_>,
        result: DoubleFastParseResult,
        literals: &[u8],
        sequences: &[SequenceWithOffset],
    ) {
        assert_eq!(
            result.parser_repeats, case.parser_repeats,
            "{} parser repeats",
            case.name
        );
        assert_eq!(
            result.wire_repeats, case.wire_repeats,
            "{} wire repeats",
            case.name
        );
        assert_eq!(
            result.trailing_literals, case.trailing_literals,
            "{} trailing",
            case.name
        );
        assert_eq!(
            literals.len(),
            case.literals.len() + case.trailing_literals,
            "{} literal bytes",
            case.name
        );
        assert_eq!(
            &literals[..case.literals.len()],
            case.literals,
            "{} literals",
            case.name
        );
        assert_eq!(
            &literals[case.literals.len()..],
            &case.source[case.source.len() - case.trailing_literals..]
        );
        assert_eq!(
            sequences.len(),
            case.sequences.len(),
            "{} sequence count",
            case.name
        );
        let mut wire = case.initial_repeats;
        let mut decoded = case.source[..case.prefix].to_vec();
        let mut literal_cursor = 0;
        let mut marker = None;
        for (index, (actual, expected)) in sequences.iter().zip(&case.sequences).enumerate() {
            let seq = actual.sequence;
            assert_eq!(
                (seq.literal_length, seq.match_length, seq.distance),
                (
                    expected.literal_length,
                    expected.match_length,
                    expected.distance
                ),
                "{} sequence{index}",
                case.name
            );
            let resolved = actual
                .offset
                .resolve(seq.literal_length, seq.distance, wire)
                .unwrap();
            assert_eq!(
                resolved.offset_value, expected.offset_value,
                "{} intent{index}",
                case.name
            );
            wire = resolved.next_repeat;
            let ll = seq.literal_length as usize;
            decoded.extend_from_slice(&literals[literal_cursor..literal_cursor + ll]);
            literal_cursor += ll;
            for _ in 0..seq.match_length {
                decoded.push(decoded[decoded.len() - seq.distance as usize]);
            }
            for (kind, length) in [(1, seq.literal_length), (2, seq.match_length - 3)] {
                if length >= 65536 {
                    assert!(marker.replace((kind, index)).is_none());
                }
            }
        }
        decoded.extend_from_slice(&literals[literal_cursor..]);
        assert_eq!(decoded, case.source, "{} replay", case.name);
        assert_eq!(wire, case.wire_repeats);
        assert_eq!(
            marker.unwrap_or((0, 0)),
            (case.long_length_kind, case.long_length_position)
        );
    }

    #[test]
    fn every_frozen_sequence_repeat_literal_and_complete_table_matches() {
        for case in cases().unwrap() {
            let mut extended = case.source.to_vec();
            extended.extend_from_slice(&[0xa7; 32]);
            // The second view exposes initialized later bytes but the logical
            // parser block ends at the same position. No physical C slack is
            // required, and neither parse may consume or hash the later bytes.
            for source in [case.source, extended.as_slice()] {
                let block = block(&case, source);
                let (long, short) = block.layouts();
                let mut tables = DoubleFastTables::new_in(long, short, Global, Global).unwrap();
                seed(&mut tables.long, source, &case.long_table.seeds);
                seed(&mut tables.short, source, &case.short_table.seeds);
                let mut literals = Vec::new();
                let mut sequences = Vec::new();
                let result = parse_block(
                    block,
                    &mut tables,
                    case.initial_repeats,
                    &mut literals,
                    &mut sequences,
                )
                .unwrap_or_else(|e| panic!("{}: {e:?}", case.name));
                compare(&case, result, &literals, &sequences);
                assert_eq!(
                    raw_cells(&tables.long),
                    case.long_table.after(&case, long).unwrap(),
                    "{} long table",
                    case.name
                );
                assert_eq!(
                    raw_cells(&tables.short),
                    case.short_table.after(&case, short).unwrap(),
                    "{} short table",
                    case.name
                );
            }
        }
    }

    #[test]
    fn reused_outputs_and_borrowed_nonclone_owners_make_no_new_requests() {
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let all = cases().unwrap();
        let case = all
            .iter()
            .find(|c| c.name == "long-literal-extended-tail")
            .unwrap();
        let block = block(case, case.source);
        let (long, short) = block.layouts();
        let mut tables = DoubleFastTables::new_in(long, short, &owner, &owner).unwrap();
        let mut literals = Vec::new_in(&owner);
        let mut sequences = Vec::new_in(&owner);
        for pass in 0..2 {
            tables.reset();
            seed(&mut tables.long, case.source, &case.long_table.seeds);
            seed(&mut tables.short, case.source, &case.short_table.seeds);
            let before = monitor.snapshot().attempts;
            let result = parse_block(
                block,
                &mut tables,
                case.initial_repeats,
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            compare(case, result, &literals, &sequences);
            if pass == 1 {
                assert_eq!(monitor.snapshot().attempts, before);
            }
        }
        assert_eq!(monitor.snapshot().allocate_calls, 4);
        assert_eq!(monitor.snapshot().grow_calls, 0);
        drop(tables);
        drop(literals);
        drop(sequences);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(owner);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }

    fn small(source: &[u8]) -> DoubleFastBlock<'_> {
        DoubleFastBlock::new(
            source,
            0..source.len(),
            0,
            SearchKeyLayout::for_double_fast_long(6).unwrap(),
            SearchKeyLayout::for_fast(7, 4).unwrap(),
            1024,
        )
        .unwrap()
    }

    fn sentinel() -> SequenceWithOffset {
        SequenceWithOffset {
            sequence: LogicalSequence {
                literal_length: 1,
                match_length: 4,
                distance: 1,
            },
            offset: OffsetIntent::Explicit,
        }
    }

    #[test]
    fn rejected_layout_and_zero_repeats_preserve_contents_before_any_request() {
        let source = [b'x'; 256];
        let block = small(&source);
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let (long, short) = block.layouts();
        let mut tables =
            DoubleFastTables::new_in(long, short, owner.clone(), owner.clone()).unwrap();
        tables.long.store(block.long_key(0).unwrap(), 19).unwrap();
        tables.short.store(block.short_key(0).unwrap(), 23).unwrap();
        let mut literals = Vec::new_in(owner.clone());
        literals.push(71);
        let mut sequences = Vec::new_in(owner);
        sequences.push(sentinel());
        let before = monitor.snapshot().attempts;
        let long_before = tables.long.test_positions().to_vec();
        let short_before = tables.short.test_positions().to_vec();
        let wrong = DoubleFastBlock::new(
            &source,
            0..256,
            0,
            long,
            SearchKeyLayout::for_fast(6, 4).unwrap(),
            1024,
        )
        .unwrap();
        assert_eq!(
            parse_block(wrong, &mut tables, [1, 4, 8], &mut literals, &mut sequences),
            Err(Error::InvalidParameter)
        );
        for repeats in [[0, 4, 8], [1, 0, 8], [1, 4, 0]] {
            assert_eq!(
                parse_block(block, &mut tables, repeats, &mut literals, &mut sequences),
                Err(Error::CorruptData)
            );
        }
        assert_eq!(literals.as_slice(), &[71]);
        assert_eq!(sequences.as_slice(), &[sentinel()]);
        assert_eq!(tables.long.test_positions(), long_before);
        assert_eq!(tables.short.test_positions(), short_before);
        assert_eq!(monitor.snapshot().attempts, before);
        drop(tables);
        drop(literals);
        drop(sequences);
        monitor.assert_released();
    }

    #[test]
    fn either_output_reserve_failure_preserves_logical_state_and_exact_owners() {
        for fail_literals in [true, false] {
            let source = [b'x'; 256];
            let block = small(&source);
            let (long, short) = block.layouts();
            let table_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let lm_owner = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let sm_owner = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let tm = table_owner.monitor();
            let lm = lm_owner.monitor();
            let sm = sm_owner.monitor();
            let mut tables =
                DoubleFastTables::new_in(long, short, table_owner.clone(), table_owner).unwrap();
            tables.long.store(block.long_key(0).unwrap(), 19).unwrap();
            tables.short.store(block.short_key(0).unwrap(), 23).unwrap();
            let long_before = tables.long.test_positions().to_vec();
            let short_before = tables.short.test_positions().to_vec();
            let mut literals = Vec::new_in(lm_owner);
            literals.push(71);
            let mut sequences = Vec::new_in(sm_owner);
            sequences.push(sentinel());
            assert!(if fail_literals {
                lm.fail_next()
            } else {
                sm.fail_next()
            });
            assert_eq!(
                parse_block(block, &mut tables, [1, 4, 8], &mut literals, &mut sequences),
                Err(Error::AllocationFailed)
            );
            assert_eq!(literals.as_slice(), &[71]);
            assert_eq!(sequences.as_slice(), &[sentinel()]);
            assert_eq!(tables.long.test_positions(), long_before);
            assert_eq!(tables.short.test_positions(), short_before);
            assert_eq!(lm.snapshot().attempts, 2);
            assert_eq!(sm.snapshot().attempts, if fail_literals { 1 } else { 2 });
            assert_eq!(tm.snapshot().attempts, 2);
            drop(tables);
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
    fn out_of_range_future_and_expired_candidates_are_rejected_before_reads() {
        let source = [b'x'; 2048];
        let long = SearchKeyLayout::for_double_fast_long(6).unwrap();
        let short = SearchKeyLayout::for_fast(7, 4).unwrap();
        for invalid in [0, 539, 1500, 1564, usize::MAX - 1, usize::MAX] {
            let block = DoubleFastBlock::new(&source, 1500..1564, 0, long, short, 1024).unwrap();
            let mut tables = DoubleFastTables::new_in(long, short, Global, Global).unwrap();
            tables
                .long
                .store(block.long_key(1500).unwrap(), invalid)
                .unwrap();
            tables
                .short
                .store(block.short_key(1500).unwrap(), invalid)
                .unwrap();
            // All repeats start unavailable. After rejecting the first candidate,
            // the parser may correctly discover a later self-history match.
            let mut literals = Vec::new();
            let mut sequences = Vec::new();
            let result = parse_block(
                block,
                &mut tables,
                [u32::MAX; 3],
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            let first = sequences.first().unwrap();
            // The later distance-one match may extend backward to the block
            // anchor. Both zero and one leading literal are valid here.
            assert!(first.sequence.literal_length <= 1);
            assert_eq!(first.sequence.distance, 1);
            assert_eq!(
                first.sequence.literal_length + first.sequence.match_length,
                64
            );
            assert_eq!(result.trailing_literals, 0);
            assert_eq!(result.parser_repeats[2], u32::MAX);
        }
    }
}
