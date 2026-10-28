use core::num::NonZeroU64;
use zstdoxide::{
    Error, FrameKind, compress, compress_into, compress_into_in, compression_bound, frame_info,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

#[test]
fn public_wrappers_emit_one_checksummed_known_size_frame_and_preserve_the_tail() {
    for input in [
        &b""[..],
        &b"a"[..],
        &b"public output public output public output"[..],
    ] {
        let expected = compress(input).unwrap();
        let mut output = vec![0xa5; compression_bound(input.len()).unwrap() + 2];
        let len = output.len();
        let size = compress_into(input, &mut output[1..len - 1]).unwrap();
        assert_eq!(&output[1..1 + size], expected);
        let info = frame_info(&output[1..1 + size]).unwrap();
        assert_eq!(info.encoded_size(), size);
        assert_eq!(info.content_size(), Some(input.len() as u64));
        assert!(matches!(info.kind(), FrameKind::Standard(header) if header.has_checksum));
        assert_eq!(output[0], 0xa5);
        assert!(output[1 + size..].iter().all(|&b| b == 0xa5));
    }
}

#[test]
fn public_short_destinations_preserve_guards_and_immutable_input() {
    let input = vec![0x35; 256];
    let expected = compress(&input).unwrap();
    for capacity in 0..=expected.len() {
        let mut output = vec![0xa5; capacity + 2];
        let result = compress_into(&input, &mut output[1..1 + capacity]);
        assert_eq!(
            result,
            if capacity == expected.len() {
                Ok(capacity)
            } else {
                Err(Error::ResourceLimit)
            }
        );
        assert_eq!((output[0], output[capacity + 1]), (0xa5, 0xa5));
        assert!(input.iter().all(|&b| b == 0x35));
    }
}

#[test]
fn public_borrowed_nonclone_allocator_releases_scratch_and_recovers_after_failure() {
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let monitor = owner.monitor();
    let input = b"caller owned compression caller owned compression";
    let expected = compress(input).unwrap();
    let mut output = vec![0xa5; expected.len() + 2];
    monitor.set_fail_at(NonZeroU64::new(1));
    assert_eq!(
        compress_into_in(input, &mut output[1..1 + expected.len()], &owner),
        Err(Error::AllocationFailed)
    );
    monitor.assert_released();
    assert_eq!((output[0], output[expected.len() + 1]), (0xa5, 0xa5));
    monitor.clear_failure();
    assert_eq!(
        compress_into_in(input, &mut output[1..1 + expected.len()], &owner),
        Ok(expected.len())
    );
    assert_eq!(&output[1..1 + expected.len()], expected);
    monitor.assert_released();
    assert_eq!(monitor.snapshot().identities_created, 1);
}

#[test]
fn public_header_capacity_rejection_precedes_any_scratch_allocation() {
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    let mut output = [0xa5; 5];
    assert_eq!(
        compress_into_in(b"", &mut output, owner),
        Err(Error::ResourceLimit)
    );
    assert_eq!(output, [0xa5; 5]);
    assert_eq!(monitor.snapshot().attempts, 0);
    monitor.assert_released();
}
