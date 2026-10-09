//! Complete facebook/zstd v1.5.7 `tests/zstreamtest.c:1551-1599` source sweep.
//!
//! Source and streaming loops derive from `SEQ_generateRoundTrip`/`SEQ_roundTrip`
//! at lines 171-226; BSD-3-Clause option, root LICENSE.zstd. The public Rust
//! Encoder/Decoder receive the original 1024-byte buffers, one prepared shared
//! dictionary, one frame and all 971 requests. Two scalar/indexed generators
//! independently supply input and compare decoded output, so no 34 MiB source
//! or output buffer is retained. The expected cursor owns only 971 request
//! records and one 1024-byte chunk. An optional observer writes encoded chunks
//! directly to an external file for the C oracle.
//!
//! Rust's independent single-thread Greedy/Chain/Lazy policies do not implement
//! numeric C level 3, borrowed CDict ownership, or inherited workers=2/job=4MiB.
//! The C baseline reconstructs those original controls and the explicit level-5
//! parameters inherited from the prior advanced initializer. Although the CDict
//! requests window log 24, that stream advertises log 21; Rust matches log 21
//! and keeps the original decoder limit and offset schedule through log 24.
//! Rust's search history
//! is limited to 128 KiB; these exact large-offset inputs do not establish that
//! Rust emits every offset code targeted by the upstream source campaign.

use crate::upstream_sequence_generator_tests::{
    INITIAL_SEED, ORIGINAL_DIGEST, ORIGINAL_FINAL_SEED, ORIGINAL_SOURCE_BYTES, SequenceGenerator,
    SequenceRequest, original_schedule,
};
use crate::xxhash::XxHash64;
use crate::{
    CompressionStrategy, DecodeOptions, Decoder, Dictionary, Encoder, EncoderOptions, EndDirective,
    FrameHeader, SharedDictionary, StreamStatus,
};
use alloc::vec::Vec;

const CHUNK: usize = 1024;
const GUARD: usize = 8;
const CALL_LIMIT: usize = 1_000_000;
const WINDOW: u64 = 1 << 21;
const MAXIMUM_WINDOW: u64 = 1 << 24;
const DICTIONARY_ID: u32 = 2_004_100_296;
const DICTIONARY: &[u8] = include_bytes!("entropy-lifetimes/original-default.dict");

fn dictionary() -> SharedDictionary {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    assert!(dictionary.is_formatted());
    assert_eq!(dictionary.id(), DICTIONARY_ID);
    dictionary.into_shared()
}

struct Expected {
    generator: SequenceGenerator,
    schedule: Vec<SequenceRequest>,
    request: usize,
    scratch: [u8; CHUNK + GUARD],
    position: usize,
    length: usize,
    compared: u64,
}

impl Expected {
    fn new() -> Self {
        let schedule: Vec<_> = original_schedule().collect();
        assert_eq!(schedule.len(), 971);
        Self {
            generator: SequenceGenerator::new(INITIAL_SEED),
            schedule,
            request: 0,
            scratch: [0xa5; CHUNK + GUARD],
            position: 0,
            length: 0,
            compared: 0,
        }
    }

    fn refill(&mut self) -> bool {
        while self.position == self.length && self.request < self.schedule.len() {
            let request = self.schedule[self.request];
            self.scratch.fill(0xa5);
            let progress = self
                .generator
                .generate(request.kind, request.value, &mut self.scratch[..CHUNK])
                .unwrap();
            assert!(
                self.scratch[progress.length..]
                    .iter()
                    .all(|&byte| byte == 0xa5)
            );
            self.position = 0;
            self.length = progress.length;
            if progress.remaining == 0 {
                self.request += 1;
            }
            if progress.length != 0 {
                return true;
            }
        }
        self.position != self.length
    }

    fn compare(&mut self, output: &[u8]) {
        let mut position = 0;
        while position < output.len() {
            assert!(self.refill(), "decoded output exceeds original source");
            let length = (self.length - self.position).min(output.len() - position);
            assert_eq!(
                &self.scratch[self.position..self.position + length],
                &output[position..position + length],
                "source mismatch at output byte {} near request {}",
                self.compared,
                self.request
            );
            self.position += length;
            self.compared += length as u64;
            position += length;
        }
    }

    fn finish(&mut self) {
        assert!(
            !self.refill(),
            "decoded output ended before original source"
        );
        assert_eq!(self.request, 971);
        assert_eq!(self.compared, ORIGINAL_SOURCE_BYTES);
        assert_eq!(self.generator.digest(), ORIGINAL_DIGEST);
        assert_eq!(self.generator.seed(), ORIGINAL_FINAL_SEED);
    }
}

struct Decoded {
    decoder: Decoder,
    expected: Expected,
    hash: XxHash64,
    calls: usize,
    input: u64,
    output: u64,
    frames: usize,
}

impl Decoded {
    fn new(dictionary: SharedDictionary) -> Self {
        Self {
            decoder: Decoder::with_dictionary(
                DecodeOptions {
                    max_output_size: ORIGINAL_SOURCE_BYTES as usize,
                    max_window_size: MAXIMUM_WINDOW,
                },
                dictionary,
            ),
            expected: Expected::new(),
            hash: XxHash64::new(),
            calls: 0,
            input: 0,
            output: 0,
            frames: 0,
        }
    }

    fn accept(&mut self, input: &[u8], ending: bool) {
        assert!(input.len() <= CHUNK);
        let mut consumed = 0;
        // Like SEQ_roundTrip, pending output may wait for the next compressed
        // chunk. At finalization, drain even after all input has been accepted.
        while consumed < input.len() || (ending && self.frames == 0) {
            let mut output = [0xa5; CHUNK + GUARD];
            self.calls += 1;
            assert!(self.calls < CALL_LIMIT);
            let progress = self
                .decoder
                .process(&input[consumed..], &mut output[..CHUNK])
                .unwrap();
            assert!(progress.consumed <= input.len() - consumed && progress.produced <= CHUNK);
            assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
            self.expected.compare(&output[..progress.produced]);
            self.hash.update(&output[..progress.produced]);
            consumed += progress.consumed;
            self.input += progress.consumed as u64;
            self.output += progress.produced as u64;
            assert_eq!(self.decoder.total_input(), self.input);
            assert_eq!(self.decoder.total_output(), self.output);
            if progress.status == StreamStatus::FrameFinished {
                self.frames += 1;
                assert_eq!(self.frames, 1);
                assert_eq!(consumed, input.len());
                break;
            }
            assert!(matches!(
                progress.status,
                StreamStatus::NeedInput | StreamStatus::NeedOutput
            ));
            assert!(
                progress.consumed != 0 || progress.produced != 0,
                "decoder stalled"
            );
        }
        assert_eq!(consumed, input.len());
    }

    fn finish(&mut self, encoded_bytes: u64) {
        if self.frames == 0 {
            self.accept(&[], true);
        }
        assert_eq!(self.frames, 1);
        self.decoder.finish().unwrap();
        assert_eq!(self.input, encoded_bytes);
        assert_eq!(self.output, ORIGINAL_SOURCE_BYTES);
        assert_eq!(self.hash.digest(), ORIGINAL_DIGEST);
        self.expected.finish();
    }
}

#[derive(Debug)]
struct Statistics {
    encoded_bytes: u64,
    encoded_digest: u64,
    generation_calls: usize,
    compression_calls: usize,
    decompression_calls: usize,
}

struct Campaign<'a> {
    encoder: Encoder,
    decoded: Decoded,
    observer: &'a mut dyn FnMut(&[u8]),
    encoded_hash: XxHash64,
    encoded_bytes: u64,
    compression_calls: usize,
    header: [u8; 18],
    header_length: usize,
    finished: bool,
}

impl<'a> Campaign<'a> {
    fn new(strategy: CompressionStrategy, observer: &'a mut dyn FnMut(&[u8])) -> Self {
        let dictionary = dictionary();
        Self {
            encoder: Encoder::with_dictionary(
                EncoderOptions {
                    strategy,
                    window_log: 21,
                    checksum: false,
                    pledged_size: None,
                    include_content_size: true,
                    include_dictionary_id: true,
                },
                dictionary.clone(),
            )
            .unwrap(),
            decoded: Decoded::new(dictionary),
            observer,
            encoded_hash: XxHash64::new(),
            encoded_bytes: 0,
            compression_calls: 0,
            header: [0; 18],
            header_length: 0,
            finished: false,
        }
    }

    fn round_trip(&mut self, input: &[u8], directive: EndDirective) {
        assert!(input.len() <= CHUNK && !self.finished);
        let initial_input = self.encoder.total_input();
        let mut consumed = 0;
        loop {
            let mut output = [0xa5; CHUNK + GUARD];
            self.compression_calls += 1;
            assert!(self.compression_calls < CALL_LIMIT);
            let progress = self
                .encoder
                .process(&input[consumed..], &mut output[..CHUNK], directive)
                .unwrap();
            assert!(progress.consumed <= input.len() - consumed && progress.produced <= CHUNK);
            assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
            consumed += progress.consumed;
            let bytes = &output[..progress.produced];
            let header_bytes = (self.header.len() - self.header_length).min(bytes.len());
            self.header[self.header_length..self.header_length + header_bytes]
                .copy_from_slice(&bytes[..header_bytes]);
            self.header_length += header_bytes;
            self.encoded_bytes += bytes.len() as u64;
            self.encoded_hash.update(bytes);
            (self.observer)(bytes);
            assert_eq!(self.encoder.total_input(), initial_input + consumed as u64);
            assert_eq!(self.encoder.total_output(), self.encoded_bytes);
            self.finished = progress.status == StreamStatus::FrameFinished;
            self.decoded.accept(bytes, self.finished);
            if consumed == input.len() && (directive == EndDirective::Continue || self.finished) {
                break;
            }
            assert_eq!(progress.status, StreamStatus::NeedOutput);
            assert!(
                progress.consumed != 0 || progress.produced != 0,
                "encoder stalled"
            );
        }
        assert_eq!(consumed, input.len());
    }

    fn finish(&mut self) {
        self.round_trip(&[], EndDirective::Finish);
        assert!(self.finished);
        assert_eq!(self.encoder.total_input(), ORIGINAL_SOURCE_BYTES);
        self.decoded.finish(self.encoded_bytes);
        let (header, _) = FrameHeader::parse(&self.header[..self.header_length]).unwrap();
        assert_eq!(header.window_size, WINDOW);
        assert_eq!(header.content_size, None);
        assert_eq!(header.dictionary_id, DICTIONARY_ID);
        assert!(!header.has_checksum);
    }
}

fn sweep(strategy: CompressionStrategy, observer: &mut dyn FnMut(&[u8])) -> Statistics {
    let mut campaign = Campaign::new(strategy, observer);
    let mut source = SequenceGenerator::new(INITIAL_SEED);
    let mut requests = 0;
    let mut generated = 0;
    let mut generation_calls = 0;
    for request in original_schedule() {
        requests += 1;
        loop {
            let mut scratch = [0xa5; CHUNK + GUARD];
            let progress = source
                .generate(request.kind, request.value, &mut scratch[..CHUNK])
                .unwrap();
            assert!(scratch[progress.length..].iter().all(|&byte| byte == 0xa5));
            generated += progress.length as u64;
            generation_calls += 1;
            assert!(generation_calls < CALL_LIMIT);
            campaign.round_trip(&scratch[..progress.length], EndDirective::Continue);
            assert_eq!(campaign.encoder.total_input(), generated);
            if progress.remaining == 0 {
                break;
            }
        }
    }
    assert_eq!(requests, 971);
    assert_eq!(generated, ORIGINAL_SOURCE_BYTES);
    assert_eq!(source.digest(), ORIGINAL_DIGEST);
    assert_eq!(source.seed(), ORIGINAL_FINAL_SEED);
    campaign.finish();
    let statistics = Statistics {
        encoded_bytes: campaign.encoded_bytes,
        encoded_digest: campaign.encoded_hash.digest(),
        generation_calls,
        compression_calls: campaign.compression_calls,
        decompression_calls: campaign.decoded.calls,
    };
    assert!(statistics.encoded_bytes > 0 && statistics.encoded_digest != 0);
    assert_eq!(statistics.generation_calls, 34_593);
    assert!(statistics.compression_calls > statistics.generation_calls);
    assert!(statistics.decompression_calls > 0);
    #[cfg(feature = "std")]
    std::eprintln!(
        "zstreamtest:1551 {strategy:?}: 971 requests / {ORIGINAL_SOURCE_BYTES} exact bytes / XXH64={ORIGINAL_DIGEST:016x}; {statistics:?}"
    );
    statistics
}

#[test]
fn complete_original_sequence_sweep_greedy() {
    sweep(CompressionStrategy::Greedy, &mut |_| {});
}

#[test]
fn complete_original_sequence_sweep_chain() {
    sweep(CompressionStrategy::Chain, &mut |_| {});
}

#[test]
fn complete_original_sequence_sweep_lazy() {
    sweep(CompressionStrategy::Lazy, &mut |_| {});
}

#[cfg(feature = "std")]
mod oracle {
    use super::*;
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(std::format!(
                "zstdoxide-sequence-sweep-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn checked_upstream() -> PathBuf {
        let upstream = PathBuf::from(
            std::env::var_os("ZSTD_UPSTREAM").expect("set ZSTD_UPSTREAM to pinned v1.5.7"),
        );
        let revision = Command::new("git")
            .arg("-C")
            .arg(&upstream)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(revision.status.success());
        assert_eq!(
            std::string::String::from_utf8(revision.stdout)
                .unwrap()
                .trim(),
            "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&upstream)
                .args([
                    "diff",
                    "--quiet",
                    "HEAD",
                    "--",
                    "lib",
                    "tests/seqgen.c",
                    "tests/seqgen.h",
                    "tests/zstreamtest.c"
                ])
                .status()
                .unwrap()
                .success(),
            "pinned source must be clean"
        );
        upstream
    }

    fn compile(upstream: &Path, directory: &Directory) -> PathBuf {
        let archive = if let Some(path) = std::env::var_os("ZSTD_MT_LIBRARY") {
            PathBuf::from(path)
        } else {
            // Invoke the pinned Makefile's archive rule with the documented
            // multithread define. A separate BUILD_DIR and explicit archive
            // target avoid its public libzstd.a copy rule, leaving existing
            // reference libraries and every source file unchanged.
            let build = directory.0.join("mt-build");
            let archive = build.join("static/libzstd.a");
            let mut make = Command::new(std::env::var_os("MAKE").unwrap_or_else(|| "make".into()));
            make.arg("-C")
                .arg(upstream.join("lib"))
                .arg("-j2")
                .arg(std::format!("BUILD_DIR={}", build.display()))
                .arg("CPPFLAGS=-DZSTD_MULTITHREAD")
                .arg(&archive);
            std::eprintln!("sequence-sweep independent MT build: {make:?}");
            let result = make.output().unwrap();
            assert!(
                result.status.success(),
                "{}\n{}",
                std::string::String::from_utf8_lossy(&result.stdout),
                std::string::String::from_utf8_lossy(&result.stderr)
            );
            archive
        };
        let executable = directory.0.join("oracle");
        let mut compiler = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()));
        compiler
            .args(["-std=c99", "-O2", "-Wall", "-Wextra", "-Werror"])
            .arg("-I")
            .arg(upstream.join("tests"))
            .arg("-I")
            .arg(upstream.join("lib"))
            .arg("-I")
            .arg(upstream.join("lib/common"))
            .arg(std::concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/oracles/sequence_sweep.c"
            ))
            .arg(upstream.join("tests/seqgen.c"))
            .arg(upstream.join("lib/common/xxhash.c"))
            .arg(&archive)
            .arg("-lpthread")
            .arg("-o")
            .arg(&executable);
        std::eprintln!("sequence-sweep independent C compile: {compiler:?}");
        let result = compiler.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&result.stderr)
        );
        executable
    }

    fn read_original(path: &Path, dictionary: SharedDictionary) -> Statistics {
        let mut file = std::fs::File::open(path).unwrap();
        let mut decoded = Decoded::new(dictionary);
        let mut encoded_hash = XxHash64::new();
        let mut encoded_bytes = 0;
        let mut header = [0; 18];
        let mut header_length = 0;
        loop {
            let mut scratch = [0; CHUNK];
            let length = file.read(&mut scratch).unwrap();
            if length == 0 {
                break;
            }
            assert_eq!(decoded.frames, 0, "single original frame only");
            let bytes = &scratch[..length];
            let copy = (header.len() - header_length).min(bytes.len());
            header[header_length..header_length + copy].copy_from_slice(&bytes[..copy]);
            header_length += copy;
            encoded_bytes += bytes.len() as u64;
            encoded_hash.update(bytes);
            decoded.accept(bytes, false);
        }
        decoded.finish(encoded_bytes);
        let (header, _) = FrameHeader::parse(&header[..header_length]).unwrap();
        assert_eq!(header.window_size, WINDOW);
        assert_eq!(header.content_size, None);
        assert_eq!(header.dictionary_id, DICTIONARY_ID);
        assert!(!header.has_checksum);
        Statistics {
            encoded_bytes,
            encoded_digest: encoded_hash.digest(),
            generation_calls: 0,
            compression_calls: 0,
            decompression_calls: decoded.calls,
        }
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM v1.5.7, a C compiler and make; optional ZSTD_MT_LIBRARY reuses an external MT archive"]
    fn original_c_baseline_and_all_named_strategies_decode_both_directions() {
        let directory = Directory::new();
        let upstream = checked_upstream();
        let executable = compile(&upstream, &directory);
        let dictionary_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/upstream/entropy-lifetimes/original-default.dict");
        let original_path = directory.0.join("original.zst");
        let baseline = Command::new(&executable)
            .arg("baseline")
            .arg(&dictionary_path)
            .arg(&original_path)
            .output()
            .unwrap();
        assert!(
            baseline.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&baseline.stderr)
        );
        let metadata = std::string::String::from_utf8(baseline.stdout).unwrap();
        for expected in [
            "\"requests\":971",
            "\"source_bytes\":34639743",
            "\"source_xxh64\":\"d6a9522c4b14b2a5\"",
            "\"final_seed\":\"0f900e80\"",
            "\"generation_calls\":34593",
            "\"window\":2097152",
            "\"cdict_window_log\":24",
            "\"requested_window_log\":21",
            "\"checksum\":0",
            "\"dictionary_id\":2004100296",
            "\"workers\":2",
            "\"job_size\":4194304",
        ] {
            assert!(metadata.contains(expected), "{metadata}");
        }
        let original = read_original(&original_path, dictionary());
        assert!(metadata.contains(&std::format!("\"frame_bytes\":{}", original.encoded_bytes)));
        assert!(metadata.contains(&std::format!(
            "\"frame_xxh64\":\"{:016x}\"",
            original.encoded_digest
        )));
        std::eprintln!("original :1551 C baseline→Rust: {metadata}");
        let mut frames = 0;
        for strategy in [
            CompressionStrategy::Greedy,
            CompressionStrategy::Chain,
            CompressionStrategy::Lazy,
        ] {
            let frame_path = directory.0.join(std::format!("{strategy:?}.zst"));
            let mut frame = std::fs::File::create(&frame_path).unwrap();
            let statistics = sweep(strategy, &mut |bytes| frame.write_all(bytes).unwrap());
            frame.flush().unwrap();
            drop(frame);
            let decoded = Command::new(&executable)
                .arg("decode")
                .arg(&dictionary_path)
                .arg(&frame_path)
                .output()
                .unwrap();
            assert!(
                decoded.status.success(),
                "{strategy:?}: {}",
                std::string::String::from_utf8_lossy(&decoded.stderr)
            );
            let metadata = std::string::String::from_utf8(decoded.stdout).unwrap();
            for expected in [
                std::format!("\"decoded_bytes\":{ORIGINAL_SOURCE_BYTES}"),
                std::format!("\"decoded_xxh64\":\"{ORIGINAL_DIGEST:016x}\""),
                std::format!("\"frame_bytes\":{}", statistics.encoded_bytes),
                std::format!("\"frame_xxh64\":\"{:016x}\"", statistics.encoded_digest),
                std::string::String::from("\"requests\":971"),
            ] {
                assert!(metadata.contains(&expected), "{metadata}");
            }
            frames += 1;
            std::eprintln!("original :1551 {strategy:?}→C: {metadata}");
        }
        assert_eq!(frames, 3);
        std::eprintln!(
            "zstreamtest:1551: one original C baseline→Rust and {frames} Rust→C frames / {} exact Rust→C output bytes; every frame preserves all 971 requests with 1024-byte source/compression/decompression buffers",
            ORIGINAL_SOURCE_BYTES * frames
        );
    }
}
