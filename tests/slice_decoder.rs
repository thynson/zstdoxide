//! Stable destination boundaries using independent native compressed fixtures.
#![forbid(unsafe_code)]

use std::alloc::Allocator;
use std::num::NonZeroU64;
use zstdoxide::{DecodeOptions, Error, SliceDecoder, StreamStatus, decompress_with_options};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

const HTTP: &[u8] = include_bytes!("upstream/data/generated/http.zst");
const HTTP_SOURCE: &[u8] = include_bytes!("upstream/data/golden-compression/http");
const MIXED: &[u8] = include_bytes!("upstream/decodecorpus/mixed-04.zst");
const MIXED_SOURCE: &[u8] = include_bytes!("upstream/decodecorpus/mixed-04.raw");
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
    let block = ((source.len() as u32) << 3) | 1 | if rle { 2 } else { 0 };
    frame.extend_from_slice(&block.to_le_bytes()[..3]);
    if rle {
        frame.push(source[0]);
    } else {
        frame.extend_from_slice(source);
    }
    frame
}
fn skip() -> Vec<u8> {
    vec![0x50, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 9, 8, 7]
}

fn feed<A: Allocator + Clone>(
    decoder: &mut SliceDecoder<'_, A>,
    encoded: &[u8],
    chunk: usize,
) -> Result<usize, Error> {
    assert!(chunk > 0);
    let mut position = 0;
    let mut frames = 0;
    while position < encoded.len() {
        let end = position + chunk.min(encoded.len() - position);
        while position < end {
            let before_input = decoder.total_input();
            let before_output = decoder.total_output();
            let p = decoder.process(&encoded[position..end])?;
            assert!(p.consumed <= end - position);
            assert!(p.consumed != 0 || p.produced != 0);
            assert_eq!(decoder.total_input() - before_input, p.consumed as u64);
            assert_eq!(decoder.total_output() - before_output, p.produced as u64);
            assert!(matches!(
                p.status,
                StreamStatus::NeedInput | StreamStatus::FrameFinished
            ));
            if p.status == StreamStatus::FrameFinished {
                frames += 1;
            }
            if p.status == StreamStatus::NeedInput {
                assert_eq!(p.consumed, end - position);
            }
            position += p.consumed;
        }
    }
    Ok(frames)
}

#[test]
fn every_split_retains_exact_progress_and_writes_only_the_output_prefix() {
    for (frame, source) in [
        (raw(b"hello", false, false), b"hello".to_vec()),
        (raw(b"qqqqqq", true, true), b"qqqqqq".to_vec()),
        (HTTP.to_vec(), HTTP_SOURCE.to_vec()),
    ] {
        for split in 0..=frame.len() {
            let mut output = vec![GUARD; source.len() + 18];
            let pointer = output.as_ptr();
            let mut decoder = SliceDecoder::new(&mut output[1..], DecodeOptions::new(source.len()));
            assert_eq!(
                decoder.process(&[]).unwrap().status,
                StreamStatus::NeedInput
            );
            let first = decoder.process(&frame[..split]).unwrap();
            assert_eq!(first.consumed, split);
            if split < frame.len() {
                assert_eq!(first.status, StreamStatus::NeedInput);
                let empty = decoder.process(&[]).unwrap();
                assert_eq!(
                    (empty.consumed, empty.produced, empty.status),
                    (0, 0, StreamStatus::NeedInput)
                );
                let second = decoder.process(&frame[split..]).unwrap();
                assert_eq!(second.consumed, frame.len() - split);
                assert_eq!(second.status, StreamStatus::FrameFinished);
                assert_eq!(first.produced + second.produced, source.len());
            } else {
                assert_eq!(first.status, StreamStatus::FrameFinished);
            }
            decoder.finish().unwrap();
            decoder.finish().unwrap();
            assert_eq!(decoder.output().unwrap(), source);
            assert_eq!(decoder.total_input(), frame.len() as u64);
            drop(decoder);
            assert_eq!(output.as_ptr(), pointer);
            assert_eq!(output[0], GUARD);
            assert!(output[1 + source.len()..].iter().all(|&b| b == GUARD));
        }
    }
}

#[test]
fn frame_boundaries_stop_before_next_magic_and_keep_new_history_separate() {
    let mut frame = skip();
    frame.extend_from_slice(MIXED);
    frame.extend_from_slice(&raw(b"", true, false));
    frame.extend_from_slice(HTTP);
    for chunk in [1, 2, 7, 19, usize::MAX] {
        let length = MIXED_SOURCE.len() + HTTP_SOURCE.len();
        let mut output = vec![GUARD; length + 8];
        let mut decoder = SliceDecoder::new(&mut output, DecodeOptions::new(length));
        assert_eq!(feed(&mut decoder, &frame, chunk), Ok(4));
        decoder.finish().unwrap();
        let decoded = decoder.output().unwrap();
        assert_eq!(&decoded[..MIXED_SOURCE.len()], MIXED_SOURCE);
        assert_eq!(&decoded[MIXED_SOURCE.len()..], HTTP_SOURCE);
        assert_eq!(decoder.process(&[]), Err(Error::InvalidState));
        assert_eq!(decoder.output(), Err(Error::InvalidState));
        decoder.reset();
        assert!(decoder.output().unwrap().is_empty());
        assert_eq!(decoder.total_input(), 0);
        assert_eq!(decoder.total_output(), 0);
        drop(decoder);
        assert_eq!(&output[length..], &[GUARD; 8]);
    }
}

#[test]
fn fixed_capacity_options_and_advertised_windows_are_independent() {
    for known in [false, true] {
        for rle in [false, true] {
            let frame = raw(b"xxxxx", known, rle);
            for chunk in [1, usize::MAX] {
                for capacity in [0, 4, 5, 9] {
                    for requested in [0, 4, 5, usize::MAX] {
                        let mut output = [GUARD; 16];
                        let limit = capacity.min(requested);
                        let mut decoder = SliceDecoder::new(
                            &mut output[1..1 + capacity],
                            DecodeOptions::new(requested),
                        );
                        assert_eq!(decoder.options().max_output_size, limit);
                        let result = feed(&mut decoder, &frame, chunk);
                        if limit < 5 {
                            assert_eq!(result, Err(Error::ResourceLimit));
                            assert_eq!(decoder.process(&[]), Err(Error::InvalidState));
                            assert_eq!(decoder.finish(), Err(Error::InvalidState));
                        } else {
                            assert_eq!(result, Ok(1));
                            decoder.finish().unwrap();
                            assert_eq!(decoder.output().unwrap(), b"xxxxx");
                        }
                        drop(decoder);
                        assert_eq!(output[0], GUARD);
                        assert!(output[1 + limit..].iter().all(|&b| b == GUARD));
                    }
                }
            }
        }
    }
    let frame = raw(b"hello", false, false);
    let mut output = [GUARD; 5];
    let mut decoder = SliceDecoder::new(
        &mut output,
        DecodeOptions {
            max_output_size: 5,
            max_window_size: 1023,
        },
    );
    assert_eq!(decoder.process(&frame), Err(Error::ResourceLimit));
}

#[test]
fn empty_output_accepts_actual_empty_and_skippable_frames_but_not_empty_input() {
    let mut output = [];
    let mut decoder = SliceDecoder::new(&mut output, DecodeOptions::new(0));
    assert_eq!(decoder.finish(), Err(Error::UnexpectedEof));
    assert_eq!(decoder.output(), Err(Error::InvalidState));
    for frame in [raw(b"", true, false), raw(b"", false, false), skip()] {
        decoder.reset();
        assert_eq!(feed(&mut decoder, &frame, 1), Ok(1));
        decoder.finish().unwrap();
        assert!(decoder.output().unwrap().is_empty());
    }
}

#[test]
fn every_truncated_prefix_is_rejected_and_reset_recovers_without_old_history() {
    for end in 0..HTTP.len() {
        let mut output = vec![GUARD; HTTP_SOURCE.len() + 8];
        let mut decoder = SliceDecoder::new(&mut output, DecodeOptions::new(HTTP_SOURCE.len()));
        let result = feed(&mut decoder, &HTTP[..end], 3);
        assert!(result.is_err() || decoder.finish().is_err());
        assert_eq!(decoder.output(), Err(Error::InvalidState));
        decoder.reset();
        assert_eq!(feed(&mut decoder, HTTP, 1), Ok(1));
        decoder.finish().unwrap();
        assert_eq!(decoder.output().unwrap(), HTTP_SOURCE);
        drop(decoder);
        assert_eq!(&output[HTTP_SOURCE.len()..], &[GUARD; 8]);
    }
}

#[test]
fn mutations_follow_one_shot_success_bytes_and_preserve_error_guards() {
    let original = include_bytes!("upstream/decodecorpus/compressed-02.zst");
    let capacity = include_bytes!("upstream/decodecorpus/compressed-02.raw").len();
    for index in 0..original.len() {
        let mut frame = original.to_vec();
        frame[index] ^= 0xff;
        let options = DecodeOptions::new(capacity);
        let expected = decompress_with_options(&frame, options);
        for chunk in [1, usize::MAX] {
            let mut output = vec![GUARD; capacity + 2];
            let mut decoder = SliceDecoder::new(&mut output[1..], options);
            let result = feed(&mut decoder, &frame, chunk).and_then(|_| decoder.finish());
            if let Ok(expected) = &expected {
                result.unwrap();
                assert_eq!(decoder.output().unwrap(), expected);
            } else {
                assert!(result.is_err(), "mutation {index}, chunk {chunk}");
            }
            drop(decoder);
            assert_eq!(output[0], GUARD);
            assert_eq!(output[capacity + 1], GUARD);
        }
    }
}

#[test]
fn raw_and_rle_sessions_use_no_payload_allocation_with_borrowed_nonclone_owner() {
    for frame in [
        raw(b"hello", true, false),
        raw(b"qqqqq", false, true),
        skip(),
    ] {
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut output = [GUARD; 5];
        let mut decoder = SliceDecoder::new_in(&mut output, DecodeOptions::new(5), &owner);
        assert_eq!(decoder.retained_capacity(), 0);
        assert_eq!(feed(&mut decoder, &frame, 1), Ok(1));
        decoder.finish().unwrap();
        assert_eq!(decoder.retained_capacity(), 0);
        assert_eq!(monitor.snapshot().attempts, 0);
        drop(decoder);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
    }
}

#[test]
fn every_complete_and_fragmented_scratch_failure_poisons_then_recovers() {
    for chunk in [1, 7, usize::MAX] {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut output = vec![GUARD; HTTP_SOURCE.len() + 8];
        let mut decoder =
            SliceDecoder::new_in(&mut output, DecodeOptions::new(HTTP_SOURCE.len()), owner);
        assert_eq!(feed(&mut decoder, HTTP, chunk), Ok(1));
        decoder.finish().unwrap();
        assert_eq!(decoder.output().unwrap(), HTTP_SOURCE);
        let requests = monitor.snapshot().attempts;
        assert!(requests > 0);
        drop(decoder);
        monitor.assert_released();
        for ordinal in 1..=requests {
            output.fill(GUARD);
            let owner = TrackingAllocator::new(AllocatorConfig {
                fail_at: NonZeroU64::new(ordinal),
                ..AllocatorConfig::default()
            })
            .unwrap();
            let monitor = owner.monitor();
            let mut decoder =
                SliceDecoder::new_in(&mut output, DecodeOptions::new(HTTP_SOURCE.len()), owner);
            assert_eq!(
                feed(&mut decoder, HTTP, chunk),
                Err(Error::AllocationFailed)
            );
            assert_eq!(monitor.snapshot().attempts, ordinal);
            assert_eq!(decoder.output(), Err(Error::InvalidState));
            assert_eq!(decoder.process(HTTP), Err(Error::InvalidState));
            decoder.reset();
            assert_eq!(feed(&mut decoder, HTTP, chunk), Ok(1));
            decoder.finish().unwrap();
            assert_eq!(decoder.output().unwrap(), HTTP_SOURCE);
            drop(decoder);
            assert_eq!(&output[HTTP_SOURCE.len()..], &[GUARD; 8]);
            assert_eq!(monitor.snapshot().failures, 1);
            monitor.assert_released();
        }
    }
}

#[test]
fn specific_format_errors_poison_the_session_without_touching_the_suffix() {
    let mut wrong_size = raw(b"hello", true, false);
    wrong_size[5] = 6;
    let mut bad_checksum = MIXED.to_vec();
    *bad_checksum.last_mut().unwrap() ^= 1;
    let mut reserved = raw(b"", true, false);
    reserved[4] |= 8;
    let dictionary = vec![0x28, 0xb5, 0x2f, 0xfd, 0x21, 1, 0, 1, 0, 0];
    for (frame, error) in [
        (vec![0xff], Error::InvalidMagic),
        (wrong_size, Error::ContentSizeMismatch),
        (bad_checksum, Error::ChecksumMismatch),
        (reserved, Error::CorruptData),
        (dictionary, Error::DictionaryRequired),
    ] {
        for chunk in [1, usize::MAX] {
            let mut output = vec![GUARD; MIXED_SOURCE.len() + 8];
            let mut decoder =
                SliceDecoder::new(&mut output, DecodeOptions::new(MIXED_SOURCE.len()));
            assert_eq!(feed(&mut decoder, &frame, chunk), Err(error));
            assert_eq!(decoder.process(&[]), Err(Error::InvalidState));
            assert_eq!(decoder.finish(), Err(Error::InvalidState));
            assert_eq!(decoder.output(), Err(Error::InvalidState));
            drop(decoder);
            assert_eq!(&output[MIXED_SOURCE.len()..], &[GUARD; 8]);
        }
    }
}

#[test]
fn resets_at_every_partial_phase_reuse_the_borrow_without_exposing_stale_output() {
    let frames = [
        raw(b"abc", false, false),
        raw(b"zzzzz", true, true),
        skip(),
        HTTP.to_vec(),
    ];
    let replacement = raw(b"new", true, false);
    for frame in frames {
        for end in 0..=frame.len() {
            let mut output = vec![GUARD; HTTP_SOURCE.len() + 8];
            let mut decoder = SliceDecoder::new(&mut output, DecodeOptions::new(HTTP_SOURCE.len()));
            feed(&mut decoder, &frame[..end], 1).unwrap();
            decoder.reset();
            assert_eq!(decoder.total_input(), 0);
            assert_eq!(decoder.total_output(), 0);
            assert!(decoder.output().unwrap().is_empty());
            assert_eq!(feed(&mut decoder, &replacement, 1), Ok(1));
            decoder.finish().unwrap();
            assert_eq!(decoder.output().unwrap(), b"new");
            drop(decoder);
            assert_eq!(&output[HTTP_SOURCE.len()..], &[GUARD; 8]);
        }
    }
}
