//! Bounded retained Ultra2 bootstrap, phase execution and repeat publication.
//! Pinned upstream: ZSTD_buildSeqStore, ZSTD_initStats_ultra, btultra2, v1.5.7.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Retained Ultra2 frame publication remains separately gated"
)]
use crate::optimal_frequencies::{FrequencySnapshot, OptimalFrequencies};
use crate::optimal_parse::{OptimalParser, PathEvent, PathObserver, PathResult};
use crate::optimal_paths::OptimalPaths;
use crate::optimal_retained::Publication;
use crate::optimal_search::OptimalSearch;
use crate::optimal_search_layout::OptimalMatch;
use crate::ultra2_plan::{Ultra2Eligibility, Ultra2Phase};
use crate::ultra2_retained_plan::RetainedUltra2Plan;
use crate::ultra2_state::{Ultra2Observer, Ultra2Reset};
use crate::{Error, Result};
use alloc::alloc::Global;
use core::alloc::Allocator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Ready,
    Pending,
    Finished,
    Poisoned,
}
struct Count<'a, O: Ultra2Observer> {
    observer: &'a mut O,
    literals: usize,
}
impl<O: Ultra2Observer> PathObserver for Count<'_, O> {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        paths: &OptimalPaths<P>,
        frequencies: &OptimalFrequencies<F>,
        matches: &[OptimalMatch],
    ) -> Result<()> {
        if let Some(row) = event.appended {
            self.literals = self
                .literals
                .checked_add(row[1] as usize)
                .ok_or(Error::CorruptData)?;
        }
        self.observer.event(event, paths, frequencies, matches)
    }
}
/// Seven independently owned workspaces; copied schedule and wire repeat owners
/// allocate nothing. Discarded bootstrap results never enter pending publication.
pub(crate) struct RetainedUltra2<
    'a,
    F: Allocator = Global,
    P: Allocator = F,
    M: Allocator = F,
    H: Allocator = F,
    N: Allocator = F,
    S: Allocator = F,
    B: Allocator = F,
> {
    parser: OptimalParser<'a, F, P, M>,
    search: OptimalSearch<'a, H, N, S, B>,
    plan: RetainedUltra2Plan,
    index: usize,
    start: usize,
    owners: [[u32; 3]; 2],
    previous: usize,
    pending: Option<PathResult>,
    phase: Phase,
}
impl<
    'a,
    F: Allocator,
    P: Allocator,
    M: Allocator,
    H: Allocator,
    N: Allocator,
    S: Allocator,
    B: Allocator,
> RetainedUltra2<'a, F, P, M, H, N, S, B>
{
    pub(crate) fn from_parts(
        parser: OptimalParser<'a, F, P, M>,
        search: OptimalSearch<'a, H, N, S, B>,
        plan: RetainedUltra2Plan,
    ) -> Result<Self> {
        let (source, minimum, target, disabled) = parser.ultra2_retained_configuration()?;
        if source.len() != plan.source_len()
            || disabled != plan.disabled_literals()
            || minimum
                != if plan.parameters().min_match() == 3 {
                    3
                } else {
                    4
                }
            || target != plan.parameters().target_length() as usize
            || search.validate_ultra2_retained_start(source, minimum, target)? != plan
        {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            parser,
            search,
            plan,
            index: 0,
            start: 0,
            owners: [[1, 4, 8], [0, 0, 0]],
            previous: 0,
            pending: None,
            phase: Phase::Ready,
        })
    }
    pub(crate) fn parse_next<O: Ultra2Observer>(&mut self, observer: &mut O) -> Result<PathResult> {
        if self.phase != Phase::Ready {
            return Err(Error::InvalidState);
        }
        self.phase = Phase::Poisoned;
        let end = self
            .start
            .checked_add(self.plan.next_block_len(self.start)?)
            .ok_or(Error::CorruptData)?;
        let origin = self.search.ultra2_retained_coordinates()?.origin();
        let current = origin.checked_add(self.start).ok_or(Error::CorruptData)?;
        let frontier = self.search.ultra2_retained_frontier()?;
        let lag = current.saturating_sub(frontier);
        let next_to_update = if lag > 384 {
            current - 192.min(lag - 384)
        } else {
            frontier
        };
        let entry = Ultra2Eligibility {
            literal_length_sum: self.frequencies()?.sums[1],
            sequence_count: 0,
            dictionary_limit: origin,
            low_limit: origin,
            current,
            source_len: end - self.start,
            next_to_update,
        };
        let seed = entry.eligible()?;
        if seed != (self.index == 0 && self.plan.seeds_first_block()) {
            return Err(Error::CorruptData);
        }
        observer.entry(entry)?;
        if seed {
            let (discarded, discarded_literals) = self.pass(end, true, observer)?;
            let before = self.search.ultra2_retained_coordinates()?;
            let frequencies = self.frequencies()?;
            self.parser.rebase_ultra2_retained(&mut self.search)?;
            let after = self.search.ultra2_retained_coordinates()?;
            if after != self.plan.visible_coordinates(end, true)?
                || self.frequencies()? != frequencies
            {
                return Err(Error::CorruptData);
            }
            observer.reset(Ultra2Reset {
                discarded,
                discarded_literals,
                original_repeats: self.owners[self.previous],
                before,
                after,
                frequencies,
            })?;
        }
        let (result, _) = self.pass(end, false, observer)?;
        self.pending = Some(result);
        self.phase = Phase::Pending;
        Ok(result)
    }
    fn pass<O: Ultra2Observer>(
        &mut self,
        end: usize,
        seeding: bool,
        observer: &mut O,
    ) -> Result<(PathResult, usize)> {
        let phase = if seeding {
            Ultra2Phase::Seeding
        } else {
            Ultra2Phase::Encoding
        };
        let coordinates = self
            .plan
            .visible_coordinates(end, !seeding && self.plan.seeds_first_block())?;
        observer.begin(phase, coordinates, self.frequencies()?)?;
        let mut counted = Count {
            observer,
            literals: 0,
        };
        let result = self.parser.parse_ultra2_retained(
            self.start,
            end,
            seeding,
            self.owners[self.previous],
            &mut self.search,
            &mut counted,
        )?;
        let literals = counted.literals;
        counted
            .observer
            .end(phase, coordinates, result, literals, self.frequencies()?)?;
        Ok((result, literals))
    }
    /// Only actual encoding results can be published; all kinds keep search and
    /// learned prices, while compressed publication alone swaps wire repeats.
    pub(crate) fn publish(&mut self, kind: Publication) -> Result<()> {
        if self.phase != Phase::Pending {
            return Err(Error::InvalidState);
        }
        let pending = self.pending.ok_or(Error::CorruptData)?;
        self.owners[1 - self.previous] = pending.repeats;
        if kind == Publication::Compressed {
            self.previous = 1 - self.previous;
        }
        self.start += self.plan.blocks()[self.index];
        self.index += 1;
        self.pending = None;
        self.phase = if self.index == self.plan.blocks().len() {
            Phase::Finished
        } else {
            Phase::Ready
        };
        Ok(())
    }
    pub(crate) fn reset(&mut self) -> Result<()> {
        self.phase = Phase::Poisoned;
        self.parser.reset()?;
        self.search.reset();
        self.index = 0;
        self.start = 0;
        self.owners = [[1, 4, 8], [0, 0, 0]];
        self.previous = 0;
        self.pending = None;
        self.phase = Phase::Ready;
        Ok(())
    }
    pub(crate) fn frequencies(&self) -> Result<FrequencySnapshot> {
        self.parser.ultra2_retained_frequencies()
    }
    pub(crate) fn position(&self) -> usize {
        self.start
    }
    pub(crate) fn committed_repeats(&self) -> [u32; 3] {
        self.owners[self.previous]
    }
    pub(crate) fn pending_result(&self) -> Result<PathResult> {
        if self.phase != Phase::Pending {
            return Err(Error::InvalidState);
        }
        self.pending.ok_or(Error::CorruptData)
    }
    pub(crate) fn is_finished(&self) -> bool {
        self.phase == Phase::Finished
    }
}

#[cfg(test)]
#[path = "../tests/support/ultra2_retained_execution.rs"]
mod tests;
