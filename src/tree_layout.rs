//! Checked value records for the bounded reference deferred binary tree.
//! Source positions and destinations are copied indices, never stored borrows.
//! Scalar records have no unsafe variant: ownership/indexing lives in the owner.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Tree owners and search consumers follow these checked records"
    )
)]

use crate::greedy_key::GreedyKeyLayout;
use crate::{Error, Result};
use core::alloc::Layout;

const EMPTY: usize = usize::MAX;
const UNSORTED: usize = usize::MAX - 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TreeSide {
    Smaller,
    Larger,
}
impl TreeSide {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Smaller => 0,
            Self::Larger => 1,
        }
    }
}

/// The second edge is a marker only during unsorted insertion. All other links
/// are logical source indices or empty. Corresponds to DUBT's raw 0/1/index cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TreeLink {
    Empty,
    Unsorted,
    Position(usize),
}
impl TreeLink {
    pub(crate) fn position(value: usize) -> Result<Self> {
        if value >= UNSORTED {
            return Err(Error::InvalidParameter);
        }
        Ok(Self::Position(value))
    }
    pub(crate) const fn from_stored(value: usize) -> Self {
        match value {
            EMPTY => Self::Empty,
            UNSORTED => Self::Unsorted,
            v => Self::Position(v),
        }
    }
    pub(crate) fn stored(self) -> Result<usize> {
        match self {
            Self::Empty => Ok(EMPTY),
            Self::Unsorted => Ok(UNSORTED),
            Self::Position(v) if v < UNSORTED => Ok(v),
            Self::Position(_) => Err(Error::InvalidParameter),
        }
    }
    pub(crate) fn for_side(self, side: TreeSide) -> Result<usize> {
        if self == Self::Unsorted && side != TreeSide::Larger {
            return Err(Error::CorruptData);
        }
        self.stored()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TreeEdge {
    Node { slot: usize, side: TreeSide },
    Discard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TreeLayout {
    key: GreedyKeyLayout,
    chain_log: u8,
    nodes: usize,
    head_bytes: usize,
    edge_bytes: usize,
    total_bytes: usize,
}
impl TreeLayout {
    /// Check all dimensions and complete independent layouts before allocation.
    /// C's chainLog counts edge cells, not paired nodes.
    pub(crate) fn new(key: GreedyKeyLayout, chain_log: u8) -> Result<Self> {
        if !(6..=19).contains(&chain_log) {
            return Err(Error::InvalidParameter);
        }
        let edges = 1_usize
            .checked_shl(u32::from(chain_log))
            .ok_or(Error::ResourceLimit)?;
        let head_bytes = Layout::array::<usize>(key.cells())
            .map_err(|_| Error::ResourceLimit)?
            .size();
        let edge_bytes = Layout::array::<usize>(edges)
            .map_err(|_| Error::ResourceLimit)?
            .size();
        let total_bytes = head_bytes
            .checked_add(edge_bytes)
            .ok_or(Error::ResourceLimit)?;
        Ok(Self {
            key,
            chain_log,
            nodes: edges / 2,
            head_bytes,
            edge_bytes,
            total_bytes,
        })
    }
    pub(crate) const fn key(self) -> GreedyKeyLayout {
        self.key
    }
    pub(crate) const fn chain_log(self) -> u8 {
        self.chain_log
    }
    pub(crate) const fn nodes(self) -> usize {
        self.nodes
    }
    pub(crate) const fn edge_cells(self) -> usize {
        self.nodes * 2
    }
    pub(crate) const fn head_bytes(self) -> usize {
        self.head_bytes
    }
    pub(crate) const fn edge_bytes(self) -> usize {
        self.edge_bytes
    }
    pub(crate) const fn total_bytes(self) -> usize {
        self.total_bytes
    }
    pub(crate) fn slot(self, position: usize) -> Result<usize> {
        TreeLink::position(position)?;
        let mask = self.nodes - 1;
        // Masking before the bias addition prevents large-position overflow.
        Ok(((position & mask) + 2) & mask)
    }
    pub(crate) fn edge(self, position: usize, side: TreeSide) -> Result<TreeEdge> {
        Ok(TreeEdge::Node {
            slot: self.slot(position)?,
            side,
        })
    }
    pub(crate) fn cell(self, edge: TreeEdge) -> Result<Option<usize>> {
        match edge {
            TreeEdge::Discard => Ok(None),
            TreeEdge::Node { slot, side } if slot < self.nodes => Ok(Some(slot * 2 + side.index())),
            TreeEdge::Node { .. } => Err(Error::CorruptData),
        }
    }
}

/// Copied inclusive probe coordinates. Raw floors retain the upstream bias so
/// strict comparisons never collapse into a saturating logical-zero boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TreeProbe {
    current: usize,
    source_end: usize,
    window_floor_raw: usize,
    node_floor_raw: usize,
}
impl TreeProbe {
    pub(crate) fn new(
        current: usize,
        source_end: usize,
        prefix_floor: usize,
        window_size: usize,
        layout: TreeLayout,
    ) -> Result<Self> {
        if !(1024..=2_097_152).contains(&window_size)
            || !window_size.is_power_of_two()
            || prefix_floor > current
            || current.checked_add(8).is_none_or(|end| end > source_end)
            || source_end.checked_add(2).is_none()
        {
            return Err(Error::InvalidParameter);
        }
        let raw = current + 2; // The checked eight-byte footprint proves this addition.
        Ok(Self {
            current,
            source_end,
            window_floor_raw: (prefix_floor + 2).max(raw.saturating_sub(window_size)),
            node_floor_raw: raw.saturating_sub(layout.nodes() - 1),
        })
    }
    pub(crate) const fn current(self) -> usize {
        self.current
    }
    pub(crate) const fn source_end(self) -> usize {
        self.source_end
    }
    pub(crate) const fn window_floor_raw(self) -> usize {
        self.window_floor_raw
    }
    pub(crate) const fn node_floor_raw(self) -> usize {
        self.node_floor_raw
    }
    pub(crate) fn candidate(self, link: TreeLink) -> Result<Option<usize>> {
        match link {
            TreeLink::Empty => Ok(None),
            TreeLink::Unsorted => Err(Error::CorruptData),
            TreeLink::Position(p) if p < self.current => {
                Ok((p + 2 > self.window_floor_raw).then_some(p))
            }
            TreeLink::Position(_) => Err(Error::CorruptData),
        }
    }
    pub(crate) fn can_follow(self, position: usize) -> Result<bool> {
        if position >= self.current {
            return Err(Error::CorruptData);
        }
        Ok(position + 2 > self.node_floor_raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn layout(c: u8) -> TreeLayout {
        TreeLayout::new(GreedyKeyLayout::new(6, 4).unwrap(), c).unwrap()
    }
    #[test]
    fn layouts_count_edges_and_nodes_separately_before_allocation() {
        for c in 6..=19 {
            let l = layout(c);
            assert_eq!(l.chain_log(), c);
            assert_eq!(l.nodes(), 1 << (c - 1));
            assert_eq!(l.edge_cells(), 1 << c);
            assert_eq!(l.key().cells(), 64);
            assert_eq!(l.head_bytes(), 64 * size_of::<usize>());
            assert_eq!(l.edge_bytes(), (1 << c) * size_of::<usize>());
            assert_eq!(l.total_bytes(), l.head_bytes() + l.edge_bytes());
        }
        for c in [0, 5, 20, u8::MAX] {
            assert_eq!(
                TreeLayout::new(GreedyKeyLayout::new(6, 4).unwrap(), c),
                Err(Error::InvalidParameter)
            );
        }
    }
    #[test]
    fn links_keep_source_zero_distinct_from_empty_and_unsorted() {
        for v in [0, 1, 32, usize::MAX - 2] {
            let p = TreeLink::position(v).unwrap();
            assert_eq!(TreeLink::from_stored(p.stored().unwrap()), p);
        }
        assert_eq!(TreeLink::from_stored(EMPTY), TreeLink::Empty);
        assert_eq!(TreeLink::from_stored(UNSORTED), TreeLink::Unsorted);
        assert_eq!(TreeLink::Empty.stored(), Ok(EMPTY));
        assert_eq!(TreeLink::Unsorted.for_side(TreeSide::Larger), Ok(UNSORTED));
        assert_eq!(
            TreeLink::Unsorted.for_side(TreeSide::Smaller),
            Err(Error::CorruptData)
        );
        for v in [usize::MAX - 1, usize::MAX] {
            assert_eq!(TreeLink::position(v), Err(Error::InvalidParameter));
            assert_eq!(TreeLink::Position(v).stored(), Err(Error::InvalidParameter));
        }
    }
    #[test]
    fn biased_slots_wrap_and_discard_has_no_cell() {
        let l = layout(6);
        for (p, s) in [
            (0, 2),
            (29, 31),
            (30, 0),
            (31, 1),
            (32, 2),
            (usize::MAX - 2, 31),
        ] {
            assert_eq!(l.slot(p), Ok(s));
            assert_eq!(
                l.cell(l.edge(p, TreeSide::Smaller).unwrap()),
                Ok(Some(2 * s))
            );
            assert_eq!(
                l.cell(l.edge(p, TreeSide::Larger).unwrap()),
                Ok(Some(2 * s + 1))
            );
        }
        assert_eq!(l.cell(TreeEdge::Discard), Ok(None));
        assert_eq!(
            l.cell(TreeEdge::Node {
                slot: 32,
                side: TreeSide::Smaller
            }),
            Err(Error::CorruptData)
        );
        assert_eq!(l.slot(usize::MAX), Err(Error::InvalidParameter));
    }
    #[test]
    fn inclusive_tail_and_overflow_domains_reject_before_source_access() {
        let l = layout(6);
        let p = TreeProbe::new(0, 8, 0, 1024, l).unwrap();
        assert_eq!(p.current(), 0);
        assert_eq!(p.source_end(), 8);
        for (q, end, floor, window) in [
            (1, 8, 0, 1024),
            (0, 7, 0, 1024),
            (0, 8, 1, 1024),
            (0, 8, 0, 1000),
            (0, 8, 0, 512),
            (0, 8, 0, 4194304),
            (usize::MAX - 7, usize::MAX, 0, 1024),
            (0, usize::MAX, 0, 1024),
        ] {
            assert_eq!(
                TreeProbe::new(q, end, floor, window, l),
                Err(Error::InvalidParameter)
            );
        }
    }
    #[test]
    fn window_floor_is_strict_and_independent_from_node_traversal() {
        let p = TreeProbe::new(2048, 2200, 0, 1024, layout(6)).unwrap();
        assert_eq!(p.window_floor_raw(), 1026);
        assert_eq!(p.node_floor_raw(), 2019);
        assert_eq!(p.candidate(TreeLink::Position(1024)), Ok(None));
        assert_eq!(p.candidate(TreeLink::Position(1025)), Ok(Some(1025)));
        assert_eq!(p.can_follow(1025), Ok(false));
        assert_eq!(p.can_follow(2017), Ok(false));
        assert_eq!(p.can_follow(2018), Ok(true));
        assert_eq!(p.candidate(TreeLink::Empty), Ok(None));
        assert_eq!(p.candidate(TreeLink::Unsorted), Err(Error::CorruptData));
        for v in [2048, 2049, usize::MAX] {
            assert_eq!(p.candidate(TreeLink::Position(v)), Err(Error::CorruptData));
            assert_eq!(p.can_follow(v), Err(Error::CorruptData));
        }
        let prefix = TreeProbe::new(100, 110, 90, 1024, layout(6)).unwrap();
        assert_eq!(prefix.candidate(TreeLink::Position(90)), Ok(None));
        assert_eq!(prefix.candidate(TreeLink::Position(91)), Ok(Some(91)));
        let early = TreeProbe::new(1, 9, 0, 1024, layout(6)).unwrap();
        assert_eq!(early.candidate(TreeLink::Position(0)), Ok(None));
        assert_eq!(early.node_floor_raw(), 0);
    }
}
