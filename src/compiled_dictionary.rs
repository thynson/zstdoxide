//! Immutable compiled dictionary state over seeded reference tables.
//!
//! One state owns the resolved scalar profile, per-profile seed tables over the
//! loaded dictionary suffix, prepared formatted-dictionary entropy, and a borrow
//! of immutable dictionary content. Construction validates admission before the
//! first allocation and seeds every table exactly once; the tables stay
//! immutable during calls and hold content-relative positions over the loaded
//! suffix. A call stages exactly that suffix before its input, so a compiled
//! table is consulted as a second read-only candidate source beside the call's
//! own mutable table without any position translation. Public emission and the
//! remaining strategy families follow in later items.
//!
//! Upstream counterparts are pinned `ZSTD_CDict` ownership and loading:
//! `ZSTD_createCDict_advanced2` resolves once with an unknown source size and
//! the dictionary size (`zstd_compress.c:5660-5707`), `ZSTD_initCDict_internal`
//! loads entropy and content once (`:5551-5604`), and
//! `ZSTD_loadDictionaryContent` truncates to the loaded suffix whose end
//! anchors window expiry (`:4898-4990`). Native reuse cutoffs, dedicated
//! dictionary search, reset directives and parameter lifecycle are not implied.
#![forbid(unsafe_code)]

use crate::dictionary::DictionaryEntropyView;
use crate::double_fast_plan::DoubleFastExecutionPlan;
use crate::double_fast_tables::DoubleFastTables;
use crate::fast_plan::FastExecutionPlan;
use crate::greedy_plan::GreedyExecutionPlan;
use crate::greedy_state::GreedyTables;
use crate::huffman_codebook::HuffmanCodebook;
use crate::matchfinder::{MatchTable, StagedMatchView};
use crate::reference_chain::StagedChainView;
use crate::row_table::StagedRowView;
use crate::{
    CompressionParameterRequest, DictionaryView, Error, ReferenceStrategy,
    ResolvedCompressionParameters, Result, SourceSize,
};
use alloc::alloc::Allocator;

/// The seeded table owners of one compiled profile.
///
/// Each owner is the same allocator-backed family the per-call parsers use and
/// keeps its independent opt-in unsafe features. Positions index the loaded
/// suffix, never the frame input.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "public compiled-dictionary emission follows")
)]
pub(crate) enum CompiledTables<A: Allocator> {
    Fast(MatchTable<A>),
    DoubleFast(DoubleFastTables<A, A>),
    Greedy(GreedyTables<A>),
}

/// Prepared entropy borrowed from a formatted dictionary, plus the first
/// block's literal encoding table derived once at construction.
///
/// The view borrows the dictionary owner like `ZSTD_CDict`'s by-reference
/// content; the codebook is plain inline values and allocates nothing. Raw
/// dictionaries compile no entropy at all.
struct CompiledEntropy<'a> {
    view: DictionaryEntropyView<'a>,
    literal_table: HuffmanCodebook,
}

/// An immutable compiled dictionary: one resolved profile, one seeded table
/// set, one borrowed content view.
///
/// The view borrows the dictionary owner for `'a`; the state owns no source,
/// destination, frame, attempt or live encoder state. Sharing uses an outer
/// `Arc` without cloning tables or content. No method mutates the tables after
/// construction returns.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "public compiled-dictionary emission follows")
)]
pub(crate) struct CompiledDictionary<'a, A: Allocator = alloc::alloc::Global> {
    view: DictionaryView<'a>,
    parameters: ResolvedCompressionParameters,
    loaded_len: usize,
    tables: CompiledTables<A>,
    entropy: Option<CompiledEntropy<'a>>,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "public compiled-dictionary emission follows")
)]
impl<'a, A: Allocator + Clone> CompiledDictionary<'a, A> {
    /// Compile one dictionary view for one request.
    ///
    /// The profile is resolved exactly once with an unknown source size and the
    /// dictionary's matchable content length, mirroring native CDict creation.
    /// The loaded suffix caps at the resolved window; its length fixes the
    /// content-relative table positions. Admission rejects Optimal, Ultra and
    /// Ultra2 with [`Error::UnsupportedFeature`] before the first allocation
    /// instead of dropping the dictionary or substituting a strategy.
    /// Resolution errors also precede every allocation.
    pub(crate) fn new(
        view: DictionaryView<'a>,
        request: &CompressionParameterRequest,
        allocator: A,
    ) -> Result<Self> {
        // Empty content never builds compiled tables: the dictionary-free
        // parameterized bytes already cover it exactly.
        if view.content().is_empty() {
            return Err(Error::InvalidParameter);
        }
        let parameters = request.resolve(SourceSize::Unknown, view.content().len())?;
        match parameters.strategy() {
            ReferenceStrategy::Fast
            | ReferenceStrategy::DoubleFast
            | ReferenceStrategy::Greedy
            | ReferenceStrategy::Lazy
            | ReferenceStrategy::Lazy2
            | ReferenceStrategy::BinaryTreeLazy2 => {}
            // Nonzero dictionary content is exactly the unsupported subset of
            // the Module 36 contract; empty content keeps the dictionary-free
            // parameterized bytes and never reaches a compiled table consumer.
            _ => return Err(Error::UnsupportedFeature),
        }
        let content = view.content();
        // A formatted dictionary's prepared entropy is borrowed, and its
        // first-block literal encoding table is built once here. Both steps
        // allocate nothing and precede the first table request, so a malformed
        // prepared tree fails without leaving an owner to release.
        let entropy = match view.entropy() {
            Some(entropy) => Some(CompiledEntropy {
                view: entropy,
                literal_table: HuffmanCodebook::from_lengths(&entropy.huffman.code_lengths(), 12)?,
            }),
            None => None,
        };
        let window = 1_usize
            .checked_shl(u32::from(parameters.window_log()))
            .ok_or(Error::ResourceLimit)?;
        let loaded_len = content.len().min(window);
        // Positions index this suffix; a later call stages exactly these bytes
        // before its input, so suffix-relative and staged positions coincide.
        let suffix = &content[content.len() - loaded_len..];
        let tables = match parameters.strategy() {
            ReferenceStrategy::Fast => {
                let plan = FastExecutionPlan::from_resolved_parameters(parameters, loaded_len)?;
                let mut table = MatchTable::with_layout_in(plan.layout(), allocator)?;
                // Mirror the staged Fast seeding loop exactly: ascending
                // positions, later positions win each slot, and the parser's
                // window floor still bounds every candidate it accepts.
                table.reset();
                let layout = plan.layout();
                if let Some(last) = loaded_len.checked_sub(usize::from(layout.key_bytes())) {
                    for position in 0..=last {
                        let key = layout
                            .read_key(&suffix[position..])
                            .ok_or(Error::InvalidState)?;
                        table.replace(key, position)?;
                    }
                }
                CompiledTables::Fast(table)
            }
            ReferenceStrategy::DoubleFast => {
                let plan =
                    DoubleFastExecutionPlan::from_resolved_parameters(parameters, loaded_len)?;
                let mut tables = DoubleFastTables::new_in(
                    plan.long_layout(),
                    plan.short_layout(),
                    allocator.clone(),
                    allocator,
                )?;
                tables.seed_prefix(suffix, loaded_len)?;
                CompiledTables::DoubleFast(tables)
            }
            strategy => {
                let plan = match strategy {
                    ReferenceStrategy::Greedy => {
                        GreedyExecutionPlan::from_resolved_parameters(parameters, loaded_len)?
                    }
                    ReferenceStrategy::Lazy | ReferenceStrategy::Lazy2 => {
                        GreedyExecutionPlan::from_family_parameters(parameters, loaded_len)?
                    }
                    ReferenceStrategy::BinaryTreeLazy2 => {
                        GreedyExecutionPlan::from_tree_parameters(parameters, loaded_len)?
                    }
                    _ => unreachable!("admission checked every rejected strategy"),
                };
                let mut tables = GreedyTables::new_in(plan.tables(), allocator.clone(), allocator)?;
                // Chain and row owners seed every suffix key position; the tree
                // family needs no seeding pass and keeps its first-query
                // back-fill contract at consultation time.
                tables.seed_prefix(suffix, loaded_len)?;
                CompiledTables::Greedy(tables)
            }
        };
        Ok(Self {
            view,
            parameters,
            loaded_len,
            tables,
            entropy,
        })
    }

    /// The resolved profile every later call must reuse.
    pub(crate) const fn parameters(&self) -> ResolvedCompressionParameters {
        self.parameters
    }

    /// The loaded suffix length that fixes content-relative table positions.
    pub(crate) const fn loaded_len(&self) -> usize {
        self.loaded_len
    }

    /// Borrow the immutable dictionary content the state was compiled from.
    pub(crate) fn content(&self) -> &'a [u8] {
        self.view.content()
    }

    /// The dictionary identifier serialized into frames that use this state.
    pub(crate) fn dictionary_id(&self) -> u32 {
        self.view.id()
    }

    /// The prepared sequence/offset entropy a formatted dictionary seeded the
    /// frame with, or `None` for raw and empty dictionaries.
    ///
    /// This is the exact view `ZSTD_loadCEntropy` fills `blockState.prevCBlock`
    /// with: the first emitted block may reuse these tables and repeats.
    pub(crate) fn entropy(&self) -> Option<DictionaryEntropyView<'a>> {
        self.entropy.as_ref().map(|entropy| entropy.view)
    }

    /// The exact first-block seed a compiled frame installs: prepared sequence
    /// entropy plus the literal codebook the combined path rebuilds per call.
    ///
    /// The codebook is copied from plain inline values; no dictionary allocation
    /// is cloned. Raw and empty dictionaries return `None`, so the first block
    /// starts frame-fresh exactly as in the dictionary-free path.
    pub(crate) fn first_block_seed(&self) -> Option<(DictionaryEntropyView<'a>, HuffmanCodebook)> {
        self.entropy
            .as_ref()
            .map(|entropy| (entropy.view, entropy.literal_table.clone()))
    }

    /// The immutable Fast candidate source for a call that stages exactly
    /// [`Self::loaded_len`] suffix bytes before its input.
    ///
    /// Positions are content-relative over that suffix, which coincides with
    /// the staged region, so the call consults them without translation and
    /// the parser's own window floor still bounds every accepted candidate.
    /// The returned handle carries the staged length, so a state constructor
    /// rejects a call that staged a different suffix instead of reading
    /// positions under foreign geometry.
    pub(crate) fn fast_candidates(&self) -> Result<StagedMatchView<'_>> {
        match &self.tables {
            CompiledTables::Fast(table) => Ok(StagedMatchView::new(table.view(), self.loaded_len)),
            _ => Err(Error::InvalidState),
        }
    }

    /// The immutable DoubleFast long/short candidate pair under the same
    /// staged-suffix geometry as [`Self::fast_candidates`].
    pub(crate) fn double_fast_candidates(
        &self,
    ) -> Result<(StagedMatchView<'_>, StagedMatchView<'_>)> {
        match &self.tables {
            CompiledTables::DoubleFast(tables) => {
                let (long, short) = tables.view();
                let staged_len = self.loaded_len;
                Ok((
                    StagedMatchView::new(long, staged_len),
                    StagedMatchView::new(short, staged_len),
                ))
            }
            _ => Err(Error::InvalidState),
        }
    }

    /// The immutable chain candidate source under the same staged-suffix
    /// geometry as [`Self::fast_candidates`].
    ///
    /// Only a compiled chain profile has one; row and tree families reject the
    /// request instead of substituting a different finder.
    pub(crate) fn greedy_chain_candidates(&self) -> Result<StagedChainView<'_>> {
        match &self.tables {
            CompiledTables::Greedy(GreedyTables::Chain(chain)) => {
                Ok(StagedChainView::new(chain.view(), self.loaded_len))
            }
            _ => Err(Error::InvalidState),
        }
    }

    /// The immutable row candidate source under the same staged-suffix geometry
    /// as [`Self::fast_candidates`].
    ///
    /// Only a compiled row profile has one; chain and tree families reject the
    /// request instead of substituting a different finder.
    pub(crate) fn greedy_row_candidates(&self) -> Result<StagedRowView<'_>> {
        match &self.tables {
            CompiledTables::Greedy(GreedyTables::Row(row)) => {
                Ok(StagedRowView::new(row.view(), self.loaded_len))
            }
            _ => Err(Error::InvalidState),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dictionary;
    use crate::double_fast_state::DoubleFastMatchState;
    use crate::fast_state::FastMatchState;
    use crate::greedy_state::{GreedyCandidates, GreedyMatchState};
    use alloc::alloc::Global;
    use alloc::vec;
    use alloc::vec::Vec;
    use zstdoxide_test_allocator::{
        AllocationMonitor, AllocationSnapshot, AllocatorConfig, TrackingAllocator,
    };

    const PREPARED: &[u8] = include_bytes!("../tests/upstream/dictionaries/formatted.dict");

    fn pattern(mut state: u32, length: usize) -> Vec<u8> {
        (0..length)
            .map(|_| {
                state ^= state.wrapping_shl(13);
                state ^= state >> 17;
                state ^= state.wrapping_shl(5);
                state as u8
            })
            .collect()
    }

    fn monitored() -> (TrackingAllocator, AllocationMonitor) {
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = allocator.monitor();
        (allocator, monitor)
    }

    /// An error return drops the consumed allocator value, so the owner-drop
    /// counters move; every allocation counter must stay zero and nothing may
    /// stay live.
    fn assert_no_allocation(snapshot: &AllocationSnapshot) {
        assert_eq!(snapshot.allocate_calls, 0);
        assert_eq!(snapshot.allocate_zeroed_calls, 0);
        assert_eq!(snapshot.grow_calls, 0);
        assert_eq!(snapshot.grow_zeroed_calls, 0);
        assert_eq!(snapshot.shrink_calls, 0);
        assert_eq!(snapshot.attempts, 0);
        assert_eq!(snapshot.successes, 0);
        assert_eq!(snapshot.failures, 0);
        assert_eq!(snapshot.live_blocks, 0);
        assert!(!snapshot.violations.any());
    }

    fn compile<'a, A: Allocator + Clone>(
        view: DictionaryView<'a>,
        level: i32,
        allocator: A,
    ) -> Result<CompiledDictionary<'a, A>> {
        CompiledDictionary::new(
            view,
            &CompressionParameterRequest::with_level(level),
            allocator,
        )
    }

    #[test]
    fn compiled_profiles_admit_the_six_stages_and_reject_optimal_before_allocation() {
        let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
        for level in [1, 2, 3, 4, 5, 6, 9] {
            let (allocator, monitor) = monitored();
            let compiled = compile(dictionary.view(), level, allocator).unwrap();
            assert_eq!(compiled.loaded_len(), dictionary.view().content().len());
            assert_eq!(compiled.dictionary_id(), dictionary.view().id());
            assert_eq!(compiled.content(), dictionary.view().content());
            drop(compiled);
            monitor.assert_released();
        }
        let mut optimal = CompressionParameterRequest::with_level(11);
        optimal.set_strategy(Some(ReferenceStrategy::Optimal));
        for (name, request) in [
            ("explicit-optimal", optimal),
            (
                "default-optimal",
                CompressionParameterRequest::with_level(11),
            ),
            ("ultra", CompressionParameterRequest::with_level(14)),
            ("ultra2", CompressionParameterRequest::with_level(17)),
        ] {
            let (allocator, monitor) = monitored();
            let outcome = CompiledDictionary::new(dictionary.view(), &request, allocator);
            drop(outcome);
            assert_no_allocation(&monitor.snapshot());
            let (allocator, _) = monitored();
            assert!(
                matches!(
                    CompiledDictionary::new(dictionary.view(), &request, allocator),
                    Err(Error::UnsupportedFeature)
                ),
                "{name} admission"
            );
        }
    }

    #[test]
    fn loaded_suffix_caps_at_the_resolved_window_and_releases_its_owner() {
        let content = pattern(0x1e37_79b9, 1 << 17);
        let dictionary = Dictionary::raw(&content).unwrap();
        let (allocator, monitor) = monitored();
        let compiled = compile(dictionary.view(), 1, allocator).unwrap();
        let window = 1_usize << compiled.parameters().window_log();
        assert_eq!(compiled.loaded_len(), content.len().min(window));
        assert_eq!(compiled.content(), dictionary.view().content());
        drop(compiled);
        monitor.assert_released();
    }

    #[test]
    fn empty_content_rejects_construction_without_allocation() {
        let dictionary = Dictionary::raw(b"").unwrap();
        for level in [1, 4, 9] {
            let (allocator, monitor) = monitored();
            let outcome = CompiledDictionary::new(
                dictionary.view(),
                &CompressionParameterRequest::with_level(level),
                allocator,
            );
            assert!(
                matches!(outcome, Err(Error::InvalidParameter)),
                "level {level}"
            );
            assert_no_allocation(&monitor.snapshot());
        }
    }

    #[test]
    fn fast_seeding_writes_every_suffix_position_under_its_content_key() {
        // Distinct four-byte counters make every four-byte key unique, so the
        // newest-wins slot of each key must hold exactly its own position.
        let mut content = Vec::new();
        for index in 0u32..2048 {
            content.extend_from_slice(&index.to_le_bytes());
            content.extend_from_slice(b"tail");
        }
        let dictionary = Dictionary::raw(&content).unwrap();
        let (allocator, monitor) = monitored();
        let compiled = compile(dictionary.view(), 1, allocator).unwrap();
        assert_eq!(compiled.parameters().strategy(), ReferenceStrategy::Fast);
        let loaded = compiled.loaded_len();
        let suffix = &content[content.len() - loaded..];
        let plan =
            FastExecutionPlan::from_resolved_parameters(compiled.parameters(), loaded).unwrap();
        let layout = plan.layout();
        let table = match &compiled.tables {
            CompiledTables::Fast(table) => table,
            _ => panic!("Fast profile compiled the wrong owner"),
        };
        // The compiled table must match a reference table built by the exact
        // staged seeding loop of `FastMatchState::from_parts_with_prefix`:
        // same layout, same ascending positions, same newest-wins slots.
        let mut reference = MatchTable::with_layout_in(layout, Global).unwrap();
        reference.reset();
        let last = loaded - usize::from(layout.key_bytes());
        for position in 0..=last {
            let key = layout.read_key(&suffix[position..]).unwrap();
            reference.replace(key, position).unwrap();
        }
        assert_eq!(table.test_positions(), reference.test_positions());
        let positions = table.test_positions();
        for position in [0, 1, 777, last] {
            let key = layout.read_key(&suffix[position..]).unwrap();
            let bucket = positions[layout.bucket(key).unwrap()];
            // Newest-wins buckets may hold a colliding later key, but the slot
            // must never stay empty after seeding a nonzero suffix.
            assert_ne!(bucket, usize::MAX, "position {position}");
        }
        drop(compiled);
        monitor.assert_released();
    }

    /// Combined-space position of each sequence's match start: `literal_length`
    /// and `match_length` are frame-relative, so a distance greater than that
    /// position proves the candidate came from the staged dictionary region.
    fn dictionary_hits(sequences: &[crate::sequence_context::SequenceWithOffset]) -> usize {
        let mut position = 0usize;
        let mut hits = 0;
        for record in sequences {
            position += record.sequence.literal_length as usize;
            if record.sequence.distance as usize > position {
                hits += 1;
            }
            position += record.sequence.match_length as usize;
        }
        hits
    }

    fn staged_with(content: &[u8], staged: usize, input: &[u8]) -> Vec<u8> {
        let mut combined = Vec::new();
        combined.extend_from_slice(&content[content.len() - staged..]);
        combined.extend_from_slice(input);
        combined
    }

    fn fast_consultation_case(
        level: i32,
        window_log: Option<u8>,
        content_len: usize,
        input_len: usize,
        repeats: usize,
        expect_hits: bool,
    ) {
        let content = pattern(0x0d15_ea2b, content_len);
        let dictionary = Dictionary::raw(&content).unwrap();
        let (allocator, monitor) = monitored();
        let mut request = CompressionParameterRequest::with_level(level);
        if let Some(window_log) = window_log {
            request.set_window_log(window_log).unwrap();
        }
        let compiled = CompiledDictionary::new(dictionary.view(), &request, allocator).unwrap();
        assert_eq!(
            compiled.parameters().strategy(),
            ReferenceStrategy::Fast,
            "level {level}"
        );
        let staged = compiled.loaded_len();
        assert!(staged >= input_len, "level {level}");
        let suffix = &content[content.len() - staged..];
        let input = suffix[staged - input_len..].repeat(repeats);
        let compiled_positions = compiled
            .fast_candidates()
            .unwrap()
            .view()
            .test_positions()
            .to_vec();
        let combined = staged_with(&content, staged, &input);
        let plan = FastExecutionPlan::from_resolved_parameters(compiled.parameters(), input.len())
            .unwrap();
        let mut reference = FastMatchState::from_parts_with_prefix(
            &combined,
            plan,
            MatchTable::with_layout_in(plan.layout(), Global).unwrap(),
            Vec::new(),
            Vec::new(),
            staged,
        )
        .unwrap();
        let mut consulted = FastMatchState::from_parts_with_compiled_prefix(
            &combined,
            plan,
            MatchTable::with_layout_in(plan.layout(), Global).unwrap(),
            Vec::new(),
            Vec::new(),
            staged,
            Some(compiled.fast_candidates().unwrap()),
        )
        .unwrap();
        let mut repeats = [1, 4, 8];
        let mut emitted = 0;
        let mut hits = 0;
        while emitted < input.len() {
            let length = (input.len() - emitted).min(plan.block_limit());
            let expected = reference.parse_next(length, repeats).unwrap();
            let actual = consulted.parse_next(length, repeats).unwrap();
            assert_eq!(actual, expected, "block at {emitted}");
            assert_eq!(consulted.literals(), reference.literals());
            assert_eq!(consulted.sequences(), reference.sequences());
            hits += dictionary_hits(consulted.sequences());
            if let Some(result) = actual {
                repeats = result.wire_repeats;
            }
            emitted += length;
        }
        assert_eq!(
            hits > 0,
            expect_hits,
            "dictionary-candidate expectation: staged {staged}, input {}, window {}, block limit {}, sequences {:?}",
            input.len(),
            plan.window_size(),
            plan.block_limit(),
            consulted.sequences(),
        );
        // The compiled source is unchanged and the per-call table holds only
        // input samples: consultation never copies or mutates compiled cells.
        assert_eq!(
            compiled.fast_candidates().unwrap().view().test_positions(),
            compiled_positions
        );
        assert!(
            consulted
                .test_sampled_positions()
                .iter()
                .all(|&position| position == usize::MAX || position >= staged)
        );
        drop(consulted);
        drop(reference);
        drop(compiled);
        monitor.assert_released();
    }

    #[test]
    fn fast_consultation_matches_combined_staging_and_uses_dictionary_candidates() {
        for level in [1, 2] {
            fast_consultation_case(level, None, 4096, 1024, 1, true);
        }
    }

    #[test]
    fn fast_consultation_covers_multiple_blocks_and_a_capped_window() {
        // An explicit window smaller than the dictionary content caps the loaded
        // suffix; three repeated copies of it need three blocks, so the window
        // floor and the sampling frontier advance across block boundaries. A
        // first block that fills the window pushes the floor to the prefix, so
        // this geometry cannot reach a dictionary candidate.
        fast_consultation_case(1, Some(13), 32 * 1024, 1 << 13, 3, false);
        // A short first block leaves part of the capped suffix inside the
        // window floor, so the staged candidates are reachable after all.
        fast_consultation_case(1, Some(13), 32 * 1024, 1024, 1, true);
    }

    fn double_fast_consultation_case(
        level: i32,
        window_log: Option<u8>,
        content_len: usize,
        input_len: usize,
        repeats: usize,
        expect_hits: bool,
    ) {
        let content = pattern(0x51ed_2c4f, content_len);
        let dictionary = Dictionary::raw(&content).unwrap();
        let (allocator, monitor) = monitored();
        let mut request = CompressionParameterRequest::with_level(level);
        if let Some(window_log) = window_log {
            request.set_window_log(window_log).unwrap();
        }
        let compiled = CompiledDictionary::new(dictionary.view(), &request, allocator).unwrap();
        assert_eq!(
            compiled.parameters().strategy(),
            ReferenceStrategy::DoubleFast,
            "level {level}"
        );
        let staged = compiled.loaded_len();
        assert!(staged >= input_len, "level {level}");
        let suffix = &content[content.len() - staged..];
        let input = suffix[staged - input_len..].repeat(repeats);
        let (compiled_long, compiled_short) = compiled.double_fast_candidates().unwrap();
        let compiled_long = compiled_long.view().test_positions().to_vec();
        let compiled_short = compiled_short.view().test_positions().to_vec();
        let combined = staged_with(&content, staged, &input);
        let plan =
            DoubleFastExecutionPlan::from_resolved_parameters(compiled.parameters(), input.len())
                .unwrap();
        let mut reference = DoubleFastMatchState::from_parts_with_prefix(
            &combined,
            plan,
            DoubleFastTables::new_in(plan.long_layout(), plan.short_layout(), Global, Global)
                .unwrap(),
            Vec::new(),
            Vec::new(),
            staged,
        )
        .unwrap();
        let mut consulted = DoubleFastMatchState::from_parts_with_compiled_prefix(
            &combined,
            plan,
            DoubleFastTables::new_in(plan.long_layout(), plan.short_layout(), Global, Global)
                .unwrap(),
            Vec::new(),
            Vec::new(),
            staged,
            Some(compiled.double_fast_candidates().unwrap()),
        )
        .unwrap();
        let mut repeats = [1, 4, 8];
        let mut emitted = 0;
        let mut hits = 0;
        while emitted < input.len() {
            let length = (input.len() - emitted).min(plan.block_limit());
            let expected = reference.parse_next(length, repeats).unwrap();
            let actual = consulted.parse_next(length, repeats).unwrap();
            assert_eq!(actual, expected, "block at {emitted}");
            assert_eq!(consulted.literals(), reference.literals());
            assert_eq!(consulted.sequences(), reference.sequences());
            hits += dictionary_hits(consulted.sequences());
            if let Some(result) = actual {
                repeats = result.wire_repeats;
            }
            emitted += length;
        }
        assert_eq!(
            hits > 0,
            expect_hits,
            "double-fast dictionary-candidate expectation: staged {staged}, input {}, window {}, block limit {}, sequences {:?}",
            input.len(),
            plan.window_size(),
            plan.block_limit(),
            consulted.sequences(),
        );
        let (long, short) = compiled.double_fast_candidates().unwrap();
        assert_eq!(long.view().test_positions(), compiled_long);
        assert_eq!(short.view().test_positions(), compiled_short);
        let (long, short) = consulted.test_sampled_positions();
        for positions in [long, short] {
            assert!(
                positions
                    .iter()
                    .all(|&position| position == usize::MAX || position >= staged)
            );
        }
        drop(consulted);
        drop(reference);
        drop(compiled);
        monitor.assert_released();
    }

    #[test]
    fn double_fast_consultation_matches_combined_staging_and_uses_both_tables() {
        // Level 3 is the only default DoubleFast profile in this resolver;
        // level 4 already resolves to Greedy.
        double_fast_consultation_case(3, None, 4096, 1024, 1, true);
    }

    #[test]
    fn double_fast_consultation_covers_multiple_blocks_and_a_capped_window() {
        double_fast_consultation_case(3, Some(12), 32 * 1024, 1 << 12, 3, false);
        double_fast_consultation_case(3, Some(12), 32 * 1024, 1024, 1, true);
    }

    fn greedy_chain_consultation_case(level: i32) {
        let content = pattern(0xc0ff_ee11, 4096);
        let input = content[content.len() - 1024..].to_vec();
        let dictionary = Dictionary::raw(&content).unwrap();
        let (allocator, monitor) = monitored();
        let mut request = CompressionParameterRequest::with_level(level);
        // A window at or below 14 selects the chain finder, so this profile is
        // a compiled chain even though the default 64 KiB-frame tiers use rows.
        request.set_window_log(13).unwrap();
        let compiled = CompiledDictionary::new(dictionary.view(), &request, allocator).unwrap();
        assert!(
            matches!(
                compiled.parameters().strategy(),
                ReferenceStrategy::Greedy | ReferenceStrategy::Lazy | ReferenceStrategy::Lazy2
            ),
            "level {level} is not an admitted lazy-family profile"
        );
        let staged = compiled.loaded_len();
        assert_eq!(staged, content.len());
        let candidate = compiled.greedy_chain_candidates().unwrap();
        assert_eq!(candidate.staged_len(), staged);
        let compiled_maps = {
            let (heads, links) = candidate.view().test_maps();
            (heads.to_vec(), links.to_vec())
        };
        let combined = staged_with(&content, staged, &input);
        let plan = GreedyExecutionPlan::from_family_parameters(compiled.parameters(), input.len())
            .unwrap();
        let mut reference = GreedyMatchState::from_parts_with_prefix(
            &combined,
            plan,
            GreedyTables::new_in(plan.tables(), Global, Global).unwrap(),
            Vec::new(),
            Vec::new(),
            staged,
        )
        .unwrap();
        let mut consulted = GreedyMatchState::from_parts_with_compiled_prefix(
            &combined,
            plan,
            GreedyTables::new_in(plan.tables(), Global, Global).unwrap(),
            Vec::new(),
            Vec::new(),
            staged,
            Some(GreedyCandidates::Chain(candidate)),
        )
        .unwrap();
        let mut repeats = [1, 4, 8];
        let mut emitted = 0;
        let mut hits = 0;
        while emitted < input.len() {
            let length = (input.len() - emitted).min(plan.block_limit());
            let expected = reference.parse_next(length, repeats).unwrap();
            let actual = consulted.parse_next(length, repeats).unwrap();
            assert_eq!(actual, expected, "level {level} block at {emitted}");
            assert_eq!(consulted.literals(), reference.literals());
            assert_eq!(consulted.sequences(), reference.sequences());
            hits += dictionary_hits(consulted.sequences());
            if let Some(result) = actual {
                repeats = result.wire_repeats;
            }
            emitted += length;
        }
        assert!(hits > 0, "level {level}: no compiled chain candidate used");
        // Consultation never writes the compiled chain: both arrays are
        // identical after every block.
        let (heads, links) = compiled
            .greedy_chain_candidates()
            .unwrap()
            .view()
            .test_maps();
        assert_eq!((heads, links), (&compiled_maps.0[..], &compiled_maps.1[..]));
        drop(consulted);
        drop(reference);
        drop(compiled);
        monitor.assert_released();
    }

    #[test]
    fn greedy_chain_consultation_matches_combined_staging_and_stays_read_only() {
        // A small dictionary tiers these levels to Greedy (4), Lazy (5) and
        // Lazy2 (7); all three use the same chain finder at window log 13.
        for level in [4, 5, 7] {
            greedy_chain_consultation_case(level);
        }
    }

    fn greedy_row_consultation_case(level: i32) {
        // A 32 KiB dictionary tiers these levels to window log 17, where the
        // lazy family selects the row finder.
        let content = pattern(0x7e11_5a3d, 32 * 1024);
        let input = content[content.len() - 1024..].to_vec();
        let dictionary = Dictionary::raw(&content).unwrap();
        let (allocator, monitor) = monitored();
        let compiled = compile(dictionary.view(), level, allocator).unwrap();
        assert!(
            matches!(
                compiled.parameters().strategy(),
                ReferenceStrategy::Greedy | ReferenceStrategy::Lazy | ReferenceStrategy::Lazy2
            ),
            "level {level} is not an admitted lazy-family profile"
        );
        assert!(compiled.parameters().window_log() > 14, "level {level}");
        let staged = compiled.loaded_len();
        assert_eq!(staged, content.len());
        let candidate = compiled.greedy_row_candidates().unwrap();
        assert_eq!(candidate.staged_len(), staged);
        let compiled_maps = {
            let (positions, tags) = candidate.view().test_maps();
            (positions.to_vec(), tags.to_vec())
        };
        let combined = staged_with(&content, staged, &input);
        let plan = GreedyExecutionPlan::from_family_parameters(compiled.parameters(), input.len())
            .unwrap();
        let mut reference = GreedyMatchState::from_parts_with_prefix(
            &combined,
            plan,
            GreedyTables::new_in(plan.tables(), Global, Global).unwrap(),
            Vec::new(),
            Vec::new(),
            staged,
        )
        .unwrap();
        let mut consulted = GreedyMatchState::from_parts_with_compiled_prefix(
            &combined,
            plan,
            GreedyTables::new_in(plan.tables(), Global, Global).unwrap(),
            Vec::new(),
            Vec::new(),
            staged,
            Some(GreedyCandidates::Row(candidate)),
        )
        .unwrap();
        let mut repeats = [1, 4, 8];
        let mut emitted = 0;
        let mut hits = 0;
        while emitted < input.len() {
            let length = (input.len() - emitted).min(plan.block_limit());
            let expected = reference.parse_next(length, repeats).unwrap();
            let actual = consulted.parse_next(length, repeats).unwrap();
            // Separate row rings can reorder candidates relative to one seeded
            // ring, so equality is measured for this deterministic pair rather
            // than asserted as a general contract.
            assert_eq!(actual, expected, "level {level} block at {emitted}");
            assert_eq!(consulted.literals(), reference.literals());
            assert_eq!(consulted.sequences(), reference.sequences());
            hits += dictionary_hits(consulted.sequences());
            if let Some(result) = actual {
                repeats = result.wire_repeats;
            }
            emitted += length;
        }
        assert!(hits > 0, "level {level}: no compiled row candidate used");
        let (positions, tags) = compiled.greedy_row_candidates().unwrap().view().test_maps();
        assert_eq!(
            (positions, tags),
            (&compiled_maps.0[..], &compiled_maps.1[..])
        );
        drop(consulted);
        drop(reference);
        drop(compiled);
        monitor.assert_released();
    }

    #[test]
    fn greedy_row_consultation_matches_combined_staging_and_stays_read_only() {
        // Levels 5, 6 and 7 cover Greedy, Lazy and Lazy2 over the row finder.
        for level in [5, 6, 7] {
            greedy_row_consultation_case(level);
        }
    }

    #[test]
    fn greedy_row_candidates_reject_other_profiles_and_the_chain_finder() {
        let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
        let (allocator, _) = monitored();
        let fast = compile(dictionary.view(), 1, allocator).unwrap();
        assert!(matches!(
            fast.greedy_row_candidates(),
            Err(Error::InvalidState)
        ));
        // A window log at or below 14 selects the chain finder for Greedy, so no
        // compiled row exists and the request is rejected explicitly.
        let (allocator, _) = monitored();
        let chain = compile_chain_13(dictionary.view(), allocator).unwrap();
        assert!(matches!(
            chain.greedy_row_candidates(),
            Err(Error::InvalidState)
        ));
    }

    fn compile_chain_13<'a, A: Allocator + Clone>(
        view: DictionaryView<'a>,
        allocator: A,
    ) -> Result<CompiledDictionary<'a, A>> {
        let mut request = CompressionParameterRequest::with_level(5);
        request.set_window_log(13).unwrap();
        CompiledDictionary::new(view, &request, allocator)
    }

    #[test]
    fn greedy_chain_candidates_reject_other_profiles_and_the_row_finder() {
        let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
        let (allocator, _) = monitored();
        let fast = compile(dictionary.view(), 1, allocator).unwrap();
        assert!(matches!(
            fast.greedy_chain_candidates(),
            Err(Error::InvalidState)
        ));
        // A 32 KiB dictionary tiers level 5 to window log 17, where the Greedy
        // family selects the row finder; no compiled chain exists and the
        // request is rejected explicitly.
        let row_content = pattern(0x32ad_5c71, 32 * 1024);
        let row_dictionary = Dictionary::raw(&row_content).unwrap();
        let (allocator, monitor) = monitored();
        let row = compile(row_dictionary.view(), 5, allocator).unwrap();
        assert_eq!(row.parameters().strategy(), ReferenceStrategy::Greedy);
        assert!(row.parameters().window_log() > 14);
        assert!(matches!(
            row.greedy_chain_candidates(),
            Err(Error::InvalidState)
        ));
        drop(row);
        monitor.assert_released();
    }

    #[test]
    fn candidate_accessors_reject_the_other_profile_and_mismatched_layouts() {
        let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
        let (allocator, _) = monitored();
        let fast = compile(dictionary.view(), 1, allocator).unwrap();
        assert_eq!(fast.parameters().strategy(), ReferenceStrategy::Fast);
        assert!(matches!(
            fast.double_fast_candidates(),
            Err(Error::InvalidState)
        ));
        let (allocator, _) = monitored();
        let double_fast = compile(dictionary.view(), 3, allocator).unwrap();
        assert_eq!(
            double_fast.parameters().strategy(),
            ReferenceStrategy::DoubleFast
        );
        assert!(matches!(
            double_fast.fast_candidates(),
            Err(Error::InvalidState)
        ));
        // A plan resolved with a different hash log has a different layout;
        // the state constructor rejects the compiled view before resetting its
        // own table or reading a single candidate.
        let mut other = CompressionParameterRequest::with_level(1);
        other.set_hash_log(9).unwrap();
        let parameters = other
            .resolve(SourceSize::Unknown, dictionary.view().content().len())
            .unwrap();
        let input = b"compiled layout rejection";
        let plan = FastExecutionPlan::from_resolved_parameters(parameters, input.len()).unwrap();
        let table = MatchTable::with_layout_in(plan.layout(), Global).unwrap();
        let outcome = FastMatchState::from_parts_with_compiled_prefix(
            input,
            plan,
            table,
            Vec::new(),
            Vec::new(),
            0,
            Some(fast.fast_candidates().unwrap()),
        );
        assert!(matches!(outcome, Err(Error::InvalidParameter)));
        // The same layout with the correct staged length is admitted, proving
        // the rejection above came from the staging geometry, not the layout.
        let plan =
            FastExecutionPlan::from_resolved_parameters(fast.parameters(), input.len()).unwrap();
        let table = MatchTable::with_layout_in(plan.layout(), Global).unwrap();
        let mut staged_source = vec![0_u8; fast.loaded_len()];
        staged_source.extend_from_slice(input);
        let accepted = FastMatchState::from_parts_with_compiled_prefix(
            &staged_source,
            plan,
            table,
            Vec::new(),
            Vec::new(),
            fast.loaded_len(),
            Some(fast.fast_candidates().unwrap()),
        );
        assert!(accepted.is_ok());
    }

    #[test]
    fn prepared_entropy_and_its_literal_table_are_compiled_once_for_formatted_only() {
        let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
        let view = dictionary.view();
        let source_entropy = view.entropy().unwrap();
        let (allocator, monitor) = monitored();
        let compiled = compile(view, 1, allocator).unwrap();
        let compiled_entropy = compiled.entropy().unwrap();
        assert_eq!(
            compiled_entropy.huffman.code_lengths(),
            source_entropy.huffman.code_lengths()
        );
        assert_eq!(compiled_entropy.repeat, source_entropy.repeat);
        let expected =
            HuffmanCodebook::from_lengths(&source_entropy.huffman.code_lengths(), 12).unwrap();
        let (seed_entropy, seed_codebook) = compiled.first_block_seed().unwrap();
        assert_eq!(seed_entropy.repeat, source_entropy.repeat);
        assert_eq!(
            seed_codebook, expected,
            "the compiled literal table must be the exact first-block seed"
        );
        drop(compiled);
        monitor.assert_released();
        // Raw dictionaries compile no prepared entropy and no literal table.
        let raw = Dictionary::raw(&pattern(0x7a3c_11d5, 2048)).unwrap();
        let (allocator, monitor) = monitored();
        let compiled = compile(raw.view(), 1, allocator).unwrap();
        assert!(compiled.entropy().is_none());
        assert!(compiled.first_block_seed().is_none());
        drop(compiled);
        monitor.assert_released();
    }
}
