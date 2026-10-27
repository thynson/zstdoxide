//! One-buffer dictionary decoding with independent immutable owners and scratch.
#![forbid(unsafe_code)]
use std::{alloc::Global, num::NonZeroU64};
use zstdoxide::{
    DecodeOptions, Dictionary, Error, SharedDictionary, compress,
    decompress_in_place_with_dictionary, decompress_in_place_with_dictionary_in,
    decompress_into_with_dictionary_in, decompression_margin,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
const DICTIONARY: &[u8] =
    include_bytes!("upstream/data/golden-dictionaries/http-dict-missing-symbols");
const FRAME: &[u8] = include_bytes!("upstream/data/generated/http-dictionary.zst");
const SOURCE: &[u8] = include_bytes!("upstream/data/golden-compression/http");
fn tracking() -> TrackingAllocator {
    TrackingAllocator::new(AllocatorConfig::default()).unwrap()
}
fn layout(frame: &[u8], capacity: usize) -> (Vec<u8>, usize) {
    assert!(frame.len() <= capacity);
    let start = capacity - frame.len();
    let mut bytes = vec![0xa5; capacity + 64];
    bytes[32 + start..32 + capacity].copy_from_slice(frame);
    (bytes, start)
}
fn raw(source: &[u8], rle: bool) -> Vec<u8> {
    let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0x20, source.len() as u8];
    frame.extend_from_slice(
        &(((source.len() as u32) << 3) | 1 | if rle { 2 } else { 0 }).to_le_bytes()[..3],
    );
    if rle {
        frame.push(source.first().copied().unwrap_or(77));
    } else {
        frame.extend_from_slice(source);
    }
    frame
}

#[test]
fn shared_prepared_dictionary_exact_margin_and_limits_preserve_buffer_and_owner() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap().into_shared();
    for extra in [0, 1, 31] {
        let capacity =
            (SOURCE.len() + decompression_margin(FRAME).unwrap()).max(FRAME.len()) + extra;
        for limit in [0, 1, SOURCE.len() - 1, SOURCE.len(), usize::MAX] {
            let (mut bytes, start) = layout(FRAME, capacity);
            let before = bytes.clone();
            let pointer = bytes.as_ptr();
            let allocation = bytes.capacity();
            let result = decompress_in_place_with_dictionary(
                &mut bytes[32..32 + capacity],
                start,
                &dictionary,
                DecodeOptions::new(limit),
            );
            assert_eq!(
                result,
                if limit < SOURCE.len() {
                    Err(Error::ResourceLimit)
                } else {
                    Ok(SOURCE.len())
                }
            );
            assert_eq!(&bytes[..32], &before[..32]);
            assert_eq!(
                &bytes[32 + limit.min(capacity)..],
                &before[32 + limit.min(capacity)..]
            );
            if result.is_ok() {
                assert_eq!(&bytes[32..32 + SOURCE.len()], SOURCE);
                assert_eq!(&bytes[32 + SOURCE.len()..], &before[32 + SOURCE.len()..]);
            }
            assert_eq!(bytes.as_ptr(), pointer);
            assert_eq!(bytes.capacity(), allocation);
            assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
        }
    }
}

#[test]
fn borrowed_nonclone_dictionary_and_scratch_owners_never_adopt_each_others_storage() {
    let dictionary_owner = tracking().into_non_clone();
    let dm = dictionary_owner.monitor();
    let dictionary = Dictionary::from_bytes_in(DICTIONARY, &dictionary_owner).unwrap();
    let dictionary_before = dm.snapshot();
    assert!(dm.fail_next());
    let scratch = tracking().into_non_clone();
    let sm = scratch.monitor();
    let capacity = SOURCE.len() + decompression_margin(FRAME).unwrap();
    let (mut bytes, start) = layout(FRAME, capacity);
    for _ in 0..2 {
        bytes[32 + start..32 + capacity].copy_from_slice(FRAME);
        assert_eq!(
            decompress_in_place_with_dictionary_in(
                &mut bytes[32..32 + capacity],
                start,
                &dictionary.view(),
                DecodeOptions::new(SOURCE.len()),
                &scratch
            ),
            Ok(SOURCE.len())
        );
        assert_eq!(&bytes[32..32 + SOURCE.len()], SOURCE);
        sm.assert_released();
        assert_eq!(sm.snapshot().identities_created, 1);
        assert_eq!(dm.snapshot(), dictionary_before);
    }
    // Global scratch and a separately typed dictionary allocator also compose.
    bytes[32 + start..32 + capacity].copy_from_slice(FRAME);
    assert_eq!(
        decompress_in_place_with_dictionary_in(
            &mut bytes[32..32 + capacity],
            start,
            &dictionary.view(),
            DecodeOptions::new(SOURCE.len()),
            Global
        ),
        Ok(SOURCE.len())
    );
    assert_eq!(dm.snapshot(), dictionary_before);
    drop(dictionary);
    dm.assert_released();
}

#[test]
fn every_scratch_failure_ordinal_preserves_dictionary_and_releases_temporary_tables() {
    let dictionary_owner = tracking().into_non_clone();
    let dm = dictionary_owner.monitor();
    let dictionary = Dictionary::from_bytes_in(DICTIONARY, &dictionary_owner).unwrap();
    let dictionary_before = dm.snapshot();
    let capacity = SOURCE.len() + decompression_margin(FRAME).unwrap();
    let (mut bytes, start) = layout(FRAME, capacity);
    let before = bytes.clone();
    let scratch = tracking();
    let sm = scratch.monitor();
    assert_eq!(
        decompress_in_place_with_dictionary_in(
            &mut bytes[32..32 + capacity],
            start,
            &dictionary.view(),
            DecodeOptions::new(SOURCE.len()),
            scratch
        ),
        Ok(SOURCE.len())
    );
    let requests = sm.snapshot().attempts;
    assert!(requests > 0);
    sm.assert_released();
    for ordinal in 1..=requests {
        bytes.copy_from_slice(&before);
        let scratch = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(ordinal),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let sm = scratch.monitor();
        assert_eq!(
            decompress_in_place_with_dictionary_in(
                &mut bytes[32..32 + capacity],
                start,
                &dictionary.view(),
                DecodeOptions::new(SOURCE.len()),
                scratch
            ),
            Err(Error::AllocationFailed),
            "ordinal {ordinal}"
        );
        assert_eq!(sm.snapshot().attempts, ordinal);
        assert_eq!(sm.snapshot().failures, 1);
        sm.assert_released();
        assert_eq!(dm.snapshot(), dictionary_before);
        assert_eq!(&bytes[..32], &before[..32]);
        assert_eq!(&bytes[32 + SOURCE.len()..], &before[32 + SOURCE.len()..]);
    }
    drop(dictionary);
    dm.assert_released();
}

#[test]
fn selected_dictionary_adds_no_payload_allocation_to_raw_rle_or_skippable_frames() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    for (frame, expected) in [
        (raw(b"hello", false), b"hello".as_slice()),
        (raw(b"qqqqqq", true), b"qqqqqq".as_slice()),
        (raw(b"", false), b"".as_slice()),
        (raw(b"", true), b"".as_slice()),
        (vec![0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0], b"".as_slice()),
    ] {
        let capacity = frame.len() + expected.len() + 16;
        let (mut bytes, start) = layout(&frame, capacity);
        let before = bytes.clone();
        let scratch = tracking().into_non_clone();
        let sm = scratch.monitor();
        assert!(sm.fail_next());
        assert_eq!(
            decompress_in_place_with_dictionary_in(
                &mut bytes[32..32 + capacity],
                start,
                &dictionary.view(),
                DecodeOptions::new(expected.len()),
                &scratch
            ),
            Ok(expected.len())
        );
        assert_eq!(&bytes[32..32 + expected.len()], expected);
        assert_eq!(
            &bytes[32 + expected.len()..],
            &before[32 + expected.len()..]
        );
        assert_eq!(sm.snapshot().attempts, 0);
        sm.assert_released();
    }
}

#[test]
fn concatenated_frames_reset_entropy_and_history_while_skips_preserve_selection() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let plain = compress(b"independent plain frame").unwrap();
    let mut frames = Vec::new();
    let mut expected = Vec::new();
    for _ in 0..2 {
        frames.extend_from_slice(&[0x5f, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 1, 2, 3]);
        frames.extend_from_slice(FRAME);
        frames.extend_from_slice(&plain);
        expected.extend_from_slice(SOURCE);
        expected.extend_from_slice(b"independent plain frame");
    }
    let capacity = (expected.len() + decompression_margin(&frames).unwrap()).max(frames.len());
    let (mut bytes, start) = layout(&frames, capacity);
    let before = bytes.clone();
    assert_eq!(
        decompress_in_place_with_dictionary_in(
            &mut bytes[32..32 + capacity],
            start,
            &dictionary.view(),
            DecodeOptions::new(expected.len()),
            Global
        ),
        Ok(expected.len())
    );
    assert_eq!(&bytes[32..32 + expected.len()], expected);
    assert_eq!(&bytes[..32], &before[..32]);
    assert_eq!(
        &bytes[32 + expected.len()..],
        &before[32 + expected.len()..]
    );
}

#[test]
fn invalid_bounds_windows_ids_and_empty_input_reject_before_output() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let capacity = SOURCE.len() + decompression_margin(FRAME).unwrap();
    let (mut bytes, start) = layout(FRAME, capacity);
    let before = bytes.clone();
    for position in [capacity + 1, usize::MAX] {
        assert_eq!(
            decompress_in_place_with_dictionary_in(
                &mut bytes[32..32 + capacity],
                position,
                &dictionary.view(),
                DecodeOptions::new(SOURCE.len()),
                Global
            ),
            Err(Error::ResourceLimit)
        );
        assert_eq!(bytes, before);
    }
    assert_eq!(
        decompress_in_place_with_dictionary_in(
            &mut bytes[32..32 + capacity],
            start,
            &dictionary.view(),
            DecodeOptions {
                max_output_size: SOURCE.len(),
                max_window_size: 0
            },
            Global
        ),
        Err(Error::ResourceLimit)
    );
    assert_eq!(bytes, before);
    assert_eq!(
        decompress_in_place_with_dictionary_in(
            &mut [],
            0,
            &dictionary.view(),
            DecodeOptions::new(0),
            Global
        ),
        Err(Error::UnexpectedEof)
    );
    let wrong = Dictionary::raw(b"wrong").unwrap();
    let frame = include_bytes!("upstream/dictionaries/formatted-id.zst");
    let mut bytes = frame.to_vec();
    assert_eq!(
        decompress_in_place_with_dictionary_in(
            &mut bytes,
            0,
            &wrong.view(),
            DecodeOptions::new(0),
            Global
        ),
        Err(Error::DictionaryMismatch)
    );
    assert_eq!(&bytes, frame);
}

#[test]
fn every_truncation_and_selected_mutations_match_direct_rejection_and_preserve_limits() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let mut cases: Vec<_> = (0..FRAME.len()).map(|end| FRAME[..end].to_vec()).collect();
    for index in [0, 4, 5, 6, 7, FRAME.len() / 2, FRAME.len() - 1] {
        for bit in [0, 3, 7] {
            let mut changed = FRAME.to_vec();
            changed[index] ^= 1 << bit;
            cases.push(changed);
        }
    }
    for frame in cases {
        let limit = SOURCE.len();
        let mut expected = vec![0xa5; limit];
        let reference = decompress_into_with_dictionary_in(
            &frame,
            &mut expected,
            &dictionary.view(),
            DecodeOptions::new(limit),
            Global,
        );
        let capacity = frame.len() + limit + 131072;
        let (mut bytes, start) = layout(&frame, capacity);
        let before = bytes.clone();
        let result = decompress_in_place_with_dictionary_in(
            &mut bytes[32..32 + capacity],
            start,
            &dictionary.view(),
            DecodeOptions::new(limit),
            Global,
        );
        match reference {
            Ok(size) => {
                assert_eq!(result, Ok(size));
                assert_eq!(&bytes[32..32 + size], &expected[..size]);
                assert_eq!(&bytes[32 + size..], &before[32 + size..]);
            }
            Err(_) => assert!(result.is_err()),
        }
        assert_eq!(&bytes[..32], &before[..32]);
        assert_eq!(&bytes[32 + limit..], &before[32 + limit..]);
    }
}
