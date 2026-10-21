//! Bounded public Ultra admission, ownership and complete frame contracts.
#![forbid(unsafe_code)]
use core::num::NonZeroU64;
use zstdoxide::{
    CompressionParameterRequest as Request, Error, ReferenceStrategy, compress_at_level,
    compress_with_parameters, compress_with_parameters_in, decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
fn request() -> Request {
    let mut r = Request::with_level(13);
    r.set_strategy(Some(ReferenceStrategy::Ultra));
    r.set_window_log(16).unwrap();
    r.set_hash_log(12).unwrap();
    r.set_chain_log(12).unwrap();
    r.set_search_log(6).unwrap();
    r.set_min_match(3).unwrap();
    r.set_target_length(4095).unwrap();
    r
}
fn source(size: usize, kind: u8) -> Vec<u8> {
    let mut state = 0x9e3779b97f4a7c15_u64;
    (0..size)
        .map(|i| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            match kind {
                0 => b'a',
                1 => b"abcXabcYabcZabcd"[i % 15],
                _ => state as u8,
            }
        })
        .collect()
}
#[test]
fn complete_bounds_targets_and_three_byte_keys_round_trip() {
    for size in [64, 65, 512, 1024, 2048, 4096, 8192, 16384] {
        for kind in 0..3 {
            let input = source(size, kind);
            for (minimum, target) in [(3, 1), (3, 4095), (4, 4095), (7, 4095)] {
                let mut r = request();
                r.set_min_match(minimum).unwrap();
                r.set_target_length(target).unwrap();
                let before = r;
                let frame = compress_with_parameters(&input, &r).unwrap_or_else(|error| {
                    panic!("size={size} kind={kind} minimum={minimum} target={target}: {error:?}")
                });
                assert_eq!(decompress(&frame, size).unwrap(), input);
                assert_eq!(r, before);
                assert_eq!(compress_with_parameters(&input, &r).unwrap(), frame);
            }
        }
    }
    for level in [13, 14] {
        for size in [64, 512, 1024, 2048] {
            let input = source(size, 1);
            assert_eq!(
                decompress(&compress_at_level(&input, level).unwrap(), size).unwrap(),
                input
            );
        }
    }
}
#[test]
fn resource_limits_and_admitted_successors_preserve_allocation_order() {
    let input = source(32769, 0);
    let mut cases = Vec::new();
    for size in [0, 1, 63, 32769] {
        cases.push((size, request(), Error::ResourceLimit));
    }
    for size in [16385, 32768] {
        cases.push((size, request(), Error::AllocationFailed));
        let mut outside_window = request();
        outside_window.set_window_log(14).unwrap();
        cases.push((size, outside_window, Error::ResourceLimit));
    }
    for dimension in 0..5 {
        let mut r = request();
        match dimension {
            0 => r.set_window_log(13),
            1 => r.set_hash_log(13),
            2 => r.set_chain_log(13),
            3 => r.set_search_log(7),
            _ => r.set_target_length(4096),
        }
        .unwrap();
        cases.push((16384, r, Error::ResourceLimit));
    }
    let mut successor = request();
    successor.set_strategy(Some(ReferenceStrategy::Ultra2));
    cases.push((64, successor, Error::AllocationFailed));
    for (size, r, error) in cases {
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(1),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        assert_eq!(
            compress_with_parameters_in(&input[..size], &r, owner).err(),
            Some(error)
        );
        let s = monitor.snapshot();
        assert_eq!(
            (s.attempts, s.identities_created, s.owners_dropped),
            if error == Error::AllocationFailed {
                (1, 1, 1)
            } else {
                (0, 1, 1)
            }
        );
        monitor.assert_released();
    }
    for level in [13, 14] {
        assert_eq!(
            compress_at_level(&input[..4096], level),
            Err(Error::ResourceLimit)
        );
    }
}
#[test]
fn output_outlives_source_and_every_allocation_failure_releases_exact_owners() {
    let r = request();
    let input = source(128, 1);
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let id = owner.identity();
    let monitor = owner.monitor();
    let frame = {
        let temporary = input.clone();
        compress_with_parameters_in(&temporary, &r, owner).unwrap()
    };
    assert_eq!(frame.allocator().identity(), id);
    assert_eq!(monitor.snapshot().live_blocks, 1);
    assert_eq!(decompress(&frame, input.len()).unwrap(), input);
    let attempts = monitor.snapshot().attempts;
    assert!(attempts >= 10);
    drop(frame);
    monitor.assert_released();
    for ordinal in 1..=attempts {
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(ordinal),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let m = owner.monitor();
        assert_eq!(
            compress_with_parameters_in(&input, &r, owner).err(),
            Some(Error::AllocationFailed)
        );
        assert_eq!(m.snapshot().failures, 1);
        m.assert_released();
        assert_eq!(m.snapshot().identities_created, m.snapshot().owners_dropped);
    }
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let m = owner.monitor();
    let frame = compress_with_parameters_in(&input, &r, &owner).unwrap();
    assert_eq!(m.snapshot().identities_created, 1);
    assert_eq!(decompress(&frame, 128).unwrap(), input);
    drop(frame);
    m.assert_released();
    assert_eq!(m.snapshot().owners_dropped, 0);
    drop(owner);
    assert_eq!(m.snapshot().owners_dropped, 1);
}
