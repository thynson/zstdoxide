//! Static dispatch over admitted reference search plans and complete owners.
#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::double_fast_plan::DoubleFastExecutionPlan;
use crate::double_fast_state::DoubleFastMatchState;
use crate::double_fast_tables::DoubleFastTables;
use crate::fast_plan::FastExecutionPlan;
use crate::fast_state::FastMatchState;
use crate::matchfinder::MatchTable;
use crate::sequence_context::SequenceWithOffset;
use crate::{
    CompressionParameterRequest, Error, ReferenceStrategy, ResolvedCompressionParameters, Result,
    SourceSize,
};

/// Each variant contains an admitted algorithm plan. Resolve the known input
/// once before dispatch; inactive algorithms allocate no table or parser storage.
#[derive(Clone, Copy)]
pub(crate) enum ReferenceSearchPlan {
    Fast(FastExecutionPlan),
    DoubleFast(DoubleFastExecutionPlan),
}

impl ReferenceSearchPlan {
    pub(crate) fn new(request: &CompressionParameterRequest, source_len: usize) -> Result<Self> {
        let source_size = u64::try_from(source_len).map_err(|_| Error::ResourceLimit)?;
        if source_size == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        let parameters = request.resolve(SourceSize::Known(source_size), 0)?;
        match parameters.strategy() {
            ReferenceStrategy::Fast => {
                FastExecutionPlan::from_resolved_parameters(parameters, source_len).map(Self::Fast)
            }
            ReferenceStrategy::DoubleFast => {
                DoubleFastExecutionPlan::from_resolved_parameters(parameters, source_len)
                    .map(Self::DoubleFast)
            }
            _ => Err(Error::UnsupportedFeature),
        }
    }

    pub(crate) fn source_len(self) -> usize {
        match self {
            Self::Fast(plan) => plan.source_len(),
            Self::DoubleFast(plan) => plan.source_len(),
        }
    }
    pub(crate) fn parameters(self) -> ResolvedCompressionParameters {
        match self {
            Self::Fast(plan) => plan.parameters(),
            Self::DoubleFast(plan) => plan.parameters(),
        }
    }
    pub(crate) fn block_limit(self) -> usize {
        match self {
            Self::Fast(plan) => plan.block_limit(),
            Self::DoubleFast(plan) => plan.block_limit(),
        }
    }
}

/// One selected parser owns its tables and scratch. Committed entropy remains
/// exclusively in the enclosing frame workspace; no repeats are duplicated here.
/// The enum adds no unchecked representation or allocator erasure.
pub(crate) enum ReferenceSearchState<'a, A: Allocator> {
    Fast(FastMatchState<'a, A>),
    DoubleFast(DoubleFastMatchState<'a, A>),
}

impl<'a, A: Allocator + Clone> ReferenceSearchState<'a, A> {
    /// Borrow the factory, cloning exactly once per selected child owner. Fast
    /// retains table/literal/sequence clone order; DoubleFast adds its separate
    /// short table before the two scratch vectors. No inactive owner is created.
    pub(crate) fn new(source: &'a [u8], plan: ReferenceSearchPlan, factory: &A) -> Result<Self> {
        if source.len() != plan.source_len() {
            return Err(Error::InvalidParameter);
        }
        match plan {
            ReferenceSearchPlan::Fast(plan) => {
                let table = MatchTable::with_layout_in(plan.layout(), factory.clone())?;
                let literals = Vec::new_in(factory.clone());
                let sequences = Vec::new_in(factory.clone());
                Ok(Self::Fast(FastMatchState::from_parts(
                    source, plan, table, literals, sequences,
                )?))
            }
            ReferenceSearchPlan::DoubleFast(plan) => {
                let tables = DoubleFastTables::new_in(
                    plan.long_layout(),
                    plan.short_layout(),
                    factory.clone(),
                    factory.clone(),
                )?;
                let literals = Vec::new_in(factory.clone());
                let sequences = Vec::new_in(factory.clone());
                Ok(Self::DoubleFast(DoubleFastMatchState::from_parts(
                    source, plan, tables, literals, sequences,
                )?))
            }
        }
    }
}

impl<A: Allocator> ReferenceSearchState<'_, A> {
    /// Report whether scratch describes a fresh candidate. Detailed parser-local
    /// repeats remain in each parser's result; emission reconstructs checked
    /// offset intents from the sole committed entropy snapshot.
    pub(crate) fn parse_next(&mut self, length: usize, committed: [u32; 3]) -> Result<bool> {
        match self {
            Self::Fast(state) => state
                .parse_next(length, committed)
                .map(|candidate| candidate.is_some()),
            Self::DoubleFast(state) => state
                .parse_next(length, committed)
                .map(|candidate| candidate.is_some()),
        }
    }
    pub(crate) fn literals(&self) -> &[u8] {
        match self {
            Self::Fast(state) => state.literals(),
            Self::DoubleFast(state) => state.literals(),
        }
    }
    pub(crate) fn sequences(&self) -> &[SequenceWithOffset] {
        match self {
            Self::Fast(state) => state.sequences(),
            Self::DoubleFast(state) => state.sequences(),
        }
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        match self {
            Self::Fast(state) => state.retained_capacity(),
            Self::DoubleFast(state) => state.retained_capacity(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompressionParameterRequest, RawCompressionParameters};
    use alloc::alloc::Global;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn plans(size: usize) -> [ReferenceSearchPlan; 2] {
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_window_log(10).unwrap();
        request.set_hash_log(10).unwrap();
        request.set_chain_log(9).unwrap();
        request.set_min_match(4).unwrap();
        let fast = FastExecutionPlan::new(&request, size).unwrap();
        request.set_strategy(Some(crate::ReferenceStrategy::DoubleFast));
        let double = DoubleFastExecutionPlan::new(&request, size).unwrap();
        [
            ReferenceSearchPlan::Fast(fast),
            ReferenceSearchPlan::DoubleFast(double),
        ]
    }

    #[test]
    fn only_selected_tables_allocate_and_each_child_retains_its_owner() {
        let input = [b'x'; 64];
        for plan in plans(input.len()) {
            let (table_count, owner_count, bytes) = match plan {
                ReferenceSearchPlan::Fast(p) => (1, 4, p.table_bytes()),
                ReferenceSearchPlan::DoubleFast(p) => (2, 5, p.table_bytes()),
            };
            let factory = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = factory.monitor();
            let state = ReferenceSearchState::new(&input, plan, &factory).unwrap();
            let snapshot = monitor.snapshot();
            assert_eq!(snapshot.attempts, table_count);
            assert_eq!(snapshot.live_blocks, table_count);
            assert_eq!(snapshot.identities_created, owner_count);
            assert_eq!(snapshot.live_requested_bytes, bytes as u128);
            assert_eq!(state.retained_capacity(), bytes);
            drop(state);
            drop(factory);
            monitor.assert_released();
            assert_eq!(monitor.snapshot().owners_dropped, owner_count);
        }
    }

    #[test]
    fn mismatched_source_precedes_clones_and_borrowed_nonclone_dispatch_is_supported() {
        let input = [b'x'; 64];
        for plan in plans(input.len()) {
            let owner = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let monitor = owner.monitor();
            let factory = &owner;
            assert!(monitor.fail_next());
            let before = monitor.snapshot();
            assert!(matches!(
                ReferenceSearchState::new(&input[..63], plan, &factory),
                Err(Error::InvalidParameter)
            ));
            assert_eq!(monitor.snapshot(), before);
            monitor.clear_failure();
            let mut state = ReferenceSearchState::new(&input, plan, &factory).unwrap();
            assert!(state.parse_next(input.len(), [1, 4, 8]).unwrap());
            assert!(!state.sequences().is_empty());
            assert_eq!(monitor.snapshot().identities_created, 1);
            drop(state);
            monitor.assert_released();
        }
    }

    #[test]
    fn unequal_double_fast_dispatch_matches_original_ordinary_candidates() {
        let cases = crate::double_fast_frame_oracle_fixture::cases().unwrap();
        let case = cases.iter().find(|c| c.name == "table_l6_s18").unwrap();
        let parameters = RawCompressionParameters {
            window_log: u32::from(case.window_log),
            chain_log: u32::from(case.short_log),
            hash_log: u32::from(case.long_log),
            search_log: u32::from(case.search_log),
            min_match: u32::from(case.min_match),
            target_length: case.target_length,
            strategy: 2,
        }
        .validate()
        .unwrap();
        let plan = DoubleFastExecutionPlan::from_observed_parameters(parameters, case.source.len())
            .unwrap();
        let mut state =
            ReferenceSearchState::new(case.source, ReferenceSearchPlan::DoubleFast(plan), &Global)
                .unwrap();
        assert!(matches!(state, ReferenceSearchState::DoubleFast(_)));
        let mut committed = [1, 4, 8];
        let mut blocks = case.blocks.iter();
        for &length in &case.schedule {
            if length == 0 {
                assert!(!state.parse_next(0, committed).unwrap());
                continue;
            }
            let block = blocks.next().unwrap();
            assert_eq!(committed, block.wire_before);
            assert_eq!(
                state.parse_next(length, committed).unwrap(),
                block.candidate.is_some()
            );
            if let Some(candidate) = &block.candidate {
                assert_eq!(state.literals(), candidate.literals);
                assert_eq!(state.sequences().len(), candidate.sequences.len());
                let mut trial = committed;
                for (got, want) in state.sequences().iter().zip(&candidate.sequences) {
                    let seq = got.sequence;
                    let offset = got
                        .offset
                        .resolve(seq.literal_length, seq.distance, trial)
                        .unwrap();
                    assert_eq!(
                        (
                            seq.literal_length,
                            seq.match_length,
                            seq.distance,
                            offset.offset_value
                        ),
                        (
                            want.literal_length,
                            want.match_length,
                            want.distance,
                            want.offset_value
                        )
                    );
                    trial = offset.next_repeat;
                }
                assert_eq!(trial, candidate.wire_repeats);
                if block.kind == 2 {
                    committed = trial;
                }
            }
            assert_eq!(committed, block.wire_after);
        }
        assert!(blocks.next().is_none());
    }
}
