//! Public streaming compiled-dictionary construction, pumping and decoding.

use zstdoxide::{
    CompiledDictionary, CompiledStreamEncoder, CompiledStreamOptions, CompressionParameterRequest,
    DecodeOptions, Dictionary, EndDirective, Error, FrameHeader, Progress, StreamStatus,
    compress_with_compiled, decompress_with_dictionary_in,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

const PREPARED: &[u8] = include_bytes!("upstream/dictionaries/formatted.dict");
const TEXT: &[u8] = include_bytes!("upstream/dictionaries/dictionary-treeless-literals.expected");

fn pattern(mut state: u32, length: usize) -> Vec<u8> {
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

/// Pump `input` in fixed chunks through a small caller buffer and return the
/// frame. `Continue` is used until the last chunk, then `Finish`.
fn pump(
    encoder: &mut CompiledStreamEncoder<'_, std::alloc::Global>,
    input: &[u8],
    input_chunk: usize,
    output_chunk: usize,
) -> Vec<u8> {
    let mut frame = Vec::new();
    let mut output = vec![0xa5_u8; output_chunk.max(1)];
    let mut position = 0;
    loop {
        let end = (position + input_chunk.max(1)).min(input.len());
        let directive = if end == input.len() {
            EndDirective::Finish
        } else {
            EndDirective::Continue
        };
        let mut consumed = 0;
        loop {
            let before = output.clone();
            let progress: Progress = encoder
                .process(&input[position + consumed..end], &mut output, directive)
                .unwrap();
            assert!(progress.produced <= output.len());
            // Bytes outside the reported progress never changed.
            assert_eq!(&output[progress.produced..], &before[progress.produced..]);
            frame.extend_from_slice(&output[..progress.produced]);
            consumed += progress.consumed;
            match progress.status {
                StreamStatus::NeedInput => break,
                StreamStatus::NeedOutput => continue,
                StreamStatus::FrameFinished => {
                    assert_eq!(position + consumed, input.len());
                    return frame;
                }
                StreamStatus::Flushed => unreachable!("no flush requested"),
            }
        }
        position = end;
    }
}

#[test]
fn streaming_compiled_frames_roundtrip_across_all_admitted_families() {
    let content = pattern(0x0ddc_0ffe, 8192);
    let dictionary = Dictionary::raw(&content).unwrap();
    let mut input = Vec::new();
    for _ in 0..12 {
        input.extend_from_slice(&content[2048..4096]);
        input.extend_from_slice(&content[0..1000]);
        input.extend_from_slice(b"streaming compiled dictionary boundary");
    }
    let input = input.repeat(6);
    for level in [1, 3, 4, 5, 7, 9] {
        for checksum in [false, true] {
            let request = request(level, &input);
            let compiled = CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global)
                .unwrap_or_else(|error| panic!("level {level}: {error:?}"));
            assert_eq!(compiled.dictionary_id(), dictionary.view().id());
            let mut encoder = CompiledStreamEncoder::with_options(
                compiled,
                CompiledStreamOptions {
                    checksum,
                    pledged_size: Some(input.len() as u64),
                    ..CompiledStreamOptions::default()
                },
            )
            .unwrap();
            let frame = pump(&mut encoder, &input, 1021, 13);
            assert_eq!(encoder.total_input(), input.len() as u64);
            assert_eq!(encoder.total_output(), frame.len() as u64);
            let header = FrameHeader::parse(&frame).unwrap().0;
            assert_eq!(header.content_size, Some(input.len() as u64));
            assert_eq!(header.has_checksum, checksum);
            assert_eq!(header.dictionary_id, dictionary.view().id());
            assert_eq!(
                decompress_with_dictionary_in(
                    &frame,
                    &dictionary.view(),
                    DecodeOptions::new(input.len()),
                    std::alloc::Global,
                )
                .unwrap(),
                input,
                "level {level} checksum {checksum}"
            );
        }
    }
}

#[test]
fn formatted_dictionary_streaming_keeps_prepared_entropy_and_identifier() {
    let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
    let input = TEXT;
    let request = request(1, input);
    let compiled =
        CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global).unwrap();
    assert!(compiled.view().is_formatted());
    assert_ne!(compiled.dictionary_id(), 0);
    let mut encoder = CompiledStreamEncoder::new(compiled).unwrap();
    let frame = pump(&mut encoder, input, input.len(), 7);
    assert_eq!(
        FrameHeader::parse(&frame).unwrap().0.dictionary_id,
        dictionary.view().id()
    );
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
fn single_block_streaming_body_matches_the_one_shot_compiled_frame() {
    let content = pattern(0x51ce_5eed, 4096);
    let dictionary = Dictionary::raw(&content).unwrap();
    let input = &content[512..3000];
    for level in [1, 3, 4, 5, 7, 9] {
        let request = request(level, input);
        let compiled =
            CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global).unwrap();
        let one_shot = compress_with_compiled(input, &request, &compiled).unwrap();
        let one_shot_header = FrameHeader::parse(&one_shot).unwrap().1;
        let mut encoder = CompiledStreamEncoder::with_options(
            compiled,
            CompiledStreamOptions {
                pledged_size: Some(input.len() as u64),
                ..CompiledStreamOptions::default()
            },
        )
        .unwrap();
        let streamed = pump(&mut encoder, input, input.len(), 64);
        let streamed_header = FrameHeader::parse(&streamed).unwrap().1;
        assert_eq!(
            &streamed[streamed_header..],
            &one_shot[one_shot_header..],
            "level {level}"
        );
    }
}

#[test]
fn session_reset_keeps_the_compiled_selection_and_frame_options() {
    let content = pattern(0xfeed_beef, 8192);
    let dictionary = Dictionary::raw(&content).unwrap();
    let input = &content[..5000];
    let request = request(4, input);
    let compiled =
        CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global).unwrap();
    let mut encoder = CompiledStreamEncoder::with_options(
        compiled,
        CompiledStreamOptions {
            checksum: true,
            pledged_size: Some(input.len() as u64),
            ..CompiledStreamOptions::default()
        },
    )
    .unwrap();
    let first = pump(&mut encoder, input, 700, 11);
    encoder.reset();
    // The session reset dropped the one-frame pledge but kept the compiled
    // selection and the checksum flag.
    assert_eq!(encoder.options().pledged_size, None);
    assert!(encoder.options().checksum);
    encoder
        .reset_with_options(CompiledStreamOptions {
            checksum: true,
            pledged_size: Some(input.len() as u64),
            ..CompiledStreamOptions::default()
        })
        .unwrap();
    let second = pump(&mut encoder, input, 700, 11);
    assert_eq!(first, second);
    assert_eq!(
        decompress_with_dictionary_in(
            &second,
            &dictionary.view(),
            DecodeOptions::new(input.len()),
            std::alloc::Global,
        )
        .unwrap(),
        input
    );
}

#[test]
fn oversized_pledge_and_directive_reuse_are_rejected_before_writes() {
    let content = pattern(0x1357_9bdf, 2048);
    let dictionary = Dictionary::raw(&content).unwrap();
    let request = request(1, &content[..100]);
    let compiled =
        CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global).unwrap();
    let mut encoder = CompiledStreamEncoder::with_options(
        compiled,
        CompiledStreamOptions {
            pledged_size: Some(100),
            ..CompiledStreamOptions::default()
        },
    )
    .unwrap();
    let mut output = [0_u8; 128];
    // A longer source than the pledge is rejected before a header is written.
    assert_eq!(
        encoder.process(&content[..101], &mut output, EndDirective::Finish),
        Err(Error::ContentSizeMismatch)
    );
    assert!(output.iter().all(|&byte| byte == 0));
    // The error poisoned the session until reset.
    assert_eq!(
        encoder.process(b"", &mut output, EndDirective::Finish),
        Err(Error::InvalidState)
    );
    encoder.reset();
    // A flush whose output does not fit latches its directive; a later call
    // with a different directive is rejected without changing the session.
    let progress = encoder
        .process(b"short", &mut output[..1], EndDirective::Flush)
        .unwrap();
    assert_eq!(progress.status, StreamStatus::NeedOutput);
    assert_eq!(
        encoder.process(b"more", &mut output, EndDirective::Continue),
        Err(Error::InvalidState)
    );
    assert_eq!(encoder.total_input(), 5);
}

#[test]
fn stream_workspace_allocator_failure_releases_every_owner() {
    let content = pattern(0x2468_ace0, 4096);
    let dictionary = Dictionary::raw(&content).unwrap();
    let request = request(1, &content[..2048]);
    for fail_at in 1..=10 {
        let compiled =
            CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global).unwrap();
        let allocator = TrackingAllocator::new(AllocatorConfig {
            fail_at: core::num::NonZeroU64::new(fail_at),
            ..AllocatorConfig::default()
        })
        .unwrap();
        let monitor = allocator.monitor();
        let mut encoder = CompiledStreamEncoder::new_in(compiled, allocator.clone()).unwrap();
        let mut output = [0_u8; 64];
        let _ = encoder.process(&content[..2048], &mut output, EndDirective::Finish);
        drop(encoder);
        drop(allocator);
        monitor.assert_released();
    }
}
