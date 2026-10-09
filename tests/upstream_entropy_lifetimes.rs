// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Derived from facebook/zstd v1.5.7 tests/zstreamtest.c:1831-1881.
// BSD-3-Clause; attribution and license are preserved in root LICENSE.zstd.
//! Original dictionary entropy lifetime schedules with explicit API adaptations.
//!
//! The original level-3, by-reference CDict setup is executed by the optional
//! C oracle. Rust prepares owned shared dictionaries and exercises its named
//! Greedy/Chain/Lazy policies; these are not equivalents of numeric C levels.
//! Source bytes, dictionary bytes, prior RNG state, and range order are exact.
#![forbid(unsafe_code)]

#[path = "upstream/support/mod.rs"]
#[allow(
    dead_code,
    reason = "the shared test module also contains unrelated one-shot helpers"
)]
mod upstream_support;

use zstdoxide::{
    CompressionStrategy, DecodeOptions, Dictionary, DictionaryMode, Encoder, EncoderOptions,
    EndDirective, FrameHeader, SharedDictionary, StreamStatus, compress_with_dictionary,
    decompress_with_dictionary,
};

const DICTIONARY: &[u8] = include_bytes!("upstream/entropy-lifetimes/original-default.dict");
const BLOCK_SIZE: usize = 128 * 1024;
const SOURCE_SIZE: usize = 2 * BLOCK_SIZE;
const DICTIONARY_ID: u32 = 2_004_100_296;
const STRATEGIES: [CompressionStrategy; 3] = [
    CompressionStrategy::Greedy,
    CompressionStrategy::Chain,
    CompressionStrategy::Lazy,
];

#[derive(Clone, Copy, Debug)]
struct Campaign {
    name: &'static str,
    maximum_prior_window_log: u32,
    draws: usize,
    seed: u32,
    source_fnv1a: u64,
}

const CAMPAIGNS: [Campaign; 2] = [
    Campaign {
        name: "small",
        maximum_prior_window_log: 26,
        draws: 339_966,
        seed: 437_141_494,
        source_fnv1a: 0x0b06_fdac_2fcf_494a,
    },
    Campaign {
        name: "big",
        maximum_prior_window_log: 29,
        draws: 399_960,
        seed: 3_173_672_436,
        source_fnv1a: 0xe617_0386_87c0_164c,
    },
];

fn prepared_dictionary() -> SharedDictionary {
    let dictionary =
        Dictionary::from_bytes_with_mode(DICTIONARY, DictionaryMode::Formatted).unwrap();
    assert_eq!(DICTIONARY.len(), 48 * 1024);
    assert_eq!(dictionary.id(), DICTIONARY_ID);
    assert_eq!(DICTIONARY.len() - dictionary.content().len(), 147);
    dictionary.into_shared()
}

fn fnv1a(input: &[u8]) -> u64 {
    input.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn source(campaign: Campaign) -> Vec<u8> {
    // basicUnitTests starts with seed zero. Dictionary training happens before any
    // mutation; the offset==windowSize case at line 1652 later consumes two draws
    // for each sequence indices 1..=9999 for window logs 10..=26 or 10..=29. Earlier draws
    // use the distinct coreSeed. RDG receives a value, not a mutable seed.
    let draws = (campaign.maximum_prior_window_log - 10 + 1) as usize * 9999 * 2;
    assert_eq!(draws, campaign.draws);
    let mut rng = upstream_support::FuzzerRng::new(0);
    for _ in 0..draws {
        rng.next();
    }
    assert_eq!(rng.state(), campaign.seed);
    let mut source = upstream_support::noise(BLOCK_SIZE, 0.0, 0.0, rng.state());
    source.extend(upstream_support::noise(
        BLOCK_SIZE - 256,
        0.05,
        0.0,
        rng.state(),
    ));
    // These are serialized dictionary ranges, including any entropy/header
    // bytes at these offsets. Substituting content-relative ranges is wrong.
    source.extend_from_slice(&DICTIONARY[256..384]);
    source.extend_from_slice(&DICTIONARY[128..256]);
    assert_eq!(source.len(), SOURCE_SIZE);
    assert_eq!(fnv1a(&source), campaign.source_fnv1a);
    assert_eq!(
        &source[SOURCE_SIZE - 256..SOURCE_SIZE - 128],
        &DICTIONARY[256..384]
    );
    assert_eq!(&source[SOURCE_SIZE - 128..], &DICTIONARY[128..256]);
    source
}

fn encode(source: &[u8], dictionary: &SharedDictionary, strategy: CompressionStrategy) -> Vec<u8> {
    let mut encoder = Encoder::with_dictionary(
        EncoderOptions {
            strategy,
            // C's CDict cParams have windowLog 17, but compress_usingCDict emits a
            // single-segment frame with a 256 KiB window. Match that actual window
            // so final dictionary references remain eligible at P <= W.
            window_log: 18,
            checksum: false,
            pledged_size: Some(source.len() as u64),
            ..EncoderOptions::default()
        },
        dictionary.clone(),
    )
    .unwrap();
    let mut consumed = 0;
    let mut frame = Vec::new();
    for call in 0..10_000 {
        let capacity = if call % 7 == 6 { 0 } else { 4096 };
        let mut output = [0xa5; 4104];
        let progress = encoder
            .process(
                &source[consumed..],
                &mut output[..capacity],
                EndDirective::Finish,
            )
            .unwrap();
        assert!(progress.consumed <= source.len() - consumed && progress.produced <= capacity);
        assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
        consumed += progress.consumed;
        frame.extend_from_slice(&output[..progress.produced]);
        assert_eq!(encoder.total_input(), consumed as u64);
        assert_eq!(encoder.total_output(), frame.len() as u64);
        if progress.status == StreamStatus::FrameFinished {
            assert_eq!(consumed, source.len());
            return frame;
        }
        assert_eq!(progress.status, StreamStatus::NeedOutput);
        assert!(progress.consumed != 0 || progress.produced != 0 || capacity == 0);
    }
    panic!("original two-block campaign exceeded bounded calls");
}

fn assert_original_block_boundary(frame: &[u8], source: &[u8]) {
    let (header, mut position) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.dictionary_id, DICTIONARY_ID);
    assert_eq!(header.content_size, Some(SOURCE_SIZE as u64));
    assert_eq!(header.window_size, SOURCE_SIZE as u64);
    let mut kinds = Vec::new();
    loop {
        let bytes = frame.get(position..position + 3).unwrap();
        let value = u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16);
        position += 3;
        let kind = (value >> 1) & 3;
        let size = (value >> 3) as usize;
        assert!(kind < 3 && size <= BLOCK_SIZE);
        if kinds.is_empty() {
            assert_eq!(
                (kind, size),
                (0, BLOCK_SIZE),
                "first original RDG block must remain raw"
            );
            assert_eq!(&frame[position..position + size], &source[..BLOCK_SIZE]);
        }
        position += if kind == 1 { 1 } else { size };
        assert!(position <= frame.len());
        kinds.push(kind);
        if value & 1 != 0 {
            break;
        }
    }
    assert_eq!(kinds, [0, 2], "exactly two original 128 KiB blocks");
    assert_eq!(
        position + if header.has_checksum { 4 } else { 0 },
        frame.len()
    );
}

fn check_campaign(campaign: Campaign) {
    let source = source(campaign);
    let dictionary = prepared_dictionary();
    // Alter only content, preserving ID/entropy/repcodes. If the frame used
    // no dictionary content, decoding with this replacement would still match.
    let mut changed = DICTIONARY.to_vec();
    changed[147..].iter_mut().for_each(|byte| *byte ^= 0x5a);
    let changed = Dictionary::from_bytes(&changed).unwrap().into_shared();
    for strategy in STRATEGIES {
        let frame = encode(&source, &dictionary, strategy);
        assert_original_block_boundary(&frame, &source);
        assert_eq!(
            decompress_with_dictionary(&frame, &dictionary, DecodeOptions::new(SOURCE_SIZE))
                .unwrap(),
            source,
            "{:?} {strategy:?}",
            campaign
        );
        if let Ok(output) =
            decompress_with_dictionary(&frame, &changed, DecodeOptions::new(SOURCE_SIZE))
        {
            assert_ne!(
                output, source,
                "{} {strategy:?}: missing intended dictionary reference",
                campaign.name
            );
        }
    }
    let one_shot = compress_with_dictionary(&source, &dictionary).unwrap();
    assert_original_block_boundary(&one_shot, &source);
    assert_eq!(
        decompress_with_dictionary(&one_shot, &dictionary, DecodeOptions::new(SOURCE_SIZE))
            .unwrap(),
        source
    );
}

#[test]
fn original_uncompressible_dictionary_campaign_after_small_prior_window_sweep() {
    check_campaign(CAMPAIGNS[0]);
}

#[test]
fn original_uncompressible_dictionary_campaign_after_big_prior_window_sweep() {
    check_campaign(CAMPAIGNS[1]);
}

#[cfg(feature = "std")]
mod oracle {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "zstdoxide-original-entropy-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
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
            std::env::var_os("ZSTD_UPSTREAM")
                .expect("set ZSTD_UPSTREAM to pinned v1.5.7 with built lib/libzstd.a"),
        );
        let revision = Command::new("git")
            .arg("-C")
            .arg(&upstream)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(revision.status.success());
        assert_eq!(
            String::from_utf8(revision.stdout).unwrap().trim(),
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
                    "programs",
                    "tests/zstreamtest.c"
                ])
                .status()
                .unwrap()
                .success(),
            "pinned upstream sources must be clean"
        );
        upstream
    }

    fn compile(upstream: &Path, directory: &Directory) -> PathBuf {
        let executable = directory.0.join("oracle");
        let result = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
            .args(["-std=c99", "-O2", "-Wall", "-Wextra", "-Werror"])
            .arg("-I")
            .arg(upstream.join("programs"))
            .arg("-I")
            .arg(upstream.join("lib"))
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/upstream/entropy-lifetimes/oracle.c"
            ))
            .arg(upstream.join("programs/datagen.c"))
            .arg(upstream.join("lib/libzstd.a"))
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        executable
    }

    fn decode(executable: &Path, directory: &Directory, frame: &[u8], source: &Path) {
        let frame_path = directory.0.join("rust-frame.zst");
        std::fs::write(&frame_path, frame).unwrap();
        let result = Command::new(executable)
            .arg("decode")
            .arg(directory.0.join("original-default.dict"))
            .arg(&frame_path)
            .arg(source)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap().trim(),
            SOURCE_SIZE.to_string()
        );
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7 with built lib/libzstd.a and a C compiler"]
    fn original_c_dictionary_and_raw_block_campaign_match_rust_bytes_and_decode() {
        let directory = Directory::new();
        let executable = compile(&checked_upstream(), &directory);
        let generated = Command::new(&executable)
            .arg("generate")
            .arg(&directory.0)
            .output()
            .unwrap();
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let metadata = String::from_utf8(generated.stdout).unwrap();
        for expected in [
            "\"dictionary_size\":49152",
            "\"dictionary_id\":2004100296",
            "\"header_size\":147",
            "\"window_log\":17",
            "\"sample_count\":214",
            "\"last_sample_size\":5461",
            "\"training_fnv1a\":\"113bd69cda9b8c20\"",
        ] {
            assert!(metadata.contains(expected), "{metadata}");
        }
        assert_eq!(
            std::fs::read(directory.0.join("original-default.dict")).unwrap(),
            DICTIONARY
        );
        let dictionary = prepared_dictionary();
        let mut decoded_frames = 0;
        let mut baseline_frames = 0;
        for campaign in CAMPAIGNS {
            for field in [
                format!("\"seed\":{}", campaign.seed),
                format!("\"rng_draws\":{}", campaign.draws),
            ] {
                assert!(metadata.contains(&field), "{metadata}");
            }
            let source_path = directory.0.join(format!("source-{}.bin", campaign.name));
            let expected = source(campaign);
            // Full byte comparison, including both independent RDG calls and
            // the ordered ranges of serialized dictionary bytes.
            assert_eq!(std::fs::read(&source_path).unwrap(), expected);
            let original_frame =
                std::fs::read(directory.0.join(format!("original-{}.zst", campaign.name))).unwrap();
            assert_original_block_boundary(&original_frame, &expected);
            assert_eq!(
                decompress_with_dictionary(
                    &original_frame,
                    &dictionary,
                    DecodeOptions::new(SOURCE_SIZE)
                )
                .unwrap(),
                expected
            );
            baseline_frames += 1;
            for strategy in STRATEGIES {
                let frame = encode(&expected, &dictionary, strategy);
                assert_original_block_boundary(&frame, &expected);
                decode(&executable, &directory, &frame, &source_path);
                eprintln!(
                    "zstreamtest:1831 {} seed={} {strategy:?}: {} encoded bytes, {} exact output bytes",
                    campaign.name,
                    campaign.seed,
                    frame.len(),
                    expected.len()
                );
                decoded_frames += 1;
            }
            let frame = compress_with_dictionary(&expected, &dictionary).unwrap();
            decode(&executable, &directory, &frame, &source_path);
            decoded_frames += 1;
        }
        assert_eq!((baseline_frames, decoded_frames), (2, 8));
        eprintln!(
            "Original C :1831 baseline: {baseline_frames} frames; C decoded {decoded_frames} Rust frames / {} bytes; exact dictionary and both RDG schedules regenerated",
            decoded_frames * SOURCE_SIZE
        );
    }
}
