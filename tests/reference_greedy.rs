//! Public bounded Greedy admission, actual source resolution and exact owners.
#![forbid(unsafe_code)]

use core::num::NonZeroU64;
use zstdoxide::{
    CompressionParameterRequest as Request, Error, FrameHeader, ReferenceStrategy, SourceSize,
    compress_at_level, compress_with_parameters, compress_with_parameters_in, decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn source(length: usize) -> Vec<u8> {
    b"retained Greedy history; independent chains or salted rows; committed repeats."
        .iter()
        .copied()
        .cycle()
        .take(length)
        .collect()
}
fn request() -> Request {
    let mut request = Request::with_level(4);
    request.set_strategy(Some(ReferenceStrategy::Greedy));
    request
}
fn verify(input: &[u8], request: &Request, frame: &[u8]) {
    let resolved = request
        .resolve(SourceSize::Known(input.len() as u64), 0)
        .unwrap();
    assert_eq!(resolved.strategy(), ReferenceStrategy::Greedy);
    let (header, _) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.content_size, Some(input.len() as u64));
    assert_eq!(
        header.window_size,
        (1_u64 << resolved.window_log()).min(input.len() as u64)
    );
    assert_eq!(header.dictionary_id, 0);
    assert!(!header.has_checksum);
    assert_eq!(decompress(frame, input.len()).unwrap(), input);
}

#[test]
fn resolved_chain_and_row_sources_ignore_hints_without_mutating_requests() {
    for size in [
        0, 1, 6, 7, 16_384, 16_385, 131_072, 131_073, 262_144, 262_145,
    ] {
        let input = source(size);
        let baseline = compress_with_parameters(&input, &request()).unwrap();
        verify(&input, &request(), &baseline);
        for hint in [0, 1, 16_384, i32::MAX as u32] {
            let mut request = request();
            request.set_source_hint(hint).unwrap();
            let before = request;
            assert_eq!(
                compress_with_parameters(&input, &request).unwrap(),
                baseline
            );
            assert_eq!(request, before);
        }
        if size <= 16_384 || (131_073..=262_144).contains(&size) {
            assert_eq!(compress_at_level(&input, 4).unwrap(), baseline);
        }
    }
}

#[test]
fn row_ignores_chain_size_and_caps_search_while_key_and_target_equivalences_hold() {
    for size in [8192, (1 << 21) + 1] {
        let input = source(size);
        let mut request = request();
        request.set_window_log(21).unwrap();
        request.set_hash_log(19).unwrap();
        request.set_search_log(6).unwrap();
        for (first, second) in [(3, 4), (6, 7)] {
            request.set_min_match(first).unwrap();
            let baseline = compress_with_parameters(&input, &request).unwrap();
            request.set_min_match(second).unwrap();
            assert_eq!(
                compress_with_parameters(&input, &request).unwrap(),
                baseline
            );
            for target in [4, 131_072] {
                request.set_target_length(target).unwrap();
                assert_eq!(
                    compress_with_parameters(&input, &request).unwrap(),
                    baseline
                );
            }
            verify(&input, &request, &baseline);
        }
        if size > 16_384 {
            let baseline = compress_with_parameters(&input, &request).unwrap();
            for (chain, search) in [(6, 6), (30, 30)] {
                request.set_chain_log(chain).unwrap();
                request.set_search_log(search).unwrap();
                assert_eq!(
                    compress_with_parameters(&input, &request).unwrap(),
                    baseline
                );
            }
        }
    }
}

#[test]
fn public_resource_and_strategy_errors_precede_allocation_and_cloning() {
    let input = source((1 << 21) + 1);
    for (window, hash, search, strategy, expected) in [
        (22, 19, 8, ReferenceStrategy::Greedy, Error::ResourceLimit),
        (21, 20, 8, ReferenceStrategy::Greedy, Error::ResourceLimit),
        (14, 15, 9, ReferenceStrategy::Greedy, Error::ResourceLimit),
        (
            31,
            30,
            30,
            ReferenceStrategy::Lazy,
            Error::UnsupportedFeature,
        ),
        (
            31,
            30,
            30,
            ReferenceStrategy::Ultra2,
            Error::UnsupportedFeature,
        ),
    ] {
        let mut request = request();
        request.set_window_log(window).unwrap();
        request.set_hash_log(hash).unwrap();
        request.set_search_log(search).unwrap();
        request.set_strategy(Some(strategy));
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(1),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        assert_eq!(
            compress_with_parameters_in(&input, &request, owner).err(),
            Some(expected)
        );
        let state = monitor.snapshot();
        assert_eq!(
            (
                state.attempts,
                state.failures,
                state.identities_created,
                state.owners_dropped
            ),
            (0, 0, 1, 1)
        );
        monitor.assert_released();
    }
    // Requested dimensions are reduced before operational resource admission.
    let mut request = request();
    request.set_window_log(31).unwrap();
    request.set_hash_log(30).unwrap();
    request.set_chain_log(30).unwrap();
    request.set_search_log(8).unwrap();
    let tiny = source(64);
    verify(
        &tiny,
        &request,
        &compress_with_parameters(&tiny, &request).unwrap(),
    );
    request.set_search_log(9).unwrap();
    assert_eq!(
        compress_with_parameters(&tiny, &request),
        Err(Error::ResourceLimit)
    );
}

#[test]
fn both_selected_families_release_each_failed_owner_and_return_borrowed_outputs() {
    for size in [8192, 32768] {
        let input = source(size);
        let request = request();
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let frame = compress_with_parameters_in(&input, &request, owner).unwrap();
        assert_eq!(frame.allocator().identity(), identity);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        let attempts = monitor.snapshot().attempts;
        verify(&input, &request, &frame);
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
            let state = monitor.snapshot();
            assert_eq!((state.attempts, state.failures), (ordinal, 1));
            assert_eq!(state.owners_dropped, state.identities_created);
            monitor.assert_released();
        }
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let frame = {
            let temporary = input.clone();
            compress_with_parameters_in(&temporary, &request, &owner).unwrap()
        };
        verify(&input, &request, &frame);
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(frame);
        monitor.assert_released();
    }
}
