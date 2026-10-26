//! Caller-owned dictionary destinations and independent allocator ownership.
#![forbid(unsafe_code)]
use std::alloc::Global;
use std::num::NonZeroU64;
use zstdoxide::{
    DecodeOptions, Dictionary, Error, SharedDictionary, compress, decompress_into_with_dictionary,
    decompress_into_with_dictionary_in, decompress_with_dictionary_in,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
const DICTIONARY: &[u8] =
    include_bytes!("upstream/data/golden-dictionaries/http-dict-missing-symbols");
const FRAME: &[u8] = include_bytes!("upstream/data/generated/http-dictionary.zst");
const SOURCE: &[u8] = include_bytes!("upstream/data/golden-compression/http");
const GUARD: u8 = 0xa5;
fn tracking() -> TrackingAllocator {
    TrackingAllocator::new(AllocatorConfig::default()).unwrap()
}

#[test]
fn public_exact_short_and_options_limits_preserve_destination_identity_and_guards() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap().into_shared();
    for extra in [0, 1, 31] {
        let mut output = vec![GUARD; SOURCE.len() + extra + 2];
        let pointer = output.as_ptr();
        let capacity = output.capacity();
        assert_eq!(
            decompress_into_with_dictionary(
                FRAME,
                &mut output[1..SOURCE.len() + extra + 1],
                &dictionary,
                DecodeOptions::new(usize::MAX)
            ),
            Ok(SOURCE.len())
        );
        assert_eq!(&output[1..SOURCE.len() + 1], SOURCE);
        assert_eq!(output[0], GUARD);
        assert!(output[SOURCE.len() + 1..].iter().all(|&b| b == GUARD));
        assert_eq!(output.as_ptr(), pointer);
        assert_eq!(output.capacity(), capacity);
    }
    for capacity in [0, 1, SOURCE.len() - 1, SOURCE.len() + 17] {
        for allowance in [0, SOURCE.len() - 1, SOURCE.len()] {
            let mut output = vec![GUARD; capacity + 2];
            let limit = capacity.min(allowance);
            let result = decompress_into_with_dictionary(
                FRAME,
                &mut output[1..capacity + 1],
                &dictionary,
                DecodeOptions::new(allowance),
            );
            assert_eq!(
                result,
                if limit < SOURCE.len() {
                    Err(Error::ResourceLimit)
                } else {
                    Ok(SOURCE.len())
                }
            );
            assert_eq!(output[0], GUARD);
            assert!(output[limit + 1..].iter().all(|&b| b == GUARD));
        }
    }
    let mut output = vec![GUARD; SOURCE.len() + 2];
    assert_eq!(
        decompress_into_with_dictionary(
            FRAME,
            &mut output[1..SOURCE.len() + 1],
            &dictionary,
            DecodeOptions {
                max_output_size: SOURCE.len(),
                max_window_size: 0
            }
        ),
        Err(Error::ResourceLimit)
    );
    assert!(output.iter().all(|&b| b == GUARD));
    assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
}

#[test]
fn borrowed_nonclone_dictionary_and_scratch_owners_remain_independent() {
    let dictionary_owner = tracking().into_non_clone();
    let dm = dictionary_owner.monitor();
    let dictionary = Dictionary::from_bytes_in(DICTIONARY, &dictionary_owner).unwrap();
    let before = dm.snapshot();
    assert!(dm.fail_next());
    let scratch = tracking().into_non_clone();
    let sm = scratch.monitor();
    let mut output = vec![GUARD; SOURCE.len() + 8];
    for _ in 0..2 {
        assert_eq!(
            decompress_into_with_dictionary_in(
                FRAME,
                &mut output,
                &dictionary.view(),
                DecodeOptions::new(SOURCE.len()),
                &scratch
            ),
            Ok(SOURCE.len())
        );
        assert_eq!(&output[..SOURCE.len()], SOURCE);
        assert!(output[SOURCE.len()..].iter().all(|&b| b == GUARD));
        sm.assert_released();
        assert_eq!(dm.snapshot(), before);
    }
    drop(dictionary);
    dm.assert_released();
}

#[test]
fn every_scratch_allocation_failure_releases_storage_and_preserves_dictionary() {
    let dictionary_owner = tracking().into_non_clone();
    let dm = dictionary_owner.monitor();
    let dictionary = Dictionary::from_bytes_in(DICTIONARY, &dictionary_owner).unwrap();
    let before = dm.snapshot();
    let scratch = tracking();
    let sm = scratch.monitor();
    let mut output = vec![GUARD; SOURCE.len() + 8];
    decompress_into_with_dictionary_in(
        FRAME,
        &mut output,
        &dictionary.view(),
        DecodeOptions::new(SOURCE.len()),
        scratch,
    )
    .unwrap();
    let requests = sm.snapshot().attempts;
    assert!(requests > 0);
    sm.assert_released();
    for ordinal in 1..=requests {
        let scratch = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(ordinal),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = scratch.monitor();
        output.fill(GUARD);
        assert_eq!(
            decompress_into_with_dictionary_in(
                FRAME,
                &mut output,
                &dictionary.view(),
                DecodeOptions::new(SOURCE.len()),
                scratch
            ),
            Err(Error::AllocationFailed)
        );
        assert_eq!(monitor.snapshot().attempts, ordinal);
        assert_eq!(monitor.snapshot().failures, 1);
        monitor.assert_released();
        assert!(output[SOURCE.len()..].iter().all(|&b| b == GUARD));
        assert_eq!(dm.snapshot(), before);
    }
    drop(dictionary);
    dm.assert_released();
}

#[test]
fn raw_rle_empty_and_skippable_frames_allocate_no_codec_payload_with_prepared_dictionary() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let frames: &[(&[u8], &[u8])] = &[
        (&[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0], b""),
        (
            &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 25, 0, 0, b'a', b'b', b'c'],
            b"abc",
        ),
        (
            &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 7, 59, 0, 0, b'q'],
            b"qqqqqqq",
        ),
        (&[0x5f, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 1, 2, 3], b""),
    ];
    for &(frame, expected) in frames {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let mut output = [GUARD; 32];
        assert!(monitor.fail_next());
        assert_eq!(
            decompress_into_with_dictionary_in(
                frame,
                &mut output,
                &dictionary.view(),
                DecodeOptions::new(expected.len()),
                &owner
            ),
            Ok(expected.len())
        );
        assert_eq!(&output[..expected.len()], expected);
        assert!(output[expected.len()..].iter().all(|&b| b == GUARD));
        assert_eq!(monitor.snapshot().attempts, 0);
        monitor.assert_released();
    }
}

#[test]
fn truncations_mutations_and_concatenation_do_not_cross_output_allowance() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let mut output = vec![GUARD; SOURCE.len() + 2];
    for end in 0..FRAME.len() {
        output.fill(GUARD);
        assert!(
            decompress_into_with_dictionary_in(
                &FRAME[..end],
                &mut output[1..SOURCE.len() + 1],
                &dictionary.view(),
                DecodeOptions::new(SOURCE.len()),
                Global
            )
            .is_err()
        );
        assert_eq!(output[0], GUARD);
        assert_eq!(output[SOURCE.len() + 1], GUARD);
    }
    for index in 0..FRAME.len() {
        let mut changed = FRAME.to_vec();
        changed[index] ^= 0x80;
        output.fill(GUARD);
        let result = decompress_into_with_dictionary_in(
            &changed,
            &mut output[1..SOURCE.len() + 1],
            &dictionary.view(),
            DecodeOptions::new(SOURCE.len()),
            Global,
        );
        if let Ok(written) = result {
            assert_eq!(
                &output[1..written + 1],
                decompress_with_dictionary_in(
                    &changed,
                    &dictionary.view(),
                    DecodeOptions::new(SOURCE.len()),
                    Global
                )
                .unwrap()
            );
            assert!(output[written + 1..].iter().all(|&b| b == GUARD));
        }
        assert_eq!(output[0], GUARD);
        assert_eq!(output[SOURCE.len() + 1], GUARD);
    }
    let plain = compress(b"after").unwrap();
    let mut combined = FRAME.to_vec();
    combined.extend_from_slice(&[0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0]);
    combined.extend_from_slice(FRAME);
    combined.extend_from_slice(&plain);
    let expected = [SOURCE, SOURCE, b"after"].concat();
    let mut output = vec![GUARD; expected.len() + 8];
    assert_eq!(
        decompress_into_with_dictionary_in(
            &combined,
            &mut output,
            &dictionary.view(),
            DecodeOptions::new(expected.len()),
            Global
        ),
        Ok(expected.len())
    );
    assert_eq!(&output[..expected.len()], expected);
    assert!(output[expected.len()..].iter().all(|&b| b == GUARD));
}
