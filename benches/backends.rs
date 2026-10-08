//! Deterministic, dependency-free backend comparisons. See docs/benchmark-design.md.
// Source modules include unit-test imports under Cargo benchmark configuration.
// This target has no libtest harness, so those imports are intentionally unused.
#![allow(dead_code, unused_imports)]

extern crate alloc;

use std::hint::black_box;
use std::time::{Duration, Instant};
use zstdoxide::{DecodeOptions, Decoder, Encoder, EndDirective, Error, Result, StreamStatus};

#[path = "../src/bitstream.rs"]
mod bitstream;
#[path = "../src/cursor.rs"]
mod cursor;
#[path = "../src/fse.rs"]
mod fse;
#[path = "../src/history.rs"]
mod history;
#[path = "../src/huffman.rs"]
mod huffman;
#[path = "../src/matchfinder.rs"]
mod matchfinder;
#[path = "../src/ring_history.rs"]
mod ring_history;

const SEED: u64 = 0x5eed_d15c_0dec_2026;
const INPUT_SIZE: usize = 64 * 1024;

struct Settings {
    samples: usize,
    duration: Duration,
}

impl Settings {
    fn read() -> Self {
        let samples = std::env::var("ZSTDOXIDE_BENCH_SAMPLES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(5);
        let milliseconds = std::env::var("ZSTDOXIDE_BENCH_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(150);
        assert!((1..=100).contains(&samples));
        assert!((1..=60_000).contains(&milliseconds));
        Self {
            samples,
            duration: Duration::from_millis(milliseconds),
        }
    }
}

fn main() {
    let settings = Settings::read();
    println!("META\tseed\t{SEED:016x}");
    println!("META\tsamples\t{}", settings.samples);
    println!("META\tsample_ms\t{}", settings.duration.as_millis());
    println!("META\tfeatures\t{}", enabled_features());
    println!(
        "COLUMNS\tcase\tunit\tunits_per_iteration\tmedian_ns_per_unit\tminimum_ns_per_unit\tmaximum_ns_per_unit\tfingerprint"
    );

    let input = deterministic_bytes(INPUT_SIZE);
    measure(&settings, "byte_cursor", "byte", INPUT_SIZE, || {
        let mut reader = cursor::ByteCursor::new(black_box(&input));
        let mut digest = 0_u64;
        let mut width = 1;
        while !reader.remaining().is_empty() {
            let count = width.min(reader.remaining().len());
            digest = digest.rotate_left(3) ^ reader.little_endian(count).unwrap();
            width = width % 8 + 1;
        }
        digest
    });

    let mut reverse_input = input.clone();
    *reverse_input.last_mut().unwrap() |= 0x80;
    measure(&settings, "reverse_bits", "byte", INPUT_SIZE, || {
        let mut reader = bitstream::ReverseBits::new(black_box(&reverse_input)).unwrap();
        let mut digest = 0_u64;
        let widths = [3, 5, 7, 11, 13, 17];
        let mut index = 0;
        while reader.remaining() != 0 {
            let count = widths[index % widths.len()].min(reader.remaining()) as u8;
            digest = digest.rotate_left(3) ^ u64::from(reader.read_bits(count).unwrap());
            index += 1;
        }
        digest
    });

    let table = fse::FseTable::from_normalized(&[0, 5, -1, 17, 9], 5).unwrap();
    measure(&settings, "fse_lookup", "transition", INPUT_SIZE, || {
        let mut digest = 0_u64;
        for &byte in black_box(&input) {
            let entry = table.entry(usize::from(byte & 31)).unwrap();
            digest = digest.rotate_left(3)
                ^ u64::from(entry.symbol)
                ^ (u64::from(entry.bits) << 8)
                ^ (u64::from(entry.base) << 16);
        }
        digest
    });

    let symbols: Vec<_> = input.iter().map(|byte| byte & 3).collect();
    let encoded_symbols = huffman_stream(&symbols);
    let (table, _) = huffman::HuffmanTable::read(&[130, 0x11, 0x20]).unwrap();
    assert_eq!(
        table.decode_stream(&encoded_symbols, INPUT_SIZE).unwrap(),
        symbols
    );
    measure(&settings, "huffman_decode", "byte", INPUT_SIZE, || {
        let output = table
            .decode_stream(black_box(&encoded_symbols), INPUT_SIZE)
            .unwrap();
        fingerprint(&output)
    });

    let prefix = &input[..7];
    measure(&settings, "history_overlap", "byte", INPUT_SIZE, || {
        let mut output = history::History::new(INPUT_SIZE);
        output.start_frame(INPUT_SIZE);
        output.append(black_box(prefix)).unwrap();
        output.copy_match(7, INPUT_SIZE - 7).unwrap();
        fingerprint(output.frame_bytes())
    });

    measure_ring_history(&settings, &input);

    let mut table = matchfinder::MatchTable::new().unwrap();
    let words: Vec<[u8; 4]> = input
        .chunks_exact(4)
        .map(|bytes| bytes.try_into().unwrap())
        .collect();
    measure(
        &settings,
        "match_table",
        "candidate",
        words.len() * 2,
        || {
            table.reset();
            let mut digest = 0_u64;
            for (index, &word) in black_box(&words).iter().chain(words.iter()).enumerate() {
                let old = table.replace(word, index * 4);
                digest = digest.rotate_left(3) ^ old.map_or(0, |position| position as u64 + 1);
            }
            digest
        },
    );

    let original = compression_input(&input);
    let compressed = zstdoxide::compress(&original).unwrap();
    assert_eq!(
        zstdoxide::decompress(&compressed, original.len()).unwrap(),
        original
    );
    println!("META\tcompress_original_bytes\t{}", original.len());
    println!("META\tcompress_encoded_bytes\t{}", compressed.len());
    println!(
        "META\tcompress_output_fnv1a64\t{:016x}",
        full_checksum(&compressed)
    );
    measure(&settings, "compress", "byte", original.len(), || {
        fingerprint(&zstdoxide::compress(black_box(&original)).unwrap())
    });

    let frame = include_bytes!("../tests/fixtures/text-3.zst");
    let expected = zstdoxide::decompress(frame, 370_326).unwrap();
    assert_eq!(expected.len(), 370_326);
    println!("META\tdecompress_encoded_bytes\t{}", frame.len());
    println!("META\tdecompress_decoded_bytes\t{}", expected.len());
    println!(
        "META\tdecompress_output_fnv1a64\t{:016x}",
        full_checksum(&expected)
    );
    measure(&settings, "decompress", "byte", expected.len(), || {
        fingerprint(&zstdoxide::decompress(black_box(frame), 370_326).unwrap())
    });

    measure_streaming(&settings, &original, frame, &expected);
}

/// Reuse a small ring across frames while emitting much more than its window.
/// Full output remains observable; this workload wraps at distance seven.
fn measure_ring_history(settings: &Settings, input: &[u8]) {
    const WINDOW: usize = 4096;
    const OUTPUT_CHUNK: usize = 1024;
    let prefix = &input[..7];
    let mut ring = ring_history::RingHistory::new(WINDOW);
    let mut output = vec![0; INPUT_SIZE];
    {
        let mut run = || {
            ring.start_frame(WINDOW).unwrap();
            let mut position = ring.append(black_box(prefix), &mut output).unwrap();
            while position < output.len() {
                let end = (position + OUTPUT_CHUNK).min(output.len());
                let count = ring
                    .copy_match(7, output.len() - position, &mut output[position..end])
                    .unwrap();
                position += count;
            }
            fingerprint(&output)
        };
        run();
    }
    let expected: Vec<_> = prefix.iter().copied().cycle().take(INPUT_SIZE).collect();
    assert_eq!(output, expected);
    println!("META\tring_window_bytes\t{WINDOW}");
    println!("META\tring_output_chunk_bytes\t{OUTPUT_CHUNK}");
    println!("META\tring_retained_bytes\t{}", ring.retained_capacity());
    println!("META\tring_output_fnv1a64\t{:016x}", full_checksum(&output));
    measure(settings, "ring_overlap", "byte", INPUT_SIZE, || {
        ring.start_frame(WINDOW).unwrap();
        let mut position = ring.append(black_box(prefix), &mut output).unwrap();
        while position < output.len() {
            let end = (position + OUTPUT_CHUNK).min(output.len());
            let count = ring
                .copy_match(7, output.len() - position, &mut output[position..end])
                .unwrap();
            position += count;
        }
        fingerprint(&output)
    });
}

const DECODE_INPUT_CHUNK: usize = 257;
const DECODE_OUTPUT_CHUNK: usize = 4096;
const ENCODE_INPUT_CHUNK: usize = 4096;
const ENCODE_OUTPUT_CHUNK: usize = 1024;

/// Decode exactly one frame without sealing the context, allowing concatenation.
/// Returns delivered bytes and the largest observed retained context capacity.
fn decode_stream_frame(decoder: &mut Decoder, input: &[u8], output: &mut [u8]) -> (usize, usize) {
    let mut consumed = 0;
    let mut produced = 0;
    let mut retained = decoder.retained_capacity();
    loop {
        let input_end = (consumed + DECODE_INPUT_CHUNK).min(input.len());
        let output_end = (produced + DECODE_OUTPUT_CHUNK).min(output.len());
        let progress = decoder
            .process(
                &input[consumed..input_end],
                &mut output[produced..output_end],
            )
            .unwrap();
        assert!(progress.consumed <= input_end - consumed);
        assert!(progress.produced <= output_end - produced);
        consumed += progress.consumed;
        produced += progress.produced;
        retained = retained.max(decoder.retained_capacity());
        if progress.status == StreamStatus::FrameFinished {
            assert_eq!(consumed, input.len());
            return (produced, retained);
        }
        assert!(
            progress.consumed != 0 || progress.produced != 0,
            "decoder stalled"
        );
    }
}

/// Encode a frame using a reusable context and a fixed caller-owned destination.
fn encode_stream_frame(encoder: &mut Encoder, input: &[u8], output: &mut [u8]) -> usize {
    let mut consumed = 0;
    let mut produced = 0;
    loop {
        let input_end = (consumed + ENCODE_INPUT_CHUNK).min(input.len());
        let output_end = (produced + ENCODE_OUTPUT_CHUNK).min(output.len());
        let directive = if input_end == input.len() {
            EndDirective::Finish
        } else {
            EndDirective::Continue
        };
        let progress = encoder
            .process(
                &input[consumed..input_end],
                &mut output[produced..output_end],
                directive,
            )
            .unwrap();
        assert!(progress.consumed <= input_end - consumed);
        assert!(progress.produced <= output_end - produced);
        consumed += progress.consumed;
        produced += progress.produced;
        if progress.status == StreamStatus::FrameFinished {
            assert_eq!(consumed, input.len());
            return produced;
        }
        assert!(
            progress.consumed != 0 || progress.produced != 0,
            "encoder stalled"
        );
    }
}

fn measure_streaming(settings: &Settings, original: &[u8], frame: &[u8], expected: &[u8]) {
    println!("META\tstream_decode_input_chunk_bytes\t{DECODE_INPUT_CHUNK}");
    println!("META\tstream_decode_output_chunk_bytes\t{DECODE_OUTPUT_CHUNK}");
    println!("META\tstream_encode_input_chunk_bytes\t{ENCODE_INPUT_CHUNK}");
    println!("META\tstream_encode_output_chunk_bytes\t{ENCODE_OUTPUT_CHUNK}");
    let mut decoder = Decoder::new(DecodeOptions::new(expected.len()));
    let mut decoded = vec![0; expected.len()];
    let (size, peak) = decode_stream_frame(&mut decoder, frame, &mut decoded);
    decoder.finish().unwrap();
    assert_eq!(size, expected.len());
    assert_eq!(decoded, expected);
    let retained = decoder.retained_capacity();
    decoder.reset();
    assert_eq!(decoder.retained_capacity(), retained);
    println!("META\tstream_decompress_encoded_bytes\t{}", frame.len());
    println!("META\tstream_decompress_decoded_bytes\t{}", expected.len());
    println!(
        "META\tstream_decompress_output_fnv1a64\t{:016x}",
        full_checksum(&decoded)
    );
    println!("META\tstream_decompress_peak_retained_bytes\t{peak}");
    println!("META\tstream_decompress_reset_retained_bytes\t{retained}");
    measure(
        settings,
        "stream_decompress",
        "byte",
        expected.len(),
        || {
            decoder.reset();
            let (size, _) = decode_stream_frame(&mut decoder, black_box(frame), &mut decoded);
            decoder.finish().unwrap();
            assert_eq!(size, expected.len());
            fingerprint(&decoded)
        },
    );

    let mut encoder = Encoder::new();
    // Generous fixed destination for this selected workload. It remains caller
    // memory and is excluded from the context's retained-capacity measurement.
    let mut encoded = vec![0; original.len() * 2 + 1024];
    let size = encode_stream_frame(&mut encoder, original, &mut encoded);
    assert_eq!(
        zstdoxide::decompress(&encoded[..size], original.len()).unwrap(),
        original
    );
    let retained = encoder.retained_capacity();
    encoder.reset();
    assert_eq!(encoder.retained_capacity(), retained);
    println!("META\tstream_compress_original_bytes\t{}", original.len());
    println!("META\tstream_compress_encoded_bytes\t{size}");
    println!(
        "META\tstream_compress_output_fnv1a64\t{:016x}",
        full_checksum(&encoded[..size])
    );
    println!("META\tstream_compress_reset_retained_bytes\t{retained}");
    measure(settings, "stream_compress", "byte", original.len(), || {
        encoder.reset();
        let produced = encode_stream_frame(&mut encoder, black_box(original), &mut encoded);
        assert_eq!(produced, size);
        fingerprint(&encoded[..produced])
    });

    // These exact-byte and retained-memory checks are outside timed regions.
    decoder_memory_plateau(frame, expected);
    encoder_memory_plateau(original);
}

/// Compare context high-water capacity after four and sixty-four whole frames.
fn decoder_memory_plateau(frame: &[u8], expected: &[u8]) {
    let mut decoder = Decoder::new(DecodeOptions::new(expected.len() * 64));
    let mut output = vec![0; expected.len()];
    let mut peak = 0;
    let mut short_peak = 0;
    for frame_index in 0..64 {
        let (size, retained) = decode_stream_frame(&mut decoder, frame, &mut output);
        assert_eq!(size, expected.len());
        assert_eq!(output, expected);
        peak = peak.max(retained);
        if frame_index == 3 {
            short_peak = peak;
        }
    }
    decoder.finish().unwrap();
    assert_eq!(
        short_peak, peak,
        "decoder retained memory grew with completed frames"
    );
    println!("META\tstream_decompress_memory_short_frames\t4");
    println!("META\tstream_decompress_memory_long_frames\t64");
    println!(
        "META\tstream_decompress_memory_output_bytes\t{}",
        expected.len() * 64
    );
    println!("META\tstream_decompress_peak_retained_bytes_4_frames\t{short_peak}");
    println!("META\tstream_decompress_peak_retained_bytes_64_frames\t{peak}");
}

/// Feed the same full block repeatedly into one frame and compare capacity.
/// Caller-collected output is used for correctness, not counted as codec state.
fn encoder_memory_plateau(original: &[u8]) {
    let block: Vec<_> = original.iter().copied().cycle().take(128 * 1024).collect();
    let mut encoder = Encoder::new();
    let mut scratch = [0; ENCODE_OUTPUT_CHUNK];
    let mut encoded = Vec::new();
    let mut peak = 0;
    let mut short_peak = 0;
    for block_index in 0..64 {
        let mut consumed = 0;
        loop {
            let end = (consumed + ENCODE_INPUT_CHUNK).min(block.len());
            let directive = if block_index == 63 && end == block.len() {
                EndDirective::Finish
            } else {
                EndDirective::Continue
            };
            let progress = encoder
                .process(&block[consumed..end], &mut scratch, directive)
                .unwrap();
            assert!(progress.consumed <= end - consumed);
            assert!(progress.produced <= scratch.len());
            consumed += progress.consumed;
            encoded.extend_from_slice(&scratch[..progress.produced]);
            peak = peak.max(encoder.retained_capacity());
            if progress.status == StreamStatus::FrameFinished {
                assert_eq!(block_index, 63);
                assert_eq!(consumed, block.len());
                break;
            }
            if consumed == block.len() && progress.status == StreamStatus::NeedInput {
                assert!(block_index < 63);
                break;
            }
            assert!(
                progress.consumed != 0 || progress.produced != 0,
                "memory trial stalled"
            );
        }
        if block_index == 3 {
            short_peak = peak;
        }
    }
    assert_eq!(
        short_peak, peak,
        "encoder retained memory grew with delivered input"
    );
    let expected: Vec<_> = block
        .iter()
        .copied()
        .cycle()
        .take(block.len() * 64)
        .collect();
    assert_eq!(
        zstdoxide::decompress(&encoded, expected.len()).unwrap(),
        expected
    );
    decoder_single_frame_memory_plateau(&encoded, &expected, block.len() * 4);
    println!("META\tstream_compress_memory_block_bytes\t{}", block.len());
    println!("META\tstream_compress_memory_short_blocks\t4");
    println!("META\tstream_compress_memory_long_blocks\t64");
    println!(
        "META\tstream_compress_memory_input_bytes\t{}",
        expected.len()
    );
    println!(
        "META\tstream_compress_memory_encoded_bytes\t{}",
        encoded.len()
    );
    println!(
        "META\tstream_compress_memory_output_fnv1a64\t{:016x}",
        full_checksum(&encoded)
    );
    println!("META\tstream_compress_peak_retained_bytes_4_blocks\t{short_peak}");
    println!("META\tstream_compress_peak_retained_bytes_64_blocks\t{peak}");
}

/// Decode one long frame into a small reused caller buffer. The prefix and full
/// frame use the same window, so retained capacity must not track frame length.
fn decoder_single_frame_memory_plateau(encoded: &[u8], expected: &[u8], short_size: usize) {
    let mut decoder = Decoder::new(DecodeOptions::new(expected.len()));
    let mut scratch = [0; DECODE_OUTPUT_CHUNK];
    let mut consumed = 0;
    let mut produced = 0;
    let mut peak = 0;
    let mut short_peak = None;
    loop {
        let end = (consumed + DECODE_INPUT_CHUNK).min(encoded.len());
        let progress = decoder
            .process(&encoded[consumed..end], &mut scratch)
            .unwrap();
        assert!(progress.consumed <= end - consumed);
        assert!(progress.produced <= scratch.len());
        assert_eq!(
            scratch[..progress.produced],
            expected[produced..produced + progress.produced]
        );
        consumed += progress.consumed;
        produced += progress.produced;
        peak = peak.max(decoder.retained_capacity());
        if produced >= short_size && short_peak.is_none() {
            short_peak = Some(peak);
        }
        if progress.status == StreamStatus::FrameFinished {
            assert_eq!(consumed, encoded.len());
            assert_eq!(produced, expected.len());
            break;
        }
        assert!(
            progress.consumed != 0 || progress.produced != 0,
            "single-frame memory trial stalled"
        );
    }
    decoder.finish().unwrap();
    let short_peak = short_peak.unwrap();
    assert_eq!(
        short_peak, peak,
        "decoder retained memory grew with one frame's delivered output"
    );
    println!("META\tstream_decompress_single_frame_short_bytes\t{short_size}");
    println!(
        "META\tstream_decompress_single_frame_long_bytes\t{}",
        expected.len()
    );
    println!("META\tstream_decompress_single_frame_short_retained_bytes\t{short_peak}");
    println!("META\tstream_decompress_single_frame_long_retained_bytes\t{peak}");
}

fn measure(
    settings: &Settings,
    name: &str,
    unit: &str,
    units: usize,
    mut operation: impl FnMut() -> u64,
) {
    let expected = black_box(operation());
    let mut measurements = Vec::with_capacity(settings.samples);
    for _ in 0..settings.samples {
        let start = Instant::now();
        let mut iterations = 0_u64;
        loop {
            let actual = black_box(operation());
            assert_eq!(actual, expected, "nondeterministic output in {name}");
            iterations += 1;
            if start.elapsed() >= settings.duration {
                break;
            }
        }
        measurements.push(start.elapsed().as_secs_f64() * 1e9 / (iterations as f64 * units as f64));
    }
    measurements.sort_by(f64::total_cmp);
    let middle = measurements.len() / 2;
    let median = if measurements.len() % 2 == 0 {
        (measurements[middle - 1] + measurements[middle]) / 2.0
    } else {
        measurements[middle]
    };
    println!(
        "RESULT\t{name}\t{unit}\t{units}\t{median:.4}\t{:.4}\t{:.4}\t{expected:016x}",
        measurements[0],
        measurements[measurements.len() - 1]
    );
}

/// Sample 64 positions while keeping the complete output observable to LLVM.
fn fingerprint(bytes: &[u8]) -> u64 {
    let bytes = black_box(bytes);
    let mut digest = bytes.len() as u64;
    if !bytes.is_empty() {
        for sample in 0..64 {
            digest = digest.rotate_left(7) ^ u64::from(bytes[sample * (bytes.len() - 1) / 63]);
        }
    }
    digest
}

/// Hash every byte outside the timed region using the standard FNV-1a recurrence.
/// This is a reproducibility checksum, not a cryptographic digest.
fn full_checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn deterministic_bytes(length: usize) -> Vec<u8> {
    let mut state = SEED;
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

fn huffman_stream(symbols: &[u8]) -> Vec<u8> {
    // The table's canonical codes are 000, 001, 01, and 1. Write fields
    // backward to obtain the reverse stream consumed by the decoder.
    let mut output = vec![0; (symbols.len() * 3 + 1).div_ceil(8)];
    let mut position = 0;
    for &symbol in symbols.iter().rev() {
        let (value, count) = match symbol {
            0 => (0, 3),
            1 => (1, 3),
            2 => (1, 2),
            3 => (1, 1),
            _ => unreachable!(),
        };
        for bit in 0..count {
            output[position / 8] |= ((value >> bit) & 1) << (position % 8);
            position += 1;
        }
    }
    output[position / 8] |= 1 << (position % 8);
    output.truncate((position + 1).div_ceil(8));
    output
}

fn compression_input(random: &[u8]) -> Vec<u8> {
    let line =
        b"Zstandard Rust codec: checked indexes, deterministic output, bounded allocation.\n";
    let mut output = Vec::new();
    for index in 0..1024 {
        output.extend_from_slice(line);
        output.extend_from_slice(&random[(index % 64) * 16..(index % 64) * 16 + 16]);
    }
    output
}

fn enabled_features() -> String {
    let features = [
        ("unsafe-byte-cursor", cfg!(feature = "unsafe-byte-cursor")),
        ("unsafe-bitstream", cfg!(feature = "unsafe-bitstream")),
        ("unsafe-fse", cfg!(feature = "unsafe-fse")),
        ("unsafe-huffman", cfg!(feature = "unsafe-huffman")),
        ("unsafe-history", cfg!(feature = "unsafe-history")),
        ("unsafe-ring-history", cfg!(feature = "unsafe-ring-history")),
        ("unsafe-match-table", cfg!(feature = "unsafe-match-table")),
    ];
    let selected: Vec<_> = features
        .iter()
        .filter_map(|&(name, enabled)| enabled.then_some(name))
        .collect();
    if selected.is_empty() {
        "safe".into()
    } else {
        selected.join(",")
    }
}
