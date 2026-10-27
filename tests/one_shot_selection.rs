//! Call-level persistent/prefix ownership over both caller-owned output paths.
#![forbid(unsafe_code)]
use std::{alloc::Global, num::NonZeroU64};
use zstdoxide::{
    DecodeOptions, Dictionary, DictionaryHandle, Error, OneShotDecoder, Result, compress,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
const DICTIONARY: &[u8] =
    include_bytes!("upstream/data/golden-dictionaries/http-dict-missing-symbols");
const FRAME: &[u8] = include_bytes!("upstream/data/generated/http-dictionary.zst");
const SOURCE: &[u8] = include_bytes!("upstream/data/golden-compression/http");
const SKIP: &[u8] = &[0x5f, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 1, 2, 3];
fn tracking() -> TrackingAllocator {
    TrackingAllocator::new(AllocatorConfig::default()).unwrap()
}
fn attempt<D: DictionaryHandle>(
    decoder: &mut OneShotDecoder<D>,
    frame: &[u8],
    limit: usize,
    in_place: bool,
) -> (Result<usize>, Vec<u8>) {
    let capacity = if in_place {
        limit + frame.len() + 131072
    } else {
        limit
    };
    let mut output = vec![0xa5; capacity + 32];
    let start = capacity - frame.len().min(capacity);
    if in_place {
        output[16 + start..16 + capacity].copy_from_slice(frame);
    }
    let before = output.clone();
    let result = if in_place {
        decoder.decompress_in_place(&mut output[16..16 + capacity], start)
    } else {
        decoder.decompress_into(frame, &mut output[16..16 + capacity])
    };
    assert_eq!(&output[..16], &before[..16]);
    assert_eq!(
        &output[16 + limit.min(capacity)..],
        &before[16 + limit.min(capacity)..]
    );
    if let Ok(n) = result {
        assert!(n <= limit);
        assert_eq!(&output[16 + n..], &before[16 + n..]);
    }
    (result, output[16..16 + limit.min(capacity)].to_vec())
}

#[test]
fn one_prefix_covers_all_dependent_frames_in_one_call_then_disappears() {
    type Case<'a> = (&'a [u8], &'a [u8], &'a [u8], bool);
    let cases: [Case<'_>; 3] = [
        (
            include_bytes!("upstream/dictionaries/boundary-dictionary.raw"),
            include_bytes!("upstream/dictionaries/p0-far-dictionary.zst"),
            include_bytes!("upstream/dictionaries/p0-far-dictionary.expected"),
            true,
        ),
        (
            include_bytes!("upstream/dictionaries/formatted.dict"),
            include_bytes!("upstream/dictionaries/formatted-id.zst"),
            include_bytes!("upstream/dictionaries/formatted-id.expected"),
            false,
        ),
        (DICTIONARY, FRAME, SOURCE, false),
    ];
    for (bytes, frame, expected, raw) in cases {
        let dictionary = if raw {
            Dictionary::raw(bytes)
        } else {
            Dictionary::from_bytes(bytes)
        }
        .unwrap();
        let mut joined = SKIP.to_vec();
        joined.extend_from_slice(frame);
        joined.extend_from_slice(SKIP);
        joined.extend_from_slice(frame);
        let mut twice = expected.to_vec();
        twice.extend_from_slice(expected);
        for in_place in [false, true] {
            let mut decoder =
                OneShotDecoder::with_prefix(DecodeOptions::new(twice.len()), dictionary.view());
            let (result, output) = attempt(&mut decoder, &joined, twice.len(), in_place);
            assert_eq!(result, Ok(twice.len()));
            assert_eq!(output, twice);
            assert!(!decoder.prefix_pending());
            assert!(decoder.dictionary().is_none());
            assert!(
                attempt(&mut decoder, frame, twice.len(), in_place)
                    .0
                    .is_err()
            );
            decoder.set_dictionary(Some(dictionary.view()));
            for _ in 0..2 {
                let (result, output) = attempt(&mut decoder, &joined, twice.len(), in_place);
                assert_eq!(result, Ok(twice.len()));
                assert_eq!(output, twice);
                assert!(decoder.dictionary().is_some());
            }
        }
    }
}

#[test]
fn failed_attempts_consume_prefix_but_preserve_persistent_selection() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let mut bad_checksum = FRAME.to_vec();
    *bad_checksum.last_mut().unwrap() ^= 1;
    let bad_magic = [0_u8; 4];
    for in_place in [false, true] {
        for prefix in [false, true] {
            for (frame, options) in [
                (&[][..], DecodeOptions::new(SOURCE.len())),
                (bad_magic.as_slice(), DecodeOptions::new(SOURCE.len())),
                (&FRAME[..FRAME.len() - 1], DecodeOptions::new(SOURCE.len())),
                (bad_checksum.as_slice(), DecodeOptions::new(SOURCE.len())),
                (FRAME, DecodeOptions::new(0)),
                (
                    FRAME,
                    DecodeOptions {
                        max_output_size: SOURCE.len(),
                        max_window_size: 0,
                    },
                ),
            ] {
                let mut decoder = if prefix {
                    OneShotDecoder::with_prefix(options, dictionary.view())
                } else {
                    OneShotDecoder::with_dictionary(options, dictionary.view())
                };
                assert!(
                    attempt(&mut decoder, frame, options.max_output_size, in_place)
                        .0
                        .is_err()
                );
                assert_eq!(decoder.dictionary().is_some(), !prefix);
                assert!(!decoder.prefix_pending());
                decoder.set_options(DecodeOptions::new(SOURCE.len()));
                let (result, output) = attempt(&mut decoder, FRAME, SOURCE.len(), in_place);
                if prefix {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result, Ok(SOURCE.len()));
                    assert_eq!(output, SOURCE);
                }
            }
        }
        let wrong = Dictionary::raw(b"wrong dictionary").unwrap();
        let mut decoder =
            OneShotDecoder::with_prefix(DecodeOptions::new(SOURCE.len()), wrong.view());
        assert_eq!(
            attempt(&mut decoder, FRAME, SOURCE.len(), in_place).0,
            Err(Error::DictionaryMismatch)
        );
        assert!(decoder.dictionary().is_none());
    }
}

#[test]
fn skippable_and_independent_empty_frames_still_consume_next_call_selection() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let empty = compress(b"").unwrap();
    let plain = compress(b"independent").unwrap();
    for in_place in [false, true] {
        for (frame, size) in [(SKIP, 0), (empty.as_slice(), 0), (plain.as_slice(), 11)] {
            let mut decoder =
                OneShotDecoder::with_prefix(DecodeOptions::new(SOURCE.len()), dictionary.view());
            assert_eq!(
                attempt(&mut decoder, frame, SOURCE.len(), in_place).0,
                Ok(size)
            );
            assert!(decoder.dictionary().is_none());
            assert!(
                attempt(&mut decoder, FRAME, SOURCE.len(), in_place)
                    .0
                    .is_err()
            );
        }
    }
}

#[test]
fn owned_prefix_drops_after_attempt_and_persistent_owner_after_clear() {
    for prefix in [false, true] {
        let owner = tracking();
        let monitor = owner.monitor();
        let dictionary = Dictionary::from_bytes_in(DICTIONARY, owner)
            .unwrap()
            .into_shared();
        let weak = std::sync::Arc::downgrade(&dictionary);
        let mut decoder = if prefix {
            OneShotDecoder::with_prefix(DecodeOptions::new(0), dictionary)
        } else {
            OneShotDecoder::with_dictionary(DecodeOptions::new(0), dictionary)
        };
        assert!(weak.upgrade().is_some());
        assert_eq!(
            decoder.decompress_into(b"", &mut []),
            Err(Error::UnexpectedEof)
        );
        assert_eq!(weak.upgrade().is_some(), !prefix);
        if !prefix {
            decoder.set_prefix(None);
        }
        assert!(weak.upgrade().is_none());
        monitor.assert_released();
    }
    let old = tracking();
    let monitor = old.monitor();
    let dictionary = Dictionary::raw_in(b"old", old).unwrap().into_shared();
    let mut decoder = OneShotDecoder::with_prefix(DecodeOptions::new(5), dictionary);
    decoder.set_dictionary(None);
    monitor.assert_released();
    assert!(!decoder.prefix_pending());
    assert!(decoder.dictionary().is_none());
}

#[test]
fn borrowed_nonclone_owners_and_independent_scratch_keep_dictionary_accounting() {
    let owner = tracking().into_non_clone();
    let dm = owner.monitor();
    let dictionary = Dictionary::from_bytes_in(DICTIONARY, &owner).unwrap();
    let before = dm.snapshot();
    assert!(dm.fail_next());
    for prefix in [false, true] {
        for in_place in [false, true] {
            let scratch = tracking().into_non_clone();
            let sm = scratch.monitor();
            let mut decoder = if prefix {
                OneShotDecoder::with_prefix(DecodeOptions::new(SOURCE.len()), dictionary.view())
            } else {
                OneShotDecoder::with_dictionary(DecodeOptions::new(SOURCE.len()), dictionary.view())
            };
            let capacity = SOURCE.len() + FRAME.len() + 131072;
            let mut output = vec![0xa5; capacity];
            let start = capacity - FRAME.len();
            let result = if in_place {
                output[start..].copy_from_slice(FRAME);
                decoder.decompress_in_place_in(&mut output, start, &scratch)
            } else {
                decoder.decompress_into_in(FRAME, &mut output, &scratch)
            };
            assert_eq!(result, Ok(SOURCE.len()));
            assert_eq!(&output[..SOURCE.len()], SOURCE);
            sm.assert_released();
            assert_eq!(sm.snapshot().identities_created, 1);
            assert_eq!(dm.snapshot(), before);
            assert_eq!(decoder.dictionary().is_some(), !prefix);
        }
    }
    drop(dictionary);
    dm.assert_released();
}

#[test]
fn every_scratch_failure_ordinal_obeys_attempt_consumption_and_releases_storage() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    let capacity = SOURCE.len() + FRAME.len() + 131072;
    let start = capacity - FRAME.len();
    for in_place in [false, true] {
        let scratch = tracking();
        let sm = scratch.monitor();
        let mut decoder =
            OneShotDecoder::with_prefix(DecodeOptions::new(SOURCE.len()), dictionary.view());
        let mut output = vec![0xa5; capacity];
        if in_place {
            output[start..].copy_from_slice(FRAME);
            assert_eq!(
                decoder.decompress_in_place_in(&mut output, start, scratch),
                Ok(SOURCE.len())
            );
        } else {
            assert_eq!(
                decoder.decompress_into_in(FRAME, &mut output, scratch),
                Ok(SOURCE.len())
            );
        }
        let requests = sm.snapshot().attempts;
        assert!(requests > 0);
        sm.assert_released();
        for prefix in [false, true] {
            for ordinal in 1..=requests {
                let scratch = TrackingAllocator::new(AllocatorConfig {
                    fail_at: NonZeroU64::new(ordinal),
                    ..AllocatorConfig::default()
                })
                .unwrap();
                let sm = scratch.monitor();
                let mut decoder = if prefix {
                    OneShotDecoder::with_prefix(DecodeOptions::new(SOURCE.len()), dictionary.view())
                } else {
                    OneShotDecoder::with_dictionary(
                        DecodeOptions::new(SOURCE.len()),
                        dictionary.view(),
                    )
                };
                let mut guarded = vec![0xa5; capacity + 32];
                if in_place {
                    guarded[16 + start..16 + capacity].copy_from_slice(FRAME);
                }
                let before = guarded.clone();
                let result = if in_place {
                    decoder.decompress_in_place_in(&mut guarded[16..16 + capacity], start, scratch)
                } else {
                    decoder.decompress_into_in(FRAME, &mut guarded[16..16 + capacity], scratch)
                };
                assert_eq!(result, Err(Error::AllocationFailed));
                assert_eq!(sm.snapshot().attempts, ordinal);
                assert_eq!(sm.snapshot().failures, 1);
                sm.assert_released();
                assert_eq!(decoder.dictionary().is_some(), !prefix);
                assert!(!decoder.prefix_pending());
                assert_eq!(&guarded[..16], &before[..16]);
                assert_eq!(&guarded[16 + SOURCE.len()..], &before[16 + SOURCE.len()..]);
            }
        }
    }
}

#[test]
fn invalid_in_place_coordinates_consume_prefix_before_any_buffer_write() {
    let dictionary = Dictionary::from_bytes(DICTIONARY).unwrap();
    for position in [17, usize::MAX] {
        let mut decoder = OneShotDecoder::with_prefix(DecodeOptions::new(0), dictionary.view());
        decoder.set_options(DecodeOptions::new(16));
        assert_eq!(decoder.options().max_output_size, 16);
        assert!(decoder.prefix_pending());
        let mut buffer = [0xa5; 16];
        assert_eq!(
            decoder.decompress_in_place(&mut buffer, position),
            Err(Error::ResourceLimit)
        );
        assert_eq!(buffer, [0xa5; 16]);
        assert!(decoder.dictionary().is_none());
    }
}

#[test]
fn empty_default_coordinator_recovers_after_errors_without_reset_or_selection() {
    let mut decoder = OneShotDecoder::new(DecodeOptions::new(5));
    assert!(decoder.dictionary().is_none());
    assert!(!decoder.prefix_pending());
    assert_eq!(
        decoder.decompress_into_in(b"", &mut [], Global),
        Err(Error::UnexpectedEof)
    );
    let frame = compress(b"hello").unwrap();
    let mut output = [0xa5; 16];
    assert_eq!(decoder.decompress_into(&frame, &mut output), Ok(5));
    assert_eq!(&output[..5], b"hello");
    assert_eq!(&output[5..], &[0xa5; 11]);
}
