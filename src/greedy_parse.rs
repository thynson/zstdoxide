//! Indexed depth-zero Greedy parsing with separate sampling and wire state.
//! Corresponds to the no-dictionary `ZSTD_compressBlock_lazy_generic` in
//! facebook/zstd v1.5.7. No C pointer, speculative source read or cache is needed.
#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::greedy_block::GreedyBlock;
use crate::greedy_search::{GreedyMatch, find_chain, find_row, longer_match};
use crate::reference_chain::ReferenceChain;
use crate::row_table::RowTable;
use crate::sequence_context::{LogicalSequence, SequenceWithOffset};
use crate::sequence_plan::OffsetIntent;
use crate::{Error, Result};

/// Borrow exactly the selected family; no trait object, allocation or owner
/// conversion is involved. The two allocator values need not be interchangeable.
pub(crate) enum GreedySearch<'a, A: Allocator, B: Allocator> {
    Chain {
        table: &'a mut ReferenceChain<A, B>,
        attempts: usize,
    },
    Row {
        table: &'a mut RowTable<A, B>,
        salt: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GreedyProgress {
    pub(crate) next: usize,
    pub(crate) salt_entropy: u32,
    pub(crate) skipping: bool,
}

impl<A: Allocator, B: Allocator> GreedySearch<'_, A, B> {
    fn validate(&self, block: GreedyBlock<'_>, next: usize) -> Result<()> {
        let matches = match self {
            Self::Chain { table, attempts } => {
                !block.row_search()
                    && table.key_layout() == block.key_layout()
                    && (1..=256).contains(attempts)
            }
            Self::Row { table, .. } => {
                block.row_search() && table.layout().key_layout() == block.key_layout()
            }
        };
        if !matches || next > block.range().start {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    fn find(
        &mut self,
        block: GreedyBlock<'_>,
        progress: &mut GreedyProgress,
        current: usize,
    ) -> Result<Option<GreedyMatch>> {
        match self {
            Self::Chain { table, attempts } => find_chain(
                block,
                table,
                &mut progress.next,
                current,
                *attempts,
                progress.skipping,
            ),
            Self::Row { table, salt } => find_row(
                block,
                table,
                &mut progress.next,
                current,
                *salt,
                &mut progress.salt_entropy,
                progress.skipping,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GreedyParseResult {
    pub(crate) parser_repeats: [u32; 3],
    pub(crate) wire_repeats: [u32; 3],
    pub(crate) trailing_literals: usize,
}

fn repeated(
    block: GreedyBlock<'_>,
    position: usize,
    distance: usize,
) -> Result<Option<GreedyMatch>> {
    let Some(previous) = position.checked_sub(distance) else {
        return Ok(None);
    };
    if distance == 0
        || distance > block.window_size()
        || previous < block.prefix_floor()
        || block
            .range()
            .end
            .checked_sub(position)
            .is_none_or(|n| n < 4)
    {
        return Ok(None);
    }
    longer_match(block, position, previous, 3)
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
        block: GreedyBlock<'_>,
        position: usize,
        found: GreedyMatch,
        intent: OffsetIntent,
    ) -> Result<()> {
        let literal_length = position
            .checked_sub(self.anchor)
            .ok_or(Error::CorruptData)?;
        let distance = position
            .checked_sub(found.previous)
            .ok_or(Error::CorruptData)?;
        let range = block.range();
        if distance == 0
            || distance > block.window_size()
            || found.previous < block.prefix_floor()
            || found.length < 4
            || found.length > range.end.checked_sub(position).ok_or(Error::CorruptData)?
            || self.sequences.len() >= (range.end - range.start) / 4
        {
            return Err(Error::CorruptData);
        }
        let sequence = LogicalSequence {
            literal_length: u32::try_from(literal_length).map_err(|_| Error::ResourceLimit)?,
            match_length: u32::try_from(found.length).map_err(|_| Error::ResourceLimit)?,
            distance: u32::try_from(distance).map_err(|_| Error::ResourceLimit)?,
        };
        let transition = intent.resolve(sequence.literal_length, sequence.distance, self.wire)?;
        // Disjoint literal extents and at least four consumed bytes per sequence
        // fit the two reservations completed before parsing began.
        self.literals
            .extend_from_slice(&block.source()[self.anchor..position]);
        self.sequences.push(SequenceWithOffset {
            sequence,
            offset: intent,
        });
        self.anchor = position + found.length;
        self.wire = transition.next_repeat;
        Ok(())
    }
}

/// Parse one already bounded direct block, preserving caller-owned sampling.
/// Check layouts, repeats and frontier, then reserve both output owners before
/// changing logical contents or progress. A failed second reserve may increase
/// literal capacity, but leaves contents and sampling unchanged. Errors after
/// parsing starts may retain partial writes: a persistent caller must poison
/// that state. No candidate repeat or coding-entropy state is published here.
pub(crate) fn parse_block<A: Allocator, B: Allocator, L: Allocator, S: Allocator>(
    block: GreedyBlock<'_>,
    mut search: GreedySearch<'_, A, B>,
    progress: &mut GreedyProgress,
    initial_repeats: [u32; 3],
    literals: &mut Vec<u8, L>,
    sequences: &mut Vec<SequenceWithOffset, S>,
) -> Result<GreedyParseResult> {
    search.validate(block, progress.next)?;
    if initial_repeats.contains(&0) {
        return Err(Error::CorruptData);
    }
    let mut first = usize::try_from(initial_repeats[0]).map_err(|_| Error::ResourceLimit)?;
    let mut second = usize::try_from(initial_repeats[1]).map_err(|_| Error::ResourceLimit)?;
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
    progress.skipping = false;
    let mut output = ParseOutput {
        literals,
        sequences,
        anchor: range.start,
        wire: initial_repeats,
    };
    let max_repeat = block.initial_position() - block.repeat_floor();
    let saved_first = if first > max_repeat {
        core::mem::replace(&mut first, 0)
    } else {
        0
    };
    let mut saved_second = if second > max_repeat {
        core::mem::replace(&mut second, 0)
    } else {
        0
    };
    let mut cursor = block.initial_cursor()?;
    while let Some(probe) = cursor {
        let repeat_position = probe.current().checked_add(1).ok_or(Error::ResourceLimit)?;
        let (mut position, mut found, intent) =
            if let Some(found) = repeated(block, repeat_position, first)? {
                // Depth zero accepts repeat1 before querying or updating any table.
                (repeat_position, found, OffsetIntent::Repeat1)
            } else if let Some(found) = search.find(block, progress, probe.current())? {
                (probe.current(), found, OffsetIntent::Explicit)
            } else {
                let miss = probe.miss()?;
                progress.skipping = miss.skipping;
                cursor = miss.next;
                continue;
            };
        if intent == OffsetIntent::Explicit {
            // Both indices come from a verified source match. Backward extension
            // retains distance, and cannot cross the anchor or retained prefix.
            while position > output.anchor
                && found.previous > block.prefix_floor()
                && block.source()[position - 1] == block.source()[found.previous - 1]
            {
                position -= 1;
                found.previous -= 1;
                found.length += 1;
            }
            second = first;
            first = position - found.previous;
        }
        output.emit(block, position, found, intent)?;
        progress.skipping = false;
        while block.cleanup_allowed(output.anchor)? {
            let Some(found) = repeated(block, output.anchor, second)? else {
                break;
            };
            core::mem::swap(&mut first, &mut second);
            // Zero literals make offBase 1 select the second wire repeat. The
            // raw parser has already swapped its two locally available offsets.
            output.emit(block, output.anchor, found, OffsetIntent::Repeat1)?;
        }
        cursor = block.search_from(output.anchor, output.anchor)?;
    }
    if saved_first != 0 && first != 0 {
        saved_second = saved_first;
    }
    let parser_repeats = [
        u32::try_from(if first == 0 { saved_first } else { first })
            .map_err(|_| Error::ResourceLimit)?,
        u32::try_from(if second == 0 { saved_second } else { second })
            .map_err(|_| Error::ResourceLimit)?,
        initial_repeats[2],
    ];
    let trailing_literals = range.end - output.anchor;
    output
        .literals
        .extend_from_slice(&block.source()[output.anchor..range.end]);
    Ok(GreedyParseResult {
        parser_repeats,
        wire_repeats: output.wire,
        trailing_literals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greedy_key::{GreedyKeyLayout, RowLayout};
    use crate::greedy_oracle_fixture::{OracleCase, cases};
    use alloc::{alloc::Global, vec};
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn raw(values: &[usize]) -> Vec<u32> {
        values
            .iter()
            .map(|&p| {
                if p == usize::MAX {
                    0
                } else {
                    u32::try_from(p).unwrap().checked_add(2).unwrap()
                }
            })
            .collect()
    }

    fn compare(
        case: &OracleCase<'_>,
        result: GreedyParseResult,
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
            "{} literal length",
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
            &case.source[case.source.len() - case.trailing_literals..],
            "{} trailing bytes",
            case.name
        );
        assert_eq!(
            sequences.len(),
            case.sequences.len(),
            "{} sequence count",
            case.name
        );
        let mut wire = case.initial_repeats;
        let mut replay = case.source[..case.prefix].to_vec();
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
                "{} sequence {index}",
                case.name
            );
            assert_eq!(
                u32::from(seq.literal_length as u16),
                expected.raw_literal_length,
                "{} raw literal length",
                case.name
            );
            assert_eq!(
                u32::from((seq.match_length - 3) as u16),
                expected.raw_match_base,
                "{} raw match base",
                case.name
            );
            let resolved = actual
                .offset
                .resolve(seq.literal_length, seq.distance, wire)
                .unwrap();
            assert_eq!(
                resolved.offset_value, expected.offset_value,
                "{} intent {index}",
                case.name
            );
            wire = resolved.next_repeat;
            let ll = seq.literal_length as usize;
            replay.extend_from_slice(&literals[literal_cursor..literal_cursor + ll]);
            literal_cursor += ll;
            for _ in 0..seq.match_length {
                replay.push(replay[replay.len() - seq.distance as usize]);
            }
            for (kind, length) in [(1, seq.literal_length), (2, seq.match_length - 3)] {
                if length >= 65536 {
                    assert!(marker.replace((kind, index)).is_none());
                }
            }
        }
        replay.extend_from_slice(&literals[literal_cursor..]);
        assert_eq!(replay, case.source, "{} independent replay", case.name);
        assert_eq!(wire, case.wire_repeats);
        assert_eq!(
            marker.unwrap_or((0, 0)),
            (case.long_length_kind, case.long_length_position),
            "{} long marker",
            case.name
        );
    }

    #[test]
    fn every_frozen_direct_case_matches_sequences_repeats_progress_and_complete_maps() {
        let cases = cases().unwrap();
        assert_eq!(cases.len(), 173);
        for case in cases {
            let mut padded = case.source.to_vec();
            padded.extend_from_slice(&[0xa7; 32]);
            for source in [case.source, padded.as_slice()] {
                let block = GreedyBlock::new(
                    source,
                    case.prefix..case.source.len(),
                    case.prefix_floor,
                    case.key_layout().unwrap(),
                    1 << case.window_log,
                    case.row_search,
                )
                .unwrap();
                let mut progress = GreedyProgress {
                    next: case.initial_next,
                    salt_entropy: case.initial_entropy,
                    skipping: true,
                };
                let mut literals = Vec::new();
                let mut sequences = Vec::new();
                let (result, first, second) = if case.row_search {
                    let layout = case.row_layout().unwrap();
                    let mut table = RowTable::new_in(layout, Global, Global).unwrap();
                    for &p in &case.seeds {
                        let p = p as usize;
                        table
                            .insert(
                                layout
                                    .key(block.key(p).unwrap(), case.initial_salt)
                                    .unwrap(),
                                p,
                            )
                            .unwrap();
                    }
                    let result = parse_block(
                        block,
                        GreedySearch::Row {
                            table: &mut table,
                            salt: case.initial_salt,
                        },
                        &mut progress,
                        case.initial_repeats,
                        &mut literals,
                        &mut sequences,
                    )
                    .unwrap_or_else(|e| panic!("{}: {e:?}", case.name));
                    let (positions, tags) = table.test_maps();
                    (
                        result,
                        raw(positions),
                        tags.iter().map(|&t| u32::from(t)).collect::<Vec<_>>(),
                    )
                } else {
                    let mut table = ReferenceChain::new_in(
                        case.key_layout().unwrap(),
                        case.chain_log,
                        Global,
                        Global,
                    )
                    .unwrap();
                    for &p in &case.seeds {
                        let p = p as usize;
                        table.insert(block.key(p).unwrap(), p).unwrap();
                    }
                    let result = parse_block(
                        block,
                        GreedySearch::Chain {
                            table: &mut table,
                            attempts: 1 << case.search_log,
                        },
                        &mut progress,
                        case.initial_repeats,
                        &mut literals,
                        &mut sequences,
                    )
                    .unwrap_or_else(|e| panic!("{}: {e:?}", case.name));
                    let (heads, links) = table.test_maps();
                    (result, raw(heads), raw(links))
                };
                compare(&case, result, &literals, &sequences);
                assert_eq!(
                    progress,
                    GreedyProgress {
                        next: case.final_next,
                        salt_entropy: case.final_entropy,
                        skipping: case.final_skipping
                    },
                    "{} progress",
                    case.name
                );
                assert_eq!(
                    first,
                    case.first_table.values(true).unwrap(),
                    "{} first owner",
                    case.name
                );
                assert_eq!(
                    second,
                    case.second_table.values(true).unwrap(),
                    "{} second owner",
                    case.name
                );
            }
        }
    }

    fn small(source: &[u8], row: bool) -> GreedyBlock<'_> {
        GreedyBlock::new(
            source,
            0..source.len(),
            0,
            GreedyKeyLayout::new(6, 4).unwrap(),
            1024,
            row,
        )
        .unwrap()
    }

    #[test]
    fn reused_outputs_and_borrowed_nonclone_owners_make_no_new_requests() {
        let all = cases().unwrap();
        for name in ["chain-long-literals-65536", "row-long-literals-65536"] {
            let case = all.iter().find(|c| c.name == name).unwrap();
            assert!(case.seeds.is_empty());
            let owner = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let monitor = owner.monitor();
            let block = GreedyBlock::new(
                case.source,
                case.prefix..case.source.len(),
                case.prefix_floor,
                case.key_layout().unwrap(),
                1 << case.window_log,
                case.row_search,
            )
            .unwrap();
            let mut literals = Vec::new_in(&owner);
            let mut sequences = Vec::new_in(&owner);
            if case.row_search {
                let mut table =
                    RowTable::new_in(case.row_layout().unwrap(), &owner, &owner).unwrap();
                for pass in 0..2 {
                    table.reset();
                    let mut progress = GreedyProgress {
                        next: case.initial_next,
                        salt_entropy: case.initial_entropy,
                        skipping: false,
                    };
                    let before = monitor.snapshot().attempts;
                    let result = parse_block(
                        block,
                        GreedySearch::Row {
                            table: &mut table,
                            salt: case.initial_salt,
                        },
                        &mut progress,
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
            } else {
                let mut table = ReferenceChain::new_in(
                    case.key_layout().unwrap(),
                    case.chain_log,
                    &owner,
                    &owner,
                )
                .unwrap();
                for pass in 0..2 {
                    table.reset();
                    let mut progress = GreedyProgress {
                        next: case.initial_next,
                        salt_entropy: case.initial_entropy,
                        skipping: false,
                    };
                    let before = monitor.snapshot().attempts;
                    let result = parse_block(
                        block,
                        GreedySearch::Chain {
                            table: &mut table,
                            attempts: 1 << case.search_log,
                        },
                        &mut progress,
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
            }
            assert_eq!(monitor.snapshot().allocate_calls, 4);
            assert_eq!(monitor.snapshot().grow_calls, 0);
            drop(literals);
            drop(sequences);
            monitor.assert_released();
            assert_eq!(monitor.snapshot().owners_dropped, 0);
            drop(owner);
            assert_eq!(monitor.snapshot().owners_dropped, 1);
        }
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
    fn either_output_reserve_failure_preserves_progress_contents_and_exact_owners() {
        for row in [false, true] {
            for fail_literals in [true, false] {
                let source = [b'x'; 256];
                let block = small(&source, row);
                let first_owner = TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone();
                let second_owner = TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone();
                let literal_owner = TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone();
                let sequence_owner = TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone();
                let fm = first_owner.monitor();
                let tm = second_owner.monitor();
                let lm = literal_owner.monitor();
                let sm = sequence_owner.monitor();
                let mut literals = Vec::new_in(literal_owner);
                literals.push(71);
                let mut sequences = Vec::new_in(sequence_owner);
                sequences.push(sentinel());
                assert!(if fail_literals {
                    lm.fail_next()
                } else {
                    sm.fail_next()
                });
                let original = GreedyProgress {
                    next: 0,
                    salt_entropy: 99,
                    skipping: true,
                };
                let mut progress = original;
                let result = if row {
                    let layout = RowLayout::new(block.key_layout(), 4).unwrap();
                    let mut table = RowTable::new_in(layout, first_owner, second_owner).unwrap();
                    let before = (table.test_maps().0.to_vec(), table.test_maps().1.to_vec());
                    let result = parse_block(
                        block,
                        GreedySearch::Row {
                            table: &mut table,
                            salt: 7,
                        },
                        &mut progress,
                        [1, 4, 8],
                        &mut literals,
                        &mut sequences,
                    );
                    assert_eq!(table.test_maps(), (&before.0[..], &before.1[..]));
                    result
                } else {
                    let mut table =
                        ReferenceChain::new_in(block.key_layout(), 6, first_owner, second_owner)
                            .unwrap();
                    let before = (table.test_maps().0.to_vec(), table.test_maps().1.to_vec());
                    let result = parse_block(
                        block,
                        GreedySearch::Chain {
                            table: &mut table,
                            attempts: 4,
                        },
                        &mut progress,
                        [1, 4, 8],
                        &mut literals,
                        &mut sequences,
                    );
                    assert_eq!(table.test_maps(), (&before.0[..], &before.1[..]));
                    result
                };
                assert_eq!(result, Err(Error::AllocationFailed));
                assert_eq!(progress, original);
                assert_eq!(literals.as_slice(), &[71]);
                assert_eq!(sequences.as_slice(), &[sentinel()]);
                assert_eq!(lm.snapshot().attempts, 2);
                assert_eq!(sm.snapshot().attempts, if fail_literals { 1 } else { 2 });
                assert_eq!(fm.snapshot().attempts, 1);
                assert_eq!(tm.snapshot().attempts, 1);
                drop(literals);
                drop(sequences);
                for monitor in [fm, tm, lm, sm] {
                    monitor.assert_released();
                    assert_eq!(
                        monitor.snapshot().owners_dropped,
                        monitor.snapshot().identities_created
                    );
                }
            }
        }
    }

    #[test]
    fn invalid_entry_arguments_preserve_scratch_progress_and_sampling() {
        let source = [b'x'; 64];
        for (row, key_mismatch, attempts, next, repeats, error) in [
            (true, false, 4, 0, [1, 4, 8], Error::InvalidParameter),
            (false, true, 4, 0, [1, 4, 8], Error::InvalidParameter),
            (false, false, 0, 0, [1, 4, 8], Error::InvalidParameter),
            (false, false, 257, 0, [1, 4, 8], Error::InvalidParameter),
            (false, false, 4, 1, [1, 4, 8], Error::InvalidParameter),
            (false, false, 4, 0, [1, 0, 8], Error::CorruptData),
        ] {
            let block = small(&source, row);
            let key = if key_mismatch {
                GreedyKeyLayout::new(7, 4).unwrap()
            } else {
                block.key_layout()
            };
            let mut table = ReferenceChain::new_in(key, 6, Global, Global).unwrap();
            let before = (table.test_maps().0.to_vec(), table.test_maps().1.to_vec());
            let original = GreedyProgress {
                next,
                salt_entropy: 99,
                skipping: true,
            };
            let mut progress = original;
            let mut literals = vec![71];
            let mut sequences = vec![sentinel()];
            assert_eq!(
                parse_block(
                    block,
                    GreedySearch::Chain {
                        table: &mut table,
                        attempts
                    },
                    &mut progress,
                    repeats,
                    &mut literals,
                    &mut sequences
                ),
                Err(error)
            );
            assert_eq!(table.test_maps(), (&before.0[..], &before.1[..]));
            assert_eq!(progress, original);
            assert_eq!(literals, [71]);
            assert_eq!(sequences, [sentinel()]);
        }
    }
}
