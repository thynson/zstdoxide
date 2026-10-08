//! Deterministic, dependency-free backend comparisons. See docs/benchmark-design.md.
// Source modules include unit-test imports under Cargo benchmark configuration.
// This target has no libtest harness, so those imports are intentionally unused.
#![allow(dead_code, unused_imports)]

extern crate alloc;

use std::hint::black_box;
use std::time::{Duration, Instant};
use zstdoxide::{Error, Result};

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
    measure(&settings, "compress", "byte", original.len(), || {
        fingerprint(&zstdoxide::compress(black_box(&original)).unwrap())
    });

    let frame = include_bytes!("../tests/fixtures/text-3.zst");
    let expected = zstdoxide::decompress(frame, 370_326).unwrap();
    assert_eq!(expected.len(), 370_326);
    measure(&settings, "decompress", "byte", expected.len(), || {
        fingerprint(&zstdoxide::decompress(black_box(frame), 370_326).unwrap())
    });
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
