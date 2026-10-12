//! Small public ownership and wire checks suitable for interpreted targets.
#![forbid(unsafe_code)]

use core::num::NonZeroU64;
use zstdoxide::{
    CompressionParameterRequest, Error, FrameHeader, ReferenceStrategy, compress_at_level_in,
    compress_with_parameters_in, decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

#[test]
fn tiny_frames_release_each_selected_owner_and_keep_borrowed_output_alive() {
    for size in [0, 1, 6] {
        let input = [0x51; 6];
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let frame = {
            let temporary = input;
            compress_at_level_in(&temporary[..size], 3, &owner).unwrap()
        };
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        let attempts = monitor.snapshot().attempts;
        // Empty input owns output only; nonempty tiny input also owns both tables.
        assert_eq!(attempts, if size == 0 { 1 } else { 3 });
        assert_eq!(decompress(&frame, size).unwrap(), input[..size]);
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
                compress_at_level_in(&input[..size], 3, owner).err(),
                Some(Error::AllocationFailed)
            );
            assert_eq!(monitor.snapshot().attempts, ordinal);
            assert_eq!(monitor.snapshot().failures, 1);
            monitor.assert_released();
        }
    }
}

#[test]
fn unequal_tables_publish_a_complete_frame_for_each_effective_key_width() {
    let source: Vec<_> = b"abcdabcd1234abcd"
        .iter()
        .copied()
        .cycle()
        .take(96)
        .collect();
    let mut request = CompressionParameterRequest::new();
    request.set_strategy(Some(ReferenceStrategy::DoubleFast));
    request.set_window_log(10).unwrap();
    request.set_hash_log(6).unwrap();
    request.set_chain_log(7).unwrap();
    let mut frames = Vec::new();
    for minimum in [3, 4, 7] {
        request.set_min_match(minimum).unwrap();
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let frame = compress_with_parameters_in(&source, &request, owner).unwrap();
        assert_eq!(frame.allocator().identity(), identity);
        let (header, _) = FrameHeader::parse(&frame).unwrap();
        assert_eq!(header.content_size, Some(96));
        assert_eq!(header.window_size, 96);
        assert_eq!(header.dictionary_id, 0);
        assert!(!header.has_checksum);
        assert_eq!(decompress(&frame, 96).unwrap(), source);
        assert!(decompress(&frame, 95).is_err());
        frames.push(frame.to_vec());
        drop(frame);
        monitor.assert_released();
    }
    assert_eq!(frames[0], frames[1]);
}
