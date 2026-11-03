//! Public compiled-dictionary construction, one-shot and coordinator calls.

use core::num::NonZeroU64;
use zstdoxide::{
    CompiledDictionary, CompiledOneShotEncoder, CompressionParameterRequest, DecodeOptions,
    Dictionary, Error, ReferenceStrategy, compress_into_with_compiled, compress_with_compiled,
    compress_with_compiled_in, decompress_with_dictionary_in,
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

fn request(level: i32, input: &[u8]) -> CompressionParameterRequest {
    let mut request = CompressionParameterRequest::with_level(level);
    request
        .set_source_hint(u32::try_from(input.len()).unwrap())
        .unwrap();
    request
}

#[test]
fn compiled_frames_match_allocating_and_caller_owned_paths() {
    let content = noise(0x51ce_5eed, 4096);
    let dictionary = Dictionary::raw(&content).unwrap();
    let input = [b"compiled frame ".as_slice(), &content[..1024]].concat();
    for level in [1, 3, 4, 9] {
        let request = request(level, &input);
        let compiled = CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global)
            .unwrap_or_else(|error| panic!("level {level}: {error:?}"));
        assert_eq!(compiled.content(), content);
        assert_eq!(compiled.dictionary_id(), 0);
        assert_eq!(compiled.loaded_len(), content.len());
        let frame = compress_with_compiled(&input, &request, &compiled).unwrap();
        let mut output = vec![0xa5; frame.len() + 4];
        assert_eq!(
            compress_into_with_compiled(
                &input,
                &mut output[2..2 + frame.len()],
                &request,
                &compiled,
            ),
            Ok(frame.len()),
            "level {level}"
        );
        assert_eq!(&output[2..2 + frame.len()], &frame);
        assert_eq!((output[0], output[1]), (0xa5, 0xa5));
        assert_eq!(
            (output[2 + frame.len()], output[3 + frame.len()]),
            (0xa5, 0xa5)
        );
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &compiled.view(),
                DecodeOptions::new(input.len()),
                std::alloc::Global,
            )
            .unwrap(),
            input
        );
        // The coordinator reuses the same compiled state across calls without
        // retaining a request, workspace or output borrow.
        let encoder = CompiledOneShotEncoder::new(compiled);
        assert_eq!(encoder.dictionary().content(), content);
        let mut again = vec![0xa5; frame.len() + 2];
        for _ in 0..3 {
            assert_eq!(
                encoder.compress_into(&input, &request, &mut again[1..1 + frame.len()]),
                Ok(frame.len())
            );
            assert_eq!(&again[1..1 + frame.len()], &frame);
            assert_eq!((again[0], again[1 + frame.len()]), (0xa5, 0xa5));
        }
    }
}

#[test]
fn formatted_compiled_entropy_seeds_frames_and_keeps_the_identifier() {
    let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
    let input = TEXT;
    let request = request(1, input);
    let compiled =
        CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global).unwrap();
    assert!(compiled.view().is_formatted());
    assert_ne!(compiled.dictionary_id(), 0);
    let frame = compress_with_compiled(input, &request, &compiled).unwrap();
    let combined =
        zstdoxide::compress_with_parameters_and_dictionary(input, &request, &dictionary.view())
            .unwrap();
    assert_eq!(frame, combined);
    assert_eq!(
        decompress_with_dictionary_in(
            &frame,
            &dictionary.view(),
            DecodeOptions::new(input.len()),
            std::alloc::Global,
        )
        .unwrap(),
        input
    );
}

#[test]
fn compiled_calls_reject_profile_mismatch_before_writes() {
    let content = noise(0x1234_5678, 2048);
    let dictionary = Dictionary::raw(&content).unwrap();
    let input = &content[..1024];
    let matching = request(1, input);
    let compiled =
        CompiledDictionary::new(dictionary.view(), &matching, std::alloc::Global).unwrap();
    let mut output = [0xa5; 128];
    let different_level = request(9, input);
    assert_eq!(
        compress_into_with_compiled(input, &mut output, &different_level, &compiled),
        Err(Error::InvalidParameter)
    );
    assert!(output.iter().all(|&byte| byte == 0xa5));
    let mut different_window = matching;
    different_window.set_window_log(10).unwrap();
    assert_eq!(
        compress_with_compiled(input, &different_window, &compiled),
        Err(Error::InvalidParameter)
    );
    // A different source length must re-resolve and can change the profile;
    // the fixed compiled profile is never used for mismatched parameters.
    let longer = vec![0x42; 200_000];
    let longer_request = request(1, &longer);
    assert!(matches!(
        compress_with_compiled(&longer, &longer_request, &compiled),
        Err(Error::InvalidParameter | Error::ResourceLimit)
    ));
    // Empty content cannot be compiled through the public constructor.
    let empty = Dictionary::raw(b"").unwrap();
    assert_eq!(
        CompiledDictionary::new(empty.view(), &matching, std::alloc::Global).err(),
        Some(Error::InvalidParameter)
    );
    // Optimal-family construction rejects before allocating.
    let mut optimal = CompressionParameterRequest::with_level(11);
    optimal.set_strategy(Some(ReferenceStrategy::Optimal));
    assert_eq!(
        CompiledDictionary::new(dictionary.view(), &optimal, std::alloc::Global).err(),
        Some(Error::UnsupportedFeature)
    );
}

#[test]
fn compiled_frame_allocation_failures_release_every_owner() {
    let content = noise(0x9abc_def0, 4096);
    let dictionary = Dictionary::raw(&content).unwrap();
    let input = &content[..2048];
    let request = request(4, input);
    let compiled =
        CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global).unwrap();
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    let frame = compress_with_compiled_in(input, &request, &compiled, owner).unwrap();
    let attempts = monitor.snapshot().attempts;
    assert!(attempts >= 5);
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
            compress_with_compiled_in(input, &request, &compiled, owner),
            Err(Error::AllocationFailed),
            "failure {fail_at}"
        );
        monitor.assert_released();
        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.owners_dropped, snapshot.identities_created);
    }
    // The coordinator's caller-owned path uses the same independent scratch
    // release contract and leaves the compiled state usable.
    let encoder = CompiledOneShotEncoder::new(compiled);
    let mut output = vec![0xa5; input.len() + 64];
    let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
    let monitor = owner.monitor();
    assert!(
        encoder
            .compress_into_in(input, &request, &mut output, owner)
            .is_ok()
    );
    let coordinator_attempts = monitor.snapshot().attempts;
    monitor.assert_released();
    for fail_at in 1..=coordinator_attempts {
        let owner = TrackingAllocator::new(AllocatorConfig {
            fail_at: NonZeroU64::new(fail_at),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = owner.monitor();
        assert_eq!(
            encoder.compress_into_in(input, &request, &mut output, owner),
            Err(Error::AllocationFailed),
            "coordinator failure {fail_at}"
        );
        monitor.assert_released();
    }
    assert!(encoder.compress_into(input, &request, &mut output).is_ok());
}
