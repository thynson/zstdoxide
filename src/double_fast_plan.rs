//! Allocation-free admission for the bounded no-dictionary DoubleFast path.
//!
//! These copied values own no storage and have one shared safe representation.
//! Long and short tables are independent hash owners; chainLog is not a chain.
#![forbid(unsafe_code)]

use crate::frame::MAX_BLOCK_SIZE;
use crate::match_search::SearchKeyLayout;
use crate::{
    CompressionParameterRequest, Error, ReferenceStrategy, ResolvedCompressionParameters, Result,
    SourceSize,
};
use core::alloc::Layout;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "DoubleFast ownership and execution integration follows"
    )
)]
pub(crate) struct DoubleFastExecutionPlan {
    parameters: ResolvedCompressionParameters,
    source_len: usize,
    source_size: u64,
    long_layout: SearchKeyLayout,
    short_layout: SearchKeyLayout,
    long_table_bytes: usize,
    short_table_bytes: usize,
    table_bytes: usize,
    window_size: usize,
    block_limit: usize,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "DoubleFast ownership and execution integration follows"
    )
)]
impl DoubleFastExecutionPlan {
    /// Resolve with the actual known size and no dictionary, then check the
    /// selected algorithm and its independent resource limits before any owner
    /// is constructed. Source hints cannot replace Known(0). Frame output-bound
    /// arithmetic belongs to the shared framing preflight before allocation.
    /// Corresponds to scalar resolution and noDict DoubleFast dispatch in C;
    /// these resource caps are zstdoxide policy, not upstream validity bounds.
    pub(crate) fn new(request: &CompressionParameterRequest, source_len: usize) -> Result<Self> {
        let source_size = u64::try_from(source_len).map_err(|_| Error::ResourceLimit)?;
        if source_size == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        let parameters = request.resolve(SourceSize::Known(source_size), 0)?;
        Self::admit(parameters, source_len, source_size)
    }

    /// Preserve the ordinary C helper's explicitly applied internal-begin tuple.
    /// Public known-source adjustment remains a separate tested boundary; all
    /// algorithm and resource checks still apply to this test-only constructor.
    #[cfg(test)]
    pub(crate) fn from_observed_parameters(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
    ) -> Result<Self> {
        let source_size = u64::try_from(source_len).map_err(|_| Error::ResourceLimit)?;
        if source_size == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        Self::admit(parameters, source_len, source_size)
    }

    fn admit(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
        source_size: u64,
    ) -> Result<Self> {
        if parameters.strategy() != ReferenceStrategy::DoubleFast {
            return Err(Error::UnsupportedFeature);
        }
        if parameters.window_log() > 21 || parameters.hash_log() > 18 || parameters.chain_log() > 18
        {
            return Err(Error::ResourceLimit);
        }
        let long_layout = SearchKeyLayout::for_double_fast_long(parameters.hash_log())?;
        // C dispatches minMatch=3 through its four-byte short-key specialization.
        let short_layout =
            SearchKeyLayout::for_fast(parameters.chain_log(), parameters.min_match().max(4))?;
        let long_table_bytes = Layout::array::<usize>(long_layout.bucket_count())
            .map_err(|_| Error::ResourceLimit)?
            .size();
        let short_table_bytes = Layout::array::<usize>(short_layout.bucket_count())
            .map_err(|_| Error::ResourceLimit)?
            .size();
        let table_bytes = long_table_bytes
            .checked_add(short_table_bytes)
            .ok_or(Error::ResourceLimit)?;
        let window_size = 1_usize
            .checked_shl(u32::from(parameters.window_log()))
            .ok_or(Error::ResourceLimit)?;
        Ok(Self {
            parameters,
            source_len,
            source_size,
            long_layout,
            short_layout,
            long_table_bytes,
            short_table_bytes,
            table_bytes,
            window_size,
            block_limit: window_size.min(MAX_BLOCK_SIZE),
        })
    }
    pub(crate) const fn parameters(&self) -> ResolvedCompressionParameters {
        self.parameters
    }
    pub(crate) const fn source_len(&self) -> usize {
        self.source_len
    }
    pub(crate) const fn source_size(&self) -> u64 {
        self.source_size
    }
    pub(crate) const fn long_layout(&self) -> SearchKeyLayout {
        self.long_layout
    }
    pub(crate) const fn short_layout(&self) -> SearchKeyLayout {
        self.short_layout
    }
    pub(crate) const fn long_table_bytes(&self) -> usize {
        self.long_table_bytes
    }
    pub(crate) const fn short_table_bytes(&self) -> usize {
        self.short_table_bytes
    }
    /// Requested payload for both position arrays; excludes every other owner.
    pub(crate) const fn table_bytes(&self) -> usize {
        self.table_bytes
    }
    pub(crate) const fn window_size(&self) -> usize {
        self.window_size
    }
    pub(crate) const fn block_limit(&self) -> usize {
        self.block_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RawCompressionParameters;
    use alloc::vec::Vec;

    fn request() -> CompressionParameterRequest {
        CompressionParameterRequest::with_level(3)
    }
    fn raw(fields: &[&str]) -> RawCompressionParameters {
        let values: [u32; 7] = core::array::from_fn(|i| fields[i].parse().unwrap());
        RawCompressionParameters {
            window_log: values[0],
            chain_log: values[1],
            hash_log: values[2],
            search_log: values[3],
            min_match: values[4],
            target_length: values[5],
            strategy: values[6],
        }
    }

    #[test]
    fn captured_known_source_profiles_admit_the_actual_double_fast_strategy() {
        let mut checked = 0;
        let mut admitted = 0;
        for line in include_str!("../tests/upstream/parameters/queries.tsv")
            .lines()
            .skip(1)
        {
            let f: Vec<_> = line.split('\t').collect();
            assert_eq!(f.len(), 18);
            if f[1] != "public" || f[4..7] != ["0", "ordinary", "known"] || f[17] != "0" {
                continue;
            }
            let source_size: u64 = f[3].parse().unwrap();
            let Ok(source_len) = usize::try_from(source_size) else {
                continue;
            };
            let request = CompressionParameterRequest::with_level(f[2].parse().unwrap());
            let expected = raw(&f[7..14]);
            let actual = DoubleFastExecutionPlan::new(&request, source_len);
            checked += 1;
            if expected.strategy == 2 {
                let direct = DoubleFastExecutionPlan::admit(
                    expected.validate().unwrap(),
                    source_len,
                    source_size,
                );
                let plan = actual.unwrap();
                assert_eq!(direct, Ok(plan));
                assert_eq!(plan.parameters().to_raw(), expected);
                assert_eq!(
                    (plan.source_len(), plan.source_size()),
                    (source_len, source_size)
                );
                assert_eq!(plan.long_layout().key_bytes(), 8);
                assert_eq!(
                    plan.short_layout().key_bytes(),
                    plan.parameters().min_match().max(4)
                );
                assert_eq!(
                    plan.table_bytes(),
                    plan.long_table_bytes() + plan.short_table_bytes()
                );
                admitted += 1;
            } else {
                assert_eq!(actual, Err(Error::UnsupportedFeature));
            }
        }
        if usize::BITS == 64 {
            assert_eq!((checked, admitted), (190, 29));
        } else if usize::BITS == 32 {
            assert_eq!((checked, admitted), (188, 28));
        }
    }

    #[test]
    fn table_dimensions_have_separate_checked_target_costs() {
        for (long, short) in [(6, 18), (18, 6), (18, 18)] {
            let mut r = request();
            r.set_window_log(21).unwrap();
            r.set_hash_log(long).unwrap();
            r.set_chain_log(short).unwrap();
            let p = DoubleFastExecutionPlan::new(&r, 2_097_152).unwrap();
            assert_eq!(
                (p.long_layout().hash_log(), p.short_layout().hash_log()),
                (long, short)
            );
            assert_eq!(p.long_table_bytes(), (1_usize << long) * size_of::<usize>());
            assert_eq!(
                p.short_table_bytes(),
                (1_usize << short) * size_of::<usize>()
            );
            assert_eq!(
                p.table_bytes(),
                p.long_table_bytes() + p.short_table_bytes()
            );
            assert_eq!((p.window_size(), p.block_limit()), (2_097_152, 131_072));
        }
    }

    #[test]
    fn resource_checks_follow_known_size_adjustment_and_algorithm_selection() {
        for dimension in 0..3 {
            let mut r = request();
            match dimension {
                0 => r.set_window_log(22).unwrap(),
                1 => r.set_hash_log(19).unwrap(),
                _ => r.set_chain_log(19).unwrap(),
            };
            assert_eq!(
                DoubleFastExecutionPlan::new(&r, 4_194_304),
                Err(Error::ResourceLimit)
            );
            assert!(DoubleFastExecutionPlan::new(&r, 16_384).is_ok());
            r.set_strategy(Some(ReferenceStrategy::Greedy));
            assert_eq!(
                DoubleFastExecutionPlan::new(&r, 4_194_304),
                Err(Error::UnsupportedFeature)
            );
        }
        // DoubleFast's window cap must not widen existing Fast admission.
        let mut r = CompressionParameterRequest::with_level(1);
        r.set_window_log(21).unwrap();
        assert_eq!(
            crate::fast_plan::FastExecutionPlan::new(&r, 1_048_577),
            Err(Error::ResourceLimit)
        );
        r.set_strategy(Some(ReferenceStrategy::DoubleFast));
        assert_eq!(
            DoubleFastExecutionPlan::new(&r, 1_048_577)
                .unwrap()
                .window_size(),
            2_097_152
        );
    }

    #[test]
    fn minimum_match_dispatch_preserves_eight_byte_long_keys() {
        for minimum in 3..=7 {
            let mut r = request();
            r.set_min_match(minimum).unwrap();
            r.set_hash_log(6).unwrap();
            r.set_chain_log(7).unwrap();
            let p = DoubleFastExecutionPlan::new(&r, 16_384).unwrap();
            assert_eq!(
                (p.long_layout().key_bytes(), p.short_layout().key_bytes()),
                (8, minimum.max(4))
            );
            assert_eq!(
                (
                    p.long_layout().bucket_count(),
                    p.short_layout().bucket_count()
                ),
                (64, 128)
            );
        }
    }

    #[test]
    fn search_and_target_fields_add_no_storage_or_fast_literal_policy() {
        let plain = DoubleFastExecutionPlan::new(&request(), 2_097_153).unwrap();
        let mut r = request();
        r.set_search_log(29).unwrap();
        r.set_target_length(131_072).unwrap();
        let changed = DoubleFastExecutionPlan::new(&r, 2_097_153).unwrap();
        assert_eq!(
            (
                changed.long_layout(),
                changed.short_layout(),
                changed.table_bytes(),
                changed.window_size(),
                changed.block_limit()
            ),
            (
                plain.long_layout(),
                plain.short_layout(),
                plain.table_bytes(),
                plain.window_size(),
                plain.block_limit()
            )
        );
        assert_eq!(changed.parameters().search_log(), 29);
        assert_eq!(changed.parameters().target_length(), 131_072);
    }

    #[test]
    fn source_domain_empty_resolution_and_public_admission_remain_separate() {
        let zero =
            DoubleFastExecutionPlan::new(&CompressionParameterRequest::with_level(0), 0).unwrap();
        let three = DoubleFastExecutionPlan::new(&request(), 0).unwrap();
        assert_eq!(zero, three);
        assert_eq!(zero.source_size(), 0);
        assert_eq!(zero.window_size(), 1024);
        let mut hinted = request();
        hinted.set_source_hint(1_048_576).unwrap();
        let before = hinted;
        assert_eq!(DoubleFastExecutionPlan::new(&hinted, 0), Ok(three));
        assert_eq!(hinted, before);
        if usize::BITS == 64 {
            assert_eq!(
                DoubleFastExecutionPlan::new(&request(), usize::MAX),
                Err(Error::InvalidParameter)
            );
        }
        // A private plan is not an implemented encoder. Keep public behavior
        // unchanged until parsing, entropy and frame publication are wired.
        assert_eq!(
            crate::compress_at_level(b"still unsupported", 3),
            Err(Error::UnsupportedFeature)
        );
    }
}
