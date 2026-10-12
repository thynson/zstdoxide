//! Allocation-free admission for bounded, no-dictionary reference Fast.
//!
//! This scalar plan neither allocates match storage nor encodes a frame. It
//! keeps reference resolution separate from the existing named Rust policies.
//! Every backend shares these checked values; no unsafe counterpart is useful.

use core::alloc::Layout;

use crate::frame::MAX_BLOCK_SIZE;
use crate::match_search::SearchKeyLayout;
use crate::{
    CompressionParameterRequest, Error, ReferenceFrameParameters, ReferenceStrategy,
    ResolvedCompressionParameters, Result, SourceSize,
};

const MAX_WINDOW_LOG: u8 = 20;
const MAX_HASH_LOG: u8 = 18;

/// An admitted known-size profile and the controls actually used by Fast.
///
/// Corresponds to resolved C compression parameters plus the no-dictionary
/// Fast dispatch, sampling and literal policies. Chain/search fields remain
/// validated in `parameters`; they do not create a chain or visit budget.
/// Positions will be indices into one borrowed input, not retained pointers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Fast parser and frame consumers follow admission")
)]
pub(crate) struct FastExecutionPlan {
    parameters: ResolvedCompressionParameters,
    source_len: usize,
    source_size: u64,
    layout: SearchKeyLayout,
    table_bytes: usize,
    window_size: usize,
    block_limit: usize,
    initial_step: usize,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Fast parser and frame consumers follow admission")
)]
impl FastExecutionPlan {
    /// Resolve known input with no dictionary, then admit its effective profile.
    ///
    /// Error order is source-domain validation and complete reference
    /// resolution, unsupported strategy, then supported-strategy resource
    /// limits and target layout checks. Thus a large valid DoubleFast profile
    /// is still UnsupportedFeature, while an oversized Fast profile is
    /// ResourceLimit. Request setters already reject invalid scalar overrides.
    /// A known empty input stays known; source hints cannot replace it.
    ///
    /// Corresponds to `ZSTD_getCParamsFromCCtxParams` for scalar resolution.
    /// The operational caps are zstdoxide policy, not upstream validity bounds.
    pub(crate) fn new(request: &CompressionParameterRequest, source_len: usize) -> Result<Self> {
        let source_size = Self::checked_source_size(source_len)?;
        let parameters = request.resolve(SourceSize::Known(source_size), 0)?;
        Self::admit(parameters, source_len, source_size)
    }

    /// Admit already resolved parameters without applying source adjustment again.
    /// The caller supplies the actual known input length used for resolution.
    pub(crate) fn from_resolved_parameters(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
    ) -> Result<Self> {
        Self::admit(
            parameters,
            source_len,
            Self::checked_source_size(source_len)?,
        )
    }

    /// Compare private parser state at the exact applied C oracle parameters.
    /// The ordinary-frame helper calls the internal begin API with explicit
    /// parameters, bypassing public known-source adjustment. This test boundary
    /// retains all operational admission checks and never changes that resolver.
    #[cfg(test)]
    pub(crate) fn from_observed_parameters(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
    ) -> Result<Self> {
        Self::from_resolved_parameters(parameters, source_len)
    }

    fn checked_source_size(source_len: usize) -> Result<u64> {
        let source_size = u64::try_from(source_len).map_err(|_| Error::ResourceLimit)?;
        if source_size == u64::MAX {
            return Err(Error::InvalidParameter);
        }
        Ok(source_size)
    }

    // The validated record makes lower scalar bounds and enum validity
    // unrepresentable here. Keeping admission separate lets tests use frozen C
    // observations without obtaining their expected values from our resolver.
    fn admit(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
        source_size: u64,
    ) -> Result<Self> {
        if parameters.strategy() != ReferenceStrategy::Fast {
            return Err(Error::UnsupportedFeature);
        }
        if parameters.window_log() > MAX_WINDOW_LOG || parameters.hash_log() > MAX_HASH_LOG {
            return Err(Error::ResourceLimit);
        }
        // ZSTD_compressBlock_fast dispatches minMatch 3 through key width 4
        // (zstd_fast.c:443–477); emitted matches still have minimum length 4.
        let layout =
            SearchKeyLayout::for_fast(parameters.hash_log(), parameters.min_match().max(4))?;
        // The actual table stores usize positions. Layout checks both the
        // multiplication and the target's alignment/isize allocation limit.
        // This is requested table payload only, not total codec/allocator memory.
        let table_bytes = Layout::array::<usize>(layout.bucket_count())
            .map_err(|_| Error::ResourceLimit)?
            .size();
        let window_size = 1_usize
            .checked_shl(u32::from(parameters.window_log()))
            .ok_or(Error::ResourceLimit)?;
        let initial_step = usize::try_from(parameters.target_length())
            .map_err(|_| Error::ResourceLimit)?
            .max(1)
            .checked_add(1)
            .ok_or(Error::ResourceLimit)?;
        Ok(Self {
            parameters,
            source_len,
            source_size,
            layout,
            table_bytes,
            window_size,
            block_limit: window_size.min(MAX_BLOCK_SIZE),
            initial_step,
        })
    }

    pub(crate) const fn parameters(&self) -> ResolvedCompressionParameters {
        self.parameters
    }

    pub(crate) const fn source_len(&self) -> usize {
        self.source_len
    }

    /// Checked known size for frame fields; never the unknown sentinel.
    pub(crate) const fn source_size(&self) -> u64 {
        self.source_size
    }

    /// Exact initialized key width, distinct from the emitted match floor.
    pub(crate) const fn key_bytes(&self) -> u8 {
        self.layout.key_bytes()
    }

    /// The admitted private Fast layout; named policy bounds remain unchanged.
    pub(crate) const fn layout(&self) -> SearchKeyLayout {
        self.layout
    }

    pub(crate) const fn bucket_count(&self) -> usize {
        self.layout.bucket_count()
    }

    /// Requested usize-cell table payload, excluding other owners/over-allocation.
    pub(crate) const fn table_bytes(&self) -> usize {
        self.table_bytes
    }

    pub(crate) const fn window_size(&self) -> usize {
        self.window_size
    }

    pub(crate) const fn block_limit(&self) -> usize {
        self.block_limit
    }

    /// Gap between sampled position pairs, not a one-byte probe stride.
    /// Corresponds to `targetLength + !targetLength + 1` in zstd_fast.c:200.
    pub(crate) const fn initial_step(&self) -> usize {
        self.initial_step
    }

    /// Raw-only literals also bypass literal RLE; block RLE is a separate choice.
    /// Corresponds to automatic `ZSTD_literalsCompressionIsDisabled`.
    pub(crate) const fn raw_literals(&self) -> bool {
        crate::reference_policy::raw_literals(self.parameters)
    }

    /// Reference defaults for the new entry points, independent of named APIs.
    pub(crate) const fn frame_parameters(&self) -> ReferenceFrameParameters {
        ReferenceFrameParameters::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    use crate::RawCompressionParameters;

    const QUERIES: &str = include_str!("../tests/upstream/parameters/queries.tsv");
    const REQUESTS: &str = include_str!("../tests/upstream/parameters/requests.tsv");
    const ADJUSTMENTS: &str = include_str!("../tests/upstream/parameters/adjustments.tsv");

    fn raw(fields: [u32; 7]) -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: fields[0],
            chain_log: fields[1],
            hash_log: fields[2],
            search_log: fields[3],
            min_match: fields[4],
            target_length: fields[5],
            strategy: fields[6],
        }
    }

    fn row(source: &str, line: usize, expected_columns: usize) -> Vec<&str> {
        let fields: Vec<_> = source.lines().nth(line - 1).unwrap().split('\t').collect();
        assert_eq!(fields.len(), expected_columns, "fixture line {line}");
        fields
    }

    fn observed(fields: &[&str], start: usize) -> RawCompressionParameters {
        raw(core::array::from_fn(|i| fields[start + i].parse().unwrap()))
    }

    fn assert_query(line: usize, level: i32, size: usize, admitted: bool) {
        let fields = row(QUERIES, line, 18);
        assert_eq!(fields[1], "public");
        assert_eq!(fields[2].parse::<i32>().unwrap(), level);
        assert_eq!(fields[3].parse::<usize>().unwrap(), size);
        assert_eq!(&fields[4..7], &["0", "ordinary", "known"]);
        assert_eq!(fields[17], "0");
        let expected = observed(&fields, 7);
        let request = CompressionParameterRequest::with_level(level);
        let before = request;
        // Admission first receives an independently captured, validated record.
        let direct = FastExecutionPlan::admit(expected.validate().unwrap(), size, size as u64);
        let resolved = FastExecutionPlan::new(&request, size);
        assert_eq!(request, before);
        if admitted {
            let plan = resolved.unwrap();
            assert_eq!(plan, direct.unwrap());
            assert_eq!(plan.parameters().to_raw(), expected);
            assert_eq!(plan.source_len(), size);
            assert_eq!(plan.source_size(), size as u64);
            let flags = plan.frame_parameters();
            assert_eq!(flags.include_content_size(), fields[14] == "1");
            assert_eq!(flags.checksum(), fields[15] == "1");
            assert_eq!(flags.include_dictionary_id(), fields[16] == "1");
        } else {
            assert_eq!(direct, Err(Error::UnsupportedFeature));
            assert_eq!(resolved, Err(Error::UnsupportedFeature));
        }
    }

    #[test]
    fn captured_level_one_profiles_cover_all_four_source_tiers() {
        for (line, size, key, window, block) in [
            (30, 16_384, 5, 16_384, 16_384),
            (31, 131_072, 6, 131_072, 131_072),
            (32, 262_144, 6, 262_144, 131_072),
            (33, 262_145, 7, 524_288, 131_072),
        ] {
            assert_query(line, 1, size, true);
            let plan =
                FastExecutionPlan::new(&CompressionParameterRequest::with_level(1), size).unwrap();
            assert_eq!(
                (plan.key_bytes(), plan.window_size(), plan.block_limit()),
                (key, window, block)
            );
            assert_eq!(plan.initial_step(), 2);
            assert!(!plan.raw_literals());
        }
    }

    #[test]
    fn captured_negative_extrema_normalize_before_sampling_arithmetic() {
        for (first, level) in [(2, i32::MIN), (6, -131_073), (10, -131_072), (22, -1)] {
            for (offset, size) in [16_384, 131_072, 262_144, 262_145].into_iter().enumerate() {
                assert_query(first + offset, level, size, true);
                let plan =
                    FastExecutionPlan::new(&CompressionParameterRequest::with_level(level), size)
                        .unwrap();
                assert_eq!(plan.initial_step(), if level == -1 { 2 } else { 131_073 });
                assert!(plan.raw_literals());
            }
        }
        let fields = row(REQUESTS, 3, 40);
        assert_eq!(&fields[..2], &["level", "1"]);
        let empty =
            FastExecutionPlan::new(&CompressionParameterRequest::with_level(i32::MIN), 0).unwrap();
        assert_eq!(empty.parameters().to_raw(), observed(&fields, 19));
        assert_eq!(empty.source_size(), 0);
    }

    #[test]
    fn captured_level_two_reenters_fast_above_the_double_fast_tier() {
        for (line, size, admitted) in [
            (127, 16_383, true),
            (131, 16_384, true),
            (135, 16_385, true),
            (139, 131_071, true),
            (143, 131_072, true),
            (147, 131_073, false),
            (151, 262_143, false),
            (155, 262_144, false),
            (159, 262_145, true),
        ] {
            assert_query(line, 2, size, admitted);
        }
    }

    #[test]
    fn captured_default_and_zero_profiles_reject_their_actual_strategy() {
        for (first, level) in [(26, 0), (38, 3)] {
            for (offset, size) in [16_384, 131_072, 262_144, 262_145].into_iter().enumerate() {
                assert_query(first + offset, level, size, false);
            }
        }
    }

    #[test]
    fn explicit_strategy_overrides_defeat_level_allowlists() {
        let fields = row(REQUESTS, 77, 40);
        assert_eq!(&fields[..2], &["strategy", "5"]);
        let mut request = CompressionParameterRequest::new();
        request.set_strategy(Some(ReferenceStrategy::Fast));
        for (size, start) in [(0, 19), (16_385, 33)] {
            let plan = FastExecutionPlan::new(&request, size).unwrap();
            assert_eq!(plan.parameters().to_raw(), observed(&fields, start));
        }
        request.set_strategy(Some(ReferenceStrategy::DoubleFast));
        request.set_level(1);
        assert_eq!(
            FastExecutionPlan::new(&request, 16_384),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(request.level(), 1);
    }

    #[test]
    fn small_hash_dimensions_preserve_adjustment_order_and_actual_cell_size() {
        // The new combinations below derive from pinned clevels.h tier 3/1 and
        // zstd_compress.c:1552–1570, not from a new executed C observation.
        for (size, expected, buckets) in [
            (0, [10, 6, 7, 1, 5, 0, 1], 128),
            (64, [10, 6, 7, 1, 5, 0, 1], 128),
            (65, [10, 7, 8, 1, 5, 0, 1], 256),
            (128, [10, 7, 8, 1, 5, 0, 1], 256),
            (129, [10, 8, 9, 1, 5, 0, 1], 512),
        ] {
            let plan =
                FastExecutionPlan::new(&CompressionParameterRequest::with_level(1), size).unwrap();
            assert_eq!(plan.parameters().to_raw(), raw(expected));
            assert_eq!(plan.bucket_count(), buckets);
            assert_eq!(plan.table_bytes(), buckets * size_of::<usize>());
        }
        // This exact valid record is independently observed after C's signed
        // clamp. It does not mean checked request setters accept u32::MAX.
        let fields = row(ADJUSTMENTS, 224, 21);
        assert_eq!(fields[0], "all_unsigned_max");
        let expected = observed(&fields, 13);
        assert_eq!(expected, raw([10, 6, 6, 1, 3, 0, 1]));
        let plan = FastExecutionPlan::admit(expected.validate().unwrap(), 0, 0).unwrap();
        assert_eq!((plan.key_bytes(), plan.bucket_count()), (4, 64));
        assert_eq!(plan.table_bytes(), 64 * size_of::<usize>());
    }

    #[test]
    fn minimum_match_three_uses_four_initialized_key_bytes() {
        for (minimum, key) in [(3, 4), (4, 4), (5, 5), (6, 6), (7, 7)] {
            let mut request = CompressionParameterRequest::with_level(1);
            request.set_min_match(minimum).unwrap();
            request.set_hash_log(6).unwrap();
            let plan = FastExecutionPlan::new(&request, 16_384).unwrap();
            assert_eq!(
                plan.parameters().to_raw(),
                raw([14, 14, 6, 1, minimum.into(), 0, 1])
            );
            assert_eq!(plan.key_bytes(), key);
            assert_eq!(plan.bucket_count(), 64);
        }
    }

    #[test]
    fn resource_limits_apply_to_resolved_values_after_both_adjustments() {
        // Manual expectations from the pinned override and adjustment rules;
        // these combinations are not presented as captured C fixture rows.
        for (size, window, hash, expected, error) in [
            (1_048_576, 0, 0, [19, 13, 14, 1, 7, 0, 1], None),
            (1_048_576, 20, 0, [20, 13, 14, 1, 7, 0, 1], None),
            (
                1_048_577,
                21,
                0,
                [21, 13, 14, 1, 7, 0, 1],
                Some(Error::ResourceLimit),
            ),
            (1_048_576, 21, 0, [20, 13, 14, 1, 7, 0, 1], None),
            (1_048_576, 0, 18, [19, 13, 18, 1, 7, 0, 1], None),
            (
                1_048_576,
                0,
                19,
                [19, 13, 19, 1, 7, 0, 1],
                Some(Error::ResourceLimit),
            ),
            (16_384, 0, 19, [14, 14, 15, 1, 5, 0, 1], None),
            (
                16_384,
                if usize::BITS == 32 { 30 } else { 31 },
                30,
                [14, 14, 15, 1, 5, 0, 1],
                None,
            ),
            (16_384, 10, 0, [10, 10, 11, 1, 5, 0, 1], None),
        ] {
            let mut request = CompressionParameterRequest::with_level(1);
            request.set_window_log(window).unwrap();
            request.set_hash_log(hash).unwrap();
            let before = request;
            let resolved = request.resolve(SourceSize::Known(size as u64), 0).unwrap();
            assert_eq!(resolved.to_raw(), raw(expected));
            let result = FastExecutionPlan::new(&request, size);
            if let Some(error) = error {
                assert_eq!(result, Err(error));
            } else {
                assert_eq!(result.unwrap().parameters(), resolved);
            }
            assert_eq!(request, before);
        }
    }

    #[test]
    fn maximum_admitted_dimensions_have_a_checked_target_layout() {
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_window_log(20).unwrap();
        request.set_hash_log(18).unwrap();
        request.set_min_match(7).unwrap();
        request.set_target_length(131_072).unwrap();
        let plan = FastExecutionPlan::new(&request, 1_048_576).unwrap();
        assert_eq!(
            plan.parameters().to_raw(),
            raw([20, 13, 18, 1, 7, 131072, 1])
        );
        assert_eq!(plan.bucket_count(), 262_144);
        assert_eq!(plan.layout(), SearchKeyLayout::for_fast(18, 7).unwrap());
        assert_eq!(plan.table_bytes(), 262_144 * size_of::<usize>());
        assert_eq!(plan.window_size(), 1_048_576);
        assert_eq!(plan.block_limit(), 131_072);
        assert_eq!(plan.initial_step(), 131_073);
        let layout = Layout::array::<usize>(plan.bucket_count()).unwrap();
        assert_eq!(layout.size(), plan.table_bytes());
        assert_eq!(layout.align(), align_of::<usize>());
    }

    #[test]
    fn validated_chain_and_search_fields_create_no_fast_effort_controls() {
        let mut request = CompressionParameterRequest::with_level(1);
        let original = FastExecutionPlan::new(&request, 1_048_576).unwrap();
        for (chain, search, expected) in [
            (29, 29, [19, 19, 14, 29, 7, 0, 1]),
            (6, 1, [19, 6, 14, 1, 7, 0, 1]),
        ] {
            request.set_chain_log(chain).unwrap();
            request.set_search_log(search).unwrap();
            let plan = FastExecutionPlan::new(&request, 1_048_576).unwrap();
            assert_eq!(plan.parameters().to_raw(), raw(expected));
            assert_eq!(plan.key_bytes(), original.key_bytes());
            assert_eq!(plan.bucket_count(), original.bucket_count());
            assert_eq!(plan.table_bytes(), original.table_bytes());
            assert_eq!(plan.window_size(), original.window_size());
            assert_eq!(plan.block_limit(), original.block_limit());
            assert_eq!(plan.initial_step(), original.initial_step());
            assert_eq!(plan.raw_literals(), original.raw_literals());
        }
    }

    #[test]
    fn known_size_overrides_hints_even_when_the_hint_would_change_admission() {
        let fields = row(REQUESTS, 133, 40);
        assert_eq!(&fields[..2], &["source_hint_small", "1"]);
        let mut reference = CompressionParameterRequest::new();
        reference.set_source_hint(16_384).unwrap();
        for (source, start) in [
            (SourceSize::Known(0), 19),
            (SourceSize::Unknown, 26),
            (SourceSize::Known(16_385), 33),
        ] {
            assert_eq!(
                reference.resolve(source, 0).unwrap().to_raw(),
                observed(&fields, start)
            );
        }
        for (level, size, expected, admitted) in [
            (2, 262_144, [18, 14, 14, 1, 5, 0, 2], false),
            (2, 262_145, [19, 15, 16, 1, 6, 0, 1], true),
            (1, 0, [10, 6, 7, 1, 5, 0, 1], true),
        ] {
            let mut request = CompressionParameterRequest::with_level(level);
            let without_hint = FastExecutionPlan::new(&request, size);
            request.set_source_hint(16_384).unwrap();
            let before = request;
            let with_hint = FastExecutionPlan::new(&request, size);
            assert_eq!(with_hint, without_hint);
            assert_eq!(request, before);
            assert_eq!(
                request
                    .resolve(SourceSize::Known(size as u64), 0)
                    .unwrap()
                    .to_raw(),
                raw(expected)
            );
            if admitted {
                assert!(with_hint.is_ok());
            } else {
                assert_eq!(with_hint, Err(Error::UnsupportedFeature));
            }
        }
    }

    #[test]
    fn source_domain_then_strategy_then_resource_checks_have_typed_errors() {
        let unsupported = raw([21, 13, 19, 1, 7, 0, 2]).validate().unwrap();
        assert_eq!(
            FastExecutionPlan::admit(unsupported, 1_048_577, 1_048_577),
            Err(Error::UnsupportedFeature)
        );
        let oversized = raw([21, 13, 19, 1, 7, 0, 1]).validate().unwrap();
        assert_eq!(
            FastExecutionPlan::admit(oversized, 1_048_577, 1_048_577),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            raw([9, 13, 14, 1, 7, 0, 1]).validate(),
            Err(Error::InvalidParameter)
        );
        // On 64-bit usize::MAX is the reserved u64 sentinel; on 32-bit it is
        // an ordinary known scalar size. Neither branch fabricates a slice.
        if usize::BITS == 64 {
            for request in [
                CompressionParameterRequest::new(),
                CompressionParameterRequest::with_level(1),
            ] {
                assert_eq!(
                    FastExecutionPlan::new(&request, usize::MAX),
                    Err(Error::InvalidParameter)
                );
            }
        } else if usize::BITS == 32 {
            let plan =
                FastExecutionPlan::new(&CompressionParameterRequest::with_level(1), usize::MAX)
                    .unwrap();
            assert_eq!(plan.source_size(), u64::from(u32::MAX));
        }
    }

    #[test]
    fn literal_policy_and_minimum_gain_are_distinct_from_sampling_step() {
        for (target, step, raw_only) in [(0, 2, false), (1, 2, true), (131_072, 131_073, true)] {
            let mut request = CompressionParameterRequest::with_level(1);
            request.set_target_length(target).unwrap();
            let plan = FastExecutionPlan::new(&request, 16_384).unwrap();
            assert_eq!(plan.initial_step(), step);
            assert_eq!(plan.raw_literals(), raw_only);
        }
        for (size, gain) in [
            (0, 2),
            (1, 2),
            (63, 2),
            (64, 3),
            (127, 3),
            (128, 4),
            (131_072, 2050),
        ] {
            assert_eq!(crate::reference_policy::minimum_gain(size), gain);
        }
        assert_eq!(
            crate::reference_policy::minimum_gain(usize::MAX),
            usize::MAX / 64 + 2
        );
    }
}
