//! Safe source comparison and bounded reference Greedy candidate search.
//! Search mutates only sampling tables/frontiers, never repeat or entropy state.
#![forbid(unsafe_code)]
#![cfg_attr(not(test), allow(dead_code, reason = "Greedy parsing follows"))]

use crate::greedy_block::GreedyBlock;
use crate::reference_chain::ReferenceChain;
use crate::{Error, Result};
use core::alloc::Allocator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GreedyMatch {
    pub(crate) previous: usize,
    pub(crate) length: usize,
}

/// Return a strictly better byte match, retaining earlier search-order ties.
/// Corresponds to the four-byte rejection and `ZSTD_count` in
/// `ZSTD_HcFindBestMatch`/`ZSTD_RowFindBestMatch` (facebook/zstd v1.5.7).
fn longer_match(
    block: GreedyBlock<'_>,
    current: usize,
    previous: usize,
    best: usize,
) -> Result<Option<GreedyMatch>> {
    let end = block.range().end;
    if previous >= current || current < block.range().start || best < 3 {
        return Err(Error::CorruptData);
    }
    let remaining = end.checked_sub(current).ok_or(Error::CorruptData)?;
    if best > remaining {
        return Err(Error::CorruptData);
    }
    if best == remaining {
        return Ok(None);
    }
    // best < remaining proves the four-byte rejection window is within both
    // suffixes. Checked slicing keeps that fact independent of table validity.
    let source = block.source();
    let query = source.get(current..end).ok_or(Error::CorruptData)?;
    let candidate = source.get(previous..end).ok_or(Error::CorruptData)?;
    let probe = best - 3..best + 1;
    if candidate.get(probe.clone()).ok_or(Error::CorruptData)?
        != query.get(probe).ok_or(Error::CorruptData)?
    {
        return Ok(None);
    }
    let length = query
        .iter()
        .zip(candidate)
        .take_while(|(a, b)| a == b)
        .count();
    Ok((length > best).then_some(GreedyMatch { previous, length }))
}

/// Insert pending source positions and search the unsalted cyclic chain.
/// Corresponds to `ZSTD_insertAndFindFirstIndex_internal` and
/// `ZSTD_HcFindBestMatch`, no-dictionary branch (facebook/zstd v1.5.7).
/// Invalid scalar arguments reject before writes. A later corruption error may
/// retain partial sampling writes; the owning parser must poison that context.
pub(crate) fn find_chain<H: Allocator, L: Allocator>(
    block: GreedyBlock<'_>,
    table: &mut ReferenceChain<H, L>,
    next: &mut usize,
    current: usize,
    attempts: usize,
    skipping: bool,
) -> Result<Option<GreedyMatch>> {
    if block.row_search()
        || table.key_layout() != block.key_layout()
        || !(1..=256).contains(&attempts)
        || *next > current
        || block.search_from(current, current)?.is_none()
    {
        return Err(Error::InvalidParameter);
    }
    let key = block.key(current)?;
    let floor = block.candidate_floor(current)?;
    for position in *next..current {
        table.insert(block.key(position)?, position)?;
        if skipping {
            break;
        }
    }
    *next = current;
    let mut best = None;
    let mut length = 3;
    for previous in table.candidates(key, current, floor, attempts)? {
        if let Some(found) = longer_match(block, current, previous?, length)? {
            length = found.length;
            best = Some(found);
            // Stop before following another cyclic link or reading best+1.
            if length == block.range().end - current {
                break;
            }
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greedy_key::GreedyKeyLayout;
    use crate::greedy_oracle_fixture::{OracleCase, cases};
    use alloc::{alloc::Global, vec, vec::Vec};

    fn block<'a>(case: &OracleCase<'a>) -> GreedyBlock<'a> {
        GreedyBlock::new(
            case.source,
            case.prefix..case.source.len(),
            case.prefix_floor,
            case.key_layout().unwrap(),
            1 << case.window_log,
            case.row_search,
        )
        .unwrap()
    }

    fn chain(case: &OracleCase<'_>) -> ReferenceChain {
        let mut table =
            ReferenceChain::new_in(case.key_layout().unwrap(), case.chain_log, Global, Global)
                .unwrap();
        let source = block(case);
        for &position in &case.seeds {
            let position = position as usize;
            table
                .insert(source.key(position).unwrap(), position)
                .unwrap();
        }
        table
    }

    fn raw(values: &[usize]) -> Vec<u32> {
        values
            .iter()
            .map(|&p| {
                if p == usize::MAX {
                    0
                } else {
                    u32::try_from(p).unwrap() + 2
                }
            })
            .collect()
    }

    fn final_chain(case: &OracleCase<'_>, table: &ReferenceChain, next: usize) {
        let (heads, links) = table.test_maps();
        assert_eq!(
            raw(heads),
            case.first_table.values(true).unwrap(),
            "{} heads",
            case.name
        );
        assert_eq!(
            raw(links),
            case.second_table.values(true).unwrap(),
            "{} links",
            case.name
        );
        assert_eq!(next, case.final_next, "{} frontier", case.name);
    }

    #[test]
    fn chain_no_match_schedules_match_complete_frozen_maps() {
        let mut checked = 0;
        let mut skipping_cases = 0;
        for case in cases()
            .unwrap()
            .iter()
            .filter(|c| !c.row_search && c.sequences.is_empty())
        {
            let source = block(case);
            let mut table = chain(case);
            let mut next = case.initial_next;
            let mut cursor = source.initial_cursor().unwrap();
            let mut skipping = false;
            while let Some(probe) = cursor {
                assert_eq!(
                    find_chain(
                        source,
                        &mut table,
                        &mut next,
                        probe.current(),
                        1 << case.search_log,
                        skipping
                    )
                    .unwrap(),
                    None,
                    "{}",
                    case.name
                );
                let miss = probe.miss().unwrap();
                cursor = miss.next;
                skipping = miss.skipping;
            }
            final_chain(case, &table, next);
            assert_eq!(skipping, case.final_skipping, "{}", case.name);
            skipping_cases += usize::from(skipping);
            checked += 1;
        }
        assert_eq!(checked, 22);
        assert_eq!(skipping_cases, 2);
    }

    #[test]
    fn chain_first_probe_full_matches_agree_with_frozen_candidates_and_maps() {
        let mut checked = 0;
        let mut maximum_history = false;
        for case in cases().unwrap().iter().filter(|c| !c.row_search) {
            let [sequence] = case.sequences.as_slice() else {
                continue;
            };
            if case.initial_current != case.prefix
                || case.final_next != case.initial_current
                || sequence.literal_length != 0
                || sequence.match_length as usize != case.source.len() - case.prefix
                || sequence.offset_value < 4
            {
                continue;
            }
            let mut table = chain(case);
            let mut next = case.initial_next;
            assert_eq!(
                find_chain(
                    block(case),
                    &mut table,
                    &mut next,
                    case.initial_current,
                    1 << case.search_log,
                    false
                )
                .unwrap(),
                Some(GreedyMatch {
                    previous: case.prefix - sequence.distance as usize,
                    length: sequence.match_length as usize,
                }),
                "{}",
                case.name
            );
            final_chain(case, &table, next);
            maximum_history |= sequence.distance == 2_097_152;
            checked += 1;
        }
        assert_eq!(checked, 14);
        assert!(maximum_history);
    }

    #[test]
    fn chain_rejects_invalid_probe_arguments_before_mutation() {
        let source = [0; 128];
        let key = GreedyKeyLayout::new(6, 4).unwrap();
        let valid = GreedyBlock::new(&source, 32..128, 0, key, 1024, false).unwrap();
        for (query, initial, attempts, row, other_key) in [
            (31, 0, 1, false, false),
            (121, 0, 1, false, false),
            (32, 33, 1, false, false),
            (32, 0, 0, false, false),
            (32, 0, 257, false, false),
            (32, 0, 1, true, false),
            (32, 0, 1, false, true),
            (usize::MAX, 0, 1, false, false),
        ] {
            let mut table = ReferenceChain::new_in(key, 6, Global, Global).unwrap();
            table.insert(valid.key(0).unwrap(), 0).unwrap();
            let before = (table.test_maps().0.to_vec(), table.test_maps().1.to_vec());
            let layout = if other_key {
                GreedyKeyLayout::new(7, 4).unwrap()
            } else {
                key
            };
            let block = GreedyBlock::new(&source, 32..128, 0, layout, 1024, row).unwrap();
            let mut next = initial;
            assert_eq!(
                find_chain(block, &mut table, &mut next, query, attempts, false),
                Err(Error::InvalidParameter)
            );
            assert_eq!(next, initial);
            assert_eq!(table.test_maps(), (&before.0[..], &before.1[..]));
        }
    }

    #[test]
    fn byte_comparison_keeps_newest_ties_and_never_uses_later_block_bytes() {
        let mut source = vec![b'x'; 96];
        source[..8].copy_from_slice(b"abcdefgh");
        source[16..24].copy_from_slice(b"abcdefgZ");
        source[32..40].copy_from_slice(b"abcdefgQ");
        source[48..56].copy_from_slice(b"abcdefgh");
        let key = GreedyKeyLayout::new(6, 4).unwrap();
        let block = GreedyBlock::new(&source, 48..72, 0, key, 1024, false).unwrap();
        let mut table = ReferenceChain::new_in(key, 6, Global, Global).unwrap();
        for p in [0, 16, 32] {
            table.insert(block.key(p).unwrap(), p).unwrap();
        }
        for (attempts, previous, length) in [(1, 32, 7), (2, 32, 7), (3, 0, 16)] {
            let mut next = 48;
            assert_eq!(
                find_chain(block, &mut table, &mut next, 48, attempts, false).unwrap(),
                Some(GreedyMatch { previous, length })
            );
        }
        let tail = GreedyBlock::new(&source, 48..55, 0, key, 1024, false).unwrap();
        assert_eq!(
            longer_match(tail, 48, 0, 3).unwrap(),
            Some(GreedyMatch {
                previous: 0,
                length: 7
            })
        );
        assert_eq!(longer_match(tail, 48, 0, 7), Ok(None));
        for (current, previous, best) in [
            (48, 48, 3),
            (48, usize::MAX, 3),
            (48, 0, 8),
            (47, 0, 3),
            (48, 0, 2),
        ] {
            assert_eq!(
                longer_match(tail, current, previous, best),
                Err(Error::CorruptData)
            );
        }
    }
}
