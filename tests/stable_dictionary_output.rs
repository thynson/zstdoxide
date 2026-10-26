//! Persistent dictionary ownership and fixed fragmented destinations.
#![forbid(unsafe_code)]
use std::alloc::{Allocator, Global};
use std::num::NonZeroU64;
use zstdoxide::{
    DecodeOptions, Dictionary, DictionaryHandle, Error, SharedDictionary, SliceDecoder,
    StreamStatus, decompress_into_with_dictionary,
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
fn feed<A: Allocator + Clone, D: DictionaryHandle>(
    decoder: &mut SliceDecoder<'_, A, D>,
    frame: &[u8],
    chunk: usize,
) -> Result<usize, Error> {
    assert!(chunk > 0);
    let mut position = 0;
    let mut frames = 0;
    let mut calls = 0;
    while position < frame.len() {
        let end = position + chunk.min(frame.len() - position);
        while position < end {
            calls += 1;
            assert!(calls <= 2 * frame.len() + 16);
            let before_in = decoder.total_input();
            let before_out = decoder.total_output();
            let p = decoder.process(&frame[position..end])?;
            assert!(p.consumed <= end - position);
            assert!(p.consumed > 0 || p.produced > 0 || p.status == StreamStatus::FrameFinished);
            assert_eq!(decoder.total_input() - before_in, p.consumed as u64);
            assert_eq!(decoder.total_output() - before_out, p.produced as u64);
            assert!(matches!(
                p.status,
                StreamStatus::NeedInput | StreamStatus::FrameFinished
            ));
            frames += usize::from(p.status == StreamStatus::FrameFinished);
            position += p.consumed;
        }
    }
    Ok(frames)
}
#[test]
fn every_prepared_split_and_reset_preserves_fixed_storage_and_dictionary_selection() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap().into_shared();
    for split in 0..=FRAME.len() {
        let mut output = vec![GUARD; SOURCE.len() + 18];
        let pointer = output.as_ptr();
        let capacity = output.capacity();
        let mut decoder = SliceDecoder::with_dictionary(
            &mut output[1..SOURCE.len() + 17],
            DecodeOptions::new(SOURCE.len()),
            dictionary.clone(),
        );
        assert_eq!(decoder.retained_capacity(), 0);
        assert!(decoder.dictionary_retained_capacity() >= DICTIONARY.len());
        assert_eq!(
            decoder.process(&[]).unwrap().status,
            StreamStatus::NeedInput
        );
        feed(&mut decoder, &FRAME[..split], usize::MAX).unwrap();
        feed(&mut decoder, &FRAME[split..], usize::MAX).unwrap();
        decoder.finish().unwrap();
        assert_eq!(decoder.output().unwrap(), SOURCE);
        let retained = decoder.retained_capacity();
        let selected = decoder.dictionary_retained_capacity();
        decoder.reset();
        assert_eq!(decoder.retained_capacity(), retained);
        assert_eq!(decoder.dictionary_retained_capacity(), selected);
        assert_eq!(decoder.total_output(), 0);
        assert!(decoder.output().unwrap().is_empty());
        feed(&mut decoder, FRAME, 7).unwrap();
        decoder.finish().unwrap();
        assert_eq!(decoder.output().unwrap(), SOURCE);
        drop(decoder);
        assert_eq!(output[0], GUARD);
        assert!(output[SOURCE.len() + 1..].iter().all(|&b| b == GUARD));
        assert_eq!(output.as_ptr(), pointer);
        assert_eq!(output.capacity(), capacity);
        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
    }
}
#[test]
fn consumed_shared_handle_keeps_independent_nonclone_dictionary_and_scratch_owners_alive() {
    let dictionary_owner = tracking().into_non_clone();
    let dm = dictionary_owner.monitor();
    let dictionary = Dictionary::from_bytes_in(DICTIONARY, &dictionary_owner)
        .unwrap()
        .into_shared();
    let weak = SharedDictionary::downgrade(&dictionary);
    let before = dm.snapshot();
    assert!(dm.fail_next());
    let scratch = tracking().into_non_clone();
    let sm = scratch.monitor();
    let mut output = vec![GUARD; SOURCE.len() + 8];
    let mut decoder = SliceDecoder::with_dictionary_in(
        &mut output,
        DecodeOptions::new(SOURCE.len()),
        dictionary,
        &scratch,
    );
    assert!(weak.upgrade().is_some());
    assert_eq!(decoder.retained_capacity(), 0);
    assert!(decoder.dictionary_retained_capacity() >= DICTIONARY.len());
    assert_eq!(sm.snapshot().attempts, 0);
    feed(&mut decoder, FRAME, 7).unwrap();
    decoder.finish().unwrap();
    assert_eq!(decoder.output().unwrap(), SOURCE);
    assert_eq!(dm.snapshot(), before);
    let capacity = decoder.retained_capacity();
    decoder.reset();
    assert_eq!(decoder.retained_capacity(), capacity);
    assert!(weak.upgrade().is_some());
    assert_eq!(dm.snapshot(), before);
    drop(decoder);
    assert!(weak.upgrade().is_none());
    dm.assert_released();
    sm.assert_released();
    assert!(output[SOURCE.len()..].iter().all(|&b| b == GUARD));
}
#[test]
fn every_scratch_failure_poisoning_reset_and_release_keeps_borrowed_dictionary_unchanged() {
    let dictionary_owner = tracking().into_non_clone();
    let dm = dictionary_owner.monitor();
    let dictionary = Dictionary::from_bytes_in(DICTIONARY, &dictionary_owner).unwrap();
    let before = dm.snapshot();
    let scratch = tracking().into_non_clone();
    let sm = scratch.monitor();
    let mut output = vec![GUARD; SOURCE.len() + 8];
    {
        let mut decoder = SliceDecoder::with_dictionary_in(
            &mut output,
            DecodeOptions::new(SOURCE.len()),
            dictionary.view(),
            &scratch,
        );
        feed(&mut decoder, FRAME, 7).unwrap();
        decoder.finish().unwrap();
    }
    let requests = sm.snapshot().attempts;
    assert!(requests > 0);
    sm.assert_released();
    for ordinal in 1..=requests {
        let scratch = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(ordinal),
            ..AllocatorConfig::default()
        })
        .unwrap()
        .into_non_clone();
        let monitor = scratch.monitor();
        output.fill(GUARD);
        let mut decoder = SliceDecoder::with_dictionary_in(
            &mut output,
            DecodeOptions::new(SOURCE.len()),
            dictionary.view(),
            &scratch,
        );
        assert_eq!(feed(&mut decoder, FRAME, 7), Err(Error::AllocationFailed));
        assert_eq!(monitor.snapshot().failures, 1);
        assert_eq!(monitor.snapshot().attempts, ordinal);
        assert_eq!(decoder.output(), Err(Error::InvalidState));
        assert_eq!(decoder.process(&[]), Err(Error::InvalidState));
        decoder.reset();
        feed(&mut decoder, FRAME, 7).unwrap();
        decoder.finish().unwrap();
        assert_eq!(decoder.output().unwrap(), SOURCE);
        drop(decoder);
        monitor.assert_released();
        assert_eq!(dm.snapshot(), before);
        assert!(output[SOURCE.len()..].iter().all(|&b| b == GUARD));
    }
    drop(dictionary);
    dm.assert_released();
}
#[test]
fn prepared_raw_rle_empty_and_skip_sessions_have_no_scratch_payload_requests() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let cases: &[(&[u8], &[u8])] = &[
        (&[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0], b""),
        (
            &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 25, 0, 0, b'a', b'b', b'c'],
            b"abc",
        ),
        (
            &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 7, 59, 0, 0, b'q'],
            b"qqqqqqq",
        ),
        (&[0x50, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 1, 2, 3], b""),
    ];
    for &(frame, expected) in cases {
        let scratch = tracking().into_non_clone();
        let monitor = scratch.monitor();
        assert!(monitor.fail_next());
        let mut output = [GUARD; 32];
        let mut decoder = SliceDecoder::with_dictionary_in(
            &mut output,
            DecodeOptions::new(expected.len()),
            dictionary.view(),
            &scratch,
        );
        assert!(decoder.dictionary_retained_capacity() > 0);
        assert_eq!(decoder.retained_capacity(), 0);
        feed(&mut decoder, frame, 1).unwrap();
        decoder.finish().unwrap();
        assert_eq!(decoder.output().unwrap(), expected);
        assert_eq!(decoder.retained_capacity(), 0);
        drop(decoder);
        assert_eq!(monitor.snapshot().attempts, 0);
        monitor.assert_released();
        assert!(output[expected.len()..].iter().all(|&b| b == GUARD));
    }
    let mut empty = [];
    let decoder = SliceDecoder::new(&mut empty, DecodeOptions::new(0));
    assert_eq!(decoder.retained_capacity(), 0);
    assert_eq!(decoder.dictionary_retained_capacity(), 0);
}
#[test]
fn truncated_and_mutated_frames_preserve_guards_and_match_one_shot_success() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap().into_shared();
    let mut output = vec![GUARD; SOURCE.len() + 2];
    for end in 0..FRAME.len() {
        output.fill(GUARD);
        let mut decoder = SliceDecoder::with_dictionary(
            &mut output[1..SOURCE.len() + 1],
            DecodeOptions::new(SOURCE.len()),
            dictionary.clone(),
        );
        let result = feed(&mut decoder, &FRAME[..end], 7).and_then(|_| decoder.finish());
        assert!(result.is_err());
        assert_eq!(decoder.output(), Err(Error::InvalidState));
        drop(decoder);
        assert_eq!(output[0], GUARD);
        assert_eq!(output[SOURCE.len() + 1], GUARD);
    }
    for index in 0..FRAME.len() {
        let mut changed = FRAME.to_vec();
        changed[index] ^= 0x80;
        output.fill(GUARD);
        let mut direct = vec![GUARD; SOURCE.len()];
        let expected = decompress_into_with_dictionary(
            &changed,
            &mut direct,
            &dictionary,
            DecodeOptions::new(SOURCE.len()),
        );
        let mut decoder = SliceDecoder::with_dictionary(
            &mut output[1..SOURCE.len() + 1],
            DecodeOptions::new(SOURCE.len()),
            dictionary.clone(),
        );
        let result = feed(&mut decoder, &changed, 7).and_then(|_| decoder.finish());
        assert_eq!(result.is_ok(), expected.is_ok(), "byte {index}");
        if let Ok(written) = expected {
            assert_eq!(decoder.output().unwrap(), &direct[..written]);
        } else {
            assert_eq!(decoder.output(), Err(Error::InvalidState));
        }
        drop(decoder);
        assert_eq!(output[0], GUARD);
        assert_eq!(output[SOURCE.len() + 1], GUARD);
    }
}
#[test]
fn persistent_concatenation_skips_and_exact_short_limits_keep_frame_history_separate() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let mut combined = FRAME.to_vec();
    combined.extend_from_slice(&[0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0]);
    combined.extend_from_slice(FRAME);
    combined.extend_from_slice(&[0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 25, 0, 0, b'e', b'n', b'd']);
    let expected = [SOURCE, SOURCE, b"end"].concat();
    for capacity in [0, expected.len() - 1, expected.len(), expected.len() + 17] {
        let mut output = vec![GUARD; capacity + 2];
        let mut decoder = SliceDecoder::with_dictionary_in(
            &mut output[1..capacity + 1],
            DecodeOptions::new(expected.len()),
            dictionary.view(),
            Global,
        );
        let result = feed(&mut decoder, &combined, 7).and_then(|frames| {
            decoder.finish()?;
            Ok(frames)
        });
        if capacity < expected.len() {
            assert_eq!(result, Err(Error::ResourceLimit));
            assert_eq!(decoder.output(), Err(Error::InvalidState));
        } else {
            assert_eq!(result, Ok(4));
            assert_eq!(decoder.output().unwrap(), expected);
        }
        drop(decoder);
        assert_eq!(output[0], GUARD);
        assert!(
            output[1 + capacity.min(expected.len())..]
                .iter()
                .all(|&b| b == GUARD)
        );
    }
    let mut output = [GUARD; 16];
    let mut decoder = SliceDecoder::with_dictionary_in(
        &mut output,
        DecodeOptions {
            max_output_size: 16,
            max_window_size: 0,
        },
        dictionary.view(),
        Global,
    );
    assert_eq!(decoder.process(FRAME), Err(Error::ResourceLimit));
    drop(decoder);
    assert_eq!(output, [GUARD; 16]);
}
