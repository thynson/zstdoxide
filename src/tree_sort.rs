//! Checked deferred sorting for no-dictionary binary-tree search.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "Tree query integration follows deferred sorting")
)]
use crate::reference_tree::ReferenceTree;
use crate::tree_layout::{TreeEdge, TreeLink, TreeProbe, TreeSide};
use crate::tree_source::TreeSource;
use crate::{Error, Result};
use core::alloc::Allocator;

pub(crate) fn probe<H: Allocator, N: Allocator>(
    block: TreeSource<'_>,
    table: &ReferenceTree<H, N>,
    current: usize,
    attempts: usize,
) -> Result<TreeProbe> {
    if block.key_layout() != table.layout().key()
        || !(1..=256).contains(&attempts)
        || current < block.range().start
    {
        return Err(Error::InvalidParameter);
    }
    TreeProbe::new(
        current,
        block.range().end,
        block.prefix_floor(),
        block.window_size(),
        table.layout(),
    )
}

/// Compare only initialized logical source bytes. Recheck the common prefix
/// inherited from the two branch bounds before relying on it.
pub(crate) fn common_length(
    block: TreeSource<'_>,
    current: usize,
    previous: usize,
    common: usize,
) -> Result<usize> {
    if previous >= current || previous < block.prefix_floor() {
        return Err(Error::CorruptData);
    }
    let q = block
        .source()
        .get(current..block.range().end)
        .ok_or(Error::CorruptData)?;
    let p = block
        .source()
        .get(previous..block.range().end)
        .ok_or(Error::CorruptData)?;
    if q.get(..common).ok_or(Error::CorruptData)? != p.get(..common).ok_or(Error::CorruptData)? {
        return Err(Error::CorruptData);
    }
    Ok(common
        + q[common..]
            .iter()
            .zip(&p[common..])
            .take_while(|(a, b)| a == b)
            .count())
}

/// Sort one pending node. node_floor is the main query's unsort limit, not
/// this node's cyclic floor. Corresponds to ZSTD_insertDUBT1 (v1.5.7).
fn sort_node<H: Allocator, N: Allocator>(
    block: TreeSource<'_>,
    table: &mut ReferenceTree<H, N>,
    current: usize,
    attempts: usize,
    node_floor: usize,
) -> Result<()> {
    let layout = table.layout();
    let bounds = TreeProbe::new(
        current,
        block.range().end,
        block.prefix_floor(),
        block.window_size(),
        layout,
    )?;
    if current + 2 < node_floor {
        return Err(Error::CorruptData);
    }
    let mut smaller = layout.edge(current, TreeSide::Smaller)?;
    let mut larger = layout.edge(current, TreeSide::Larger)?;
    let mut link = table.edge(smaller)?;
    let (mut cs, mut cl) = (0, 0);
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
        if length == block.range().end - current {
            break;
        }
        // common_length proves both discriminating bytes exist.
        if block.source()[previous + length] < block.source()[current + length] {
            table.set_edge(smaller, link)?;
            cs = length;
            if previous + 2 <= node_floor {
                smaller = TreeEdge::Discard;
                break;
            }
            smaller = layout.edge(previous, TreeSide::Larger)?;
            link = table.edge(smaller)?;
        } else {
            table.set_edge(larger, link)?;
            cl = length;
            if previous + 2 <= node_floor {
                larger = TreeEdge::Discard;
                break;
            }
            larger = layout.edge(previous, TreeSide::Smaller)?;
            link = table.edge(larger)?;
        }
    }
    table.set_edge(smaller, TreeLink::Empty)?;
    table.set_edge(larger, TreeLink::Empty)
}

/// Insert pending positions and sort only the current hash's bounded pending
/// chain. Scalar errors reject before writes; later malformed-state errors may
/// leave partial sampling writes, requiring the caller to poison retained state.
pub(crate) fn prepare<H: Allocator, N: Allocator>(
    block: TreeSource<'_>,
    table: &mut ReferenceTree<H, N>,
    next: &mut usize,
    current: usize,
    attempts: usize,
) -> Result<bool> {
    let bounds = probe(block, table, current, attempts)?;
    if *next < block.prefix_floor() || *next > block.range().end {
        return Err(Error::InvalidParameter);
    }
    if current < *next {
        return Ok(false);
    }
    for position in *next..current {
        table.insert_unsorted(block.key(position)?, position)?;
    }
    *next = current;
    let layout = table.layout();
    let limit = bounds.window_floor_raw().max(bounds.node_floor_raw());
    let mut link = table.head(block.key(current)?)?;
    let mut stacked = TreeLink::Empty;
    let mut remaining = attempts;
    let mut ceiling = current;
    while let Some(position) = bounds.candidate(link)? {
        if position >= ceiling {
            return Err(Error::CorruptData);
        }
        if position + 2 <= limit
            || table.edge(layout.edge(position, TreeSide::Larger)?)? != TreeLink::Unsorted
        {
            break;
        }
        if remaining == 1 {
            table.set_edge(layout.edge(position, TreeSide::Smaller)?, TreeLink::Empty)?;
            table.set_edge(layout.edge(position, TreeSide::Larger)?, TreeLink::Empty)?;
            break;
        }
        table.set_edge(layout.edge(position, TreeSide::Larger)?, stacked)?;
        stacked = link;
        link = table.edge(layout.edge(position, TreeSide::Smaller)?)?;
        ceiling = position;
        remaining -= 1;
    }
    let mut last = None;
    while let Some(position) = bounds.candidate(stacked)? {
        if remaining >= attempts || last.is_some_and(|p| position <= p) || position + 2 <= limit {
            return Err(Error::CorruptData);
        }
        let successor = table.edge(layout.edge(position, TreeSide::Larger)?)?;
        sort_node(block, table, position, remaining, limit)?;
        stacked = successor;
        last = Some(position);
        remaining += 1;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greedy_key::GreedyKeyLayout;
    use crate::tree_layout::TreeLayout;
    use alloc::alloc::Global;
    fn setup(source: &[u8]) -> (TreeSource<'_>, ReferenceTree) {
        let key = GreedyKeyLayout::new(6, 4).unwrap();
        (
            TreeSource::new(source, 0..source.len(), 0, key, 1024).unwrap(),
            ReferenceTree::new_in(TreeLayout::new(key, 6).unwrap(), Global, Global).unwrap(),
        )
    }
    #[test]
    fn pending_chain_reverses_truncates_and_sorts_equal_tails() {
        for attempts in [1, 2, 4, 16, 256] {
            let source = [b'a'; 128];
            let (block, mut table) = setup(&source);
            let mut next = 0;
            assert!(prepare(block, &mut table, &mut next, 12, attempts).unwrap());
            assert_eq!(next, 12);
            assert_eq!(
                table.head(block.key(12).unwrap()),
                Ok(TreeLink::Position(11))
            );
            let count = attempts.min(11);
            for position in 12 - count..12 {
                for side in [TreeSide::Smaller, TreeSide::Larger] {
                    assert_eq!(
                        table.edge(table.layout().edge(position, side).unwrap()),
                        Ok(TreeLink::Empty)
                    );
                }
            }
            if attempts < 11 {
                assert_eq!(
                    table.edge(
                        table
                            .layout()
                            .edge(10 - attempts, TreeSide::Larger)
                            .unwrap()
                    ),
                    Ok(TreeLink::Unsorted)
                );
            }
        }
    }
    #[test]
    fn stale_ring_links_stop_before_reused_children() {
        let source = [b'a'; 128];
        let (block, mut table) = setup(&source);
        let mut next = 0;
        prepare(block, &mut table, &mut next, 80, 256).unwrap();
        assert_eq!(next, 80);
        for p in 50..80 {
            assert_eq!(
                table.edge(table.layout().edge(p, TreeSide::Larger).unwrap()),
                Ok(TreeLink::Empty)
            );
        }
        assert_eq!(
            table.head(block.key(80).unwrap()),
            Ok(TreeLink::Position(79))
        );
    }
    #[test]
    fn strict_ordering_and_terminal_floor_preserve_the_candidate() {
        let mut source = [b'z'; 128];
        source[10] = b'a';
        source[20] = b'm';
        source[30] = b'z';
        let (block, mut table) = setup(&source);
        let layout = table.layout();
        table
            .set_edge(
                layout.edge(20, TreeSide::Smaller).unwrap(),
                TreeLink::Position(10),
            )
            .unwrap();
        table
            .set_edge(
                layout.edge(10, TreeSide::Larger).unwrap(),
                TreeLink::Unsorted,
            )
            .unwrap();
        // Raw candidate 12 equals node floor: compare it, never follow marker.
        sort_node(block, &mut table, 20, 8, 12).unwrap();
        assert_eq!(
            table.edge(layout.edge(20, TreeSide::Smaller).unwrap()),
            Ok(TreeLink::Position(10))
        );
        assert_eq!(
            table.edge(layout.edge(10, TreeSide::Larger).unwrap()),
            Ok(TreeLink::Unsorted)
        );
        table
            .set_edge(
                layout.edge(20, TreeSide::Smaller).unwrap(),
                TreeLink::Position(10),
            )
            .unwrap();
        assert_eq!(
            sort_node(block, &mut table, 20, 8, 11),
            Err(Error::CorruptData)
        );
    }
    #[test]
    fn skipped_frontier_and_scalar_errors_do_not_mutate_owners() {
        let source = [b'a'; 128];
        let (block, mut table) = setup(&source);
        let mut next = 21;
        let before = (table.test_maps().0.to_vec(), table.test_maps().1.to_vec());
        assert!(!prepare(block, &mut table, &mut next, 20, 8).unwrap());
        for (current, attempts) in [(121, 8), (20, 0), (20, 257), (usize::MAX, 8)] {
            assert_eq!(
                prepare(block, &mut table, &mut next, current, attempts),
                Err(Error::InvalidParameter)
            );
        }
        assert_eq!(next, 21);
        assert_eq!(table.test_maps(), (&before.0[..], &before.1[..]));
        assert_eq!(common_length(block, 20, 20, 0), Err(Error::CorruptData));
        assert_eq!(common_length(block, 20, 1, 109), Err(Error::CorruptData));
    }
    #[test]
    fn cyclic_and_future_pending_links_fail_with_bounded_work() {
        for previous in [TreeLink::Position(10), TreeLink::Position(12)] {
            let source = [b'a'; 128];
            let (block, mut table) = setup(&source);
            let mut next = 12;
            table.insert_unsorted(block.key(10).unwrap(), 10).unwrap();
            table
                .set_edge(
                    table.layout().edge(10, TreeSide::Smaller).unwrap(),
                    previous,
                )
                .unwrap();
            assert_eq!(
                prepare(block, &mut table, &mut next, 12, 8),
                Err(Error::CorruptData)
            );
        }
    }
}
