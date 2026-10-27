//! Suffix-input ownership, malformed bounds, and independent scratch ownership.
#![forbid(unsafe_code)]

use std::num::NonZeroU64;
use zstdoxide::{
    DecodeOptions, Error, FrameHeader, compress, decompress_in_place, decompress_in_place_in,
    decompress_in_place_with_options, decompress_in_place_with_options_in, decompress_with_options,
    decompression_margin,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

const HTTP: &[u8] = include_bytes!("upstream/data/generated/http.zst");
const HTTP_SOURCE: &[u8] = include_bytes!("upstream/data/golden-compression/http");
const LARGER: &[u8] = include_bytes!("upstream/data/generated/huffman-compressed-larger.zst");
const LARGER_SOURCE: &[u8] =
    include_bytes!("upstream/data/golden-compression/huffman-compressed-larger");
const GUARD: u8 = 0xa5;

fn layout(frame: &[u8], capacity: usize) -> (Vec<u8>, usize) {
    assert!(frame.len() <= capacity);
    let start = capacity - frame.len();
    let mut buffer = vec![GUARD; capacity + 64];
    buffer[32 + start..32 + capacity].copy_from_slice(frame);
    (buffer, start)
}

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
    frame.extend_from_slice(
        &(((source.len() as u32) << 3) | 1 | if rle { 2 } else { 0 }).to_le_bytes()[..3],
    );
    if rle {
        frame.push(source.first().copied().unwrap_or(7));
    } else {
        frame.extend_from_slice(source);
    }
    frame
}

#[test]
fn fixed_allocation_exact_margin_and_subslice_end_preserve_every_unused_byte() {
    for (frame, source) in [(HTTP, HTTP_SOURCE), (LARGER, LARGER_SOURCE)] {
        for extra in [0, 1, 31] {
            let capacity =
                (source.len() + decompression_margin(frame).unwrap()).max(frame.len()) + extra;
            let (mut buffer, start) = layout(frame, capacity);
            let before = buffer.clone();
            let pointer = buffer.as_ptr();
            let original_capacity = buffer.capacity();
            assert_eq!(
                decompress_in_place(&mut buffer[32..32 + capacity], start),
                Ok(source.len())
            );
            assert_eq!(&buffer[32..32 + source.len()], source);
            assert_eq!(&buffer[..32], &before[..32]);
            assert_eq!(&buffer[32 + source.len()..], &before[32 + source.len()..]);
            assert_eq!(buffer.as_ptr(), pointer);
            assert_eq!(buffer.capacity(), original_capacity);
        }
    }
}

#[test]
fn raw_rle_empty_and_skips_never_request_codec_payload_allocations() {
    let mut cases = vec![vec![0x5f, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 1, 2, 3]];
    for known in [false, true] {
        for (source, rle) in [
            (&b""[..], false),
            (&b""[..], true),
            (&b"abc"[..], false),
            (&b"qqqqqqq"[..], true),
        ] {
            cases.push(raw(source, known, rle));
        }
    }
    for frame in cases {
        let source = decompress_with_options(&frame, DecodeOptions::new(100)).unwrap();
        let capacity = frame.len() + source.len() + 32;
        let (mut buffer, start) = layout(&frame, capacity);
        let before = buffer.clone();
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        assert!(monitor.fail_next());
        assert_eq!(
            decompress_in_place_in(&mut buffer[32..32 + capacity], start, &owner),
            Ok(source.len())
        );
        assert_eq!(&buffer[32..32 + source.len()], source);
        assert_eq!(&buffer[32 + source.len()..], &before[32 + source.len()..]);
        assert_eq!(monitor.snapshot().attempts, 0);
        monitor.assert_released();
    }
}

#[test]
fn each_scratch_failure_ordinal_releases_independent_owners_and_preserves_bounds() {
    let capacity = HTTP_SOURCE.len() + decompression_margin(HTTP).unwrap();
    let (mut buffer, start) = layout(HTTP, capacity);
    let before = buffer.clone();
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    assert_eq!(
        decompress_in_place_with_options_in(
            &mut buffer[32..32 + capacity],
            start,
            DecodeOptions::new(HTTP_SOURCE.len()),
            owner
        ),
        Ok(HTTP_SOURCE.len())
    );
    let attempts = monitor.snapshot().attempts;
    assert!(attempts > 3);
    monitor.assert_released();
    for ordinal in 1..=attempts {
        buffer.copy_from_slice(&before);
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(ordinal),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        assert_eq!(
            decompress_in_place_with_options_in(
                &mut buffer[32..32 + capacity],
                start,
                DecodeOptions::new(HTTP_SOURCE.len()),
                owner
            ),
            Err(Error::AllocationFailed),
            "ordinal {ordinal}"
        );
        assert_eq!(monitor.snapshot().attempts, ordinal);
        assert_eq!(monitor.snapshot().failures, 1);
        monitor.assert_released();
        assert_eq!(&buffer[..32], &before[..32]);
        assert_eq!(
            &buffer[32 + HTTP_SOURCE.len()..],
            &before[32 + HTTP_SOURCE.len()..]
        );
    }
    buffer.copy_from_slice(&before);
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let monitor = owner.monitor();
    assert_eq!(
        decompress_in_place_in(&mut buffer[32..32 + capacity], start, &owner),
        Ok(HTTP_SOURCE.len())
    );
    assert_eq!(monitor.snapshot().identities_created, 1);
    monitor.assert_released();
}

#[test]
fn every_small_layout_obeys_options_capacity_and_live_input_fences() {
    for known in [false, true] {
        for rle in [false, true] {
            let frame = raw(b"xxxxxxxxxxxx", known, rle);
            for gap in 0..=16 {
                let capacity = frame.len() + gap;
                for requested in [0, 1, 11, 12, 13, usize::MAX] {
                    let (mut buffer, start) = layout(&frame, capacity);
                    let before = buffer.clone();
                    let limit = requested.min(capacity);
                    let result = decompress_in_place_with_options(
                        &mut buffer[32..32 + capacity],
                        start,
                        DecodeOptions::new(requested),
                    );
                    let admitted = limit >= 12 && (!rle || 12 <= start + 9);
                    assert_eq!(
                        result,
                        if admitted {
                            Ok(12)
                        } else {
                            Err(Error::ResourceLimit)
                        }
                    );
                    assert_eq!(&buffer[..32], &before[..32]);
                    assert_eq!(&buffer[32 + limit..], &before[32 + limit..]);
                    if admitted {
                        assert_eq!(&buffer[32..44], b"xxxxxxxxxxxx");
                        assert_eq!(&buffer[44..], &before[44..]);
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_initial_ranges_windows_and_dictionary_ids_reject_without_writing() {
    let frame = raw(b"hello", true, false);
    let (mut buffer, start) = layout(&frame, 32);
    let before = buffer.clone();
    for index in [33, usize::MAX] {
        assert_eq!(
            decompress_in_place(&mut buffer[32..64], index),
            Err(Error::ResourceLimit)
        );
        assert_eq!(buffer, before);
    }
    assert_eq!(
        decompress_in_place(&mut buffer[32..64], 32),
        Err(Error::UnexpectedEof)
    );
    assert_eq!(buffer, before);
    assert_eq!(decompress_in_place(&mut [], 0), Err(Error::UnexpectedEof));
    let window = FrameHeader::parse(&frame).unwrap().0.window_size;
    assert_eq!(
        decompress_in_place_with_options(
            &mut buffer[32..64],
            start,
            DecodeOptions {
                max_output_size: usize::MAX,
                max_window_size: window - 1
            }
        ),
        Err(Error::ResourceLimit)
    );
    assert_eq!(buffer, before);
    let mut dictionary = frame;
    dictionary[4] |= 1;
    dictionary.insert(5, 1);
    let before = dictionary.clone();
    assert_eq!(
        decompress_in_place(&mut dictionary, 0),
        Err(Error::DictionaryRequired)
    );
    assert_eq!(dictionary, before);
}

#[test]
fn complete_truncations_and_deterministic_corruptions_preserve_allowance() {
    let frame = compress(b"checksum checksum checksum and a literal tail").unwrap();
    let mut cases: Vec<Vec<u8>> = (0..frame.len()).map(|n| frame[..n].to_vec()).collect();
    for index in 0..frame.len() {
        for bit in 0..8 {
            let mut mutated = frame.clone();
            mutated[index] ^= 1 << bit;
            cases.push(mutated);
        }
    }
    for frame in cases {
        for limit in [0, 2, 64, 512] {
            let options = DecodeOptions::new(limit);
            let expected = decompress_with_options(&frame, options);
            // Large gap removes overlap exhaustion; compare decoded bytes or
            // rejection, without imposing the existing API's error precedence.
            let capacity = frame.len() + 1024;
            let (mut buffer, start) = layout(&frame, capacity);
            let before = buffer.clone();
            let actual =
                decompress_in_place_with_options(&mut buffer[32..32 + capacity], start, options);
            match expected {
                Ok(source) => {
                    assert_eq!(actual, Ok(source.len()));
                    assert_eq!(&buffer[32..32 + source.len()], source);
                    assert_eq!(&buffer[32 + source.len()..], &before[32 + source.len()..]);
                }
                Err(_) => assert!(actual.is_err()),
            }
            assert_eq!(&buffer[..32], &before[..32]);
            assert_eq!(&buffer[32 + limit..], &before[32 + limit..]);
        }
    }
}

#[test]
fn frame_history_is_never_inherited_across_concatenation() {
    // No literals followed by a match: cannot use a preceding frame as history.
    let block = [0, 1, 0x54, 0, 0, 0, 1];
    let mut frame = raw(b"prior", true, false);
    frame.extend_from_slice(&[0x28, 0xb5, 0x2f, 0xfd, 0, 0]);
    frame.extend_from_slice(&(((block.len() as u32) << 3) | 5).to_le_bytes()[..3]);
    frame.extend_from_slice(&block);
    assert!(decompress_with_options(&frame, DecodeOptions::new(100)).is_err());
    let (mut buffer, start) = layout(&frame, 256);
    let before = buffer.clone();
    assert!(decompress_in_place(&mut buffer[32..288], start).is_err());
    assert_eq!(&buffer[32..37], b"prior");
    assert_eq!(&buffer[37..], &before[37..]);
}
