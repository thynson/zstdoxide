//! Exact inline byte fixtures extracted from the official zstd v1.5.7 tests.
//!
//! Source literals, locations, and SHA-256 hashes are preserved alongside the
//! bytes in `tests/upstream/inline-regressions/manifest.json`. Upstream fixture
//! copyright: Meta Platforms, Inc. and affiliates; license: `LICENSE.zstd`.

use zstdoxide::{Error, decompress};

const MODERN_FRAME: &[u8] = include_bytes!("upstream/inline-regressions/legacy-v0.8.0.zst");
const FRAME_OUTPUT: &[u8] = include_bytes!("upstream/inline-regressions/legacy-frame.raw");

#[test]
fn decodes_current_format_component_of_official_legacy_fixture() {
    // tests/legacy.c COMPRESSED[738..917]: v0.8 uses modern frame magic.
    assert_eq!(
        decompress(MODERN_FRAME, FRAME_OUTPUT.len()).unwrap(),
        FRAME_OUTPUT
    );
}

#[test]
fn current_format_component_rejects_truncation_and_short_output_budget() {
    assert_eq!(
        decompress(MODERN_FRAME, FRAME_OUTPUT.len() - 1),
        Err(Error::ResourceLimit)
    );
    for end in 0..MODERN_FRAME.len() {
        assert!(
            decompress(&MODERN_FRAME[..end], FRAME_OUTPUT.len()).is_err(),
            "accepted incomplete v0.8 fixture ending at {end}"
        );
    }
}

#[test]
fn skips_exact_fuzzer_variant_two_frame() {
    // tests/fuzzer.c:4114, skippableFrame: variant 2 with payload "abcde".
    let encoded = include_bytes!("upstream/inline-regressions/skippable-variant-two.zst");
    assert!(decompress(encoded, 0).unwrap().is_empty());
    for end in 0..encoded.len() {
        assert!(decompress(&encoded[..end], 0).is_err());
    }
}

#[test]
fn rejects_exact_fuzzer_skippable_size_minus_eight() {
    // tests/fuzzer.c:732, skippable8: header advertises 0xfffffff8 payload
    // bytes but contains no payload. It must not overflow into an empty frame.
    let encoded = include_bytes!("upstream/inline-regressions/skippable-size-minus-eight.zst");
    assert!(decompress(encoded, 0).is_err());
}

#[test]
#[ignore = "requires legacy v0.4-v0.7 decoders; must decode all five official frames successfully"]
fn decodes_all_five_official_legacy_frames() {
    // Port of tests/legacy.c::testSimpleAPI. This desired-success assertion is
    // executable now and will fail until the missing legacy decoders exist.
    let encoded = include_bytes!("upstream/inline-regressions/legacy-all.zst");
    let expected = include_bytes!("upstream/inline-regressions/legacy-all.raw");
    let actual = decompress(encoded, expected.len()).unwrap();
    assert_eq!(actual, expected);
}
