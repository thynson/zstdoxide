//! Private no-dictionary sorted Optimal match enumeration over immutable source.
//! Observable rules correspond to facebook/zstd v1.5.7 zstd_opt.c,
//! ZSTD_insertBt1, ZSTD_insertBtAndGetAllMatches and ZSTD_BtGetAllMatches.
//! Reference revision f8745da6ff1ad1e7bab384bd1f9d742439278e99 (LICENSE.zstd).
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Private Optimal path parsing follows search qualification"
)]
use crate::optimal_matches::OptimalMatches;
use crate::optimal_search_layout::{
    OptimalMatch, OptimalQuery, OptimalSearchLayout, OptimalSource,
};
use crate::optimal_short_key::OptimalShortKey;
use crate::reference_tree::ReferenceTree;
use crate::tree_layout::{TreeEdge, TreeLink, TreeSide};
use crate::{Error, Result};
use alloc::alloc::Global;
use core::alloc::Allocator;

pub(crate) struct OptimalSearch<
    'a,
    H: Allocator = Global,
    N: Allocator = H,
    S: Allocator = H,
    M: Allocator = H,
> {
    source: OptimalSource<'a>,
    layout: OptimalSearchLayout,
    tree: ReferenceTree<H, N>,
    short: OptimalShortKey<S>,
    matches: OptimalMatches<M>,
    frontier: usize,
    short_frontier: usize,
    failed: bool,
}
impl<'a, H: Allocator, N: Allocator, S: Allocator, M: Allocator> OptimalSearch<'a, H, N, S, M> {
    pub(crate) fn from_parts(
        source: &'a [u8],
        layout: OptimalSearchLayout,
        mut tree: ReferenceTree<H, N>,
        mut short: OptimalShortKey<S>,
        mut matches: OptimalMatches<M>,
    ) -> Result<Self> {
        let source = OptimalSource::new(source)?;
        if tree.layout() != layout.tree() || short.log() != layout.short_log() {
            return Err(Error::InvalidParameter);
        }
        tree.reset();
        short.reset();
        matches.clear();
        Ok(Self {
            source,
            layout,
            tree,
            short,
            matches,
            frontier: 0,
            short_frontier: 0,
            failed: false,
        })
    }
    pub(crate) fn reset(&mut self) {
        self.tree.reset();
        self.short.reset();
        self.matches.clear();
        self.frontier = 0;
        self.short_frontier = 0;
        self.failed = false;
    }
    pub(crate) fn query(&mut self, query: OptimalQuery) -> Result<&[OptimalMatch]> {
        self.source.validate(query, self.layout)?;
        self.checked_query(query)
    }
    /// Ordinary fresh-block entry qualified independently from direct requests.
    pub(crate) fn query_fresh(&mut self, query: OptimalQuery) -> Result<&[OptimalMatch]> {
        self.source.validate_fresh(query, self.layout)?;
        self.checked_query(query)
    }
    fn checked_query(&mut self, query: OptimalQuery) -> Result<&[OptimalMatch]> {
        if self.failed {
            return Err(Error::CorruptData);
        }
        if let Err(error) = self.run_query(query) {
            self.failed = true;
            return Err(error);
        }
        self.matches.as_slice()
    }
    #[cfg(test)]
    pub(crate) fn test_snapshot(&self) -> alloc::vec::Vec<u32> {
        let (heads, nodes) = self.tree.test_maps();
        let mut result = alloc::vec::Vec::new();
        for &value in heads.iter().chain(nodes).chain(self.short.test_cells()) {
            result.push(if value == usize::MAX {
                0
            } else {
                u32::try_from(value + 2).unwrap()
            });
        }
        result.extend([(self.frontier + 2) as u32, (self.short_frontier + 2) as u32]);
        result
    }
    fn add(&mut self, intent: u32, length: usize) -> Result<()> {
        self.matches.push(OptimalMatch::new(intent, length)?)
    }
    fn direct(&mut self, current: usize, previous: usize, length: usize) -> Result<()> {
        let intent = current
            .checked_sub(previous)
            .and_then(|d| d.checked_add(3))
            .and_then(|v| u32::try_from(v).ok())
            .ok_or(Error::CorruptData)?;
        self.add(intent, length)
    }
    fn eligible(&self, link: TreeLink, current: usize, target: usize) -> Result<Option<usize>> {
        match link {
            TreeLink::Empty => Ok(None),
            TreeLink::Position(p) if p < self.layout.floor(target) => Ok(None),
            TreeLink::Position(p) if p < current => Ok(Some(p)),
            _ => Err(Error::CorruptData),
        }
    }
    /// Insert either a catch-up node or a queried node. Pending links are checked
    /// value destinations; expired links become Discard rather than raw pointers.
    fn insert(
        &mut self,
        current: usize,
        target: usize,
        mut best: usize,
        emit: bool,
    ) -> Result<(usize, usize)> {
        let key = self.source.key(current, self.layout.tree().key())?;
        let mut candidate = self.tree.replace_head(key, current)?;
        let layout = self.layout.tree();
        let mask = layout.nodes() - 1;
        let expired_native = (current + 2).saturating_sub(mask);
        let mut smaller = layout.edge(current, TreeSide::Smaller)?;
        let mut larger = layout.edge(current, TreeSide::Larger)?;
        let mut farthest = current + 9;
        for _ in 0..self.layout.attempts() {
            let Some(previous) = self.eligible(candidate, current, target)? else {
                break;
            };
            let length = self.source.match_length(current, previous)?;
            let improved = length > best;
            if improved {
                best = length;
                farthest = farthest.max(previous + length);
                if emit {
                    self.direct(current, previous, length)?;
                }
            }
            let at_end = current + length == self.source.len();
            if (!emit && at_end) || (emit && improved && (length > 4096 || at_end)) {
                break;
            }
            if at_end {
                return Err(Error::CorruptData);
            }
            if self.source.byte(previous + length)? < self.source.byte(current + length)? {
                self.tree.set_edge(smaller, TreeLink::Position(previous))?;
                if previous + 2 <= expired_native {
                    smaller = TreeEdge::Discard;
                    break;
                }
                smaller = layout.edge(previous, TreeSide::Larger)?;
                candidate = self.tree.edge(smaller)?;
            } else {
                self.tree.set_edge(larger, TreeLink::Position(previous))?;
                if previous + 2 <= expired_native {
                    larger = TreeEdge::Discard;
                    break;
                }
                larger = layout.edge(previous, TreeSide::Smaller)?;
                candidate = self.tree.edge(larger)?;
            }
        }
        self.tree.set_edge(smaller, TreeLink::Empty)?;
        self.tree.set_edge(larger, TreeLink::Empty)?;
        Ok((best, farthest))
    }
    fn run_query(&mut self, q: OptimalQuery) -> Result<()> {
        self.matches.clear();
        let current = q.position;
        if current < self.frontier {
            return Ok(());
        }
        let mut position = self.frontier;
        while position < current {
            let (best, end) = self.insert(position, current, 8, false)?;
            let skip = best
                .saturating_sub(384)
                .min(192)
                .max(end.checked_sub(position + 8).ok_or(Error::CorruptData)?);
            if skip == 0 {
                return Err(Error::CorruptData);
            }
            position = position.checked_add(skip).ok_or(Error::CorruptData)?;
        }
        self.frontier = current;
        let mut best = q.length_to_beat - 1;
        let sufficient = self.layout.sufficient();
        let repeats = if q.zero_literals {
            [q.repeats[1], q.repeats[2], q.repeats[0].wrapping_sub(1)]
        } else {
            q.repeats
        };
        for (i, distance) in repeats.into_iter().enumerate() {
            if distance == 0 {
                continue;
            }
            let Some(previous) = current.checked_sub(distance as usize) else {
                continue;
            };
            if previous < self.layout.floor(current) {
                continue;
            }
            let length = self.source.match_length(current, previous)?;
            if length > best && length >= if self.layout.minimum() == 3 { 3 } else { 4 } {
                best = length;
                self.add(i as u32 + 1, length)?;
                if length > sufficient || current + length == self.source.len() {
                    return Ok(());
                }
            }
        }
        if self.layout.minimum() == 3 && best < 3 {
            let log = self.layout.short_log().ok_or(Error::CorruptData)?;
            for position in self.short_frontier..current {
                self.short
                    .replace(self.source.short_bucket(position, log)?, position)?;
            }
            self.short_frontier = current;
            if let Some(previous) = self.short.get(self.source.short_bucket(current, log)?)? {
                if previous >= current {
                    return Err(Error::CorruptData);
                }
                if previous >= self.layout.floor(current) && current - previous < (1 << 18) {
                    let length = self.source.match_length(current, previous)?;
                    if length >= 3 {
                        best = length;
                        self.direct(current, previous, length)?;
                        if length > sufficient || current + length == self.source.len() {
                            self.frontier = current + 1;
                            return Ok(());
                        }
                    }
                }
            }
        }
        let (_, end) = self.insert(current, current, best, true)?;
        self.frontier = end.checked_sub(8).ok_or(Error::CorruptData)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/support/optimal_match_broad_fixture.rs"]
mod broad_fixture;
#[cfg(test)]
#[path = "../tests/support/optimal_match_fixture.rs"]
mod tiny_fixture;
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    fn layout(g: [u32; 7]) -> OptimalSearchLayout {
        OptimalSearchLayout::new(
            g[0] as u8,
            g[1] as u8,
            g[2] as u8,
            g[3] as u8,
            g[4] as u8,
            g[5] as usize,
            (g[6] != 0).then_some(g[6] as u8),
        )
        .unwrap()
    }
    fn state(source: &[u8], layout: OptimalSearchLayout) -> OptimalSearch<'_> {
        OptimalSearch::from_parts(
            source,
            layout,
            ReferenceTree::new_in(layout.tree(), Global, Global).unwrap(),
            OptimalShortKey::new_in(layout.short_log(), Global).unwrap(),
            OptimalMatches::new_in(Global).unwrap(),
        )
        .unwrap()
    }
    fn query(q: [u32; 6]) -> OptimalQuery {
        OptimalQuery {
            position: q[0] as usize,
            repeats: [q[1], q[2], q[3]],
            zero_literals: q[4] != 0,
            length_to_beat: q[5] as usize,
        }
    }
    fn native(values: &[usize]) -> Vec<u32> {
        values
            .iter()
            .map(|&v| {
                if v == usize::MAX {
                    0
                } else {
                    u32::try_from(v + 2).unwrap()
                }
            })
            .collect()
    }
    type Snapshot = (u32, u32, Vec<u32>, Vec<u32>, Vec<u32>);
    fn snapshot(s: &OptimalSearch<'_>) -> Snapshot {
        let (h, n) = s.tree.test_maps();
        (
            (s.frontier + 2) as u32,
            (s.short_frontier + 2) as u32,
            native(h),
            native(n),
            native(s.short.test_cells()),
        )
    }
    fn check_tiny(s: &OptimalSearch<'_>, want: &tiny_fixture::Snapshot, name: &str) {
        let (f, sf, h, n, sh) = snapshot(s);
        assert_eq!(f, want.frontier, "{name} main frontier");
        assert_eq!(sf, want.short_frontier, "{name} short frontier");
        assert_eq!(h, want.heads, "{name} heads");
        assert_eq!(n, want.nodes, "{name} edges");
        assert_eq!(sh, want.short_heads, "{name} short heads");
    }
    fn check_broad(s: &OptimalSearch<'_>, want: &broad_fixture::Snapshot, name: &str) {
        let (f, sf, h, n, sh) = snapshot(s);
        assert_eq!(f, want.frontier, "{name} main frontier");
        assert_eq!(sf, want.short_frontier, "{name} short frontier");
        assert!(h.iter().copied().eq(want.heads.full()), "{name} heads");
        assert!(n.iter().copied().eq(want.nodes.full()), "{name} edges");
        assert!(
            sh.iter().copied().eq(want.short_heads.full()),
            "{name} short heads"
        );
    }
    #[test]
    fn every_tiny_c_map_frontier_and_candidate() {
        let mut count = 0;
        for case in tiny_fixture::read(tiny_fixture::PACK).unwrap() {
            let mut s = state(case.source, layout(case.geometry));
            for o in case.observations {
                check_tiny(&s, &o.before, case.name);
                let actual = s
                    .query(query(o.query))
                    .unwrap_or_else(|e| panic!("{} query {:?}: {e:?}", case.name, o.query))
                    .iter()
                    .map(|m| [m.off_base(), m.length() as u32])
                    .collect::<Vec<_>>();
                assert_eq!(actual, o.matches, "{} query {:?}", case.name, o.query);
                check_tiny(&s, &o.after, case.name);
                count += 1;
            }
        }
        assert_eq!(count, 273);
    }
    #[test]
    fn every_broader_c_map_frontier_and_candidate() {
        let mut count = 0;
        for case in broad_fixture::read(broad_fixture::PACK).unwrap() {
            let mut s = state(case.source, layout(case.geometry));
            for o in case.observations {
                check_broad(&s, &o.before, case.name);
                let actual = s
                    .query(query(o.query))
                    .unwrap_or_else(|e| panic!("{} query {:?}: {e:?}", case.name, o.query))
                    .iter()
                    .map(|m| [m.off_base(), m.length() as u32])
                    .collect::<Vec<_>>();
                assert_eq!(actual, o.matches, "{} query {:?}", case.name, o.query);
                check_broad(&s, &o.after, case.name);
                count += 1;
            }
        }
        assert_eq!(count, 82);
    }
    #[test]
    fn invalid_requests_preserve_owners_and_live_candidates() {
        let bytes = b"abcdabcdabcdabcdabcdabcdabcdabcd";
        let l = layout([10, 6, 6, 1, 4, 0, 0]);
        let mut s = state(bytes, l);
        let q = query([8, 1, 4, 8, 0, 4]);
        s.query(q).unwrap();
        let before = snapshot(&s);
        let matches = s.matches.as_slice().unwrap().to_vec();
        for q in [
            OptimalQuery {
                position: usize::MAX,
                ..q
            },
            OptimalQuery {
                length_to_beat: 3,
                ..q
            },
            OptimalQuery {
                repeats: [4097, 1, 1],
                ..q
            },
        ] {
            assert!(s.query(q).is_err());
            assert_eq!(snapshot(&s), before);
            assert_eq!(s.matches.as_slice().unwrap(), matches);
            assert!(!s.failed);
        }
        s.reset();
        assert_eq!(s.frontier, 0);
        assert_eq!(s.short_frontier, 0);
        assert!(s.matches.as_slice().unwrap().is_empty());
        s.query(q).unwrap();
        assert_eq!(snapshot(&s), before);
        assert_eq!(s.matches.as_slice().unwrap(), matches);
    }
    #[test]
    fn corrupted_tree_poison_requires_reset() {
        let bytes = b"abcdabcdabcdabcdabcdabcdabcdabcd";
        let l = layout([10, 6, 6, 1, 4, 0, 0]);
        let mut s = state(bytes, l);
        let key = s.source.key(0, l.tree().key()).unwrap();
        s.tree.replace_head(key, 9).unwrap();
        assert!(s.query(query([8, 1, 4, 8, 0, 4])).is_err());
        assert!(s.failed);
        assert!(s.query(query([0, 1, 1, 1, 0, 4])).is_err());
        s.reset();
        assert!(!s.failed);
        s.query(query([8, 1, 4, 8, 0, 4])).unwrap();
    }
    #[test]
    fn mismatched_parts_reject_and_empty_query_is_legal() {
        let l = layout([10, 6, 6, 1, 3, 0, 6]);
        let bytes = b"abcdefgh";
        assert!(
            OptimalSearch::from_parts(
                bytes,
                l,
                ReferenceTree::new_in(l.tree(), Global, Global).unwrap(),
                OptimalShortKey::new_in(None, Global).unwrap(),
                OptimalMatches::new_in(Global).unwrap()
            )
            .is_err()
        );
        let mut s = state(bytes, l);
        assert!(s.query(query([0, 1, 1, 1, 0, 3])).unwrap().is_empty());
        assert_eq!(s.frontier, 1);
    }
    #[test]
    fn composite_allocation_failures_release_exact_moved_owners() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        fn construct<A: Allocator>(
            source: &[u8],
            l: OptimalSearchLayout,
            owners: [A; 4],
        ) -> Result<OptimalSearch<'_, A>> {
            let [h, n, s, m] = owners;
            let tree = ReferenceTree::new_in(l.tree(), h, n)?;
            let short = OptimalShortKey::new_in(l.short_log(), s)?;
            let matches = OptimalMatches::new_in(m)?;
            OptimalSearch::from_parts(source, l, tree, short, matches)
        }
        for reject in 0..=4 {
            let owners = core::array::from_fn(|_| {
                TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone()
            });
            let monitors = owners.each_ref().map(|a| a.monitor());
            if reject > 0 {
                assert!(monitors[reject - 1].fail_next());
            }
            let result = construct(b"abcdefghabcdefgh", layout([10, 6, 6, 1, 3, 0, 6]), owners);
            if reject > 0 {
                assert!(matches!(result, Err(Error::AllocationFailed)));
            } else {
                drop(result.unwrap());
            }
            for (i, m) in monitors.into_iter().enumerate() {
                m.assert_released();
                assert_eq!(m.snapshot().owners_dropped, 1);
                assert_eq!(m.snapshot().attempts, u64::from(reject == 0 || i < reject));
            }
        }
    }
    #[test]
    fn borrowed_composite_reuses_every_owner_without_allocation() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let a = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = a.monitor();
        let l = layout([10, 6, 6, 2, 3, 4096, 6]);
        let mut s = OptimalSearch::from_parts(
            b"abcXabcYabcZabcXabcYabcZ",
            l,
            ReferenceTree::new_in(l.tree(), &a, &a).unwrap(),
            OptimalShortKey::new_in(l.short_log(), &a).unwrap(),
            OptimalMatches::new_in(&a).unwrap(),
        )
        .unwrap();
        assert_eq!(monitor.snapshot().attempts, 4);
        assert!(monitor.fail_next());
        for _ in 0..3 {
            s.reset();
            for q in [0, 4, 8, 12] {
                s.query(query([q, 1, 1, 1, 0, 3])).unwrap();
            }
        }
        assert_eq!(monitor.snapshot().attempts, 4);
        drop(s);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(a);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
    #[test]
    fn zero_repeats_and_wrapped_rep0_minus_one_are_ineligible() {
        let l = layout([10, 6, 6, 1, 3, 0, 6]);
        let mut s = state(b"abcdefghabcdefgh", l);
        for ll0 in [0, 1] {
            s.reset();
            assert!(s.query(query([0, 0, 0, 0, ll0, 3])).unwrap().is_empty());
            assert_eq!(s.frontier, 1);
        }
        assert_eq!(
            s.eligible(TreeLink::Unsorted, 4, 4),
            Err(Error::CorruptData)
        );
        assert_eq!(
            s.eligible(TreeLink::Position(4), 4, 4),
            Err(Error::CorruptData)
        );
    }
}

#[cfg(test)]
#[path = "../tests/support/optimal_startup_fixture.rs"]
mod startup_fixture;
#[cfg(test)]
mod fresh_tests {
    use super::*;
    use alloc::{vec, vec::Vec};
    fn state(source: &[u8], layout: OptimalSearchLayout) -> OptimalSearch<'_> {
        OptimalSearch::from_parts(
            source,
            layout,
            ReferenceTree::new_in(layout.tree(), Global, Global).unwrap(),
            OptimalShortKey::new_in(layout.short_log(), Global).unwrap(),
            OptimalMatches::new_in(Global).unwrap(),
        )
        .unwrap()
    }
    fn layout(case: &startup_fixture::Case<'_>) -> OptimalSearchLayout {
        OptimalSearchLayout::new(
            case.window,
            case.hash,
            case.chain,
            case.search,
            case.path.minimum as u8,
            case.path.target as usize,
            (case.path.minimum == 3).then_some(case.window),
        )
        .unwrap()
    }
    fn query(q: [u32; 6]) -> OptimalQuery {
        OptimalQuery {
            position: q[0] as usize,
            repeats: [q[1], q[2], q[3]],
            zero_literals: q[4] == 1,
            length_to_beat: q[5] as usize,
        }
    }
    #[test]
    fn every_fresh_c_query_candidate_map_and_frontier_matches() {
        let mut queries = 0;
        for case in startup_fixture::read(startup_fixture::PACK).unwrap() {
            let mut state = state(case.path.source, layout(&case));
            let mut expected = vec![0; case.search_cells()];
            for (index, event) in case.path.events.iter().enumerate() {
                if let Some(q) = event.query {
                    assert_eq!(
                        state.test_snapshot(),
                        expected,
                        "{} before {index}",
                        case.path.name
                    );
                    let actual = state
                        .query_fresh(query(q))
                        .unwrap()
                        .iter()
                        .map(|m| [m.off_base(), m.length() as u32])
                        .collect::<Vec<_>>();
                    assert_eq!(actual, event.matches, "{} query {index}", case.path.name);
                    queries += 1;
                }
                for &(at, value) in &case.search_changes[index] {
                    expected[at] = value;
                }
                assert_eq!(
                    state.test_snapshot(),
                    expected,
                    "{} after {index}",
                    case.path.name
                );
            }
        }
        assert_eq!(queries, 1406);
    }
    #[test]
    fn direct_repeat_guards_remain_and_invalid_fresh_queries_preserve_state() {
        let source = [b'a'; 64];
        let layout = OptimalSearchLayout::new(10, 6, 6, 1, 4, 0, None).unwrap();
        let mut state = state(&source, layout);
        let q = query([1, 1, 4, 8, 0, 4]);
        assert!(state.query(q).is_err());
        state.query_fresh(q).unwrap();
        let before = state.test_snapshot();
        let matches = state.matches.as_slice().unwrap().to_vec();
        for bad in [
            OptimalQuery { position: 0, ..q },
            OptimalQuery {
                position: usize::MAX,
                ..q
            },
            OptimalQuery {
                repeats: [0, 4, 8],
                ..q
            },
            OptimalQuery {
                repeats: [65, 4, 8],
                ..q
            },
            OptimalQuery {
                length_to_beat: 5,
                ..q
            },
        ] {
            assert!(state.query_fresh(bad).is_err());
            assert_eq!(state.test_snapshot(), before);
            assert_eq!(state.matches.as_slice().unwrap(), matches);
            assert!(!state.failed);
        }
        state.reset();
        state.query_fresh(q).unwrap();
        assert_eq!(state.test_snapshot(), before);
    }
    #[test]
    fn fresh_entry_rejects_unqualified_source_and_geometry_before_mutation() {
        let source = [b'a'; 513];
        let q = query([1, 1, 4, 8, 0, 4]);
        for (length, w, h, c, s, m, t, short) in [
            (63, 10, 6, 6, 1, 4, 0, None),
            (513, 10, 6, 6, 1, 4, 0, None),
            (64, 13, 6, 6, 1, 4, 0, None),
            (64, 10, 10, 6, 1, 4, 0, None),
            (64, 10, 6, 10, 1, 4, 0, None),
            (64, 10, 6, 6, 5, 4, 0, None),
            (64, 10, 6, 6, 1, 4, 4096, None),
            (64, 10, 6, 6, 1, 3, 0, Some(6)),
        ] {
            let layout = OptimalSearchLayout::new(w, h, c, s, m, t, short).unwrap();
            let mut state = state(&source[..length], layout);
            let before = state.test_snapshot();
            assert!(state.query_fresh(q).is_err());
            assert_eq!(state.test_snapshot(), before);
            assert!(!state.failed);
        }
    }
}
