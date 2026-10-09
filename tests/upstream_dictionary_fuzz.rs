// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Derived from facebook/zstd v1.5.7 tests/fuzz/dictionary_loader.c,
// raw_dictionary_round_trip.c, and fuzz_data_producer.c.
// Used under the BSD-3-Clause option; see LICENSE.zstd.
//! Dictionary fuzz contracts with the original tail-selected source regions.
//!
//! Ownership uses copied immutable shared dictionaries instead of C by-reference
//! load methods. Raw round trips preserve source/dictionary splitting, prefix
//! choice, checksum disabling, and the smaller compression-bound destination.
//! Advanced randomized parameters and external sequence producers remain outside
//! this adapter's supported API coverage. Neither target invokes dictionary training.
#![forbid(unsafe_code)]

#[path = "upstream/fuzz_support.rs"]
#[allow(dead_code)] // The shared producer also serves the other upstream adapters.
mod fuzz_support;

use fuzz_support::DataProducer;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use zstdoxide::{
    DecodeOptions, Decoder, Dictionary, DictionaryMode, Encoder, EncoderOptions, EndDirective,
    Error, SharedDictionary, StreamStatus,
};

const MAX_INPUT: usize = 2 * 1024 * 1024;
const MAX_FILES: usize = 65_536;
const MAX_DEPTH: usize = 64;

/// Official ZSTD_COMPRESSBOUND arithmetic, used only for the fuzz assertion.
fn compression_bound(size: usize) -> usize {
    size + (size >> 8)
        + if size < 128 * 1024 {
            (128 * 1024 - size) >> 11
        } else {
            0
        }
}

fn round_trip(
    source: &[u8],
    dictionary: SharedDictionary,
    prefix: bool,
    checksum: bool,
    capacity: usize,
) {
    let options = EncoderOptions {
        checksum,
        pledged_size: Some(source.len() as u64),
        ..EncoderOptions::default()
    };
    let mut encoder = Encoder::with_options(options).unwrap();
    if prefix {
        encoder.set_prefix(dictionary.clone()).unwrap();
    } else {
        encoder.set_dictionary(Some(dictionary.clone())).unwrap();
    }
    let mut consumed = 0;
    let mut encoded = Vec::new();
    let mut scratch = [0xa5; 4096 + 8];
    loop {
        scratch.fill(0xa5);
        let remaining = capacity - encoded.len();
        let available = remaining.min(4096);
        let progress = encoder
            .process(
                &source[consumed..],
                &mut scratch[..available],
                EndDirective::Finish,
            )
            .unwrap();
        assert!(progress.consumed <= source.len() - consumed);
        assert!(progress.produced <= available);
        assert!(
            scratch[progress.produced..]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        consumed += progress.consumed;
        encoded.extend_from_slice(&scratch[..progress.produced]);
        if progress.status == StreamStatus::FrameFinished {
            assert_eq!(consumed, source.len());
            break;
        }
        assert!(
            available != 0,
            "upstream compression-bound destination was exhausted"
        );
        assert!(progress.consumed != 0 || progress.produced != 0);
    }
    assert_eq!(encoder.total_input(), source.len() as u64);
    assert_eq!(encoder.total_output(), encoded.len() as u64);
    let mut decoder = Decoder::new(DecodeOptions::new(source.len()));
    if prefix {
        decoder.set_prefix(dictionary).unwrap();
    } else {
        decoder.set_dictionary(Some(dictionary)).unwrap();
    }
    let mut consumed = 0;
    let mut produced = 0;
    loop {
        scratch.fill(0xa5);
        let progress = decoder
            .process(&encoded[consumed..], &mut scratch[..4096])
            .unwrap();
        assert!(progress.consumed <= encoded.len() - consumed);
        assert!(progress.produced <= source.len() - produced);
        assert!(
            scratch[progress.produced..]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        assert_eq!(
            &scratch[..progress.produced],
            &source[produced..produced + progress.produced]
        );
        consumed += progress.consumed;
        produced += progress.produced;
        if progress.status == StreamStatus::FrameFinished {
            assert_eq!(consumed, encoded.len());
            assert_eq!(produced, source.len());
            decoder.finish().unwrap();
            break;
        }
        assert!(progress.consumed != 0 || progress.produced != 0);
    }
}

/// `dictionary_loader.c::LLVMFuzzerTestOneInput`: the same selected bytes are
/// both source and dictionary, including formatted metadata when it exists.
fn dictionary_loader(input: &[u8]) -> bool {
    assert!(input.len() <= MAX_INPUT);
    let mut producer = DataProducer::new(input);
    let prefix = producer.unsigned_range(0, 1) != 0;
    // Preserve this selector's consumption even though Rust always owns bytes.
    let _load_method = producer.unsigned_range(0, 1);
    let mode = match producer.unsigned_range(0, 2) {
        0 => DictionaryMode::Auto,
        1 => DictionaryMode::Raw,
        2 => DictionaryMode::Formatted,
        _ => unreachable!(),
    };
    let source = &input[..producer.remaining()];
    let dictionary = match Dictionary::from_bytes_with_mode(source, mode) {
        Ok(dictionary) => dictionary.into_shared(),
        Err(error) => {
            assert_ne!(
                mode,
                DictionaryMode::Raw,
                "raw dictionary loading must succeed"
            );
            assert_eq!(error, Error::InvalidDictionary);
            return false;
        }
    };
    round_trip(
        source,
        dictionary,
        prefix,
        true,
        compression_bound(source.len()),
    );
    true
}

/// `raw_dictionary_round_trip.c::LLVMFuzzerTestOneInput`: preserve all selectors
/// up to FUZZ_setRandomParameters. Later unsupported parameter choices do not
/// affect the original source, dictionary, prefix, or capacity selections.
fn raw_dictionary_round_trip(input: &[u8]) {
    assert!(input.len() <= MAX_INPUT);
    let mut producer = DataProducer::new(input);
    let payload = producer.reserve_prefix();
    let source_size = producer.unsigned_range(0, payload as u32) as usize;
    let one_byte_less = producer.unsigned_range(0, 1) as usize;
    let prefix = producer.unsigned_range(0, 1) != 0;
    let source = &input[..source_size];
    let dictionary = Dictionary::raw(&input[source_size..payload])
        .unwrap()
        .into_shared();
    round_trip(
        source,
        dictionary,
        prefix,
        false,
        compression_bound(source_size) - one_byte_less,
    );
}

fn loader_case(source: &[u8], mode: u8, load_method: u8, prefix: bool) -> Vec<u8> {
    let mut input = source.to_vec();
    input.extend_from_slice(&[mode, load_method, u8::from(prefix)]);
    input
}

/// Append reverse-order selectors, preserving the exact source/dictionary split.
fn raw_case(source: &[u8], dictionary: &[u8], prefix: bool, one_byte_less: bool) -> Vec<u8> {
    fn width(value: usize) -> usize {
        ((usize::BITS - value.leading_zeros()) as usize).div_ceil(8)
    }
    let payload = source.len() + dictionary.len();
    let mut parameters = vec![u8::from(prefix), u8::from(one_byte_less)];
    parameters.extend_from_slice(&(source.len() as u32).to_le_bytes()[..width(payload)]);
    for selector_bytes in 0..=4 {
        let total = payload + parameters.len() + selector_bytes;
        if width(total) != selector_bytes {
            continue;
        }
        let mut input = [source, dictionary].concat();
        input.extend_from_slice(&parameters);
        input.extend_from_slice(&(parameters.len() as u32).to_le_bytes()[..selector_bytes]);
        let mut producer = DataProducer::new(&input);
        assert_eq!(producer.reserve_prefix(), payload);
        assert_eq!(
            producer.unsigned_range(0, payload as u32),
            source.len() as u32
        );
        assert_eq!(producer.unsigned_range(0, 1), u32::from(one_byte_less));
        assert_eq!(producer.unsigned_range(0, 1), u32::from(prefix));
        return input;
    }
    panic!("no consistent raw dictionary fuzz selector width");
}

#[test]
fn dictionary_loader_covers_all_modes_prefixes_and_load_method_selectors() {
    let formatted = include_bytes!("upstream/dictionaries/formatted.dict");
    let mut invalid = formatted.to_vec();
    invalid[8] = 0;
    let inputs: &[&[u8]] = &[
        b"",
        b"a",
        b"abc",
        b"abcd",
        b"abababababababab",
        include_bytes!("upstream/dictionaries/boundary-dictionary.raw"),
        formatted,
        include_bytes!("upstream/dictionaries/trained.dict"),
        include_bytes!("upstream/dictionaries/duplicate-reps.dict"),
        &formatted[..7],
        &formatted[..8],
        &formatted[..32],
        &invalid,
    ];
    let mut successful = 0;
    let mut rejected = 0;
    for input in inputs {
        for mode in 0..=2 {
            for load_method in 0..=1 {
                for prefix in [false, true] {
                    if dictionary_loader(&loader_case(input, mode, load_method, prefix)) {
                        successful += 1;
                    } else {
                        rejected += 1;
                    }
                }
            }
        }
    }
    assert!(successful >= 80);
    assert!(rejected >= 16);
}

#[test]
fn raw_dictionary_round_trip_covers_empty_short_large_and_formatted_looking_content() {
    let raw = include_bytes!("upstream/dictionaries/boundary-dictionary.raw");
    let formatted = include_bytes!("upstream/dictionaries/formatted.dict");
    for dictionary in [
        b"".as_slice(),
        b"x",
        b"ab",
        b"xyz",
        &raw[..],
        &formatted[..],
    ] {
        for source in [
            b"".as_slice(),
            b"abababababababab",
            &raw[..1024],
            &raw.repeat(65),
        ] {
            for prefix in [false, true] {
                for smaller in [false, true] {
                    raw_dictionary_round_trip(&raw_case(source, dictionary, prefix, smaller));
                }
            }
        }
    }
}

#[test]
fn dictionary_fuzz_adapters_replay_deterministic_noise_and_truncated_dictionaries() {
    let mut state = 0x5eed_1234_u32;
    for length in 0..512 {
        let input: Vec<_> = (0..length)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        dictionary_loader(&input);
        raw_dictionary_round_trip(&input);
    }
    let bytes = include_bytes!("upstream/dictionaries/formatted.dict");
    for length in 0..150 {
        dictionary_loader(&loader_case(&bytes[..length], 0, 0, false));
        dictionary_loader(&loader_case(&bytes[..length], 2, 1, true));
    }
}

fn collect_files(roots: Vec<PathBuf>, maximum: usize) -> io::Result<Vec<PathBuf>> {
    let mut pending: Vec<_> = roots.into_iter().map(|path| (path, 0)).collect();
    let mut files = Vec::new();
    let mut visited = pending.len();
    if visited > 4 * maximum {
        return Err(io::Error::other("too many corpus roots"));
    }
    while let Some((path, depth)) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
            return Err(io::Error::other(format!(
                "non-regular corpus path: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            if depth >= MAX_DEPTH {
                return Err(io::Error::other("corpus directory depth exceeds limit"));
            }
            for entry in fs::read_dir(path)? {
                visited += 1;
                if visited > 4 * maximum {
                    return Err(io::Error::other("corpus traversal exceeds entry limit"));
                }
                pending.push((entry?.path(), depth + 1));
            }
        } else {
            if metadata.len() > MAX_INPUT as u64 {
                return Err(io::Error::other(format!(
                    "corpus input exceeds {MAX_INPUT} bytes: {}",
                    path.display()
                )));
            }
            files.push(path);
            if files.len() > maximum {
                return Err(io::Error::other("corpus file count exceeds limit"));
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

#[test]
#[ignore = "requires ZSTDOXIDE_DICTIONARY_CORPUS and ZSTDOXIDE_DICTIONARY_TARGET=loader|raw_round_trip; no files are skipped"]
fn external_dictionary_file_corpus() {
    let roots = std::env::var_os("ZSTDOXIDE_DICTIONARY_CORPUS")
        .expect("ZSTDOXIDE_DICTIONARY_CORPUS is required");
    let target = std::env::var("ZSTDOXIDE_DICTIONARY_TARGET")
        .expect("ZSTDOXIDE_DICTIONARY_TARGET is required");
    assert!(matches!(target.as_str(), "loader" | "raw_round_trip"));
    let maximum = std::env::var("ZSTDOXIDE_DICTIONARY_MAX_FILES").map_or(1024, |value| {
        value
            .parse::<usize>()
            .expect("file limit must be an integer")
    });
    assert!((1..=MAX_FILES).contains(&maximum));
    let files = collect_files(std::env::split_paths(&roots).collect(), maximum).unwrap();
    assert!(!files.is_empty());
    let mut loaded = 0;
    for path in &files {
        eprintln!("{target}: {}", path.display());
        let mut input = Vec::new();
        fs::File::open(path)
            .unwrap()
            .take((MAX_INPUT + 1) as u64)
            .read_to_end(&mut input)
            .unwrap();
        assert!(
            input.len() <= MAX_INPUT,
            "input grew beyond cap: {}",
            path.display()
        );
        if target == "loader" {
            loaded += usize::from(dictionary_loader(&input));
        } else {
            raw_dictionary_round_trip(&input);
            loaded += 1;
        }
    }
    eprintln!(
        "Replayed {} dictionary corpus files, {loaded} successful round trips, {} rejected formatted dictionaries; no files skipped",
        files.len(),
        files.len() - loaded
    );
}
