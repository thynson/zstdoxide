//! Public DoubleFast selection, resolved resource caps and exact allocator owners.
#![forbid(unsafe_code)]

use core::num::NonZeroU64;
use zstdoxide::{
    CompressionParameterRequest as Request, Error, FrameHeader, ReferenceStrategy, SourceSize,
    compress_at_level, compress_with_parameters, compress_with_parameters_in, decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn source(length: usize) -> Vec<u8> {
    b"two independent sampled tables; shared committed entropy and indexed history."
        .iter()
        .copied()
        .cycle()
        .take(length)
        .collect()
}

fn check(input: &[u8], request: &Request, frame: &[u8]) {
    let resolved = request
        .resolve(SourceSize::Known(input.len() as u64), 0)
        .unwrap();
    assert_eq!(resolved.strategy(), ReferenceStrategy::DoubleFast);
    let (header, _) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.content_size, Some(input.len() as u64));
    assert_eq!(header.dictionary_id, 0);
    assert!(!header.has_checksum);
    assert_eq!(
        header.window_size,
        (1_u64 << resolved.window_log()).min(input.len() as u64)
    );
    assert_eq!(decompress(frame, input.len()).unwrap(), input);
}

#[test]
fn default_zero_and_three_are_identical_across_source_tiers_and_hints() {
    for size in [
        0, 1, 6, 7, 16_384, 16_385, 131_072, 131_073, 262_144, 262_145, 1_048_577,
    ] {
        let input = source(size);
        let baseline = compress_at_level(&input, 3).unwrap();
        assert_eq!(compress_at_level(&input, 0).unwrap(), baseline);
        for hint in [0, 1, 16_384, i32::MAX as u32] {
            let mut request = Request::new();
            request.set_source_hint(hint).unwrap();
            let before = request;
            assert_eq!(
                compress_with_parameters(&input, &request).unwrap(),
                baseline
            );
            assert_eq!(request, before);
            check(&input, &request, &baseline);
        }
    }
}

#[test]
fn level_four_admission_follows_all_four_pinned_source_tiers() {
    for size in [
        0, 16_383, 16_384, 16_385, 131_071, 131_072, 131_073, 262_144, 262_145, 1_048_577,
        2_097_153,
    ] {
        let input = source(size);
        let request = Request::with_level(4);
        let result = compress_with_parameters(&input, &request);
        assert_eq!(result, compress_at_level(&input, 4));
        if (16_385..=131_072).contains(&size) || size > 262_144 {
            check(&input, &request, &result.unwrap());
        } else {
            assert_eq!(
                request
                    .resolve(SourceSize::Known(size as u64), 0)
                    .unwrap()
                    .strategy(),
                ReferenceStrategy::Greedy
            );
            assert_eq!(decompress(&result.unwrap(), size).unwrap(), input);
        }
    }
}

#[test]
fn explicit_two_table_dimensions_window21_and_nonoperative_controls() {
    let input = source((1 << 21) + 1);
    let mut request = Request::with_level(-7);
    request.set_strategy(Some(ReferenceStrategy::DoubleFast));
    request.set_window_log(21).unwrap();
    request.set_hash_log(18).unwrap();
    request.set_chain_log(6).unwrap();
    request.set_min_match(4).unwrap();
    request.set_target_length(0).unwrap();
    let frame = compress_with_parameters(&input, &request).unwrap();
    check(&input, &request, &frame);
    assert_eq!(FrameHeader::parse(&frame).unwrap().0.window_size, 1 << 21);
    for (search, target, minimum) in [(1, 7, 3), (30, 131_072, 4)] {
        let mut changed = request;
        changed.set_search_log(search).unwrap();
        changed.set_target_length(target).unwrap();
        changed.set_min_match(minimum).unwrap();
        assert_eq!(compress_with_parameters(&input, &changed).unwrap(), frame);
    }
    request.set_hash_log(6).unwrap();
    request.set_chain_log(18).unwrap();
    for minimum in 3..=7 {
        request.set_min_match(minimum).unwrap();
        check(
            &input,
            &request,
            &compress_with_parameters(&input, &request).unwrap(),
        );
    }
}

#[test]
fn each_resource_cap_and_unsupported_strategy_precede_clones_and_allocation() {
    let input = source((1 << 21) + 1);
    let mut request = Request::new();
    request.set_window_log(22).unwrap();
    request.set_hash_log(19).unwrap();
    request.set_chain_log(19).unwrap();
    let mut requests = Vec::new();
    for oversized in 0..3 {
        let mut changed = request;
        changed
            .set_window_log(if oversized == 0 { 22 } else { 21 })
            .unwrap();
        changed
            .set_hash_log(if oversized == 1 { 19 } else { 18 })
            .unwrap();
        changed
            .set_chain_log(if oversized == 2 { 19 } else { 18 })
            .unwrap();
        requests.push((changed, Error::ResourceLimit));
    }
    for strategy in [ReferenceStrategy::Lazy, ReferenceStrategy::Ultra2] {
        let mut changed = request;
        changed.set_strategy(Some(strategy));
        requests.push((changed, Error::UnsupportedFeature));
    }
    for (request, error) in requests {
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(1),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        let before = monitor.snapshot();
        assert_eq!(
            compress_with_parameters_in(&input, &request, owner).err(),
            Some(error)
        );
        let after = monitor.snapshot();
        assert_eq!(after.attempts, 0);
        assert_eq!(after.failures, 0);
        assert_eq!(after.identities_created, before.identities_created);
        assert_eq!(after.owners_dropped, 1);
        monitor.assert_released();
    }
    // Operational limits apply to the actual adjusted profile, not the setters.
    let tiny = source(64);
    request.set_window_log(31).unwrap();
    request.set_hash_log(30).unwrap();
    request.set_chain_log(30).unwrap();
    check(
        &tiny,
        &request,
        &compress_with_parameters(&tiny, &request).unwrap(),
    );
}

#[test]
fn public_failure_ordinals_and_borrowed_result_preserve_exact_allocator_owners() {
    let input = source(8192);
    let request = Request::new();
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let identity = owner.identity();
    let monitor = owner.monitor();
    let frame = compress_with_parameters_in(&input, &request, owner).unwrap();
    assert_eq!(frame.allocator().identity(), identity);
    assert_eq!(monitor.snapshot().live_blocks, 1);
    let attempts = monitor.snapshot().attempts;
    check(&input, &request, &frame);
    drop(frame);
    monitor.assert_released();
    for ordinal in 1..=attempts {
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(ordinal),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        assert_eq!(
            compress_with_parameters_in(&input, &request, owner).err(),
            Some(Error::AllocationFailed)
        );
        assert_eq!(monitor.snapshot().attempts, ordinal);
        assert_eq!(monitor.snapshot().failures, 1);
        monitor.assert_released();
    }
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let monitor = owner.monitor();
    let frame = {
        let temporary = input.clone();
        let request = Request::new();
        compress_with_parameters_in(&temporary, &request, &owner).unwrap()
    };
    check(&input, &request, &frame);
    assert_eq!(monitor.snapshot().identities_created, 1);
    drop(frame);
    monitor.assert_released();
}
