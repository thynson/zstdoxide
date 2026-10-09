// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Selector contracts derive from facebook/zstd v1.5.7
// tests/fuzz/huf_round_trip.c:43-61. BSD option; see LICENSE.zstd.
//! Adapted complete Huffman fuzz control flow with exact upstream parameters.
//!
//! `HuffmanParameters` contains plain scalar values and an immutable source
//! prefix borrowed for the input's lifetime. `DataProducer` replaces the C
//! non-owning cursor with checked indices. Generated test inputs own `Vec`s.
//! These metadata records require no artificial unsafe backend. The flags are
//! observations of upstream selectors, not invented Rust codec controls.
//!
//! Source extents, cheap depths, and stream-capacity rules match the original
//! target. Independent Rust trees and estimated-size search need not select C's
//! exact table. Copying a selected description uses Rust's exact-fit contract,
//! not C's destination-dependent raw/FSE serialization choice. X1/X2 and CPU
//! selectors remain recorded C engine choices; Rust has one checked decoder.
//! Independent C selector, table, stream, and capacity tests cover those
//! boundaries without inventing Rust implementations for each C engine.
#![forbid(unsafe_code)]

use crate::entropy_output::{EntropyOutput, EntropyStreams, write_description};
use crate::huffman::{HuffmanTable, decode_streams};
use crate::huffman_encode::{StandaloneDepthPolicy, build_standalone_table};
use crate::upstream_entropy_tests::fuzz_support::DataProducer;
use crate::{Error, Result};
use alloc::vec::Vec;

const INPUT_LIMIT: usize = 1 << 20;
const SOURCE_LIMIT: usize = 256 << 10;

#[derive(Debug, PartialEq, Eq)]
struct HuffmanParameters<'a> {
    four_streams: bool,
    decode_two_symbols: bool,
    bmi2: bool,
    optimal_depth: bool,
    prefer_repeat: bool,
    suspect_uncompressible: bool,
    disable_assembly: bool,
    disable_fast: bool,
    output_capacity: u32,
    requested_table_log: u8,
    remaining_before_cap: usize,
    source: &'a [u8],
}

/// Read exactly the original target's parameters before any histogram or
/// early exit. The explicit test-harness input bound keeps `4 * original_len`
/// within the upstream producer's u32 range. Unavailable BMI does not consume
/// a selector byte; both layouts can therefore be tested on every Rust host.
fn select_parameters(input: &[u8], bmi2_available: bool) -> Result<HuffmanParameters<'_>> {
    if input.len() > INPUT_LIMIT {
        return Err(Error::ResourceLimit);
    }
    let mut producer = DataProducer::new(input);
    let four_streams = producer.signed_range(0, 1) != 0;
    let decode_two_symbols = producer.signed_range(0, 1) != 0;
    let bmi2 = bmi2_available && producer.signed_range(0, 1) != 0;
    let optimal_depth = producer.signed_range(0, 1) != 0;
    let prefer_repeat = producer.signed_range(0, 1) != 0;
    let suspect_uncompressible = producer.signed_range(0, 1) != 0;
    let disable_assembly = producer.signed_range(0, 1) != 0;
    let disable_fast = producer.signed_range(0, 1) != 0;
    let output_capacity = producer.unsigned_range(0, 4 * input.len() as u32);
    let requested_table_log = producer.unsigned_range(1, 12) as u8;
    let remaining_before_cap = producer.remaining();
    Ok(HuffmanParameters {
        four_streams,
        decode_two_symbols,
        bmi2,
        optimal_depth,
        prefer_repeat,
        suspect_uncompressible,
        disable_assembly,
        disable_fast,
        output_capacity,
        requested_table_log,
        remaining_before_cap,
        source: &input[..remaining_before_cap.min(SOURCE_LIMIT)],
    })
}

impl HuffmanParameters<'_> {
    /// Wire order of the independent C selector oracle, not a codec API.
    fn fields(&self) -> [u32; 12] {
        [
            u32::from(self.four_streams),
            u32::from(self.decode_two_symbols),
            u32::from(self.bmi2),
            u32::from(self.optimal_depth),
            u32::from(self.prefer_repeat),
            u32::from(self.suspect_uncompressible),
            u32::from(self.disable_assembly),
            u32::from(self.disable_fast),
            self.output_capacity,
            u32::from(self.requested_table_log),
            self.remaining_before_cap as u32,
            self.source.len() as u32,
        ]
    }
}

/// Each original early return is an observed outcome, never a skipped input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Empty,
    Single,
    Uniform,
    TableUnavailable,
    DescriptionTooSmall,
    StreamZero,
    RoundTrip,
}

/// Owned scalar evidence; no source borrow or scratch allocation escapes a call.
#[derive(Debug, Eq, PartialEq)]
struct Trial {
    outcome: Outcome,
    original_size: usize,
    parameters: [u32; 12],
    input_fingerprint: u64,
    source_fingerprint: u64,
    highest_symbol: Option<u8>,
    distinct_symbols: u16,
    adjusted_depth: u8,
    selected_cap: u8,
    actual_depth: u8,
    description_size: usize,
    description_fingerprint: u64,
    encoded_size: usize,
    encoded_fingerprint: u64,
}

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    })
}

/// The fuzz driver's conservative limit uses the highest byte value, including
/// holes, rather than the number of present symbols: ceil(log2(max + 1)) + 1.
fn adjusted_depth(requested: u8, highest_symbol: u8) -> u8 {
    let alphabet = u32::from(highest_symbol) + 1;
    requested.max((u32::BITS - (alphabet - 1).leading_zeros() + 1) as u8)
}

/// Exact scalar path of v1.5.7 HUF_optimalTableLog without optimalDepth:
/// FSE_optimalTableLog_internal(adjusted, source_size, highest_symbol, 1).
/// Nonuniform sources guarantee size >= 2 and highest symbol >= 1. The source
/// cap makes every u32 conversion exact. C's size-two subtraction wraps; using
/// saturation would incorrectly change a requested depth twelve to five.
fn cheap_depth(adjusted: u8, source_size: usize, highest_symbol: u8) -> u8 {
    assert!((2..=SOURCE_LIMIT).contains(&source_size));
    assert!(highest_symbol != 0);
    let size = source_size as u32;
    let source_maximum = (31 - (size - 1).leading_zeros()).wrapping_sub(1);
    let minimum = (32 - size.leading_zeros()).min(33 - u32::from(highest_symbol).leading_zeros());
    u32::from(adjusted)
        .min(source_maximum)
        .max(minimum)
        .clamp(5, 12) as u8
}

fn untouched(guarded: &[u8], written: usize, previous_description: &[u8]) {
    assert_eq!(guarded[0], 0xa5);
    assert_eq!(guarded.last(), Some(&0xa5));
    for (position, &byte) in guarded[1..guarded.len() - 1]
        .iter()
        .enumerate()
        .skip(written)
    {
        assert_eq!(
            byte,
            previous_description.get(position).copied().unwrap_or(0xa5)
        );
    }
}

/// Preserve the target's source prefix and reuse the same selected destination
/// for its table and data stream. Every nonzero stream is decoded through the
/// written description; an unexpected codec/allocation error fails the case.
fn round_trip(input: &[u8], bmi: bool, scratch: &mut EntropyOutput) -> Result<Trial> {
    let selected = select_parameters(input, bmi)?;
    let mut trial = Trial {
        outcome: Outcome::Empty,
        original_size: input.len(),
        parameters: selected.fields(),
        input_fingerprint: fingerprint(input),
        source_fingerprint: fingerprint(selected.source),
        highest_symbol: None,
        distinct_symbols: 0,
        adjusted_depth: 0,
        selected_cap: 0,
        actual_depth: 0,
        description_size: 0,
        description_fingerprint: 0,
        encoded_size: 0,
        encoded_fingerprint: 0,
    };
    if selected.source.is_empty() {
        return Ok(trial);
    }
    if selected.source.len() == 1 {
        trial.outcome = Outcome::Single;
        return Ok(trial);
    }
    let mut counts = [0_u64; 256];
    for &symbol in selected.source {
        counts[usize::from(symbol)] += 1;
    }
    let highest = counts.iter().rposition(|&count| count != 0).unwrap() as u8;
    trial.highest_symbol = Some(highest);
    trial.distinct_symbols = counts.iter().filter(|&&count| count != 0).count() as u16;
    if trial.distinct_symbols == 1 {
        trial.outcome = Outcome::Uniform;
        return Ok(trial);
    }
    trial.adjusted_depth = adjusted_depth(selected.requested_table_log, highest);
    let policy = if selected.optimal_depth {
        // This is the original adjusted hard cap, not a claimed C optimal log.
        trial.selected_cap = trial.adjusted_depth;
        StandaloneDepthPolicy::SmallestEstimatedSize
    } else {
        trial.selected_cap = cheap_depth(trial.adjusted_depth, selected.source.len(), highest);
        StandaloneDepthPolicy::Requested
    };
    let Some(built) = build_standalone_table(&counts, trial.selected_cap, policy)? else {
        trial.outcome = Outcome::TableUnavailable;
        return Ok(trial);
    };
    trial.actual_depth = built.codebook().table_log();
    trial.description_size = built.description().len();
    trial.description_fingerprint = fingerprint(built.description());
    assert!(trial.actual_depth <= trial.selected_cap);
    let capacity = usize::try_from(selected.output_capacity).map_err(|_| Error::ResourceLimit)?;
    let mut guarded = Vec::new();
    guarded
        .try_reserve_exact(capacity + 2)
        .map_err(|_| Error::AllocationFailed)?;
    guarded.resize(capacity + 2, 0xa5);
    if capacity < built.description().len() {
        assert_eq!(
            write_description(&built, &mut guarded[1..1 + capacity]),
            Err(Error::ResourceLimit)
        );
        untouched(&guarded, 0, &[]);
        trial.outcome = Outcome::DescriptionTooSmall;
        return Ok(trial);
    }
    let written = write_description(&built, &mut guarded[1..1 + capacity])?;
    assert_eq!(&guarded[1..1 + written], built.description());
    untouched(&guarded, written, &[]);
    let (table, consumed) = HuffmanTable::read_dictionary(&guarded[1..1 + written])?;
    assert_eq!(consumed, written);
    let streams = if selected.four_streams {
        EntropyStreams::Four
    } else {
        EntropyStreams::One
    };
    let result = scratch.encode(
        built.codebook(),
        selected.source,
        &mut guarded[1..1 + capacity],
        streams,
    )?;
    let Some(encoded_size) = result else {
        untouched(&guarded, 0, built.description());
        trial.outcome = Outcome::StreamZero;
        return Ok(trial);
    };
    untouched(&guarded, encoded_size, built.description());
    let encoded = &guarded[1..1 + encoded_size];
    let decoded = decode_streams(
        &table,
        encoded,
        selected.source.len(),
        selected.four_streams,
    )?;
    assert_eq!(decoded, selected.source);
    trial.encoded_size = encoded_size;
    trial.encoded_fingerprint = fingerprint(encoded);
    trial.outcome = Outcome::RoundTrip;
    Ok(trial)
}

fn pattern(length: usize) -> Vec<u8> {
    (0..length).map(|index| (index * 17 + 123) as u8).collect()
}

// Generated by scripts/oracles/huffman_parameters.c linked to the unchanged
// pinned v1.5.7 fuzz_data_producer.c and fuzz_helpers.c; no Rust selector was
// involved in producing these expected fields. Inputs are (17*i + 123) mod256.
// huffman_parameters.c SHA-256: 327ba39d85523f931a40e0735fb8c0104127e49903c9942e8ae46e24507360da
// fuzz_data_producer.c SHA-256: 9d5b2085b1d56b982659ac931f0c7ce0c154decb0e93b0be4819063fbc370cf6
// fuzz_helpers.c SHA-256: 9425ffd8b57508382442b54cbc5b2c20719952cbd3cbc85d2414d56d096b6055
// huf.h SHA-256: 9a83d899c8c9bf03389d482562090ec2443bc0f87f27864a2b891b180333f2d4
const C_VECTORS: &[(usize, bool, [u32; 12])] = &[
    (0, false, [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]),
    (0, true, [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]),
    (1, false, [1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]),
    (1, true, [1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]),
    (2, false, [0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]),
    (2, true, [0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]),
    (3, false, [1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0]),
    (3, true, [1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 0, 0]),
    (4, false, [0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0]),
    (4, true, [0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0]),
    (5, false, [1, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0]),
    (5, true, [1, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0]),
    (6, false, [0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0]),
    (6, true, [0, 1, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0]),
    (7, false, [1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0]),
    (7, true, [1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0]),
    (8, false, [0, 1, 0, 0, 1, 0, 1, 0, 24, 1, 0, 0]),
    (8, true, [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0]),
    (9, false, [1, 0, 0, 1, 0, 1, 0, 1, 29, 4, 0, 0]),
    (9, true, [1, 0, 1, 0, 1, 0, 1, 0, 12, 1, 0, 0]),
    (10, false, [0, 1, 0, 0, 1, 0, 1, 0, 34, 9, 1, 1]),
    (10, true, [0, 1, 0, 1, 0, 1, 0, 1, 17, 4, 0, 0]),
    (11, false, [1, 0, 0, 1, 0, 1, 0, 1, 39, 2, 2, 2]),
    (11, true, [1, 0, 1, 0, 1, 0, 1, 0, 22, 9, 1, 1]),
    (12, false, [0, 1, 0, 0, 1, 0, 1, 0, 44, 7, 3, 3]),
    (12, true, [0, 1, 0, 1, 0, 1, 0, 1, 27, 2, 2, 2]),
    (13, false, [1, 0, 0, 1, 0, 1, 0, 1, 49, 12, 4, 4]),
    (13, true, [1, 0, 1, 0, 1, 0, 1, 0, 32, 7, 3, 3]),
    (16, false, [0, 1, 0, 0, 1, 0, 1, 0, 3, 3, 7, 7]),
    (16, true, [0, 1, 0, 1, 0, 1, 0, 1, 47, 10, 6, 6]),
    (31, false, [1, 0, 0, 1, 0, 1, 0, 1, 2, 2, 22, 22]),
    (31, true, [1, 0, 1, 0, 1, 0, 1, 0, 116, 9, 21, 21]),
    (32, false, [0, 1, 0, 0, 1, 0, 1, 0, 19, 3, 23, 23]),
    (32, true, [0, 1, 0, 1, 0, 1, 0, 1, 2, 2, 22, 22]),
    (63, false, [1, 0, 0, 1, 0, 1, 0, 1, 34, 6, 54, 54]),
    (63, true, [1, 0, 1, 0, 1, 0, 1, 0, 17, 1, 53, 53]),
    (64, false, [0, 1, 0, 0, 1, 0, 1, 0, 240, 6, 54, 54]),
    (64, true, [0, 1, 0, 1, 0, 1, 0, 1, 240, 1, 53, 53]),
    (65, false, [1, 0, 0, 1, 0, 1, 0, 1, 233, 11, 55, 55]),
    (65, true, [1, 0, 1, 0, 1, 0, 1, 0, 40, 6, 54, 54]),
    (255, false, [1, 0, 0, 1, 0, 1, 0, 1, 889, 1, 245, 245]),
    (255, true, [1, 0, 1, 0, 1, 0, 1, 0, 604, 8, 244, 244]),
    (256, false, [0, 1, 0, 0, 1, 0, 1, 0, 934, 6, 246, 246]),
    (256, true, [0, 1, 0, 1, 0, 1, 0, 1, 665, 1, 245, 245]),
    (
        16383,
        false,
        [1, 0, 0, 1, 0, 1, 0, 1, 58065, 1, 16373, 16373],
    ),
    (
        16383,
        true,
        [1, 0, 1, 0, 1, 0, 1, 0, 53696, 8, 16372, 16372],
    ),
    (
        16384,
        false,
        [0, 1, 0, 0, 1, 0, 1, 0, 57822, 1, 16373, 16373],
    ),
    (
        16384,
        true,
        [0, 1, 0, 1, 0, 1, 0, 1, 53470, 8, 16372, 16372],
    ),
    (
        16385,
        false,
        [1, 0, 0, 1, 0, 1, 0, 1, 62414, 6, 16374, 16374],
    ),
    (
        16385,
        true,
        [1, 0, 1, 0, 1, 0, 1, 0, 56850, 1, 16373, 16373],
    ),
    (
        65535,
        false,
        [1, 0, 0, 1, 0, 1, 0, 1, 184936, 8, 65524, 65524],
    ),
    (
        65535,
        true,
        [1, 0, 1, 0, 1, 0, 1, 0, 115019, 3, 65523, 65523],
    ),
    (
        65536,
        false,
        [0, 1, 0, 0, 1, 0, 1, 0, 254613, 1, 65525, 65525],
    ),
    (
        65536,
        true,
        [0, 1, 0, 1, 0, 1, 0, 1, 184712, 8, 65524, 65524],
    ),
    (
        262143,
        false,
        [1, 0, 0, 1, 0, 1, 0, 1, 184810, 8, 262132, 262132],
    ),
    (
        262143,
        true,
        [1, 0, 1, 0, 1, 0, 1, 0, 114902, 3, 262131, 262131],
    ),
    (
        262144,
        false,
        [0, 1, 0, 0, 1, 0, 1, 0, 254658, 1, 262133, 262133],
    ),
    (
        262144,
        true,
        [0, 1, 0, 1, 0, 1, 0, 1, 184754, 8, 262132, 262132],
    ),
    (
        262145,
        false,
        [1, 0, 0, 1, 0, 1, 0, 1, 324578, 6, 262134, 262134],
    ),
    (
        262145,
        true,
        [1, 0, 1, 0, 1, 0, 1, 0, 254598, 1, 262133, 262133],
    ),
    (
        262155,
        false,
        [1, 0, 0, 1, 0, 1, 0, 1, 957386, 4, 262144, 262144],
    ),
    (
        262155,
        true,
        [1, 0, 1, 0, 1, 0, 1, 0, 887526, 11, 262143, 262143],
    ),
    (
        262156,
        false,
        [0, 1, 0, 0, 1, 0, 1, 0, 1027202, 9, 262145, 262144],
    ),
    (
        262156,
        true,
        [0, 1, 0, 1, 0, 1, 0, 1, 957346, 4, 262144, 262144],
    ),
    (
        1048576,
        false,
        [0, 1, 0, 0, 1, 0, 1, 0, 3400398, 1, 1048565, 262144],
    ),
    (
        1048576,
        true,
        [0, 1, 0, 1, 0, 1, 0, 1, 2281917, 8, 1048564, 262144],
    ),
];

#[test]
fn huffman_selectors_match_independent_c_vectors_and_source_ranges() {
    for &(length, bmi, expected) in C_VECTORS {
        let input = pattern(length);
        let selected = select_parameters(&input, bmi).unwrap();
        assert_eq!(selected.fields(), expected, "length={length}, BMI={bmi}");
        assert_eq!(selected.source, &input[..expected[11] as usize]);
        assert!(core::ptr::eq(
            selected.source,
            &input[..selected.source.len()]
        ));
    }
}

/// Construct selectors backwards only for the exhaustive flag/depth checks.
/// The fixed vectors above are generated from unrelated full-input patterns.
fn explicit_case(source: &[u8], bmi: bool, mask: u8, requested_log: u8) -> (Vec<u8>, [u32; 12]) {
    assert!((1..=12).contains(&requested_log));
    let flag_count = if bmi { 8 } else { 7 };
    let mut widths = (1..=4).filter(|&width| {
        let total = source.len() + flag_count + width + 1;
        (u32::BITS - (4 * total as u32).leading_zeros()).div_ceil(8) as usize == width
    });
    let width = widths.next().unwrap();
    assert!(widths.next().is_none());
    let total = source.len() + flag_count + width + 1;
    let capacity = match mask % 3 {
        0 => 0,
        1 => total as u32,
        _ => 4 * total as u32,
    };
    let mut input = source.to_vec();
    input.push(requested_log - 1);
    input.extend_from_slice(&capacity.to_le_bytes()[..width]);
    let flags: Vec<_> = (0..flag_count).map(|bit| (mask >> bit) & 1).collect();
    input.extend(flags.iter().rev());
    let mut expected = [0; 12];
    expected[0] = u32::from(flags[0]);
    expected[1] = u32::from(flags[1]);
    let remaining_flags = if bmi {
        expected[2] = u32::from(flags[2]);
        &flags[3..]
    } else {
        &flags[2..]
    };
    for (target, &flag) in expected[3..8].iter_mut().zip(remaining_flags) {
        *target = u32::from(flag);
    }
    expected[8] = capacity;
    expected[9] = u32::from(requested_log);
    expected[10] = source.len() as u32;
    expected[11] = source.len().min(SOURCE_LIMIT) as u32;
    (input, expected)
}

fn explicit_capacity(source: &[u8], bmi: bool, mask: u8, log: u8, capacity: u32) -> Vec<u8> {
    let (mut input, _) = explicit_case(source, bmi, mask, log);
    assert!(capacity <= 4 * input.len() as u32);
    let flag_count = if bmi { 8 } else { 7 };
    let width = input.len() - source.len() - flag_count - 1;
    input[source.len() + 1..source.len() + 1 + width]
        .copy_from_slice(&capacity.to_le_bytes()[..width]);
    let selected = select_parameters(&input, bmi).unwrap();
    assert_eq!(selected.output_capacity, capacity);
    assert_eq!(selected.source, &source[..source.len().min(SOURCE_LIMIT)]);
    input
}

#[test]
fn complete_huffman_driver_records_every_early_exit_and_round_trip() {
    let alternating: Vec<_> = (0..128).map(|index| (index & 1) as u8).collect();
    let alphabet: Vec<_> = (0..=255).collect();
    let mut scratch = EntropyOutput::new();
    for bmi in [false, true] {
        for (source, capacity, expected) in [
            (&[][..], 0, Outcome::Empty),
            (&[9][..], 0, Outcome::Single),
            (&[5; 128][..], 0, Outcome::Uniform),
            (&alphabet, 1024, Outcome::TableUnavailable),
            (&alternating, 0, Outcome::DescriptionTooSmall),
            (&[0, 1][..], 2, Outcome::StreamZero),
            (&alternating, 512, Outcome::RoundTrip),
        ] {
            let input = explicit_capacity(source, bmi, 0, 12, capacity);
            let trial = round_trip(&input, bmi, &mut scratch).unwrap();
            assert_eq!(
                trial.outcome,
                expected,
                "BMI={bmi}, source={}",
                source.len()
            );
            assert_eq!(trial.original_size, input.len());
            assert_eq!(trial.source_fingerprint, fingerprint(source));
            assert_eq!(trial.parameters[11] as usize, source.len());
            assert_eq!(round_trip(&input, bmi, &mut scratch).unwrap(), trial);
        }
        // These are successful wire encodings, but the original standalone
        // 4X compressor returns zero for sources below twelve symbols.
        for size in [6, 9, 11] {
            let input = explicit_capacity(&alternating[..size], bmi, 1, 12, 48);
            assert_eq!(
                round_trip(&input, bmi, &mut scratch).unwrap().outcome,
                Outcome::StreamZero
            );
        }
    }
}

#[test]
fn complete_huffman_driver_preserves_source_caps_depths_and_engine_selectors() {
    let mut source: Vec<_> = (0..SOURCE_LIMIT + 512)
        .map(|index| u8::from(index % 17 == 0))
        .collect();
    source[SOURCE_LIMIT] = 255; // Outside the capped prefix; cannot affect its histogram.
    let mut scratch = EntropyOutput::new();
    for bmi in [false, true] {
        for mask in [0, 1, if bmi { 0xff } else { 0x7f }] {
            let input = explicit_capacity(&source, bmi, mask, 12, 1 << 20);
            let trial = round_trip(&input, bmi, &mut scratch).unwrap();
            assert_eq!(trial.outcome, Outcome::RoundTrip);
            assert_eq!(trial.parameters[10] as usize, source.len());
            assert_eq!(trial.parameters[11] as usize, SOURCE_LIMIT);
            assert_eq!(trial.highest_symbol, Some(1));
            assert_eq!(trial.distinct_symbols, 2);
            assert_eq!(
                trial.source_fingerprint,
                fingerprint(&source[..SOURCE_LIMIT])
            );
        }
        for log in 1..=12 {
            let input = explicit_capacity(&[0, 255], bmi, 0, log, 40);
            let trial = round_trip(&input, bmi, &mut scratch).unwrap();
            assert_eq!(trial.outcome, Outcome::RoundTrip);
            assert_eq!(trial.adjusted_depth, log.max(9));
            assert_eq!(trial.selected_cap, log.max(9)); // size-two C unsigned wrap.
            assert_eq!(trial.actual_depth, 1);
            let optimal_mask = if bmi { 1 << 3 } else { 1 << 2 };
            let input = explicit_capacity(&[0, 255], bmi, optimal_mask, log, 40);
            let trial = round_trip(&input, bmi, &mut scratch).unwrap();
            assert_eq!(trial.outcome, Outcome::RoundTrip);
            assert_eq!(trial.selected_cap, trial.adjusted_depth);
            assert_eq!(trial.actual_depth, 1);
        }
    }
    assert!(scratch.retained_capacity() > 0);
}

#[test]
fn highest_symbol_adjustment_and_cheap_depth_match_reference_corner_vectors() {
    for (highest, minimum) in [(1, 2), (2, 3), (3, 3), (4, 4), (127, 8), (128, 9), (255, 9)] {
        for requested in 1..=12 {
            assert_eq!(adjusted_depth(requested, highest), requested.max(minimum));
        }
    }
    // Independently observed with pinned HUF_optimalTableLog flags=0; size two
    // demonstrates why a saturating subtraction changes the reference policy.
    for (cap, size, highest, expected) in [
        (2, 2, 1, 5),
        (12, 2, 1, 12),
        (9, 2, 255, 9),
        (12, 300, 2, 7),
        (12, 609, 12, 8),
        (12, 100255, 255, 12),
    ] {
        assert_eq!(cheap_depth(cap, size, highest), expected);
    }
}

#[test]
fn complete_huffman_driver_applies_a_meaningful_independent_optimal_policy() {
    // The 609-symbol Fibonacci distribution makes the cheap policy choose
    // depth eight. Searching the adjusted cap permits a cheaper deeper tree.
    let mut frequencies = [1_usize; 13];
    for index in 2..frequencies.len() {
        frequencies[index] = frequencies[index - 1] + frequencies[index - 2];
    }
    let mut source = Vec::new();
    for (symbol, count) in frequencies.into_iter().enumerate() {
        source.extend(core::iter::repeat_n(symbol as u8, count));
    }
    let mut scratch = EntropyOutput::new();
    for bmi in [false, true] {
        let capacity = (4 * source.len()) as u32;
        let cheap_input = explicit_capacity(&source, bmi, 0, 12, capacity);
        let optimal_input = explicit_capacity(&source, bmi, if bmi { 8 } else { 4 }, 12, capacity);
        let cheap = round_trip(&cheap_input, bmi, &mut scratch).unwrap();
        let optimal = round_trip(&optimal_input, bmi, &mut scratch).unwrap();
        assert_eq!(cheap.outcome, Outcome::RoundTrip);
        assert_eq!(optimal.outcome, Outcome::RoundTrip);
        assert_eq!(cheap.source_fingerprint, optimal.source_fingerprint);
        assert_ne!(
            cheap.description_fingerprint,
            optimal.description_fingerprint
        );
        assert!(
            optimal.description_size + optimal.encoded_size
                < cheap.description_size + cheap.encoded_size
        );
        assert_eq!(optimal.selected_cap, 12);
        assert_eq!(cheap.selected_cap, 8);
    }
}

#[test]
fn huffman_selectors_preserve_every_flag_pattern_depth_and_capacity_bound() {
    let source = pattern(63);
    let mut cases = 0;
    for bmi in [false, true] {
        for mask in 0..if bmi { 256 } else { 128 } {
            for log in 1..=12 {
                let (input, expected) = explicit_case(&source, bmi, mask as u8, log);
                let selected = select_parameters(&input, bmi).unwrap();
                assert_eq!(selected.fields(), expected);
                assert_eq!(selected.source, source);
                assert!(selected.output_capacity <= 4 * input.len() as u32);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 4608);
}

#[test]
fn huffman_selectors_reject_oversized_harness_input_before_consumption() {
    let input = alloc::vec![0; INPUT_LIMIT + 1];
    for bmi in [false, true] {
        assert_eq!(select_parameters(&input, bmi), Err(Error::ResourceLimit));
    }
    // Empty input still passes through every selector; exhaustion returns the
    // requested range minimum, including table log one, without underflow.
    for bmi in [false, true] {
        let selected = select_parameters(&[], bmi).unwrap();
        assert_eq!(selected.fields(), [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]);
    }
}

#[cfg(feature = "std")]
mod corpus {
    use super::*;
    use std::io::{self, Read};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    const FILES: usize = 833;
    const UNPACKED_BYTES: usize = 2_350_233;
    const LARGEST_FILE: usize = 238_262;
    // FNV-1a over sorted [u64 name length, UTF-8 name, u64 size, exact bytes].
    // Snapshot SHA-256 is 0a865a54eb80d629ea8639925f92aeab078650f93ac215714460fc6d55de6c22;
    // all members were independently compared against that archive before replay.
    const CORPUS_FINGERPRINT: u64 = 3_531_851_935_441_662_720;
    const NAMES: [&str; 7] = [
        "empty",
        "single",
        "uniform",
        "table_unavailable",
        "description_too_small",
        "stream_zero",
        "round_trip",
    ];

    fn invalid(message: &str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }

    fn ordered_unique(mut paths: Vec<PathBuf>) -> io::Result<Vec<PathBuf>> {
        paths.sort();
        if paths.len() > FILES || paths.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("too many or duplicate corpus paths"));
        }
        Ok(paths)
    }

    fn validate_file(path: &Path) -> io::Result<()> {
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() || metadata.len() > INPUT_LIMIT as u64 {
            return Err(invalid("nonregular, symlink, or oversized corpus file"));
        }
        Ok(())
    }

    fn files(root: &Path) -> io::Result<Vec<PathBuf>> {
        if !std::fs::symlink_metadata(root)?.file_type().is_dir() {
            return Err(invalid("corpus root must be a real directory"));
        }
        let mut paths = Vec::new();
        // This pinned archive has one flat directory. Reject nested directories
        // and every other non-file entry instead of silently omitting them.
        for entry in std::fs::read_dir(root)? {
            let path = entry?.path();
            validate_file(&path)?;
            if path.file_name().and_then(|name| name.to_str()).is_none() {
                return Err(invalid("corpus names must have stable UTF-8 bytes"));
            }
            paths.push(path);
            if paths.len() > FILES {
                return Err(invalid("too many corpus files"));
            }
        }
        ordered_unique(paths)
    }

    fn read_input(path: &Path) -> io::Result<Vec<u8>> {
        validate_file(path)?;
        let file = std::fs::File::open(path)?;
        if !file.metadata()?.is_file() {
            return Err(invalid("opened corpus input is not a file"));
        }
        let mut bytes = Vec::new();
        file.take(INPUT_LIMIT as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > INPUT_LIMIT {
            return Err(invalid("corpus input grew past the harness bound"));
        }
        Ok(bytes)
    }

    fn mix(hash: &mut u64, bytes: &[u8]) {
        for &byte in bytes {
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }

    fn outcome_index(outcome: Outcome) -> usize {
        match outcome {
            Outcome::Empty => 0,
            Outcome::Single => 1,
            Outcome::Uniform => 2,
            Outcome::TableUnavailable => 3,
            Outcome::DescriptionTooSmall => 4,
            Outcome::StreamZero => 5,
            Outcome::RoundTrip => 6,
        }
    }

    struct Totals {
        cases: usize,
        outcomes: [usize; 7],
        stream_outcomes: [[usize; 7]; 2],
        flags: [[usize; 2]; 8],
        requested: [usize; 13],
        adjusted: [usize; 13],
        selected: [usize; 13],
        actual: [usize; 13],
        capacity_buckets: [usize; 8],
        source_bytes: usize,
        capped_sources: usize,
        maximum_original: usize,
        maximum_remaining: usize,
        maximum_source: usize,
        maximum_capacity: usize,
        maximum_description: usize,
        maximum_encoded: usize,
        maximum_retained: usize,
        fingerprint: u64,
    }

    impl Totals {
        fn new() -> Self {
            Self {
                cases: 0,
                outcomes: [0; 7],
                stream_outcomes: [[0; 7]; 2],
                flags: [[0; 2]; 8],
                requested: [0; 13],
                adjusted: [0; 13],
                selected: [0; 13],
                actual: [0; 13],
                capacity_buckets: [0; 8],
                source_bytes: 0,
                capped_sources: 0,
                maximum_original: 0,
                maximum_remaining: 0,
                maximum_source: 0,
                maximum_capacity: 0,
                maximum_description: 0,
                maximum_encoded: 0,
                maximum_retained: 0,
                fingerprint: 0xcbf2_9ce4_8422_2325,
            }
        }

        fn observe(&mut self, name: &str, trial: &Trial, retained: usize) {
            let outcome = outcome_index(trial.outcome);
            self.cases += 1;
            self.outcomes[outcome] += 1;
            self.stream_outcomes[trial.parameters[0] as usize][outcome] += 1;
            for (flag, &value) in self.flags.iter_mut().zip(&trial.parameters[..8]) {
                flag[value as usize] += 1;
            }
            self.requested[trial.parameters[9] as usize] += 1;
            self.adjusted[usize::from(trial.adjusted_depth)] += 1;
            self.selected[usize::from(trial.selected_cap)] += 1;
            self.actual[usize::from(trial.actual_depth)] += 1;
            let capacity = trial.parameters[8] as usize;
            let bucket = match capacity {
                0 => 0,
                1..=7 => 1,
                8..=16 => 2,
                17..=63 => 3,
                64..=255 => 4,
                256..=4095 => 5,
                4096..=65535 => 6,
                _ => 7,
            };
            self.capacity_buckets[bucket] += 1;
            self.source_bytes += trial.parameters[11] as usize;
            self.capped_sources += usize::from(trial.parameters[10] as usize > SOURCE_LIMIT);
            self.maximum_original = self.maximum_original.max(trial.original_size);
            self.maximum_remaining = self.maximum_remaining.max(trial.parameters[10] as usize);
            self.maximum_source = self.maximum_source.max(trial.parameters[11] as usize);
            self.maximum_capacity = self.maximum_capacity.max(capacity);
            self.maximum_description = self.maximum_description.max(trial.description_size);
            self.maximum_encoded = self.maximum_encoded.max(trial.encoded_size);
            self.maximum_retained = self.maximum_retained.max(retained);
            mix(&mut self.fingerprint, &(name.len() as u64).to_le_bytes());
            mix(&mut self.fingerprint, name.as_bytes());
            for value in trial.parameters.map(u64::from).into_iter().chain([
                outcome as u64,
                trial.original_size as u64,
                trial.input_fingerprint,
                trial.source_fingerprint,
                trial.highest_symbol.map_or(256, u64::from),
                u64::from(trial.distinct_symbols),
                u64::from(trial.adjusted_depth),
                u64::from(trial.selected_cap),
                u64::from(trial.actual_depth),
                trial.description_size as u64,
                trial.description_fingerprint,
                trial.encoded_size as u64,
                trial.encoded_fingerprint,
            ]) {
                mix(&mut self.fingerprint, &value.to_le_bytes());
            }
        }

        fn report(&self, bmi: bool) {
            assert_eq!(self.cases, FILES);
            assert_eq!(self.outcomes.iter().sum::<usize>(), FILES);
            for histogram in [
                &self.requested,
                &self.adjusted,
                &self.selected,
                &self.actual,
            ] {
                assert_eq!(histogram.iter().sum::<usize>(), FILES);
            }
            std::eprintln!(
                concat!(
                    "HUF_SUMMARY {{\"bmi_layout\":{},\"cases\":{},\"outcomes\":{:?},",
                    "\"stream_outcomes\":{:?},\"flag_false_true\":{:?},\"requested_depths\":{:?},",
                    "\"adjusted_depths\":{:?},\"selected_caps\":{:?},\"actual_depths\":{:?},",
                    "\"capacity_buckets\":{:?},\"source_bytes\":{},\"capped_sources\":{},",
                    "\"maximum_original\":{},\"maximum_remaining\":{},\"maximum_source\":{},",
                    "\"maximum_capacity\":{},\"maximum_description\":{},\"maximum_encoded\":{},",
                    "\"maximum_retained\":{},\"fingerprint\":\"{:016x}\"}}"
                ),
                bmi,
                self.cases,
                self.outcomes,
                self.stream_outcomes,
                self.flags,
                self.requested,
                self.adjusted,
                self.selected,
                self.actual,
                self.capacity_buckets,
                self.source_bytes,
                self.capped_sources,
                self.maximum_original,
                self.maximum_remaining,
                self.maximum_source,
                self.maximum_capacity,
                self.maximum_description,
                self.maximum_encoded,
                self.maximum_retained,
                self.fingerprint
            );
        }
    }

    #[test]
    #[ignore = "requires all833 pinned huf_round_trip files in ZSTDOXIDE_HUF_ROUND_TRIP_CORPUS; emits auditable per-file JSON"]
    fn complete_pinned_huffman_round_trip_corpus_replays_both_bmi_layouts() {
        let root = PathBuf::from(
            std::env::var_os("ZSTDOXIDE_HUF_ROUND_TRIP_CORPUS")
                .expect("set ZSTDOXIDE_HUF_ROUND_TRIP_CORPUS to the pinned extracted archive"),
        );
        let paths = files(&root).unwrap();
        assert_eq!(paths.len(), FILES, "the full pinned corpus is required");
        let mut totals = [Totals::new(), Totals::new()];
        let mut scratch = [EntropyOutput::new(), EntropyOutput::new()];
        let mut corpus_hash = 0xcbf2_9ce4_8422_2325;
        let mut bytes = 0;
        let mut maximum = 0;
        for path in paths {
            let input = read_input(&path).unwrap();
            let name = path.file_name().unwrap().to_str().unwrap();
            let name_hex: std::string::String = name
                .as_bytes()
                .iter()
                .map(|byte| alloc::format!("{byte:02x}"))
                .collect();
            mix(&mut corpus_hash, &(name.len() as u64).to_le_bytes());
            mix(&mut corpus_hash, name.as_bytes());
            mix(&mut corpus_hash, &(input.len() as u64).to_le_bytes());
            mix(&mut corpus_hash, &input);
            bytes += input.len();
            maximum = maximum.max(input.len());
            for layout in 0..2 {
                let bmi = layout != 0;
                let trial = round_trip(&input, bmi, &mut scratch[layout])
                    .unwrap_or_else(|error| panic!("{}, BMI={bmi}: {error:?}", path.display()));
                totals[layout].observe(name, &trial, scratch[layout].retained_capacity());
                std::eprintln!(
                    concat!(
                        "HUF_RECORD {{\"name_utf8_hex\":\"{}\",\"bmi_layout\":{},\"outcome\":\"{}\",",
                        "\"original_size\":{},\"parameters\":{:?},\"input_fingerprint\":\"{:016x}\",",
                        "\"source_fingerprint\":\"{:016x}\",\"highest_symbol\":{},\"distinct_symbols\":{},",
                        "\"adjusted_depth\":{},\"selected_cap\":{},\"actual_depth\":{},\"description_size\":{},",
                        "\"description_fingerprint\":\"{:016x}\",\"encoded_size\":{},\"encoded_fingerprint\":\"{:016x}\"}}"
                    ),
                    name_hex,
                    bmi,
                    NAMES[outcome_index(trial.outcome)],
                    trial.original_size,
                    trial.parameters,
                    trial.input_fingerprint,
                    trial.source_fingerprint,
                    trial.highest_symbol.map_or(256, usize::from),
                    trial.distinct_symbols,
                    trial.adjusted_depth,
                    trial.selected_cap,
                    trial.actual_depth,
                    trial.description_size,
                    trial.description_fingerprint,
                    trial.encoded_size,
                    trial.encoded_fingerprint
                );
            }
        }
        assert_eq!(bytes, UNPACKED_BYTES);
        assert_eq!(maximum, LARGEST_FILE);
        assert_eq!(
            corpus_hash, CORPUS_FINGERPRINT,
            "changed corpus bytes/names"
        );
        for (layout, total) in totals.iter().enumerate() {
            total.report(layout != 0);
        }
        std::eprintln!(
            "HUF_CORPUS {{\"files\":{FILES},\"bytes\":{bytes},\"largest_file\":{maximum},\"fingerprint\":\"{corpus_hash:016x}\",\"source_limit\":{SOURCE_LIMIT},\"input_limit\":{INPUT_LIMIT}}}"
        );
    }

    #[test]
    fn corpus_loader_rejects_duplicate_nonregular_symlink_and_oversized_inputs() {
        static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
        struct Directory(PathBuf);
        impl Drop for Directory {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = Directory(std::env::temp_dir().join(alloc::format!(
            "zstdoxide-huffman-loader-{}-{stamp}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&directory.0).unwrap();
        let valid = directory.0.join("b");
        std::fs::write(&valid, b"valid").unwrap();
        std::fs::write(directory.0.join("a"), b"").unwrap();
        assert_eq!(
            files(&directory.0).unwrap(),
            [directory.0.join("a"), valid.clone()]
        );
        assert_eq!(read_input(&valid).unwrap(), b"valid");
        assert!(ordered_unique(alloc::vec![valid.clone(), valid.clone()]).is_err());
        assert!(
            ordered_unique(
                (0..=FILES)
                    .map(|i| directory.0.join(i.to_string()))
                    .collect()
            )
            .is_err()
        );
        assert!(files(&valid).is_err());
        let nested = directory.0.join("nested");
        std::fs::create_dir(&nested).unwrap();
        assert!(files(&directory.0).is_err());
        std::fs::remove_dir(&nested).unwrap();
        let oversized = directory.0.join("oversized");
        std::fs::File::create(&oversized)
            .unwrap()
            .set_len(INPUT_LIMIT as u64 + 1)
            .unwrap();
        assert!(read_input(&oversized).is_err());
        assert!(files(&directory.0).is_err());
        std::fs::remove_file(&oversized).unwrap();
        #[cfg(unix)]
        {
            let link = directory.0.join("link");
            std::os::unix::fs::symlink(&valid, &link).unwrap();
            assert!(read_input(&link).is_err());
            assert!(files(&directory.0).is_err());
            std::fs::remove_file(&link).unwrap();
            let root_link = directory.0.join("root-link");
            std::os::unix::fs::symlink(&directory.0, &root_link).unwrap();
            assert!(files(&root_link).is_err());
        }
    }
}

#[cfg(feature = "std")]
mod oracle {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct OracleDirectory(PathBuf);

    impl Drop for OracleDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn compile() -> OracleDirectory {
        let upstream = PathBuf::from(
            std::env::var_os("ZSTD_UPSTREAM").expect("set ZSTD_UPSTREAM to pinned v1.5.7"),
        );
        let revision = Command::new("git")
            .arg("-C")
            .arg(&upstream)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(revision.status.success());
        assert_eq!(
            std::str::from_utf8(&revision.stdout).unwrap().trim(),
            "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&upstream)
                .args(["diff", "--quiet", "HEAD", "--", "lib", "tests/fuzz"])
                .status()
                .unwrap()
                .success(),
            "upstream producer and headers must be clean"
        );
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = OracleDirectory(std::env::temp_dir().join(alloc::format!(
            "zstdoxide-huffman-parameters-{}-{stamp}-{sequence}",
            std::process::id()
        )));
        std::fs::create_dir(&directory.0).unwrap();
        let compiled = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
            .args(["-std=c99", "-O2", "-Wall", "-Wextra", "-Werror"])
            .arg("-I")
            .arg(upstream.join("lib"))
            .arg("-I")
            .arg(upstream.join("lib/common"))
            .arg("-I")
            .arg(upstream.join("tests/fuzz"))
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/oracles/huffman_parameters.c"
            ))
            .arg(upstream.join("tests/fuzz/fuzz_data_producer.c"))
            .arg(upstream.join("tests/fuzz/fuzz_helpers.c"))
            .arg("-o")
            .arg(directory.0.join("parameters"))
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&compiled.stderr)
        );
        directory
    }

    fn compare_batch(oracle: &OracleDirectory, cases: &[(Vec<u8>, bool, [u32; 12])]) {
        assert!(cases.len() <= 4096);
        assert!(cases.iter().map(|case| case.0.len()).sum::<usize>() <= 32 << 20);
        let mut request = Vec::from(&b"ZHPRM001"[..]);
        request.extend_from_slice(&(cases.len() as u32).to_le_bytes());
        for (input, bmi, _) in cases {
            assert!(input.len() <= INPUT_LIMIT);
            request.extend_from_slice(&(input.len() as u32).to_le_bytes());
            request.extend_from_slice(&u32::from(*bmi).to_le_bytes());
            request.extend_from_slice(input);
        }
        let mut child = Command::new(oracle.0.join("parameters"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = std::thread::scope(|scope| {
            let mut stdin = child.stdin.take().unwrap();
            let writer = scope.spawn(move || stdin.write_all(&request));
            let result = child.wait_with_output().unwrap();
            writer.join().unwrap().unwrap();
            result
        });
        assert!(
            output.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 12 + cases.len() * 48);
        assert_eq!(&output.stdout[..8], b"ZHPOUT01");
        assert_eq!(
            u32::from_le_bytes(output.stdout[8..12].try_into().unwrap()) as usize,
            cases.len()
        );
        for ((input, bmi, expected), bytes) in
            cases.iter().zip(output.stdout[12..].as_chunks::<48>().0)
        {
            let actual: [u32; 12] = core::array::from_fn(|index| {
                u32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
            });
            assert_eq!(
                &actual,
                expected,
                "independent C parameter vector, length={}, BMI={bmi}",
                input.len()
            );
            assert_eq!(select_parameters(input, *bmi).unwrap().fields(), actual);
        }
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7 with built lib/libzstd.a and a C compiler"]
    fn official_c_depth_selection_matches_adjustment_and_cheap_policy() {
        let directory = compile(); // Also verifies the clean pinned source revision.
        let upstream = PathBuf::from(std::env::var_os("ZSTD_UPSTREAM").unwrap());
        let executable = directory.0.join("depths");
        let compiled = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
            .args(["-std=c99", "-O2", "-Wall", "-Wextra", "-Werror"])
            .arg("-I")
            .arg(upstream.join("lib"))
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/oracles/huffman_depths.c"
            ))
            .arg(upstream.join("lib/libzstd.a"))
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&compiled.stderr)
        );
        let mut cases = Vec::new();
        for highest in [1_u8, 2, 3, 4, 127, 128, 254, 255] {
            for size in [
                2_usize,
                3,
                4,
                5,
                31,
                32,
                33,
                255,
                256,
                257,
                4095,
                4096,
                4097,
                SOURCE_LIMIT,
            ] {
                for requested in 1..=12_u8 {
                    let mut counts = [0_u32; 256];
                    counts[0] = size as u32 - 1;
                    counts[usize::from(highest)] = 1;
                    cases.push((counts, requested, size, highest));
                }
            }
        }
        assert_eq!(cases.len(), 1344);
        let mut request = Vec::from(&b"ZHDEP001"[..]);
        request.extend_from_slice(&(cases.len() as u32).to_le_bytes());
        for (counts, requested, _, _) in &cases {
            request.extend_from_slice(&u32::from(*requested).to_le_bytes());
            for count in counts {
                request.extend_from_slice(&count.to_le_bytes());
            }
        }
        let mut child = Command::new(&executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let result = std::thread::scope(|scope| {
            let mut stdin = child.stdin.take().unwrap();
            let writer = scope.spawn(move || stdin.write_all(&request));
            let result = child.wait_with_output().unwrap();
            writer.join().unwrap().unwrap();
            result
        });
        assert!(
            result.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout.len(), 12 + cases.len() * 8);
        assert_eq!(&result.stdout[..8], b"ZHDOUT01");
        assert_eq!(
            u32::from_le_bytes(result.stdout[8..12].try_into().unwrap()) as usize,
            cases.len()
        );
        for ((_, requested, size, highest), bytes) in
            cases.iter().zip(result.stdout[12..].as_chunks::<8>().0)
        {
            let adjusted = adjusted_depth(*requested, *highest);
            assert_eq!(
                u32::from_le_bytes(bytes[..4].try_into().unwrap()),
                u32::from(adjusted)
            );
            assert_eq!(
                u32::from_le_bytes(bytes[4..].try_into().unwrap()),
                u32::from(cheap_depth(adjusted, *size, *highest))
            );
        }
        std::eprintln!(
            "C verified all1344 adjusted/cheap-depth combinations, including size-two unsigned wrap and every requested depth"
        );
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7 and a C compiler; no built codec library is needed"]
    fn official_c_producer_matches_huffman_parameters_for_both_bmi_layouts() {
        let oracle = compile();
        let mut cases: Vec<_> = C_VECTORS
            .iter()
            .map(|&(length, bmi, expected)| (pattern(length), bmi, expected))
            .collect();
        let source = pattern(63);
        for bmi in [false, true] {
            for mask in 0..if bmi { 256 } else { 128 } {
                for log in 1..=12 {
                    let (input, expected) = explicit_case(&source, bmi, mask as u8, log);
                    cases.push((input, bmi, expected));
                }
            }
        }
        assert_eq!(cases.len(), 4674);
        for batch in cases.chunks(2048) {
            compare_batch(&oracle, batch);
        }
        std::eprintln!(
            "C verified all 4674 selector records: 66 stored boundary vectors and 4608 flag/depth/capacity cases, both BMI consumption layouts; no codec target replay"
        );
    }
}
