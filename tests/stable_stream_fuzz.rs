// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Adapted from facebook/zstd v1.5.7 tests/fuzz/stream_decompress.c:68-119.
// Used under the BSD-3-Clause option; see LICENSE.zstd.
//! Preserve the original stable selector and its conditional parameter consumption.
#![forbid(unsafe_code)]
use zstdoxide::{
    DecodeOptions, Decoder, Error, SliceDecoder, StreamStatus, decompress_with_options,
};
#[path = "upstream/fuzz_support.rs"]
#[allow(dead_code)]
mod fuzz_support;

#[derive(Debug)]
struct Outcome {
    stable: bool,
    source_size: usize,
    buffer_size: usize,
    first_output: usize,
    first_input: Option<usize>,
    calls: usize,
    written: u64,
    status: &'static str,
    digest: u64,
}
fn digest(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    hash
}
const INITIAL: u64 = 0xcbf29ce484222325;

fn replay(input: &[u8]) -> Outcome {
    assert!(input.len() <= 2 * 1024 * 1024);
    let mut producer = fuzz_support::DataProducer::new(input);
    let source_size = producer.reserve_prefix();
    let source = &input[..source_size];
    let buffer_size = (10 * source_size).max(128 * 1024);
    let stable = producer.unsigned_range(0, 10) == 5;
    let mut outcome = Outcome {
        stable,
        source_size,
        buffer_size,
        first_output: buffer_size,
        first_input: None,
        calls: 0,
        written: 0,
        status: "eof",
        digest: INITIAL,
    };
    let mut storage = vec![0xa5; buffer_size + 64];
    let mut source_position = 0;
    // Default upstream fuzzing frees the context per input. Stateful fuzzing is
    // separate. The fixed output gets the exact upstream capacity; the buffered
    // branch keeps the prior Rust replay's explicit 10 MiB aggregate limit.
    if stable {
        let options = DecodeOptions::new(buffer_size);
        let (prefix, rest) = storage.split_at_mut(32);
        let (output, suffix) = rest.split_at_mut(buffer_size);
        let mut decoder = SliceDecoder::new(output, options);
        'source: while source_position < source.len() {
            let remaining = source.len() - source_position;
            let selected = if producer.is_empty() {
                remaining
            } else {
                producer.unsigned_range(0, remaining as u32) as usize
            };
            outcome.first_input.get_or_insert(selected);
            let chunk = &source[source_position..source_position + selected];
            source_position += selected;
            let mut chunk_position = 0;
            loop {
                outcome.calls += 1;
                assert!(outcome.calls <= 3 * input.len() + 32);
                let before = decoder.total_output();
                let progress = match decoder.process(&chunk[chunk_position..]) {
                    Ok(p) => p,
                    Err(_) => {
                        outcome.status = "error";
                        assert_eq!(decoder.process(&[]), Err(Error::InvalidState));
                        assert_eq!(decoder.output(), Err(Error::InvalidState));
                        break 'source;
                    }
                };
                assert!(progress.consumed <= chunk.len() - chunk_position);
                assert_eq!(decoder.total_output() - before, progress.produced as u64);
                assert!(decoder.total_output() <= buffer_size as u64);
                assert!(matches!(
                    progress.status,
                    StreamStatus::NeedInput | StreamStatus::FrameFinished
                ));
                assert!(
                    progress.consumed > 0 || progress.produced > 0 || chunk_position == chunk.len()
                );
                chunk_position += progress.consumed;
                assert!(prefix.iter().chain(suffix.iter()).all(|&b| b == 0xa5));
                if decoder.total_output() == buffer_size as u64 {
                    outcome.status = "full";
                    break 'source;
                }
                if chunk_position == chunk.len() {
                    break;
                }
            }
        }
        outcome.written = decoder.total_output();
        if outcome.status == "eof" {
            if decoder.finish().is_ok() {
                outcome.status = "complete";
                let bytes = decoder.output().unwrap();
                outcome.digest = digest(INITIAL, bytes);
                assert_eq!(bytes, decompress_with_options(source, options).unwrap());
            } else {
                outcome.status = "truncated";
            }
        }
        drop(decoder);
    } else {
        let options = DecodeOptions::new(10 * 1024 * 1024);
        let mut decoder = Decoder::new(options);
        let mut size = if producer.is_empty() {
            buffer_size
        } else {
            producer.unsigned_range(0, buffer_size as u32) as usize
        };
        outcome.first_output = size;
        let mut position = 0;
        'source: while source_position < source.len() {
            let remaining = source.len() - source_position;
            let selected = if producer.is_empty() {
                remaining
            } else {
                producer.unsigned_range(0, remaining as u32) as usize
            };
            outcome.first_input.get_or_insert(selected);
            let chunk = &source[source_position..source_position + selected];
            source_position += selected;
            let mut chunk_position = 0;
            loop {
                outcome.calls += 1;
                assert!(outcome.calls <= 3 * input.len() + 1024);
                storage[32 + position..32 + buffer_size].fill(0xa5);
                let progress = match decoder.process(
                    &chunk[chunk_position..],
                    &mut storage[32 + position..32 + size],
                ) {
                    Ok(p) => p,
                    Err(_) => {
                        outcome.status = "error";
                        assert!(storage[32 + size..].iter().all(|&b| b == 0xa5));
                        assert_eq!(decoder.process(&[], &mut []), Err(Error::InvalidState));
                        break 'source;
                    }
                };
                assert!(progress.consumed <= chunk.len() - chunk_position);
                assert!(progress.produced <= size - position);
                assert_ne!(progress.status, StreamStatus::Flushed);
                assert!(
                    progress.consumed > 0
                        || progress.produced > 0
                        || chunk_position == chunk.len()
                        || position == size
                );
                outcome.digest = digest(
                    outcome.digest,
                    &storage[32 + position..32 + position + progress.produced],
                );
                chunk_position += progress.consumed;
                position += progress.produced;
                assert!(storage[32 + position..].iter().all(|&b| b == 0xa5));
                assert!(storage[..32].iter().all(|&b| b == 0xa5));
                if position == size {
                    position = 0;
                    size = if producer.is_empty() {
                        buffer_size
                    } else {
                        producer.unsigned_range(0, buffer_size as u32) as usize
                    };
                }
                if chunk_position == chunk.len() {
                    break;
                }
            }
        }
        outcome.written = decoder.total_output();
        if outcome.status == "eof" {
            if decoder.finish().is_ok() {
                outcome.status = "complete";
                let expected = decompress_with_options(source, options).unwrap();
                assert_eq!(expected.len() as u64, outcome.written);
                assert_eq!(digest(INITIAL, &expected), outcome.digest);
            } else {
                outcome.status = "truncated";
            }
        }
    }
    assert_eq!(&storage[..32], &[0xa5; 32]);
    assert_eq!(&storage[32 + buffer_size..], &[0xa5; 32]);
    outcome
}

fn with_tail(frame: &[u8], parameters: &[u8]) -> Vec<u8> {
    for width in 0..=4 {
        let length = frame.len() + parameters.len() + width;
        if ((u32::BITS - (length as u32).leading_zeros()) as usize).div_ceil(8) != width {
            continue;
        }
        let mut result = frame.to_vec();
        result.extend_from_slice(parameters);
        result.extend_from_slice(&(parameters.len() as u32).to_le_bytes()[..width]);
        let mut check = fuzz_support::DataProducer::new(&result);
        assert_eq!(check.reserve_prefix(), frame.len());
        return result;
    }
    panic!("inconsistent selector width");
}

#[test]
fn exact_selector_keeps_fixed_output_and_conditional_tail_positions() {
    let frame = include_bytes!("upstream/data/generated/http.zst");
    let source = include_bytes!("upstream/data/golden-compression/http");
    for stable in [false, true] {
        for fill in [0, 1, 7, 0xff] {
            let mut parameters = vec![fill; 32];
            parameters.push(if stable { 5 } else { 0 });
            let result = replay(&with_tail(frame, &parameters));
            assert_eq!(result.stable, stable);
            assert_eq!(result.source_size, frame.len());
            assert_eq!(result.buffer_size, 128 * 1024);
            assert_eq!(result.status, "complete");
            assert_eq!(result.written, source.len() as u64);
            assert_eq!(result.digest, digest(INITIAL, source));
        }
    }
}

#[test]
fn arbitrary_inputs_and_stable_truncations_preserve_guards_and_terminate() {
    for seed in 0_u32..1024 {
        let mut state = seed;
        let mut input = Vec::new();
        for _ in 0..seed as usize % 513 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            input.push((state >> 24) as u8);
        }
        replay(&input);
    }
    let frame = include_bytes!("upstream/decodecorpus/compressed-05.zst");
    for end in 0..frame.len() {
        replay(&with_tail(&frame[..end], &[0, 0, 0, 5]));
    }
}

#[test]
#[ignore = "requires the pinned stream_decompress corpus and a new output ledger path"]
fn external_exact_selector_streaming_corpus() {
    use std::io::Write;
    use std::path::PathBuf;
    let corpus =
        PathBuf::from(std::env::var_os("ZSTDOXIDE_STABLE_STREAM_CORPUS").expect("corpus path"));
    let ledger =
        PathBuf::from(std::env::var_os("ZSTDOXIDE_STABLE_STREAM_LEDGER").expect("ledger path"));
    let mut pending = vec![corpus.clone()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        if metadata.is_dir() {
            pending.extend(std::fs::read_dir(path).unwrap().map(|e| e.unwrap().path()));
        } else {
            assert!(metadata.is_file() && metadata.len() <= 2 * 1024 * 1024);
            files.push(path);
            assert!(files.len() <= 65536);
        }
    }
    files.sort();
    assert_eq!(files.len(), 12462, "entire pinned corpus required");
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(ledger)
        .unwrap();
    let mut stable_count = 0;
    let mut total_bytes = 0;
    for (index, path) in files.iter().enumerate() {
        let input = std::fs::read(path).unwrap();
        total_bytes += input.len();
        let r = replay(&input);
        stable_count += usize::from(r.stable);
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:016x}\t{}\t{}",
            path.strip_prefix(&corpus).unwrap().display(),
            u8::from(r.stable),
            r.source_size,
            r.buffer_size,
            r.calls,
            r.written,
            r.status,
            r.digest,
            r.first_output,
            r.first_input.unwrap_or(0)
        )
        .unwrap();
        if (index + 1) % 1000 == 0 {
            eprintln!(
                "exact stable-selector corpus: {} / {}",
                index + 1,
                files.len()
            );
        }
    }
    assert_eq!(total_bytes, 80339212);
    assert!(stable_count > 0 && stable_count < files.len());
    eprintln!(
        "replayed {} files / {total_bytes} bytes; {stable_count} selected stable; {} selected buffered; zero skipped",
        files.len(),
        files.len() - stable_count
    );
}
