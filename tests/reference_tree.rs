//! Public bounded binary-tree requests, adjusted limits and exact owner failures.
#![forbid(unsafe_code)]
use core::num::NonZeroU64;
use zstdoxide::{
    CompressionParameterRequest as Request, Error, FrameHeader, ReferenceStrategy, SourceSize,
    compress_with_parameters, compress_with_parameters_in, decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
fn source(size: usize) -> Vec<u8> {
    b"binary tree delayed sorting and repeat publication across retained blocks."
        .iter()
        .copied()
        .cycle()
        .take(size)
        .collect()
}
fn request() -> Request {
    let mut r = Request::with_level(9);
    r.set_strategy(Some(ReferenceStrategy::BinaryTreeLazy2));
    r.set_search_log(4).unwrap();
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
fn tree_known_size_ignores_hints_and_preserves_key_and_target_equivalences() {
    for size in [0, 1, 6, 7, 96, 16_384, 16_385, 131_073, 262_145] {
        let input = source(size);
        let mut r = request();
        r.set_window_log(21).unwrap();
        r.set_hash_log(19).unwrap();
        r.set_chain_log(19).unwrap();
        r.set_min_match(4).unwrap();
        let baseline = compress_with_parameters(&input, &r).unwrap();
        verify(&input, &r, &baseline);
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
    }
}
#[test]
fn every_tree_resource_limit_rejects_before_allocator_cloning() {
    let input = source((1 << 21) + 1);
    for (w, h, c, s) in [
        (22, 19, 19, 8),
        (21, 20, 19, 8),
        (21, 19, 20, 8),
        (21, 19, 19, 9),
    ] {
        let mut r = request();
        r.set_window_log(w).unwrap();
        r.set_hash_log(h).unwrap();
        r.set_chain_log(c).unwrap();
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
    let mut r = request();
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
    for strategy in [
        ReferenceStrategy::Optimal,
        ReferenceStrategy::Ultra,
        ReferenceStrategy::Ultra2,
    ] {
        r.set_strategy(Some(strategy));
        assert_eq!(
            compress_with_parameters(&tiny, &r),
            Err(
                if matches!(
                    strategy,
                    ReferenceStrategy::Optimal | ReferenceStrategy::Ultra
                ) {
                    Error::ResourceLimit
                } else {
                    Error::UnsupportedFeature
                }
            )
        );
    }
}
#[test]
fn public_tree_results_outlive_sources_and_release_each_failed_owner() {
    for size in [8192, 32768] {
        let input = source(size);
        let r = request();
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let m = owner.monitor();
        let id = owner.identity();
        let frame = compress_with_parameters_in(&input, &r, owner).unwrap();
        assert_eq!(frame.allocator().identity(), id);
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
