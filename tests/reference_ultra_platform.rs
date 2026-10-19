//! Small public Ultra contracts for interpreted and foreign-endian targets.
#![forbid(unsafe_code)]
use zstdoxide::{
    CompressionParameterRequest, Error, ReferenceStrategy, compress_with_parameters_in, decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
fn request() -> CompressionParameterRequest {
    let mut r = CompressionParameterRequest::with_level(13);
    r.set_strategy(Some(ReferenceStrategy::Ultra));
    r.set_window_log(10).unwrap();
    r.set_hash_log(6).unwrap();
    r.set_chain_log(6).unwrap();
    r.set_search_log(1).unwrap();
    r.set_min_match(3).unwrap();
    r.set_target_length(1).unwrap();
    r
}
#[test]
fn complete_three_byte_key_frame_preserves_borrowed_owner_and_wire_bytes() {
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let monitor = owner.monitor();
    let input = [b'a'; 64];
    let frame = {
        let temporary = input;
        compress_with_parameters_in(&temporary, &request(), &owner).unwrap()
    };
    assert_eq!(monitor.snapshot().identities_created, 1);
    assert_eq!(monitor.snapshot().live_blocks, 1);
    assert_eq!(decompress(&frame, 64).unwrap(), input);
    drop(frame);
    monitor.assert_released();
    assert_eq!(monitor.snapshot().owners_dropped, 0);
    drop(owner);
    assert_eq!(monitor.snapshot().owners_dropped, 1);
}
#[test]
fn rejected_source_never_clones_or_allocates_on_the_target() {
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    assert_eq!(
        compress_with_parameters_in(&[b'a'; 63], &request(), owner).err(),
        Some(Error::ResourceLimit)
    );
    let s = monitor.snapshot();
    assert_eq!(
        (s.attempts, s.identities_created, s.owners_dropped),
        (0, 1, 1)
    );
    monitor.assert_released();
}
