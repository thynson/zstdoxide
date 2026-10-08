//! Behavioral ports of the one-shot subset of facebook/zstd v1.5.7 tests.
//! See upstream/FUZZER.md for exact coverage and explicit capability gaps.
#![forbid(unsafe_code)]

#[path = "upstream/support/mod.rs"]
mod support;

use std::sync::OnceLock;
use support::{FuzzerRng, noise, skippable_frame, three_byte_sequences};
use zstdoxide::{DecodeOptions, Error, FrameHeader, compress, decompress, decompress_with_options};

const BASIC_SIZE: usize = 5 * 1024 * 1024;
const BASIC_SEED: u32 = 0;
const SEED: u32 = 0x1234_5678;

fn basic_source() -> &'static [u8] {
    static SOURCE: OnceLock<Vec<u8>> = OnceLock::new();
    SOURCE.get_or_init(|| noise(BASIC_SIZE, 0.5, 0.0, BASIC_SEED))
}

fn basic_frame() -> &'static [u8] {
    static FRAME: OnceLock<Vec<u8>> = OnceLock::new();
    FRAME.get_or_init(|| compress(basic_source()).unwrap())
}

fn roundtrip(source: &[u8]) -> Vec<u8> {
    let frame = compress(source).unwrap();
    assert_eq!(decompress(&frame, source.len()).unwrap(), source);
    frame
}

/// `basicUnitTests`: compress/decompress five MiB and compare every byte.
#[test]
fn basic_compression_and_decompression() {
    assert_eq!(
        decompress(basic_frame(), BASIC_SIZE).unwrap(),
        basic_source()
    );
}

/// `ZSTD_getFrameContentSize`/`ZSTD_getDecompressedSize` observable metadata.
#[test]
fn frame_content_size() {
    let (header, consumed) = FrameHeader::parse(basic_frame()).unwrap();
    assert_eq!(header.content_size, Some(BASIC_SIZE as u64));
    assert_eq!(header.dictionary_id, 0);
    assert!(header.has_checksum);
    assert!((6..=18).contains(&consumed));
}

/// Exact bytes from "decompress skippable frame -8 size".
#[test]
fn skippable_frame_minus_eight_size() {
    let frame = [0x50, 0x2a, 0x4d, 0x18, 0xf8, 0xff, 0xff, 0xff];
    assert_eq!(decompress(&frame, 0), Err(Error::UnexpectedEof));
}

#[test]
fn decompress_with_one_missing_byte() {
    let frame = basic_frame();
    assert_eq!(
        decompress(&frame[..frame.len() - 1], BASIC_SIZE),
        Err(Error::UnexpectedEof)
    );
}

#[test]
fn decompress_with_one_extra_byte() {
    let mut frame = basic_frame().to_vec();
    frame.push(0);
    assert_eq!(decompress(&frame, BASIC_SIZE), Err(Error::UnexpectedEof));
}

#[test]
fn decompress_with_large_trailing_input() {
    let mut frame = basic_frame().to_vec();
    frame.resize(frame.len() + 4096, 0);
    assert_eq!(decompress(&frame, BASIC_SIZE), Err(Error::InvalidMagic));
}

/// Rust's explicit zero output limit replaces C's NULL/zero-capacity buffer.
#[test]
fn decompress_into_zero_output_limit() {
    assert_eq!(decompress(basic_frame(), 0), Err(Error::ResourceLimit));
}

/// Port checksum rejection; forceIgnoreChecksum remains an unsupported API.
#[test]
fn decompress_with_corrupted_checksum() {
    for index in 1..=4 {
        let mut frame = basic_frame().to_vec();
        let checksum_byte = frame.len() - index;
        frame[checksum_byte] = frame[checksum_byte].wrapping_add(1);
        assert_eq!(decompress(&frame, BASIC_SIZE), Err(Error::ChecksumMismatch));
    }
}

/// `compress a NULL input` and the one-shot part of `decompress empty frame
/// into NULL`. No compression-level or streaming API is implied.
#[test]
fn compress_and_decompress_empty_frames() {
    let known_size = roundtrip(&[]);
    assert_eq!(
        FrameHeader::parse(&known_size).unwrap().0.content_size,
        Some(0)
    );
    let unknown_size = [0x28, 0xb5, 0x2f, 0xfd, 0, 0, 1, 0, 0];
    assert_eq!(
        FrameHeader::parse(&unknown_size).unwrap().0.content_size,
        None
    );
    assert_eq!(decompress(&unknown_size, 0).unwrap(), b"");
}

/// Exact four-way source division and 129-KiB skip used by basicUnitTests.
#[test]
fn decompress_multiple_frames_with_skippable_frame() {
    let segment_size = BASIC_SIZE / 2 / 4;
    let mut combined = Vec::new();
    for (index, source) in basic_source()[..BASIC_SIZE / 2]
        .chunks(segment_size)
        .enumerate()
    {
        combined.extend_from_slice(&compress(source).unwrap());
        if index == 2 {
            combined.extend_from_slice(&skippable_frame(
                (BASIC_SEED % 15) as u8,
                &vec![0; 129 * 1024],
            ));
        }
    }
    assert_eq!(
        decompress(&combined, BASIC_SIZE / 2).unwrap(),
        &basic_source()[..BASIC_SIZE / 2]
    );
    assert_eq!(
        decompress(&combined, BASIC_SIZE / 2 - 1),
        Err(Error::ResourceLimit)
    );
}

/// Decode the upstream nonempty/empty skip workloads for all sixteen magics.
/// There is no public payload extraction/write API in the Rust library yet.
#[test]
fn skippable_frames_with_all_magic_variants() {
    let payload: Vec<_> = (0..129 * 1024)
        .map(|index| BASIC_SEED.wrapping_add(index) as u8)
        .collect();
    for variant in 0..16 {
        for payload in [&payload[..], &[]] {
            let frame = skippable_frame(variant, payload);
            assert_eq!(decompress(&frame, 0).unwrap(), b"");
            assert!(decompress(&frame[..frame.len() - 1], 0).is_err());
        }
    }
}

#[test]
fn check_input_length_for_magic_number() {
    for length in 0..4 {
        assert_eq!(
            decompress(&[0x28, 0xb5, 0x2f][..length], BASIC_SIZE),
            Err(Error::UnexpectedEof)
        );
    }
}

#[test]
fn check_magic_number() {
    assert_eq!(
        decompress(&[1, 0xb5, 0x2f, 0xfd], BASIC_SIZE),
        Err(Error::InvalidMagic)
    );
}

#[test]
fn multiple_blocks_of_zeroes() {
    roundtrip(&vec![0; 1_000_000]);
}

/// Regression for upstream issue #137, retaining the original length 100.
#[test]
fn all_zeroes_issue_137() {
    roundtrip(&[0; 100]);
}

/// Retains NB3BYTESSEQ=512, seed=1, and _3BYTESTESTLENGTH=131000.
#[test]
fn many_three_byte_sequences() {
    roundtrip(&three_byte_sequences(131_000));
}

/// Preserve every original sequence count; reusable contexts and level 19
/// remain separate pending capabilities.
#[test]
fn growing_sequence_count() {
    let source = three_byte_sequences(131_000);
    let mut count = 1;
    while count <= source.len() / 3 {
        roundtrip(&source[..count * 3]);
        count += if count < 100 { 1 } else { count >> 2 };
    }
}

/// Preserve upstream's complete 1..5-MiB growth schedule and literal profile.
#[test]
fn growing_literals_buffer() {
    let source = noise(BASIC_SIZE, 0.0, 0.1, BASIC_SEED);
    let mut length = 1;
    while length <= source.len() {
        roundtrip(&source[..length]);
        length += if length < 100 { 1 } else { length >> 2 };
    }
}

/// Retain both the original two-run workload and its 46-byte encoded bound.
#[test]
fn rle_detection_workload() {
    let mut source = vec![b'B'; 256 * 1024 - 2];
    source.extend_from_slice(&vec![b'A'; 100 * 1024]);
    let frame = roundtrip(&source);
    assert!(
        frame.len() <= 46,
        "compressed size {} exceeds upstream bound 46",
        frame.len()
    );
}

/// `roundTripCrash.c::roundTripCheck`: compare length and every output byte.
/// Its XXH32-derived levels and CCtxParams/multithreading path remain pending.
#[test]
fn round_trip_crash_corpora() {
    for probability in [0.0, 0.05, 0.5, 0.95, 1.0] {
        for length in [0, 1, 127, 128, 255, 256, 65_535, 65_536, 131_072, 131_073] {
            roundtrip(&noise(length, probability, 0.0, SEED));
        }
    }
}

/// Reproduce longmatch.c's 256-KiB input layout and 63-byte duplicated marker.
/// Its continuous 8-GiB streaming/index-overflow regression is not covered.
#[test]
fn long_match_input_layout() {
    let marker = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\0";
    let mut source = noise(1 << 18, 0.0, 0.0, SEED);
    source[..marker.len()].copy_from_slice(marker);
    let end = source.len();
    source[end - marker.len()..].copy_from_slice(marker);
    roundtrip(&source);
}

fn bounded_noisy_decode(frame: &[u8], maximum: usize, seed: u32, cycle: usize) {
    let options = DecodeOptions {
        max_output_size: maximum,
        max_window_size: 1 << 27,
    };
    if let Ok(output) = decompress_with_options(frame, options) {
        assert!(output.len() <= maximum, "seed={seed}, cycle={cycle}");
    }
}

/// One-shot portions of fuzzerTests; retains RNG, profile weights, logarithmic
/// sample lengths, slice selection, and the noisy-source mutation algorithm.
fn fuzz_cycles(seed: u32, cycles: usize, maximum_log: u32) {
    let source_size = 1_usize << (maximum_log + 1);
    let profiles: Vec<_> = [0.0, 0.05, 0.5, 0.95, 1.0]
        .into_iter()
        .map(|probability| noise(source_size, probability, 0.0, seed))
        .collect();
    let mut core = FuzzerRng::new(seed);
    for cycle in 1..=cycles {
        core.next();
        let mut random = FuzzerRng::new(core.state() ^ 2_654_435_761);
        let source = &profiles[random.noise_index()];
        let sample_size = random.log_length(maximum_log);
        let start = random.next() as usize % (source.len() - sample_size);
        let sample = &source[start..start + sample_size];
        let mut frame = compress(sample).unwrap_or_else(|error| {
            panic!("compress seed={seed}, cycle={cycle}, size={sample_size}: {error}")
        });
        assert_eq!(
            FrameHeader::parse(&frame).unwrap().0.content_size,
            Some(sample_size as u64)
        );
        let margin = if random.next() & 1 != 0 {
            0
        } else {
            (random.next() & 31) as usize + 1
        };
        assert_eq!(
            decompress(&frame, sample_size + margin).unwrap(),
            sample,
            "seed={seed}, cycle={cycle}"
        );
        let missing = random.next() as usize % (frame.len() - 2) + 1;
        assert!(
            decompress(&frame[..frame.len() - missing], sample_size).is_err(),
            "seed={seed}, cycle={cycle}"
        );
        if sample_size > 3 {
            let missing = random.next() as usize % (sample_size - 2) + 1;
            assert_eq!(
                decompress(&frame, sample_size - missing),
                Err(Error::ResourceLimit),
                "seed={seed}, cycle={cycle}"
            );
        }
        let maximum_bits = usize::BITS - (frame.len() - 4).leading_zeros();
        let mut position = 4;
        loop {
            let bits = random.next() % maximum_bits;
            let mask = (1_usize << bits) - 1;
            position += random.next() as usize & mask;
            if position >= frame.len() {
                break;
            }
            let bits = (random.next() % maximum_bits).saturating_sub(1);
            let mask = (1_usize << bits) - 1;
            let length = ((random.next() as usize & mask) + 1).min(frame.len() - position);
            let start = random.next() as usize % (source.len() - length);
            frame[position..position + length].copy_from_slice(&source[start..start + length]);
            position += length;
        }
        bounded_noisy_decode(&frame, sample_size, seed, cycle);
    }
}

#[test]
fn deterministic_fuzzer_one_shot_cycles() {
    for seed in [0, 1, SEED, u32::MAX] {
        fuzz_cycles(seed, 64, 17);
    }
}

/// Upstream uses 30,000 iterations and maxSampleLog=22. The full run is
/// intentionally opt-in; environment overrides make failures reproducible.
#[test]
#[ignore = "intensive upstream-sized fuzz run; ZSTDOXIDE_FUZZ_CYCLES and ZSTDOXIDE_FUZZ_SEED override defaults"]
fn intensive_fuzzer_one_shot_cycles() {
    fn setting<T: core::str::FromStr>(name: &str, default: T) -> T {
        match std::env::var(name) {
            Ok(value) => value
                .parse()
                .unwrap_or_else(|_| panic!("invalid {name}: {value}")),
            Err(std::env::VarError::NotPresent) => default,
            Err(error) => panic!("invalid {name}: {error}"),
        }
    }
    let cycles = setting("ZSTDOXIDE_FUZZ_CYCLES", 30_000);
    let seed = setting("ZSTDOXIDE_FUZZ_SEED", SEED);
    assert!(cycles > 0, "at least one fuzz cycle is required");
    fuzz_cycles(seed, cycles, 22);
}
