//! Allocation-free plans for bounded complete Optimal and private Ultra sources.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Internal layout diagnostics share the checked execution plan"
    )
)]

use crate::optimal_frequencies::CELLS;
use crate::optimal_matches::MATCH_CAPACITY;
use crate::optimal_path_layout::{PATH_CAPACITY, PathCell};
use crate::optimal_search_layout::{OptimalMatch, OptimalSearchLayout};
use crate::reference_frame::{ReferenceFrameHeader, ReferenceFrameSettings};
use crate::sequence_context::SequenceWithOffset;
use crate::{Error, ReferenceStrategy, ResolvedCompressionParameters, Result};
use core::alloc::Layout;

/// Plain copied values shared by every storage backend. Each byte count belongs
/// to a distinct allocator owner, in parser/search/literal/sequence order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OptimalExecutionPlan {
    parameters: ResolvedCompressionParameters,
    layout: OptimalSearchLayout,
    settings: ReferenceFrameSettings,
    owner_bytes: [usize; 9],
    frame_bound: usize,
}
impl OptimalExecutionPlan {
    /// Parameters have already undergone known-size resolution. No allocations
    /// or allocator clones occur here; local resource bounds are not format limits.
    pub(crate) fn from_resolved_parameters(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
    ) -> Result<Self> {
        Self::from_strategy(parameters, source_len, ReferenceStrategy::Optimal)
    }
    /// Private Ultra admission retains the ordinary source/geometry envelope.
    /// The public request dispatcher remains closed until its separate gates pass.
    pub(crate) fn from_ultra_parameters(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
    ) -> Result<Self> {
        Self::from_strategy(parameters, source_len, ReferenceStrategy::Ultra)
    }
    fn from_strategy(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
        expected: ReferenceStrategy,
    ) -> Result<Self> {
        let size = u64::try_from(source_len).map_err(|_| Error::ResourceLimit)?;
        if size == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        if parameters.strategy() != expected {
            return Err(Error::UnsupportedFeature);
        }
        if !(64..=16384).contains(&source_len)
            || parameters.window_log() > 16
            || parameters.hash_log() > 12
            || parameters.chain_log() > 12
            || parameters.search_log() > 6
            || parameters.target_length() > 4095
        {
            return Err(Error::ResourceLimit);
        }
        let layout = OptimalSearchLayout::new(
            parameters.window_log(),
            parameters.hash_log(),
            parameters.chain_log(),
            parameters.search_log(),
            parameters.min_match(),
            parameters.target_length() as usize,
            (parameters.min_match() == 3).then_some(parameters.window_log()),
        )?;
        layout
            .validate_fresh(source_len)
            .map_err(|_| Error::ResourceLimit)?;
        fn bytes<T>(count: usize) -> Result<usize> {
            Layout::array::<T>(count)
                .map(|v| v.size())
                .map_err(|_| Error::ResourceLimit)
        }
        let owner_bytes = [
            bytes::<u32>(CELLS)?,
            bytes::<PathCell>(PATH_CAPACITY)?,
            bytes::<OptimalMatch>(MATCH_CAPACITY)?,
            bytes::<usize>(layout.tree().key().cells())?,
            bytes::<usize>(
                layout
                    .tree()
                    .nodes()
                    .checked_mul(2)
                    .ok_or(Error::ResourceLimit)?,
            )?,
            bytes::<usize>(layout.short_log().map_or(0, |log| 1_usize << log))?,
            bytes::<OptimalMatch>(MATCH_CAPACITY)?,
            bytes::<u8>(source_len)?,
            bytes::<SequenceWithOffset>(source_len / 3)?,
        ];
        owner_bytes.iter().try_fold(0_usize, |sum, &v| {
            sum.checked_add(v).ok_or(Error::ResourceLimit)
        })?;
        let settings = ReferenceFrameSettings::new(source_len, parameters.window_log())?;
        let frame_bound = ReferenceFrameHeader::raw_frame_bound(&settings)?;
        bytes::<u8>(frame_bound)?;
        Ok(Self {
            parameters,
            layout,
            settings,
            owner_bytes,
            frame_bound,
        })
    }
    pub(crate) const fn parameters(self) -> ResolvedCompressionParameters {
        self.parameters
    }
    pub(crate) const fn layout(self) -> OptimalSearchLayout {
        self.layout
    }
    pub(crate) const fn source_len(self) -> usize {
        self.settings.source_len()
    }
    pub(crate) const fn block_limit(self) -> usize {
        self.source_len()
    }
    pub(crate) const fn owner_bytes(self) -> [usize; 9] {
        self.owner_bytes
    }
    pub(crate) const fn frame_bound(self) -> usize {
        self.frame_bound
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompressionParameterRequest, RawCompressionParameters, SourceSize};
    fn raw() -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: 16,
            hash_log: 12,
            chain_log: 12,
            search_log: 6,
            min_match: 3,
            target_length: 4095,
            strategy: 7,
        }
    }
    #[test]
    fn exact_boundaries_have_nine_checked_independent_owner_sizes() {
        for size in [64, 65, 512, 1024, 2048, 4096, 8192, 16384] {
            for minimum in 3..=7 {
                let mut r = raw();
                r.min_match = minimum;
                let p = OptimalExecutionPlan::from_resolved_parameters(r.validate().unwrap(), size)
                    .unwrap();
                assert_eq!(p.layout().tree().key().cells(), 4096);
                assert_eq!(p.source_len(), size);
                assert_eq!(p.block_limit(), size);
                assert_eq!(p.parameters(), r.validate().unwrap());
                assert_eq!(
                    p.owner_bytes(),
                    [
                        377 * 4,
                        PATH_CAPACITY * core::mem::size_of::<PathCell>(),
                        MATCH_CAPACITY * 8,
                        4096 * core::mem::size_of::<usize>(),
                        4096 * core::mem::size_of::<usize>(),
                        if minimum == 3 {
                            65536 * core::mem::size_of::<usize>()
                        } else {
                            0
                        },
                        MATCH_CAPACITY * 8,
                        size,
                        size / 3 * core::mem::size_of::<SequenceWithOffset>()
                    ]
                );
                assert!(p.frame_bound() > size);
            }
        }
    }
    #[test]
    fn out_of_scope_source_or_resolved_geometry_is_resource_limited() {
        for size in [0, 1, 63, 16385, 32768, usize::MAX / 2] {
            assert_eq!(
                OptimalExecutionPlan::from_resolved_parameters(raw().validate().unwrap(), size),
                Err(Error::ResourceLimit)
            );
        }
        for field in 0..6 {
            let mut r = raw();
            match field {
                0 => r.window_log = 17,
                1 => r.hash_log = 13,
                2 => r.chain_log = 13,
                3 => r.search_log = 7,
                4 => r.target_length = 4096,
                _ => r.window_log = 13,
            };
            assert_eq!(
                OptimalExecutionPlan::from_resolved_parameters(r.validate().unwrap(), 16384),
                Err(Error::ResourceLimit)
            );
        }
        for strategy in [1, 6, 8, 9] {
            let mut r = raw();
            r.strategy = strategy;
            assert_eq!(
                OptimalExecutionPlan::from_resolved_parameters(r.validate().unwrap(), 0),
                Err(Error::UnsupportedFeature)
            );
        }
    }
    #[test]
    fn known_size_resolution_is_used_once_and_defaults_have_precise_bounds() {
        for level in [11, 12] {
            for size in [64, 512, 1024, 2048, 4096, 16384] {
                let request = CompressionParameterRequest::with_level(level);
                let parameters = request.resolve(SourceSize::Known(size as u64), 0).unwrap();
                let result = OptimalExecutionPlan::from_resolved_parameters(parameters, size);
                if size <= 2048 {
                    assert_eq!(result.unwrap().parameters(), parameters);
                } else {
                    assert_eq!(result, Err(Error::ResourceLimit));
                }
            }
        }
    }
}

#[cfg(test)]
mod ultra_plan_tests {
    use super::*;
    use crate::{CompressionParameterRequest, RawCompressionParameters};
    fn raw() -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: 16,
            hash_log: 12,
            chain_log: 12,
            search_log: 6,
            min_match: 3,
            target_length: 4095,
            strategy: 8,
        }
    }
    #[test]
    fn exact_boundaries_have_nine_checked_independent_owner_sizes() {
        for size in [64, 65, 512, 1024, 2048, 4096, 8192, 16384] {
            for minimum in 3..=7 {
                let mut r = raw();
                r.min_match = minimum;
                let p = OptimalExecutionPlan::from_ultra_parameters(r.validate().unwrap(), size)
                    .unwrap();
                assert_eq!(p.layout().tree().key().cells(), 4096);
                assert_eq!(p.source_len(), size);
                assert_eq!(p.block_limit(), size);
                assert_eq!(p.parameters(), r.validate().unwrap());
                assert_eq!(
                    p.owner_bytes(),
                    [
                        377 * 4,
                        PATH_CAPACITY * core::mem::size_of::<PathCell>(),
                        MATCH_CAPACITY * 8,
                        4096 * core::mem::size_of::<usize>(),
                        4096 * core::mem::size_of::<usize>(),
                        if minimum == 3 {
                            65536 * core::mem::size_of::<usize>()
                        } else {
                            0
                        },
                        MATCH_CAPACITY * 8,
                        size,
                        size / 3 * core::mem::size_of::<SequenceWithOffset>()
                    ]
                );
                assert!(p.frame_bound() > size);
            }
        }
    }
    #[test]
    fn out_of_scope_source_or_resolved_geometry_is_resource_limited() {
        for size in [0, 1, 63, 16385, 32768, usize::MAX / 2] {
            assert_eq!(
                OptimalExecutionPlan::from_ultra_parameters(raw().validate().unwrap(), size),
                Err(Error::ResourceLimit)
            );
        }
        for field in 0..6 {
            let mut r = raw();
            match field {
                0 => r.window_log = 17,
                1 => r.hash_log = 13,
                2 => r.chain_log = 13,
                3 => r.search_log = 7,
                4 => r.target_length = 4096,
                _ => r.window_log = 13,
            };
            assert_eq!(
                OptimalExecutionPlan::from_ultra_parameters(r.validate().unwrap(), 16384),
                Err(Error::ResourceLimit)
            );
        }
        for strategy in [1, 6, 7, 9] {
            let mut r = raw();
            r.strategy = strategy;
            assert_eq!(
                OptimalExecutionPlan::from_ultra_parameters(r.validate().unwrap(), 0),
                Err(Error::UnsupportedFeature)
            );
        }
    }
    #[test]
    fn original_admission_and_public_dispatch_remain_closed_for_ultra() {
        let parameters = raw().validate().unwrap();
        assert_eq!(
            OptimalExecutionPlan::from_resolved_parameters(parameters, 64),
            Err(Error::UnsupportedFeature)
        );
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_strategy(Some(ReferenceStrategy::Ultra));
        assert!(matches!(
            crate::reference_search::ReferenceSearchPlan::new(&request, 64),
            Err(Error::UnsupportedFeature)
        ));
    }
    #[test]
    fn all_native_ultra_geometries_produce_exact_checked_plans() {
        let mut count = 0;
        for case in crate::ultra_fresh_fixture::read(crate::ultra_fresh_fixture::PACK)
            .unwrap()
            .into_iter()
            .filter(|c| c.policy == 2)
        {
            let parameters = RawCompressionParameters {
                window_log: case.window.into(),
                hash_log: case.hash.into(),
                chain_log: case.chain.into(),
                search_log: case.search.into(),
                min_match: case.minimum,
                target_length: case.target,
                strategy: 8,
            }
            .validate()
            .unwrap();
            let plan =
                OptimalExecutionPlan::from_ultra_parameters(parameters, case.source.len()).unwrap();
            assert_eq!(plan.parameters(), parameters);
            assert_eq!(plan.source_len(), case.source.len());
            assert_eq!(plan.owner_bytes()[7], case.source.len());
            assert_eq!(
                plan.owner_bytes()[8],
                case.source.len() / 3 * core::mem::size_of::<SequenceWithOffset>()
            );
            assert_eq!(plan.layout().tree().key().cells(), 1_usize << case.hash);
            count += 1;
        }
        assert_eq!(count, 22);
    }
}
