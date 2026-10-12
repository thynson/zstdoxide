//! Retained selected Greedy tables for one complete contiguous source.
#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::greedy_block::GreedyBlock;
use crate::greedy_parse::{GreedyParseResult, GreedyProgress, GreedySearch, parse_block};
use crate::greedy_plan::{GreedyExecutionPlan, GreedyTablePlan};
use crate::reference_chain::ReferenceChain;
use crate::row_table::RowTable;
use crate::sequence_context::SequenceWithOffset;
use crate::{Error, Result};

/// Exactly two independent owners for the selected family. No inactive chain
/// storage, allocator clone, trait object or unsafe composite is introduced.
/// The existing leaf features select the equivalent access representations.
pub(crate) enum GreedyTables<A: Allocator> {
    Chain(ReferenceChain<A>),
    Row(RowTable<A>),
}

impl<A: Allocator> GreedyTables<A> {
    pub(crate) fn new_in(plan: GreedyTablePlan, first: A, second: A) -> Result<Self> {
        match plan {
            GreedyTablePlan::Chain { key, chain_log, .. } => {
                ReferenceChain::new_in(key, chain_log, first, second).map(Self::Chain)
            }
            GreedyTablePlan::Row(layout) => RowTable::new_in(layout, first, second).map(Self::Row),
        }
    }
    fn matches(&self, plan: GreedyTablePlan) -> bool {
        match (self, plan) {
            (Self::Chain(table), GreedyTablePlan::Chain { key, chain_log, .. }) => {
                table.key_layout() == key && table.chain_log() == chain_log
            }
            (Self::Row(table), GreedyTablePlan::Row(layout)) => table.layout() == layout,
            _ => false,
        }
    }
    fn reset(&mut self) {
        match self {
            Self::Chain(table) => table.reset(),
            Self::Row(table) => table.reset(),
        }
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        match self {
            Self::Chain(table) => table.retained_capacity(),
            Self::Row(table) => table.retained_capacity(),
        }
    }
}

/// Sampling persists independently of candidate entropy/repeat publication.
///
/// Positions index the sole borrowed source; ownership cannot outlive it. The
/// caller supplies committed repeats, including after raw/RLE fallback. A parse
/// error poisons this owner so partially updated tables can never be reused.
/// Corresponds to the no-dictionary match/window part of `ZSTD_CCtx`.
pub(crate) struct GreedyMatchState<'a, A: Allocator> {
    source: &'a [u8],
    plan: GreedyExecutionPlan,
    tables: GreedyTables<A>,
    literals: Vec<u8, A>,
    sequences: Vec<SequenceWithOffset, A>,
    position: usize,
    prefix_floor: usize,
    progress: GreedyProgress,
    failed: bool,
}

impl<'a, A: Allocator> GreedyMatchState<'a, A> {
    /// Bind supplied owners to a source without allocation or allocator cloning.
    /// Reject mismatched geometry before resetting any logical owner contents.
    pub(crate) fn from_parts(
        source: &'a [u8],
        plan: GreedyExecutionPlan,
        mut tables: GreedyTables<A>,
        mut literals: Vec<u8, A>,
        mut sequences: Vec<SequenceWithOffset, A>,
    ) -> Result<Self> {
        if source.len() != plan.source_len() || !tables.matches(plan.tables()) {
            return Err(Error::InvalidParameter);
        }
        tables.reset();
        literals.clear();
        sequences.clear();
        Ok(Self {
            source,
            plan,
            tables,
            literals,
            sequences,
            position: 0,
            prefix_floor: 0,
            progress: GreedyProgress {
                next: 0,
                salt_entropy: 0,
                skipping: false,
            },
            failed: false,
        })
    }

    /// Prepare a candidate for the next bounded contiguous block.
    ///
    /// Empty calls leave every field unchanged. Tiny calls preserve stale
    /// scratch/tables but still clamp the sampling frontier to the block-start
    /// window floor. Fresh calls then apply the separate 384/192 adjustment.
    /// Argument errors precede mutation; any direct-parser failure requires
    /// dropping the poisoned state. Corresponds to `ZSTD_compress_frameChunk`
    /// followed by `ZSTD_buildSeqStore` and the selected Greedy block parser.
    pub(crate) fn parse_next(
        &mut self,
        length: usize,
        committed: [u32; 3],
    ) -> Result<Option<GreedyParseResult>> {
        if self.failed {
            return Err(Error::InvalidState);
        }
        let end = self
            .position
            .checked_add(length)
            .filter(|&end| end <= self.source.len())
            .ok_or(Error::InvalidParameter)?;
        if length > self.plan.block_limit() || committed.contains(&0) {
            return Err(Error::InvalidParameter);
        }
        if length == 0 {
            return Ok(None);
        }
        let floor = self
            .prefix_floor
            .max(self.position.saturating_sub(self.plan.window_size()));
        let candidate = if length < 7 {
            self.progress.next = self.progress.next.max(floor);
            None
        } else {
            // All constructor conditions are checked before changing progress.
            let block = GreedyBlock::new(
                self.source,
                self.position..end,
                floor,
                self.plan.tables().key_layout(),
                self.plan.window_size(),
                self.plan.tables().is_row(),
            )?;
            self.progress.next = self.progress.next.max(floor);
            let gap = self.position - self.progress.next;
            if gap > 384 {
                self.progress.next = self.position - 192.min(gap - 384);
            }
            let search = match &mut self.tables {
                GreedyTables::Chain(table) => GreedySearch::Chain {
                    table,
                    attempts: self.plan.tables().attempts(),
                },
                GreedyTables::Row(table) => GreedySearch::Row {
                    table,
                    salt: crate::greedy_key::FRESH_ROW_SALT,
                },
            };
            match parse_block(
                block,
                search,
                &mut self.progress,
                committed,
                &mut self.literals,
                &mut self.sequences,
            ) {
                Ok(result) => Some(result),
                Err(error) => {
                    self.failed = true;
                    return Err(error);
                }
            }
        };
        self.position = end;
        self.prefix_floor = floor;
        Ok(candidate)
    }
    pub(crate) fn literals(&self) -> &[u8] {
        &self.literals
    }
    pub(crate) fn sequences(&self) -> &[SequenceWithOffset] {
        &self.sequences
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        self.tables.retained_capacity()
            + self.literals.capacity()
            + self.sequences.capacity() * core::mem::size_of::<SequenceWithOffset>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RawCompressionParameters;
    use crate::greedy_frame_oracle_fixture::OracleFrameCase;
    use alloc::{alloc::Global, vec};
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn plan(length: usize, row: bool) -> GreedyExecutionPlan {
        GreedyExecutionPlan::from_resolved_parameters(
            RawCompressionParameters {
                window_log: if row { 15 } else { 10 },
                chain_log: 10,
                hash_log: 10,
                search_log: 4,
                min_match: 4,
                target_length: 0,
                strategy: 3,
            }
            .validate()
            .unwrap(),
            length,
        )
        .unwrap()
    }
    fn observed_plan(case: &OracleFrameCase<'_>) -> GreedyExecutionPlan {
        GreedyExecutionPlan::from_resolved_parameters(
            RawCompressionParameters {
                window_log: u32::from(case.window_log),
                chain_log: u32::from(case.chain_log),
                hash_log: u32::from(case.hash_log),
                search_log: u32::from(case.search_log),
                min_match: u32::from(case.min_match),
                target_length: case.target_length,
                strategy: 3,
            }
            .validate()
            .unwrap(),
            case.source.len(),
        )
        .unwrap()
    }
    fn raw_index(value: usize) -> u32 {
        if value == usize::MAX {
            0
        } else {
            u32::try_from(value.checked_add(2).unwrap()).unwrap()
        }
    }
    fn maps<A: Allocator>(tables: &GreedyTables<A>) -> (Vec<u32>, Vec<u32>) {
        match tables {
            GreedyTables::Chain(table) => {
                let (heads, links) = table.test_maps();
                (
                    heads.iter().map(|&p| raw_index(p)).collect(),
                    links.iter().map(|&p| raw_index(p)).collect(),
                )
            }
            GreedyTables::Row(table) => {
                let (positions, tags) = table.test_maps();
                (
                    positions.iter().map(|&p| raw_index(p)).collect(),
                    tags.iter().map(|&p| u32::from(p)).collect(),
                )
            }
        }
    }

    #[test]
    fn all_ordinary_c_candidates_tables_and_frontiers_match_without_reseeding() {
        let mut blocks = 0;
        let mut fresh = 0;
        let mut discarded = [0; 2];
        for case in crate::greedy_frame_oracle_fixture::cases().unwrap() {
            let plan = observed_plan(&case);
            let mut state = GreedyMatchState::from_parts(
                case.source,
                plan,
                GreedyTables::new_in(plan.tables(), Global, Global).unwrap(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
            let mut expected_first = vec![0; 1usize << case.hash_log];
            let mut expected_second = vec![
                0;
                1usize
                    << if case.row_search {
                        case.hash_log
                    } else {
                        case.chain_log
                    }
            ];
            let mut committed = [1, 4, 8];
            let mut observations = case.blocks.iter();
            for &length in &case.schedule {
                if length == 0 {
                    let before = (
                        state.position,
                        state.prefix_floor,
                        state.progress,
                        maps(&state.tables),
                        state.literals().to_vec(),
                        state.sequences().to_vec(),
                    );
                    assert_eq!(state.parse_next(0, committed), Ok(None));
                    assert_eq!(
                        (
                            state.position,
                            state.prefix_floor,
                            state.progress,
                            maps(&state.tables),
                            state.literals().to_vec(),
                            state.sequences().to_vec()
                        ),
                        before
                    );
                    continue;
                }
                let expected = observations.next().unwrap();
                assert_eq!(
                    (
                        state.position,
                        state.progress.next,
                        state.progress.skipping,
                        state.progress.salt_entropy
                    ),
                    (
                        expected.start,
                        expected.sampling_before.next().unwrap(),
                        expected.sampling_before.skipping,
                        expected.sampling_before.salt_entropy
                    ),
                    "{} call{}",
                    case.name,
                    expected.call
                );
                assert_eq!(committed, expected.wire_before);
                let previous_scratch = (state.literals().to_vec(), state.sequences().to_vec());
                let actual = state
                    .parse_next(length, committed)
                    .unwrap_or_else(|e| panic!("{} call{}: {e:?}", case.name, expected.call));
                assert_eq!(
                    (
                        state.prefix_floor,
                        state.progress.next,
                        state.progress.skipping,
                        state.progress.salt_entropy
                    ),
                    (
                        expected.prefix_floor,
                        expected.sampling_after.next().unwrap(),
                        expected.sampling_after.skipping,
                        expected.sampling_after.salt_entropy
                    ),
                    "{} call{}",
                    case.name,
                    expected.call
                );
                match (actual, &expected.candidate) {
                    (Some(actual), Some(candidate)) => {
                        fresh += 1;
                        assert_eq!(
                            state.literals(),
                            candidate.literals,
                            "{} call{}",
                            case.name,
                            expected.call
                        );
                        assert_eq!(state.sequences().len(), candidate.sequences.len());
                        let mut trial = committed;
                        for (got, want) in state.sequences().iter().zip(&candidate.sequences) {
                            let sequence = got.sequence;
                            let resolved = got
                                .offset
                                .resolve(sequence.literal_length, sequence.distance, trial)
                                .unwrap();
                            assert_eq!(
                                (
                                    sequence.literal_length,
                                    sequence.match_length,
                                    sequence.distance,
                                    resolved.offset_value
                                ),
                                (
                                    want.literal_length,
                                    want.match_length,
                                    want.distance,
                                    want.offset_value
                                ),
                                "{} call{}",
                                case.name,
                                expected.call
                            );
                            trial = resolved.next_repeat;
                        }
                        assert_eq!(actual.trailing_literals, candidate.trailing_literals);
                        assert_eq!(actual.parser_repeats[..2], candidate.parser_repeats[..2]);
                        // Greedy never reads the raw third C cell. Rust carries
                        // the incoming semantic third and publishes full wire repeats.
                        assert_eq!(actual.parser_repeats[2], committed[2]);
                        assert_eq!(actual.wire_repeats, candidate.wire_repeats);
                        assert_eq!(trial, actual.wire_repeats);
                        if expected.kind == 2 {
                            committed = trial;
                        } else if trial[..2] != committed[..2] {
                            discarded[usize::from(expected.kind)] += 1;
                        }
                    }
                    (None, None) => assert_eq!(
                        (state.literals(), state.sequences()),
                        (previous_scratch.0.as_slice(), previous_scratch.1.as_slice())
                    ),
                    _ => panic!("{} fresh/stale mismatch", case.name),
                }
                assert_eq!(committed, expected.wire_after);
                expected
                    .first_table
                    .apply(&mut expected_first, case.hash_log, 4)
                    .unwrap();
                expected
                    .second_table
                    .apply(
                        &mut expected_second,
                        if case.row_search {
                            case.hash_log
                        } else {
                            case.chain_log
                        },
                        if case.row_search { 1 } else { 4 },
                    )
                    .unwrap();
                let actual = maps(&state.tables);
                assert_eq!(
                    actual.0, expected_first,
                    "{} call{} first owner",
                    case.name, expected.call
                );
                assert_eq!(
                    actual.1, expected_second,
                    "{} call{} second owner",
                    case.name, expected.call
                );
                blocks += 1;
            }
            assert!(observations.next().is_none());
            assert_eq!(state.position, case.source.len());
        }
        assert_eq!((blocks, fresh, discarded), (395, 367, [25, 12]));
    }

    #[test]
    fn invalid_arguments_precede_allocation_and_leave_state_usable() {
        for row in [false, true] {
            let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = owner.monitor();
            let input = [b'z'; 96];
            let plan = plan(input.len(), row);
            let mut state = GreedyMatchState::from_parts(
                &input,
                plan,
                GreedyTables::new_in(plan.tables(), owner.clone(), owner.clone()).unwrap(),
                Vec::new_in(owner.clone()),
                Vec::new_in(owner),
            )
            .unwrap();
            let before = (
                state.position,
                state.prefix_floor,
                state.progress,
                maps(&state.tables),
            );
            assert!(monitor.fail_next());
            let allocations = monitor.snapshot();
            for length in [97, 131073, usize::MAX] {
                assert_eq!(
                    state.parse_next(length, [1, 4, 8]),
                    Err(Error::InvalidParameter)
                );
            }
            assert_eq!(state.parse_next(7, [1, 0, 8]), Err(Error::InvalidParameter));
            assert_eq!(
                (
                    state.position,
                    state.prefix_floor,
                    state.progress,
                    maps(&state.tables)
                ),
                before
            );
            assert_eq!(monitor.snapshot(), allocations);
            assert!(!state.failed);
            monitor.clear_failure();
            state.parse_next(96, [1, 4, 8]).unwrap();
            drop(state);
            monitor.assert_released();
        }
    }

    #[test]
    fn a_search_error_after_insertion_poisoned_state_cannot_be_reused() {
        let mut input: Vec<u8> = (0..96).collect();
        // Separate the first pending row from the next query row while keeping
        // the source free of four-byte repeats.
        input[0] = 200;
        for row in [false, true] {
            let plan = plan(input.len(), row);
            let mut state = GreedyMatchState::from_parts(
                &input,
                plan,
                GreedyTables::new_in(plan.tables(), Global, Global).unwrap(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
            let layout = plan.tables().key_layout();
            let pending = layout.read_key(&input).unwrap();
            let query = layout.read_key(&input[1..]).unwrap();
            // A valid initialized owner with a deliberately future source
            // position lets the parser insert pending position zero, then fail
            // when it inspects the query's candidates. This exercises poisoning
            // after logical writes, independently of allocation failures.
            match &mut state.tables {
                GreedyTables::Chain(table) => {
                    assert_ne!(layout.bucket(pending), layout.bucket(query));
                    table.insert(query, input.len() + 8).unwrap();
                }
                GreedyTables::Row(table) => {
                    let pending = table.layout().key(pending, 0x8358_92b4_bb2c_ae74).unwrap();
                    let query = table.layout().key(query, 0x8358_92b4_bb2c_ae74).unwrap();
                    assert_ne!(pending.row_start(), query.row_start());
                    table.insert(query, input.len() + 8).unwrap();
                }
            }
            let before = maps(&state.tables);
            assert_eq!(
                state.parse_next(input.len(), [1, 4, 8]),
                Err(Error::CorruptData)
            );
            assert_ne!(maps(&state.tables), before);
            assert!(state.failed);
            assert_eq!(state.position, 0);
            for size in [0, 1, 96] {
                assert_eq!(state.parse_next(size, [1, 4, 8]), Err(Error::InvalidState));
            }
        }
    }

    #[test]
    fn either_scratch_allocation_failure_poisons_and_releases_exact_owners() {
        for row in [false, true] {
            for failing in [0, 1] {
                let first = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let fm = first.monitor();
                let second = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let sm = second.monitor();
                let literals = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let lm = literals.monitor();
                let sequences = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let qm = sequences.monitor();
                let input = [b'z'; 96];
                let plan = plan(input.len(), row);
                let mut state = GreedyMatchState::from_parts(
                    &input,
                    plan,
                    GreedyTables::new_in(plan.tables(), first, second).unwrap(),
                    Vec::new_in(literals),
                    Vec::new_in(sequences),
                )
                .unwrap();
                assert!(if failing == 0 {
                    lm.fail_next()
                } else {
                    qm.fail_next()
                });
                assert_eq!(
                    state.parse_next(96, [1, 4, 8]),
                    Err(Error::AllocationFailed)
                );
                assert_eq!(state.position, 0);
                assert!(state.failed);
                lm.clear_failure();
                qm.clear_failure();
                for n in [0, 1, 96] {
                    assert_eq!(state.parse_next(n, [1, 4, 8]), Err(Error::InvalidState));
                }
                drop(state);
                for m in [fm, sm, lm, qm] {
                    m.assert_released();
                    assert_eq!(m.snapshot().owners_dropped, m.snapshot().identities_created);
                }
            }
        }
    }

    #[test]
    fn selected_table_allocation_failures_release_only_the_supplied_owners() {
        for row in [false, true] {
            for failing in [0, 1] {
                let first = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let fm = first.monitor();
                let second = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let sm = second.monitor();
                assert!(if failing == 0 {
                    fm.fail_next()
                } else {
                    sm.fail_next()
                });
                assert!(matches!(
                    GreedyTables::new_in(plan(96, row).tables(), first, second),
                    Err(Error::AllocationFailed)
                ));
                for m in [fm, sm] {
                    m.assert_released();
                    assert_eq!(m.snapshot().owners_dropped, 1);
                }
            }
        }
    }

    #[test]
    fn binding_resets_indices_with_nonclone_owners_and_reuses_scratch() {
        for row in [false, true] {
            let owners: [_; 4] = core::array::from_fn(|_| {
                TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone()
            });
            let monitors = owners.each_ref().map(|a| a.monitor());
            let input = [b'x'; 192];
            let plan = plan(input.len(), row);
            let mut tables = GreedyTables::new_in(plan.tables(), &owners[0], &owners[1]).unwrap();
            let key = plan.tables().key_layout().read_key(&input).unwrap();
            match &mut tables {
                GreedyTables::Chain(table) => table.insert(key, usize::MAX - 1).unwrap(),
                GreedyTables::Row(table) => table
                    .insert(
                        table.layout().key(key, 0x8358_92b4_bb2c_ae74).unwrap(),
                        usize::MAX - 1,
                    )
                    .unwrap(),
            }
            let mut state = GreedyMatchState::from_parts(
                &input,
                plan,
                tables,
                Vec::new_in(&owners[2]),
                Vec::new_in(&owners[3]),
            )
            .unwrap();
            let empty = maps(&state.tables);
            assert!(empty.0.iter().chain(&empty.1).all(|&v| v == 0));
            state.parse_next(96, [1, 4, 8]).unwrap();
            let before = monitors.each_ref().map(|m| m.snapshot());
            let capacity = state.retained_capacity();
            state.parse_next(96, [1, 4, 8]).unwrap();
            assert_eq!(capacity, state.retained_capacity());
            for (m, b) in monitors.iter().zip(before) {
                assert_eq!(m.snapshot().attempts, b.attempts);
                assert_eq!(m.snapshot().identities_created, 1);
            }
            drop(state);
            for m in &monitors {
                m.assert_released();
            }
            drop(owners);
            for m in &monitors {
                assert_eq!(m.snapshot().owners_dropped, 1);
            }
        }
    }

    #[test]
    fn mismatched_source_or_selected_geometry_reject_before_reset_or_requests() {
        for row in [false, true] {
            for wrong in 0..3 {
                let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let monitor = owner.monitor();
                let input = [b'z'; 64];
                let expected = plan(64, row);
                let mut actual = plan(64, !row).tables();
                if wrong != 0 {
                    actual = expected.tables();
                }
                if wrong == 2 {
                    let key = crate::greedy_key::GreedyKeyLayout::new(9, 4).unwrap();
                    actual = if row {
                        GreedyTablePlan::Row(crate::greedy_key::RowLayout::new(key, 4).unwrap())
                    } else {
                        GreedyTablePlan::Chain {
                            key,
                            chain_log: 10,
                            attempts: 16,
                        }
                    };
                }
                let tables = GreedyTables::new_in(actual, owner.clone(), owner.clone()).unwrap();
                let literals = Vec::new_in(owner.clone());
                let sequences = Vec::new_in(owner);
                assert!(monitor.fail_next());
                let before = monitor.snapshot();
                let source = if wrong == 1 { &input[..63] } else { &input[..] };
                assert!(matches!(
                    GreedyMatchState::from_parts(source, expected, tables, literals, sequences),
                    Err(Error::InvalidParameter)
                ));
                assert_eq!(monitor.snapshot().attempts, before.attempts);
                assert_eq!(monitor.snapshot().failures, 0);
                monitor.assert_released();
            }
        }
    }
}
