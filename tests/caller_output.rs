//! Direct destination contracts; native fixtures retain their upstream attribution.
#![forbid(unsafe_code)]

use std::num::NonZeroU64;
use zstdoxide::{
    DecodeOptions, Error, FrameHeader, compress, decompress_into, decompress_into_in,
    decompress_into_with_options, decompress_into_with_options_in, decompress_with_options,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

const HTTP: &[u8] = include_bytes!("upstream/data/generated/http.zst");
const HTTP_SOURCE: &[u8] = include_bytes!("upstream/data/golden-compression/http");
const LARGER: &[u8] = include_bytes!("upstream/data/generated/huffman-compressed-larger.zst");
const LARGER_SOURCE: &[u8] =
    include_bytes!("upstream/data/golden-compression/huffman-compressed-larger");
const GUARD: u8 = 0xa5;

fn raw(source: &[u8], known: bool, rle: bool) -> Vec<u8> {
    assert!(source.len() < 256);
    let mut frame = vec![
        0x28,
        0xb5,
        0x2f,
        0xfd,
        if known { 0x20 } else { 0 },
        if known { source.len() as u8 } else { 0 },
    ];
    let header = ((source.len() as u32) << 3) | 1 | if rle { 2 } else { 0 };
    frame.extend_from_slice(&header.to_le_bytes()[..3]);
    if rle {
        assert!(!source.is_empty() && source.iter().all(|b| *b == source[0]));
        frame.push(source[0]);
    } else {
        frame.extend_from_slice(source);
    }
    frame
}
fn skipped() -> Vec<u8> {
    vec![0x5f, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 1, 2, 3]
}

#[test]
fn native_frames_decode_directly_at_exact_capacity_and_preserve_suffix() {
    for (frame, source) in [(HTTP, HTTP_SOURCE), (LARGER, LARGER_SOURCE)] {
        for extra in [0, 1, 31] {
            let mut output = vec![GUARD; source.len() + extra + 2];
            let pointer = output.as_ptr();
            let capacity = output.capacity();
            assert_eq!(
                decompress_into(frame, &mut output[1..1 + source.len() + extra]),
                Ok(source.len())
            );
            assert_eq!(&output[1..1 + source.len()], source);
            assert_eq!(output[0], GUARD);
            assert!(output[1 + source.len()..].iter().all(|b| *b == GUARD));
            assert_eq!(output.as_ptr(), pointer);
            assert_eq!(output.capacity(), capacity);
        }
        for size in [0, 1, source.len() - 1] {
            let mut output = vec![GUARD; size + 2];
            assert_eq!(
                decompress_into(frame, &mut output[1..1 + size]),
                Err(Error::ResourceLimit)
            );
            assert_eq!(output[0], GUARD);
            assert_eq!(output[size + 1], GUARD);
        }
    }
}

#[test]
fn raw_rle_empty_and_skippable_streams_need_no_payload_allocation() {
    for known in [false, true] {
        for (source, rle) in [
            (&b""[..], false),
            (&b"abc"[..], false),
            (&b"qqqqqqq"[..], true),
        ] {
            let frame = raw(source, known, rle);
            let owner = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let monitor = owner.monitor();
            let mut output = [GUARD; 32];
            assert_eq!(
                decompress_into_in(&frame, &mut output, &owner),
                Ok(source.len())
            );
            assert_eq!(&output[..source.len()], source);
            assert!(output[source.len()..].iter().all(|b| *b == GUARD));
            assert_eq!(monitor.snapshot().attempts, 0);
            monitor.assert_released();
        }
    }
    assert_eq!(decompress_into(&[], &mut []), Err(Error::UnexpectedEof));
    assert_eq!(decompress_into(&raw(&[], true, false), &mut []), Ok(0));
    assert_eq!(decompress_into(&raw(&[], false, false), &mut []), Ok(0));
    assert_eq!(decompress_into(&skipped(), &mut []), Ok(0));
}

#[test]
fn capacity_options_and_window_limits_are_independent() {
    for known in [false, true] {
        let frame = raw(b"hello", known, false);
        for capacity in [0, 4, 5, 9] {
            for requested in [0, 4, 5, usize::MAX] {
                let limit = capacity.min(requested);
                let mut output = [GUARD; 12];
                let result = decompress_into_with_options(
                    &frame,
                    &mut output[1..1 + capacity],
                    DecodeOptions::new(requested),
                );
                assert_eq!(
                    result,
                    if limit < 5 {
                        Err(Error::ResourceLimit)
                    } else {
                        Ok(5)
                    }
                );
                assert_eq!(output[0], GUARD);
                assert!(output[1 + limit..].iter().all(|b| *b == GUARD));
                if result.is_ok() {
                    assert_eq!(&output[1..6], b"hello");
                    assert!(output[6..].iter().all(|b| *b == GUARD));
                }
            }
        }
        let window = FrameHeader::parse(&frame).unwrap().0.window_size;
        let mut output = [GUARD; 8];
        assert_eq!(
            decompress_into_with_options(
                &frame,
                &mut output,
                DecodeOptions {
                    max_output_size: usize::MAX,
                    max_window_size: window - 1
                }
            ),
            Err(Error::ResourceLimit)
        );
        assert_eq!(output, [GUARD; 8]);
    }
}

#[test]
fn concatenation_and_skips_share_only_the_destination_allowance() {
    let mut frame = raw(b"abc", false, false);
    frame.extend(skipped());
    frame.extend(raw(b"dddd", true, true));
    let mut output = [GUARD; 12];
    assert_eq!(decompress_into(&frame, &mut output), Ok(7));
    assert_eq!(&output[..7], b"abcdddd");
    assert_eq!(&output[7..], &[GUARD; 5]);
    output.fill(GUARD);
    assert_eq!(
        decompress_into_with_options(&frame, &mut output, DecodeOptions::new(6)),
        Err(Error::ResourceLimit)
    );
    assert_eq!(&output[..3], b"abc");
    assert_eq!(&output[6..], &[GUARD; 6]);
}

#[test]
fn malformed_and_truncated_frames_preserve_owned_error_precedence_and_tail() {
    let encoded = compress(b"checksum checksum checksum").unwrap();
    assert!(FrameHeader::parse(&encoded).unwrap().0.has_checksum);
    let mut bad_checksum = encoded.clone();
    *bad_checksum.last_mut().unwrap() ^= 1;
    let mut dictionary = raw(b"abc", true, false);
    dictionary[4] |= 1;
    dictionary.insert(5, 1);
    assert_eq!(
        decompress_into(&bad_checksum, &mut [0; 64]),
        Err(Error::ChecksumMismatch)
    );
    assert_eq!(
        decompress_into(&dictionary, &mut [0; 64]),
        Err(Error::DictionaryRequired)
    );
    let mut cases = vec![vec![], vec![0, 0, 0, 0], bad_checksum, dictionary];
    for frame in [HTTP, &encoded, &raw(b"abc", false, false)] {
        for end in (0..frame.len()).step_by((frame.len() / 31).max(1)) {
            cases.push(frame[..end].to_vec());
        }
    }
    for frame in cases {
        for limit in [0, 2, 64, HTTP_SOURCE.len()] {
            let options = DecodeOptions::new(limit);
            let expected = decompress_with_options(&frame, options).map(|v| v.len());
            let mut output = vec![GUARD; limit + 8];
            assert_eq!(
                decompress_into_with_options(&frame, &mut output, options),
                expected
            );
            assert!(output[limit..].iter().all(|b| *b == GUARD));
        }
    }
}

#[test]
fn owned_and_borrowed_scratch_allocators_release_every_owner() {
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    let mut output = vec![GUARD; HTTP_SOURCE.len() + 8];
    let pointer = output.as_ptr();
    assert_eq!(
        decompress_into_in(HTTP, &mut output, owner),
        Ok(HTTP_SOURCE.len())
    );
    monitor.assert_released();
    assert_eq!(
        monitor.snapshot().owners_dropped,
        monitor.snapshot().identities_created
    );
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(&output[..HTTP_SOURCE.len()], HTTP_SOURCE);
    assert_eq!(&output[HTTP_SOURCE.len()..], &[GUARD; 8]);
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let monitor = owner.monitor();
    assert_eq!(
        decompress_into_with_options_in(
            LARGER,
            &mut vec![0; LARGER_SOURCE.len()],
            DecodeOptions::new(LARGER_SOURCE.len()),
            &owner
        ),
        Ok(LARGER_SOURCE.len())
    );
    monitor.assert_released();
    assert_eq!(monitor.snapshot().owners_dropped, 0);
}

#[test]
fn each_scratch_allocation_failure_releases_storage_and_preserves_guard_bytes() {
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    let mut output = vec![GUARD; HTTP_SOURCE.len() + 8];
    decompress_into_in(HTTP, &mut output, owner).unwrap();
    let requests = monitor.snapshot().attempts;
    assert!(requests > 0);
    monitor.assert_released();
    for ordinal in 1..=requests {
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(ordinal),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        output.fill(GUARD);
        assert_eq!(
            decompress_into_with_options_in(
                HTTP,
                &mut output,
                DecodeOptions::new(HTTP_SOURCE.len()),
                owner
            ),
            Err(Error::AllocationFailed)
        );
        assert_eq!(monitor.snapshot().attempts, ordinal);
        assert_eq!(monitor.snapshot().failures, 1);
        monitor.assert_released();
        assert_eq!(&output[HTTP_SOURCE.len()..], &[GUARD; 8]);
    }
}
