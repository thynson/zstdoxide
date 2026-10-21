//! Private bounded policy-bound Optimal/Ultra lifecycle and repeat publication.
//! Upstream: ZSTD_buildSeqStore and prevCBlock/nextCBlock publication, v1.5.7.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Retained Ultra frame admission remains separately gated"
)]
use crate::optimal_parse::{OptimalParser, PathObserver, PathResult};
use crate::optimal_price::PricePolicy;
use crate::optimal_retained_plan::RetainedOptimalPlan;
use crate::optimal_search::OptimalSearch;
use crate::{Error, Result};
use alloc::alloc::Global;
use core::alloc::Allocator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Publication {
    Raw,
    Rle,
    Compressed,
}
impl TryFrom<u8> for Publication {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Raw),
            1 => Ok(Self::Rle),
            2 => Ok(Self::Compressed),
            _ => Err(Error::InvalidParameter),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Ready,
    Pending,
    Finished,
    Poisoned,
}

/// Owns the original independently allocator-parameterized parser/search parts.
/// A fixed validated schedule keeps parsing and publication separate without
/// allocating. Observer side effects remain caller-owned on failure.
pub(crate) struct RetainedOptimal<
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
    policy: PricePolicy,
    schedule: [usize; 4],
    count: usize,
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
> RetainedOptimal<'a, F, P, M, H, N, S, B>
{
    pub(crate) fn from_parts(
        parser: OptimalParser<'a, F, P, M>,
        search: OptimalSearch<'a, H, N, S, B>,
        blocks: &[usize],
    ) -> Result<Self> {
        let (source, minimum, target, _) = parser.retained_configuration()?;
        search.validate_retained_start(source, minimum, target)?;
        Self::initialize_parts(parser, search, blocks, source.len(), PricePolicy::Optimal)
    }
    /// Bind all seven moved owners to the checked policy-2 plan before parsing.
    pub(crate) fn from_ultra_parts(
        parser: OptimalParser<'a, F, P, M>,
        search: OptimalSearch<'a, H, N, S, B>,
        plan: RetainedOptimalPlan,
    ) -> Result<Self> {
        if plan.policy() != PricePolicy::Ultra {
            return Err(Error::UnsupportedFeature);
        }
        let (source, minimum, target, disabled) = parser.ultra_retained_configuration()?;
        if source.len() != plan.source_len()
            || minimum
                != if plan.parameters().min_match() == 3 {
                    3
                } else {
                    4
                }
            || target != plan.parameters().target_length() as usize
            || disabled != plan.disabled_literals()
            || search.layout() != plan.layout()
        {
            return Err(Error::InvalidParameter);
        }
        search.validate_retained_start(source, minimum, target)?;
        Self::initialize_parts(
            parser,
            search,
            plan.blocks(),
            source.len(),
            PricePolicy::Ultra,
        )
    }
    fn initialize_parts(
        parser: OptimalParser<'a, F, P, M>,
        search: OptimalSearch<'a, H, N, S, B>,
        blocks: &[usize],
        source_len: usize,
        policy: PricePolicy,
    ) -> Result<Self> {
        if !(2..=4).contains(&blocks.len()) || blocks.iter().any(|n| !(64..=8192).contains(n)) {
            return Err(Error::InvalidParameter);
        }
        let total = blocks
            .iter()
            .try_fold(0_usize, |a, &b| a.checked_add(b))
            .ok_or(Error::InvalidParameter)?;
        if total != source_len {
            return Err(Error::InvalidParameter);
        }
        let mut schedule = [0; 4];
        schedule[..blocks.len()].copy_from_slice(blocks);
        Ok(Self {
            parser,
            search,
            policy,
            schedule,
            count: blocks.len(),
            index: 0,
            start: 0,
            owners: [[1, 4, 8], [0, 0, 0]],
            previous: 0,
            pending: None,
            phase: Phase::Ready,
        })
    }
    pub(crate) fn parse_next<O: PathObserver>(&mut self, observer: &mut O) -> Result<PathResult> {
        if self.phase != Phase::Ready {
            return Err(Error::InvalidState);
        }
        self.phase = Phase::Poisoned;
        let end = self
            .start
            .checked_add(self.schedule[self.index])
            .ok_or(Error::CorruptData)?;
        let result = match self.policy {
            PricePolicy::Optimal => self.parser.parse_retained(
                self.start,
                end,
                self.owners[self.previous],
                &mut self.search,
                observer,
            ),
            PricePolicy::Ultra => self.parser.parse_ultra_retained(
                self.start,
                end,
                self.owners[self.previous],
                &mut self.search,
                observer,
            ),
        }?;
        self.pending = Some(result);
        self.phase = Phase::Pending;
        Ok(result)
    }
    /// Publish the actual encoder decision. Candidate model/search state persists
    /// for every kind; only a compressed block replaces the committed repeats.
    pub(crate) fn publish(&mut self, kind: Publication) -> Result<()> {
        if self.phase != Phase::Pending {
            return Err(Error::InvalidState);
        }
        let candidate = self.pending.ok_or(Error::CorruptData)?;
        self.owners[1 - self.previous] = candidate.repeats;
        if kind == Publication::Compressed {
            self.previous = 1 - self.previous;
        }
        self.start += self.schedule[self.index];
        self.index += 1;
        self.pending = None;
        self.phase = if self.index == self.count {
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
#[path = "../tests/support/optimal_retained_parse.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/support/optimal_retained_wide_parse.rs"]
mod wide_tests;

#[cfg(test)]
#[path = "../tests/support/optimal_retained_frames_parse.rs"]
mod frames_tests;

#[cfg(test)]
#[path = "../tests/support/ultra_retained_execution.rs"]
mod ultra_execution_tests;

#[cfg(test)]
#[path = "../tests/support/ultra_retained_parse.rs"]
mod ultra_state_tests;
