//! Deterministic, dependency-free backend comparisons. See docs/benchmark-design.md.
// Source modules include unit-test imports under Cargo benchmark configuration.
// This target has no libtest harness, so those imports are intentionally unused.
#![allow(dead_code, unused_imports)]

extern crate alloc;

use std::hint::black_box;
use std::time::{Duration, Instant};
use zstdoxide::{
    CompressionStrategy, DecodeOptions, Decoder, Dictionary, Encoder, EncoderOptions, EndDirective,
    Error, RawTrainingOptions, Result, StreamStatus, train_raw_dictionary,
};

#[path = "../src/bitstream.rs"]
mod bitstream;
#[path = "../src/bitwriter.rs"]
mod bitwriter;
#[path = "../src/constants.rs"]
mod constants;
#[path = "../src/cursor.rs"]
mod cursor;
#[path = "../src/entropy_encode.rs"]
mod entropy_encode;
#[path = "../src/frame.rs"]
mod frame;
#[path = "../src/fse.rs"]
mod fse;
#[path = "../src/fse_encode.rs"]
mod fse_encode;
#[path = "../src/history.rs"]
mod history;
#[path = "../src/huffman.rs"]
mod huffman;
#[path = "../src/huffman_codebook.rs"]
mod huffman_codebook;
#[path = "../src/matchchain.rs"]
mod matchchain;
#[path = "../src/matchfinder.rs"]
mod matchfinder;
#[path = "../src/ring_history.rs"]
mod ring_history;

const SEED: u64 = 0x5eed_d15c_0dec_2026;
const INPUT_SIZE: usize = 64 * 1024;

struct Settings {
    samples: usize,
    duration: Duration,
    smoke: bool,
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
            smoke: std::env::var("ZSTDOXIDE_BENCH_SMOKE").is_ok_and(|value| value == "1"),
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
        "META\tmode\t{}",
        if settings.smoke { "smoke" } else { "timed" }
    );
    if settings.smoke {
        println!("COLUMNS\tcase\tunit\tunits_per_operation\tfingerprint");
    } else {
        println!(
            "COLUMNS\tcase\tunit\tunits_per_iteration\tmedian_ns_per_unit\tminimum_ns_per_unit\tmaximum_ns_per_unit\tfingerprint"
        );
    }

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
    measure_dictionaries(&settings, &input);
    measure_raw_training(&settings);

    // Append new cases so consumers retain the original fifteen-case order.
    measure_encoder_accessors(&settings, &input);
    measure_strategies(&settings, &original);
}

/// Isolate the new checked accessors using prebuilt owned storage. Query tuples
/// and expected result byte vectors contain scalar values only; they need no
/// artificial unsafe representation. All complete relation checks precede timing.
fn measure_encoder_accessors(settings: &Settings, input: &[u8]) {
    let counts = [0, 5, -1, 17, 9];
    let decoding = fse::FseTable::from_normalized(&counts, 5).unwrap();
    let encoding = fse_encode::FseEncodingTable::from_decoding(&decoding).unwrap();
    for symbol in 1..=4 {
        for next in 0..32 {
            let inverse = encoding.transition(symbol, next).unwrap();
            let entry = decoding.entry(usize::from(inverse.state)).unwrap();
            assert_eq!(usize::from(entry.symbol), symbol);
            assert_eq!(entry.bits, inverse.bits);
            assert_eq!(entry.base + inverse.value, next);
            assert!(u32::from(inverse.value) < 1 << inverse.bits);
        }
    }
    let queries: Vec<_> = input
        .chunks_exact(2)
        .map(|pair| (usize::from(pair[0] & 3) + 1, u16::from(pair[1] & 31)))
        .collect();
    let mut complete = Vec::with_capacity(queries.len() * 5);
    for &(symbol, next) in &queries {
        let inverse = encoding.transition(symbol, next).unwrap();
        complete.extend_from_slice(&inverse.state.to_le_bytes());
        complete.extend_from_slice(&inverse.value.to_le_bytes());
        complete.push(inverse.bits);
    }
    println!("META\tfse_encode_lookup_transitions\t{}", queries.len());
    println!("META\tfse_encode_lookup_table_log\t5");
    println!(
        "META\tfse_encode_lookup_retained_bytes\t{}",
        encoding.retained_capacity()
    );
    println!(
        "META\tfse_encode_lookup_output_fnv1a64\t{:016x}",
        full_checksum(&complete)
    );
    measure(
        settings,
        "fse_encode_lookup",
        "transition",
        queries.len(),
        || {
            let mut digest = 0_u64;
            for &(symbol, next) in black_box(&queries) {
                let inverse = encoding.transition(symbol, next).unwrap();
                digest = digest.rotate_left(3)
                    ^ u64::from(inverse.state)
                    ^ (u64::from(inverse.value) << 16)
                    ^ (u64::from(inverse.bits) << 32);
            }
            digest
        },
    );

    // Complete Kraft mass: one 1-bit symbol, one 8-bit symbol, and 254 9-bit
    // symbols. Independently enumerated canonical ranks exercise all 256 slots.
    let mut lengths = [9; 256];
    lengths[0] = 1;
    lengths[1] = 8;
    let codebook = huffman_codebook::HuffmanCodebook::from_lengths(&lengths, 9).unwrap();
    let mut complete = Vec::with_capacity(input.len() * 3);
    for &symbol in input {
        let code = codebook.code(symbol).unwrap();
        let expected = match symbol {
            0 => (1, 1),
            1 => (127, 8),
            _ => (u16::from(symbol) - 2, 9),
        };
        assert_eq!((code.value, code.bits), expected);
        complete.extend_from_slice(&code.value.to_le_bytes());
        complete.push(code.bits);
    }
    println!("META\thuffman_codebook_symbols\t{}", input.len());
    println!("META\thuffman_codebook_table_log\t9");
    println!(
        "META\thuffman_codebook_inline_bytes\t{}",
        core::mem::size_of_val(&codebook)
    );
    println!(
        "META\thuffman_codebook_output_fnv1a64\t{:016x}",
        full_checksum(&complete)
    );
    measure(settings, "huffman_codebook", "symbol", input.len(), || {
        let mut digest = 0_u64;
        for &symbol in black_box(input) {
            let code = codebook.code(symbol).unwrap();
            digest = digest.rotate_left(3) ^ u64::from(code.value) ^ (u64::from(code.bits) << 16);
        }
        digest
    });

    // Repeated keys retain sixteen candidates apiece. The independent expected
    // lists use insertion coordinates only, and the distinct buckets are checked
    // during setup. This hot traversal includes neither allocation nor insertion.
    const KEYS: usize = 256;
    const ROUNDS: usize = 16;
    const BUDGET: usize = 16;
    let keys: Vec<_> = (0..KEYS as u32).map(u32::to_le_bytes).collect();
    let mut buckets = std::collections::BTreeSet::new();
    for &key in &keys {
        let bucket = u32::from_le_bytes(key).wrapping_mul(0x9e37_79b1) >> 16;
        assert!(buckets.insert(bucket), "benchmark keys share a hash bucket");
    }
    let mut chain = matchchain::MatchChain::new(KEYS * ROUNDS).unwrap();
    for round in 0..ROUNDS {
        for (index, &key) in keys.iter().enumerate() {
            chain.insert(key, (round * KEYS + index) as u64).unwrap();
        }
    }
    let mut complete = Vec::with_capacity(input.len() * BUDGET * 8);
    for &index in input {
        let actual: Vec<_> = chain
            .candidates(keys[usize::from(index)], u64::MAX, BUDGET)
            .unwrap()
            .collect();
        let expected: Vec<_> = (0..ROUNDS)
            .rev()
            .map(|round| (round * KEYS + usize::from(index)) as u64)
            .collect();
        assert_eq!(actual, expected);
        for position in actual {
            complete.extend_from_slice(&position.to_le_bytes());
        }
    }
    println!("META\tmatch_chain_queries\t{}", input.len());
    println!("META\tmatch_chain_candidates\t{}", input.len() * BUDGET);
    println!("META\tmatch_chain_node_limit\t{}", KEYS * ROUNDS);
    println!("META\tmatch_chain_visit_budget\t{BUDGET}");
    println!(
        "META\tmatch_chain_retained_bytes\t{}",
        chain.retained_capacity()
    );
    println!(
        "META\tmatch_chain_output_fnv1a64\t{:016x}",
        full_checksum(&complete)
    );
    measure(
        settings,
        "match_chain",
        "candidate",
        input.len() * BUDGET,
        || {
            let mut digest = 0_u64;
            for &index in black_box(input) {
                for position in chain
                    .candidates(keys[usize::from(index)], u64::MAX, BUDGET)
                    .unwrap()
                {
                    digest = digest.rotate_left(3) ^ position;
                }
            }
            digest
        },
    );
}

/// Prepare identical sources and optional raw dictionary once per strategy.
/// Dictionary parsing, first allocation, exact decoding, and complete hashes
/// stay outside timing; reusable-context reset remains inside each operation.
fn measure_strategies(settings: &Settings, compressible: &[u8]) {
    let random = deterministic_bytes(128 * 64);
    let mut dictionary_bytes = Vec::new();
    let mut ambiguous = Vec::new();
    for pattern in random.chunks_exact(64) {
        dictionary_bytes.push(b'X');
        dictionary_bytes.extend_from_slice(&pattern[..3]);
        dictionary_bytes.push(u8::from(pattern[3] == 0));
        dictionary_bytes.extend_from_slice(pattern);
        dictionary_bytes.push(0);
        ambiguous.push(b'X');
        ambiguous.extend_from_slice(pattern);
    }
    let dictionary = Dictionary::raw(&dictionary_bytes).unwrap().into_shared();
    for (name, strategy) in [
        ("greedy", CompressionStrategy::Greedy),
        ("chain", CompressionStrategy::Chain),
        ("lazy", CompressionStrategy::Lazy),
    ] {
        for (source_name, source, selected_dictionary) in [
            ("compressible", compressible, None),
            ("ambiguous", ambiguous.as_slice(), Some(&dictionary)),
        ] {
            let case = format!("strategy_{name}_{source_name}");
            let options = EncoderOptions {
                strategy,
                ..EncoderOptions::default()
            };
            let mut encoder = if let Some(dictionary) = selected_dictionary {
                Encoder::with_dictionary(options, dictionary.clone()).unwrap()
            } else {
                Encoder::with_options(options).unwrap()
            };
            let mut encoded = vec![0; source.len() * 2 + 1024];
            let size = encode_stream_frame(&mut encoder, source, &mut encoded);
            let expected = encoded[..size].to_vec();
            let decoded = if let Some(dictionary) = selected_dictionary {
                zstdoxide::decompress_with_dictionary(
                    &expected,
                    dictionary,
                    DecodeOptions::new(source.len()),
                )
                .unwrap()
            } else {
                zstdoxide::decompress(&expected, source.len()).unwrap()
            };
            assert_eq!(decoded, source);
            let retained = encoder.retained_capacity();
            encoder.reset();
            assert_eq!(encoder.retained_capacity(), retained);
            assert_eq!(encoder.options().strategy, strategy);
            assert_eq!(
                encode_stream_frame(&mut encoder, source, &mut encoded),
                size
            );
            assert_eq!(encoded[..size], expected);
            let dictionary_source =
                selected_dictionary.map_or(&[][..], |_| dictionary_bytes.as_slice());
            println!("META\t{case}_original_bytes\t{}", source.len());
            println!(
                "META\t{case}_original_fnv1a64\t{:016x}",
                full_checksum(source)
            );
            println!("META\t{case}_encoded_bytes\t{size}");
            println!(
                "META\t{case}_output_fnv1a64\t{:016x}",
                full_checksum(&expected)
            );
            println!("META\t{case}_reset_retained_bytes\t{retained}");
            println!("META\t{case}_dictionary_bytes\t{}", dictionary_source.len());
            println!(
                "META\t{case}_dictionary_fnv1a64\t{:016x}",
                full_checksum(dictionary_source)
            );
            measure(settings, &case, "byte", source.len(), || {
                encoder.reset();
                let produced = encode_stream_frame(&mut encoder, black_box(source), &mut encoded);
                assert_eq!(produced, size);
                fingerprint(&encoded[..produced])
            });
        }
    }
}

/// Compare public dictionary operations with shared preparation outside timing.
fn measure_dictionaries(settings: &Settings, indices: &[u8]) {
    let serialized = include_bytes!("../tests/upstream/dictionaries/formatted.dict");
    let dictionary = Dictionary::from_bytes(serialized).unwrap().into_shared();
    println!("META\tdictionary_serialized_bytes\t{}", serialized.len());
    println!(
        "META\tdictionary_content_bytes\t{}",
        dictionary.content().len()
    );
    println!("META\tdictionary_id\t{}", dictionary.id());
    println!(
        "META\tdictionary_fnv1a64\t{:016x}",
        full_checksum(serialized)
    );
    let positions: Vec<_> = indices
        .chunks_exact(2)
        .map(|pair| {
            usize::from(u16::from_le_bytes([pair[0], pair[1]])) % dictionary.content().len()
        })
        .collect();
    let expected_reads: Vec<_> = positions
        .iter()
        .map(|&index| dictionary.content()[index])
        .collect();
    assert_eq!(
        positions
            .iter()
            .map(|&index| dictionary.content_byte(index).unwrap())
            .collect::<Vec<_>>(),
        expected_reads
    );
    println!("META\tdictionary_content_read_bytes\t{}", positions.len());
    println!(
        "META\tdictionary_content_output_fnv1a64\t{:016x}",
        full_checksum(&expected_reads)
    );
    measure(
        settings,
        "dictionary_content",
        "byte",
        positions.len(),
        || {
            let mut digest = 0_u64;
            for &index in black_box(&positions) {
                digest = digest.rotate_left(3) ^ u64::from(dictionary.content_byte(index).unwrap());
            }
            digest
        },
    );

    let original: Vec<_> = dictionary
        .content()
        .iter()
        .copied()
        .cycle()
        .take(32 * 1024)
        .collect();
    let mut encoder =
        Encoder::with_dictionary(EncoderOptions::default(), dictionary.clone()).unwrap();
    let mut encoded = vec![0; original.len() * 2 + 1024];
    let size = encode_stream_frame(&mut encoder, &original, &mut encoded);
    assert_eq!(
        zstdoxide::decompress_with_dictionary(
            &encoded[..size],
            &dictionary,
            DecodeOptions::new(original.len())
        )
        .unwrap(),
        original
    );
    let no_dictionary_size = zstdoxide::compress(&original).unwrap().len();
    assert!(
        size < no_dictionary_size,
        "dictionary benchmark must exercise useful matches"
    );
    let retained = encoder.retained_capacity();
    encoder.reset();
    assert_eq!(encoder.retained_capacity(), retained);
    println!(
        "META\tdictionary_compress_original_bytes\t{}",
        original.len()
    );
    println!(
        "META\tdictionary_compress_original_fnv1a64\t{:016x}",
        full_checksum(&original)
    );
    println!("META\tdictionary_compress_encoded_bytes\t{size}");
    println!("META\tdictionary_compress_without_dictionary_bytes\t{no_dictionary_size}");
    println!(
        "META\tdictionary_compress_output_fnv1a64\t{:016x}",
        full_checksum(&encoded[..size])
    );
    println!("META\tdictionary_compress_reset_retained_bytes\t{retained}");
    measure(
        settings,
        "dictionary_compress",
        "byte",
        original.len(),
        || {
            encoder.reset();
            let produced = encode_stream_frame(&mut encoder, black_box(&original), &mut encoded);
            assert_eq!(produced, size);
            fingerprint(&encoded[..produced])
        },
    );

    const FRAMES: usize = 64;
    let http_dictionary_bytes =
        include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
    let frame = include_bytes!("../tests/upstream/data/generated/http-dictionary.zst");
    let expected_frame = include_bytes!("../tests/upstream/data/golden-compression/http");
    let http = Dictionary::from_bytes(http_dictionary_bytes)
        .unwrap()
        .into_shared();
    let mut decoded = vec![0; expected_frame.len() * FRAMES];
    let expected: Vec<_> = expected_frame
        .iter()
        .copied()
        .cycle()
        .take(decoded.len())
        .collect();
    let mut decoder = Decoder::with_dictionary(DecodeOptions::new(decoded.len()), http.clone());
    let mut peak = 0;
    for output in decoded.chunks_exact_mut(expected_frame.len()) {
        let (produced, retained) = decode_stream_frame(&mut decoder, frame, output);
        assert_eq!(produced, expected_frame.len());
        peak = peak.max(retained);
    }
    decoder.finish().unwrap();
    assert_eq!(decoded, expected);
    let retained = decoder.retained_capacity();
    decoder.reset();
    assert_eq!(decoder.retained_capacity(), retained);
    println!(
        "META\tdictionary_decompress_dictionary_bytes\t{}",
        http_dictionary_bytes.len()
    );
    println!(
        "META\tdictionary_decompress_dictionary_content_bytes\t{}",
        http.content().len()
    );
    println!("META\tdictionary_decompress_dictionary_id\t{}", http.id());
    println!(
        "META\tdictionary_decompress_dictionary_fnv1a64\t{:016x}",
        full_checksum(http_dictionary_bytes)
    );
    println!("META\tdictionary_decompress_frames\t{FRAMES}");
    println!(
        "META\tdictionary_decompress_encoded_bytes\t{}",
        frame.len() * FRAMES
    );
    println!(
        "META\tdictionary_decompress_decoded_bytes\t{}",
        decoded.len()
    );
    println!(
        "META\tdictionary_decompress_output_fnv1a64\t{:016x}",
        full_checksum(&decoded)
    );
    println!("META\tdictionary_decompress_peak_retained_bytes\t{peak}");
    println!("META\tdictionary_decompress_reset_retained_bytes\t{retained}");
    measure(
        settings,
        "dictionary_decompress",
        "byte",
        decoded.len(),
        || {
            decoder.reset();
            for output in decoded.chunks_exact_mut(expected_frame.len()) {
                let (produced, _) = decode_stream_frame(&mut decoder, black_box(frame), output);
                assert_eq!(produced, expected_frame.len());
            }
            decoder.finish().unwrap();
            fingerprint(&decoded)
        },
    );
}

/// This measures the complete allocating selector, not only its frequency reads.
fn measure_raw_training(settings: &Settings) {
    let input = include_bytes!("../tests/upstream/dictionaries/training-samples.bin");
    let samples: Vec<_> = input.chunks_exact(512).collect();
    let options = RawTrainingOptions {
        dictionary_size: 2048,
        max_sample_bytes: 64 * 1024,
        segment_size: 64,
    };
    assert_eq!(samples.len(), 256);
    let dictionary = train_raw_dictionary(&samples, options).unwrap();
    assert!(!dictionary.is_formatted());
    assert!((8..=options.dictionary_size).contains(&dictionary.content().len()));
    let expected = dictionary.content();
    assert_eq!(
        train_raw_dictionary(&samples, options).unwrap().content(),
        expected
    );
    println!("META\traw_training_sample_count\t{}", samples.len());
    println!("META\traw_training_input_bytes\t{}", input.len());
    println!(
        "META\traw_training_input_fnv1a64\t{:016x}",
        full_checksum(input)
    );
    println!(
        "META\traw_training_sample_bytes_limit\t{}",
        options.max_sample_bytes
    );
    println!(
        "META\traw_training_dictionary_bytes_limit\t{}",
        options.dictionary_size
    );
    println!("META\traw_training_segment_bytes\t{}", options.segment_size);
    println!("META\traw_training_output_bytes\t{}", expected.len());
    println!(
        "META\traw_training_output_fnv1a64\t{:016x}",
        full_checksum(expected)
    );
    measure(
        settings,
        "raw_training",
        "byte",
        options.max_sample_bytes,
        || {
            let trained = train_raw_dictionary(black_box(&samples), options).unwrap();
            assert_eq!(trained.content().len(), expected.len());
            fingerprint(trained.content())
        },
    );
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
    if settings.smoke {
        assert_eq!(
            black_box(operation()),
            expected,
            "nondeterministic output in {name}"
        );
        println!("CHECK\t{name}\t{unit}\t{units}\t{expected:016x}");
        return;
    }
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
        ("unsafe-fse-encoder", cfg!(feature = "unsafe-fse-encoder")),
        ("unsafe-huffman", cfg!(feature = "unsafe-huffman")),
        (
            "unsafe-huffman-codebook",
            cfg!(feature = "unsafe-huffman-codebook"),
        ),
        ("unsafe-history", cfg!(feature = "unsafe-history")),
        ("unsafe-ring-history", cfg!(feature = "unsafe-ring-history")),
        ("unsafe-match-table", cfg!(feature = "unsafe-match-table")),
        ("unsafe-match-chain", cfg!(feature = "unsafe-match-chain")),
        ("unsafe-dictionary", cfg!(feature = "unsafe-dictionary")),
        ("unsafe-training", cfg!(feature = "unsafe-training")),
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
