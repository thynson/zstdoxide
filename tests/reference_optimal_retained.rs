//! Additive retained Optimal public source and allocator boundaries.
#![forbid(unsafe_code)]
use core::num::NonZeroU64;
use zstdoxide::{
    CompressionParameterRequest as Request, Error, ReferenceStrategy, compress_with_parameters,
    compress_with_parameters_in, decompress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
fn request() -> Request {
    let mut r = Request::with_level(11);
    r.set_strategy(Some(ReferenceStrategy::Optimal));
    r.set_window_log(16).unwrap();
    r.set_hash_log(6).unwrap();
    r.set_chain_log(6).unwrap();
    r.set_search_log(1).unwrap();
    r.set_min_match(3).unwrap();
    r.set_target_length(1).unwrap();
    r
}
#[test]
fn retained_public_balanced_boundaries_and_minimum_target_modes() {
    for size in [16385, 24576, 24577, 32768] {
        for kind in 0..3 {
            let mut seed = 0x9e3779b9_u32;
            let source: Vec<_> = (0..size)
                .map(|i| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    match kind {
                        0 => b'a',
                        1 => b"abcXabcYabcZabcd"[i % 15],
                        _ => seed as u8,
                    }
                })
                .collect();
            for (minimum, target) in [(3, 0), (3, 1), (3, 4095), (4, 1), (5, 1), (6, 1), (7, 4095)]
            {
                let mut r = request();
                r.set_min_match(minimum).unwrap();
                r.set_target_length(target).unwrap();
                let before = r;
                let frame = compress_with_parameters(&source, &r).unwrap();
                assert_eq!(decompress(&frame, size).unwrap(), source);
                assert_eq!(r, before);
                assert!(frame.len() <= size + 7 + 3 * size.div_ceil(8192));
                let (_, mut position) = zstdoxide::FrameHeader::parse(&frame).unwrap();
                let mut count = 0;
                loop {
                    let word = u32::from_le_bytes([
                        frame[position],
                        frame[position + 1],
                        frame[position + 2],
                        0,
                    ]);
                    position += 3;
                    let kind = (word >> 1) & 3;
                    assert_ne!(kind, 3);
                    position += if kind == 1 { 1 } else { (word >> 3) as usize };
                    count += 1;
                    if word & 1 != 0 {
                        break;
                    }
                }
                assert_eq!(position, frame.len());
                assert_eq!(count, size.div_ceil(8192));
            }
        }
    }
}
#[test]
fn retained_public_early_rejections_and_output_owner_lifetime() {
    let source = vec![b'a'; 32769];
    for (size, dimension) in [
        (32769, 0),
        (32768, 1),
        (32768, 2),
        (32768, 3),
        (32768, 4),
        (32768, 5),
        (32768, 6),
        (32768, 7),
    ] {
        let mut r = request();
        match dimension {
            0 => {}
            1 => r.set_window_log(14).unwrap(),
            2 => r.set_hash_log(13).unwrap(),
            3 => r.set_chain_log(13).unwrap(),
            4 => r.set_search_log(7).unwrap(),
            5 => r.set_target_length(4096).unwrap(),
            6 => {
                r.set_strategy(Some(ReferenceStrategy::Ultra));
                r.set_window_log(14).unwrap();
            }
            _ => r.set_strategy(Some(ReferenceStrategy::Ultra2)),
        }
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(1),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let m = owner.monitor();
        assert_eq!(
            compress_with_parameters_in(&source[..size], &r, owner).err(),
            Some(Error::ResourceLimit)
        );
        assert_eq!(m.snapshot().attempts, 0);
        assert_eq!(m.snapshot().identities_created, 1);
        m.assert_released();
    }
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let monitor = owner.monitor();
    let frame = {
        let input = vec![b'x'; 16385];
        compress_with_parameters_in(&input, &request(), &owner).unwrap()
    };
    assert_eq!(monitor.snapshot().identities_created, 1);
    assert_eq!(monitor.snapshot().live_blocks, 1);
    assert_eq!(decompress(&frame, 16385).unwrap(), vec![b'x'; 16385]);
    drop(frame);
    monitor.assert_released();
}
