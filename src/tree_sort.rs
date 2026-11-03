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

/// The pinned eight-byte query footprint (`HASH_READ_SIZE`): positions within
/// this distance of the loaded end are never inserted, because no full query
/// footprint fits behind them.
const QUERY_FOOTPRINT: usize = 8;

/// Common prefix length of two positions inside one explicit load limit.
///
/// The pinned insertion compares only within the loaded region
/// (`ZSTD_count(ip+matchLength, match+matchLength, iend)` with `iend` at the
/// load end), so discriminating bytes stay inside `source[..limit]` and never
/// cross into later input. Same checked shape as `common_length`.
fn fill_common_length(
    source: &[u8],
    current: usize,
    previous: usize,
    common: usize,
    limit: usize,
) -> Result<usize> {
    if previous >= current {
        return Err(Error::CorruptData);
    }
    let q = source.get(current..limit).ok_or(Error::CorruptData)?;
    let p = source.get(previous..limit).ok_or(Error::CorruptData)?;
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

/// Insert and fully sort one position, mirroring `ZSTD_insertBt1` (v1.5.7).
///
/// The head is replaced first; the traversal then sorts the new node against
/// the old chain with path-compressed edge writes, a window floor computed
/// from the fill target, a per-position node floor, and `attempts` comparisons.
/// A candidate matching to the load end is dropped ("equal : no way to know if
/// inf or sup"), leaving the node's edges empty. Returns the forward skip:
/// the largest common match extends the sampling frontier, plus the repetitive
/// pattern speedup (up to 192 positions once a match exceeds 384 bytes).
fn insert_sorted_node<H: Allocator, N: Allocator>(
    table: &mut ReferenceTree<H, N>,
    source: &[u8],
    limit: usize,
    current: usize,
    fill_target: usize,
    window_size: usize,
    attempts: usize,
) -> Result<usize> {
    let layout = table.layout();
    let key = layout
        .key()
        .read_key(source.get(current..limit).ok_or(Error::InvalidState)?)
        .ok_or(Error::InvalidState)?;
    let bounds = TreeProbe::for_fill(current, limit, fill_target, window_size, layout)?;
    let mut link = table.replace_head(key, current)?;
    let mut smaller = layout.edge(current, TreeSide::Smaller)?;
    let mut larger = layout.edge(current, TreeSide::Larger)?;
    let (mut cs, mut cl) = (0, 0);
    let mut remaining = attempts;
    let mut ceiling = current;
    let mut best_length = 8;
    let mut match_end = current + 9;
    while remaining > 0 {
        let Some(previous) = bounds.candidate(link)? else {
            break;
        };
        remaining -= 1;
        if previous >= ceiling {
            return Err(Error::CorruptData);
        }
        ceiling = previous;
        let length = fill_common_length(source, current, previous, cs.min(cl), limit)?;
        if length > best_length {
            best_length = length;
            // previous + length stays below the limit because the comparison
            // stopped at it.
            match_end = match_end.max(previous + length);
        }
        if current + length == limit {
            break;
        }
        // fill_common_length proves both discriminating bytes exist.
        if source[previous + length] < source[current + length] {
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
    let speedup = best_length.saturating_sub(384).min(192);
    Ok(match_end.saturating_sub(current + 8).max(speedup))
}

/// Eagerly insert and fully sort every loaded prefix position.
///
/// This is the dictionary-side load contract for the binary-tree family
/// ("we want the dictionary table fully sorted", `ZSTD_loadDictionaryContent`)
/// that `ZSTD_updateTree` performs through `ZSTD_insertBt1`: ascending
/// positions, target-based window floors, per-fill attempt caps, and the
/// forward skip over repetitive patterns. Positions within one query
/// footprint of the end are never inserted, exactly as in the pinned update.
/// A read-only consultation requires these fully sorted edges because it has
/// no path to resolve an unsorted mark.
pub(crate) fn load_sorted_prefix<H: Allocator, N: Allocator>(
    table: &mut ReferenceTree<H, N>,
    source: &[u8],
    prefix_len: usize,
    window_size: usize,
    attempts: usize,
) -> Result<()> {
    if prefix_len > source.len() || !(1..=256).contains(&attempts) {
        return Err(Error::InvalidParameter);
    }
    let target = prefix_len.saturating_sub(QUERY_FOOTPRINT);
    let mut index = 0;
    while index < target {
        let forward = insert_sorted_node(
            table,
            source,
            prefix_len,
            index,
            target,
            window_size,
            attempts,
        )?;
        index = index.checked_add(forward).ok_or(Error::ResourceLimit)?;
    }
    Ok(())
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

    #[test]
    fn sorted_prefix_load_builds_a_descending_sorted_tree() {
        let source = [b'a'; 160];
        let key = GreedyKeyLayout::new(6, 4).unwrap();
        let layout = TreeLayout::new(key, 8).unwrap();
        let mut table = ReferenceTree::new_in(layout, Global, Global).unwrap();
        load_sorted_prefix(&mut table, &source, 120, 1024, 8).unwrap();
        // The head of the query key is the newest inserted position, and every
        // stored edge points strictly below its parent position: an in-order
        // descent can only visit decreasing positions.
        let query = key.read_key(&source).unwrap();
        let head = table.head(query).unwrap();
        let TreeLink::Position(head) = head else {
            panic!("a loaded tree must hold the query head")
        };
        assert!(head >= 112 - 8, "newest inserted position {head}");
        let mut stack = vec![head];
        while let Some(position) = stack.pop() {
            for side in [TreeSide::Smaller, TreeSide::Larger] {
                let link = table.edge(layout.edge(position, side).unwrap()).unwrap();
                match link {
                    TreeLink::Empty => {}
                    TreeLink::Position(child) => {
                        assert!(child < position, "{position} -> {child}");
                        stack.push(child);
                    }
                    TreeLink::Unsorted => panic!("loaded tree holds an unsorted mark"),
                }
            }
        }
        // The last inserted position is the target boundary minus one; the
        // final eight positions are never inserted.
        let last_inserted = 120 - 8 - 1;
        assert!(head <= last_inserted, "head {head} above {last_inserted}");
        // Two independent loads of the identical prefix produce the identical
        // sorted shape (reloading onto a loaded tree is not a contract: the
        // ascending insertion contract forbids backward heads).
        let mut reference = ReferenceTree::new_in(layout, Global, Global).unwrap();
        load_sorted_prefix(&mut reference, &source, 120, 1024, 8).unwrap();
        assert_eq!(table.test_maps(), reference.test_maps());
    }

    #[test]
    fn sorted_prefix_load_rejects_malformed_arguments_before_writes() {
        let source = [b'a'; 128];
        let key = GreedyKeyLayout::new(6, 4).unwrap();
        let layout = TreeLayout::new(key, 8).unwrap();
        let mut table = ReferenceTree::new_in(layout, Global, Global).unwrap();
        let before = (table.test_maps().0.to_vec(), table.test_maps().1.to_vec());
        assert_eq!(
            load_sorted_prefix(&mut table, &source, 129, 1024, 8),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            load_sorted_prefix(&mut table, &source, 64, 1024, 0),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            load_sorted_prefix(&mut table, &source, 64, 1024, 257),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            load_sorted_prefix(&mut table, &source, 64, 512, 8),
            Err(Error::InvalidParameter)
        );
        assert_eq!(table.test_maps(), (&before.0[..], &before.1[..]));
    }

    #[test]
    fn sorted_prefix_load_skips_tiny_prefixes_and_repeats() {
        let key = GreedyKeyLayout::new(6, 4).unwrap();
        let layout = TreeLayout::new(key, 6).unwrap();
        // A prefix shorter than one query footprint inserts nothing.
        let short = [b'a'; 8];
        let mut table = ReferenceTree::new_in(layout, Global, Global).unwrap();
        load_sorted_prefix(&mut table, &short, 8, 1024, 8).unwrap();
        let (heads, _) = table.test_maps();
        assert!(heads.iter().all(|&h| h == usize::MAX));
        // A repetitive pattern stays fully sorted with a bounded node count:
        // the forward skip keeps the update visiting a shrinking prefix.
        let repeat = [b'a'; 256];
        let mut table = ReferenceTree::new_in(layout, Global, Global).unwrap();
        load_sorted_prefix(&mut table, &repeat, 256, 1024, 4).unwrap();
        let (heads, _) = table.test_maps();
        assert!(
            heads.iter().any(|&h| h != usize::MAX),
            "the query head must exist"
        );
    }

    #[test]
    fn fill_probe_uses_the_target_window_floor() {
        let l = TreeLayout::new(GreedyKeyLayout::new(6, 4).unwrap(), 6).unwrap();
        // A floor from the fill target, not the inserted position.
        let p = TreeProbe::for_fill(2000, 2200, 2048, 1024, l).unwrap();
        assert_eq!(p.window_floor_raw(), 1026);
        assert_eq!(p.node_floor_raw(), 1971);
        assert_eq!(p.candidate(TreeLink::Position(1024)), Ok(None));
        assert_eq!(p.candidate(TreeLink::Position(1025)), Ok(Some(1025)));
        for (current, source_end, window) in [(1, 8, 1024), (0, 8, 512), (0, 7, 1024)] {
            assert!(TreeProbe::for_fill(current, source_end, 8, window, l).is_err());
        }
    }
}
