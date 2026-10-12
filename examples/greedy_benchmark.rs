//! Thirty-two frozen reference cases; use scripts/benchmark_greedy.py.
//! Default smoke uses no clock. Global compression and output drop alone are
//! timed; independent verification and allocation accounting remain outside.
#![forbid(unsafe_code)]

use std::hint::black_box;
use std::time::{Duration, Instant};
use zstdoxide::{CompressionParameterRequest, ReferenceStrategy, SourceSize};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

const SEED: u64 = 0x5eed_fa57_2026_1009;
const BLOCK: usize = 131_072;
const FEATURES: &[(&str, bool)] = &[
    ("unsafe-byte-cursor", cfg!(feature = "unsafe-byte-cursor")),
    ("unsafe-bitstream", cfg!(feature = "unsafe-bitstream")),
    ("unsafe-history", cfg!(feature = "unsafe-history")),
    ("unsafe-ring-history", cfg!(feature = "unsafe-ring-history")),
    ("unsafe-fse", cfg!(feature = "unsafe-fse")),
    ("unsafe-fse-encoder", cfg!(feature = "unsafe-fse-encoder")),
    ("unsafe-huffman", cfg!(feature = "unsafe-huffman")),
    (
        "unsafe-huffman-codebook",
        cfg!(feature = "unsafe-huffman-codebook"),
    ),
    ("unsafe-match-table", cfg!(feature = "unsafe-match-table")),
    ("unsafe-match-chain", cfg!(feature = "unsafe-match-chain")),
    ("unsafe-dictionary", cfg!(feature = "unsafe-dictionary")),
    ("unsafe-training", cfg!(feature = "unsafe-training")),
    (
        "unsafe-reference-chain",
        cfg!(feature = "unsafe-reference-chain"),
    ),
    ("unsafe-row-table", cfg!(feature = "unsafe-row-table")),
];

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |v, &b| {
        (v ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}
fn next(state: &mut u64) -> u8 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state as u8
}
fn noise(mut state: u64, count: usize) -> Vec<u8> {
    (0..count).map(|_| next(&mut state)).collect()
}
fn skewed(count: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(count);
    let mut state = SEED;
    let phrase = b"Reference Fast one-shot benchmark with skewed literal frequencies.\n";
    for i in 0..count {
        let random = next(&mut state);
        result.push(if i >= 8192 && i % 509 != 0 {
            result[i - 8192]
        } else if i % 11 < 8 {
            phrase[i % phrase.len()]
        } else {
            random
        });
    }
    result
}
fn corpus() -> [(&'static str, Vec<u8>); 4] {
    let first = noise(SEED ^ 1, BLOCK);
    let second = noise(SEED ^ 2, BLOCK);
    let text = skewed(BLOCK);
    let gap = noise(SEED ^ 3, 11 * BLOCK);
    let far = [&first[..], &gap[..], &first[..], &text.repeat(4)].concat();
    [
        ("far-history", far),
        ("skewed", skewed(32768)),
        ("history", [&first[..], &second[..], &first[..]].concat()),
        (
            "mixed",
            [&text[..], &second[..], &vec![b'z'; BLOCK], &text[..]].concat(),
        ),
    ]
}
fn request(profile: &str) -> CompressionParameterRequest {
    match profile {
        "level-1" => CompressionParameterRequest::with_level(1),
        "level-3" => CompressionParameterRequest::with_level(3),
        "chain-4" | "chain-8" | "row-16" | "row-32" | "row-64" | "row-capped" => {
            let (window, hash, chain, search, target) = match profile {
                "chain-4" => (14, 15, 14, 4, 0),
                "chain-8" => (14, 15, 14, 8, 0),
                "row-16" => (21, 12, 6, 1, 0),
                "row-32" => (21, 12, 6, 5, 0),
                "row-64" => (21, 19, 6, 6, 0),
                "row-capped" => (21, 19, 30, 30, 131_072),
                _ => unreachable!(),
            };
            let mut value = CompressionParameterRequest::with_level(3);
            value.set_strategy(Some(ReferenceStrategy::Greedy));
            value.set_window_log(window).unwrap();
            value.set_hash_log(hash).unwrap();
            value.set_chain_log(chain).unwrap();
            value.set_search_log(search).unwrap();
            value.set_min_match(4).unwrap();
            value.set_target_length(target).unwrap();
            value
        }
        _ => unreachable!(),
    }
}
fn verify(frame: &[u8], source: &[u8]) {
    assert_eq!(zstdoxide::decompress(frame, source.len()).unwrap(), source);
}
fn survey(source: &[u8], request: &CompressionParameterRequest, expected: &[u8]) -> String {
    let owner = TrackingAllocator::new(AllocatorConfig {
        max_live_allocations: 512,
        fail_at: None,
    })
    .unwrap();
    let monitor = owner.monitor();
    let frame = zstdoxide::compress_with_parameters_in(source, request, owner).unwrap();
    assert_eq!(frame, expected);
    let resident = monitor.snapshot();
    assert!(!resident.violations.any());
    assert_eq!(resident.failures, 0);
    assert_eq!(resident.zero_size_requests, 0);
    assert_eq!(resident.allocate_zeroed_calls, 0);
    assert_eq!(resident.grow_zeroed_calls, 0);
    assert_eq!(resident.shrink_calls, 0);
    assert_eq!(resident.live_blocks, 1);
    drop(frame);
    monitor.assert_released();
    let released = monitor.snapshot();
    assert_eq!(released.identities_created, released.owners_dropped);
    format!(
        concat!(
            "{{\"attempts\":{},\"allocate_calls\":{},\"grow_calls\":{},",
            "\"peak_live_blocks\":{},\"peak_requested_bytes\":{},\"peak_bytes\":{},",
            "\"returned_requested_bytes\":{},\"returned_bytes\":{},\"identities_created\":{},",
            "\"owners_dropped\":{},\"final_live_blocks\":{},\"final_live_bytes\":{},\"violations\":false}}"
        ),
        resident.attempts,
        resident.allocate_calls,
        resident.grow_calls,
        resident.peak_live_blocks,
        resident.peak_requested_bytes,
        resident.peak_bytes,
        resident.live_requested_bytes,
        resident.live_bytes,
        released.identities_created,
        released.owners_dropped,
        released.live_blocks,
        released.live_bytes
    )
}
fn main() {
    let timed = match std::env::var("ZSTDOXIDE_GREEDY_BENCH_TIMED").as_deref() {
        Err(_) | Ok("0") => false,
        Ok("1") => true,
        Ok(_) => panic!("expected timed 0 or 1"),
    };
    let features = FEATURES
        .iter()
        .filter_map(|(name, enabled)| enabled.then_some(format!("\"{name}\"")))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"record\":\"metadata\",\"schema_version\":1,\"features\":[{features}],\"seed\":\"{SEED:016x}\",\"timed\":{timed},\"samples\":5,\"minimum_ms\":100,\"expected_rows\":32}}"
    );
    for (workload, source) in corpus() {
        for profile in [
            "level-1",
            "level-3",
            "chain-4",
            "chain-8",
            "row-16",
            "row-32",
            "row-64",
            "row-capped",
        ] {
            let request = request(profile);
            let params = request
                .resolve(SourceSize::Known(source.len() as u64), 0)
                .unwrap()
                .to_raw();
            let expected = zstdoxide::compress_with_parameters(&source, &request).unwrap();
            verify(&expected, &source);
            assert_eq!(
                zstdoxide::compress_with_parameters(&source, &request).unwrap(),
                expected
            );
            let allocation = survey(&source, &request, &expected);
            let mut samples = Vec::new();
            if timed {
                for _ in 0..5 {
                    let start = Instant::now();
                    let mut iterations = 0_u64;
                    let elapsed = loop {
                        let frame = zstdoxide::compress_with_parameters(
                            black_box(&source),
                            black_box(&request),
                        )
                        .unwrap();
                        black_box(&frame);
                        drop(frame);
                        iterations += 1;
                        let elapsed = start.elapsed();
                        if elapsed >= Duration::from_millis(100) {
                            break elapsed.as_nanos();
                        }
                    };
                    // Compression is deterministic. Verify a complete additional
                    // call after each sample; scanning is never in its clock.
                    assert_eq!(
                        zstdoxide::compress_with_parameters(&source, &request).unwrap(),
                        expected
                    );
                    samples.push(format!(
                        "{{\"iterations\":{iterations},\"elapsed_ns\":{elapsed}}}"
                    ));
                }
            }
            verify(&expected, &source);
            if let Some(directory) = std::env::var_os("ZSTDOXIDE_GREEDY_BENCH_FRAMES") {
                // The driver creates a fresh per-configuration directory.
                // Complete frames are compared between backends outside clocks.
                let path =
                    std::path::PathBuf::from(directory).join(format!("{workload}-{profile}.zst"));
                std::fs::write(path, &expected).unwrap();
            }
            println!(
                concat!(
                    "{{\"record\":\"case\",\"workload\":\"{}\",\"profile\":\"{}\",",
                    "\"source_bytes\":{},\"source_fnv1a64\":\"{:016x}\",",
                    "\"resolved\":[{},{},{},{},{},{},{}],\"frame_bytes\":{},\"frame_fnv1a64\":\"{:016x}\",",
                    "\"allocation\":{},\"samples\":[{}]}}"
                ),
                workload,
                profile,
                source.len(),
                fingerprint(&source),
                params.window_log,
                params.chain_log,
                params.hash_log,
                params.search_log,
                params.min_match,
                params.target_length,
                params.strategy,
                expected.len(),
                fingerprint(&expected),
                allocation,
                samples.join(",")
            );
        }
    }
}
