//! Twenty-six independently captured complete Ultra2 cases; use scripts/benchmark_retained_ultra2_frames.py.
//! Default smoke uses no clock. Global compression and output drop alone are
//! timed; independent verification and allocation accounting remain outside.
#![forbid(unsafe_code)]

use std::hint::black_box;
use std::time::{Duration, Instant};
use zstdoxide::{CompressionParameterRequest, ReferenceStrategy, SourceSize};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

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
    (
        "unsafe-reference-tree",
        cfg!(feature = "unsafe-reference-tree"),
    ),
    (
        "unsafe-optimal-frequencies",
        cfg!(feature = "unsafe-optimal-frequencies"),
    ),
    (
        "unsafe-optimal-paths",
        cfg!(feature = "unsafe-optimal-paths"),
    ),
    (
        "unsafe-optimal-matches",
        cfg!(feature = "unsafe-optimal-matches"),
    ),
    (
        "unsafe-optimal-short-key",
        cfg!(feature = "unsafe-optimal-short-key"),
    ),
];

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |v, &b| {
        (v ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}
struct Case {
    name: String,
    source: Vec<u8>,
    frame: Vec<u8>,
    request: CompressionParameterRequest,
    resolved: [u32; 7],
}
struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take(&mut self, n: usize) -> &[u8] {
        assert!(n <= self.0.len());
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        head
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
    fn i64(&mut self) -> i64 {
        i64::from_le_bytes(self.take(8).try_into().unwrap())
    }
}
fn corpus() -> Vec<Case> {
    let path = std::env::var_os("ZSTDOXIDE_RETAINED_ULTRA_BENCH_INPUT")
        .expect("audited capture fixture path");
    let bytes = std::fs::read(path).unwrap();
    assert!(bytes.len() <= 2 * 1024 * 1024);
    let mut r = Reader(&bytes);
    assert_eq!(r.take(8), b"ZU2B0001");
    assert_eq!(r.u32(), 26);
    let mut cases = Vec::new();
    for _ in 0..26 {
        let n = r.u32() as usize;
        assert!((1..=128).contains(&n));
        let name = String::from_utf8(r.take(n).to_vec()).unwrap();
        let q: [i64; 9] = core::array::from_fn(|_| r.i64());
        let resolved = core::array::from_fn(|_| r.u32());
        let source_len = r.u32() as usize;
        let frame_len = r.u32() as usize;
        assert!((16385..=32768).contains(&source_len));
        assert!(frame_len <= source_len + 7 + 3 * source_len.div_ceil(8192));
        let source = r.take(source_len).to_vec();
        let frame = r.take(frame_len).to_vec();
        let mut request = CompressionParameterRequest::with_level(i32::try_from(q[0]).unwrap());
        for (value, set) in q[1..6].iter().zip([
            CompressionParameterRequest::set_window_log,
            CompressionParameterRequest::set_chain_log,
            CompressionParameterRequest::set_hash_log,
            CompressionParameterRequest::set_search_log,
            CompressionParameterRequest::set_min_match,
        ]) {
            if *value != 0 {
                set(&mut request, u8::try_from(*value).unwrap()).unwrap();
            }
        }
        if q[6] != 0 {
            request
                .set_target_length(u32::try_from(q[6]).unwrap())
                .unwrap();
        }
        assert!(q[7] == 0 || q[7] == 9);
        if q[7] == 9 {
            request.set_strategy(Some(ReferenceStrategy::Ultra2));
        }
        assert_eq!(q[8], 0);
        cases.push(Case {
            name,
            source,
            frame,
            request,
            resolved,
        });
    }
    assert!(r.0.is_empty());
    cases
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
    let timed = match std::env::var("ZSTDOXIDE_RETAINED_ULTRA_BENCH_TIMED").as_deref() {
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
        "{{\"record\":\"metadata\",\"schema_version\":1,\"features\":[{features}],\"corpus\":\"reference-retained-ultra2-frame-baseline\",\"timed\":{timed},\"samples\":5,\"minimum_ms\":100,\"expected_rows\":26}}"
    );
    for case in corpus() {
        let source = case.source;
        let request = case.request;
        let params = request
            .resolve(SourceSize::Known(source.len() as u64), 0)
            .unwrap()
            .to_raw();
        assert_eq!(
            [
                params.window_log,
                params.chain_log,
                params.hash_log,
                params.search_log,
                params.min_match,
                params.target_length,
                params.strategy
            ],
            case.resolved
        );
        let expected = zstdoxide::compress_with_parameters(&source, &request).unwrap();
        assert_eq!(expected, case.frame);
        verify(&expected, &source);
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
        if let Some(directory) = std::env::var_os("ZSTDOXIDE_RETAINED_ULTRA_BENCH_FRAMES") {
            // The driver creates a fresh per-configuration directory.
            // Complete frames are compared between backends outside clocks.
            let path = std::path::PathBuf::from(directory)
                .join(format!("{}-retained-ultra2.zst", case.name));
            std::fs::write(path, &expected).unwrap();
        }
        println!(
            concat!(
                "{{\"record\":\"case\",\"workload\":\"{}\",\"profile\":\"{}\",",
                "\"source_bytes\":{},\"source_fnv1a64\":\"{:016x}\",",
                "\"resolved\":[{},{},{},{},{},{},{}],\"frame_bytes\":{},\"frame_fnv1a64\":\"{:016x}\",",
                "\"allocation\":{},\"samples\":[{}]}}"
            ),
            case.name,
            "retained-ultra2",
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
