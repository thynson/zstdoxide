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
    retained: Option<(usize, usize)>,
    ultra2: Option<bool>,
    ultra2_retained: Option<(crate::ultra2_retained_plan::RetainedUltra2Plan, bool)>,
}
impl<'a, H: Allocator, N: Allocator, S: Allocator, M: Allocator> OptimalSearch<'a, H, N, S, M> {
    pub(crate) fn from_parts(
        source: &'a [u8],
        layout: OptimalSearchLayout,
        tree: ReferenceTree<H, N>,
        short: OptimalShortKey<S>,
        matches: OptimalMatches<M>,
    ) -> Result<Self> {
        let source = OptimalSource::new(source)?;
        Self::initialize_parts(source, layout, tree, short, matches, None)
    }
    pub(crate) fn from_ultra2_parts(
        source: &'a [u8],
        layout: OptimalSearchLayout,
        tree: ReferenceTree<H, N>,
        short: OptimalShortKey<S>,
        matches: OptimalMatches<M>,
    ) -> Result<Self> {
        layout.validate_ultra2(source.len())?;
        let source = OptimalSource::for_ultra2(source)?;
        Self::initialize_parts(source, layout, tree, short, matches, Some(false))
    }
    fn initialize_parts(
        source: OptimalSource<'a>,
        layout: OptimalSearchLayout,
        mut tree: ReferenceTree<H, N>,
        mut short: OptimalShortKey<S>,
        mut matches: OptimalMatches<M>,
        ultra2: Option<bool>,
    ) -> Result<Self> {
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
            retained: None,
            ultra2,
            ultra2_retained: None,
        })
    }
    pub(crate) const fn layout(&self) -> OptimalSearchLayout {
        self.layout
    }
    /// Pair an untouched search with the exact immutable source used by a parser.
    /// Parameter rejection does not clear or otherwise modify existing owners.
    pub(crate) fn validate_fresh_start(
        &self,
        source: &[u8],
        minimum: usize,
        target: usize,
    ) -> Result<()> {
        self.layout.validate_fresh(source.len())?;
        if !self.source.is_same_source(source)
            || minimum != if self.layout.minimum() == 3 { 3 } else { 4 }
            || target != self.layout.sufficient()
        {
            return Err(Error::InvalidParameter);
        }
        if self.ultra2_retained.is_some()
            || self.ultra2.is_some()
            || self.failed
            || self.retained.is_some()
            || self.frontier != 0
            || self.short_frontier != 0
            || !self.matches.as_slice()?.is_empty()
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
    pub(crate) fn reset(&mut self) {
        self.tree.reset();
        self.short.reset();
        self.matches.clear();
        self.frontier = 0;
        self.short_frontier = 0;
        self.failed = false;
        self.retained = None;
        self.ultra2 = self.ultra2.map(|_| false);
        self.ultra2_retained = self.ultra2_retained.map(|(plan, _)| (plan, false));
        self.source.restore_end();
    }
    pub(crate) fn query(&mut self, query: OptimalQuery) -> Result<&[OptimalMatch]> {
        if self.ultra2_retained.is_some() || self.retained.is_some() || self.ultra2.is_some() {
            return Err(Error::InvalidState);
        }
        self.source.validate(query, self.layout)?;
        self.checked_query(query)
    }
    /// Ordinary fresh-block entry qualified independently from direct requests.
    pub(crate) fn query_fresh(&mut self, query: OptimalQuery) -> Result<&[OptimalMatch]> {
        if self.ultra2_retained.is_some() || self.retained.is_some() || self.ultra2.is_some() {
            return Err(Error::InvalidState);
        }
        self.source.validate_fresh(query, self.layout)?;
        self.checked_query(query)
    }
    pub(crate) fn validate_ultra2_start(
        &self,
        source: &[u8],
        minimum: usize,
        target: usize,
    ) -> Result<()> {
        self.validate_ultra2_phase(source, minimum, target, false)
    }
    pub(crate) fn validate_ultra2_binding(
        &self,
        source: &[u8],
        minimum: usize,
        target: usize,
        rebased: bool,
    ) -> Result<()> {
        self.layout.validate_ultra2(source.len())?;
        if !self.source.is_same_source(source)
            || minimum != if self.layout.minimum() == 3 { 3 } else { 4 }
            || target != self.layout.sufficient()
        {
            return Err(Error::InvalidParameter);
        }
        let origin = if rebased { source.len() } else { 0 };
        if self.ultra2 != Some(rebased)
            || self.failed
            || self.retained.is_some()
            || self.source.origin() != origin
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
    pub(crate) fn validate_ultra2_phase(
        &self,
        source: &[u8],
        minimum: usize,
        target: usize,
        rebased: bool,
    ) -> Result<()> {
        self.validate_ultra2_binding(source, minimum, target, rebased)?;
        let origin = self.source.origin();
        if self.frontier != origin
            || self.short_frontier != origin
            || !self.matches.as_slice()?.is_empty()
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
    pub(crate) fn ultra2_coordinates(&self) -> Result<crate::ultra2_plan::Ultra2Coordinates> {
        if self.ultra2.is_none() {
            return Err(Error::InvalidState);
        }
        crate::ultra2_plan::Ultra2Coordinates::new(self.source.origin(), self.source.backing_len())
    }
    /// The paired coordinator verifies a finished seeding parser before calling
    /// this boundary. All validation precedes mutation of these search owners.
    pub(crate) fn rebase_ultra2(&mut self) -> Result<()> {
        if self.ultra2 != Some(false) || self.failed || self.retained.is_some() {
            return Err(Error::InvalidState);
        }
        self.matches.as_slice()?;
        self.source.rebase_ultra2()?;
        self.frontier = self.source.origin();
        self.short_frontier = self.frontier;
        self.matches.clear();
        self.ultra2 = Some(true);
        Ok(())
    }
    /// Parser positions remain source-relative in both passes. Physical table
    /// positions are translated once here and filtered before any source read.
    pub(crate) fn query_ultra2(&mut self, mut query: OptimalQuery) -> Result<&[OptimalMatch]> {
        if self.ultra2.is_none() || self.retained.is_some() {
            return Err(Error::InvalidState);
        }
        let length = self.source.backing_len();
        if query.position == 0
            || !query
                .position
                .checked_add(8)
                .is_some_and(|end| end <= length)
            || query.length_to_beat != if self.layout.minimum() == 3 { 3 } else { 4 }
            || query
                .repeats
                .iter()
                .any(|&v| v == 0 || v as usize > length.max(8))
        {
            return Err(Error::InvalidParameter);
        }
        query.position = self
            .source
            .origin()
            .checked_add(query.position)
            .ok_or(Error::InvalidParameter)?;
        self.checked_query(query)
    }
    /// Validate the exact immutable source and retained search geometry.
    pub(crate) fn validate_retained_binding(
        &self,
        source: &[u8],
        minimum: usize,
        target: usize,
    ) -> Result<()> {
        self.layout.validate_retained(source.len())?;
        if !self.source.is_same_source(source)
            || minimum != if self.layout.minimum() == 3 { 3 } else { 4 }
            || target != self.layout.sufficient()
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
    /// Additionally require untouched owners before the first retained block.
    pub(crate) fn validate_retained_start(
        &self,
        source: &[u8],
        minimum: usize,
        target: usize,
    ) -> Result<()> {
        self.validate_retained_binding(source, minimum, target)?;
        if self.ultra2_retained.is_some()
            || self.ultra2.is_some()
            || self.failed
            || self.retained.is_some()
            || self.frontier != 0
            || self.short_frontier != 0
            || !self.matches.as_slice()?.is_empty()
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }
    /// Begin a contiguous bounded block without exposing future source bytes.
    /// ZSTD_buildSeqStore adjusts the main frontier before the parser copies it
    /// into its block-local short frontier. Argument errors precede all mutation.
    pub(crate) fn begin_retained(&mut self, start: usize, end: usize) -> Result<()> {
        if self.ultra2_retained.is_some() || self.ultra2.is_some() {
            return Err(Error::InvalidState);
        }
        self.layout.validate_retained(self.source.backing_len())?;
        if end > self.source.backing_len()
            || !end
                .checked_sub(start)
                .is_some_and(|n| (64..=8192).contains(&n))
        {
            return Err(Error::InvalidParameter);
        }
        if self.failed {
            return Err(Error::InvalidState);
        }
        match self.retained {
            Some((_, previous_end)) if previous_end == start => {}
            None if start == 0
                && self.frontier == 0
                && self.short_frontier == 0
                && self.matches.as_slice()?.is_empty() => {}
            _ => return Err(Error::InvalidState),
        }
        let frontier = if start > self.frontier + 384 {
            start - 192.min(start - self.frontier - 384)
        } else {
            self.frontier
        };
        self.source.set_end(end)?;
        self.frontier = frontier;
        self.short_frontier = frontier;
        self.matches.clear();
        self.retained = Some((start, end));
        Ok(())
    }
    pub(crate) fn query_retained(&mut self, query: OptimalQuery) -> Result<&[OptimalMatch]> {
        if self.ultra2_retained.is_some() {
            return Err(Error::InvalidState);
        }
        let (start, end) = self.retained.ok_or(Error::InvalidState)?;
        if query.position < start.max(1)
            || query.position > end - 8
            || query.length_to_beat != if self.layout.minimum() == 3 { 3 } else { 4 }
            || query
                .repeats
                .iter()
                .any(|&v| v == 0 || v as usize > self.source.backing_len())
        {
            return Err(Error::InvalidParameter);
        }
        self.checked_query(query)
    }
    /// Bind the existing four owners to one checked retained Ultra2 schedule.
    pub(crate) fn from_ultra2_retained_parts(
        source: &'a [u8],
        plan: crate::ultra2_retained_plan::RetainedUltra2Plan,
        tree: ReferenceTree<H, N>,
        short: OptimalShortKey<S>,
        matches: OptimalMatches<M>,
    ) -> Result<Self> {
        if source.len() != plan.source_len() {
            return Err(Error::InvalidParameter);
        }
        plan.layout().validate_retained_ultra2(source.len())?;
        let mut result = Self::initialize_parts(
            OptimalSource::new(source)?,
            plan.layout(),
            tree,
            short,
            matches,
            None,
        )?;
        result.ultra2_retained = Some((plan, false));
        Ok(result)
    }
    pub(crate) fn validate_ultra2_retained_binding(
        &self,
        source: &[u8],
        minimum: usize,
        target: usize,
    ) -> Result<crate::ultra2_retained_plan::RetainedUltra2Plan> {
        let (plan, _) = self.ultra2_retained.ok_or(Error::InvalidState)?;
        if !self.source.is_same_source(source)
            || source.len() != plan.source_len()
            || minimum != if self.layout.minimum() == 3 { 3 } else { 4 }
            || target != self.layout.sufficient()
        {
            return Err(Error::InvalidParameter);
        }
        if self.failed {
            return Err(Error::InvalidState);
        }
        Ok(plan)
    }
    pub(crate) fn validate_ultra2_retained_start(
        &self,
        source: &[u8],
        minimum: usize,
        target: usize,
    ) -> Result<crate::ultra2_retained_plan::RetainedUltra2Plan> {
        let plan = self.validate_ultra2_retained_binding(source, minimum, target)?;
        if self.ultra2_retained != Some((plan, false))
            || self.retained.is_some()
            || self.source.origin() != 0
            || self.frontier != 0
            || self.short_frontier != 0
            || !self.matches.as_slice()?.is_empty()
        {
            return Err(Error::InvalidState);
        }
        Ok(plan)
    }
    /// Enter a complete scheduled prefix. A seeded first block must run its
    /// discarded pass, rebase once, then run its actual pass before later blocks.
    /// All range, phase and owner checks precede changes to any search state.
    pub(crate) fn begin_ultra2_retained(
        &mut self,
        start: usize,
        end: usize,
        seeding: bool,
    ) -> Result<()> {
        let (plan, rebased) = self.ultra2_retained.ok_or(Error::InvalidState)?;
        if self.failed {
            return Err(Error::InvalidState);
        }
        let length = end.checked_sub(start).ok_or(Error::InvalidParameter)?;
        if plan.next_block_len(start)? != length || end > plan.source_len() {
            return Err(Error::InvalidParameter);
        }
        if seeding && (start != 0 || !plan.seeds_first_block() || rebased)
            || !seeding && rebased != plan.seeds_first_block()
        {
            return Err(Error::InvalidState);
        }
        let origin = if rebased { plan.first_block_len() } else { 0 };
        if self.source.origin() != origin {
            return Err(Error::InvalidState);
        }
        match self.retained {
            Some((_, previous_end)) if previous_end == start && !seeding => {}
            None if start == 0
                && self.frontier == origin
                && self.short_frontier == origin
                && self.matches.as_slice()?.is_empty() => {}
            _ => return Err(Error::InvalidState),
        }
        let current = origin.checked_add(start).ok_or(Error::InvalidParameter)?;
        let lag = current.saturating_sub(self.frontier);
        let frontier = if lag > 384 {
            current - 192.min(lag - 384)
        } else {
            self.frontier
        };
        let coordinates = plan.visible_coordinates(end, rebased)?;
        self.source.set_ultra2_retained_view(coordinates)?;
        self.frontier = frontier;
        self.short_frontier = frontier;
        self.matches.clear();
        self.retained = Some((start, end));
        Ok(())
    }
    /// The paired parser must establish a completed discarded pass. Search
    /// tables remain allocated and retain stale integer cells below the new bias.
    pub(crate) fn rebase_ultra2_retained(&mut self) -> Result<()> {
        let (plan, rebased) = self.ultra2_retained.ok_or(Error::InvalidState)?;
        if self.failed
            || rebased
            || !plan.seeds_first_block()
            || self.retained != Some((0, plan.first_block_len()))
            || self.source.origin() != 0
        {
            return Err(Error::InvalidState);
        }
        self.matches.as_slice()?;
        let coordinates = plan.visible_coordinates(plan.first_block_len(), true)?;
        self.source.set_ultra2_retained_view(coordinates)?;
        self.frontier = coordinates.origin();
        self.short_frontier = self.frontier;
        self.matches.clear();
        self.retained = None;
        self.ultra2_retained = Some((plan, true));
        Ok(())
    }
    pub(crate) fn ultra2_retained_frontier(&self) -> Result<usize> {
        if self.ultra2_retained.is_none() {
            return Err(Error::InvalidState);
        }
        Ok(self.frontier)
    }
    pub(crate) fn ultra2_retained_coordinates(
        &self,
    ) -> Result<crate::ultra2_plan::Ultra2Coordinates> {
        if self.ultra2_retained.is_none() {
            return Err(Error::InvalidState);
        }
        crate::ultra2_plan::Ultra2Coordinates::new(
            self.source.origin(),
            self.source
                .len()
                .checked_sub(self.source.origin())
                .ok_or(Error::CorruptData)?,
        )
    }
    /// The parser uses physical frame offsets; only this boundary converts them
    /// to the current logical index space. No stale table cell authorizes a read.
    pub(crate) fn query_ultra2_retained(
        &mut self,
        mut query: OptimalQuery,
    ) -> Result<&[OptimalMatch]> {
        if self.ultra2_retained.is_none() {
            return Err(Error::InvalidState);
        }
        let (start, end) = self.retained.ok_or(Error::InvalidState)?;
        if query.position < start.max(1)
            || !query.position.checked_add(8).is_some_and(|n| n <= end)
            || query.length_to_beat != if self.layout.minimum() == 3 { 3 } else { 4 }
            || query
                .repeats
                .iter()
                .any(|&v| v == 0 || v as usize > self.source.backing_len())
        {
            return Err(Error::InvalidParameter);
        }
        query.position = self
            .source
            .origin()
            .checked_add(query.position)
            .ok_or(Error::InvalidParameter)?;
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
            TreeLink::Position(p) if p < self.history_floor(target) => Ok(None),
            TreeLink::Position(p) if p < current => Ok(Some(p)),
            _ => Err(Error::CorruptData),
        }
    }
    fn history_floor(&self, position: usize) -> usize {
        self.layout.floor(position).max(self.source.origin())
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
            if previous < self.history_floor(current) {
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
                if previous >= self.history_floor(current) && current - previous < (1 << 18) {
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
mod fresh_tests {
    use super::*;
    use crate::optimal_startup_fixture as startup_fixture;
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
        let source = [b'a'; 16385];
        let q = query([1, 1, 4, 8, 0, 4]);
        for (length, w, h, c, s, m, t, short) in [
            (63, 10, 6, 6, 1, 4, 0, None),
            (16385, 16, 6, 6, 1, 4, 0, None),
            (64, 17, 6, 6, 1, 4, 0, None),
            (64, 10, 13, 6, 1, 4, 0, None),
            (64, 10, 6, 13, 1, 4, 0, None),
            (64, 10, 6, 6, 7, 4, 0, None),
            (16384, 13, 6, 6, 1, 4, 0, None),
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

#[cfg(test)]
mod retained_tests {
    use super::*;
    use crate::optimal_retained_fixture as fixture;
    use alloc::{vec, vec::Vec};
    fn layout(minimum: u8) -> OptimalSearchLayout {
        OptimalSearchLayout::new(10, 6, 6, 1, minimum, 0, (minimum == 3).then_some(10)).unwrap()
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
            zero_literals: q[4] == 1,
            length_to_beat: q[5] as usize,
        }
    }
    #[test]
    fn every_retained_query_map_and_frontier_matches_without_future_bytes() {
        let mut queries = 0;
        for case in fixture::read(fixture::PACK).unwrap() {
            let layout = OptimalSearchLayout::new(
                case.window,
                case.hash,
                case.chain,
                case.search,
                case.minimum as u8,
                case.target as usize,
                (case.minimum == 3).then_some(case.window),
            )
            .unwrap();
            let mut actual = state(case.source, layout);
            let mut expected = vec![0; case.search_cells()];
            for block in &case.blocks {
                actual
                    .begin_retained(block.start as usize, block.start as usize + block.length)
                    .unwrap();
                assert_eq!(actual.source.len(), block.start as usize + block.length);
                for (index, event) in block.events.iter().enumerate() {
                    if let Some(q) = event.query {
                        assert_eq!(
                            actual.test_snapshot(),
                            expected,
                            "{} before {index}",
                            case.name
                        );
                        let candidates: Vec<_> = actual
                            .query_retained(query(q))
                            .unwrap()
                            .iter()
                            .map(|m| [m.off_base(), m.length() as u32])
                            .collect();
                        assert_eq!(candidates, event.matches, "{} query {index}", case.name);
                        queries += 1;
                    }
                    for &(at, value) in &block.search_changes[index] {
                        expected[at] = value;
                    }
                    assert_eq!(
                        actual.test_snapshot(),
                        expected,
                        "{} block {} event {index}",
                        case.name,
                        block.start
                    );
                }
            }
        }
        assert_eq!(queries, 1523);
    }
    #[test]
    fn invalid_retained_ranges_queries_and_mode_switches_preserve_state() {
        let source = [b'a'; 128];
        let mut actual = state(&source, layout(3));
        let q = query([1, 1, 4, 8, 0, 3]);
        assert!(actual.query_retained(q).is_err());
        for (start, end) in [(0, 63), (1, 65), (0, 129), (usize::MAX, 64)] {
            assert!(actual.begin_retained(start, end).is_err());
            assert_eq!(actual.source.len(), 128);
            assert_eq!(actual.frontier, 0);
            assert!(actual.retained.is_none());
        }
        actual.begin_retained(0, 64).unwrap();
        actual.query_retained(q).unwrap();
        let before = actual.test_snapshot();
        let candidates = actual.matches.as_slice().unwrap().to_vec();
        for bad in [
            OptimalQuery { position: 0, ..q },
            OptimalQuery { position: 57, ..q },
            OptimalQuery {
                position: usize::MAX,
                ..q
            },
            OptimalQuery {
                repeats: [0, 4, 8],
                ..q
            },
            OptimalQuery {
                repeats: [129, 4, 8],
                ..q
            },
            OptimalQuery {
                length_to_beat: 4,
                ..q
            },
        ] {
            assert!(actual.query_retained(bad).is_err());
            assert_eq!(actual.test_snapshot(), before);
            assert_eq!(actual.matches.as_slice().unwrap(), candidates);
        }
        assert!(actual.query(q).is_err());
        assert!(actual.query_fresh(q).is_err());
        assert!(actual.begin_retained(0, 64).is_err());
        assert!(actual.begin_retained(65, 128).is_err());
        assert_eq!(actual.test_snapshot(), before);
        actual.begin_retained(64, 128).unwrap();
        assert!(actual.query_retained(q).is_err());
        actual.reset();
        assert_eq!(actual.source.len(), 128);
        assert!(actual.retained.is_none());
        actual.begin_retained(0, 64).unwrap();
        actual.query_retained(q).unwrap();
        assert_eq!(actual.test_snapshot(), before);
    }
    #[test]
    fn hidden_future_bytes_cannot_change_first_block_candidates_or_maps() {
        for minimum in 3..=7 {
            let first = [b'a'; 128];
            let mut changed = first;
            changed[64..].fill(b'z');
            let mut a = state(&first, layout(minimum));
            let mut b = state(&changed, layout(minimum));
            a.begin_retained(0, 64).unwrap();
            b.begin_retained(0, 64).unwrap();
            let q = query([1, 1, 4, 8, 0, if minimum == 3 { 3 } else { 4 }]);
            assert_eq!(a.query_retained(q).unwrap(), b.query_retained(q).unwrap());
            assert_eq!(a.test_snapshot(), b.test_snapshot());
            assert!(
                a.matches
                    .as_slice()
                    .unwrap()
                    .iter()
                    .all(|m| m.length() <= 63)
            );
            assert!(a.source.byte(64).is_err());
            assert!(a.source.key(63, layout(minimum).tree().key()).is_err());
            assert!(a.source.short_bucket(62, 10).is_err());
            assert_eq!(a.source.match_length(1, 0).unwrap(), 63);
        }
    }
    #[test]
    fn retained_source_identity_geometry_and_used_owner_gates_are_separate() {
        let source = [b'a'; 1025];
        let other = [b'a'; 128];
        let q = query([1, 1, 4, 8, 0, 4]);
        for size in [127, 1025] {
            let mut a = state(&source[..size], layout(4));
            assert!(a.begin_retained(0, 64).is_err());
            assert!(a.retained.is_none());
        }
        let mut a = state(&source[..128], layout(4));
        assert!(a.validate_retained_start(&other, 4, 0).is_err());
        assert!(a.validate_retained_start(&source[..128], 3, 0).is_err());
        assert!(a.validate_retained_start(&source[..128], 4, 1).is_err());
        a.validate_retained_start(&source[..128], 4, 0).unwrap();
        a.query_fresh(q).unwrap();
        assert!(a.validate_retained_start(&source[..128], 4, 0).is_err());
        assert!(a.begin_retained(0, 64).is_err());
        a.reset();
        a.begin_retained(0, 64).unwrap();
        assert!(a.validate_fresh_start(&source[..128], 4, 0).is_err());
    }
}

#[cfg(test)]
mod retained_wide_tests {
    use super::*;
    use crate::optimal_retained_wide_fixture as fixture;
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
    fn query(q: [u32; 6]) -> OptimalQuery {
        OptimalQuery {
            position: q[0] as usize,
            repeats: [q[1], q[2], q[3]],
            zero_literals: q[4] == 1,
            length_to_beat: q[5] as usize,
        }
    }
    #[test]
    fn every_wider_retained_query_map_and_frontier_matches_without_future_bytes() {
        let mut queries = 0;
        for case in fixture::read(fixture::PACK).unwrap() {
            let layout = OptimalSearchLayout::new(
                case.window,
                case.hash,
                case.chain,
                case.search,
                case.minimum as u8,
                case.target as usize,
                (case.minimum == 3).then_some(case.window),
            )
            .unwrap();
            let mut actual = state(case.source, layout);
            let mut expected = vec![0; case.search_cells()];
            for block in &case.blocks {
                actual
                    .begin_retained(block.start as usize, block.start as usize + block.length)
                    .unwrap();
                assert_eq!(actual.source.len(), block.start as usize + block.length);
                for (index, event) in block.events.iter().enumerate() {
                    if let Some(q) = event.query {
                        assert_eq!(
                            actual.test_snapshot(),
                            expected,
                            "{} before {index}",
                            case.name
                        );
                        let candidates: Vec<_> = actual
                            .query_retained(query(q))
                            .unwrap()
                            .iter()
                            .map(|m| [m.off_base(), m.length() as u32])
                            .collect();
                        assert_eq!(candidates, event.matches, "{} query {index}", case.name);
                        queries += 1;
                    }
                    for &(at, value) in &block.search_changes[index] {
                        expected[at] = value;
                    }
                    assert_eq!(
                        actual.test_snapshot(),
                        expected,
                        "{} block {} event {index}",
                        case.name,
                        block.start
                    );
                }
            }
        }
        assert_eq!(queries, 12510);
    }

    #[test]
    fn wider_retained_bounds_reject_before_mode_or_visibility_mutation() {
        let source = [b'a'; 32769];
        for (size, w, h, c, attempts, minimum, target, short) in [
            (127, 14, 12, 12, 6, 4, 0, None),
            (32769, 16, 12, 12, 6, 4, 0, None),
            (16384, 13, 12, 12, 6, 4, 0, None),
            (16384, 17, 12, 12, 6, 4, 0, None),
            (16384, 14, 13, 12, 6, 4, 0, None),
            (16384, 14, 12, 13, 6, 4, 0, None),
            (16384, 14, 12, 12, 7, 4, 0, None),
            (16384, 14, 12, 12, 6, 4, 4096, None),
            (16384, 14, 12, 12, 6, 3, 0, Some(13)),
        ] {
            let layout =
                OptimalSearchLayout::new(w, h, c, attempts, minimum, target, short).unwrap();
            let mut search = state(&source[..size], layout);
            let before = search.test_snapshot();
            assert!(search.begin_retained(0, 64).is_err());
            assert_eq!(search.test_snapshot(), before);
            assert!(search.retained.is_none());
            assert_eq!(search.source.len(), size);
        }
        let layout = OptimalSearchLayout::new(14, 12, 12, 6, 4, 4095, None).unwrap();
        let mut search = state(&source[..16384], layout);
        let before = search.test_snapshot();
        for (start, end) in [
            (0, 8193),
            (0, 63),
            (1, 8192),
            (8192, 16384),
            (0, usize::MAX),
        ] {
            assert!(search.begin_retained(start, end).is_err());
            assert_eq!(search.test_snapshot(), before);
            assert!(search.retained.is_none());
        }
        search
            .validate_fresh_start(&source[..16384], 4, 4095)
            .unwrap();
        search.begin_retained(0, 8192).unwrap();
        assert!(
            search
                .validate_fresh_start(&source[..16384], 4, 4095)
                .is_err()
        );
    }
    #[test]
    fn wider_retained_long_matches_cannot_observe_future_backing_bytes() {
        let source = [b'a'; 16384];
        let mut changed = source;
        changed[8192..].fill(b'b');
        for minimum in [3, 4] {
            let layout = OptimalSearchLayout::new(
                16,
                12,
                12,
                6,
                minimum,
                4095,
                (minimum == 3).then_some(16),
            )
            .unwrap();
            let mut a = state(&source, layout);
            let mut b = state(&changed, layout);
            a.begin_retained(0, 8192).unwrap();
            b.begin_retained(0, 8192).unwrap();
            let q = query([1, 1, 4, 8, 0, minimum.clamp(3, 4) as u32]);
            assert_eq!(a.query_retained(q).unwrap(), b.query_retained(q).unwrap());
            assert_eq!(a.matches.as_slice().unwrap().last().unwrap().length(), 8191);
            assert_eq!(a.test_snapshot(), b.test_snapshot());
            assert!(a.source.byte(8192).is_err());
            assert!(a.source.key(8191, layout.tree().key()).is_err());
            assert!(a.source.short_bucket(8190, 16).is_err());
            assert_eq!(a.source.match_length(1, 0).unwrap(), 8191);
        }
    }
}

#[cfg(test)]
mod fresh_wide_tests {
    use super::*;
    use crate::optimal_fresh_wide_fixture as startup_fixture;
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
            case.minimum as u8,
            case.target as usize,
            (case.minimum == 3).then_some(case.window),
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
            let mut state = state(case.source, layout(&case));
            let mut expected = vec![0; case.search_cells()];
            for (index, event) in case.blocks[0].events.iter().enumerate() {
                if let Some(q) = event.query {
                    assert_eq!(
                        state.test_snapshot(),
                        expected,
                        "{} before {index}",
                        case.name
                    );
                    let actual = state
                        .query_fresh(query(q))
                        .unwrap()
                        .iter()
                        .map(|m| [m.off_base(), m.length() as u32])
                        .collect::<Vec<_>>();
                    assert_eq!(actual, event.matches, "{} query {index}", case.name);
                    queries += 1;
                }
                for &(at, value) in &case.blocks[0].search_changes[index] {
                    expected[at] = value;
                }
                assert_eq!(
                    state.test_snapshot(),
                    expected,
                    "{} after {index}",
                    case.name
                );
            }
        }
        assert_eq!(queries, 30314);
    }
}

#[cfg(test)]
mod ultra_fresh_tests {
    use super::*;
    use crate::ultra_fresh_fixture as startup_fixture;
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
            case.minimum as u8,
            case.target as usize,
            (case.minimum == 3).then_some(case.window),
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
            let mut state = state(case.source, layout(&case));
            let mut expected = vec![0; case.search_cells()];
            for (index, event) in case.blocks[0].events.iter().enumerate() {
                if let Some(q) = event.query {
                    assert_eq!(
                        state.test_snapshot(),
                        expected,
                        "{} before {index}",
                        case.name
                    );
                    let actual = state
                        .query_fresh(query(q))
                        .unwrap()
                        .iter()
                        .map(|m| [m.off_base(), m.length() as u32])
                        .collect::<Vec<_>>();
                    assert_eq!(actual, event.matches, "{} query {index}", case.name);
                    queries += 1;
                }
                for &(at, value) in &case.blocks[0].search_changes[index] {
                    expected[at] = value;
                }
                assert_eq!(
                    state.test_snapshot(),
                    expected,
                    "{} after {index}",
                    case.name
                );
            }
        }
        assert_eq!(queries, 95581);
    }
}

#[cfg(test)]
#[path = "../tests/support/ultra2_search.rs"]
mod ultra2_tests;

#[cfg(test)]
#[path = "../tests/support/optimal_retained_frames_search.rs"]
mod retained_frames_tests;

#[cfg(test)]
#[path = "../tests/support/ultra_retained_search.rs"]
mod ultra_search_tests;

#[cfg(test)]
#[path = "../tests/support/ultra2_retained_search.rs"]
mod ultra2_retained_tests;
