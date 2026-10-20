//! Private discarded-pass coordinator with nine unchanged allocator identities.
//! ZSTD_initStats_ultra and ZSTD_compressBlock_btultra2, pinned zstd v1.5.7.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Private two-pass frame integration follows qualification"
)]
use crate::optimal_frequencies::{FrequencySnapshot, OptimalFrequencies};
use crate::optimal_parse::{OptimalParser, PathEvent, PathObserver, PathResult};
use crate::optimal_paths::OptimalPaths;
use crate::optimal_search::OptimalSearch;
use crate::optimal_search_layout::OptimalMatch;
use crate::optimal_state::Collect;
use crate::sequence_context::SequenceWithOffset;
use crate::ultra2_plan::{Ultra2Coordinates, Ultra2Eligibility, Ultra2ExecutionPlan, Ultra2Phase};
use crate::{Error, Result};
use alloc::vec::Vec;
use core::alloc::Allocator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Ultra2Reset {
    pub(crate) discarded: PathResult,
    pub(crate) discarded_literals: usize,
    pub(crate) original_repeats: [u32; 3],
    pub(crate) before: Ultra2Coordinates,
    pub(crate) after: Ultra2Coordinates,
    pub(crate) frequencies: FrequencySnapshot,
}
pub(crate) trait Ultra2Observer: PathObserver {
    fn entry(&mut self, _entry: Ultra2Eligibility) -> Result<()> {
        Ok(())
    }
    fn begin(
        &mut self,
        _phase: Ultra2Phase,
        _coordinates: Ultra2Coordinates,
        _frequencies: FrequencySnapshot,
    ) -> Result<()> {
        Ok(())
    }
    fn end(
        &mut self,
        _phase: Ultra2Phase,
        _coordinates: Ultra2Coordinates,
        _result: PathResult,
        _literal_count: usize,
        _frequencies: FrequencySnapshot,
    ) -> Result<()> {
        Ok(())
    }
    fn reset(&mut self, _reset: Ultra2Reset) -> Result<()> {
        Ok(())
    }
}
struct Quiet;
impl PathObserver for Quiet {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        _event: PathEvent,
        _paths: &OptimalPaths<P>,
        _frequencies: &OptimalFrequencies<F>,
        _matches: &[OptimalMatch],
    ) -> Result<()> {
        Ok(())
    }
}
impl Ultra2Observer for Quiet {}
struct ObservedCollect<'o, 's, 'v, A: Allocator, O: Ultra2Observer> {
    collect: Collect<'s, 'v, A>,
    observer: &'o mut O,
}
impl<A: Allocator, O: Ultra2Observer> PathObserver for ObservedCollect<'_, '_, '_, A, O> {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        paths: &OptimalPaths<P>,
        frequencies: &OptimalFrequencies<F>,
        matches: &[OptimalMatch],
    ) -> Result<()> {
        self.collect.event(event, paths, frequencies, matches)?;
        self.observer.event(event, paths, frequencies, matches)
    }
}
pub(crate) struct Ultra2MatchState<'a, A: Allocator> {
    source: &'a [u8],
    plan: Ultra2ExecutionPlan,
    parser: OptimalParser<'a, A, A, A>,
    search: OptimalSearch<'a, A, A, A, A>,
    literals: Vec<u8, A>,
    sequences: Vec<SequenceWithOffset, A>,
    phase: Ultra2Phase,
}
impl<'a, A: Allocator> Ultra2MatchState<'a, A> {
    pub(crate) fn from_parts(
        source: &'a [u8],
        plan: Ultra2ExecutionPlan,
        parser: OptimalParser<'a, A, A, A>,
        search: OptimalSearch<'a, A, A, A, A>,
        mut literals: Vec<u8, A>,
        mut sequences: Vec<SequenceWithOffset, A>,
    ) -> Result<Self> {
        let (original, minimum, target, disabled) = parser.ultra2_configuration()?;
        if !core::ptr::eq(original, source)
            || source.len() != plan.source_len()
            || search.layout() != plan.layout()
            || minimum
                != if plan.parameters().min_match() == 3 {
                    3
                } else {
                    4
                }
            || target != plan.parameters().target_length() as usize
            || disabled != plan.disabled_literals()
        {
            return Err(Error::InvalidParameter);
        }
        search.validate_ultra2_start(source, minimum, target)?;
        literals.clear();
        sequences.clear();
        Ok(Self {
            source,
            plan,
            parser,
            search,
            literals,
            sequences,
            phase: Ultra2Phase::Ready,
        })
    }
    pub(crate) fn parse_next(&mut self, length: usize, repeats: [u32; 3]) -> Result<bool> {
        self.parse_observed(length, repeats, &mut Quiet)
    }
    pub(crate) fn parse_observed<O: Ultra2Observer>(
        &mut self,
        length: usize,
        repeats: [u32; 3],
        observer: &mut O,
    ) -> Result<bool> {
        if self.phase != Ultra2Phase::Ready {
            return Err(Error::InvalidState);
        }
        if length != self.source.len() || repeats != [1, 4, 8] {
            return Err(Error::InvalidParameter);
        }
        let result = self.run(observer);
        if result.is_err() {
            self.phase = Ultra2Phase::Failed;
        }
        result
    }
    fn run<O: Ultra2Observer>(&mut self, observer: &mut O) -> Result<bool> {
        self.literals
            .try_reserve_exact(self.source.len())
            .map_err(|_| Error::AllocationFailed)?;
        self.sequences
            .try_reserve_exact(self.source.len() / 3)
            .map_err(|_| Error::AllocationFailed)?;
        observer.entry(self.plan.entry())?;
        if self.plan.seeds()? {
            self.phase = self.phase.advance(Ultra2Phase::Seeding)?;
            let discarded = self.pass(observer, false)?;
            let before = self.search.ultra2_coordinates()?;
            let frequencies = self.parser.ultra2_frequencies()?;
            let discarded_literals = self.literals.len();
            self.parser.rebase_ultra2(&mut self.search)?;
            let after = self.search.ultra2_coordinates()?;
            if after != before.rebase()? || self.parser.ultra2_frequencies()? != frequencies {
                return Err(Error::CorruptData);
            }
            self.literals.clear();
            self.sequences.clear();
            self.phase = self.phase.advance(Ultra2Phase::Rebased)?;
            observer.reset(Ultra2Reset {
                discarded,
                discarded_literals,
                original_repeats: [1, 4, 8],
                before,
                after,
                frequencies,
            })?;
        }
        self.phase = self.phase.advance(Ultra2Phase::Encoding)?;
        self.pass(observer, true)?;
        self.phase = self.phase.advance(Ultra2Phase::Complete)?;
        Ok(true)
    }
    fn pass<O: Ultra2Observer>(&mut self, observer: &mut O, publish: bool) -> Result<PathResult> {
        let coordinates = self.search.ultra2_coordinates()?;
        observer.begin(self.phase, coordinates, self.parser.ultra2_frequencies()?)?;
        let result;
        {
            let mut sink = ObservedCollect {
                collect: Collect::new(self.source, &mut self.literals, &mut self.sequences),
                observer: &mut *observer,
            };
            result = self.parser.parse_ultra2(&mut self.search, &mut sink)?;
            sink.collect.finish(result, false)?;
        }
        // The lifecycle observation counts only sequence literals. The final
        // encoding tail is appended afterwards and never updates frequencies.
        observer.end(
            self.phase,
            coordinates,
            result,
            self.literals.len(),
            self.parser.ultra2_frequencies()?,
        )?;
        if publish {
            let tail = self
                .source
                .get(
                    self.source
                        .len()
                        .checked_sub(result.tail)
                        .ok_or(Error::CorruptData)?..,
                )
                .ok_or(Error::CorruptData)?;
            if tail.len() > self.literals.capacity() - self.literals.len() {
                return Err(Error::CorruptData);
            }
            self.literals.extend_from_slice(tail);
        }
        Ok(result)
    }
    pub(crate) fn literals(&self) -> &[u8] {
        if self.phase == Ultra2Phase::Complete {
            &self.literals
        } else {
            &[]
        }
    }
    pub(crate) fn sequences(&self) -> &[SequenceWithOffset] {
        if self.phase == Ultra2Phase::Complete {
            &self.sequences
        } else {
            &[]
        }
    }
    pub(crate) const fn phase(&self) -> Ultra2Phase {
        self.phase
    }
    pub(crate) fn reset(&mut self) -> Result<()> {
        self.phase = Ultra2Phase::Failed;
        self.parser.reset()?;
        self.search.reset();
        self.literals.clear();
        self.sequences.clear();
        self.phase = Ultra2Phase::Ready;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        self.plan.owner_bytes()[..7].iter().sum::<usize>()
            + self.literals.capacity()
            + self.sequences.capacity() * core::mem::size_of::<SequenceWithOffset>()
    }
}

#[cfg(test)]
#[path = "../tests/support/ultra2_state.rs"]
mod tests;
