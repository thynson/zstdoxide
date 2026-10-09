// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Source workload: facebook/zstd v1.5.7 tests/fuzzer.c:148-307.
// Used under the BSD-3-Clause option; see LICENSE.zstd.
//! Custom context allocation with caller-owned source/output storage.
//!
//! The exact large source preserves the official allocator workload size and
//! RDG parameters. Rust's three named strategies do not reproduce C levels or
//! worker groups. Balance, byte equality and allocator identity are additional
//! Rust assertions; upstream's original loop reports allocation counters only.
#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "upstream/support/mod.rs"]
mod support;

use std::alloc::Allocator;
use zstdoxide::{
    CompressionStrategy, DecodeOptions, Decoder, Encoder, EncoderOptions, EndDirective,
    StreamStatus,
};
use zstdoxide_test_allocator::{AllocationSnapshot, AllocatorConfig, TrackingAllocator};

const SOURCE_SIZE: usize = 89_456_640;
const SEED: u32 = 0x1234_5678;
const STRATEGIES: [CompressionStrategy; 3] = [
    CompressionStrategy::Greedy,
    CompressionStrategy::Chain,
    CompressionStrategy::Lazy,
];

fn allocator() -> TrackingAllocator {
    TrackingAllocator::new(AllocatorConfig::default()).unwrap()
}

fn options(size: usize, strategy: CompressionStrategy) -> EncoderOptions {
    EncoderOptions {
        strategy,
        pledged_size: Some(size as u64),
        ..EncoderOptions::default()
    }
}

/// Caller storage is deliberately outside the context's tracked domain.
fn append<A: Allocator + Clone>(
    encoder: &mut Encoder<A>,
    source: &[u8],
    directive: EndDirective,
    output: &mut Vec<u8>,
) {
    let mut consumed = 0;
    let mut scratch = [0xa5; 65_539];
    loop {
        scratch.fill(0xa5);
        let progress = encoder
            .process(&source[consumed..], &mut scratch[1..65_538], directive)
            .unwrap();
        assert_eq!(scratch[0], 0xa5);
        assert!(scratch[1 + progress.produced..].iter().all(|&b| b == 0xa5));
        consumed += progress.consumed;
        output.extend_from_slice(&scratch[1..1 + progress.produced]);
        let complete = match directive {
            EndDirective::Continue => progress.status == StreamStatus::NeedInput,
            EndDirective::Finish => progress.status == StreamStatus::FrameFinished,
            EndDirective::Flush => unreachable!("the upstream allocation schedule does not flush"),
        };
        if complete {
            assert_eq!(consumed, source.len());
            return;
        }
        assert_eq!(progress.status, StreamStatus::NeedOutput);
        assert!(progress.consumed != 0 || progress.produced != 0);
    }
}

fn encode<A: Allocator + Clone>(
    encoder: &mut Encoder<A>,
    source: &[u8],
    fragmented: bool,
) -> Vec<u8> {
    let mut frame = Vec::new();
    if fragmented {
        for fragment in source.chunks(65_521) {
            append(encoder, fragment, EndDirective::Continue, &mut frame);
        }
        append(encoder, &[], EndDirective::Finish, &mut frame);
    } else {
        append(encoder, source, EndDirective::Finish, &mut frame);
    }
    assert_eq!(encoder.total_input(), source.len() as u64);
    assert_eq!(encoder.total_output(), frame.len() as u64);
    frame
}

fn decode_and_compare<A: Allocator + Clone>(
    decoder: &mut Decoder<A>,
    frame: &[u8],
    expected: &[u8],
) {
    let mut consumed = 0;
    let mut produced = 0;
    let mut scratch = [0xa5; 16_387];
    loop {
        scratch.fill(0xa5);
        let end = frame.len().min(consumed + 16_381);
        let progress = decoder
            .process(&frame[consumed..end], &mut scratch[1..16_386])
            .unwrap();
        assert_eq!(scratch[0], 0xa5);
        assert!(scratch[1 + progress.produced..].iter().all(|&b| b == 0xa5));
        let next = produced + progress.produced;
        assert_eq!(
            &scratch[1..1 + progress.produced],
            &expected[produced..next]
        );
        consumed += progress.consumed;
        produced = next;
        if progress.status == StreamStatus::FrameFinished {
            assert_eq!(consumed, frame.len());
            assert_eq!(produced, expected.len());
            assert_eq!(decoder.total_input(), frame.len() as u64);
            assert_eq!(decoder.total_output(), expected.len() as u64);
            decoder.finish().unwrap();
            return;
        }
        assert!(progress.consumed != 0 || progress.produced != 0);
    }
}

fn assert_accounting(snapshot: AllocationSnapshot, retained: usize) {
    assert!(snapshot.attempts > 0);
    assert_eq!(snapshot.failures, 0);
    assert!(!snapshot.violations.any());
    assert_eq!(snapshot.live_bytes, retained as u128);
    assert!(snapshot.peak_bytes >= snapshot.live_bytes);
}

fn custom_frame(
    source: &[u8],
    strategy: CompressionStrategy,
    fragmented: bool,
) -> (
    Vec<u8>,
    AllocationSnapshot,
    usize,
    AllocationSnapshot,
    usize,
) {
    let owner = allocator().into_non_clone();
    let monitor = owner.monitor();
    let mut encoder = Encoder::with_options_in(options(source.len(), strategy), &owner).unwrap();
    assert_eq!(monitor.snapshot().attempts, 0);
    let frame = encode(&mut encoder, source, fragmented);
    let retained = encoder.retained_capacity();
    let encoded = monitor.snapshot();
    assert_accounting(encoded, retained);
    assert_eq!(encoded.identities_created, 1);
    drop(encoder);
    monitor.assert_released();

    let decode_owner = allocator();
    let decode_monitor = decode_owner.monitor();
    let mut decoder = Decoder::new_in(DecodeOptions::new(source.len()), decode_owner);
    decode_and_compare(&mut decoder, &frame, source);
    let decode_retained = decoder.retained_capacity();
    let decoded = decode_monitor.snapshot();
    assert_accounting(decoded, decode_retained);
    drop(decoder);
    decode_monitor.assert_released();
    (frame, encoded, retained, decoded, decode_retained)
}

#[test]
fn custom_context_counters_exclude_caller_buffers_and_balance_for_each_strategy() {
    let source = support::noise(8193, 0.50, 0.0, SEED);
    for strategy in STRATEGIES {
        let mut global = Encoder::with_options(options(source.len(), strategy)).unwrap();
        let expected = encode(&mut global, &source, false);
        for fragmented in [false, true] {
            let (frame, _, _, _, _) = custom_frame(&source, strategy, fragmented);
            assert!(
                frame == expected,
                "Global/custom complete frame bytes differ"
            );
        }
    }
}

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    })
}

#[test]
#[ignore = "exact 89,456,640-byte upstream allocation workload; requires pinned ZSTD_UPSTREAM with lib/libzstd.a and a C compiler"]
fn original_custom_context_source_campaign_has_complete_byte_comparisons() {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            // Remove only this test's newly created temporary directory.
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let upstream =
        PathBuf::from(std::env::var_os("ZSTD_UPSTREAM").expect("ZSTD_UPSTREAM is required"));
    let directory = Directory(std::env::temp_dir().join(format!(
        "zstdoxide-allocation-campaign-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    )));
    fs::create_dir(&directory.0).unwrap();
    let helper = directory.0.join("oracle");
    let compiled = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .args(["-std=c99", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg("-I")
        .arg(upstream.join("programs"))
        .arg("-I")
        .arg(upstream.join("lib"))
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/upstream/allocator/oracle.c"))
        .arg(upstream.join("programs/datagen.c"))
        .arg(upstream.join("lib/libzstd.a"))
        .arg("-o")
        .arg(&helper)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let generated = Command::new(&helper)
        .args(["generate", &SEED.to_string()])
        .output()
        .unwrap();
    assert!(generated.status.success());
    let source = support::noise(SOURCE_SIZE, 0.50, 0.0, SEED);
    assert!(
        generated.stdout == source,
        "complete Rust/C RDG source differs"
    );
    drop(generated);
    println!(
        "source_size={SOURCE_SIZE} seed={SEED} compressibility=0.50 literal_distribution=0.0 source_fnv64={}",
        fingerprint(&source)
    );

    let started = Instant::now();
    let frame_path = directory.0.join("frame.zst");
    for strategy in STRATEGIES {
        let mut global = Encoder::with_options(options(source.len(), strategy)).unwrap();
        let expected = encode(&mut global, &source, false);
        drop(global);
        for fragmented in [false, true] {
            let (frame, encoded, retained, decoded, decode_retained) =
                custom_frame(&source, strategy, fragmented);
            assert!(
                frame == expected,
                "Global/custom complete frame bytes differ"
            );
            fs::write(&frame_path, &frame).unwrap();
            let oracle = Command::new(&helper)
                .arg("decode")
                .arg(&frame_path)
                .output()
                .unwrap();
            assert!(oracle.status.success());
            assert!(oracle.stdout == source, "complete C decoded source differs");
            println!(
                "strategy={strategy:?} fragmented={fragmented} frame_bytes={} frame_fnv64={} encoder_retained={retained} decoder_retained={decode_retained} encoder={encoded:?} decoder={decoded:?}",
                frame.len(),
                fingerprint(&frame)
            );
        }
    }
    println!(
        "Verified 6 custom encoder/decoder configurations, 3 Global reference frames, and 6 complete C decodes over the exact source in {:?}; numeric levels and worker groups remain unsupported",
        started.elapsed()
    );
}
