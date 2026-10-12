// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Derived assertions: facebook/zstd v1.5.7 tests/fuzz/simple_round_trip.c and
// tests/fuzzer.c. BSD-3-Clause option; see root LICENSE.zstd.
//! Supported numeric reference assertions from the original one-shot tests.
//!
//! The C selector capture predates these Rust tests. This port preserves input
//! selection, levels, determinism and exact decoded content. An owned return
//! vector does not model caller-buffer capacity failure, reusable CCtx/DCtx,
//! decoder setting mutation, the advanced parameter branch or in-place overlap.
#![forbid(unsafe_code)]

// Other targets use the same producer's additional methods and packet helper.
#[allow(dead_code)]
#[path = "upstream/fuzz_support.rs"]
mod fuzz_support;

use fuzz_support::DataProducer;
use zstdoxide::{
    CompressionParameterRequest, DecodeOptions, Error, FrameHeader, SourceSize, compress_at_level,
    compress_with_parameters, decompress_with_options,
};

const CAPTURE: &str = include_str!("upstream/reference-fast-fuzz/observations.tsv");
const HEADER: &str = "case\tfamily\tprefix_recipe_size\ttail_hex\tinput_sha256\tinput_size\tprefix_size\tremaining_after_prefix\tcompression_bound\tcapacity_reduction\tremaining_after_capacity\tbranch\tremaining_after_branch\tlevel\tremaining_after_level\tdecoder_toggle\tremaining_after_decoder";

#[derive(Debug)]
struct Case {
    name: String,
    family: &'static str,
    source_size: usize,
    tail: Vec<u8>,
    input: Vec<u8>,
    level: i32,
    branch: u32,
    reduction: u32,
    decoder: u32,
}

fn width(value: usize) -> usize {
    (usize::BITS - value.leading_zeros()).div_ceil(8) as usize
}

fn controlled(cases: &mut Vec<Case>, family: &'static str, size: usize, level: i32, branch: u32) {
    let index = cases.len();
    let reduction = (index & 1) as u32;
    let decoder = ((index >> 1) & 1) as u32;
    let reserved_width = (1..=4)
        .find(|&bytes| width(size + 4 + bytes) == bytes)
        .expect("bounded self-consistent reservation width");
    let mut tail = vec![
        decoder as u8,
        (level + 3) as u8,
        branch as u8,
        reduction as u8,
    ];
    tail.extend_from_slice(&4_u32.to_le_bytes()[..reserved_width]);
    let mut input: Vec<_> = (0..size).map(|i| (17 * i + 123) as u8).collect();
    input.extend_from_slice(&tail);
    cases.push(Case {
        name: format!("{family}-{index:02}"),
        family,
        source_size: size,
        tail,
        input,
        level,
        branch,
        reduction,
        decoder,
    });
}

fn cases() -> Vec<Case> {
    let mut result = Vec::new();
    for size in [
        0, 1, 30, 250, 251, 65_529, 65_530, 131_072, 131_073, 262_144, 262_145,
    ] {
        for level in [-3, -2, -1, 1, 2] {
            controlled(&mut result, "tier", size, level, 0);
        }
    }
    for level in [0].into_iter().chain(3..=19) {
        controlled(&mut result, "unsupported", 30, level, 0);
    }
    for raw in [&[][..], &[0], &[1], &[255]] {
        result.push(Case {
            name: format!("exhausted-{:02}", result.len()),
            family: "exhausted",
            source_size: 0,
            tail: raw.to_vec(),
            input: raw.to_vec(),
            level: -3,
            branch: 0,
            reduction: 0,
            decoder: 0,
        });
    }
    for size in [0, 30, 251, 65_530] {
        controlled(&mut result, "advanced", size, -3, 1);
    }
    assert_eq!(result.len(), 81);
    result
}

#[derive(Debug, Eq, PartialEq)]
struct SimpleSelectors {
    level: i32,
    remaining_after_level: usize,
    decoder: u32,
    remaining_after_decoder: usize,
}

#[derive(Debug, Eq, PartialEq)]
struct Selectors {
    prefix: usize,
    remaining_after_prefix: usize,
    reduction: u32,
    remaining_after_capacity: usize,
    branch: u32,
    remaining_after_branch: usize,
    simple: Option<SimpleSelectors>,
}

fn select(input: &[u8]) -> Selectors {
    let mut producer = DataProducer::new(input);
    let prefix = producer.reserve_prefix();
    let remaining_after_prefix = producer.remaining();
    let reduction = producer.unsigned_range(0, 1);
    let remaining_after_capacity = producer.remaining();
    let branch = producer.unsigned_range(0, 1);
    let remaining_after_branch = producer.remaining();
    // simple_round_trip.c:61-93. Advanced consumes a separate parameter schedule
    // and replays it after rollback; stop here instead of inventing its draws.
    let simple = if branch == 0 {
        let level = producer.signed_range(-3, 19);
        let remaining_after_level = producer.remaining();
        let decoder = producer.unsigned_range(0, 1);
        Some(SimpleSelectors {
            level,
            remaining_after_level,
            decoder,
            remaining_after_decoder: producer.remaining(),
        })
    } else {
        assert_eq!(producer.remaining(), remaining_after_branch);
        None
    };
    Selectors {
        prefix,
        remaining_after_prefix,
        reduction,
        remaining_after_capacity,
        branch,
        remaining_after_branch,
        simple,
    }
}

fn number<T: core::str::FromStr + core::fmt::Display>(value: &str) -> T
where
    T::Err: core::fmt::Debug,
{
    let result: T = value.parse().expect("captured integer");
    assert_eq!(result.to_string(), value, "canonical captured integer");
    result
}

/// Read only the immutable C observations. The offline importer independently
/// binds the complete TSV and input SHA-256; Cargo also reconstructs every packet
/// from the independently declared recipe, without a hash or JSON dependency.
fn observations(cases: &[Case]) -> Vec<(Selectors, usize)> {
    let mut lines = CAPTURE.lines();
    assert_eq!(lines.next(), Some(HEADER));
    let rows: Vec<_> = lines.collect();
    assert_eq!(rows.len(), cases.len());
    rows.iter()
        .zip(cases)
        .map(|(row, case)| {
            let fields: Vec<_> = row.split('\t').collect();
            assert_eq!(fields.len(), 17);
            assert_eq!(fields[0], case.name);
            assert_eq!(fields[1], case.family);
            assert_eq!(number::<usize>(fields[2]), case.source_size);
            assert_eq!(
                fields[3],
                case.tail
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
            assert_eq!(fields[4].len(), 64);
            assert!(
                fields[4]
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            );
            assert_eq!(number::<usize>(fields[5]), case.input.len());
            let simple = if fields[11] == "0" {
                Some(SimpleSelectors {
                    level: number(fields[13]),
                    remaining_after_level: number(fields[14]),
                    decoder: number(fields[15]),
                    remaining_after_decoder: number(fields[16]),
                })
            } else {
                assert_eq!(&fields[13..], &["-", "-", "-", "-"]);
                None
            };
            (
                Selectors {
                    prefix: number(fields[6]),
                    remaining_after_prefix: number(fields[7]),
                    reduction: number(fields[9]),
                    remaining_after_capacity: number(fields[10]),
                    branch: number(fields[11]),
                    remaining_after_branch: number(fields[12]),
                    simple,
                },
                number(fields[8]),
            )
        })
        .collect()
}

#[test]
fn exact_c_selectors_preserve_all_81_inputs_and_unconsumed_advanced_tails() {
    let cases = cases();
    let expected = observations(&cases);
    let mut advanced = 0;
    let mut decoder_toggles = [0; 2];
    for (case, (expected, _)) in cases.iter().zip(expected) {
        let actual = select(&case.input);
        assert_eq!(actual, expected, "{}", case.name);
        assert_eq!(actual.prefix, case.source_size);
        assert_eq!(actual.reduction, case.reduction);
        assert_eq!(actual.branch, case.branch);
        if let Some(simple) = actual.simple {
            assert_eq!(simple.level, case.level);
            assert_eq!(simple.decoder, case.decoder);
            decoder_toggles[simple.decoder as usize] += 1;
        } else {
            assert_eq!(actual.remaining_after_branch, 2);
            advanced += 1;
        }
    }
    assert_eq!(advanced, 4);
    assert_eq!(decoder_toggles.iter().sum::<usize>(), 77);
    assert!(decoder_toggles.iter().all(|&count| count > 0));
}

fn check_decode(frame: &[u8], source: &[u8], original_capacity: usize) {
    let output = decompress_with_options(
        frame,
        DecodeOptions {
            max_output_size: original_capacity,
            max_window_size: 1 << 20,
        },
    )
    .unwrap();
    assert_eq!(output, source);
    assert_eq!(output.len(), source.len());
    assert_eq!(
        FrameHeader::parse(frame).unwrap().0.content_size,
        Some(source.len() as u64)
    );
}

#[test]
fn original_simple_branch_successor_profiles_are_deterministic_and_fit_c_size_budgets() {
    let cases = cases();
    let expected = observations(&cases);
    let (mut admitted, mut unsupported, mut advanced) = (0, 0, 0);
    let mut selected_levels = std::collections::BTreeSet::new();
    for (case, (expected, bound)) in cases.iter().zip(expected) {
        let actual = select(&case.input);
        assert_eq!(actual, expected, "{}", case.name);
        let Some(simple) = actual.simple else {
            advanced += 1;
            continue;
        };
        let source = &case.input[..actual.prefix];
        let supported = successor_fuzz_status(&case.name, source.len(), simple.level) == "success";
        if !supported {
            assert_eq!(
                compress_at_level(source, simple.level),
                Err(Error::UnsupportedFeature),
                "{}",
                case.name
            );
            unsupported += 1;
            continue;
        }
        // :80-90 performs two simple compressions with the selected level. Full
        // byte equality is stronger than the original size and XXH64 comparison,
        // but these fresh one-shot calls do not establish reusable-context parity.
        let first = compress_at_level(source, simple.level).unwrap();
        let second = compress_at_level(source, simple.level).unwrap();
        assert_eq!(first, second, "{}", case.name);
        assert!(
            first.len() <= bound - actual.reduction as usize,
            "{}",
            case.name
        );
        // :93's exact toggle has been consumed and compared, but the public API
        // has no mutable DCtx maxBlockSize setting. Decode uses original N as
        // the result bound, as :145 did before source-prefix reservation.
        check_decode(&first, source, case.input.len());
        selected_levels.insert(simple.level);
        admitted += 1;
    }
    assert_eq!((admitted, unsupported, advanced), (62, 15, 4));
    assert_eq!(
        selected_levels,
        [-3, -2, -1, 0, 1, 2, 3, 4].into_iter().collect()
    );
}

#[test]
fn fuzzer_empty_level_loop_uses_the_frozen_successor_admission() {
    // fuzzer.c:1554-1566: exact -1..22 range. An empty slice replaces NULL input;
    // Vec output and detached requests do not port NULL destinations or CCtx reuse.
    assert_eq!(CompressionParameterRequest::MAX_LEVEL, 22);
    let (mut admitted, mut unsupported) = (0, 0);
    for level in -1..=22 {
        let request = CompressionParameterRequest::with_level(level);
        let direct = compress_at_level(&[], level);
        let requested = compress_with_parameters(&[], &request);
        if successor_loop_status("empty", 0, level) == "success" {
            let frame = direct.unwrap();
            assert_eq!(requested.unwrap(), frame);
            check_decode(&frame, &[], 0);
            admitted += 1;
        } else {
            assert_eq!(direct, Err(Error::UnsupportedFeature));
            assert_eq!(requested, Err(Error::UnsupportedFeature));
            unsupported += 1;
        }
    }
    assert_eq!((admitted, unsupported), (6, 18));
}

#[test]
fn fuzzer_30_byte_initial_match_preserves_level_range_and_supported_output_equalities() {
    // fuzzer.c:1637-1664, issue #1241: original bytes 0..19 followed by 0..9.
    // Preserve direct/requested equality for admitted levels. Caller-buffer -1,
    // forced streaming and reused CCtx portions remain outside this one-shot API.
    let source: Vec<u8> = (0..20).chain(0..10).collect();
    assert_eq!(source.len(), 30);
    let (mut admitted, mut unsupported) = (0, 0);
    for level in 1..=19 {
        let first = compress_at_level(&source, level);
        let request = CompressionParameterRequest::with_level(level);
        if successor_loop_status("initial_match", source.len(), level) == "success" {
            let first = first.unwrap();
            assert_eq!(compress_at_level(&source, level).unwrap(), first);
            assert_eq!(compress_with_parameters(&source, &request).unwrap(), first);
            check_decode(&first, &source, source.len());
            admitted += 1;
        } else {
            assert_eq!(first, Err(Error::UnsupportedFeature));
            assert_eq!(
                compress_with_parameters(&source, &request),
                Err(Error::UnsupportedFeature)
            );
            unsupported += 1;
        }
    }
    assert_eq!((admitted, unsupported), (4, 15));
}

// Version3 expectations are additive fixtures. Original C selector tuples and
// Module12/13 success/failure evidence retain their historical policy unchanged.
fn successor_fuzz_status(name: &str, size: usize, level: i32) -> &'static str {
    let mut rows = include_str!("fixtures/reference-admission-v3/fuzz.tsv")
        .lines()
        .skip(1)
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .filter(|row| row[0] == name);
    let row = rows.next().unwrap();
    assert!(rows.next().is_none());
    assert_eq!(row.len(), 9);
    assert_eq!(row[2].parse::<usize>().unwrap(), size);
    assert_eq!(row[3].parse::<i32>().unwrap(), level);
    row[5]
}

fn successor_loop_status(family: &str, size: usize, level: i32) -> &'static str {
    let mut rows = include_str!("fixtures/reference-admission-v3/loops.tsv")
        .lines()
        .skip(1)
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .filter(|row| row[0] == family && row[2].parse::<i32>().unwrap() == level);
    let row = rows.next().unwrap();
    assert!(rows.next().is_none());
    assert_eq!(row.len(), 8);
    assert_eq!(row[1].parse::<usize>().unwrap(), size);
    row[4]
}

#[test]
fn successor_policy_preserves_original_selectors_and_exact_double_fast_additions() {
    let cases = cases();
    let original: Vec<_> = CAPTURE.lines().skip(1).collect();
    let successor: Vec<_> = include_str!("fixtures/reference-admission-v2/fuzz.tsv")
        .lines()
        .skip(1)
        .collect();
    assert_eq!(original.len(), 81);
    assert_eq!(successor.len(), original.len());
    let mut additions = Vec::new();
    let mut historical_counts = [0; 3];
    let mut successor_counts = [0; 3];
    for ((case, original), successor) in cases.iter().zip(original).zip(successor) {
        let original: Vec<_> = original.split('\t').collect();
        let row: Vec<_> = successor.split('\t').collect();
        assert_eq!(row.len(), 9);
        assert_eq!(row[0], case.name);
        assert_eq!(row[0], original[0]);
        assert_eq!(row[1], original[4]); // Preserve the frozen input SHA256.
        assert_eq!(row[2], original[6]);
        assert_eq!(row[3], original[13]);
        for (status, counts) in [
            (row[4], &mut historical_counts),
            (row[5], &mut successor_counts),
        ] {
            counts[match status {
                "success" => 0,
                "unsupported" => 1,
                "advanced" => 2,
                _ => panic!("unexpected admission"),
            }] += 1;
        }
        let selectors = select(&case.input);
        let Some(simple) = selectors.simple else {
            assert_eq!(&row[4..], &["advanced", "advanced", "-", "-", "-"]);
            assert_eq!(selectors.remaining_after_branch, 2);
            continue;
        };
        // Preset strategy remains unchanged by source adjustment. The seven
        // adjusted scalars are not inferred from these pre-adjustment columns.
        let request = CompressionParameterRequest::with_level(simple.level);
        let resolved = request
            .resolve(SourceSize::Known(selectors.prefix as u64), 0)
            .unwrap();
        assert_eq!(resolved.to_raw().strategy, row[8].parse::<u32>().unwrap());
        if row[4] != row[5] {
            assert_eq!((row[4], row[5]), ("unsupported", "success"));
            assert_eq!(resolved.to_raw().strategy, 2);
            additions.push(case.name.as_str());
        }
    }
    assert_eq!(historical_counts, [57, 20, 4]);
    assert_eq!(successor_counts, [61, 16, 4]);
    assert_eq!(
        additions,
        ["tier-44", "tier-49", "unsupported-55", "unsupported-56"]
    );
}

#[test]
fn original_fuzzer_loops_add_only_default_three_double_fast_profiles() {
    let mut rows = include_str!("fixtures/reference-admission-v2/loops.tsv")
        .lines()
        .skip(1);
    let mut additions = Vec::new();
    let mut totals = [0; 2];
    for (family, size, levels) in [("empty", 0, -1..=22), ("initial_match", 30, 1..=19)] {
        for level in levels {
            let row: Vec<_> = rows.next().unwrap().split('\t').collect();
            assert_eq!(row.len(), 8);
            assert_eq!(row[0], family);
            assert_eq!(row[1].parse::<usize>().unwrap(), size);
            assert_eq!(row[2].parse::<i32>().unwrap(), level);
            let resolved = CompressionParameterRequest::with_level(level)
                .resolve(SourceSize::Known(size as u64), 0)
                .unwrap();
            assert_eq!(resolved.to_raw().strategy, row[7].parse::<u32>().unwrap());
            totals[0] += usize::from(row[3] == "success");
            totals[1] += usize::from(row[4] == "success");
            if row[3] != row[4] {
                assert_eq!((row[3], row[4]), ("unsupported", "success"));
                assert_eq!(resolved.to_raw().strategy, 2);
                additions.push((family, level));
            }
        }
    }
    assert!(rows.next().is_none());
    assert_eq!(totals, [5, 8]);
    assert_eq!(
        additions,
        [("empty", 0), ("empty", 3), ("initial_match", 3)]
    );
}
