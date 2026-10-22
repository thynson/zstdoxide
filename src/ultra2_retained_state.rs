//! Retained Ultra2 actual-pass token scratch with checked block and publication boundaries.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Retained frame integration is separately qualified"
)]
use crate::optimal_frequencies::{FrequencySnapshot, OptimalFrequencies};
use crate::optimal_parse::{OptimalParser, PathEvent, PathObserver, PathResult};
use crate::optimal_paths::OptimalPaths;
use crate::optimal_retained::Publication;
use crate::optimal_search::OptimalSearch;
use crate::optimal_search_layout::OptimalMatch;
use crate::optimal_state::Collect;
use crate::sequence_context::SequenceWithOffset;
use crate::ultra2_plan::{Ultra2Coordinates, Ultra2Phase};
use crate::ultra2_retained::RetainedUltra2;
use crate::ultra2_retained_plan::RetainedUltra2Plan;
use crate::ultra2_state::Ultra2Observer;
use crate::{Error, Result};
use alloc::vec::Vec;
use core::alloc::Allocator;

/// The same seven parser/search owners plus two scratch owners. Source is borrowed;
/// every stored position is an index and all transitions preserve allocator identity.
pub(crate) struct RetainedUltra2State<'a, A: Allocator> {
    source: &'a [u8],
    plan: RetainedUltra2Plan,
    coordinator: RetainedUltra2<'a, A, A, A, A, A, A, A>,
    literals: Vec<u8, A>,
    sequences: Vec<SequenceWithOffset, A>,
    ready_tokens: bool,
    failed: bool,
}
impl<'a, A: Allocator> RetainedUltra2State<'a, A> {
    pub(crate) fn from_parts(
        source: &'a [u8],
        plan: RetainedUltra2Plan,
        parser: OptimalParser<'a, A, A, A>,
        search: OptimalSearch<'a, A, A, A, A>,
        mut literals: Vec<u8, A>,
        mut sequences: Vec<SequenceWithOffset, A>,
    ) -> Result<Self> {
        let (original, minimum, target, disabled) = parser.ultra2_retained_configuration()?;
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
        let coordinator = RetainedUltra2::from_parts(parser, search, plan)?;
        literals.clear();
        sequences.clear();
        Ok(Self {
            source,
            plan,
            coordinator,
            literals,
            sequences,
            ready_tokens: false,
            failed: false,
        })
    }

    /// Scheduling/repeat errors are transactional. Once reservation or parsing
    /// begins, failure hides partial tokens until a complete reset succeeds.
    pub(crate) fn parse_next(&mut self, length: usize, committed: [u32; 3]) -> Result<bool> {
        if self.failed || self.ready_tokens || self.coordinator.is_finished() {
            return Err(Error::InvalidState);
        }
        let start = self.coordinator.position();
        if length != self.plan.next_block_len(start)?
            || committed != self.coordinator.committed_repeats()
        {
            return Err(Error::InvalidParameter);
        }
        let end = start
            .checked_add(length)
            .filter(|&n| n <= self.source.len())
            .ok_or(Error::CorruptData)?;
        self.failed = true;
        self.literals.clear();
        self.sequences.clear();
        self.literals
            .try_reserve_exact(self.plan.maximum_block_len())
            .map_err(|_| Error::AllocationFailed)?;
        self.sequences
            .try_reserve_exact(self.plan.maximum_block_len() / 3)
            .map_err(|_| Error::AllocationFailed)?;
        let collect = Collect::for_ultra2_retained_block(
            &self.source[..end],
            start,
            committed,
            &mut self.literals,
            &mut self.sequences,
        )?;
        let mut sink = ActualCollect {
            collect,
            seeding: true,
        };
        let result = self.coordinator.parse_next(&mut sink)?;
        sink.collect.finish(result, true)?;
        self.ready_tokens = true;
        self.failed = false;
        Ok(true)
    }

    pub(crate) fn pending_result(&self) -> Result<PathResult> {
        if self.failed || !self.ready_tokens {
            return Err(Error::InvalidState);
        }
        self.coordinator.pending_result()
    }
    /// Check the entire publication while output/entropy remain unchanged.
    pub(crate) fn validate_publication(
        &self,
        kind: Publication,
        committed_after: [u32; 3],
    ) -> Result<()> {
        let candidate = self.pending_result()?;
        let expected = if kind == Publication::Compressed {
            candidate.repeats
        } else {
            self.coordinator.committed_repeats()
        };
        if expected != committed_after {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
    /// Publish only the selected complete wire block. No allocator is touched.
    /// Rejected arguments preserve pending tokens; an internal failure poisons them.
    pub(crate) fn publish(&mut self, kind: Publication, committed_after: [u32; 3]) -> Result<()> {
        self.validate_publication(kind, committed_after)?;
        self.failed = true;
        self.coordinator.publish(kind)?;
        if self.coordinator.committed_repeats() != committed_after {
            return Err(Error::CorruptData);
        }
        self.ready_tokens = false;
        self.failed = false;
        Ok(())
    }
    pub(crate) fn literals(&self) -> &[u8] {
        if self.ready_tokens && !self.failed {
            &self.literals
        } else {
            &[]
        }
    }
    pub(crate) fn sequences(&self) -> &[SequenceWithOffset] {
        if self.ready_tokens && !self.failed {
            &self.sequences
        } else {
            &[]
        }
    }
    pub(crate) fn is_finished(&self) -> bool {
        !self.failed && self.coordinator.is_finished()
    }
    pub(crate) fn reset(&mut self) -> Result<()> {
        self.failed = true;
        self.coordinator.reset()?;
        self.literals.clear();
        self.sequences.clear();
        self.ready_tokens = false;
        self.failed = false;
        Ok(())
    }
    /// Test-only observation of completed scratch after publication; never used
    /// as parser input or exposed through the production token accessors.
    #[cfg(test)]
    pub(crate) fn audit_tokens(&self) -> Result<(&[u8], &[SequenceWithOffset])> {
        if self.failed {
            return Err(Error::InvalidState);
        }
        Ok((&self.literals, &self.sequences))
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        self.plan.owner_bytes()[..7].iter().sum::<usize>()
            + self.literals.capacity()
            + self.sequences.capacity() * core::mem::size_of::<SequenceWithOffset>()
    }
}

/// The learning pass updates parser/search statistics but never enters token
/// scratch. Both actual and discarded events still reach the coordinator's
/// independent literal counter; only encoding events reach this collector.
struct ActualCollect<'s, 'v, A: Allocator> {
    collect: Collect<'s, 'v, A>,
    seeding: bool,
}
impl<A: Allocator> PathObserver for ActualCollect<'_, '_, A> {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        paths: &OptimalPaths<P>,
        frequencies: &OptimalFrequencies<F>,
        matches: &[OptimalMatch],
    ) -> Result<()> {
        if self.seeding {
            Ok(())
        } else {
            self.collect.event(event, paths, frequencies, matches)
        }
    }
}
impl<A: Allocator> Ultra2Observer for ActualCollect<'_, '_, A> {
    fn begin(
        &mut self,
        phase: Ultra2Phase,
        _: Ultra2Coordinates,
        _: FrequencySnapshot,
    ) -> Result<()> {
        self.seeding = phase == Ultra2Phase::Seeding;
        Ok(())
    }
}
#[cfg(test)]
#[path = "../tests/support/ultra2_retained_tokens.rs"]
mod tests;
