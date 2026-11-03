//! No-dictionary reference binary-tree query with offset-cost selection.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Private tree parser integration follows query conformance"
    )
)]
use crate::greedy_search::GreedyMatch;
use crate::reference_tree::{ReferenceTree, ReferenceTreeView};
use crate::tree_layout::{TreeEdge, TreeLink, TreeSide};
#[cfg(test)]
use crate::tree_sort::load_sorted_prefix;
use crate::tree_sort::{common_length, prepare, probe};
use crate::tree_source::TreeSource;
use crate::{Error, Result};
use core::alloc::Allocator;

/// Inclusive eight-byte query, corresponding to ZSTD_BtFindBestMatch and
/// ZSTD_DUBT_findBestMatch in facebook/zstd v1.5.7. A malformed retained table
/// may leave partial sampling writes; its caller must poison that state.
///
/// The optional compiled view is consulted only after the per-call tree and
/// only with the remaining attempt budget, mirroring the pinned
/// `ZSTD_DUBT_findBetterDictMatch` branch: the compiled tree is fully sorted
/// and strictly read-only, ties keep the context match, and an improving
/// full-block context match suppresses the compiled traversal entirely.
pub(crate) fn find<H: Allocator, N: Allocator>(
    block: TreeSource<'_>,
    table: &mut ReferenceTree<H, N>,
    dictionary: Option<ReferenceTreeView<'_>>,
    next: &mut usize,
    current: usize,
    attempts: usize,
) -> Result<Option<GreedyMatch>> {
    let bounds = probe(block, table, current, attempts)?;
    // Reject a foreign compiled layout before any sampling write, exactly like
    // the per-call table layout probe above.
    if dictionary.is_some_and(|view| view.layout().key() != block.key_layout()) {
        return Err(Error::InvalidParameter);
    }
    if !prepare(block, table, next, current, attempts)? {
        return Ok(None);
    }
    let layout = table.layout();
    let mut link = table.replace_head(block.key(current)?, current)?;
    let mut smaller = layout.edge(current, TreeSide::Smaller)?;
    let mut larger = layout.edge(current, TreeSide::Larger)?;
    let (mut cs, mut cl) = (0, 0);
    let mut best = None;
    let mut best_length = 0;
    let mut offset = 999_999_999usize;
    // The inclusive eight-byte footprint permits a scalar one-past-source
    // endpoint here. It is never dereferenced; end+2 was checked by TreeSource.
    let mut match_end = current + 9;
    let mut ceiling = current;
    let mut used = 0;
    let mut full_block = false;
    for _ in 0..attempts {
        let Some(previous) = bounds.candidate(link)? else {
            break;
        };
        used += 1;
        if previous >= ceiling {
            return Err(Error::CorruptData);
        }
        ceiling = previous;
        let length = common_length(block, current, previous, cs.min(cl))?;
        if length > best_length {
            match_end = match_end.max(previous + length);
            let gain = i64::try_from(length - best_length)
                .map_err(|_| Error::ResourceLimit)?
                .checked_mul(4)
                .ok_or(Error::ResourceLimit)?;
            let distance = current - previous;
            let cost = i64::from((distance + 1).ilog2()) - i64::from(offset.ilog2());
            if gain > cost {
                best_length = length;
                offset = distance + 3;
                best = Some(GreedyMatch { previous, length });
            }
            if length == block.range().end - current {
                full_block = true;
                break;
            }
        }
        if length == block.range().end - current {
            return Err(Error::CorruptData);
        }
        if block.source()[previous + length] < block.source()[current + length] {
            table.set_edge(smaller, link)?;
            cs = length;
            if !bounds.can_follow(previous)? {
                smaller = TreeEdge::Discard;
                break;
            }
            smaller = layout.edge(previous, TreeSide::Larger)?;
            link = table.edge(smaller)?;
        } else {
            table.set_edge(larger, link)?;
            cl = length;
            if !bounds.can_follow(previous)? {
                larger = TreeEdge::Discard;
                break;
            }
            larger = layout.edge(previous, TreeSide::Smaller)?;
            link = table.edge(larger)?;
        }
    }
    table.set_edge(smaller, TreeLink::Empty)?;
    table.set_edge(larger, TreeLink::Empty)?;
    // The pinned dictionary branch runs only while the shared budget remains
    // and never reads past this query: it starts at the compiled head and
    // descends read-only with the same floor and ordering checks.
    if let Some(dictionary) = dictionary.filter(|_| !full_block && used < attempts) {
        let mut link = dictionary.head(block.key(current)?)?;
        let (mut cs, mut cl) = (0, 0);
        let mut ceiling = current;
        for _ in 0..(attempts - used) {
            let Some(previous) = bounds.candidate(link)? else {
                break;
            };
            if previous >= ceiling {
                return Err(Error::CorruptData);
            }
            ceiling = previous;
            let length = common_length(block, current, previous, cs.min(cl))?;
            if length > best_length {
                let gain = i64::try_from(length - best_length)
                    .map_err(|_| Error::ResourceLimit)?
                    .checked_mul(4)
                    .ok_or(Error::ResourceLimit)?;
                let distance = current - previous;
                let cost = i64::from((distance + 1).ilog2()) - i64::from(offset.ilog2());
                if gain > cost {
                    best_length = length;
                    offset = distance + 3;
                    best = Some(GreedyMatch { previous, length });
                }
                if length == block.range().end - current {
                    break;
                }
            } else if length == block.range().end - current {
                // The context best already fills the block; no remaining
                // compiled candidate can strictly improve it, so the walk
                // ends instead of comparing a boundary byte.
                break;
            }
            if block.source()[previous + length] < block.source()[current + length] {
                cs = length;
                if !bounds.can_follow(previous)? {
                    break;
                }
                link = dictionary.edge(layout.edge(previous, TreeSide::Larger)?)?;
            } else {
                cl = length;
                if !bounds.can_follow(previous)? {
                    break;
                }
                link = dictionary.edge(layout.edge(previous, TreeSide::Smaller)?)?;
            }
        }
    }
    *next = match_end - 8;
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greedy_key::GreedyKeyLayout;
    use crate::tree_layout::TreeLayout;
    use crate::tree_oracle_fixture::{self as oracle, cells};
    use alloc::alloc::Global;
    fn raw(v: usize) -> usize {
        match TreeLink::from_stored(v) {
            TreeLink::Empty => 0,
            TreeLink::Unsorted => 1,
            TreeLink::Position(p) => p + 2,
        }
    }
    #[test]
    fn every_frozen_direct_operation_matches_complete_c_state() {
        let cases = oracle::read(oracle::PACK).unwrap();
        assert_eq!(cases.len(), 103);
        let mut count = 0;
        for case in cases {
            let key = GreedyKeyLayout::new(case.hash, case.minimum.clamp(4, 6)).unwrap();
            let layout = TreeLayout::new(key, case.chain).unwrap();
            let block =
                TreeSource::new(case.source, 0..case.source.len(), 0, key, 1 << case.window)
                    .unwrap();
            let mut table = ReferenceTree::new_in(layout, Global, Global).unwrap();
            let mut next = 0;
            for (i, op) in case.operations.into_iter().enumerate() {
                assert_eq!(next, op.before, "{} operation {i} before", case.name);
                let bounds = probe(block, &table, op.query, 1 << case.search).unwrap();
                assert_eq!(bounds.window_floor_raw(), op.window_floor);
                assert_eq!(bounds.node_floor_raw(), op.node_floor);
                let found = find(
                    block,
                    &mut table,
                    None,
                    &mut next,
                    op.query,
                    1 << case.search,
                )
                .unwrap_or_else(|e| panic!("{} operation {i}: {e:?}", case.name));
                let (length, offset) =
                    found.map_or((0, 999999999), |m| (m.length, op.query - m.previous + 3));
                assert_eq!(
                    (next, length, offset),
                    (op.after, op.length, op.offset),
                    "{} operation {i} selection",
                    case.name
                );
                for (which, actual, expected) in [
                    ("head", table.test_maps().0, op.heads),
                    ("edge", table.test_maps().1, op.edges),
                ] {
                    assert_eq!(actual.len() * 4, expected.len());
                    for (slot, (&a, b)) in actual.iter().zip(cells(expected)).enumerate() {
                        assert_eq!(raw(a), b, "{} operation {i} {which} {slot}", case.name);
                    }
                }
                count += 1;
            }
        }
        assert_eq!(count, 2334);
    }
    #[test]
    fn malformed_sorted_cycles_and_future_heads_reject() {
        let mut source = [b'z'; 128];
        source[10] = b'a';
        let key = GreedyKeyLayout::new(6, 4).unwrap();
        let layout = TreeLayout::new(key, 6).unwrap();
        let block = TreeSource::new(&source, 0..128, 0, key, 1024).unwrap();
        for future in [false, true] {
            let mut table = ReferenceTree::new_in(layout, Global, Global).unwrap();
            let mut next = 20;
            table
                .replace_head(block.key(20).unwrap(), if future { 21 } else { 10 })
                .unwrap();
            if !future {
                table
                    .set_edge(
                        layout.edge(10, TreeSide::Larger).unwrap(),
                        TreeLink::Position(10),
                    )
                    .unwrap();
            }
            assert_eq!(
                find(block, &mut table, None, &mut next, 20, 8),
                Err(Error::CorruptData)
            );
        }
    }
    #[test]
    fn inclusive_tail_and_skipped_queries_preserve_frontier_and_maps() {
        let source = [b'a'; 64];
        let key = GreedyKeyLayout::new(6, 4).unwrap();
        let layout = TreeLayout::new(key, 6).unwrap();
        let block = TreeSource::new(&source, 0..64, 0, key, 1024).unwrap();
        let mut table = ReferenceTree::new_in(layout, Global, Global).unwrap();
        let mut next = 0;
        let found = find(block, &mut table, None, &mut next, 8, 8)
            .unwrap()
            .unwrap();
        assert_eq!(found.length, 56);
        assert_eq!(next, 55);
        let maps = (table.test_maps().0.to_vec(), table.test_maps().1.to_vec());
        assert_eq!(find(block, &mut table, None, &mut next, 9, 8), Ok(None));
        assert_eq!(next, 55);
        assert_eq!(table.test_maps(), (&maps.0[..], &maps.1[..]));
        assert_eq!(
            find(block, &mut table, None, &mut next, 56, 8)
                .unwrap()
                .unwrap()
                .length,
            8
        );
        assert_eq!(
            find(block, &mut table, None, &mut next, 57, 8),
            Err(Error::InvalidParameter)
        );
    }
    #[test]
    fn fixture_access_rejects_truncation_metadata_and_trailing_bytes() {
        for n in [0, 7, 11, 12, 51, 100, oracle::PACK.len() - 1] {
            assert!(oracle::read(&oracle::PACK[..n]).is_err());
        }
        let mut b = oracle::PACK.to_vec();
        b.push(0);
        assert!(oracle::read(&b).is_err());
        for (offset, value) in [
            (8, 104_u32),
            (24, 9),
            (28, 5),
            (32, 13),
            (36, 0),
            (40, 2),
            (44, 1),
            (48, 5),
        ] {
            let mut b = oracle::PACK.to_vec();
            b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(oracle::read(&b).is_err(), "offset {offset}");
        }
    }

    /// Combined staging geometry for the consultation cases: a 48-byte
    /// dictionary region ("abcdefgh" three times, then "abcdZZZZ", then
    /// "abcdefgh" twice) followed by input ("abcdefgh" then "Z" padding).
    /// Positions below 48 are compiled-tree candidates; the query block starts
    /// at 48. Under hash log 8 every distinct four-byte key owns its bucket:
    /// the query key "abcd" heads at compiled position 32, and the "Z" keys
    /// share the bucket headed at compiled position 28.
    fn staged() -> alloc::vec::Vec<u8> {
        let mut combined = alloc::vec::Vec::new();
        combined.extend_from_slice(b"abcdefghabcdefghabcdefgh");
        combined.extend_from_slice(b"abcdZZZZ");
        combined.extend_from_slice(b"abcdefghabcdefgh");
        combined.extend_from_slice(b"abcdefgh");
        combined.extend_from_slice(&[b'Z'; 104]);
        combined
    }

    fn staged_layout(key: GreedyKeyLayout) -> TreeLayout {
        TreeLayout::new(key, 6).unwrap()
    }

    fn staged_block<'a>(combined: &'a [u8]) -> TreeSource<'a> {
        let key = GreedyKeyLayout::new(8, 4).unwrap();
        TreeSource::new(combined, 48..combined.len(), 0, key, 1024).unwrap()
    }

    fn compiled_tree(combined: &[u8]) -> ReferenceTree {
        let key = GreedyKeyLayout::new(8, 4).unwrap();
        let mut tree = ReferenceTree::new_in(staged_layout(key), Global, Global).unwrap();
        load_sorted_prefix(&mut tree, combined, 48, 1024, 8).unwrap();
        tree
    }

    fn maps(tree: &ReferenceTree) -> (alloc::vec::Vec<usize>, alloc::vec::Vec<usize>) {
        let (heads, edges) = tree.test_maps();
        (heads.to_vec(), edges.to_vec())
    }

    #[test]
    fn compiled_candidates_are_consulted_after_an_empty_context_tree() {
        let combined = staged();
        let dictionary = compiled_tree(&combined);
        let view = dictionary.view();
        let block = staged_block(&combined);
        // A fresh per-call tree with the frontier at the query consults only
        // the compiled tree. The compiled head for the query key holds an
        // eight-byte match from the dictionary region.
        let key = GreedyKeyLayout::new(8, 4).unwrap();
        let mut table = ReferenceTree::new_in(staged_layout(key), Global, Global).unwrap();
        let mut next = 48;
        let found = find(block, &mut table, Some(view), &mut next, 48, 8).unwrap();
        assert_eq!(
            found,
            Some(GreedyMatch {
                previous: 32,
                length: 8
            }),
            "the compiled candidate from the dictionary region must win"
        );
        // The context tree only received the query position; the pinned
        // frontier advance skips the eight-byte query footprint.
        let (heads, _) = table.test_maps();
        assert!(heads.contains(&48));
        assert_eq!(next, 49);
        // The compiled cells stay untouched.
        let (heads, edges) = dictionary.test_maps();
        let fresh = compiled_tree(&combined);
        assert_eq!((heads, edges), fresh.test_maps());
    }

    #[test]
    fn compiled_consultation_shares_the_remaining_attempt_budget() {
        let combined = staged();
        let dictionary = compiled_tree(&combined);
        let view = dictionary.view();
        let block = staged_block(&combined);
        let key = GreedyKeyLayout::new(8, 4).unwrap();
        // The context candidate at 24 matches four bytes and consumes one
        // attempt. The compiled head (position 34, a hash collision matching
        // zero bytes) and the next node (33) each consume one more, so only
        // with four total attempts does the walk reach position 32 and win by
        // the gain rule.
        for (attempts, expected) in [
            (
                1,
                Some(GreedyMatch {
                    previous: 24,
                    length: 4,
                }),
            ),
            (
                4,
                Some(GreedyMatch {
                    previous: 32,
                    length: 8,
                }),
            ),
        ] {
            let mut table = ReferenceTree::new_in(staged_layout(key), Global, Global).unwrap();
            table.insert_unsorted(block.key(24).unwrap(), 24).unwrap();
            let mut next = 48;
            let found = find(block, &mut table, Some(view), &mut next, 48, attempts).unwrap();
            assert_eq!(found, expected, "attempts {attempts}");
        }
    }

    #[test]
    fn strictly_longer_context_matches_survive_compiled_ties() {
        let combined = staged();
        let dictionary = compiled_tree(&combined);
        let view = dictionary.view();
        let block = staged_block(&combined);
        let key = GreedyKeyLayout::new(8, 4).unwrap();
        // The context candidate at 32 ties the compiled head exactly; the
        // pinned strictly-longer gate keeps the context result.
        let mut table = ReferenceTree::new_in(staged_layout(key), Global, Global).unwrap();
        table.insert_unsorted(block.key(32).unwrap(), 32).unwrap();
        let mut next = 48;
        let found = find(block, &mut table, Some(view), &mut next, 48, 8).unwrap();
        assert_eq!(
            found,
            Some(GreedyMatch {
                previous: 32,
                length: 8
            })
        );
    }

    #[test]
    fn an_improving_full_block_match_suppresses_the_compiled_traversal() {
        let combined = staged();
        let mut dictionary = compiled_tree(&combined);
        // A future-position link in the compiled tree would corrupt any
        // consultation that reaches it, making suppression observable.
        dictionary
            .set_edge(
                dictionary.layout().edge(28, TreeSide::Smaller).unwrap(),
                TreeLink::Position(170),
            )
            .unwrap();
        let view = dictionary.view();
        let block = staged_block(&combined);
        let key = GreedyKeyLayout::new(8, 4).unwrap();
        // Without the context match the walk reaches the corrupted link.
        let mut empty = ReferenceTree::new_in(staged_layout(key), Global, Global).unwrap();
        let mut next = 58;
        assert_eq!(
            find(block, &mut empty, Some(view), &mut next, 58, 8),
            Err(Error::CorruptData)
        );
        // A context candidate that fills the block suppresses the walk.
        let mut table = ReferenceTree::new_in(staged_layout(key), Global, Global).unwrap();
        table.insert_unsorted(block.key(56).unwrap(), 56).unwrap();
        let mut next = 58;
        let found = find(block, &mut table, Some(view), &mut next, 58, 8).unwrap();
        assert_eq!(
            found,
            Some(GreedyMatch {
                previous: 56,
                length: 102
            }),
            "the context full-block match wins and the compiled tree is never read"
        );
    }

    #[test]
    fn foreign_compiled_layouts_reject_before_any_write() {
        let combined = staged();
        let other = GreedyKeyLayout::new(6, 4).unwrap();
        let mut foreign = ReferenceTree::new_in(staged_layout(other), Global, Global).unwrap();
        load_sorted_prefix(&mut foreign, &combined, 48, 1024, 8).unwrap();
        let view = foreign.view();
        let block = staged_block(&combined);
        let key = GreedyKeyLayout::new(8, 4).unwrap();
        let mut table = ReferenceTree::new_in(staged_layout(key), Global, Global).unwrap();
        table.insert_unsorted(block.key(24).unwrap(), 24).unwrap();
        let before = maps(&table);
        let mut next = 48;
        assert_eq!(
            find(block, &mut table, Some(view), &mut next, 48, 8),
            Err(Error::InvalidParameter)
        );
        assert_eq!(maps(&table), before);
        assert_eq!(next, 48);
    }
}
