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
use std::ops::Range;
use zstdoxide::{
    Dictionary, DictionaryMode, DictionaryTrainingOptions, Error, RawTrainingOptions,
    SharedDictionary, train_dictionary,
};

const MAX_INPUT: usize = 2 * 1024 * 1024;
const SAMPLE_COUNT: usize = 100;

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
