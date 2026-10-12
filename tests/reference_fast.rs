//! Public reference-Fast admission, framing and allocator ownership contracts.
//!
//! These tests exercise the separate reference APIs, not a numeric mapping of
//! the named Greedy/Chain/Lazy policies. Admission examples come from pinned
//! v1.5.7 clevels.h and tests/upstream/parameters/{queries,requests}.tsv. Wire
//! assertions inspect initialized frame bytes independently of encoder state.
//! No C process or runtime fixture generator is required.
#![forbid(unsafe_code)]

use std::num::NonZeroU64;

use zstdoxide::{
    CompressionParameterRequest as Request, Error, FrameHeader, ReferenceStrategy, SourceSize,
    compress_at_level, compress_at_level_in, compress_with_parameters, compress_with_parameters_in,
    decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn noise(mut state: u32, length: usize) -> Vec<u8> {
    (0..length)
        .map(|_| {
            state ^= state.wrapping_shl(13);
            state ^= state >> 17;
            state ^= state.wrapping_shl(5);
            state as u8
        })
        .collect()
}

fn text(length: usize) -> Vec<u8> {
    b"independent reference Fast source: repeated words and history."
        .iter()
        .copied()
        .cycle()
        .take(length)
        .collect()
}

fn compact_request(level: i32) -> Request {
    let mut request = Request::with_level(level);
    request.set_window_log(10).unwrap();
    request.set_hash_log(10).unwrap();
    request.set_min_match(4).unwrap();
    request
}

fn tracking(ordinal: Option<u64>) -> TrackingAllocator {
    TrackingAllocator::new(AllocatorConfig {
        max_live_allocations: 256,
        fail_at: ordinal.map(|value| NonZeroU64::new(value).unwrap()),
    })
    .unwrap()
}

fn little(bytes: &[u8]) -> u64 {
    assert!(bytes.len() <= 5);
    bytes.iter().enumerate().fold(0, |value, (shift, &byte)| {
        value | (u64::from(byte) << (shift * 8))
    })
}

#[derive(Debug)]
struct Block {
    kind: u8,
    literal_kind: Option<u8>,
    sequence_modes: Option<[u8; 3]>,
}

/// Only locate block, literal and sequence headers, without entropy decoding.
/// Full byte equality is checked independently through the public decoder.
fn inspect(frame: &[u8]) -> Vec<Block> {
    let (header, mut position) = FrameHeader::parse(frame).unwrap();
    assert!(!header.has_checksum);
    assert_eq!(frame[4] & 3, 0, "no dictionary-ID field is emitted");
    let mut blocks = Vec::new();
    loop {
        let value = little(frame.get(position..position + 3).unwrap());
        position += 3;
        let kind = ((value >> 1) & 3) as u8;
        assert!(kind < 3);
        let size = usize::try_from(value >> 3).unwrap();
        assert!(size <= 128 * 1024 && size as u64 <= header.window_size);
        let stored = if kind == 1 { 1 } else { size };
        let payload = frame.get(position..position + stored).unwrap();
        position += stored;
        let mut block = Block {
            kind,
            literal_kind: None,
            sequence_modes: None,
        };
        if kind == 2 {
            let literal_kind = payload[0] & 3;
            let format = (payload[0] >> 2) & 3;
            let literal_bytes = if literal_kind < 2 {
                let (width, shift) = match format {
                    0 | 2 => (1, 3),
                    1 => (2, 4),
                    _ => (3, 4),
                };
                let size = usize::try_from(little(&payload[..width]) >> shift).unwrap();
                width + if literal_kind == 1 { 1 } else { size }
            } else {
                let (width, bits) = match format {
                    0 | 1 => (3, 10),
                    2 => (4, 14),
                    _ => (5, 18),
                };
                width + usize::try_from(little(&payload[..width]) >> (4 + bits)).unwrap()
            };
            let sequences = payload.get(literal_bytes..).unwrap();
            let (count, width) = match sequences[0] {
                first @ 0..=127 => (usize::from(first), 1),
                first @ 128..=254 => (
                    ((usize::from(first) - 128) << 8) + usize::from(sequences[1]),
                    2,
                ),
                _ => (
                    usize::try_from(little(&sequences[1..3])).unwrap() + 0x7f00,
                    3,
                ),
            };
            assert!(count <= 128 * 1024 / 3);
            block.literal_kind = Some(literal_kind);
            if count == 0 {
                assert_eq!(sequences, [0]);
            } else {
                let modes = sequences[width];
                assert_eq!(modes & 3, 0);
                assert!(sequences.len() > width + 1);
                block.sequence_modes = Some([modes >> 6, (modes >> 4) & 3, (modes >> 2) & 3]);
            }
        }
        blocks.push(block);
        if value & 1 != 0 {
            break;
        }
    }
    assert_eq!(position, frame.len());
    blocks
}

fn assert_frame(input: &[u8], request: &Request, frame: &[u8]) -> Vec<Block> {
    let resolved = request
        .resolve(SourceSize::Known(input.len() as u64), 0)
        .unwrap();
    assert!(matches!(
        resolved.strategy(),
        ReferenceStrategy::Fast | ReferenceStrategy::DoubleFast
    ));
    let (header, _) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.content_size, Some(input.len() as u64));
    assert_eq!(
        header.window_size,
        (input.len() as u64).min(1_u64 << resolved.window_log())
    );
    assert_eq!(header.dictionary_id, 0);
    assert!(!header.has_checksum);
    assert_eq!(decompress(frame, input.len()).unwrap(), input);
    inspect(frame)
}

#[test]
fn level_one_and_negative_profiles_execute_every_size_tier() {
    // queries.tsv:30–33 captures level 1 at these four distinct tiers.
    // :2–5 and :10–13 capture normalized minimum-level acceleration.
    for size in [16_384, 131_072, 262_144, 262_145] {
        let input = text(size);
        for level in [1, -1, -131_072, i32::MIN] {
            let request = Request::with_level(level);
            let before = request;
            let frame = compress_at_level(&input, level).unwrap();
            assert_eq!(frame, compress_with_parameters(&input, &request).unwrap());
            assert_eq!(request, before);
            assert_frame(&input, &request, &frame);
        }
        assert_eq!(
            compress_at_level(&input, i32::MIN),
            compress_at_level(&input, -131_072)
        );
    }
}

#[test]
fn level_two_and_explicit_overrides_follow_the_resolved_search_strategy() {
    // queries.tsv:127–159 records the tier edges, not a level allowlist.
    for size in [0, 16_384, 131_072, 131_073, 262_144, 262_145] {
        let input = text(size);
        let request = Request::with_level(2);
        let frame = compress_at_level(&input, 2).unwrap();
        assert_eq!(frame, compress_with_parameters(&input, &request).unwrap());
        assert_frame(&input, &request, &frame);
    }
    // Explicit strategy overrides select the algorithm independently of level.
    let input = text(16_385);
    let mut request = Request::new();
    request.set_strategy(Some(ReferenceStrategy::Fast));
    assert_frame(
        &input,
        &request,
        &compress_with_parameters(&input, &request).unwrap(),
    );
    request.set_level(1);
    request.set_strategy(Some(ReferenceStrategy::DoubleFast));
    assert_frame(
        &input,
        &request,
        &compress_with_parameters(&input, &request).unwrap(),
    );
}

#[test]
fn known_input_ignores_source_hints_including_empty_and_gap_cases() {
    for (level, size) in [(1, 0), (1, 64), (2, 262_144), (2, 262_145)] {
        let input = text(size);
        let expected = compress_at_level(&input, level);
        for hint in [1, 16_384, i32::MAX as u32] {
            let mut request = Request::with_level(level);
            request.set_source_hint(hint).unwrap();
            let before = request;
            assert_eq!(compress_with_parameters(&input, &request), expected);
            assert_eq!(request, before);
        }
    }
}

#[test]
fn minimum_match_three_and_four_have_identical_effective_keys() {
    let mut input = noise(0x1020_3040, 1024);
    input.extend(text(8192));
    let mut three = compact_request(1);
    three.set_min_match(3).unwrap();
    let mut four = three;
    four.set_min_match(4).unwrap();
    let frame = compress_with_parameters(&input, &three).unwrap();
    assert_eq!(frame, compress_with_parameters(&input, &four).unwrap());
    assert_frame(&input, &three, &frame);
}

#[test]
fn supported_hash_key_and_acceleration_boundaries_produce_valid_frames() {
    let mut input = noise(0x4567_89ab, 1024);
    input.extend(text(4096));
    for (hash, minimum, target) in [
        (6, 4, 0),
        (7, 5, 1),
        (8, 6, 2),
        (9, 7, 3),
        (10, 4, 4),
        (10, 7, 131_072),
    ] {
        let mut request = compact_request(1);
        request.set_hash_log(hash).unwrap();
        request.set_min_match(minimum).unwrap();
        request.set_target_length(target).unwrap();
        let resolved = request
            .resolve(SourceSize::Known(input.len() as u64), 0)
            .unwrap();
        assert_eq!(
            (
                resolved.window_log(),
                resolved.hash_log(),
                resolved.min_match(),
                resolved.target_length()
            ),
            (10, hash, minimum, target)
        );
        let before = request;
        let frame = compress_with_parameters(&input, &request).unwrap();
        let blocks = assert_frame(&input, &request, &frame);
        assert_eq!(request, before);
        if target != 0 {
            assert!(
                blocks
                    .iter()
                    .filter(|b| b.kind == 2)
                    .all(|b| b.literal_kind == Some(0))
            );
        }
    }
}

#[test]
fn unused_chain_and_search_fields_do_not_change_fast_output_or_storage() {
    let input = text(8192);
    let request = compact_request(1);
    let baseline_owner = tracking(None);
    let baseline_monitor = baseline_owner.monitor();
    let baseline = compress_with_parameters_in(&input, &request, baseline_owner).unwrap();
    let expected_attempts = baseline_monitor.snapshot().attempts;
    let expected_peak = baseline_monitor.snapshot().peak_requested_bytes;
    for (chain, search) in [(6, 1), (29, 29)] {
        let mut changed = request;
        changed.set_chain_log(chain).unwrap();
        changed.set_search_log(search).unwrap();
        let owner = tracking(None);
        let monitor = owner.monitor();
        let frame = compress_with_parameters_in(&input, &changed, owner).unwrap();
        assert_eq!(frame.as_slice(), baseline.as_slice());
        assert_eq!(monitor.snapshot().attempts, expected_attempts);
        assert_eq!(monitor.snapshot().peak_requested_bytes, expected_peak);
        drop(frame);
        monitor.assert_released();
    }
    drop(baseline);
    baseline_monitor.assert_released();
}

#[test]
fn unsupported_and_resource_failures_precede_allocator_clone_or_requests() {
    let input = noise(0x2233_4455, 1_048_577);
    let before = input.clone();
    let mut window = Request::with_level(1);
    window.set_window_log(21).unwrap();
    let mut hash = Request::with_level(1);
    hash.set_hash_log(19).unwrap();
    let mut double_resource = window;
    double_resource.set_hash_log(19).unwrap();
    double_resource.set_strategy(Some(ReferenceStrategy::DoubleFast));
    let mut unsupported = double_resource;
    unsupported.set_strategy(Some(ReferenceStrategy::Lazy));
    for (request, error) in [
        (unsupported, Error::UnsupportedFeature),
        (double_resource, Error::ResourceLimit),
        (window, Error::ResourceLimit),
        (hash, Error::ResourceLimit),
    ] {
        let owner = tracking(Some(1));
        let monitor = owner.monitor();
        let initial = monitor.snapshot();
        let request_before = request;
        assert_eq!(
            compress_with_parameters_in(&input, &request, owner).err(),
            Some(error)
        );
        let final_state = monitor.snapshot();
        assert_eq!(final_state.attempts, initial.attempts);
        assert_eq!(final_state.identities_created, initial.identities_created);
        assert_eq!(final_state.owners_dropped, 1);
        assert_eq!(
            final_state.failures, 0,
            "armed first allocation was not reached"
        );
        assert_eq!(request, request_before);
        monitor.assert_released();
    }
    assert_eq!(input, before);
    // A requested excessive hash is legal if actual known-size adjustment
    // reduces it below the operational cap (the cap is not a setter limit).
    let tiny = text(64);
    let frame = compress_with_parameters(&tiny, &hash).unwrap();
    assert_frame(&tiny, &hash, &frame);
}

#[test]
fn frame_flags_and_window_remain_truthful_at_content_size_boundaries() {
    for size in [0, 1, 6, 7, 255, 256, 1023, 1024, 1025, 65_791, 65_792] {
        let request = compact_request(1);
        let input = text(size);
        let frame = compress_with_parameters(&input, &request).unwrap();
        assert_frame(&input, &request, &frame);
        assert_eq!(frame[4] & 0x20 != 0, size <= 1024);
    }
}

#[test]
fn output_retains_original_owner_and_outlives_source_and_request() {
    let owner = tracking(None);
    let identity = owner.identity();
    let monitor = owner.monitor();
    let (frame, expected) = {
        let input = text(6144);
        let expected = input.clone();
        let request = compact_request(1);
        (
            compress_with_parameters_in(&input, &request, owner).unwrap(),
            expected,
        )
    };
    assert_eq!(frame.allocator().identity(), identity);
    assert_eq!(monitor.snapshot().live_blocks, 1);
    assert_eq!(monitor.snapshot().live_bytes, frame.capacity() as u128);
    assert!(monitor.snapshot().identities_created > 1);
    assert_eq!(
        monitor.snapshot().owners_dropped + 1,
        monitor.snapshot().identities_created
    );
    assert_eq!(decompress(&frame, expected.len()).unwrap(), expected);
    drop(frame);
    monitor.assert_released();
    assert_eq!(
        monitor.snapshot().owners_dropped,
        monitor.snapshot().identities_created
    );

    let owner = tracking(None);
    let identity = owner.identity();
    let monitor = owner.monitor();
    let frame = compress_at_level_in(&[], -1, owner).unwrap();
    assert_eq!(frame.allocator().identity(), identity);
    assert_eq!(
        monitor.snapshot().identities_created,
        1,
        "empty frame needs no scratch clone"
    );
    assert_eq!(monitor.snapshot().attempts, 1);
    assert_eq!(decompress(&frame, 0).unwrap(), []);
    drop(frame);
    monitor.assert_released();
}

#[test]
fn borrowed_nonclone_allocator_supports_both_allocator_entry_points() {
    let owner = tracking(None).into_non_clone();
    let monitor = owner.monitor();
    let input = text(8192);
    let expected = compress_at_level(&input, -3).unwrap();
    let level = compress_at_level_in(&input, -3, &owner).unwrap();
    let request = Request::with_level(-3);
    let explicit = compress_with_parameters_in(&input, &request, &owner).unwrap();
    assert_eq!(level.as_slice(), expected);
    assert_eq!(explicit.as_slice(), expected);
    assert!(std::ptr::eq(*level.allocator(), &owner));
    assert!(std::ptr::eq(*explicit.allocator(), &owner));
    assert_eq!(monitor.snapshot().identities_created, 1);
    assert_eq!(monitor.snapshot().live_blocks, 2);
    assert_eq!(
        monitor.snapshot().live_bytes,
        (level.capacity() + explicit.capacity()) as u128
    );
    drop((level, explicit));
    monitor.assert_released();
}

/// Different deterministic blocks preserve a small literal alphabet while
/// creating repeated 32-byte phrases and varying LL/ML code frequencies.
fn entropy_source() -> Vec<u8> {
    let mut input = Vec::new();
    for index in 0..8 {
        let mut block = noise(0x3141_5926 + index * 0x0102_0304, 1024);
        for byte in &mut block {
            *byte &= 15;
        }
        for start in (80..960).step_by(64) {
            let length = 24 + start % 13;
            block.copy_within(start - 64..start - 64 + length, start);
        }
        input.extend(block);
    }
    input.extend_from_slice(b"tiny!");
    input
}

#[test]
fn bounded_multiblock_campaign_reaches_adaptive_and_reused_entropy() {
    let input = entropy_source();
    let request = compact_request(1);
    let frame = compress_with_parameters(&input, &request).unwrap();
    let blocks = assert_frame(&input, &request, &frame);
    eprintln!("reference Fast entropy branch witness: {blocks:?}");
    assert!(
        blocks.iter().any(|b| b.literal_kind == Some(2)),
        "new Huffman"
    );
    assert!(
        blocks.iter().any(|b| b.literal_kind == Some(3)),
        "reused Huffman"
    );
    assert!(
        blocks
            .iter()
            .filter_map(|b| b.sequence_modes)
            .flatten()
            .any(|m| m == 2),
        "new FSE"
    );
    assert!(
        blocks
            .iter()
            .filter_map(|b| b.sequence_modes)
            .flatten()
            .any(|m| m == 3),
        "reused FSE"
    );
    assert_eq!(
        blocks.last().unwrap().kind,
        0,
        "five-byte final block bypasses fresh entropy"
    );
    let negative = compact_request(-1);
    let frame = compress_with_parameters(&input, &negative).unwrap();
    let raw = assert_frame(&input, &negative, &frame);
    assert!(
        raw.iter().any(|b| b.kind == 2),
        "raw literal policy still permits sequence compression"
    );
    assert!(
        raw.iter()
            .filter(|b| b.kind == 2)
            .all(|b| b.literal_kind == Some(0))
    );
}

#[test]
fn every_observed_allocation_failure_releases_partial_output_and_scratch() {
    let input = entropy_source();
    let source_before = input.clone();
    let mut total_ordinals = 0;
    for request in [compact_request(1), compact_request(-1)] {
        let request_before = request;
        let owner = tracking(None);
        let monitor = owner.monitor();
        let baseline = compress_with_parameters_in(&input, &request, owner).unwrap();
        let expected = baseline.to_vec();
        let attempts = monitor.snapshot().attempts;
        assert!(
            (1..=1024).contains(&attempts),
            "bounded complete failure sweep: {attempts}"
        );
        assert!(
            monitor.snapshot().grow_calls > 0,
            "source exercises scratch growth"
        );
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert_eq!(monitor.snapshot().live_bytes, baseline.capacity() as u128);
        drop(baseline);
        monitor.assert_released();
        for ordinal in 1..=attempts {
            let owner = tracking(Some(ordinal));
            let monitor = owner.monitor();
            assert_eq!(
                compress_with_parameters_in(&input, &request, owner).err(),
                Some(Error::AllocationFailed),
                "level {} ordinal {ordinal}/{attempts}",
                request.level()
            );
            let state = monitor.snapshot();
            assert_eq!(state.failures, 1);
            assert_eq!(state.attempts, ordinal);
            assert_eq!(state.identities_created, state.owners_dropped);
            monitor.assert_released();
        }
        assert_eq!(request, request_before);
        assert_eq!(
            compress_with_parameters(&input, &request).unwrap(),
            expected
        );
        total_ordinals += attempts;
        eprintln!(
            "reference Fast level {}: every {attempts} observed allocation ordinal rejected and released",
            request.level()
        );
    }
    assert_eq!(input, source_before);
    assert!(total_ordinals > 2);
}

#[test]
fn maximum_admitted_hash_uses_bounded_storage_and_releases_it_before_return() {
    let input = text(262_145);
    let mut request = Request::with_level(1);
    request.set_window_log(19).unwrap();
    request.set_hash_log(18).unwrap();
    request.set_min_match(7).unwrap();
    assert_eq!(
        request
            .resolve(SourceSize::Known(input.len() as u64), 0)
            .unwrap()
            .hash_log(),
        18,
    );
    let owner = tracking(None);
    let monitor = owner.monitor();
    let frame = compress_with_parameters_in(&input, &request, owner).unwrap();
    assert_frame(&input, &request, &frame);
    assert!(
        monitor.snapshot().peak_requested_bytes >= (262_144 * std::mem::size_of::<usize>()) as u128
    );
    assert_eq!(monitor.snapshot().live_blocks, 1);
    assert_eq!(monitor.snapshot().live_bytes, frame.capacity() as u128);
    drop(frame);
    monitor.assert_released();
}
