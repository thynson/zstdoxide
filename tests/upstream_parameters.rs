//! Source-anchored scalar assertions from facebook/zstd v1.5.7.
//!
//! Reference revision: f8745da6ff1ad1e7bab384bd1f9d742439278e99.
//! These public inspection values do not configure the live encoder or prove
//! numerical-level execution. Public C size-zero calls are explicitly adapted
//! to SourceSize::Unknown; typed Known(0) retains empty-source semantics.
//!
//! Derived assertion contracts use the upstream BSD-3-Clause option; see
//! LICENSE.zstd. Bulk context setters and numeric execution remain separate.
#![forbid(unsafe_code)]

use zstdoxide::{
    CompressionParameterRequest, EncoderOptions, Error, RawCompressionParameters,
    ReferenceFrameParameters, ReferenceStrategy, SourceSize, inspect_compression_level,
};

/// fuzzer.c:680-690: observe the minimum and default reference levels.
/// The Rust request's constants replace the C standalone level getters.
/// Query equivalence does not imply a level can execute in the Rust encoder.
#[test]
fn reference_minimum_and_default_levels_preserve_original_scalar_assertions() {
    assert_eq!(CompressionParameterRequest::MIN_LEVEL, -131_072);
    assert_eq!(CompressionParameterRequest::MAX_LEVEL, 22);
    assert_eq!(CompressionParameterRequest::DEFAULT_LEVEL, 3);
    assert_eq!(
        CompressionParameterRequest::with_level(i32::MIN).level(),
        CompressionParameterRequest::MIN_LEVEL
    );
    assert_eq!(
        CompressionParameterRequest::default().level(),
        CompressionParameterRequest::DEFAULT_LEVEL
    );
    assert_eq!(
        CompressionParameterRequest::with_level(0).level(),
        CompressionParameterRequest::DEFAULT_LEVEL
    );
    for source in [
        SourceSize::Unknown,
        SourceSize::Known(0),
        SourceSize::Known(16_383),
    ] {
        assert_eq!(
            inspect_compression_level(0, source, 0).unwrap(),
            inspect_compression_level(3, source, 0).unwrap()
        );
        assert_eq!(
            inspect_compression_level(i32::MIN, source, 0).unwrap(),
            inspect_compression_level(-131_072, source, 0).unwrap()
        );
        assert_eq!(
            inspect_compression_level(-131_072, source, 0)
                .unwrap()
                .target_length(),
            131_072
        );
    }
}

/// fuzzer.c:570-586, :804-809 and :1764-1807, concrete validation subset.
/// Invalid window99 and strategy-1 (u32::MAX) reject, and valid derived records
/// retain all seven fields. This does not implement the original atomic
/// ZSTD_CCtx_setCParams bulk setter or attach these records to a live context.
#[test]
fn concrete_validation_retains_all_fields_and_rejects_original_invalid_values() {
    for level in [1, 3, 12] {
        let resolved = inspect_compression_level(level, SourceSize::Unknown, 0).unwrap();
        let raw = resolved.to_raw();
        assert_eq!(raw.validate(), Ok(resolved));
        assert_eq!(
            RawCompressionParameters {
                window_log: 99,
                ..raw
            }
            .validate(),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            RawCompressionParameters {
                strategy: u32::MAX,
                ..raw
            }
            .validate(),
            Err(Error::InvalidParameter)
        );
    }
}

/// Exact fuzzer.c:702-710 adjustment example: all unspecified raw fields are
/// zero and therefore invalid before adjustment. These sizes are known values.
#[test]
fn original_adjustment_example_reduces_hash_and_chain_for_source_and_dictionary() {
    let raw = RawCompressionParameters {
        window_log: 10,
        hash_log: 19,
        chain_log: 19,
        ..RawCompressionParameters::default()
    };
    assert_eq!(raw.validate(), Err(Error::InvalidParameter));
    let adjusted = raw.adjust(SourceSize::Known(1_000), 100_000).unwrap();
    assert_eq!(adjusted.hash_log(), 18);
    assert_eq!(adjusted.chain_log(), 17);
    assert_eq!(adjusted.to_raw().validate(), Ok(adjusted));
}

/// Exact fuzzer.c:4332-4343 fieldwise dictionary-query equalities at level1.
/// C's public source0 means unknown, so each corresponding call spells Unknown.
/// Rust value equality replaces C memcmp without depending on struct padding.
#[test]
fn dictionary_sizes_select_the_original_equal_reference_profiles() {
    let medium = inspect_compression_level(1, SourceSize::Known(16 * 1024 - 1), 0).unwrap();
    let large = inspect_compression_level(1, SourceSize::Known(128 * 1024 - 1), 0).unwrap();
    let small_dictionary = inspect_compression_level(1, SourceSize::Unknown, 400).unwrap();
    let medium_dictionary = inspect_compression_level(1, SourceSize::Unknown, 10_000).unwrap();
    let large_dictionary = inspect_compression_level(1, SourceSize::Unknown, 100_000).unwrap();
    assert_eq!(small_dictionary, medium_dictionary);
    assert_eq!(medium, medium_dictionary);
    assert_eq!(large, large_dictionary);
}

/// Exact fuzzer.c:4346-4353: a small dictionary leaves the unknown-source
/// adjustment unchanged, whereas known source500 and dictionary400 use window10.
#[test]
fn dictionary_adjustment_preserves_unknown_profile_and_downsizes_known_input() {
    let original = inspect_compression_level(1, SourceSize::Unknown, 0).unwrap();
    let unknown = original.to_raw().adjust(SourceSize::Unknown, 400).unwrap();
    let known = original
        .to_raw()
        .adjust(SourceSize::Known(500), 400)
        .unwrap();
    assert_eq!(unknown, original);
    assert_eq!(known.window_log(), 10);
}

/// fuzzer.c:1721-1760 requested-getter subset before context processing.
/// Replacing the detached request with Default restores the reference defaults;
/// this does not simulate the original live-session or parameter-reset calls.
#[test]
fn requested_hash_override_survives_level_changes_and_default_replacement() {
    let mut request = CompressionParameterRequest::default();
    assert_eq!(request.level(), 3);
    assert_eq!(request.hash_log(), None);
    request.set_hash_log(6).unwrap();
    assert_eq!(request.level(), 3);
    assert_eq!(request.hash_log(), Some(6));
    request.set_level(7);
    assert_eq!(request.level(), 7);
    assert_eq!(request.hash_log(), Some(6));
    let requested = request;
    request.resolve(SourceSize::Known(1_024), 0).unwrap();
    assert_eq!(request, requested);
    request = CompressionParameterRequest::default();
    assert_eq!(request.level(), 3);
    assert_eq!(request.hash_log(), None);
}

/// fuzzer.c:1721 getter independence, expanded by the pinned requests.tsv
/// hash_then_level/level_then_hash observations. This is scalar ordering only;
/// it does not port fuzzer.c:3496/:3513 LDM-enabled compressed-byte equivalence.
#[test]
fn explicit_override_order_and_zero_auto_preserve_requested_and_resolved_values() {
    let mut hash_first = CompressionParameterRequest::default();
    hash_first.set_hash_log(14).unwrap();
    hash_first.set_level(9);
    let mut level_first = CompressionParameterRequest::with_level(9);
    level_first.set_hash_log(14).unwrap();
    assert_eq!(hash_first, level_first);
    for source in [
        SourceSize::Unknown,
        SourceSize::Known(0),
        SourceSize::Known(16_385),
    ] {
        assert_eq!(
            hash_first.resolve(source, 0).unwrap(),
            level_first.resolve(source, 0).unwrap()
        );
    }
    hash_first.set_level(1);
    assert_eq!(hash_first.hash_log(), Some(14));
    hash_first.set_hash_log(0).unwrap();
    assert_eq!(hash_first, CompressionParameterRequest::with_level(1));
    for source in [
        SourceSize::Unknown,
        SourceSize::Known(0),
        SourceSize::Known(16_385),
    ] {
        assert_eq!(
            hash_first.resolve(source, 0).unwrap(),
            inspect_compression_level(1, source, 0).unwrap()
        );
    }
}

/// zstreamtest.c:1621-1628 source-hint maximum/getter assertion through a
/// detached request. C's negative-1 input cannot reach a u32 Rust setter;
/// the upper overflow witness verifies checked runtime rejection and atomicity.
#[test]
fn source_hint_bounds_preserve_requested_values_after_rejection() {
    let mut request = CompressionParameterRequest::default();
    request.set_source_hint(i32::MAX as u32).unwrap();
    assert_eq!(request.source_hint(), Some(i32::MAX as u32));
    let before = request;
    for invalid in [i32::MAX as u32 + 1, u32::MAX] {
        assert_eq!(
            request.set_source_hint(invalid),
            Err(Error::InvalidParameter)
        );
        assert_eq!(request, before);
    }
    request.set_source_hint(0).unwrap();
    assert_eq!(request.source_hint(), None);
}

/// Source-hint precedence in zstd_compress.c:1638-1649, independently captured
/// by requests.tsv source_hint_small. A nonzero small hint must have an effect;
/// accepting only INT_MAX would not expose a resolver that ignores hints.
#[test]
fn a_small_source_hint_changes_only_unknown_size_resolution() {
    let mut request = CompressionParameterRequest::default();
    let no_hint_unknown = request.resolve(SourceSize::Unknown, 0).unwrap();
    let no_hint_empty = request.resolve(SourceSize::Known(0), 0).unwrap();
    let no_hint_known = request.resolve(SourceSize::Known(16_385), 0).unwrap();
    request.set_source_hint(16_384).unwrap();
    let before = request;
    let hinted = request.resolve(SourceSize::Unknown, 0).unwrap();
    assert_ne!(hinted, no_hint_unknown);
    assert_eq!(
        hinted,
        inspect_compression_level(3, SourceSize::Known(16_384), 0).unwrap()
    );
    assert_eq!(request.resolve(SourceSize::Known(0), 0), Ok(no_hint_empty));
    assert_eq!(
        request.resolve(SourceSize::Known(16_385), 0),
        Ok(no_hint_known)
    );
    assert_eq!(request, before);
}

/// zstd_compress.c:1643-1649: preset adjustment happens before overrides and
/// again after them. requests.tsv strategy step6 independently captures this
/// witness: performing just one adjustment would incorrectly yield chain7.
#[test]
fn request_resolution_preserves_the_two_pass_empty_source_chain_witness() {
    let mut request = CompressionParameterRequest::default();
    request.set_strategy(Some(ReferenceStrategy::Ultra2));
    let before = request;
    let resolved = request.resolve(SourceSize::Known(0), 0).unwrap();
    assert_eq!(
        resolved.to_raw(),
        RawCompressionParameters {
            window_log: 10,
            chain_log: 6,
            hash_log: 7,
            search_log: 2,
            min_match: 4,
            target_length: 0,
            strategy: 9,
        }
    );
    assert_eq!(request, before);
}

/// fuzzer.c:1808-1830 and :1832-1890 frame-default subset, plus pinned
/// getParams defaults (zstd.h:1946-1949). Frame flag mutation and bulk context
/// installation are separate lifecycle adapters, not part of this scalar test.
#[test]
fn reference_frame_defaults_remain_separate_from_live_encoder_defaults() {
    let reference = ReferenceFrameParameters::default();
    assert!(reference.include_content_size());
    assert!(!reference.checksum());
    assert!(reference.include_dictionary_id());
    let encoder = EncoderOptions::default();
    assert!(encoder.include_content_size);
    assert!(encoder.checksum);
    assert!(encoder.include_dictionary_id);
}
