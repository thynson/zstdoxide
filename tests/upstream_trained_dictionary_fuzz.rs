// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Derived from facebook/zstd v1.5.7 tests/fuzz/zstd_helpers.c and the three
// dictionary_{decompress,round_trip,stream_round_trip}.c fuzz targets.
// Used under the BSD-3-Clause option; see LICENSE.zstd.
//! Training-dependent dictionary fuzz contracts with exact source sampling.
//!
//! Sample selection preserves FUZZ_train's 100 tail-selected ranges and caps.
//! Dictionary construction uses the independent Rust trainer, rather than
//! upstream fastCover. Compression levels, randomized advanced parameters, and
//! external sequence producers remain explicit unsupported target branches.
#![forbid(unsafe_code)]

#[path = "upstream/fuzz_support.rs"]
#[allow(dead_code)] // The shared producer also serves the other upstream adapters.
mod fuzz_support;

use fuzz_support::DataProducer;
use std::fs;
use std::io::{self, Read};
use std::ops::Range;
use std::path::PathBuf;
use zstdoxide::{
    DecodeOptions, Decoder, Dictionary, DictionaryMode, DictionaryTrainingOptions, Encoder,
    EncoderOptions, EndDirective, Error, RawTrainingOptions, SharedDictionary, StreamStatus,
    train_dictionary,
};

const MAX_INPUT: usize = 2 * 1024 * 1024;
const SAMPLE_COUNT: usize = 100;
const MAX_FILES: usize = 65_536;
const MAX_DEPTH: usize = 64;

/// Shared source ranges are ordinary initialized value records, not pointers.
/// No dictionary retains these temporary source borrows after training.
struct TrainingSamples {
    ranges: [Range<usize>; SAMPLE_COUNT],
    dictionary_capacity: usize,
    sample_capacity: usize,
    selected_bytes: usize,
}

/// Corresponds to the sample selection in `zstd_helpers.c::FUZZ_train`.
fn select_training_samples(source_size: usize, producer: &mut DataProducer<'_>) -> TrainingSamples {
    assert!(source_size <= MAX_INPUT);
    let dictionary_capacity = (source_size / 8).max(1024);
    let sample_capacity = dictionary_capacity.checked_mul(11).unwrap();
    let mut selected_bytes = 0;
    let ranges = core::array::from_fn(|sample| {
        let remaining = sample_capacity.checked_sub(selected_bytes).unwrap();
        // A zero-width range consumes no bytes, as in the original producer.
        // Still call it for each of the 100 samples, including empty samples.
        let offset = producer.unsigned_range(0, source_size.max(1) as u32 - 1) as usize;
        let length = (source_size - offset).min(remaining / (SAMPLE_COUNT - sample));
        selected_bytes = selected_bytes.checked_add(length).unwrap();
        offset..offset.checked_add(length).unwrap()
    });
    TrainingSamples {
        ranges,
        dictionary_capacity,
        sample_capacity,
        selected_bytes,
    }
}

struct TrainedDictionary {
    dictionary: SharedDictionary,
    trained: bool,
}

/// Preserve the original empty-dictionary fallback only for TrainingFailed.
/// Allocation failures and unexpected implementation errors must fail the test.
fn train_selected(source: &[u8], selection: &TrainingSamples) -> TrainedDictionary {
    let samples: [_; SAMPLE_COUNT] =
        core::array::from_fn(|index| &source[selection.ranges[index].clone()]);
    assert_eq!(
        samples.iter().map(|sample| sample.len()).sum::<usize>(),
        selection.selected_bytes
    );
    let options = DictionaryTrainingOptions {
        selection: RawTrainingOptions {
            dictionary_size: selection.dictionary_capacity,
            max_sample_bytes: selection.sample_capacity,
            ..RawTrainingOptions::default()
        },
        ..DictionaryTrainingOptions::default()
    };
    match train_dictionary(&samples, options) {
        Ok(dictionary) => {
            assert!(dictionary.is_formatted());
            assert!(dictionary.as_bytes().len() <= selection.dictionary_capacity);
            TrainedDictionary {
                dictionary: dictionary.into_shared(),
                trained: true,
            }
        }
        Err(Error::TrainingFailed) => TrainedDictionary {
            dictionary: Dictionary::raw(&[]).unwrap().into_shared(),
            trained: false,
        },
        Err(error) => panic!("unexpected dictionary training error: {error:?}"),
    }
}

/// C context loading treats a zero-size dictionary as disabled even when the
/// requested content mode is formatted. Do not feed it to the formatted parser.
fn load_mode(dictionary: &SharedDictionary, mode: DictionaryMode) -> SharedDictionary {
    if dictionary.as_bytes().is_empty() {
        return Dictionary::raw(&[]).unwrap().into_shared();
    }
    Dictionary::from_bytes_with_mode(dictionary.as_bytes(), mode)
        .unwrap()
        .into_shared()
}

fn selector_case(length: usize, pre_training: u8) -> (Vec<u8>, usize, TrainingSamples, usize) {
    let input: Vec<_> = (0..length).map(|index| (index * 17 + 123) as u8).collect();
    let mut producer = DataProducer::new(&input);
    let source_size = producer.reserve_prefix();
    match pre_training {
        0 => {} // dictionary_decompress: no earlier selector.
        1 => {
            producer.unsigned_range(0, 1);
        } // round_trip: reduced bound.
        2 => {
            producer.unsigned_range(0, 2);
        } // stream: content mode.
        _ => unreachable!(),
    }
    let samples = select_training_samples(source_size, &mut producer);
    let remaining = producer.remaining();
    (input, source_size, samples, remaining)
}

#[test]
fn training_sample_ranges_match_pinned_c_vectors_and_producer_positions() {
    // Generated with v1.5.7's original FUZZ_train sample loop and separately
    // compiled fuzz_data_producer.c. The ignored oracle below compares every
    // range directly; these FNV-1a fingerprints retain normal-suite evidence.
    for (length, mode, source_size, capacity, selected, remaining, expected_hash) in [
        (0, 0, 0, 1024, 0, 0, 0xa947e50590de8025_u64),
        (1, 0, 0, 1024, 0, 0, 0xa947e50590de8025),
        (2, 0, 0, 1024, 0, 1, 0xa947e50590de8025),
        (17, 1, 3, 1024, 289, 0, 0x48d70056b81622c5),
        (255, 0, 165, 1024, 9746, 0, 0xeb462d8b0b84649b),
        (256, 1, 14, 1024, 726, 139, 0x8c307202f54e0661),
        (1024, 2, 447, 1024, 11118, 374, 0x1c4d89c9f19765cd),
        (8192, 0, 5544, 1024, 11264, 2446, 0xe0344ae12bffbf68),
        (65535, 1, 42677, 5334, 58674, 22655, 0x8106db78a54d6a1c),
        (65536, 2, 42783, 5347, 58817, 22549, 0x986c95f2c938a9f8),
    ] {
        let (_, actual_size, samples, actual_remaining) = selector_case(length, mode);
        assert_eq!((actual_size, actual_remaining), (source_size, remaining));
        assert_eq!(samples.dictionary_capacity, capacity);
        assert_eq!(samples.selected_bytes, selected);
        assert_eq!(samples.sample_capacity, capacity * 11);
        let mut hash = 0xcbf29ce484222325_u64;
        for range in &samples.ranges {
            assert!(range.start <= range.end && range.end <= source_size);
            for value in [range.start, range.len()] {
                for byte in (value as u64).to_le_bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                }
            }
        }
        assert_eq!(hash, expected_hash, "length {length}, mode {mode}");
    }
}

#[test]
fn training_samples_cover_empty_exhausted_and_full_capacity_sources() {
    for length in [0, 1, 7, 8, 111, 112, 113, 8191, 8192, MAX_INPUT] {
        let mut producer = DataProducer::new(&[]);
        let samples = select_training_samples(length, &mut producer);
        assert!(producer.is_empty());
        assert_eq!(samples.ranges.len(), 100);
        assert_eq!(
            samples.selected_bytes,
            (100 * length).min(samples.sample_capacity)
        );
        assert!(
            samples
                .ranges
                .iter()
                .all(|range| range.start == 0 && range.end <= length)
        );
        assert!(samples.selected_bytes <= samples.sample_capacity);
    }
    // A one-byte source has offset range 0..=0, so all selectors remain intact.
    let mut producer = DataProducer::new(&[1, 2, 3, 4]);
    select_training_samples(1, &mut producer);
    assert_eq!(producer.remaining(), 4);
}

#[test]
fn training_succeeds_or_uses_only_the_explicit_empty_fallback() {
    for source in [
        &b""[..],
        &b"a"[..],
        &b"1234567"[..],
        &b"12345678"[..],
        &b"the common repeated dictionary content"[..],
    ] {
        let selection = select_training_samples(source.len(), &mut DataProducer::new(&[]));
        let first = train_selected(source, &selection);
        let second = train_selected(source, &selection);
        assert_eq!(first.trained, source.len() >= 8);
        assert_eq!(first.dictionary.as_bytes(), second.dictionary.as_bytes());
        for mode in [
            DictionaryMode::Auto,
            DictionaryMode::Raw,
            DictionaryMode::Formatted,
        ] {
            let loaded = load_mode(&first.dictionary, mode);
            assert_eq!(loaded.as_bytes(), first.dictionary.as_bytes());
            if !first.trained {
                assert!(loaded.content().is_empty());
                assert!(!loaded.is_formatted());
            }
        }
    }
}

#[test]
#[ignore = "requires ZSTD_UPSTREAM pointing to pinned v1.5.7 sources and a C compiler"]
fn official_training_sample_loop_matches_every_selected_range() {
    use std::fmt::Write;
    use std::process::Command;
    let upstream = std::path::PathBuf::from(
        std::env::var_os("ZSTD_UPSTREAM").expect("ZSTD_UPSTREAM is required"),
    );
    let source = std::fs::read_to_string(upstream.join("tests/fuzz/zstd_helpers.c")).unwrap();
    let start = source
        .find("    for (sample = 0; sample < nbSamples; ++sample) {")
        .unwrap();
    let end = source[start..]
        .find("    memset(samples + pos, 0, totalSampleSize - pos);")
        .unwrap()
        + start;
    let original_loop = &source[start..end];
    let copy = "      memcpy(samples + pos, (const char*)src + offset, toCopy);";
    assert_eq!(original_loop.matches(copy).count(), 1);
    let observed_loop = original_loop.replace(
        copy,
        &format!("      printf(\"%zu %zu\\n\", offset, toCopy);\n{copy}"),
    );
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            // Remove only the directory exclusively created by this test.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = std::env::temp_dir().join(format!(
        "zstdoxide-training-samples-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let directory = Directory(directory);
    let source_file = directory.0.join("samples.c");
    let executable = directory.0.join("samples");
    let mut harness = String::from(
        r#"
#include "fuzz_helpers.h"
#include <string.h>
void* FUZZ_malloc(size_t size) { void* p=malloc(size ? size : 1); if(!p) abort(); return p; }
int main(int argc, char** argv) {
    size_t length, srcSize, dictSize, totalSampleSize, pos=0, sample=0;
    unsigned nbSamples=100; size_t samplesSizes[100];
    unsigned char *src; char *samples; FUZZ_dataProducer_t *producer;
    if(argc!=3) return 2;
    length=(size_t)strtoull(argv[1],NULL,10);
    src=FUZZ_malloc(length); for(size_t i=0;i<length;i++) src[i]=(unsigned char)(i*17+123);
    producer=FUZZ_dataProducer_create(src,length);
    srcSize=FUZZ_dataProducer_reserveDataPrefix(producer);
    if(atoi(argv[2])==1) FUZZ_dataProducer_uint32Range(producer,0,1);
    if(atoi(argv[2])==2) FUZZ_dataProducer_uint32Range(producer,0,2);
    dictSize=MAX(srcSize/8,1024); totalSampleSize=dictSize*11;
    samples=FUZZ_malloc(totalSampleSize);
    printf("%zu %zu %zu\n",srcSize,dictSize,totalSampleSize);
"#,
    );
    harness.push_str(&observed_loop);
    harness.push_str(
        r#"
    printf("%zu %zu\n",pos,FUZZ_dataProducer_remainingBytes(producer));
    free(samples); free(src); FUZZ_dataProducer_free(producer); return 0;
}
"#,
    );
    std::fs::write(&source_file, harness).unwrap();
    let built = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .arg("-I")
        .arg(upstream.join("lib"))
        .arg("-I")
        .arg(upstream.join("lib/common"))
        .arg("-I")
        .arg(upstream.join("tests/fuzz"))
        .arg(&source_file)
        .arg(upstream.join("tests/fuzz/fuzz_data_producer.c"))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let lengths = [
        0, 1, 2, 17, 255, 256, 1024, 8191, 8192, 65535, 65536, MAX_INPUT,
    ];
    for length in lengths {
        for mode in 0..=2 {
            let (_, size, samples, remaining) = selector_case(length, mode);
            let mut expected = format!(
                "{size} {} {}\n",
                samples.dictionary_capacity, samples.sample_capacity
            );
            for range in samples.ranges {
                writeln!(expected, "{} {}", range.start, range.len()).unwrap();
            }
            writeln!(expected, "{} {remaining}", samples.selected_bytes).unwrap();
            let result = Command::new(&executable)
                .arg(length.to_string())
                .arg(mode.to_string())
                .output()
                .unwrap();
            assert!(result.status.success());
            assert_eq!(
                result.stdout,
                expected.as_bytes(),
                "length {length}, mode {mode}"
            );
        }
    }
    eprintln!("Official FUZZ_train sample loop verified 36 cases and all 3,600 selected ranges");
}

/// Official ZSTD_COMPRESSBOUND arithmetic for the bounded destination contract.
fn compression_bound(size: usize) -> usize {
    size + (size >> 8)
        + if size < 128 * 1024 {
            (128 * 1024 - size) >> 11
        } else {
            0
        }
}

fn selected_mode(producer: &mut DataProducer<'_>) -> DictionaryMode {
    match producer.unsigned_range(0, 2) {
        0 => DictionaryMode::Auto,
        1 => DictionaryMode::Raw,
        2 => DictionaryMode::Formatted,
        _ => unreachable!(),
    }
}

#[derive(Default)]
struct Outcome {
    trained: bool,
    decoded: bool,
}

/// Assert the incremental equivalent of a bounded one-shot destination, with a
/// canary after every caller-owned output slice. Malformed bytes may fail, while
/// every reported success must consume all input and agree with cumulative totals.
fn decode_checked(
    source: &[u8],
    dictionary: SharedDictionary,
    prefix: bool,
    capacity: usize,
) -> zstdoxide::Result<Vec<u8>> {
    let mut decoder = Decoder::new(DecodeOptions::new(capacity));
    if prefix {
        decoder.set_prefix(dictionary).unwrap();
    } else {
        decoder.set_dictionary(Some(dictionary)).unwrap();
    }
    let mut consumed = 0;
    let mut output = Vec::new();
    let mut scratch = [0xa5; 4096 + 8];
    loop {
        scratch.fill(0xa5);
        let result = decoder.process(&source[consumed..], &mut scratch[..4096]);
        assert!(scratch[4096..].iter().all(|&byte| byte == 0xa5));
        let progress = match result {
            Ok(progress) => progress,
            Err(error) => {
                assert_ne!(error, Error::AllocationFailed);
                assert_ne!(error, Error::InvalidState);
                return Err(error);
            }
        };
        assert!(progress.consumed <= source.len() - consumed);
        assert!(progress.produced <= 4096);
        assert!(
            scratch[progress.produced..]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        consumed += progress.consumed;
        output.extend_from_slice(&scratch[..progress.produced]);
        assert!(output.len() <= capacity);
        assert_eq!(decoder.total_input(), consumed as u64);
        assert_eq!(decoder.total_output(), output.len() as u64);
        if consumed == source.len()
            && matches!(
                progress.status,
                StreamStatus::NeedInput | StreamStatus::FrameFinished
            )
        {
            decoder.finish()?;
            return Ok(output);
        }
        assert!(progress.consumed != 0 || progress.produced != 0);
    }
}

/// `dictionary_decompress.c::LLVMFuzzerTestOneInput`, retaining every selector
/// before decompression. A prepared dictionary and owned loaded dictionary share
/// the Rust handle implementation; by-reference loading becomes immutable sharing.
fn dictionary_decompress(input: &[u8]) -> Outcome {
    assert!(input.len() <= MAX_INPUT);
    let mut producer = DataProducer::new(input);
    let size = producer.reserve_prefix();
    let source = &input[..size];
    let samples = select_training_samples(size, &mut producer);
    let trained = train_selected(source, &samples);
    let (prefix, mode) = if producer.unsigned_range(0, 1) == 0 {
        (false, DictionaryMode::Auto)
    } else if producer.unsigned_range(0, 1) == 0 {
        let _load_method = producer.unsigned_range(0, 1);
        (false, selected_mode(&mut producer))
    } else {
        (true, selected_mode(&mut producer))
    };
    let capacity = producer.unsigned_range(0, (10 * size) as u32) as usize;
    let dictionary = load_mode(&trained.dictionary, mode);
    let decoded = decode_checked(source, dictionary, prefix, capacity).is_ok();
    Outcome {
        trained: trained.trained,
        decoded,
    }
}

fn encode_bounded(
    source: &[u8],
    dictionary: SharedDictionary,
    prefix: bool,
    capacity: usize,
) -> Vec<u8> {
    let options = EncoderOptions {
        checksum: false,
        pledged_size: Some(source.len() as u64),
        ..EncoderOptions::default()
    };
    let mut encoder = Encoder::with_options(options).unwrap();
    if prefix {
        encoder.set_prefix(dictionary).unwrap();
    } else {
        encoder.set_dictionary(Some(dictionary)).unwrap();
    }
    let mut consumed = 0;
    let mut output = Vec::new();
    let mut scratch = [0xa5; 4096 + 8];
    loop {
        scratch.fill(0xa5);
        let available = (capacity - output.len()).min(4096);
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
        output.extend_from_slice(&scratch[..progress.produced]);
        assert_eq!(encoder.total_input(), consumed as u64);
        assert_eq!(encoder.total_output(), output.len() as u64);
        if progress.status == StreamStatus::FrameFinished {
            assert_eq!(consumed, source.len());
            return output;
        }
        assert!(
            available != 0,
            "upstream reduced compression bound was exhausted"
        );
        assert!(progress.consumed != 0 || progress.produced != 0);
    }
}

/// Preserve source, reduced bound, training, prefix and content-mode selectors
/// from `dictionary_round_trip.c::roundTripTest`. The legacy level is consumed,
/// then default Rust compression is used. Advanced C parameter consumption and
/// load-method selectors following it are omitted: their offsets are not claimed
/// equivalent. Both Rust runs use identical options and must encode identically.
fn dictionary_round_trip(input: &[u8]) -> Outcome {
    assert!(input.len() <= MAX_INPUT);
    let mut producer = DataProducer::new(input);
    let size = producer.reserve_prefix();
    let source = &input[..size];
    let capacity = compression_bound(size) - producer.unsigned_range(0, 1) as usize;
    let samples = select_training_samples(size, &mut producer);
    let trained = train_selected(source, &samples);
    let prefix = producer.unsigned_range(0, 1) != 0;
    let mode = if producer.unsigned_range(0, 15) == 0 {
        // v1.5.7 zstd_helpers.c: kMinClevel=-3, kMaxClevel=19.
        let _unsupported_level = producer.signed_range(-3, 19);
        DictionaryMode::Auto
    } else {
        selected_mode(&mut producer)
    };
    let dictionary = load_mode(&trained.dictionary, mode);
    let first = encode_bounded(source, dictionary.clone(), prefix, capacity);
    let second = encode_bounded(source, dictionary.clone(), prefix, capacity);
    assert_eq!(first, second);
    let decoded = decode_checked(&first, dictionary, prefix, size).unwrap();
    assert_eq!(decoded, source);
    Outcome {
        trained: trained.trained,
        decoded: true,
    }
}

/// Append selectors in reverse call order. Each pair is (inclusive maximum,
/// requested value), with minimum zero; solve the reserve-prefix width boundary.
fn target_case(source: &[u8], selectors: &[(u32, u32)]) -> Vec<u8> {
    fn width(value: u32) -> usize {
        ((u32::BITS - value.leading_zeros()) as usize).div_ceil(8)
    }
    let mut tail = Vec::new();
    for &(maximum, value) in selectors.iter().rev() {
        assert!(value <= maximum);
        tail.extend_from_slice(&value.to_le_bytes()[..width(maximum)]);
    }
    for reserve_bytes in 0..=4 {
        let total = source.len() + tail.len() + reserve_bytes;
        if width(total as u32) != reserve_bytes {
            continue;
        }
        let mut input = source.to_vec();
        input.extend_from_slice(&tail);
        input.extend_from_slice(&(tail.len() as u32).to_le_bytes()[..reserve_bytes]);
        let mut producer = DataProducer::new(&input);
        assert_eq!(producer.reserve_prefix(), source.len());
        for &(maximum, value) in selectors {
            assert_eq!(producer.unsigned_range(0, maximum), value);
        }
        assert!(producer.is_empty());
        return input;
    }
    panic!("no consistent training fuzz reserve-prefix width");
}

fn sample_selectors(size: usize) -> Vec<(u32, u32)> {
    vec![(size.max(1) as u32 - 1, 0); SAMPLE_COUNT]
}

#[test]
fn trained_decompression_covers_prepared_loaded_prefix_modes_and_valid_controls() {
    // Independent raw-frame bytes: one final five-byte raw block containing hello.
    let frame = &[
        0x28, 0xb5, 0x2f, 0xfd, 0x20, 5, 0x29, 0, 0, b'h', b'e', b'l', b'l', b'o',
    ];
    for mode in 0..=2 {
        for prefix in [false, true] {
            for load_method in 0..=1 {
                let mut selectors = sample_selectors(frame.len());
                selectors.extend_from_slice(&[(1, 1), (1, u32::from(prefix))]);
                if !prefix {
                    selectors.push((1, load_method));
                }
                selectors.extend_from_slice(&[(2, mode), ((10 * frame.len()) as u32, 5)]);
                let outcome = dictionary_decompress(&target_case(frame, &selectors));
                assert!(outcome.trained && outcome.decoded);
                selectors.last_mut().unwrap().1 = 4;
                assert!(!dictionary_decompress(&target_case(frame, &selectors)).decoded);
            }
        }
    }
    let mut selectors = sample_selectors(frame.len());
    selectors.extend_from_slice(&[(1, 0), ((10 * frame.len()) as u32, 5)]);
    assert!(dictionary_decompress(&target_case(frame, &selectors)).decoded);
    let empty = &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0];
    let mut selectors = sample_selectors(empty.len());
    selectors.extend_from_slice(&[(1, 0), ((10 * empty.len()) as u32, 0)]);
    assert!(dictionary_decompress(&target_case(empty, &selectors)).decoded);
}

#[test]
fn trained_roundtrip_covers_modes_prefixes_reduced_bounds_and_training_fallback() {
    for size in [0, 1, 7, 8, 255, 256, 2048, 8192] {
        let source: Vec<_> = (0..size).map(|index| (index * 17 + 123) as u8).collect();
        for mode in 0..=2 {
            for prefix in [false, true] {
                for less in 0..=1 {
                    let mut selectors = vec![(1, less)];
                    selectors.extend(sample_selectors(size));
                    selectors.extend_from_slice(&[(1, u32::from(prefix)), (15, 1), (2, mode)]);
                    let outcome = dictionary_round_trip(&target_case(&source, &selectors));
                    assert_eq!(outcome.trained, size >= 8);
                    assert!(outcome.decoded);
                }
            }
        }
        let mut legacy = vec![(1, 1)];
        legacy.extend(sample_selectors(size));
        legacy.extend_from_slice(&[(1, 1), (15, 0), (22, 22)]);
        assert!(dictionary_round_trip(&target_case(&source, &legacy)).decoded);
    }
}

#[test]
fn trained_dictionary_targets_accept_deterministic_noise_and_truncated_frames() {
    let mut random = 0x70af_c531_u32;
    let noise: Vec<_> = (0..4096)
        .map(|_| {
            random ^= random << 13;
            random ^= random >> 17;
            random ^= random << 5;
            random as u8
        })
        .collect();
    for length in [0, 1, 2, 3, 4, 7, 8, 17, 31, 255, 256, 1024, 4096] {
        dictionary_decompress(&noise[..length]);
        dictionary_round_trip(&noise[..length]);
    }
    let frame = include_bytes!("upstream/dictionaries/formatted-id.zst");
    for length in 0..=frame.len() {
        let mut selectors = sample_selectors(length);
        selectors.extend_from_slice(&[(1, 0), ((10 * length) as u32, (10 * length) as u32)]);
        dictionary_decompress(&target_case(&frame[..length], &selectors));
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
#[ignore = "requires ZSTDOXIDE_DICTIONARY_CORPUS and ZSTDOXIDE_DICTIONARY_TARGET=decompress|round_trip|stream_round_trip; no files are skipped"]
fn external_trained_dictionary_file_corpus() {
    let roots = std::env::var_os("ZSTDOXIDE_DICTIONARY_CORPUS")
        .expect("ZSTDOXIDE_DICTIONARY_CORPUS is required");
    let target = std::env::var("ZSTDOXIDE_DICTIONARY_TARGET")
        .expect("ZSTDOXIDE_DICTIONARY_TARGET is required");
    assert!(matches!(
        target.as_str(),
        "decompress" | "round_trip" | "stream_round_trip"
    ));
    let maximum = std::env::var("ZSTDOXIDE_DICTIONARY_MAX_FILES").map_or(1024, |value| {
        value
            .parse::<usize>()
            .expect("file limit must be an integer")
    });
    assert!((1..=MAX_FILES).contains(&maximum));
    let files = collect_files(std::env::split_paths(&roots).collect(), maximum).unwrap();
    assert!(!files.is_empty());
    let mut trained = 0;
    let mut decoded = 0;
    let started = std::time::Instant::now();
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
        let outcome = match target.as_str() {
            "decompress" => dictionary_decompress(&input),
            "round_trip" => dictionary_round_trip(&input),
            "stream_round_trip" => dictionary_stream_round_trip(&input),
            _ => unreachable!(),
        };
        trained += usize::from(outcome.trained);
        decoded += usize::from(outcome.decoded);
    }
    eprintln!(
        "Replayed {} {target} corpus files: {trained} trained dictionaries, {} TrainingFailed empty fallbacks, {decoded} successful decodes, {} rejected compressed inputs, no files skipped; input cap {MAX_INPUT}, output cap 10*source for decompression, window cap 128 MiB; elapsed {:.3} s",
        files.len(),
        files.len() - trained,
        files.len() - decoded,
        started.elapsed().as_secs_f64()
    );
}

struct EncodedStream {
    bytes: Vec<u8>,
    frames: Vec<Range<usize>>,
    modes: u16,
}

/// Adapt the original makeInBuffer/makeOutBuffer and mode state machine after
/// the unsupported FUZZ_setRandomParameters call. Selector ordering within this
/// schedule is preserved; its initial tail position is explicitly adapted.
fn encode_streamed(
    source: &[u8],
    dictionary: SharedDictionary,
    prefix: bool,
    producer: &mut DataProducer<'_>,
) -> EncodedStream {
    let mut encoder = Encoder::with_options(EncoderOptions {
        checksum: false,
        ..EncoderOptions::default()
    })
    .unwrap();
    if prefix {
        encoder.set_prefix(dictionary).unwrap();
    } else {
        encoder.set_dictionary(Some(dictionary)).unwrap();
    }
    let capacity = compression_bound(source.len()).checked_mul(15).unwrap();
    let mut scratch = vec![0xa5; capacity + 8];
    let mut bytes = Vec::new();
    let mut frames = Vec::new();
    let mut modes = 0_u16;
    let mut frame_start = 0;
    let mut position = 0;
    let mut calls = 0;
    let call_limit = 32 * (source.len() + producer.remaining() + 1) + 1024;
    while position < source.len() {
        let length = producer.unsigned_range(1, (source.len() - position) as u32) as usize;
        let end = position + length;
        let mut mode = None;
        while position < end || mode.is_some() {
            calls += 1;
            assert!(calls <= call_limit, "stream fuzz progress budget exhausted");
            assert!(bytes.len() < capacity, "upstream stream bound exhausted");
            let output_size = producer.unsigned_range(1, (capacity - bytes.len()) as u32) as usize;
            let selected = *mode.get_or_insert_with(|| producer.unsigned_range(0, 9));
            modes |= 1 << selected;
            let directive = match selected {
                0..=2 => EndDirective::Flush,
                3 => EndDirective::Finish,
                _ => EndDirective::Continue,
            };
            if selected == 4 {
                let empty = encoder
                    .process(&[], &mut [], EndDirective::Continue)
                    .unwrap();
                assert_eq!((empty.consumed, empty.produced), (0, 0));
            }
            scratch[..output_size + 8].fill(0xa5);
            let progress = encoder
                .process(
                    &source[position..end],
                    &mut scratch[..output_size],
                    directive,
                )
                .unwrap();
            assert!(progress.consumed <= end - position);
            assert!(progress.produced <= output_size);
            assert!(
                scratch[progress.produced..output_size + 8]
                    .iter()
                    .all(|&byte| byte == 0xa5)
            );
            position += progress.consumed;
            bytes.extend_from_slice(&scratch[..progress.produced]);
            assert!(
                progress.consumed != 0
                    || progress.produced != 0
                    || matches!(
                        progress.status,
                        StreamStatus::Flushed | StreamStatus::FrameFinished
                    )
            );
            match directive {
                EndDirective::Continue => mode = None,
                EndDirective::Flush if progress.status == StreamStatus::Flushed => mode = None,
                EndDirective::Finish if progress.status == StreamStatus::FrameFinished => {
                    frames.push(frame_start..bytes.len());
                    frame_start = bytes.len();
                    encoder.reset();
                    // Preserve the branch selector, excluding the unsupported
                    // random-parameter function it would conditionally call.
                    let _change_parameters = producer.unsigned_range(0, 7) == 0;
                    mode = None;
                }
                _ => {}
            }
        }
    }
    loop {
        calls += 1;
        assert!(calls <= call_limit);
        assert!(bytes.len() < capacity);
        let output_size = producer.unsigned_range(1, (capacity - bytes.len()) as u32) as usize;
        scratch[..output_size + 8].fill(0xa5);
        let progress = encoder
            .process(&[], &mut scratch[..output_size], EndDirective::Finish)
            .unwrap();
        assert_eq!(progress.consumed, 0);
        assert!(progress.produced <= output_size);
        assert!(
            scratch[progress.produced..output_size + 8]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        bytes.extend_from_slice(&scratch[..progress.produced]);
        if progress.status == StreamStatus::FrameFinished {
            frames.push(frame_start..bytes.len());
            break;
        }
        assert!(progress.produced != 0);
    }
    assert_eq!(position, source.len());
    EncodedStream {
        bytes,
        frames,
        modes,
    }
}

/// Exact pre-training mode/sample/prefix selectors from
/// `dictionary_stream_round_trip.c`, followed by the adapted Rust schedule.
fn dictionary_stream_round_trip(input: &[u8]) -> Outcome {
    assert!(input.len() <= MAX_INPUT);
    let mut producer = DataProducer::new(input);
    let size = producer.reserve_prefix();
    let source = &input[..size];
    let mode = selected_mode(&mut producer);
    let samples = select_training_samples(size, &mut producer);
    let trained = train_selected(source, &samples);
    let prefix = producer.unsigned_range(0, 1) != 0;
    let dictionary = load_mode(&trained.dictionary, mode);
    let encoded = encode_streamed(source, dictionary.clone(), prefix, &mut producer);
    let decoded = decode_checked(&encoded.bytes, dictionary, prefix, size).unwrap();
    assert_eq!(decoded, source);
    Outcome {
        trained: trained.trained,
        decoded: true,
    }
}

fn decode_stream_fragments(
    encoded: &[u8],
    source: &[u8],
    dictionary: SharedDictionary,
    prefix: bool,
) {
    let mut decoder = Decoder::new(DecodeOptions::new(source.len()));
    if prefix {
        decoder.set_prefix(dictionary).unwrap();
    } else {
        decoder.set_dictionary(Some(dictionary)).unwrap();
    }
    let mut consumed = 0;
    let mut produced = 0;
    let mut scratch = [0xa5; 17 + 8];
    let mut calls = 0;
    loop {
        calls += 1;
        assert!(calls <= 32 * (encoded.len() + source.len() + 1) + 1024);
        let end = (consumed + 13).min(encoded.len());
        scratch.fill(0xa5);
        let progress = decoder
            .process(&encoded[consumed..end], &mut scratch[..17])
            .unwrap();
        assert!(progress.consumed <= end - consumed);
        assert!(progress.produced <= 17);
        assert!(
            scratch[progress.produced..]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        assert!(progress.produced <= source.len() - produced);
        assert_eq!(
            &scratch[..progress.produced],
            &source[produced..produced + progress.produced]
        );
        consumed += progress.consumed;
        produced += progress.produced;
        assert_eq!(decoder.total_input(), consumed as u64);
        assert_eq!(decoder.total_output(), produced as u64);
        if consumed == encoded.len()
            && matches!(
                progress.status,
                StreamStatus::FrameFinished | StreamStatus::NeedInput
            )
        {
            decoder.finish().unwrap();
            assert_eq!(produced, source.len());
            break;
        }
        assert!(progress.consumed != 0 || progress.produced != 0);
    }
}

#[test]
fn trained_stream_schedules_cover_every_mode_and_prefix_lifetime_across_reset() {
    let source: Vec<_> = (0..256).map(|index| (index * 17 + 123) as u8).collect();
    let samples = select_training_samples(source.len(), &mut DataProducer::new(&[]));
    let trained = train_selected(&source, &samples);
    assert!(trained.trained);
    let mut seen = 0;
    for prefix in [false, true] {
        for mode in 0..=9 {
            let selectors = vec![mode; 65536];
            let first = encode_streamed(
                &source,
                trained.dictionary.clone(),
                prefix,
                &mut DataProducer::new(&selectors),
            );
            let second = encode_streamed(
                &source,
                trained.dictionary.clone(),
                prefix,
                &mut DataProducer::new(&selectors),
            );
            assert_eq!(first.bytes, second.bytes);
            assert_eq!(first.frames, second.frames);
            assert_eq!(first.modes, second.modes);
            assert_ne!(first.modes & (1 << mode), 0);
            seen |= first.modes;
            if mode == 3 {
                assert!(first.frames.len() > 1);
            }
            for (index, frame) in first.frames.iter().enumerate() {
                let (header, _) =
                    zstdoxide::FrameHeader::parse(&first.bytes[frame.clone()]).unwrap();
                let expected = if prefix && index != 0 {
                    0
                } else {
                    trained.dictionary.id()
                };
                assert_eq!(header.dictionary_id, expected);
            }
            decode_stream_fragments(&first.bytes, &source, trained.dictionary.clone(), prefix);
        }
    }
    assert_eq!(seen, (1 << 10) - 1);
}

#[test]
fn trained_stream_target_covers_modes_prefixes_empty_sources_and_noise() {
    for size in [0, 1, 7, 8, 255, 256, 2048] {
        let source: Vec<_> = (0..size).map(|index| (index * 17 + 123) as u8).collect();
        for mode in 0..=2 {
            for prefix in [false, true] {
                let mut selectors = vec![(2, mode)];
                selectors.extend(sample_selectors(size));
                selectors.push((1, u32::from(prefix)));
                let outcome = dictionary_stream_round_trip(&target_case(&source, &selectors));
                assert_eq!(outcome.trained, size >= 8);
                assert!(outcome.decoded);
            }
        }
    }
    let mut state = 0x417b_8351_u32;
    let noise: Vec<_> = (0..8192)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    for length in [0, 1, 3, 8, 31, 255, 256, 1024, 4096, 8192] {
        assert!(dictionary_stream_round_trip(&noise[..length]).decoded);
    }
}
