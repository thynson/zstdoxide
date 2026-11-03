//! Static dispatch over admitted reference search plans and complete owners.
#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::double_fast_plan::DoubleFastExecutionPlan;
use crate::double_fast_state::DoubleFastMatchState;
use crate::double_fast_tables::DoubleFastTables;
use crate::fast_plan::FastExecutionPlan;
use crate::fast_state::FastMatchState;
use crate::greedy_plan::GreedyExecutionPlan;
use crate::greedy_state::{GreedyCandidates, GreedyMatchState, GreedyTables};
use crate::matchfinder::{MatchTable, StagedMatchView};
use crate::optimal_frequencies::OptimalFrequencies;
use crate::optimal_matches::OptimalMatches;
use crate::optimal_parse::OptimalParser;
use crate::optimal_paths::OptimalPaths;
use crate::optimal_plan::OptimalExecutionPlan;
use crate::optimal_retained::Publication;
use crate::optimal_retained_plan::RetainedOptimalPlan;
use crate::optimal_retained_state::RetainedOptimalState;
use crate::optimal_search::OptimalSearch;
use crate::optimal_short_key::OptimalShortKey;
use crate::optimal_state::OptimalMatchState;
use crate::reference_tree::ReferenceTree;
use crate::sequence_context::SequenceWithOffset;
use crate::ultra2_frame_plan::Ultra2FramePlan;
use crate::ultra2_retained_plan::RetainedUltra2Plan;
use crate::ultra2_retained_state::RetainedUltra2State;
use crate::ultra2_state::Ultra2MatchState;
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
    Greedy(GreedyExecutionPlan),
    Optimal(OptimalExecutionPlan),
    Ultra(OptimalExecutionPlan),
    Ultra2(Ultra2FramePlan),
    #[allow(
        dead_code,
        reason = "Explicit observed schedules share the retained frame dispatch"
    )]
    RetainedOptimal(RetainedOptimalPlan),
    RetainedUltra(RetainedOptimalPlan),
    RetainedUltra2(RetainedUltra2Plan),
}

impl ReferenceSearchPlan {
    pub(crate) fn new(request: &CompressionParameterRequest, source_len: usize) -> Result<Self> {
        Self::new_with_dictionary(request, source_len, 0)
    }

    /// Resolve with the pinned reference's dictionary-size tier input, as
    /// `ZSTD_getCParamsFromCCtxParams` does for an attached dictionary. The
    /// staged content reach stays bounded by the resolved window at emission;
    /// this resolution only adjusts preset selection and table sizing.
    pub(crate) fn new_with_dictionary(
        request: &CompressionParameterRequest,
        source_len: usize,
        dictionary_size: usize,
    ) -> Result<Self> {
        let source_size = u64::try_from(source_len).map_err(|_| Error::ResourceLimit)?;
        if source_size == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        let parameters = request.resolve(SourceSize::Known(source_size), dictionary_size)?;
        Self::from_resolved(parameters, source_len)
    }

    /// Build a plan from an already-resolved profile instead of a request, as
    /// a compiled dictionary does: the profile is fixed once and every later
    /// call must reuse it rather than resolve a different one. Admission,
    /// caps and geometry match `new_with_dictionary` for the same resolved
    /// record and source length.
    pub(crate) fn from_resolved(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
    ) -> Result<Self> {
        match parameters.strategy() {
            ReferenceStrategy::Fast => {
                FastExecutionPlan::from_resolved_parameters(parameters, source_len).map(Self::Fast)
            }
            ReferenceStrategy::DoubleFast => {
                DoubleFastExecutionPlan::from_resolved_parameters(parameters, source_len)
                    .map(Self::DoubleFast)
            }
            ReferenceStrategy::Greedy => {
                GreedyExecutionPlan::from_resolved_parameters(parameters, source_len)
                    .map(Self::Greedy)
            }
            ReferenceStrategy::Lazy | ReferenceStrategy::Lazy2 => {
                GreedyExecutionPlan::from_family_parameters(parameters, source_len)
                    .map(Self::Greedy)
            }
            ReferenceStrategy::BinaryTreeLazy2 => {
                GreedyExecutionPlan::from_tree_parameters(parameters, source_len).map(Self::Greedy)
            }
            ReferenceStrategy::Optimal => {
                if source_len > 16384 {
                    RetainedOptimalPlan::for_frame(parameters, source_len)
                        .map(Self::RetainedOptimal)
                } else {
                    OptimalExecutionPlan::from_resolved_parameters(parameters, source_len)
                        .map(Self::Optimal)
                }
            }
            ReferenceStrategy::Ultra => {
                if source_len > 16384 {
                    RetainedOptimalPlan::for_ultra_frame(parameters, source_len)
                        .map(Self::RetainedUltra)
                } else {
                    OptimalExecutionPlan::from_ultra_parameters(parameters, source_len)
                        .map(Self::Ultra)
                }
            }
            ReferenceStrategy::Ultra2 => {
                if source_len > 16384 {
                    RetainedUltra2Plan::for_frame(parameters, source_len).map(Self::RetainedUltra2)
                } else {
                    Ultra2FramePlan::new(parameters, source_len).map(Self::Ultra2)
                }
            }
        }
    }

    pub(crate) fn source_len(self) -> usize {
        match self {
            Self::Fast(plan) => plan.source_len(),
            Self::DoubleFast(plan) => plan.source_len(),
            Self::Greedy(plan) => plan.source_len(),
            Self::Optimal(plan) | Self::Ultra(plan) => plan.source_len(),
            Self::Ultra2(plan) => plan.source_len(),
            Self::RetainedUltra2(plan) => plan.source_len(),
            Self::RetainedOptimal(plan) | Self::RetainedUltra(plan) => plan.source_len(),
        }
    }
    pub(crate) fn parameters(self) -> ResolvedCompressionParameters {
        match self {
            Self::Fast(plan) => plan.parameters(),
            Self::DoubleFast(plan) => plan.parameters(),
            Self::Greedy(plan) => plan.parameters(),
            Self::Optimal(plan) | Self::Ultra(plan) => plan.parameters(),
            Self::Ultra2(plan) => plan.parameters(),
            Self::RetainedUltra2(plan) => plan.parameters(),
            Self::RetainedOptimal(plan) | Self::RetainedUltra(plan) => plan.parameters(),
        }
    }
    pub(crate) fn block_limit(self) -> usize {
        match self {
            Self::Fast(plan) => plan.block_limit(),
            Self::DoubleFast(plan) => plan.block_limit(),
            Self::Greedy(plan) => plan.block_limit(),
            Self::Optimal(plan) | Self::Ultra(plan) => plan.block_limit(),
            Self::Ultra2(plan) => plan.block_limit(),
            Self::RetainedUltra2(plan) => plan.maximum_block_len(),
            Self::RetainedOptimal(plan) | Self::RetainedUltra(plan) => plan.maximum_block_len(),
        }
    }
    pub(crate) fn next_block_len(self, emitted: usize) -> Result<usize> {
        if let Self::RetainedUltra2(plan) = self {
            return plan.next_block_len(emitted);
        }
        if let Self::RetainedOptimal(plan) | Self::RetainedUltra(plan) = self {
            plan.next_block_len(emitted)
        } else {
            self.source_len()
                .checked_sub(emitted)
                .filter(|&n| n != 0)
                .map(|n| n.min(self.block_limit()))
                .ok_or(Error::InvalidParameter)
        }
    }
    pub(crate) fn is_retained(self) -> bool {
        matches!(
            self,
            Self::RetainedOptimal(_) | Self::RetainedUltra(_) | Self::RetainedUltra2(_)
        )
    }

    /// Supported combined-staging subset: the hash-table profiles and the
    /// complete Greedy family. Optimal-family plans reject a nonzero staged
    /// prefix instead of silently dropping the dictionary.
    pub(crate) fn admits_dictionary_staging(self) -> bool {
        matches!(self, Self::Fast(_) | Self::DoubleFast(_) | Self::Greedy(_))
    }
    pub(crate) fn validate_retained_policy(self) -> Result<()> {
        let (plan, expected) = match self {
            Self::RetainedOptimal(plan) => (plan, crate::optimal_price::PricePolicy::Optimal),
            Self::RetainedUltra(plan) => (plan, crate::optimal_price::PricePolicy::Ultra),
            _ => return Ok(()),
        };
        if plan.policy() != expected {
            return Err(Error::UnsupportedFeature);
        }
        Ok(())
    }
    pub(crate) fn frame_bound(self) -> Result<usize> {
        if let Self::RetainedUltra2(plan) = self {
            return Ok(plan.frame_bound());
        }
        self.validate_retained_policy()?;
        if let Self::RetainedOptimal(plan) | Self::RetainedUltra(plan) = self {
            Ok(plan.frame_bound())
        } else {
            let settings = crate::reference_frame::ReferenceFrameSettings::new(
                self.source_len(),
                self.parameters().window_log(),
            )?;
            crate::reference_frame::ReferenceFrameHeader::raw_frame_bound(&settings)
        }
    }
}

/// Read-only compiled candidate sources for one admitted family.
///
/// The variants carry staged-prefix positions and reject a call that stages a
/// different suffix. A plan must always match its compiled family; the
/// compiled frame path validates that pairing before writing output.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CompiledCandidates<'a> {
    Fast(StagedMatchView<'a>),
    DoubleFast(StagedMatchView<'a>, StagedMatchView<'a>),
    Greedy(GreedyCandidates<'a>),
}

impl CompiledCandidates<'_> {
    /// Whether these candidates belong to the selected plan's family.
    pub(crate) fn matches(self, plan: ReferenceSearchPlan) -> bool {
        matches!(
            (plan, self),
            (ReferenceSearchPlan::Fast(_), Self::Fast(_))
                | (ReferenceSearchPlan::DoubleFast(_), Self::DoubleFast(_, _))
                | (ReferenceSearchPlan::Greedy(_), Self::Greedy(_))
        )
    }
}

/// One selected parser owns its tables and scratch. Committed entropy remains
/// in the enclosing frame workspace; retained search checks a copied publication
/// snapshot before every transition.
/// The enum adds no unchecked representation or allocator erasure.
#[allow(
    clippy::large_enum_variant,
    reason = "Inline owners preserve allocator identities without a dispatch allocation"
)]
pub(crate) enum ReferenceSearchState<'a, A: Allocator> {
    Fast(FastMatchState<'a, A>),
    DoubleFast(DoubleFastMatchState<'a, A>),
    Greedy(GreedyMatchState<'a, A>),
    Optimal(OptimalMatchState<'a, A>),
    Ultra2(Ultra2MatchState<'a, A>),
    RetainedOptimal(RetainedOptimalState<'a, A>),
    RetainedUltra2(RetainedUltra2State<'a, A>),
}

impl<'a, A: Allocator + Clone> ReferenceSearchState<'a, A> {
    /// Borrow the factory, cloning exactly once per selected child owner in
    /// each family's established order, over a dictionary-free source.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "zero-prefix convenience constructor for tests")
    )]
    pub(crate) fn new(source: &'a [u8], plan: ReferenceSearchPlan, factory: &A) -> Result<Self> {
        Self::new_with_prefix(source, 0, plan, factory)
    }

    /// Bind a combined borrow whose first `prefix_len` bytes precede the frame.
    /// A zero prefix is the ordinary dictionary-free binding. Combined staging
    /// is admitted for the Fast profile first; every other profile keeps its
    /// dictionary-free contract until its own stage extends this dispatch, and
    /// a nonzero prefix for it is an explicit unsupported feature rather than
    /// a silent dictionary drop. The factory is borrowed, cloning exactly once
    /// per selected child owner in each family's established order.
    pub(crate) fn new_with_prefix(
        source: &'a [u8],
        prefix_len: usize,
        plan: ReferenceSearchPlan,
        factory: &A,
    ) -> Result<Self> {
        if source
            .len()
            .checked_sub(prefix_len)
            .is_none_or(|frame_len| frame_len != plan.source_len())
        {
            return Err(Error::InvalidParameter);
        }
        plan.validate_retained_policy()?;
        if prefix_len > 0 && !plan.admits_dictionary_staging() {
            // Supported dictionary staging subset: the hash-table profiles and
            // the complete Greedy family. Every optimal-family profile (fresh
            // and retained Optimal/Ultra/Ultra2) is explicitly rejected under
            // the design's supported-subset clause instead of silently
            // dropping the dictionary.
            return Err(Error::UnsupportedFeature);
        }
        let ultra = matches!(plan, ReferenceSearchPlan::Ultra(_));
        let retained_ultra = matches!(plan, ReferenceSearchPlan::RetainedUltra(_));
        if matches!(
            plan,
            ReferenceSearchPlan::Optimal(_) | ReferenceSearchPlan::Ultra(_)
        ) && plan.parameters().strategy()
            != if ultra {
                ReferenceStrategy::Ultra
            } else {
                ReferenceStrategy::Optimal
            }
        {
            return Err(Error::UnsupportedFeature);
        }
        match plan {
            ReferenceSearchPlan::Optimal(plan) | ReferenceSearchPlan::Ultra(plan) => {
                // Exact clone order: frequencies, paths, parser candidates, tree
                // heads, tree edges, short heads, search candidates, literals,
                // sequences. The optional short owner may allocate no storage.
                let parameters = plan.parameters();
                let constructor = if ultra {
                    OptimalParser::from_ultra_fresh_parts
                } else {
                    OptimalParser::from_parts
                };
                let parser = constructor(
                    source,
                    parameters.min_match(),
                    parameters.target_length() as usize,
                    OptimalFrequencies::new_with_policy_in(
                        if ultra {
                            crate::optimal_price::PricePolicy::Ultra
                        } else {
                            crate::optimal_price::PricePolicy::Optimal
                        },
                        false,
                        factory.clone(),
                    )?,
                    OptimalPaths::new_in(factory.clone())?,
                    OptimalMatches::new_in(factory.clone())?,
                )?;
                let layout = plan.layout();
                let search = OptimalSearch::from_parts(
                    source,
                    layout,
                    ReferenceTree::new_in(layout.tree(), factory.clone(), factory.clone())?,
                    OptimalShortKey::new_in(layout.short_log(), factory.clone())?,
                    OptimalMatches::new_in(factory.clone())?,
                )?;
                let compose = if ultra {
                    OptimalMatchState::from_ultra_parts
                } else {
                    OptimalMatchState::from_parts
                };
                Ok(Self::Optimal(compose(
                    source,
                    plan,
                    parser,
                    search,
                    Vec::new_in(factory.clone()),
                    Vec::new_in(factory.clone()),
                )?))
            }
            ReferenceSearchPlan::RetainedOptimal(plan)
            | ReferenceSearchPlan::RetainedUltra(plan) => {
                // Exact retained order: frequencies, paths, parser candidates,
                // tree heads/edges, short heads, search candidates, two scratch owners.
                let parameters = plan.parameters();
                let constructor = if retained_ultra {
                    OptimalParser::from_ultra_retained_parts
                } else {
                    OptimalParser::from_parts
                };
                let parser = constructor(
                    source,
                    parameters.min_match(),
                    parameters.target_length() as usize,
                    OptimalFrequencies::new_with_policy_in(
                        plan.policy(),
                        plan.disabled_literals(),
                        factory.clone(),
                    )?,
                    OptimalPaths::new_in(factory.clone())?,
                    OptimalMatches::new_in(factory.clone())?,
                )?;
                let layout = plan.layout();
                let search = OptimalSearch::from_parts(
                    source,
                    layout,
                    ReferenceTree::new_in(layout.tree(), factory.clone(), factory.clone())?,
                    OptimalShortKey::new_in(layout.short_log(), factory.clone())?,
                    OptimalMatches::new_in(factory.clone())?,
                )?;
                let compose = if retained_ultra {
                    RetainedOptimalState::from_ultra_parts
                } else {
                    RetainedOptimalState::from_parts
                };
                Ok(Self::RetainedOptimal(compose(
                    source,
                    plan,
                    parser,
                    search,
                    Vec::new_in(factory.clone()),
                    Vec::new_in(factory.clone()),
                )?))
            }
            ReferenceSearchPlan::RetainedUltra2(plan) => {
                // Exact retained order: frequencies, paths, parser candidates,
                // tree heads/edges, short heads, search candidates, two scratch owners.
                let parameters = plan.parameters();
                let constructor = OptimalParser::from_ultra2_retained_parts;
                let parser = constructor(
                    source,
                    parameters.min_match(),
                    parameters.target_length() as usize,
                    OptimalFrequencies::new_with_policy_in(
                        crate::optimal_price::PricePolicy::Ultra,
                        plan.disabled_literals(),
                        factory.clone(),
                    )?,
                    OptimalPaths::new_in(factory.clone())?,
                    OptimalMatches::new_in(factory.clone())?,
                )?;
                let layout = plan.layout();
                let search = OptimalSearch::from_ultra2_retained_parts(
                    source,
                    plan,
                    ReferenceTree::new_in(layout.tree(), factory.clone(), factory.clone())?,
                    OptimalShortKey::new_in(layout.short_log(), factory.clone())?,
                    OptimalMatches::new_in(factory.clone())?,
                )?;
                Ok(Self::RetainedUltra2(RetainedUltra2State::from_parts(
                    source,
                    plan,
                    parser,
                    search,
                    Vec::new_in(factory.clone()),
                    Vec::new_in(factory.clone()),
                )?))
            }
            ReferenceSearchPlan::Ultra2(plan) => {
                // Preserve the nine qualified owner identities and clone order.
                let execution = plan.execution();
                let parameters = plan.parameters();
                let parser = OptimalParser::from_ultra2_parts(
                    source,
                    parameters.min_match(),
                    parameters.target_length() as usize,
                    OptimalFrequencies::new_with_policy_in(
                        crate::optimal_price::PricePolicy::Ultra,
                        false,
                        factory.clone(),
                    )?,
                    OptimalPaths::new_in(factory.clone())?,
                    OptimalMatches::new_in(factory.clone())?,
                )?;
                let layout = execution.layout();
                let search = OptimalSearch::from_ultra2_parts(
                    source,
                    layout,
                    ReferenceTree::new_in(layout.tree(), factory.clone(), factory.clone())?,
                    OptimalShortKey::new_in(layout.short_log(), factory.clone())?,
                    OptimalMatches::new_in(factory.clone())?,
                )?;
                Ok(Self::Ultra2(Ultra2MatchState::from_parts(
                    source,
                    execution,
                    parser,
                    search,
                    Vec::new_in(factory.clone()),
                    Vec::new_in(factory.clone()),
                )?))
            }
            ReferenceSearchPlan::Fast(plan) => {
                let table = MatchTable::with_layout_in(plan.layout(), factory.clone())?;
                let literals = Vec::new_in(factory.clone());
                let sequences = Vec::new_in(factory.clone());
                Ok(Self::Fast(FastMatchState::from_parts_with_prefix(
                    source, plan, table, literals, sequences, prefix_len,
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
                Ok(Self::DoubleFast(
                    DoubleFastMatchState::from_parts_with_prefix(
                        source, plan, tables, literals, sequences, prefix_len,
                    )?,
                ))
            }
            ReferenceSearchPlan::Greedy(plan) => {
                let tables = GreedyTables::new_in(plan.tables(), factory.clone(), factory.clone())?;
                let literals = Vec::new_in(factory.clone());
                let sequences = Vec::new_in(factory.clone());
                Ok(Self::Greedy(GreedyMatchState::from_parts_with_prefix(
                    source, plan, tables, literals, sequences, prefix_len,
                )?))
            }
        }
    }

    /// Bind per-call owners to compiled candidate sources.
    ///
    /// The combined borrow must stage exactly the compiled suffix length, the
    /// plan must belong to the candidate family, and every layout must match
    /// before any owner is reset. Unlike [`Self::new_with_prefix`], no table
    /// seeding pass runs: the compiled owners already hold the loaded suffix.
    pub(crate) fn new_with_compiled(
        source: &'a [u8],
        prefix_len: usize,
        plan: ReferenceSearchPlan,
        factory: &A,
        candidates: CompiledCandidates<'a>,
    ) -> Result<Self> {
        if source
            .len()
            .checked_sub(prefix_len)
            .is_none_or(|frame_len| frame_len != plan.source_len())
            || prefix_len == 0
            || !plan.admits_dictionary_staging()
            || !candidates.matches(plan)
        {
            return Err(Error::InvalidParameter);
        }
        match (plan, candidates) {
            (ReferenceSearchPlan::Fast(plan), CompiledCandidates::Fast(dictionary)) => {
                let table = MatchTable::with_layout_in(plan.layout(), factory.clone())?;
                let literals = Vec::new_in(factory.clone());
                let sequences = Vec::new_in(factory.clone());
                Ok(Self::Fast(FastMatchState::from_parts_with_compiled_prefix(
                    source,
                    plan,
                    table,
                    literals,
                    sequences,
                    prefix_len,
                    Some(dictionary),
                )?))
            }
            (
                ReferenceSearchPlan::DoubleFast(plan),
                CompiledCandidates::DoubleFast(long, short),
            ) => {
                let tables = DoubleFastTables::new_in(
                    plan.long_layout(),
                    plan.short_layout(),
                    factory.clone(),
                    factory.clone(),
                )?;
                let literals = Vec::new_in(factory.clone());
                let sequences = Vec::new_in(factory.clone());
                Ok(Self::DoubleFast(
                    DoubleFastMatchState::from_parts_with_compiled_prefix(
                        source,
                        plan,
                        tables,
                        literals,
                        sequences,
                        prefix_len,
                        Some((long, short)),
                    )?,
                ))
            }
            (ReferenceSearchPlan::Greedy(plan), CompiledCandidates::Greedy(dictionary)) => {
                let tables = GreedyTables::new_in(plan.tables(), factory.clone(), factory.clone())?;
                let literals = Vec::new_in(factory.clone());
                let sequences = Vec::new_in(factory.clone());
                Ok(Self::Greedy(
                    GreedyMatchState::from_parts_with_compiled_prefix(
                        source,
                        plan,
                        tables,
                        literals,
                        sequences,
                        prefix_len,
                        Some(dictionary),
                    )?,
                ))
            }
            _ => Err(Error::InvalidParameter),
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
            Self::Greedy(state) => state
                .parse_next(length, committed)
                .map(|candidate| candidate.is_some()),
            Self::Optimal(state) => state.parse_next(length, committed),
            Self::Ultra2(state) => state.parse_next(length, committed),
            Self::RetainedOptimal(state) => state.parse_next(length, committed),
            Self::RetainedUltra2(state) => state.parse_next(length, committed),
        }
    }
    pub(crate) fn literals(&self) -> &[u8] {
        match self {
            Self::Fast(state) => state.literals(),
            Self::DoubleFast(state) => state.literals(),
            Self::Greedy(state) => state.literals(),
            Self::Optimal(state) => state.literals(),
            Self::Ultra2(state) => state.literals(),
            Self::RetainedOptimal(state) => state.literals(),
            Self::RetainedUltra2(state) => state.literals(),
        }
    }
    pub(crate) fn sequences(&self) -> &[SequenceWithOffset] {
        match self {
            Self::Fast(state) => state.sequences(),
            Self::DoubleFast(state) => state.sequences(),
            Self::Greedy(state) => state.sequences(),
            Self::Optimal(state) => state.sequences(),
            Self::Ultra2(state) => state.sequences(),
            Self::RetainedOptimal(state) => state.sequences(),
            Self::RetainedUltra2(state) => state.sequences(),
        }
    }
    pub(crate) fn validate_publication(&self, kind: Publication, after: [u32; 3]) -> Result<()> {
        if let Self::RetainedUltra2(state) = self {
            state.validate_publication(kind, after)
        } else if let Self::RetainedOptimal(state) = self {
            state.validate_publication(kind, after)
        } else {
            Ok(())
        }
    }
    pub(crate) fn publish(&mut self, kind: Publication, after: [u32; 3]) -> Result<()> {
        if let Self::RetainedUltra2(state) = self {
            state.publish(kind, after)
        } else if let Self::RetainedOptimal(state) = self {
            state.publish(kind, after)
        } else {
            Ok(())
        }
    }
    #[cfg(test)]
    pub(crate) fn reset_retained(&mut self) -> Result<()> {
        if let Self::RetainedUltra2(state) = self {
            state.reset()
        } else if let Self::RetainedOptimal(state) = self {
            state.reset()
        } else {
            Err(Error::InvalidState)
        }
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        match self {
            Self::Fast(state) => state.retained_capacity(),
            Self::DoubleFast(state) => state.retained_capacity(),
            Self::Greedy(state) => state.retained_capacity(),
            Self::Optimal(state) => state.retained_capacity(),
            Self::Ultra2(state) => state.retained_capacity(),
            Self::RetainedOptimal(state) => state.retained_capacity(),
            Self::RetainedUltra2(state) => state.retained_capacity(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompressionParameterRequest, RawCompressionParameters};
    use alloc::alloc::Global;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn plans(size: usize) -> [ReferenceSearchPlan; 9] {
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_window_log(10).unwrap();
        request.set_hash_log(10).unwrap();
        request.set_chain_log(9).unwrap();
        request.set_min_match(4).unwrap();
        let fast = FastExecutionPlan::new(&request, size).unwrap();
        request.set_strategy(Some(crate::ReferenceStrategy::DoubleFast));
        let double = DoubleFastExecutionPlan::new(&request, size).unwrap();
        let greedy = |window_log, strategy| {
            ReferenceSearchPlan::Greedy(
                GreedyExecutionPlan::from_family_parameters(
                    RawCompressionParameters {
                        window_log,
                        chain_log: 9,
                        hash_log: 10,
                        search_log: 4,
                        min_match: 4,
                        target_length: 0,
                        strategy,
                    }
                    .validate()
                    .unwrap(),
                    size,
                )
                .unwrap(),
            )
        };
        [
            ReferenceSearchPlan::Fast(fast),
            ReferenceSearchPlan::DoubleFast(double),
            greedy(10, 3),
            greedy(15, 3),
            greedy(10, 4),
            greedy(15, 4),
            greedy(10, 5),
            greedy(15, 5),
            ReferenceSearchPlan::Optimal(
                OptimalExecutionPlan::from_resolved_parameters(
                    RawCompressionParameters {
                        window_log: 10,
                        chain_log: 6,
                        hash_log: 6,
                        search_log: 1,
                        min_match: 3,
                        target_length: 0,
                        strategy: 7,
                    }
                    .validate()
                    .unwrap(),
                    size,
                )
                .unwrap(),
            ),
        ]
    }

    #[test]
    fn resolved_optimal_family_plans_keep_rejecting_a_nonzero_staged_prefix() {
        // A compiled call builds its plan through `from_resolved`; every
        // fresh and retained optimal-family plan must keep rejecting a
        // nonzero staged prefix with `UnsupportedFeature` before any
        // allocation, exactly like the request-driven constructor's frame
        // paths. Retained plans only exist for sources of 16385..=32768
        // under the qualified retained geometry (window <= 16, hash and
        // chain <= 12, search <= 6).
        for strategy in [
            crate::ReferenceStrategy::Optimal,
            crate::ReferenceStrategy::Ultra,
            crate::ReferenceStrategy::Ultra2,
        ] {
            let mut request = CompressionParameterRequest::with_level(13);
            request.set_strategy(Some(strategy));
            request.set_window_log(16).unwrap();
            request.set_hash_log(12).unwrap();
            request.set_chain_log(12).unwrap();
            request.set_search_log(6).unwrap();
            let retained_parameters = request.resolve(SourceSize::Known(20_000), 0).unwrap();
            assert_eq!(retained_parameters.strategy(), strategy);
            let retained = ReferenceSearchPlan::from_resolved(retained_parameters, 20_000).unwrap();
            assert!(retained.is_retained(), "{strategy:?}");
            assert!(!retained.admits_dictionary_staging(), "{strategy:?}");
            let fresh_parameters = request.resolve(SourceSize::Known(16_384), 0).unwrap();
            assert_eq!(fresh_parameters.strategy(), strategy);
            let fresh = ReferenceSearchPlan::from_resolved(fresh_parameters, 16_384).unwrap();
            assert!(!fresh.is_retained(), "{strategy:?}");
            assert!(!fresh.admits_dictionary_staging(), "{strategy:?}");
            // A staged prefix of dictionary bytes in front of the frame must
            // reject before any owner is allocated, and the same plan binds a
            // zero prefix (the ordinary dictionary-free path).
            let mut combined = alloc::vec![b'd'; 64];
            combined.extend_from_slice(&alloc::vec![b'x'; 20_000]);
            let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = owner.monitor();
            assert!(
                matches!(
                    ReferenceSearchState::new_with_prefix(&combined, 64, retained, &owner),
                    Err(Error::UnsupportedFeature)
                ),
                "{strategy:?}"
            );
            assert_eq!(monitor.snapshot().attempts, 0);
            let frame = &combined[64..];
            assert!(
                ReferenceSearchState::new_with_prefix(frame, 0, retained, &owner).is_ok(),
                "{strategy:?}"
            );
            assert!(
                matches!(
                    ReferenceSearchState::new_with_prefix(
                        &combined[..16_384 + 64],
                        64,
                        fresh,
                        &owner
                    ),
                    Err(Error::UnsupportedFeature)
                ),
                "{strategy:?}"
            );
            assert!(
                ReferenceSearchState::new_with_prefix(&combined[64..16_384 + 64], 0, fresh, &owner)
                    .is_ok(),
                "{strategy:?}"
            );
        }
    }

    #[test]
    fn resolved_profile_rebuilds_every_dispatch_branch_of_the_request_plan() {
        // A compiled dictionary fixes one resolved profile at construction and
        // every later call must rebuild its plan from that exact record.
        // Levels 1/3/4/6/9/11 dispatch Fast, DoubleFast, Greedy, Lazy, Lazy2,
        // BinaryTreeLazy2 and the Optimal family beside fresh and retained
        // optimal schedules; each resolved record must rebuild a plan with the
        // same profile, source length, block limit and block schedule as the
        // request-driven constructor for every checked source size.
        for level in [1, 3, 4, 6, 9, 11] {
            for size in [0usize, 100, 16_384, 16_385, 65_536] {
                let request = CompressionParameterRequest::with_level(level);
                let expected = match ReferenceSearchPlan::new(&request, size) {
                    Ok(plan) => plan,
                    Err(error) => {
                        assert_eq!(
                            ReferenceSearchPlan::from_resolved(
                                request
                                    .resolve(SourceSize::Known(u64::try_from(size).unwrap()), 0)
                                    .unwrap(),
                                size
                            )
                            .map(|plan| (
                                plan.parameters(),
                                plan.source_len(),
                                plan.block_limit()
                            )),
                            Err(error),
                            "level {level} size {size}"
                        );
                        continue;
                    }
                };
                let resolved = request
                    .resolve(SourceSize::Known(u64::try_from(size).unwrap()), 0)
                    .unwrap();
                let plan = ReferenceSearchPlan::from_resolved(resolved, size)
                    .unwrap_or_else(|error| panic!("level {level} size {size}: {error:?}"));
                assert_eq!(plan.parameters(), expected.parameters());
                assert_eq!(plan.source_len(), expected.source_len());
                assert_eq!(plan.block_limit(), expected.block_limit());
                let mut emitted = 0;
                loop {
                    assert_eq!(
                        plan.next_block_len(emitted),
                        expected.next_block_len(emitted),
                        "level {level} size {size} emitted {emitted}"
                    );
                    match plan.next_block_len(emitted) {
                        Ok(next) => emitted += next,
                        Err(_) => break,
                    }
                }
            }
        }
    }

    #[test]
    fn retained_plans_from_resolved_records_reject_nonzero_staged_prefixes() {
        // The frame paths rebuild retained plans from an explicit resolved
        // record when a compiled dictionary supplies one profile. The
        // supported-subset boundary must still reject a nonzero staged prefix
        // for every retained optimal-family plan before any parser works.
        for strategy in [
            crate::ReferenceStrategy::Optimal,
            crate::ReferenceStrategy::Ultra,
            crate::ReferenceStrategy::Ultra2,
        ] {
            let source_len = 20_000;
            let mut request = CompressionParameterRequest::with_level(11);
            request.set_strategy(Some(strategy));
            request.set_window_log(16).unwrap();
            request.set_hash_log(12).unwrap();
            request.set_chain_log(12).unwrap();
            request.set_search_log(6).unwrap();
            request.set_min_match(3).unwrap();
            request.set_target_length(1).unwrap();
            let resolved = request
                .resolve(SourceSize::Known(source_len as u64), 0)
                .unwrap();
            let plan = ReferenceSearchPlan::from_resolved(resolved, source_len).unwrap();
            assert!(plan.is_retained(), "{strategy:?}");
            assert!(!plan.admits_dictionary_staging(), "{strategy:?}");
            let staged = alloc::vec![0_u8; source_len + 512];
            assert_eq!(
                ReferenceSearchState::new_with_prefix(&staged, 512, plan, &Global).err(),
                Some(Error::UnsupportedFeature),
                "{strategy:?}"
            );
        }
    }

    #[test]
    fn retained_ultra_plan_cannot_allocate_through_ordinary_dispatch() {
        let input = [b'x'; 128];
        let plan = ReferenceSearchPlan::RetainedOptimal(
            RetainedOptimalPlan::new_ultra(
                RawCompressionParameters {
                    window_log: 10,
                    hash_log: 6,
                    chain_log: 6,
                    search_log: 1,
                    min_match: 3,
                    target_length: 0,
                    strategy: 8,
                }
                .validate()
                .unwrap(),
                input.len(),
                &[64, 64],
                false,
            )
            .unwrap(),
        );
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        assert_eq!(plan.frame_bound(), Err(Error::UnsupportedFeature));
        assert!(matches!(
            ReferenceSearchState::new(&input, plan, &owner),
            Err(Error::UnsupportedFeature)
        ));
        assert!(matches!(
            crate::reference_frame_encode::compress_reference_in(&input, plan, &owner),
            Err(Error::UnsupportedFeature)
        ));
        assert_eq!(monitor.snapshot(), before);
        drop(owner);
        monitor.assert_released();
    }

    #[test]
    fn retained_ultra_factory_preserves_exact_nine_child_identities_and_bounds() {
        let input = [b'x'; 128];
        for minimum in [3, 4] {
            let bounds = RetainedOptimalPlan::new_ultra(
                RawCompressionParameters {
                    window_log: 10,
                    hash_log: 6,
                    chain_log: 6,
                    search_log: 1,
                    min_match: minimum,
                    target_length: 0,
                    strategy: 8,
                }
                .validate()
                .unwrap(),
                input.len(),
                &[64, 64],
                false,
            )
            .unwrap();
            let factory = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = factory.monitor();
            let mut state = ReferenceSearchState::new(
                &input,
                ReferenceSearchPlan::RetainedUltra(bounds),
                &factory,
            )
            .unwrap();
            let initial = monitor.snapshot();
            assert_eq!(initial.identities_created, 10);
            assert_eq!(initial.attempts, if minimum == 3 { 7 } else { 6 });
            assert_eq!(
                initial.live_requested_bytes,
                bounds.owner_bytes()[..7].iter().sum::<usize>() as u128
            );
            assert!(state.parse_next(64, [1, 4, 8]).unwrap());
            let pending = monitor.snapshot();
            assert_eq!(pending.identities_created, 10);
            assert_eq!(pending.attempts, if minimum == 3 { 9 } else { 8 });
            assert_eq!(
                pending.live_requested_bytes,
                bounds.owner_bytes().iter().sum::<usize>() as u128
            );
            assert_eq!(
                state.retained_capacity(),
                bounds.owner_bytes().iter().sum::<usize>()
            );
            state.publish(Publication::Raw, [1, 4, 8]).unwrap();
            assert!(monitor.fail_next());
            state.reset_retained().unwrap();
            assert_eq!(monitor.snapshot(), pending);
            monitor.clear_failure();
            drop(state);
            monitor.assert_released();
            assert_eq!(monitor.snapshot().owners_dropped, 9);
            drop(factory);
            assert_eq!(monitor.snapshot().owners_dropped, 10);
        }
    }

    #[test]
    fn retained_ultra_dispatch_rejects_an_ordinary_plan_before_allocator_contact() {
        let input = [b'x'; 128];
        let bounds = RetainedOptimalPlan::new(
            RawCompressionParameters {
                window_log: 10,
                hash_log: 6,
                chain_log: 6,
                search_log: 1,
                min_match: 3,
                target_length: 0,
                strategy: 7,
            }
            .validate()
            .unwrap(),
            input.len(),
            &[64, 64],
            false,
        )
        .unwrap();
        let plan = ReferenceSearchPlan::RetainedUltra(bounds);
        let factory = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = factory.monitor();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        assert_eq!(plan.frame_bound(), Err(Error::UnsupportedFeature));
        assert!(matches!(
            ReferenceSearchState::new(&input, plan, &factory),
            Err(Error::UnsupportedFeature)
        ));
        assert!(matches!(
            crate::reference_frame_encode::compress_reference_in(&input, plan, &factory),
            Err(Error::UnsupportedFeature)
        ));
        assert_eq!(monitor.snapshot(), before);
        drop(factory);
        monitor.assert_released();
    }

    #[test]
    fn only_selected_tables_allocate_and_each_child_retains_its_owner() {
        let input = [b'x'; 64];
        for plan in plans(input.len()) {
            let (table_count, owner_count, bytes) = match plan {
                ReferenceSearchPlan::Ultra2(p) => (7, 10, p.owner_bytes()[..7].iter().sum()),
                ReferenceSearchPlan::RetainedUltra2(p) => {
                    (7, 10, p.owner_bytes()[..7].iter().sum())
                }
                ReferenceSearchPlan::RetainedOptimal(p) | ReferenceSearchPlan::RetainedUltra(p) => {
                    (7, 10, p.owner_bytes()[..7].iter().sum())
                }
                ReferenceSearchPlan::Fast(p) => (1, 4, p.table_bytes()),
                ReferenceSearchPlan::DoubleFast(p) => (2, 5, p.table_bytes()),
                ReferenceSearchPlan::Greedy(p) => (2, 5, p.table_bytes()),
                ReferenceSearchPlan::Optimal(p) | ReferenceSearchPlan::Ultra(p) => {
                    (7, 10, p.owner_bytes()[..7].iter().sum())
                }
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
    fn public_greedy_dispatch_uses_the_actual_resolved_source_family() {
        let mut request = CompressionParameterRequest::with_level(3);
        request.set_strategy(Some(ReferenceStrategy::Greedy));
        for size in [0, 64, 16384, 16385, 32768, 131072, 262144] {
            let ReferenceSearchPlan::Greedy(plan) =
                ReferenceSearchPlan::new(&request, size).unwrap()
            else {
                panic!("wrong selected strategy");
            };
            assert_eq!(plan.tables().is_row(), size > 16384);
            assert_eq!(
                plan.parameters(),
                request.resolve(SourceSize::Known(size as u64), 0).unwrap()
            );
        }
    }

    #[test]
    fn private_greedy_dispatch_keeps_exact_selected_owners_and_candidates() {
        for row in [false, true] {
            let cases = crate::greedy_frame_oracle_fixture::cases().unwrap();
            let name = if row {
                "row_mixed_fallback"
            } else {
                "chain_mixed_fallback"
            };
            let case = cases.iter().find(|c| c.name == name).unwrap();
            let parameters = RawCompressionParameters {
                window_log: u32::from(case.window_log),
                chain_log: u32::from(case.chain_log),
                hash_log: u32::from(case.hash_log),
                search_log: u32::from(case.search_log),
                min_match: u32::from(case.min_match),
                target_length: case.target_length,
                strategy: 3,
            }
            .validate()
            .unwrap();
            let plan = ReferenceSearchPlan::Greedy(
                GreedyExecutionPlan::from_resolved_parameters(parameters, case.source.len())
                    .unwrap(),
            );
            assert_eq!(plan.parameters(), parameters);
            assert_eq!(plan.source_len(), case.source.len());
            let mut state = ReferenceSearchState::new(case.source, plan, &Global).unwrap();
            assert!(matches!(state, ReferenceSearchState::Greedy(_)));
            let mut committed = [1, 4, 8];
            for block in &case.blocks {
                assert_eq!(
                    state.parse_next(block.length, committed).unwrap(),
                    block.candidate.is_some()
                );
                if let Some(candidate) = &block.candidate {
                    assert_eq!(state.literals(), candidate.literals);
                    assert_eq!(state.sequences().len(), candidate.sequences.len());
                    let mut trial = committed;
                    for (got, want) in state.sequences().iter().zip(&candidate.sequences) {
                        let s = got.sequence;
                        let offset = got
                            .offset
                            .resolve(s.literal_length, s.distance, trial)
                            .unwrap();
                        assert_eq!(
                            (
                                s.literal_length,
                                s.match_length,
                                s.distance,
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

#[cfg(test)]
mod ultra_dispatch_tests {
    use super::*;
    use crate::RawCompressionParameters;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    fn plan(strategy: u32) -> OptimalExecutionPlan {
        let p = RawCompressionParameters {
            window_log: 10,
            hash_log: 6,
            chain_log: 6,
            search_log: 1,
            min_match: 3,
            target_length: 1,
            strategy,
        }
        .validate()
        .unwrap();
        if strategy == 8 {
            OptimalExecutionPlan::from_ultra_parameters(p, 64).unwrap()
        } else {
            OptimalExecutionPlan::from_resolved_parameters(p, 64).unwrap()
        }
    }
    #[test]
    fn private_ultra_preserves_exact_nine_clone_order_and_borrowed_identity() {
        let source = [b'a'; 64];
        let p = plan(8);
        let factory = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = factory.monitor();
        let mut state =
            ReferenceSearchState::new(&source, ReferenceSearchPlan::Ultra(p), &factory).unwrap();
        let before = monitor.snapshot();
        assert_eq!(before.identities_created, 10);
        assert_eq!(before.attempts, 7);
        assert_eq!(before.live_blocks, 7);
        assert_eq!(
            before.live_requested_bytes,
            p.owner_bytes()[..7].iter().sum::<usize>() as u128
        );
        state.parse_next(64, [1, 4, 8]).unwrap();
        assert_eq!(monitor.snapshot().attempts, 9);
        drop(state);
        drop(factory);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 10);
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut state =
            ReferenceSearchState::new(&source, ReferenceSearchPlan::Ultra(p), &&owner).unwrap();
        state.parse_next(64, [1, 4, 8]).unwrap();
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(state);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(owner);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
    #[test]
    fn forged_variant_strategy_and_source_fail_before_any_owner_clone() {
        let source = [b'a'; 64];
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        for p in [
            ReferenceSearchPlan::Optimal(plan(8)),
            ReferenceSearchPlan::Ultra(plan(7)),
        ] {
            assert!(matches!(
                ReferenceSearchState::new(&source, p, &owner),
                Err(Error::UnsupportedFeature)
            ));
            assert_eq!(monitor.snapshot(), before);
        }
        assert!(matches!(
            ReferenceSearchState::new(&source[..63], ReferenceSearchPlan::Ultra(plan(8)), &owner),
            Err(Error::InvalidParameter)
        ));
        assert_eq!(monitor.snapshot(), before);
    }
}

#[cfg(test)]
mod ultra2_dispatch_tests {
    use super::*;
    use crate::RawCompressionParameters;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    fn plan() -> Ultra2FramePlan {
        Ultra2FramePlan::new(
            RawCompressionParameters {
                window_log: 10,
                hash_log: 6,
                chain_log: 6,
                search_log: 1,
                min_match: 3,
                target_length: 1,
                strategy: 9,
            }
            .validate()
            .unwrap(),
            64,
        )
        .unwrap()
    }
    #[test]
    fn ultra2_preserves_exact_nine_clone_order_and_borrowed_identity() {
        let source = [b'a'; 64];
        let p = plan();
        let factory = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = factory.monitor();
        let mut state =
            ReferenceSearchState::new(&source, ReferenceSearchPlan::Ultra2(p), &factory).unwrap();
        let before = monitor.snapshot();
        assert_eq!(before.identities_created, 10);
        assert_eq!(before.attempts, 7);
        assert_eq!(before.live_blocks, 7);
        assert_eq!(
            before.live_requested_bytes,
            p.owner_bytes()[..7].iter().sum::<usize>() as u128
        );
        state.parse_next(64, [1, 4, 8]).unwrap();
        assert_eq!(monitor.snapshot().attempts, 9);
        drop(state);
        drop(factory);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 10);
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut state =
            ReferenceSearchState::new(&source, ReferenceSearchPlan::Ultra2(p), &&owner).unwrap();
        state.parse_next(64, [1, 4, 8]).unwrap();
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(state);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(owner);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }

    #[test]
    fn ultra2_wrong_source_precedes_every_factory_clone() {
        let source = [b'a'; 63];
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        assert!(matches!(
            ReferenceSearchState::new(&source, ReferenceSearchPlan::Ultra2(plan()), &owner),
            Err(Error::InvalidParameter)
        ));
        assert_eq!(monitor.snapshot(), before);
    }
}

#[cfg(test)]
mod retained_ultra2_integration_dispatch {
    use super::*;
    use crate::RawCompressionParameters;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    fn plan(minimum: u32) -> RetainedUltra2Plan {
        RetainedUltra2Plan::new(
            RawCompressionParameters {
                window_log: 10,
                hash_log: 6,
                chain_log: 6,
                search_log: 1,
                min_match: minimum,
                target_length: 0,
                strategy: 9,
            }
            .validate()
            .unwrap(),
            128,
            &[64, 64],
            false,
        )
        .unwrap()
    }
    #[test]
    fn retained_ultra2_integration_factory_clones_exact_nine_owners_and_reuses_them() {
        let source = [b'a'; 128];
        for minimum in [3, 4] {
            let p = plan(minimum);
            let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = owner.monitor();
            let mut state =
                ReferenceSearchState::new(&source, ReferenceSearchPlan::RetainedUltra2(p), &owner)
                    .unwrap();
            let before = monitor.snapshot();
            assert_eq!(before.identities_created, 10);
            assert_eq!(before.attempts, if minimum == 3 { 7 } else { 6 });
            assert_eq!(
                before.live_requested_bytes,
                p.owner_bytes()[..7].iter().sum::<usize>() as u128
            );
            for _ in 0..2 {
                state.parse_next(64, [1, 4, 8]).unwrap();
                state.publish(Publication::Raw, [1, 4, 8]).unwrap();
            }
            let complete = monitor.snapshot();
            state.reset_retained().unwrap();
            assert_eq!(monitor.snapshot(), complete);
            for _ in 0..2 {
                state.parse_next(64, [1, 4, 8]).unwrap();
                state.publish(Publication::Raw, [1, 4, 8]).unwrap();
            }
            assert_eq!(monitor.snapshot().attempts, complete.attempts);
            drop(state);
            monitor.assert_released();
            drop(owner);
            assert_eq!(monitor.snapshot().owners_dropped, 10);
            assert!(!monitor.snapshot().violations.any());
        }
    }
    #[test]
    fn retained_ultra2_integration_dispatch_rejects_source_before_allocator_contact_and_keeps_default_limits()
     {
        let source = [b'a'; 128];
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        assert!(matches!(
            ReferenceSearchState::new(
                &source[..127],
                ReferenceSearchPlan::RetainedUltra2(plan(3)),
                &owner
            ),
            Err(Error::InvalidParameter)
        ));
        assert_eq!(monitor.snapshot(), before);
        monitor.clear_failure();
        drop(owner);
        monitor.assert_released();
        let mut request = CompressionParameterRequest::with_level(13);
        request.set_strategy(Some(ReferenceStrategy::Ultra2));
        assert!(matches!(
            ReferenceSearchPlan::new(&request, 32768),
            Err(Error::ResourceLimit)
        ));
    }
}
