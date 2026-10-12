//! Allocation-free admission for bounded no-dictionary reference Greedy.
//! Parameter resolution and strategy/resource checks precede every allocator
//! clone or request. Plans are copied scalar values shared by all backends.
#![forbid(unsafe_code)]

use crate::greedy_key::{GreedyKeyLayout, RowLayout};
use crate::reference_frame::{ReferenceFrameHeader, ReferenceFrameSettings};
#[cfg(test)]
use crate::{CompressionParameterRequest, SourceSize};
use crate::{Error, ReferenceStrategy, ResolvedCompressionParameters, Result};
use core::alloc::Layout;

/// Only the selected family contributes owners. Chain has two usize arrays;
/// rows have one usize array and one byte array, with no chain allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GreedyTablePlan {
    Chain {
        key: GreedyKeyLayout,
        chain_log: u8,
        attempts: usize,
    },
    Row(RowLayout),
}

impl GreedyTablePlan {
    pub(crate) const fn key_layout(self) -> GreedyKeyLayout {
        match self {
            Self::Chain { key, .. } => key,
            Self::Row(row) => row.key_layout(),
        }
    }
    pub(crate) const fn attempts(self) -> usize {
        match self {
            Self::Chain { attempts, .. } => attempts,
            Self::Row(row) => row.attempts(),
        }
    }
    pub(crate) const fn is_row(self) -> bool {
        matches!(self, Self::Row(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GreedyExecutionPlan {
    parameters: ResolvedCompressionParameters,
    settings: ReferenceFrameSettings,
    tables: GreedyTablePlan,
    first_bytes: usize,
    second_bytes: usize,
    table_bytes: usize,
    frame_bound: usize,
}

impl GreedyExecutionPlan {
    /// Resolve using the actual known source size, including Known(0); hints
    /// never substitute for it. Corresponds to parameter resolution followed by
    /// `ZSTD_resolveRowMatchFinderMode`: resolved windowLog > 14 selects rows,
    /// independently of target SIMD.
    #[cfg(test)]
    pub(crate) fn new(request: &CompressionParameterRequest, source_len: usize) -> Result<Self> {
        let source_size = Self::source_size_checked(source_len)?;
        let parameters = request.resolve(SourceSize::Known(source_size), 0)?;
        Self::from_resolved_parameters(parameters, source_len)
    }

    /// Use an already resolved tuple without a second source-size adjustment.
    /// Error precedence is source domain, unsupported strategy, operational
    /// resources, then complete target layouts and output-bound arithmetic.
    /// The W21/H19, chain C18/S8 caps are local resource policy, not upstream
    /// parameter validity. The unused chain dimension never limits row mode.
    pub(crate) fn from_resolved_parameters(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
    ) -> Result<Self> {
        Self::source_size_checked(source_len)?;
        if parameters.strategy() != ReferenceStrategy::Greedy {
            return Err(Error::UnsupportedFeature);
        }
        if parameters.window_log() > 21 || parameters.hash_log() > 19 {
            return Err(Error::ResourceLimit);
        }
        let key = GreedyKeyLayout::new(parameters.hash_log(), parameters.min_match())?;
        let first_bytes = Layout::array::<usize>(key.cells())
            .map_err(|_| Error::ResourceLimit)?
            .size();
        let (tables, second_bytes) = if parameters.window_log() > 14 {
            let row = RowLayout::new(key, parameters.search_log())?;
            (
                GreedyTablePlan::Row(row),
                Layout::array::<u8>(key.cells())
                    .map_err(|_| Error::ResourceLimit)?
                    .size(),
            )
        } else {
            if parameters.chain_log() > 18 || parameters.search_log() > 8 {
                return Err(Error::ResourceLimit);
            }
            let cells = 1_usize
                .checked_shl(u32::from(parameters.chain_log()))
                .ok_or(Error::ResourceLimit)?;
            let bytes = Layout::array::<usize>(cells)
                .map_err(|_| Error::ResourceLimit)?
                .size();
            (
                GreedyTablePlan::Chain {
                    key,
                    chain_log: parameters.chain_log(),
                    attempts: 1 << parameters.search_log(),
                },
                bytes,
            )
        };
        let table_bytes = first_bytes
            .checked_add(second_bytes)
            .ok_or(Error::ResourceLimit)?;
        let settings = ReferenceFrameSettings::new(source_len, parameters.window_log())?;
        let frame_bound = ReferenceFrameHeader::raw_frame_bound(&settings)?;
        // Vec output must fit the target's allocation/isize limit as well as
        // usize arithmetic. This checks a mathematical layout, without allocating
        // input/output or imposing an arbitrary oracle-sized source cap.
        Layout::array::<u8>(frame_bound).map_err(|_| Error::ResourceLimit)?;
        Ok(Self {
            parameters,
            settings,
            tables,
            first_bytes,
            second_bytes,
            table_bytes,
            frame_bound,
        })
    }

    fn source_size_checked(source_len: usize) -> Result<u64> {
        let value = u64::try_from(source_len).map_err(|_| Error::ResourceLimit)?;
        if value == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        Ok(value)
    }
    pub(crate) const fn parameters(self) -> ResolvedCompressionParameters {
        self.parameters
    }
    pub(crate) const fn source_len(self) -> usize {
        self.settings.source_len()
    }
    #[cfg(test)]
    pub(crate) const fn source_size(self) -> u64 {
        self.settings.source_size()
    }
    pub(crate) const fn window_size(self) -> usize {
        self.settings.window_size()
    }
    pub(crate) const fn block_limit(self) -> usize {
        self.settings.block_limit()
    }
    pub(crate) const fn tables(self) -> GreedyTablePlan {
        self.tables
    }
    #[cfg(test)]
    pub(crate) const fn first_bytes(self) -> usize {
        self.first_bytes
    }
    #[cfg(test)]
    pub(crate) const fn second_bytes(self) -> usize {
        self.second_bytes
    }
    #[cfg(test)]
    pub(crate) const fn table_bytes(self) -> usize {
        self.table_bytes
    }
    #[cfg(test)]
    pub(crate) const fn frame_bound(self) -> usize {
        self.frame_bound
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RawCompressionParameters;
    use alloc::vec::Vec;

    fn raw() -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: 14,
            chain_log: 6,
            hash_log: 6,
            search_log: 1,
            min_match: 4,
            target_length: 0,
            strategy: 3,
        }
    }
    fn direct(raw: RawCompressionParameters, size: usize) -> Result<GreedyExecutionPlan> {
        GreedyExecutionPlan::from_resolved_parameters(raw.validate().unwrap(), size)
    }
    fn request() -> CompressionParameterRequest {
        let mut request = CompressionParameterRequest::with_level(3);
        request.set_strategy(Some(ReferenceStrategy::Greedy));
        request
    }

    #[test]
    fn frozen_known_source_queries_select_the_actual_greedy_family() {
        let mut checked = 0;
        let mut admitted = 0;
        let mut rows = 0;
        for line in include_str!("../tests/upstream/parameters/queries.tsv")
            .lines()
            .skip(1)
        {
            let f: Vec<_> = line.split('\t').collect();
            if f[1] != "public" || f[4..7] != ["0", "ordinary", "known"] || f[17] != "0" {
                continue;
            }
            let source_size: u64 = f[3].parse().unwrap();
            let Ok(source_len) = usize::try_from(source_size) else {
                continue;
            };
            let values: [u32; 7] = core::array::from_fn(|i| f[i + 7].parse().unwrap());
            let expected = RawCompressionParameters {
                window_log: values[0],
                chain_log: values[1],
                hash_log: values[2],
                search_log: values[3],
                min_match: values[4],
                target_length: values[5],
                strategy: values[6],
            };
            let request = CompressionParameterRequest::with_level(f[2].parse().unwrap());
            let result = GreedyExecutionPlan::new(&request, source_len);
            checked += 1;
            if expected.strategy == 3 {
                let plan = result.unwrap();
                admitted += 1;
                rows += usize::from(plan.tables().is_row());
                assert_eq!(plan.parameters().to_raw(), expected);
                assert_eq!(direct(expected, source_len), Ok(plan));
                assert_eq!(
                    (plan.source_len(), plan.source_size()),
                    (source_len, source_size)
                );
                assert_eq!(plan.tables().is_row(), expected.window_log > 14);
            } else {
                assert_eq!(result, Err(Error::UnsupportedFeature));
            }
        }
        assert_eq!((admitted, rows), (5, 4));
        assert_eq!(checked, if usize::BITS == 32 { 188 } else { 190 });
    }

    #[test]
    fn actual_source_adjustment_precedes_branch_choice_and_resource_checks() {
        let mut r = request();
        r.set_window_log(22).unwrap();
        r.set_hash_log(20).unwrap();
        r.set_chain_log(19).unwrap();
        assert_eq!(
            GreedyExecutionPlan::new(&r, 4_194_304),
            Err(Error::ResourceLimit)
        );
        let chain = GreedyExecutionPlan::new(&r, 16_384).unwrap();
        let row = GreedyExecutionPlan::new(&r, 16_385).unwrap();
        assert!(!chain.tables().is_row());
        assert!(row.tables().is_row());
        assert_eq!(
            (
                chain.parameters().window_log(),
                row.parameters().window_log()
            ),
            (14, 15)
        );
        assert_eq!(chain.tables().key_layout().hash_log(), 15);
        // The same large search setting is rejected by the actually selected
        // chain branch but capped without rejection by the selected row branch.
        r.set_search_log(29).unwrap();
        assert_eq!(
            GreedyExecutionPlan::new(&r, 16_384),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            GreedyExecutionPlan::new(&r, 16_385)
                .unwrap()
                .tables()
                .attempts(),
            64
        );
        let mut hinted = request();
        hinted.set_source_hint(2_097_152).unwrap();
        let before = hinted;
        assert_eq!(
            GreedyExecutionPlan::new(&hinted, 0),
            GreedyExecutionPlan::new(&request(), 0)
        );
        assert_eq!(hinted, before);
    }

    #[test]
    fn exact_independent_table_costs_and_row_only_budget_caps() {
        for (hash, chain) in [(6, 18), (19, 6), (19, 18)] {
            let mut r = raw();
            r.hash_log = hash;
            r.chain_log = chain;
            r.search_log = 8;
            let plan = direct(r, 2_097_152).unwrap();
            assert_eq!(plan.first_bytes(), (1_usize << hash) * size_of::<usize>());
            assert_eq!(plan.second_bytes(), (1_usize << chain) * size_of::<usize>());
            assert_eq!(plan.table_bytes(), plan.first_bytes() + plan.second_bytes());
            assert_eq!(plan.tables().attempts(), 256);
            r.window_log = 21;
            r.chain_log = 29;
            for search in [1, 4, 5, 6, 7, 29] {
                r.search_log = search;
                let plan = direct(r, 2_097_152).unwrap();
                assert_eq!(plan.second_bytes(), 1_usize << hash);
                assert_eq!(
                    plan.table_bytes(),
                    (1_usize << hash) * (size_of::<usize>() + 1)
                );
                assert_eq!(plan.tables().attempts(), 1 << search.min(6));
                assert_eq!(
                    (plan.window_size(), plan.block_limit()),
                    (2_097_152, 131_072)
                );
            }
        }
    }

    #[test]
    fn unsupported_strategy_precedes_resources_and_prior_caps_stay_separate() {
        let mut r = raw();
        r.window_log = 22;
        r.hash_log = 20;
        r.chain_log = 19;
        r.search_log = 9;
        for strategy in [1, 2, 4, 5, 6, 7, 8, 9] {
            r.strategy = strategy;
            assert_eq!(direct(r, 4_194_304), Err(Error::UnsupportedFeature));
        }
        r.strategy = 3;
        assert_eq!(direct(r, 4_194_304), Err(Error::ResourceLimit));
        r.window_log = 14;
        r.hash_log = 19;
        assert_eq!(direct(r, 4_194_304), Err(Error::ResourceLimit));
        r.chain_log = 18;
        assert_eq!(direct(r, 4_194_304), Err(Error::ResourceLimit));
        r.search_log = 8;
        assert!(direct(r, 4_194_304).is_ok());
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_window_log(21).unwrap();
        assert!(matches!(
            crate::fast_plan::FastExecutionPlan::new(&request, 2_097_152),
            Err(Error::ResourceLimit)
        ));
        request.set_strategy(Some(ReferenceStrategy::DoubleFast));
        request.set_hash_log(19).unwrap();
        assert!(matches!(
            crate::double_fast_plan::DoubleFastExecutionPlan::new(&request, 2_097_152),
            Err(Error::ResourceLimit)
        ));
    }

    #[test]
    fn effective_keys_and_target_length_do_not_change_the_greedy_policy() {
        for window in [14, 15, 21] {
            for minimum in 3..=7 {
                let mut r = raw();
                r.window_log = window;
                r.min_match = minimum;
                let plain = direct(r, 131_072).unwrap();
                r.target_length = 131_072;
                let changed = direct(r, 131_072).unwrap();
                assert_eq!(plain.tables(), changed.tables());
                assert_eq!(
                    plain.tables().key_layout().key_bytes(),
                    (minimum as u8).clamp(4, 6)
                );
                assert_eq!(plain.frame_bound(), changed.frame_bound());
                assert_eq!(plain.table_bytes(), changed.table_bytes());
            }
        }
    }

    #[test]
    fn frame_bounds_and_large_source_domain_are_checked_without_allocation() {
        let mut r = raw();
        r.window_log = 21;
        for size in [0, 1, 131_072, 131_073, 2_097_153] {
            let plan = direct(r, size).unwrap();
            let header = ReferenceFrameHeader::new(&ReferenceFrameSettings::new(size, 21).unwrap());
            assert_eq!(
                plan.frame_bound(),
                size + header.as_bytes().len() + 3 * size.div_ceil(131_072).max(1)
            );
        }
        assert_eq!(direct(r, isize::MAX as usize), Err(Error::ResourceLimit));
        if usize::BITS == 64 {
            assert_eq!(
                GreedyExecutionPlan::new(&request(), usize::MAX),
                Err(Error::InvalidParameter)
            );
            let size = usize::try_from(u64::from(u32::MAX) + 17).unwrap();
            let plan = direct(r, size).unwrap();
            assert_eq!(plan.source_size(), u64::from(u32::MAX) + 17);
            assert_eq!(plan.source_len(), size);
        }
    }
}
