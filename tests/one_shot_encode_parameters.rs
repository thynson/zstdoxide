//! Public parameterized dictionary one-shot compression and coordinator calls.

use core::num::NonZeroU64;
use zstdoxide::{
    CompressionParameterRequest, DecodeOptions, Dictionary, Error, FrameHeader, OneShotEncoder,
    ReferenceStrategy, compress_into_with_parameters, compress_into_with_parameters_and_dictionary,
    compress_into_with_parameters_and_dictionary_in, compress_with_parameters,
    compress_with_parameters_and_dictionary, compress_with_parameters_and_dictionary_in,
    decompress, decompress_with_dictionary,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

const PREPARED: &[u8] = include_bytes!("upstream/dictionaries/formatted.dict");
const TEXT: &[u8] = include_bytes!("upstream/dictionaries/dictionary-treeless-literals.expected");

fn noise(mut state: u32, length: usize) -> Vec<u8> {
    (0..length)
        .map(|_| {
            state ^= state.wrapping_shl(13);
            state ^= state >> 17;
            state ^= state.wrapping_shl(5);
            state as u8
        })
        .collect()
}

fn bound_input() -> Vec<u8> {
    let pattern = noise(0x51ce_5eed, 1024);
    [noise(0x1020_3040, 128), pattern, noise(0x0bad_f00d, 96)].concat()
}

fn optimal_request() -> CompressionParameterRequest {
    let mut request = CompressionParameterRequest::with_level(11);
    request.set_strategy(Some(ReferenceStrategy::Optimal));
    request.set_window_log(16).unwrap();
    request.set_hash_log(12).unwrap();
    request.set_chain_log(12).unwrap();
    request.set_search_log(6).unwrap();
    request.set_min_match(3).unwrap();
    request.set_target_length(1).unwrap();
    request
}

#[test]
fn parameterized_slice_bytes_match_the_allocating_api() {
    let input = bound_input();
    for level in [1, 4, 9] {
        let request = CompressionParameterRequest::with_level(level);
        let frame = compress_with_parameters(&input, &request).unwrap();
        let mut guarded = vec![0xa5; frame.len() + 4];
        let written =
            compress_into_with_parameters(&input, &mut guarded[2..2 + frame.len()], &request)
                .unwrap();
        assert_eq!(written, frame.len(), "level {level}");
        assert_eq!(&guarded[2..2 + written], &frame, "level {level}");
        assert_eq!(&guarded[..2], &[0xa5, 0xa5]);
        assert_eq!(&guarded[2 + written..], &[0xa5, 0xa5]);
        assert_eq!(
            decompress(&guarded[2..2 + written], input.len()).unwrap(),
            input
        );
        // One byte short leaves both guard bytes untouched.
        let mut short = vec![0xa5; frame.len() + 1];
        assert_eq!(
            compress_into_with_parameters(&input, &mut short[1..frame.len()], &request),
            Err(Error::ResourceLimit)
        );
        assert_eq!((short[0], short[frame.len()]), (0xa5, 0xa5));
    }
}

#[test]
fn parameterized_dictionary_frames_match_views_and_decode() {
    let dictionary = Dictionary::from_bytes(PREPARED).unwrap().into_shared();
    let view = dictionary.view();
    let raw = Dictionary::raw(view.content()).unwrap().into_shared();
    for level in [1, 4, 9] {
        let request = CompressionParameterRequest::with_level(level);
        let frame = compress_with_parameters_and_dictionary(TEXT, &request, &view).unwrap();
        assert_eq!(
            compress_with_parameters_and_dictionary(TEXT, &request, &dictionary).unwrap(),
            frame,
            "level {level}"
        );
        assert_eq!(
            compress_with_parameters_and_dictionary(TEXT, &request, &view).unwrap(),
            frame,
            "level {level}"
        );
        let header = FrameHeader::parse(&frame).unwrap().0;
        assert_eq!(header.dictionary_id, view.id());
        assert_eq!(header.content_size, Some(TEXT.len() as u64));
        assert!(!header.has_checksum);
        let mut output = vec![0xa5; frame.len() + 2];
        assert_eq!(
            compress_into_with_parameters_and_dictionary(
                TEXT,
                &mut output[1..1 + frame.len()],
                &request,
                &view,
            ),
            Ok(frame.len())
        );
        assert_eq!(&output[1..1 + frame.len()], &frame);
        assert_eq!((output[0], output[frame.len() + 1]), (0xa5, 0xa5));
        assert_eq!(
            decompress_with_dictionary(&frame, &dictionary, DecodeOptions::new(TEXT.len()))
                .unwrap(),
            TEXT
        );
        // Prepared entropy makes the formatted frame smaller than the same
        // content used as a raw dictionary; both remain decodable.
        let raw_frame = compress_with_parameters_and_dictionary(TEXT, &request, &raw).unwrap();
        assert!(frame.len() < raw_frame.len(), "level {level}");
        assert_eq!(
            decompress_with_dictionary(&raw_frame, &raw, DecodeOptions::new(TEXT.len())).unwrap(),
            TEXT
        );
    }
}

#[test]
fn parameterized_dictionary_rejects_optimal_profiles_before_writes() {
    let dictionary = Dictionary::from_bytes(PREPARED).unwrap().into_shared();
    let view = dictionary.view();
    let input = noise(0x7fff_0001, 20_000);
    let request = optimal_request();
    let mut output = vec![0xa5; 128];
    assert_eq!(
        compress_into_with_parameters_and_dictionary(&input, &mut output, &request, &view),
        Err(Error::UnsupportedFeature)
    );
    assert!(output.iter().all(|&byte| byte == 0xa5));
    assert_eq!(
        compress_with_parameters_and_dictionary(&input, &request, &view),
        Err(Error::UnsupportedFeature)
    );
}

#[test]
fn coordinator_parameterized_calls_consume_prefix_and_match_stateless_bytes() {
    let pattern = noise(0x1234_abcd, 1024);
    let input = [b"coordinator parameterized ".as_slice(), &pattern].concat();
    let request = CompressionParameterRequest::with_level(4);
    let dictionary = Dictionary::raw(&pattern).unwrap().into_shared();
    let expected = compress_with_parameters_and_dictionary(&input, &request, &dictionary).unwrap();
    let mut encoder = OneShotEncoder::with_dictionary(dictionary.clone());
    let mut output = vec![0xa5; input.len() + 64];
    for _ in 0..3 {
        assert_eq!(
            encoder.compress_into_with_parameters(
                &input,
                &request,
                &mut output[1..1 + expected.len()]
            ),
            Ok(expected.len())
        );
        assert_eq!(&output[1..1 + expected.len()], &expected);
        assert_eq!((output[0], output[expected.len() + 1]), (0xa5, 0xa5));
        assert_eq!(zstdoxide::SharedDictionary::strong_count(&dictionary), 2);
    }
    // A prefix is consumed by the parameterized attempt; the next call uses
    // parameterized dictionary-free bytes.
    let mut prefix = OneShotEncoder::with_prefix(dictionary.view());
    let view_frame =
        compress_with_parameters_and_dictionary(&input, &request, &dictionary.view()).unwrap();
    assert_eq!(
        prefix.compress_into_with_parameters(&input, &request, &mut output[..view_frame.len()]),
        Ok(view_frame.len())
    );
    assert!(!prefix.prefix_pending());
    assert!(prefix.dictionary().is_none());
    let plain = compress_with_parameters(&input, &request).unwrap();
    assert_eq!(
        prefix.compress_into_with_parameters(&input, &request, &mut output[..plain.len()]),
        Ok(plain.len())
    );
    assert_eq!(&output[..plain.len()], &plain);
    // A short destination consumes a pending prefix and preserves guards.
    let mut short = OneShotEncoder::with_prefix(dictionary.view());
    let mut guards = [0xa5; 3];
    assert_eq!(
        short.compress_into_with_parameters(&input, &request, &mut guards[1..2]),
        Err(Error::ResourceLimit)
    );
    assert_eq!((guards[0], guards[2]), (0xa5, 0xa5));
    assert!(short.dictionary().is_none());
}

#[test]
fn parameterized_dictionary_allocation_failures_release_every_owner() {
    let dictionary = Dictionary::from_bytes(PREPARED).unwrap().into_shared();
    let request = CompressionParameterRequest::with_level(4);
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    let frame =
        compress_with_parameters_and_dictionary_in(TEXT, &request, &dictionary.view(), owner)
            .unwrap();
    let attempts = monitor.snapshot().attempts;
    assert!(attempts >= 5);
    assert_eq!(monitor.snapshot().live_blocks, 1);
    drop(frame);
    monitor.assert_released();
    for fail_at in 1..=attempts {
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(fail_at),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        assert_eq!(
            compress_with_parameters_and_dictionary_in(TEXT, &request, &dictionary.view(), owner),
            Err(Error::AllocationFailed),
            "failure {fail_at}"
        );
        monitor.assert_released();
        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.owners_dropped, snapshot.identities_created);
    }
}

#[test]
fn parameterized_slice_allocation_failures_keep_guards_and_release_owners() {
    let dictionary = Dictionary::from_bytes(PREPARED).unwrap().into_shared();
    let request = CompressionParameterRequest::with_level(1);
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    let mut output = vec![0xa5; TEXT.len() * 2];
    let written = compress_into_with_parameters_and_dictionary_in(
        TEXT,
        &mut output,
        &request,
        &dictionary.view(),
        owner,
    )
    .unwrap();
    let attempts = monitor.snapshot().attempts;
    assert!(written < output.len());
    drop(output);
    monitor.assert_released();
    for fail_at in 1..=attempts {
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(fail_at),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        let mut guarded = vec![0xa5; TEXT.len() * 2 + 2];
        let result = compress_into_with_parameters_and_dictionary_in(
            TEXT,
            &mut guarded[1..1 + TEXT.len() * 2],
            &request,
            &dictionary.view(),
            owner,
        );
        assert_eq!(result, Err(Error::AllocationFailed), "failure {fail_at}");
        assert_eq!((guarded[0], guarded[TEXT.len() * 2 + 1]), (0xa5, 0xa5));
        monitor.assert_released();
        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.owners_dropped, snapshot.identities_created);
    }
}
