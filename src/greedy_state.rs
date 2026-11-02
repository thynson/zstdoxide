//! Retained selected Greedy tables for one complete contiguous source.
#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::greedy_block::GreedyBlock;
use crate::greedy_parse::{GreedyParseResult, GreedyProgress, GreedySearch, parse_with_depth};
use crate::greedy_plan::{GreedyExecutionPlan, GreedyTablePlan};
use crate::reference_chain::{ReferenceChain, StagedChainView};
use crate::reference_tree::ReferenceTree;
use crate::row_table::{RowTable, StagedRowView};
use crate::sequence_context::SequenceWithOffset;
use crate::{Error, Result};

/// Exactly two independent owners for the selected family. No inactive chain
/// storage, allocator clone, trait object or unsafe composite is introduced.
/// The existing leaf features select the equivalent access representations.
pub(crate) enum GreedyTables<A: Allocator> {
    Tree(ReferenceTree<A>),
    Chain(ReferenceChain<A>),
    Row(RowTable<A>),
}

impl<A: Allocator> GreedyTables<A> {
    pub(crate) fn new_in(plan: GreedyTablePlan, first: A, second: A) -> Result<Self> {
        match plan {
            GreedyTablePlan::Tree { layout, .. } => {
                ReferenceTree::new_in(layout, first, second).map(Self::Tree)
            }
            GreedyTablePlan::Chain { key, chain_log, .. } => {
                ReferenceChain::new_in(key, chain_log, first, second).map(Self::Chain)
            }
            GreedyTablePlan::Row(layout) => RowTable::new_in(layout, first, second).map(Self::Row),
        }
    }
    fn matches(&self, plan: GreedyTablePlan) -> bool {
        match (self, plan) {
            (Self::Tree(table), GreedyTablePlan::Tree { layout, .. }) => table.layout() == layout,
            (Self::Chain(table), GreedyTablePlan::Chain { key, chain_log, .. }) => {
                table.key_layout() == key && table.chain_log() == chain_log
            }
            (Self::Row(table), GreedyTablePlan::Row(layout)) => table.layout() == layout,
            _ => false,
        }
    }
    fn reset(&mut self) {
        match self {
            Self::Tree(table) => table.reset(),
            Self::Chain(table) => table.reset(),
            Self::Row(table) => table.reset(),
        }
    }

    /// Seed every staged key position of a combined borrow so candidates can
    /// point into the dictionary region; parsing never inserts those
    /// positions itself. Only called with a staged prefix: the plain
    /// zero-prefix path never seeds and keeps every family's fresh behavior.
    /// Ascending insertion satisfies both owners' newest-first contracts, and
    /// the parser's window floor still bounds every candidate it accepts.
    /// Returns the parse sampling frontier: the prefix boundary for the
    /// seeded chain and row owners, or the combined-buffer start for the tree
    /// family, which needs no seeding because its first query back-fills the
    /// whole staged region through the unsorted-insert frontier. This is the
    /// shared combined dictionary staging contract of the Fast and DoubleFast
    /// profiles.
    pub(crate) fn seed_prefix(&mut self, source: &[u8], prefix_len: usize) -> Result<usize> {
        match self {
            Self::Tree(_) => Ok(0),
            Self::Chain(table) => {
                seed_chain(table, source, prefix_len)?;
                Ok(prefix_len)
            }
            Self::Row(table) => {
                seed_row(table, source, prefix_len)?;
                Ok(prefix_len)
            }
        }
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        match self {
            Self::Tree(table) => table.layout().total_bytes(),
            Self::Chain(table) => table.retained_capacity(),
            Self::Row(table) => table.retained_capacity(),
        }
    }
}

/// Seed the chain owner with every staged key position of the combined
/// borrow, mirroring upstream dictionary loading through one insert per
/// position. Later positions win each bucket; the parser's window floor
/// bounds what it accepts from these candidates.
fn seed_chain<A: Allocator>(
    table: &mut ReferenceChain<A, A>,
    source: &[u8],
    prefix_len: usize,
) -> Result<()> {
    let layout = table.key_layout();
    if let Some(last) = prefix_len.checked_sub(usize::from(layout.key_bytes())) {
        for position in 0..=last {
            let key = layout
                .read_key(&source[position..])
                .ok_or(Error::InvalidState)?;
            table.insert(key, position)?;
        }
    }
    Ok(())
}

/// Seed the row owner with every staged key position under the same fresh
/// salt the parser searches with, so seeded tags match every later lookup.
/// Ascending insertion satisfies each row's newest-first cycle, mirroring
/// upstream row dictionary loading.
fn seed_row<A: Allocator>(table: &mut RowTable<A>, source: &[u8], prefix_len: usize) -> Result<()> {
    let layout = table.layout();
    let key_layout = layout.key_layout();
    if let Some(last) = prefix_len.checked_sub(usize::from(key_layout.key_bytes())) {
        for position in 0..=last {
            let key = key_layout
                .read_key(&source[position..])
                .ok_or(Error::InvalidState)?;
            table.insert(
                layout.key(key, crate::greedy_key::FRESH_ROW_SALT)?,
                position,
            )?;
        }
    }
    Ok(())
}

/// Sampling persists independently of candidate entropy/repeat publication.
///
/// Positions index the sole borrowed source; ownership cannot outlive it. The
/// caller supplies committed repeats, including after raw/RLE fallback. A parse
/// error poisons this owner so partially updated tables can never be reused.
/// Corresponds to the no-dictionary match/window part of `ZSTD_CCtx`.
/// A read-only compiled candidate source for the selected Greedy family.
///
/// A chain profile carries a compiled chain; a row profile carries a compiled
/// row. The variant must match the per-call owner family.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "public compiled-dictionary emission follows")
)]
pub(crate) enum GreedyCandidates<'a> {
    Chain(StagedChainView<'a>),
    Row(StagedRowView<'a>),
}

pub(crate) struct GreedyMatchState<'a, A: Allocator> {
    source: &'a [u8],
    plan: GreedyExecutionPlan,
    tables: GreedyTables<A>,
    dictionary: Option<GreedyCandidates<'a>>,
    literals: Vec<u8, A>,
    sequences: Vec<SequenceWithOffset, A>,
    position: usize,
    prefix_floor: usize,
    progress: GreedyProgress,
    failed: bool,
}

impl<'a, A: Allocator> GreedyMatchState<'a, A> {
    /// Bind complete owners to one source, invalidating earlier sampled indices.
    /// Does not clone an allocator, allocate, or borrow temporary source storage.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "zero-prefix convenience constructor for tests")
    )]
    pub(crate) fn from_parts(
        source: &'a [u8],
        plan: GreedyExecutionPlan,
        tables: GreedyTables<A>,
        literals: Vec<u8, A>,
        sequences: Vec<SequenceWithOffset, A>,
    ) -> Result<Self> {
        Self::from_parts_with_prefix(source, plan, tables, literals, sequences, 0)
    }

    /// Bind owners to a combined borrow whose first `prefix_len` bytes precede
    /// the frame. Every staged key position is seeded into the selected chain
    /// or row owner (the tree family instead back-fills through its
    /// unsorted-insert frontier), parsing starts at the prefix boundary with
    /// its sampling frontier at the owner-appropriate point, and the existing
    /// window floor bounds every emitted offset.
    pub(crate) fn from_parts_with_prefix(
        source: &'a [u8],
        plan: GreedyExecutionPlan,
        tables: GreedyTables<A>,
        literals: Vec<u8, A>,
        sequences: Vec<SequenceWithOffset, A>,
        prefix_len: usize,
    ) -> Result<Self> {
        Self::from_parts_with_compiled_prefix(
            source, plan, tables, literals, sequences, prefix_len, None,
        )
    }

    /// Bind owners to a combined borrow whose staged prefix equals a compiled
    /// dictionary's loaded suffix.
    ///
    /// The per-call chain starts empty and receives every sample; the compiled
    /// chain is consulted only after the per-call chain and only with the
    /// remaining attempt budget, matching the pinned `dictMatchState` order.
    /// A missing compiled owner keeps the seeding path. A chain owner may carry
    /// only a compiled chain and a row owner only a compiled row; layouts and
    /// the staged length must match before either owner is reset.
    pub(crate) fn from_parts_with_compiled_prefix(
        source: &'a [u8],
        plan: GreedyExecutionPlan,
        mut tables: GreedyTables<A>,
        mut literals: Vec<u8, A>,
        mut sequences: Vec<SequenceWithOffset, A>,
        prefix_len: usize,
        dictionary: Option<GreedyCandidates<'a>>,
    ) -> Result<Self> {
        let candidate_matches = match (&tables, dictionary) {
            (GreedyTables::Chain(table), Some(GreedyCandidates::Chain(candidate))) => {
                candidate.staged_len() == prefix_len
                    && candidate.view().key_layout() == table.key_layout()
            }
            (GreedyTables::Row(table), Some(GreedyCandidates::Row(candidate))) => {
                candidate.staged_len() == prefix_len && candidate.view().layout() == table.layout()
            }
            (GreedyTables::Chain(_) | GreedyTables::Row(_) | GreedyTables::Tree(_), None) => true,
            _ => false,
        };
        if source
            .len()
            .checked_sub(prefix_len)
            .is_none_or(|frame_len| frame_len != plan.source_len())
            || !tables.matches(plan.tables())
            || !candidate_matches
        {
            return Err(Error::InvalidParameter);
        }
        tables.reset();
        let next = if prefix_len == 0 {
            0
        } else if dictionary.is_some() {
            // Compiled chain/row positions already index the staged prefix, so
            // the sampling frontier starts exactly at the prefix boundary.
            prefix_len
        } else {
            tables.seed_prefix(source, prefix_len)?
        };
        literals.clear();
        sequences.clear();
        Ok(Self {
            source,
            plan,
            tables,
            dictionary,
            literals,
            sequences,
            position: prefix_len,
            prefix_floor: 0,
            progress: GreedyProgress {
                next,
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
                GreedyTables::Tree(table) => GreedySearch::Tree {
                    table,
                    attempts: self.plan.tables().attempts(),
                },
                GreedyTables::Chain(table) => GreedySearch::Chain {
                    table,
                    dictionary: match self.dictionary {
                        Some(GreedyCandidates::Chain(candidate)) => Some(candidate.view()),
                        _ => None,
                    },
                    attempts: self.plan.tables().attempts(),
                },
                GreedyTables::Row(table) => GreedySearch::Row {
                    table,
                    dictionary: match self.dictionary {
                        Some(GreedyCandidates::Row(candidate)) => Some(candidate.view()),
                        _ => None,
                    },
                    salt: crate::greedy_key::FRESH_ROW_SALT,
                },
            };
            match parse_with_depth(
                block,
                search,
                &mut self.progress,
                committed,
                &mut self.literals,
                &mut self.sequences,
                self.plan.depth(),
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

    fn plan(length: usize, row: bool, depth: u8) -> GreedyExecutionPlan {
        GreedyExecutionPlan::from_family_parameters(
            RawCompressionParameters {
                window_log: if row { 15 } else { 10 },
                chain_log: 10,
                hash_log: 10,
                search_log: 4,
                min_match: 4,
                target_length: 0,
                strategy: u32::from(depth) + 3,
            }
            .validate()
            .unwrap(),
            length,
        )
        .unwrap()
    }
    fn observed_plan(case: &OracleFrameCase<'_>, depth: u8) -> GreedyExecutionPlan {
        let parameters = RawCompressionParameters {
            window_log: u32::from(case.window_log),
            chain_log: u32::from(case.chain_log),
            hash_log: u32::from(case.hash_log),
            search_log: u32::from(case.search_log),
            min_match: u32::from(case.min_match),
            target_length: case.target_length,
            strategy: if case.tree_search {
                6
            } else {
                u32::from(depth) + 3
            },
        }
        .validate()
        .unwrap();
        if case.tree_search {
            GreedyExecutionPlan::from_tree_parameters(parameters, case.source.len())
        } else {
            GreedyExecutionPlan::from_family_parameters(parameters, case.source.len())
        }
        .unwrap()
    }

    #[test]
    fn tree_search_corruption_after_pending_insert_poisons_retained_state() {
        let mut input: Vec<u8> = (0..96).collect();
        input[0] = 255;
        let plan = GreedyExecutionPlan::from_tree_parameters(
            crate::RawCompressionParameters {
                window_log: 10,
                hash_log: 6,
                chain_log: 6,
                search_log: 4,
                min_match: 4,
                target_length: 0,
                strategy: 6,
            }
            .validate()
            .unwrap(),
            input.len(),
        )
        .unwrap();
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut state = GreedyMatchState::from_parts(
            &input,
            plan,
            GreedyTables::new_in(plan.tables(), owner.clone(), owner.clone()).unwrap(),
            Vec::new_in(owner.clone()),
            Vec::new_in(owner),
        )
        .unwrap();
        let key = plan.tables().key_layout();
        let pending = key.read_key(&input).unwrap();
        let query = key.read_key(&input[1..]).unwrap();
        assert_ne!(key.bucket(pending), key.bucket(query));
        let GreedyTables::Tree(table) = &mut state.tables else {
            panic!("tree table")
        };
        table.replace_head(query, input.len() + 8).unwrap();
        let before = maps(&state.tables);
        assert_eq!(
            state.parse_next(input.len(), [1, 4, 8]),
            Err(Error::CorruptData)
        );
        assert!(state.failed);
        assert_ne!(maps(&state.tables), before);
        let after = (
            monitor.snapshot(),
            maps(&state.tables),
            state.progress,
            state.position,
        );
        for length in [0, 1, 96, usize::MAX] {
            assert_eq!(
                state.parse_next(length, [1, 4, 8]),
                Err(Error::InvalidState)
            );
        }
        assert_eq!(
            (
                monitor.snapshot(),
                maps(&state.tables),
                state.progress,
                state.position
            ),
            after
        );
        drop(state);
        monitor.assert_released();
        assert_eq!(
            monitor.snapshot().owners_dropped,
            monitor.snapshot().identities_created
        );
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
            GreedyTables::Tree(table) => {
                let (h, e) = table.test_maps();
                let convert = |v: &usize| match crate::tree_layout::TreeLink::from_stored(*v) {
                    crate::tree_layout::TreeLink::Empty => 0,
                    crate::tree_layout::TreeLink::Unsorted => 1,
                    crate::tree_layout::TreeLink::Position(p) => u32::try_from(p + 2).unwrap(),
                };
                (
                    h.iter().map(convert).collect(),
                    e.iter().map(convert).collect(),
                )
            }
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

    fn check_ordinary_cases<'a>(
        cases: impl IntoIterator<Item = (u8, OracleFrameCase<'a>)>,
    ) -> (usize, usize, [usize; 2]) {
        let mut blocks = 0;
        let mut fresh = 0;
        let mut discarded = [0; 2];
        for (depth, case) in cases {
            let plan = observed_plan(&case, depth);
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
                        // The lazy family never reads the raw third C cell. Rust carries
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
        (blocks, fresh, discarded)
    }

    #[test]
    fn all_ordinary_c_candidates_tables_and_frontiers_match_without_reseeding() {
        assert_eq!(
            check_ordinary_cases(
                crate::greedy_frame_oracle_fixture::cases()
                    .unwrap()
                    .into_iter()
                    .map(|c| (0, c)),
            ),
            (395, 367, [25, 12]),
        );
    }

    #[test]
    fn all_lazy_family_ordinary_trajectories_retain_exact_state() {
        assert_eq!(
            check_ordinary_cases(
                crate::lazy_frame_oracle_fixture::cases()
                    .unwrap()
                    .into_iter()
                    .map(|c| (c.depth, c.observation)),
            ),
            (1377, 1293, [165, 36]),
        );
    }

    #[test]
    fn all_tree_ordinary_trajectories_retain_exact_state() {
        assert_eq!(
            check_ordinary_cases(
                crate::tree_frame_oracle_fixture::cases()
                    .unwrap()
                    .into_iter()
                    .map(|c| (2, c))
            ),
            (531, 503, [38, 32])
        );
    }

    #[test]
    fn invalid_arguments_precede_allocation_and_leave_state_usable() {
        for (depth, row) in (0..=2).flat_map(|depth| [false, true].map(|row| (depth, row))) {
            let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = owner.monitor();
            let input = [b'z'; 96];
            let plan = plan(input.len(), row, depth);
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
        for (depth, row) in (0..=2).flat_map(|depth| [false, true].map(|row| (depth, row))) {
            let plan = plan(input.len(), row, depth);
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
                GreedyTables::Tree(_) => unreachable!("chain/row predecessor fixture"),
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
        for (depth, row) in (0..=2).flat_map(|depth| [false, true].map(|row| (depth, row))) {
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
                let plan = plan(input.len(), row, depth);
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
        for (depth, row) in (0..=2).flat_map(|depth| [false, true].map(|row| (depth, row))) {
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
                    GreedyTables::new_in(plan(96, row, depth).tables(), first, second),
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
        for (depth, row) in (0..=2).flat_map(|depth| [false, true].map(|row| (depth, row))) {
            let owners: [_; 4] = core::array::from_fn(|_| {
                TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone()
            });
            let monitors = owners.each_ref().map(|a| a.monitor());
            let input = [b'x'; 192];
            let plan = plan(input.len(), row, depth);
            let mut tables = GreedyTables::new_in(plan.tables(), &owners[0], &owners[1]).unwrap();
            let key = plan.tables().key_layout().read_key(&input).unwrap();
            match &mut tables {
                GreedyTables::Tree(_) => unreachable!("chain/row predecessor fixture"),
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
        for (depth, row) in (0..=2).flat_map(|depth| [false, true].map(|row| (depth, row))) {
            for wrong in 0..3 {
                let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
                let monitor = owner.monitor();
                let input = [b'z'; 64];
                let expected = plan(64, row, depth);
                let mut actual = plan(64, !row, depth).tables();
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
