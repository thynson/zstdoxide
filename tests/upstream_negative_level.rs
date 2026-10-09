//! Pinned v1.5.7 `tests/fuzzer.c:3469–3495` negative-level assertions.
//!
//! The source is the exact first MiB inherited by the original group after its
//! four preceding writers, captured through the attributed, independently
//! checked source-state replay in `upstream/negative-level`. Cargo does not
//! regenerate that state or execute the preceding C unit-test harness.
//!
//! The Rust comparison adapts the original simple-versus-advanced assertion to
//! the supported level and parameter-request entry points. It does not exercise
//! a live CCtx setter, a caller-owned compression destination, or the original
//! destination capacity. Rust and C frame bytes need not be identical.
#![forbid(unsafe_code)]

use zstdoxide::{
    CompressionParameterRequest, FrameHeader, compress_at_level, compress_with_parameters,
    decompress,
};

const SOURCE: &[u8; 1_048_576] = include_bytes!("upstream/negative-level/source.raw");
const C_SIMPLE: &[u8; 566_817] = include_bytes!("upstream/negative-level/simple.zst");
const C_ADVANCED: &[u8; 566_817] = include_bytes!("upstream/negative-level/advanced.zst");

fn assert_source_identity(source: &[u8]) {
    assert_eq!(source.len(), 1_048_576);
    // The last inherited writer is fuzzer.c:3447, after the 31 KiB writer at
    // :2556. A fresh RDG buffer would not preserve this mutation.
    assert_eq!(source[0], 1);
    // Independent capture metadata supplies this full-input fingerprint; the
    // compiler-free collection auditor additionally verifies SHA256 pins.
    let fingerprint = source
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    assert_eq!(fingerprint, 0xf305_81ad_76b2_7b0d);
}

#[test]
fn both_original_c_negative_level_frames_decode_the_exact_inherited_source() {
    assert_source_identity(SOURCE);
    // The original assertion requires non-error and equal sizes. These exact
    // sizes and equal bytes are additional frozen C capture observations.
    assert_eq!(C_SIMPLE.len(), 566_817);
    assert_eq!(C_ADVANCED.len(), 566_817);
    assert_eq!(C_SIMPLE, C_ADVANCED);
    for frame in [C_SIMPLE.as_slice(), C_ADVANCED.as_slice()] {
        let (header, header_bytes) = FrameHeader::parse(frame).unwrap();
        assert_eq!(header_bytes, 10);
        assert_eq!(header.content_size, Some(1_048_576));
        assert_eq!(header.window_size, 524_288);
        assert_eq!(header.dictionary_id, 0);
        assert!(!header.has_checksum);
        assert_eq!(decompress(frame, SOURCE.len()).unwrap(), SOURCE.as_slice());
    }
}

#[test]
fn negative_level_simple_and_parameter_requests_preserve_the_original_size_assertion() {
    assert_source_identity(SOURCE);
    let input = SOURCE.to_vec();
    let request = CompressionParameterRequest::with_level(-1);
    let request_before = request;

    // fuzzer.c:3475–3486: both operations must succeed and their sizes must
    // match. The request object stands in for the fresh advanced C context's
    // single compression-level setter; its context lifecycle remains a gap.
    let simple = compress_at_level(&input, -1).unwrap();
    let advanced = compress_with_parameters(&input, &request).unwrap();
    assert_eq!(simple.len(), advanced.len());

    // Supplementary Rust contracts: deterministic bytes, borrowed-input and
    // request preservation, and complete byte-exact decompression for both.
    assert_eq!(simple, advanced);
    assert_eq!(request, request_before);
    assert_eq!(input, SOURCE.as_slice());
    for frame in [&simple, &advanced] {
        let (header, _) = FrameHeader::parse(frame).unwrap();
        assert_eq!(header.content_size, Some(input.len() as u64));
        assert_eq!(header.dictionary_id, 0);
        assert!(!header.has_checksum);
        assert_eq!(decompress(frame, input.len()).unwrap(), input);
    }
}
