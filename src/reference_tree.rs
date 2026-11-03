//! Independent initialized owners for reference deferred binary-tree storage.
//! Non-owning positions/edges are indices; no source pointer is retained.
#![cfg_attr(not(feature = "unsafe-reference-tree"), forbid(unsafe_code))]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Tree search integration follows checked ownership"
    )
)]

use crate::greedy_key::GreedyKey;
use crate::tree_layout::{TreeEdge, TreeLayout, TreeLink, TreeSide};
use crate::{Error, Result};
use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

/// Borrowed tree cells shared by an owner and its immutable view.
///
/// The slices are the initialized head and edge arrays; the checked layout
/// fixes every bucket, slot and cyclic floor. No path in this type writes.
#[derive(Clone, Copy, Debug)]
struct TreeCells<'a> {
    heads: &'a [usize],
    edges: &'a [usize],
    layout: TreeLayout,
}

impl TreeCells<'_> {
    fn head(&self, key: GreedyKey) -> Result<TreeLink> {
        let value = TreeLink::from_stored(read_cell(self.heads, self.layout.key().bucket(key)?)?);
        if value == TreeLink::Unsorted {
            return Err(Error::CorruptData);
        }
        Ok(value)
    }
    fn edge(&self, edge: TreeEdge) -> Result<TreeLink> {
        let TreeEdge::Node { side, .. } = edge else {
            return Err(Error::InvalidParameter);
        };
        let index = self.layout.cell(edge)?.ok_or(Error::CorruptData)?;
        let value = TreeLink::from_stored(read_cell(self.edges, index)?);
        value.for_side(side)?;
        Ok(value)
    }
}

pub(crate) struct ReferenceTree<H: Allocator = Global, N: Allocator = H> {
    heads: Vec<usize, H>,
    edges: Vec<usize, N>,
    layout: TreeLayout,
}

fn read_cell(values: &[usize], index: usize) -> Result<usize> {
    if index >= values.len() {
        return Err(Error::CorruptData);
    }
    #[cfg(not(feature = "unsafe-reference-tree"))]
    let value = values[index];
    #[cfg(feature = "unsafe-reference-tree")]
    let value = {
        // SAFETY: The runtime guard proves index < initialized length, not
        // capacity. The slice borrows aligned, valid initialized usize storage
        // from its live owner. This shared borrow excludes mutation
        // or reallocation for the access. Only a copied value escapes; the
        // temporary reference cannot outlive this borrow or alias a write.
        unsafe { *values.get_unchecked(index) }
    };
    Ok(value)
}
fn write_cell(values: &mut [usize], index: usize, value: usize) -> Result<()> {
    if index >= values.len() {
        return Err(Error::CorruptData);
    }
    #[cfg(not(feature = "unsafe-reference-tree"))]
    {
        values[index] = value;
    }
    #[cfg(feature = "unsafe-reference-tree")]
    {
        // SAFETY: The runtime guard proves index < initialized length. This
        // slice borrows aligned live initialized usize storage; exclusive borrowing
        // excludes every concurrent read/write or reallocation. The reference
        // ends at this assignment. Copy replacement preserves initialization,
        // calls no allocator/drop code, and exposes no reference to the caller.
        unsafe {
            *values.get_unchecked_mut(index) = value;
        }
    }
    Ok(())
}

impl<H: Allocator, N: Allocator> ReferenceTree<H, N> {
    pub(crate) fn new_in(layout: TreeLayout, heads: H, nodes: N) -> Result<Self> {
        // TreeLayout already checked both complete allocation layouts and their
        // combined size. A failure drops each exact moved allocator owner once.
        fn initialized<A: Allocator>(count: usize, owner: A) -> Result<Vec<usize, A>> {
            let mut values = Vec::new_in(owner);
            values
                .try_reserve_exact(count)
                .map_err(|_| Error::AllocationFailed)?;
            while values.len() < count {
                values.push(usize::MAX);
            }
            Ok(values)
        }
        let heads = initialized(layout.key().cells(), heads)?;
        let edges = initialized(layout.edge_cells(), nodes)?;
        Ok(Self {
            heads,
            edges,
            layout,
        })
    }
    pub(crate) const fn layout(&self) -> TreeLayout {
        self.layout
    }
    pub(crate) fn reset(&mut self) {
        self.heads.fill(usize::MAX);
        self.edges.fill(usize::MAX);
    }
    fn cells(&self) -> TreeCells<'_> {
        TreeCells {
            heads: &self.heads,
            edges: &self.edges,
            layout: self.layout,
        }
    }
    pub(crate) fn head(&self, key: GreedyKey) -> Result<TreeLink> {
        self.cells().head(key)
    }
    fn preceding_head(&self, key: GreedyKey, position: usize) -> Result<(usize, TreeLink)> {
        TreeLink::position(position)?;
        let bucket = self.layout.key().bucket(key)?;
        let previous = self.head(key)?;
        match previous {
            TreeLink::Empty => {}
            TreeLink::Position(p) if p < position => {}
            _ => return Err(Error::CorruptData),
        }
        Ok((bucket, previous))
    }
    /// Install the query's head after pending inserts; reject malformed heads
    /// and duplicate/backward positions before writing. Returns its old head.
    pub(crate) fn replace_head(&mut self, key: GreedyKey, position: usize) -> Result<TreeLink> {
        let (bucket, previous) = self.preceding_head(key, position)?;
        write_cell(&mut self.heads, bucket, position)?;
        Ok(previous)
    }
    /// One ZSTD_updateDUBT insertion. Validate all three destinations and the
    /// predecessor before changing any owner, including shortened-owner errors.
    pub(crate) fn insert_unsorted(&mut self, key: GreedyKey, position: usize) -> Result<()> {
        let (bucket, previous) = self.preceding_head(key, position)?;
        let smaller = self
            .layout
            .cell(self.layout.edge(position, TreeSide::Smaller)?)?
            .ok_or(Error::CorruptData)?;
        let larger = self
            .layout
            .cell(self.layout.edge(position, TreeSide::Larger)?)?
            .ok_or(Error::CorruptData)?;
        if bucket >= self.heads.len() || smaller >= self.edges.len() || larger >= self.edges.len() {
            return Err(Error::CorruptData);
        }
        let previous = previous.stored()?;
        let marker = TreeLink::Unsorted.for_side(TreeSide::Larger)?;
        // No allocation, callbacks or resizing intervene after all live bounds
        // were checked. Each helper additionally enforces its own bounds.
        write_cell(&mut self.heads, bucket, position)?;
        write_cell(&mut self.edges, smaller, previous)?;
        write_cell(&mut self.edges, larger, marker)
    }
    pub(crate) fn edge(&self, edge: TreeEdge) -> Result<TreeLink> {
        self.cells().edge(edge)
    }
    /// Borrow the immutable heads and edges without allocating or gaining a
    /// write path. A compiled dictionary keeps this view alive while calls
    /// sample and sort their own tree.
    pub(crate) fn view(&self) -> ReferenceTreeView<'_> {
        ReferenceTreeView {
            cells: self.cells(),
        }
    }
    pub(crate) fn set_edge(&mut self, edge: TreeEdge, value: TreeLink) -> Result<()> {
        let stored = match edge {
            TreeEdge::Discard => {
                value.stored()?;
                return Ok(());
            }
            TreeEdge::Node { side, .. } => value.for_side(side)?,
        };
        let index = self.layout.cell(edge)?.ok_or(Error::CorruptData)?;
        write_cell(&mut self.edges, index, stored)
    }
    #[cfg(test)]
    pub(crate) fn test_maps(&self) -> (&[usize], &[usize]) {
        (&self.heads, &self.edges)
    }
}

/// An immutable borrow of a compiled tree's heads and edges.
///
/// The view offers the same checked head and edge reads as the owner and no
/// insert, replace, edge write, reset or reconfiguration, so a call can
/// consult a fully sorted compiled tree while its own tree receives sampling
/// and lazy sorting. Positions index the staged prefix.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReferenceTreeView<'a> {
    cells: TreeCells<'a>,
}

impl<'a> ReferenceTreeView<'a> {
    /// The checked layout the borrowed cells were built under.
    pub(crate) const fn layout(&self) -> TreeLayout {
        self.cells.layout
    }

    /// The same checked head read the owner offers; cells stay read-only.
    pub(crate) fn head(&self, key: GreedyKey) -> Result<TreeLink> {
        self.cells.head(key)
    }

    /// The same checked edge read the owner offers; cells stay read-only.
    pub(crate) fn edge(&self, edge: TreeEdge) -> Result<TreeLink> {
        self.cells.edge(edge)
    }

    /// Inspect the borrowed cells for frozen equality checks only.
    #[cfg(test)]
    pub(crate) const fn test_maps(&self) -> (&'a [usize], &'a [usize]) {
        (self.cells.heads, self.cells.edges)
    }
}

/// A tree view paired with the staged-prefix length its positions index.
///
/// A call must stage exactly this many suffix bytes before its input for the
/// positions to index the same bytes, so per-call states reject a different
/// staged length before consulting any candidate.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StagedTreeView<'a> {
    view: ReferenceTreeView<'a>,
    staged_len: usize,
}

impl<'a> StagedTreeView<'a> {
    /// Pair an immutable tree view with the prefix length it indexes.
    pub(crate) const fn new(view: ReferenceTreeView<'a>, staged_len: usize) -> Self {
        Self { view, staged_len }
    }

    /// The read-only tree a parser consults.
    pub(crate) const fn view(&self) -> ReferenceTreeView<'a> {
        self.view
    }

    /// The exact staged prefix length the positions require.
    pub(crate) const fn staged_len(&self) -> usize {
        self.staged_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greedy_key::GreedyKeyLayout;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    fn layout() -> TreeLayout {
        TreeLayout::new(GreedyKeyLayout::new(6, 4).unwrap(), 6).unwrap()
    }
    fn key() -> GreedyKey {
        layout().key().read_key(b"abcd").unwrap()
    }
    fn tree() -> ReferenceTree {
        ReferenceTree::new_in(layout(), Global, Global).unwrap()
    }
    #[test]
    fn complete_unsorted_maps_preserve_zero_bias_and_wrap() {
        let mut t = tree();
        let bucket = layout().key().bucket(key()).unwrap();
        let mut heads = alloc::vec![usize::MAX;64];
        let mut edges = alloc::vec![usize::MAX;64];
        for position in 0..=96 {
            t.insert_unsorted(key(), position).unwrap();
            let slot = ((position + 2) % 32) * 2;
            edges[slot] = heads[bucket];
            edges[slot + 1] = usize::MAX - 1;
            heads[bucket] = position;
            assert_eq!(t.test_maps(), (&heads[..], &edges[..]));
        }
        assert_eq!(t.head(key()), Ok(TreeLink::Position(96)));
        assert_eq!(t.layout(), layout());
        t.reset();
        assert!(
            t.test_maps()
                .0
                .iter()
                .chain(t.test_maps().1)
                .all(|&v| v == usize::MAX)
        );
        t.insert_unsorted(key(), 0).unwrap();
        assert_eq!(t.head(key()), Ok(TreeLink::Position(0)));
        assert_eq!(t.replace_head(key(), 2), Ok(TreeLink::Position(0)));
        assert_eq!(t.head(key()), Ok(TreeLink::Position(2)));
    }
    #[test]
    fn checked_edges_never_alias_borrows_and_discard_changes_no_owner() {
        let mut t = tree();
        let a = layout().edge(7, TreeSide::Smaller).unwrap();
        let b = layout().edge(7, TreeSide::Larger).unwrap();
        t.set_edge(a, TreeLink::Position(0)).unwrap();
        t.set_edge(b, TreeLink::Unsorted).unwrap();
        assert_eq!(t.edge(a), Ok(TreeLink::Position(0)));
        assert_eq!(t.edge(b), Ok(TreeLink::Unsorted));
        let value = t.edge(a).unwrap();
        t.set_edge(b, value).unwrap();
        assert_eq!(t.edge(b), Ok(value));
        let before = (t.heads.clone(), t.edges.clone());
        t.set_edge(TreeEdge::Discard, TreeLink::Empty).unwrap();
        assert_eq!(t.edge(TreeEdge::Discard), Err(Error::InvalidParameter));
        assert_eq!(t.set_edge(a, TreeLink::Unsorted), Err(Error::CorruptData));
        assert_eq!(
            t.set_edge(a, TreeLink::Position(usize::MAX)),
            Err(Error::InvalidParameter)
        );
        assert_eq!(t.test_maps(), (&before.0[..], &before.1[..]));
    }
    #[test]
    fn malformed_heads_dimensions_and_short_owners_reject_before_writes() {
        let mut t = tree();
        let bucket = layout().key().bucket(key()).unwrap();
        for value in [usize::MAX - 1, 7, 8, usize::MAX - 2] {
            t.heads[bucket] = value;
            let before = (t.heads.clone(), t.edges.clone());
            assert_eq!(t.insert_unsorted(key(), 7), Err(Error::CorruptData));
            assert_eq!(t.replace_head(key(), 7), Err(Error::CorruptData));
            assert_eq!(t.test_maps(), (&before.0[..], &before.1[..]));
        }
        t.reset();
        t.edges.truncate(4);
        let before = t.heads.clone();
        assert_eq!(t.insert_unsorted(key(), 0), Err(Error::CorruptData));
        assert_eq!(t.heads, before);
        assert_eq!(
            t.edge(layout().edge(0, TreeSide::Larger).unwrap()),
            Err(Error::CorruptData)
        );
        assert_eq!(
            t.set_edge(
                TreeEdge::Node {
                    slot: 32,
                    side: TreeSide::Smaller
                },
                TreeLink::Empty
            ),
            Err(Error::CorruptData)
        );
        t.heads.clear();
        assert_eq!(t.head(key()), Err(Error::CorruptData));
        assert_eq!(t.insert_unsorted(key(), 0), Err(Error::CorruptData));
    }
    #[test]
    fn incompatible_keys_reserved_positions_and_markers_are_checked() {
        let mut t = tree();
        let other = GreedyKeyLayout::new(6, 5)
            .unwrap()
            .read_key(b"abcde")
            .unwrap();
        assert_eq!(t.insert_unsorted(other, 0), Err(Error::InvalidParameter));
        assert_eq!(t.replace_head(other, 0), Err(Error::InvalidParameter));
        for p in [usize::MAX - 1, usize::MAX] {
            assert_eq!(t.insert_unsorted(key(), p), Err(Error::InvalidParameter));
        }
        let smaller = layout().edge(0, TreeSide::Smaller).unwrap();
        let index = layout().cell(smaller).unwrap().unwrap();
        t.edges[index] = usize::MAX - 1;
        assert_eq!(t.edge(smaller), Err(Error::CorruptData));
        t.reset();
        t.insert_unsorted(key(), usize::MAX - 2).unwrap();
        assert_eq!(t.head(key()), Ok(TreeLink::Position(usize::MAX - 2)));
    }
    #[test]
    fn independent_nonclone_allocators_release_after_each_failure() {
        for reject in 0..=2 {
            let h = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let n = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let hm = h.monitor();
            let nm = n.monitor();
            if reject == 1 {
                assert!(hm.fail_next());
            }
            if reject == 2 {
                assert!(nm.fail_next());
            }
            let result = ReferenceTree::new_in(layout(), h, n);
            if reject == 0 {
                let mut t = result.unwrap();
                assert_eq!(
                    hm.snapshot().live_requested_bytes,
                    layout().head_bytes() as u128
                );
                assert_eq!(
                    nm.snapshot().live_requested_bytes,
                    layout().edge_bytes() as u128
                );
                t.insert_unsorted(key(), 0).unwrap();
                t.reset();
                assert_eq!(hm.snapshot().attempts, 1);
                assert_eq!(nm.snapshot().attempts, 1);
                drop(t);
            } else {
                assert!(matches!(result, Err(Error::AllocationFailed)));
            }
            assert_eq!(hm.snapshot().attempts, 1);
            assert_eq!(nm.snapshot().attempts, u64::from(reject != 1));
            for monitor in [hm, nm] {
                monitor.assert_released();
                assert_eq!(monitor.snapshot().owners_dropped, 1);
            }
        }
    }
    #[test]
    fn borrowed_allocators_survive_reset_and_release_exact_layouts() {
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut t = ReferenceTree::new_in(layout(), &owner, &owner).unwrap();
        assert_eq!(
            monitor.snapshot().live_requested_bytes,
            layout().total_bytes() as u128
        );
        assert!(monitor.fail_next());
        t.reset();
        t.insert_unsorted(key(), 1).unwrap();
        assert_eq!(monitor.snapshot().attempts, 2);
        drop(t);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        assert!(matches!(
            ReferenceTree::new_in(layout(), &owner, &owner),
            Err(Error::AllocationFailed)
        ));
        drop(owner);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }

    #[test]
    fn view_reads_match_the_owner_and_expose_no_write_path() {
        let mut t = tree();
        t.insert_unsorted(key(), 10).unwrap();
        t.set_edge(
            layout().edge(10, TreeSide::Larger).unwrap(),
            TreeLink::Unsorted,
        )
        .unwrap();
        let view = t.view();
        assert_eq!(view.layout(), layout());
        assert_eq!(view.head(key()), Ok(TreeLink::Position(10)));
        assert_eq!(
            view.edge(layout().edge(10, TreeSide::Larger).unwrap()),
            Ok(TreeLink::Unsorted)
        );
        assert_eq!(
            view.edge(layout().edge(10, TreeSide::Smaller).unwrap()),
            Ok(TreeLink::Empty)
        );
        assert_eq!(view.edge(TreeEdge::Discard), Err(Error::InvalidParameter));
        assert_eq!(view.test_maps(), t.test_maps());
        let other = GreedyKeyLayout::new(6, 5)
            .unwrap()
            .read_key(b"abcde")
            .unwrap();
        assert_eq!(view.head(other), Err(Error::InvalidParameter));
        // Owner writes stay the only write path; a fresh view re-derives from
        // the same borrowed cells.
        t.set_edge(
            layout().edge(10, TreeSide::Smaller).unwrap(),
            TreeLink::Position(4),
        )
        .unwrap();
        let view = t.view();
        assert_eq!(
            view.edge(layout().edge(10, TreeSide::Smaller).unwrap()),
            Ok(TreeLink::Position(4))
        );
        assert_eq!(view.head(key()), Ok(TreeLink::Position(10)));
    }

    #[test]
    fn staged_tree_view_carries_and_reports_its_prefix_length() {
        let mut t = tree();
        t.insert_unsorted(key(), 3).unwrap();
        let view = t.view();
        let staged = StagedTreeView::new(view, 64);
        assert_eq!(staged.staged_len(), 64);
        assert_eq!(staged.view().layout(), layout());
        assert_eq!(staged.view().head(key()), Ok(TreeLink::Position(3)));
    }

    #[cfg(feature = "std")]
    fn workload_keys(layout: TreeLayout, count: usize) -> Vec<GreedyKey> {
        let mut state = 0x5eed_b71e_2026_1013_u64;
        (0..count)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                layout.key().read_key(&state.to_le_bytes()).unwrap()
            })
            .collect()
    }
    #[cfg(feature = "std")]
    fn workload(tree: &mut ReferenceTree, keys: &[GreedyKey]) {
        tree.reset();
        let layout = tree.layout();
        for (position, key) in keys.iter().enumerate() {
            let previous = tree
                .replace_head(core::hint::black_box(*key), position)
                .unwrap();
            let a = layout.edge(position, TreeSide::Smaller).unwrap();
            let b = layout.edge(position, TreeSide::Larger).unwrap();
            tree.set_edge(a, previous).unwrap();
            tree.set_edge(b, TreeLink::Unsorted).unwrap();
            if position % 5 == 0 {
                let v = tree.edge(a).unwrap();
                tree.set_edge(b, v).unwrap();
                tree.set_edge(a, TreeLink::Empty).unwrap();
            }
            if position % 7 == 0 {
                tree.set_edge(b, TreeLink::Empty).unwrap();
            }
        }
        core::hint::black_box(tree.test_maps());
    }
    #[test]
    #[cfg(feature = "std")]
    fn deterministic_workload_preserves_complete_abstract_cell_state() {
        for (h, c, count) in [(6, 6, 4096), (12, 12, 16384), (19, 19, 32768)] {
            let layout = TreeLayout::new(GreedyKeyLayout::new(h, 4).unwrap(), c).unwrap();
            let keys = workload_keys(layout, count);
            let mut t = ReferenceTree::new_in(layout, Global, Global).unwrap();
            workload(&mut t, &keys);
            let mut heads = alloc::vec![usize::MAX;layout.key().cells()];
            let mut edges = alloc::vec![usize::MAX;layout.edge_cells()];
            for (p, key) in keys.into_iter().enumerate() {
                let bucket = layout.key().bucket(key).unwrap();
                let previous = core::mem::replace(&mut heads[bucket], p);
                let index = ((p + 2) % layout.nodes()) * 2;
                edges[index] = if p % 5 == 0 { usize::MAX } else { previous };
                edges[index + 1] = if p % 7 == 0 {
                    usize::MAX
                } else if p % 5 == 0 {
                    previous
                } else {
                    usize::MAX - 1
                };
            }
            assert_eq!(t.test_maps(), (&heads[..], &edges[..]));
        }
    }
    /// Explicit primitive benchmark: reset and checked access only; allocation,
    /// key generation, complete map serialization and validation are untimed.
    #[test]
    #[ignore = "explicit owner timing requires completed task-owned CPU checks"]
    #[cfg(feature = "std")]
    fn measure_identical_owner_workloads() {
        use std::time::{Duration, Instant};
        let output = std::path::PathBuf::from(
            std::env::var_os("ZSTDOXIDE_TREE_OWNER_OUTPUT")
                .expect("explicit fresh output directory"),
        );
        assert!(!output.exists());
        std::fs::create_dir_all(&output).unwrap();
        for (h, c, count) in [(6, 6, 4096), (12, 12, 16384), (19, 19, 32768)] {
            let layout = TreeLayout::new(GreedyKeyLayout::new(h, 4).unwrap(), c).unwrap();
            let keys = workload_keys(layout, count);
            let mut t = ReferenceTree::new_in(layout, Global, Global).unwrap();
            workload(&mut t, &keys);
            let mut samples = Vec::new();
            for _ in 0..5 {
                let start = Instant::now();
                let mut iterations = 0_u64;
                while start.elapsed() < Duration::from_millis(100) {
                    workload(&mut t, &keys);
                    iterations += 1;
                }
                samples.push(format!(
                    "{{\"iterations\":{iterations},\"nanoseconds\":{}}}",
                    start.elapsed().as_nanos()
                ));
            }
            let mut bytes = Vec::new();
            for &cell in t.test_maps().0.iter().chain(t.test_maps().1) {
                let raw = match TreeLink::from_stored(cell) {
                    TreeLink::Empty => 0,
                    TreeLink::Unsorted => 1,
                    TreeLink::Position(p) => p as u64 + 2,
                };
                bytes.extend_from_slice(&raw.to_le_bytes());
            }
            let name = format!("h{h}-c{c}");
            std::fs::write(output.join(format!("{name}.bin")), &bytes).unwrap();
            println!(
                "TREE_OWNER {{\"case\":\"{name}\",\"keys\":{count},\"map_bytes\":{},\"unsafe_tree\":{},\"samples\":[{}]}}",
                bytes.len(),
                cfg!(feature = "unsafe-reference-tree"),
                samples.join(",")
            );
        }
    }
}
