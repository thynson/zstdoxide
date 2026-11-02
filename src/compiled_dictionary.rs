//! Immutable compiled dictionary state over seeded reference tables.
//!
//! One state owns the resolved scalar profile, per-profile seed tables over the
//! loaded dictionary suffix, and a borrow of immutable dictionary content.
//! Construction validates admission before the first allocation and seeds every
//! table exactly once; the tables stay immutable during calls and hold
//! content-relative positions over the loaded suffix, so a later call consults
//! them as a second read-only candidate source beside its own per-call tables.
//! This leaf checkpoint stops at ownership: emission consultation, prepared
//! entropy wiring and the public surface follow in later items.
//!
//! Upstream counterparts are pinned `ZSTD_CDict` ownership and loading:
//! `ZSTD_createCDict_advanced2` resolves once with an unknown source size and
//! the dictionary size (`zstd_compress.c:5660-5707`), `ZSTD_initCDict_internal`
//! loads entropy and content once (`:5551-5604`), and
//! `ZSTD_loadDictionaryContent` truncates to the loaded suffix whose end
//! anchors window expiry (`:4898-4990`). Native reuse cutoffs, dedicated
//! dictionary search, reset directives and parameter lifecycle are not implied.
#![forbid(unsafe_code)]

use crate::double_fast_plan::DoubleFastExecutionPlan;
use crate::double_fast_tables::DoubleFastTables;
use crate::fast_plan::FastExecutionPlan;
use crate::greedy_plan::GreedyExecutionPlan;
use crate::greedy_state::GreedyTables;
use crate::matchfinder::MatchTable;
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
    allow(
        dead_code,
        reason = "emission consultation follows this leaf checkpoint"
    )
)]
pub(crate) enum CompiledTables<A: Allocator> {
    Fast(MatchTable<A>),
    DoubleFast(DoubleFastTables<A, A>),
    Greedy(GreedyTables<A>),
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
    allow(
        dead_code,
        reason = "emission consultation follows this leaf checkpoint"
    )
)]
pub(crate) struct CompiledDictionary<'a, A: Allocator = alloc::alloc::Global> {
    view: DictionaryView<'a>,
    parameters: ResolvedCompressionParameters,
    loaded_len: usize,
    tables: CompiledTables<A>,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "emission consultation follows this leaf checkpoint"
    )
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dictionary;
    use alloc::alloc::Global;
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
}
