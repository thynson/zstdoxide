//! Prefix-once lifetime, boundary replacement and independent allocation owners.
#![forbid(unsafe_code)]
use std::alloc::{Allocator, Global};
use std::num::NonZeroU64;
use zstdoxide::{
    DecodeOptions, Dictionary, DictionaryHandle, Error, FrameHeader, SharedDictionary,
    SliceDecoder, StreamStatus,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
const DICTIONARY: &[u8] =
    include_bytes!("upstream/data/golden-dictionaries/http-dict-missing-symbols");
const FRAME: &[u8] = include_bytes!("upstream/data/generated/http-dictionary.zst");
const SOURCE: &[u8] = include_bytes!("upstream/data/golden-compression/http");
const EMPTY: &[u8] = &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0];
const GUARD: u8 = 0xa5;
fn feed<A: Allocator + Clone, D: DictionaryHandle>(
    d: &mut SliceDecoder<'_, A, D>,
    input: &[u8],
    chunk: usize,
) -> Result<(), Error> {
    let mut p = 0;
    let mut calls = 0;
    while p < input.len() {
        calls += 1;
        assert!(calls <= input.len() * 2 + 16);
        let end = p + chunk.min(input.len() - p);
        let before = (d.total_input(), d.total_output());
        let progress = d.process(&input[p..end])?;
        assert!(progress.consumed <= end - p);
        assert!(progress.consumed > 0 || progress.status == StreamStatus::FrameFinished);
        assert_eq!(d.total_input() - before.0, progress.consumed as u64);
        assert_eq!(d.total_output() - before.1, progress.produced as u64);
        assert_ne!(progress.status, StreamStatus::NeedOutput);
        p += progress.consumed;
    }
    Ok(())
}
#[test]
fn every_prepared_split_consumes_one_prefix_and_boundary_replacement_keeps_output() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap().into_shared();
    for split in 0..=FRAME.len() {
        let mut output = vec![GUARD; SOURCE.len() * 2 + 2];
        let mut d = SliceDecoder::with_prefix(
            &mut output[1..SOURCE.len() * 2 + 1],
            DecodeOptions::new(SOURCE.len() * 2),
            dictionary.clone(),
        );
        assert_eq!(d.process(&[]).unwrap().status, StreamStatus::NeedInput);
        feed(&mut d, &FRAME[..split], usize::MAX).unwrap();
        feed(&mut d, &FRAME[split..], usize::MAX).unwrap();
        assert_eq!(d.output().unwrap(), SOURCE);
        assert_eq!(d.dictionary_retained_capacity(), 0);
        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
        let before = (d.total_input(), d.total_output(), d.retained_capacity());
        d.set_prefix(dictionary.clone()).unwrap();
        assert_eq!(
            (d.total_input(), d.total_output(), d.retained_capacity()),
            before
        );
        feed(&mut d, FRAME, 7).unwrap();
        d.finish().unwrap();
        assert_eq!(d.output().unwrap(), [SOURCE, SOURCE].concat());
        assert_eq!(d.dictionary_retained_capacity(), 0);
        d.reset();
        assert_eq!(d.dictionary_retained_capacity(), 0);
        assert!(d.output().unwrap().is_empty());
        d.set_dictionary(Some(dictionary.clone())).unwrap();
        feed(&mut d, FRAME, 7).unwrap();
        d.finish().unwrap();
        d.reset();
        assert!(d.dictionary_retained_capacity() > 0);
        d.set_dictionary(None).unwrap();
        assert_eq!(d.dictionary_retained_capacity(), 0);
        drop(d);
        assert_eq!(output[0], GUARD);
        assert_eq!(*output.last().unwrap(), GUARD);
        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
    }
}
#[test]
fn each_truncated_prefix_claims_only_after_the_complete_valid_header() {
    let (_, header_size) = FrameHeader::parse(FRAME).unwrap();
    for end in 0..FRAME.len() {
        let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap().into_shared();
        let weak = SharedDictionary::downgrade(&dictionary);
        let mut output = vec![GUARD; SOURCE.len() + 2];
        let mut d = SliceDecoder::with_prefix(
            &mut output[1..SOURCE.len() + 1],
            DecodeOptions::new(SOURCE.len()),
            dictionary,
        );
        assert!(
            feed(&mut d, &FRAME[..end], 7)
                .and_then(|_| d.finish())
                .is_err()
        );
        assert_eq!(d.output(), Err(Error::InvalidState));
        assert!(weak.upgrade().is_some());
        d.reset();
        assert_eq!(weak.upgrade().is_none(), end >= header_size, "end={end}");
        assert_eq!(d.dictionary_retained_capacity() > 0, end < header_size);
        drop(d);
        assert!(weak.upgrade().is_none());
        assert_eq!(output[0], GUARD);
        assert_eq!(*output.last().unwrap(), GUARD);
    }
}
#[test]
fn replacement_rejection_preserves_counters_output_and_original_selection_at_every_phase() {
    let cases: &[&[u8]] = &[
        &[0x28, 0xb5],                                      // Partial header.
        &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 3],                 // Block header pending.
        &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 25, 0, 0, b'a'], // Raw body.
        &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 27, 0, 0],       // RLE byte.
        &[0x28, 0xb5, 0x2f, 0xfd, 0, 0, 21, 0, 0, 0],       // Compressed body.
        &[0x50, 0x2a, 0x4d, 0x18],                          // Skip length.
        &[0x50, 0x2a, 0x4d, 0x18, 2, 0, 0, 0, 7],           // Skip body.
        &[0x28, 0xb5, 0x2f, 0xfd, 0x24, 0, 1, 0, 0],        // Checksum.
        &[0],                                               // Failed magic.
    ];
    for &input in cases {
        let original = Dictionary::raw(b"original").unwrap().into_shared();
        let weak = SharedDictionary::downgrade(&original);
        let mut output = [GUARD; 32];
        let mut d = SliceDecoder::with_prefix(&mut output, DecodeOptions::new(16), original);
        let _ = d.process(input);
        let before = (
            d.total_input(),
            d.total_output(),
            d.retained_capacity(),
            d.dictionary_retained_capacity(),
            d.output().map(<[u8]>::to_vec),
        );
        for prefix in [false, true] {
            let next = Dictionary::raw(b"replacement").unwrap().into_shared();
            let rejected = SharedDictionary::downgrade(&next);
            let result = if prefix {
                d.set_prefix(next)
            } else {
                d.set_dictionary(Some(next))
            };
            assert_eq!(result, Err(Error::InvalidState));
            assert!(rejected.upgrade().is_none());
            assert!(weak.upgrade().is_some());
            assert_eq!(
                (
                    d.total_input(),
                    d.total_output(),
                    d.retained_capacity(),
                    d.dictionary_retained_capacity(),
                    d.output().map(<[u8]>::to_vec)
                ),
                before
            );
        }
        assert_eq!(d.set_dictionary(None), Err(Error::InvalidState));
        drop(d);
        assert!(weak.upgrade().is_none());
        assert!(output[16..].iter().all(|&b| b == GUARD));
    }
    let mut output = [];
    let mut d = SliceDecoder::new(&mut output, DecodeOptions::new(0));
    d.process(EMPTY).unwrap();
    d.finish().unwrap();
    assert_eq!(d.set_dictionary(None), Err(Error::InvalidState));
    d.reset();
    d.set_dictionary(None).unwrap();
}
#[test]
fn pending_replacement_and_successful_empty_frames_release_exact_shared_owners() {
    let old = Dictionary::raw(b"old").unwrap().into_shared();
    let weak_old = SharedDictionary::downgrade(&old);
    let next = Dictionary::raw(b"next").unwrap().into_shared();
    let weak_next = SharedDictionary::downgrade(&next);
    let mut output = [GUARD; 1];
    let mut d = SliceDecoder::with_prefix(&mut output, DecodeOptions::new(0), old);
    d.set_dictionary(Some(next)).unwrap();
    assert!(weak_old.upgrade().is_none());
    d.process(EMPTY).unwrap();
    assert!(weak_next.upgrade().is_some());
    let last = Dictionary::raw(b"last").unwrap().into_shared();
    let weak_last = SharedDictionary::downgrade(&last);
    d.set_prefix(last).unwrap();
    assert!(weak_next.upgrade().is_none());
    d.process(EMPTY).unwrap();
    assert!(weak_last.upgrade().is_none());
    d.finish().unwrap();
    drop(d);
    assert_eq!(output, [GUARD]);
}
#[test]
fn scratch_failures_release_claimed_prefix_and_preserve_independent_borrowed_owners() {
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let dm = owner.monitor();
    let dictionary = Dictionary::from_bytes_in(DICTIONARY, &owner).unwrap();
    let before = dm.snapshot();
    assert!(dm.fail_next());
    let scratch = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let sm = scratch.monitor();
    let mut output = vec![GUARD; SOURCE.len() + 8];
    {
        let mut d = SliceDecoder::with_prefix_in(
            &mut output,
            DecodeOptions::new(SOURCE.len()),
            dictionary.view(),
            &scratch,
        );
        feed(&mut d, FRAME, 7).unwrap();
        d.finish().unwrap();
        assert_eq!(d.output().unwrap(), SOURCE);
        assert_eq!(d.dictionary_retained_capacity(), 0);
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
        let mut d = SliceDecoder::with_prefix_in(
            &mut output,
            DecodeOptions::new(SOURCE.len()),
            dictionary.view(),
            &scratch,
        );
        assert_eq!(feed(&mut d, FRAME, 7), Err(Error::AllocationFailed));
        assert_eq!(monitor.snapshot().attempts, ordinal);
        assert_eq!(monitor.snapshot().failures, 1);
        assert_eq!(d.output(), Err(Error::InvalidState));
        assert!(d.dictionary_retained_capacity() > 0);
        d.reset();
        assert_eq!(d.dictionary_retained_capacity(), 0);
        d.set_prefix(dictionary.view()).unwrap();
        feed(&mut d, FRAME, 7).unwrap();
        d.finish().unwrap();
        assert_eq!(d.output().unwrap(), SOURCE);
        drop(d);
        monitor.assert_released();
        assert_eq!(dm.snapshot(), before);
        assert!(output[SOURCE.len()..].iter().all(|&b| b == GUARD));
    }
    drop(dictionary);
    dm.assert_released();
}
#[test]
fn raw_rle_empty_and_skip_paths_allocate_no_codec_payload_and_preserve_unclaimed_prefixes() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let cases: &[(&[u8], &[u8], bool)] = &[
        (EMPTY, b"", false),
        (
            &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 25, 0, 0, b'a', b'b', b'c'],
            b"abc",
            false,
        ),
        (
            &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 7, 59, 0, 0, b'q'],
            b"qqqqqqq",
            false,
        ),
        (&[0x50, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 1, 2, 3], b"", true),
    ];
    for &(input, expected, pending) in cases {
        let scratch = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = scratch.monitor();
        assert!(monitor.fail_next());
        let mut output = [GUARD; 32];
        let mut d = SliceDecoder::with_prefix_in(
            &mut output,
            DecodeOptions::new(expected.len()),
            dictionary.view(),
            &scratch,
        );
        feed(&mut d, input, 1).unwrap();
        d.finish().unwrap();
        assert_eq!(d.output().unwrap(), expected);
        assert_eq!(d.dictionary_retained_capacity() > 0, pending);
        d.reset();
        assert_eq!(d.dictionary_retained_capacity() > 0, pending);
        drop(d);
        assert_eq!(monitor.snapshot().attempts, 0);
        monitor.assert_released();
        assert!(output[expected.len()..].iter().all(|&b| b == GUARD));
    }
    // The borrowed view also works with an unowned global scratch allocator.
    let mut output = [];
    let d = SliceDecoder::with_prefix_in(
        &mut output,
        DecodeOptions::new(0),
        dictionary.view(),
        Global,
    );
    assert!(d.dictionary_retained_capacity() > 0);
}
