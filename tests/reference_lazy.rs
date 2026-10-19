//! Public bounded Lazy/Lazy2 requests, actual-size admission and exact owners.
#![forbid(unsafe_code)]

use core::num::NonZeroU64;
use zstdoxide::{
    CompressionParameterRequest as Request, Error, FrameHeader, ReferenceStrategy, SourceSize,
    compress_at_level, compress_with_parameters, compress_with_parameters_in, decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn source(size: usize) -> Vec<u8> {
    b"lazy lookahead; repeated history; commit offsets only after a complete block."
        .iter()
        .copied()
        .cycle()
        .take(size)
        .collect()
}
fn request(strategy: ReferenceStrategy) -> Request {
    let mut r = Request::with_level(6);
    r.set_strategy(Some(strategy));
    r
}
fn verify(input: &[u8], r: &Request, frame: &[u8]) {
    let p = r.resolve(SourceSize::Known(input.len() as u64), 0).unwrap();
    let (h, _) = FrameHeader::parse(frame).unwrap();
    assert_eq!(h.content_size, Some(input.len() as u64));
    assert_eq!(
        h.window_size,
        (1u64 << p.window_log()).min(input.len() as u64)
    );
    assert_eq!(h.dictionary_id, 0);
    assert!(!h.has_checksum);
    assert_eq!(decompress(frame, input.len()).unwrap(), input);
}

#[test]
fn default_levels_resolve_by_actual_source_tier_and_preserve_error_kinds() {
    for size in [
        0, 1, 16_384, 16_385, 131_072, 131_073, 262_144, 262_145, 2_097_153,
    ] {
        let input = source(size);
        for level in 5..=22 {
            let r = Request::with_level(level);
            let p = r.resolve(SourceSize::Known(size as u64), 0).unwrap();
            // Preserve the original levels and source sizes. The source-aware
            // successor classifies every newly implemented Ultra tuple here as
            // resource-limited; this corpus has no 64..=2048-byte Ultra inputs.
            let expected = if p.strategy() == ReferenceStrategy::Ultra {
                Err(Error::ResourceLimit)
            } else {
                match level {
                    5 | 6 => Ok(()),
                    7..=9 if size <= 262_144 => Ok(()),
                    10..=12 if (16_385..=262_144).contains(&size) => Ok(()),
                    10 if size <= 16_384 => Err(Error::ResourceLimit),
                    11..=12 if size <= 16_384 => Err(Error::ResourceLimit),
                    13..=15 if (16_385..=262_144).contains(&size) => Err(Error::ResourceLimit),
                    7..=17 if size > 262_144 => Err(Error::ResourceLimit),
                    _ => Err(Error::UnsupportedFeature),
                }
            };
            let result = compress_at_level(&input, level);
            assert_eq!(
                result.as_ref().map(|_| ()).map_err(|e| *e),
                expected,
                "level {level}, size {size}, tuple {:?}",
                p.to_raw()
            );
            match result {
                Ok(frame) => {
                    assert_eq!(compress_with_parameters(&input, &r).unwrap(), frame);
                    verify(&input, &r, &frame);
                }
                Err(e) => assert_eq!(compress_with_parameters(&input, &r), Err(e)),
            }
        }
    }
}

#[test]
fn lazy_requests_ignore_hints_and_preserve_chain_row_key_and_target_equivalences() {
    for strategy in [ReferenceStrategy::Lazy, ReferenceStrategy::Lazy2] {
        for size in [0, 1, 6, 7, 8192, 16_384, 16_385, 131_073, 262_145] {
            let input = source(size);
            let mut r = request(strategy);
            r.set_window_log(21).unwrap();
            r.set_hash_log(19).unwrap();
            r.set_search_log(6).unwrap();
            r.set_min_match(4).unwrap();
            let baseline = compress_with_parameters(&input, &r).unwrap();
            for hint in [1, 16_384, i32::MAX as u32] {
                r.set_source_hint(hint).unwrap();
                let before = r;
                assert_eq!(compress_with_parameters(&input, &r).unwrap(), baseline);
                assert_eq!(r, before);
            }
            for target in [0, 4, 131_072] {
                r.set_target_length(target).unwrap();
                assert_eq!(compress_with_parameters(&input, &r).unwrap(), baseline);
            }
            r.set_min_match(3).unwrap();
            assert_eq!(compress_with_parameters(&input, &r).unwrap(), baseline);
            r.set_min_match(6).unwrap();
            let wide = compress_with_parameters(&input, &r).unwrap();
            r.set_min_match(7).unwrap();
            assert_eq!(compress_with_parameters(&input, &r).unwrap(), wide);
            verify(&input, &r, &wide);
            if size > 16_384 {
                for (chain, search) in [(6, 6), (30, 30)] {
                    r.set_chain_log(chain).unwrap();
                    r.set_search_log(search).unwrap();
                    assert_eq!(compress_with_parameters(&input, &r).unwrap(), wide);
                }
            }
        }
    }
}

#[test]
fn resource_rejection_precedes_owners_and_small_source_adjustment_precedes_caps() {
    let input = source(2_097_153);
    for strategy in [ReferenceStrategy::Lazy, ReferenceStrategy::Lazy2] {
        for (w, h, s) in [(22, 19, 8), (21, 20, 8), (14, 15, 9)] {
            let mut r = request(strategy);
            r.set_window_log(w).unwrap();
            r.set_hash_log(h).unwrap();
            r.set_search_log(s).unwrap();
            let owner = TrackingAllocator::new(AllocatorConfig {
                fail_at: NonZeroU64::new(1),
                ..AllocatorConfig::default()
            })
            .unwrap();
            let m = owner.monitor();
            assert_eq!(
                compress_with_parameters_in(&input, &r, owner).err(),
                Some(Error::ResourceLimit)
            );
            let state = m.snapshot();
            assert_eq!(
                (
                    state.attempts,
                    state.failures,
                    state.identities_created,
                    state.owners_dropped
                ),
                (0, 0, 1, 1)
            );
            m.assert_released();
        }
        let mut r = request(strategy);
        r.set_window_log(31).unwrap();
        r.set_hash_log(30).unwrap();
        r.set_chain_log(30).unwrap();
        r.set_search_log(8).unwrap();
        let tiny = source(64);
        verify(&tiny, &r, &compress_with_parameters(&tiny, &r).unwrap());
        r.set_search_log(9).unwrap();
        assert_eq!(
            compress_with_parameters(&tiny, &r),
            Err(Error::ResourceLimit)
        );
    }
}

#[test]
fn both_lazy_depths_release_every_failed_allocation_and_return_borrowed_results() {
    for strategy in [ReferenceStrategy::Lazy, ReferenceStrategy::Lazy2] {
        for size in [8192, 32768] {
            let input = source(size);
            let r = request(strategy);
            let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let m = owner.monitor();
            let identity = owner.identity();
            let frame = compress_with_parameters_in(&input, &r, owner).unwrap();
            assert_eq!(frame.allocator().identity(), identity);
            verify(&input, &r, &frame);
            let attempts = m.snapshot().attempts;
            drop(frame);
            m.assert_released();
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
                let state = m.snapshot();
                assert_eq!((state.attempts, state.failures), (ordinal, 1));
                assert_eq!(state.owners_dropped, state.identities_created);
                m.assert_released();
            }
            let owner = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let m = owner.monitor();
            let frame = {
                let temporary = input.clone();
                compress_with_parameters_in(&temporary, &r, &owner).unwrap()
            };
            verify(&input, &r, &frame);
            assert_eq!(m.snapshot().identities_created, 1);
            drop(frame);
            m.assert_released();
            drop(owner);
            assert_eq!(m.snapshot().owners_dropped, 1);
        }
    }
}
