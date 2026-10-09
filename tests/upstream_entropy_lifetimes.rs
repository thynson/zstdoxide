// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Derived from facebook/zstd v1.5.7 tests/zstreamtest.c:1831-1919.
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
const SIX_BYTE_FLUSHES: usize = (256_usize * 1024).div_ceil(6);
const SMALL_BLOCK_SOURCE_SIZE: usize = SIX_BYTE_FLUSHES * 6 + 9 * 128;
// (8 + 200 KiB) + ZSTD_compressBound(10 MiB), exactly basicUnitTests' capacity.
const ORIGINAL_OUTPUT_CAPACITY: usize = 10_731_528;
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

fn small_block_source() -> Vec<u8> {
    // zstreamtest:1884 consumes no RNG. Both prior window-sweep modes therefore
    // share these exact bytes and the dictionary trained before the first draw.
    assert_eq!(SIX_BYTE_FLUSHES, 43_691);
    let mut input = vec![0xaa; SIX_BYTE_FLUSHES * 6];
    for offset in (0..9).rev().map(|index| index * 128) {
        input.extend_from_slice(&DICTIONARY[offset..offset + 128]);
    }
    assert_eq!(input.len(), SMALL_BLOCK_SOURCE_SIZE);
    assert_eq!(input.len(), 263_298);
    assert_eq!(fnv1a(&input), 0xa384_0785_d84a_0530);
    input
}

fn encode_small_blocks(
    dictionary: &SharedDictionary,
    strategy: CompressionStrategy,
) -> (Vec<u8>, Vec<usize>) {
    let mut encoder = Encoder::with_dictionary(
        EncoderOptions {
            strategy,
            // Measured original unknown-size streaming frame. CDict's initial
            // cParams log17 is not the frame window selected by compressStream2.
            window_log: 21,
            checksum: true,
            pledged_size: None,
            ..EncoderOptions::default()
        },
        dictionary.clone(),
    )
    .unwrap();
    let mut frame = vec![0xa5; ORIGINAL_OUTPUT_CAPACITY + 8];
    let mut position = 0;
    let mut boundaries = Vec::with_capacity(SIX_BYTE_FLUSHES + 9);
    for index in 0..SIX_BYTE_FLUSHES {
        let progress = encoder
            .process(
                &[0xaa; 6],
                &mut frame[position..ORIGINAL_OUTPUT_CAPACITY],
                EndDirective::Flush,
            )
            .unwrap();
        assert_eq!(progress.consumed, 6, "{strategy:?} flush {index}");
        assert_eq!(progress.status, StreamStatus::Flushed);
        position += progress.produced;
        assert!(
            frame[position..position + 8]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        boundaries.push(position);
        assert_eq!(encoder.total_input(), ((index + 1) * 6) as u64);
        assert_eq!(encoder.total_output(), position as u64);
    }
    for (index, offset) in (0..9).rev().map(|index| index * 128).enumerate() {
        let final_call = offset == 0;
        let directive = if final_call {
            EndDirective::Finish
        } else {
            EndDirective::Continue
        };
        let progress = encoder
            .process(
                &DICTIONARY[offset..offset + 128],
                &mut frame[position..ORIGINAL_OUTPUT_CAPACITY],
                directive,
            )
            .unwrap();
        assert_eq!(progress.consumed, 128, "{strategy:?} range {offset}");
        assert_eq!(
            progress.status,
            if final_call {
                StreamStatus::FrameFinished
            } else {
                StreamStatus::NeedInput
            }
        );
        if !final_call {
            assert_eq!(progress.produced, 0, "Continue buffers each original range");
        }
        position += progress.produced;
        assert!(
            frame[position..position + 8]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        boundaries.push(position);
        assert_eq!(
            encoder.total_input(),
            (SIX_BYTE_FLUSHES * 6 + (index + 1) * 128) as u64
        );
        assert_eq!(encoder.total_output(), position as u64);
    }
    assert_eq!(boundaries.len(), 43_700);
    assert_eq!(encoder.total_input(), SMALL_BLOCK_SOURCE_SIZE as u64);
    assert!(frame[position..].iter().all(|&byte| byte == 0xa5));
    frame.truncate(position);
    (frame, boundaries)
}

/// Independently locate the final literal section's end and sequence modes.
/// This reads format bitfields, never private production entropy state.
fn sequence_modes(payload: &[u8]) -> (usize, [u8; 3]) {
    fn little_endian(bytes: &[u8]) -> u64 {
        assert!(bytes.len() <= 5);
        bytes.iter().enumerate().fold(0, |value, (index, &byte)| {
            value | (u64::from(byte) << (8 * index))
        })
    }
    let kind = payload[0] & 3;
    let format = (payload[0] >> 2) & 3;
    let size = if kind < 2 {
        let (width, shift) = match format {
            0 | 2 => (1, 3),
            1 => (2, 4),
            _ => (3, 4),
        };
        let regenerated = usize::try_from(little_endian(&payload[..width]) >> shift).unwrap();
        assert!(regenerated <= 1152);
        width + if kind == 1 { 1 } else { regenerated }
    } else {
        let (width, bits) = match format {
            0 | 1 => (3, 10),
            2 => (4, 14),
            _ => (5, 18),
        };
        let value = little_endian(&payload[..width]);
        assert!((value >> 4) & ((1 << bits) - 1) <= 1152);
        width + usize::try_from(value >> (4 + bits)).unwrap()
    };
    let section = &payload[size..];
    let (count, width) = match section[0] {
        byte @ 0..=127 => (usize::from(byte), 1),
        byte @ 128..=254 => (
            ((usize::from(byte) - 128) << 8) + usize::from(section[1]),
            2,
        ),
        _ => (
            usize::from(u16::from_le_bytes([section[1], section[2]])) + 0x7f00,
            3,
        ),
    };
    assert!(count > 0 && count <= 1152 / 3);
    assert!(
        section.len() > width + 1,
        "sequence bitstream includes its marker"
    );
    let mode = section[width];
    assert_eq!(mode & 3, 0);
    (count, [mode >> 6, (mode >> 4) & 3, (mode >> 2) & 3])
}

fn assert_small_block_boundaries(
    frame: &[u8],
    boundaries: &[usize],
    original_c: bool,
) -> (usize, [u8; 3]) {
    let (header, mut position) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.dictionary_id, DICTIONARY_ID);
    assert_eq!(header.content_size, None);
    assert_eq!(header.window_size, 2 * 1024 * 1024);
    assert!(header.has_checksum);
    assert_eq!(boundaries.len(), SIX_BYTE_FLUSHES + 9);
    for (index, &boundary) in boundaries[..SIX_BYTE_FLUSHES].iter().enumerate() {
        let bytes: [u8; 3] = frame[position..position + 3].try_into().unwrap();
        let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
        let kind = (value >> 1) & 3;
        assert_eq!(value & 1, 0, "flush {index} cannot end frame");
        assert_eq!(value >> 3, 6, "exact six-byte block {index}");
        // C's small-block threshold emits raw bytes; Rust's uniform-byte fast
        // path emits RLE. Both preserve one complete block per original flush.
        assert_eq!(kind, if original_c { 0 } else { 1 });
        position += 3;
        let size = if original_c { 6 } else { 1 };
        assert!(
            frame[position..position + size]
                .iter()
                .all(|&byte| byte == 0xaa)
        );
        position += size;
        assert_eq!(position, boundary, "flush {index} output boundary");
    }
    for &boundary in &boundaries[SIX_BYTE_FLUSHES..SIX_BYTE_FLUSHES + 8] {
        assert_eq!(boundary, position, "Continue must not emit a new block");
    }
    let bytes: [u8; 3] = frame[position..position + 3].try_into().unwrap();
    let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
    assert_eq!(value & 1, 1);
    assert_eq!(
        (value >> 1) & 3,
        2,
        "dictionary ranges produce one compressed block"
    );
    let size = (value >> 3) as usize;
    assert!(size < 9 * 128);
    let modes = sequence_modes(&frame[position + 3..position + 3 + size]);
    if original_c {
        // The original source exercises inherited LL/ML tables and replaces
        // the OF channel with predefined tables after the raw block campaign.
        assert_eq!(modes, (12, [3, 0, 3]));
    }
    position += 3 + size;
    assert_eq!(
        position + 4,
        frame.len(),
        "only the checksum follows final block"
    );
    assert_eq!(*boundaries.last().unwrap(), frame.len());
    modes
}

fn assert_rust_small_block_modes(strategy: CompressionStrategy, modes: (usize, [u8; 3])) {
    let expected = match strategy {
        CompressionStrategy::Greedy => (14, [3, 0, 0]),
        CompressionStrategy::Chain => (13, [3, 0, 0]),
        CompressionStrategy::Lazy => (12, [3, 2, 0]),
    };
    // All policies must still reuse the dictionary's LL table after every RLE
    // block. OF/ML choices belong to each Rust policy, not the C level-3 path.
    assert_eq!(modes, expected, "{strategy:?} final entropy modes");
}

#[test]
fn original_dictionary_ranges_after_43691_six_byte_flushes() {
    let dictionary = prepared_dictionary();
    let source = small_block_source();
    let mut changed = DICTIONARY.to_vec();
    changed[147..].iter_mut().for_each(|byte| *byte ^= 0x5a);
    let changed = Dictionary::from_bytes(&changed).unwrap().into_shared();
    for strategy in STRATEGIES {
        let (frame, boundaries) = encode_small_blocks(&dictionary, strategy);
        let modes = assert_small_block_boundaries(&frame, &boundaries, false);
        assert_rust_small_block_modes(strategy, modes);
        assert_eq!(
            decompress_with_dictionary(&frame, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source,
            "{strategy:?}"
        );
        if let Ok(output) =
            decompress_with_dictionary(&frame, &changed, DecodeOptions::new(source.len()))
        {
            assert_ne!(
                output, source,
                "{strategy:?}: final dictionary ranges must reference content"
            );
        }
        eprintln!(
            "zstreamtest:1884 {strategy:?}: {} exact flushes, {} encoded bytes, final sequences/modes {modes:?}, {} exact output bytes",
            SIX_BYTE_FLUSHES,
            frame.len(),
            source.len()
        );
    }
}

#[cfg(feature = "std")]
mod oracle {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ORACLE: AtomicUsize = AtomicUsize::new(0);

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "zstdoxide-original-entropy-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_ORACLE.fetch_add(1, Ordering::Relaxed)
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

    fn decode(
        executable: &Path,
        directory: &Directory,
        frame: &[u8],
        source: &Path,
        source_size: usize,
    ) {
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
            source_size.to_string()
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
                decode(&executable, &directory, &frame, &source_path, SOURCE_SIZE);
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
            decode(&executable, &directory, &frame, &source_path, SOURCE_SIZE);
            decoded_frames += 1;
        }
        assert_eq!((baseline_frames, decoded_frames), (2, 8));
        eprintln!(
            "Original C :1831 baseline: {baseline_frames} frames; C decoded {decoded_frames} Rust frames / {} bytes; exact dictionary and both RDG schedules regenerated",
            decoded_frames * SOURCE_SIZE
        );
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7 with built lib/libzstd.a and a C compiler"]
    fn original_c_43691_flush_campaign_preserves_boundaries_and_decodes_both_directions() {
        let directory = Directory::new();
        let executable = compile(&checked_upstream(), &directory);
        let dictionary_path = directory.0.join("original-default.dict");
        std::fs::write(&dictionary_path, DICTIONARY).unwrap();
        let generated = Command::new(&executable)
            .arg("small-blocks")
            .arg(&dictionary_path)
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
            "\"window_log\":17",
            "\"frame_window\":2097152",
            "\"content_size_unknown\":true",
            "\"checksum\":1",
            "\"flush_calls\":43691",
            "\"complete_flush_calls\":43691",
            "\"dictionary_ranges\":9",
            "\"output_capacity\":10731528",
            "\"source_size\":263298",
            "\"source_fnv1a\":\"a3840785d84a0530\"",
            "\"frame_size\":393411",
            "\"boundary_fnv1a\":\"1ac5eb482bb6821b\"",
        ] {
            assert!(metadata.contains(expected), "{metadata}");
        }
        let source = small_block_source();
        let source_path = directory.0.join("source-six-byte.bin");
        assert_eq!(std::fs::read(&source_path).unwrap(), source);
        let original = std::fs::read(directory.0.join("original-six-byte.zst")).unwrap();
        let encoded_boundaries =
            std::fs::read(directory.0.join("original-six-byte-boundaries.le32")).unwrap();
        assert_eq!(encoded_boundaries.len(), 43_700 * 4);
        assert_eq!(fnv1a(&encoded_boundaries), 0x1ac5_eb48_2bb6_821b);
        let boundaries: Vec<_> = encoded_boundaries
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()) as usize)
            .collect();
        assert_small_block_boundaries(&original, &boundaries, true);
        let dictionary = prepared_dictionary();
        assert_eq!(
            decompress_with_dictionary(&original, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
        let mut decoded_frames = 0;
        for strategy in STRATEGIES {
            let (frame, boundaries) = encode_small_blocks(&dictionary, strategy);
            let modes = assert_small_block_boundaries(&frame, &boundaries, false);
            assert_rust_small_block_modes(strategy, modes);
            decode(&executable, &directory, &frame, &source_path, source.len());
            eprintln!(
                "zstreamtest:1884 C decoded {strategy:?}: {} encoded bytes, final sequences/modes {modes:?}, {} exact output bytes",
                frame.len(),
                source.len()
            );
            decoded_frames += 1;
        }
        assert_eq!(decoded_frames, 3);
        eprintln!(
            "Original C :1884 baseline: 1 frame, 43691 flushes, 9 dictionary ranges; C decoded {decoded_frames} Rust frames / {} bytes; all 43700 output boundaries checked",
            decoded_frames * source.len()
        );
    }
}
