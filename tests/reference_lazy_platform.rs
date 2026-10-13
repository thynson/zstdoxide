//! Tiny public Lazy/Lazy2 ownership and wire checks for interpreted targets.
#![forbid(unsafe_code)]

use core::num::NonZeroU64;
use zstdoxide::{Error, compress_at_level_in, decompress};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

#[test]
fn tiny_frames_release_each_selected_owner_and_keep_borrowed_output_alive() {
    for (level, size) in [5, 6]
        .into_iter()
        .flat_map(|level| [0, 1, 6].map(|size| (level, size)))
    {
        let input = [0x51; 6];
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let frame = {
            let temporary = input;
            compress_at_level_in(&temporary[..size], level, &owner).unwrap()
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
                compress_at_level_in(&input[..size], level, owner).err(),
                Some(Error::AllocationFailed)
            );
            assert_eq!(monitor.snapshot().attempts, ordinal);
            assert_eq!(monitor.snapshot().failures, 1);
            monitor.assert_released();
        }
    }
}
