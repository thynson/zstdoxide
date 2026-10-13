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
use crate::reference_tree::ReferenceTree;
use crate::tree_layout::{TreeEdge, TreeLink, TreeSide};
use crate::tree_sort::{common_length, prepare, probe};
use crate::tree_source::TreeSource;
use crate::{Error, Result};
use core::alloc::Allocator;

/// Inclusive eight-byte query, corresponding to ZSTD_BtFindBestMatch and
/// ZSTD_DUBT_findBestMatch in facebook/zstd v1.5.7. A malformed retained table
/// may leave partial sampling writes; its caller must poison that state.
pub(crate) fn find<H: Allocator, N: Allocator>(
    block: TreeSource<'_>,
    table: &mut ReferenceTree<H, N>,
    next: &mut usize,
    current: usize,
    attempts: usize,
) -> Result<Option<GreedyMatch>> {
    let bounds = probe(block, table, current, attempts)?;
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
    for _ in 0..attempts {
        let Some(previous) = bounds.candidate(link)? else {
            break;
        };
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
                let found = find(block, &mut table, &mut next, op.query, 1 << case.search)
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
                find(block, &mut table, &mut next, 20, 8),
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
        let found = find(block, &mut table, &mut next, 8, 8).unwrap().unwrap();
        assert_eq!(found.length, 56);
        assert_eq!(next, 55);
        let maps = (table.test_maps().0.to_vec(), table.test_maps().1.to_vec());
        assert_eq!(find(block, &mut table, &mut next, 9, 8), Ok(None));
        assert_eq!(next, 55);
        assert_eq!(table.test_maps(), (&maps.0[..], &maps.1[..]));
        assert_eq!(
            find(block, &mut table, &mut next, 56, 8)
                .unwrap()
                .unwrap()
                .length,
            8
        );
        assert_eq!(
            find(block, &mut table, &mut next, 57, 8),
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
}
