//! Public streaming selection lifecycle: prefix replacement, claim-once
//! semantics, selection removal and dictionary-free continuation frames.
//!
//! Every emitted frame is decoded through the Rust decoder with the matching
//! attachment. Native replacement and reset schedules live in
//! `compiled_stream_selection_native.rs`.

use zstdoxide::{
    CompiledDictionary, CompiledStreamEncoder, CompiledStreamOptions, CompressionParameterRequest,
    DecodeOptions, Dictionary, EndDirective, Error, FrameHeader, Progress, StreamStatus,
    compress_with_parameters_and_dictionary, decompress_with_dictionary_in,
};

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

/// Pump `input` in fixed chunks through a small caller buffer.
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
            let progress: Progress = encoder
                .process(&input[position + consumed..end], &mut output, directive)
                .unwrap();
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
fn prefix_replacement_produces_one_prefix_frame_then_dictionary_free_frames() {
    let content = pattern(0x3e1f_00da, 8192);
    let dictionary = Dictionary::raw(&content).unwrap();
    let prefix_owner = Dictionary::raw(&content[..2048]).unwrap();
    let mut input = Vec::new();
    for _ in 0..10 {
        input.extend_from_slice(&content[0..700]);
        input.extend_from_slice(&content[2500..3200]);
        input.extend_from_slice(b"selection boundary marker");
    }
    for level in [1, 3, 4, 5, 7, 9] {
        let compiled = CompiledDictionary::new(
            dictionary.view(),
            &request(level, &input),
            std::alloc::Global,
        )
        .unwrap();
        let mut encoder = CompiledStreamEncoder::new(compiled).unwrap();
        // Baseline compiled frame for the displaced-state comparison.
        let baseline = pump(&mut encoder, &input, 1009, 31);
        encoder.reset();

        let displaced = encoder.set_prefix(prefix_owner.view()).unwrap();
        assert!(displaced.is_some(), "level {level}");
        let prefix_frame = pump(&mut encoder, &input, 1009, 31);
        let header = FrameHeader::parse(&prefix_frame).unwrap().0;
        assert_eq!(header.dictionary_id, 0, "raw prefix level {level}");
        assert_eq!(
            decompress_with_dictionary_in(
                &prefix_frame,
                &prefix_owner.view(),
                DecodeOptions::new(input.len()),
                std::alloc::Global,
            )
            .unwrap(),
            input,
            "level {level}"
        );
        // The prefix frame cannot decode dictionary-free: a match reaches
        // before the frame start.
        assert!(zstdoxide::decompress(prefix_frame.as_slice(), input.len()).is_err());

        // The completed prefix frame leaves no selection.
        encoder.reset();
        let tail = b"dictionary-free continuation frame";
        let tail_frame = pump(&mut encoder, tail, 13, 9);
        assert_eq!(FrameHeader::parse(&tail_frame).unwrap().0.dictionary_id, 0);
        assert_eq!(
            zstdoxide::decompress(tail_frame.as_slice(), tail.len()).unwrap(),
            tail
        );

        // The displaced compiled state still encodes its baseline bytes.
        let mut restored = CompiledStreamEncoder::new(displaced.unwrap()).unwrap();
        assert_eq!(
            pump(&mut restored, &input, 1009, 31),
            baseline,
            "level {level}"
        );
    }
}

#[test]
fn compiled_reselection_requires_the_fixed_profile_and_restores_tables() {
    let content = pattern(0x2e1e_710a, 8192);
    let dictionary = Dictionary::raw(&content).unwrap();
    let mut input = Vec::new();
    for _ in 0..8 {
        input.extend_from_slice(&content[0..900]);
        input.extend_from_slice(b"compiled reselection body");
    }
    let mut request = request(3, &input);
    request.set_window_log(16).unwrap();
    let first = CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global).unwrap();
    // Resolution clamps the window for this source size, so the genuine
    // mismatch is an explicit strategy, which resolution never rewrites.
    let mut mismatched_request = request;
    mismatched_request.set_strategy(Some(zstdoxide::ReferenceStrategy::Greedy));
    let mismatched =
        CompiledDictionary::new(dictionary.view(), &mismatched_request, std::alloc::Global)
            .unwrap();
    let window_log = first.parameters().window_log();
    let mut encoder = CompiledStreamEncoder::new_in(first, std::alloc::Global).unwrap();
    // A different profile is refused without touching the selection.
    assert!(matches!(
        encoder.set_compiled(mismatched),
        Err(Error::InvalidParameter)
    ));
    assert!(encoder.selection().is_some());
    // An identical-profile twin is accepted and clears a pending prefix.
    let prefix_owner = Dictionary::raw(&content[..1024]).unwrap();
    assert!(encoder.set_prefix(prefix_owner.view()).is_ok());
    assert_eq!(
        encoder.selection().map(|view| view.content()),
        Some(prefix_owner.view().content())
    );
    let twin = CompiledDictionary::new(dictionary.view(), &request, std::alloc::Global).unwrap();
    assert!(encoder.set_compiled(twin).unwrap().is_none());
    assert!(encoder.selection().is_some());
    let frame = pump(&mut encoder, &input, 511, 21);
    assert_eq!(
        FrameHeader::parse(&frame).unwrap().0.window_size,
        1 << window_log
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
fn formatted_prefix_frames_carry_identifier_and_entropy_for_one_frame() {
    let dictionary = Dictionary::from_bytes(PREPARED).unwrap();
    let filler = Dictionary::raw(&pattern(0x9e37_79b9, 4096)).unwrap();
    let mut encoder = CompiledStreamEncoder::new(
        CompiledDictionary::new(filler.view(), &request(4, TEXT), std::alloc::Global).unwrap(),
    )
    .unwrap();
    encoder.set_prefix(dictionary.view()).unwrap().unwrap();
    let frame = pump(&mut encoder, TEXT, TEXT.len(), 17);
    assert_eq!(
        FrameHeader::parse(&frame).unwrap().0.dictionary_id,
        dictionary.view().id()
    );
    assert_eq!(
        decompress_with_dictionary_in(
            &frame,
            &dictionary.view(),
            DecodeOptions::new(TEXT.len()),
            std::alloc::Global,
        )
        .unwrap(),
        TEXT
    );
    // One frame only: the selection is gone and the next frame is plain.
    encoder.reset();
    let second = pump(&mut encoder, TEXT, 77, 5);
    assert_eq!(FrameHeader::parse(&second).unwrap().0.dictionary_id, 0);
    assert!(zstdoxide::decompress(second.as_slice(), TEXT.len()).is_ok());
}

#[test]
fn prefix_body_matches_the_one_shot_parameterized_dictionary_frame() {
    let content = pattern(0x55ee_db1e, 4096);
    let dictionary = Dictionary::raw(&content).unwrap();
    let prefix_owner = Dictionary::raw(&content[..1024]).unwrap();
    let input = &content[512..3000];
    for level in [1, 3, 4, 5, 7, 9] {
        let shared = request(level, input);
        let compiled =
            CompiledDictionary::new(dictionary.view(), &shared, std::alloc::Global).unwrap();
        let mut encoder = CompiledStreamEncoder::with_options(
            compiled,
            CompiledStreamOptions {
                pledged_size: Some(input.len() as u64),
                ..CompiledStreamOptions::default()
            },
        )
        .unwrap();
        encoder.set_prefix(prefix_owner.view()).unwrap().unwrap();
        let streamed = pump(&mut encoder, input, input.len(), 64);
        let one_shot =
            compress_with_parameters_and_dictionary(input, &shared, &prefix_owner.view()).unwrap();
        let streamed_used = FrameHeader::parse(&streamed).unwrap().1;
        let one_shot_used = FrameHeader::parse(&one_shot).unwrap().1;
        assert_eq!(
            &streamed[streamed_used..],
            &one_shot[one_shot_used..],
            "level {level}"
        );
    }
}

#[test]
fn session_reset_preserves_a_pending_prefix_and_options_replacement_keeps_it() {
    let content = pattern(0x5a55_1e55, 4096);
    let dictionary = Dictionary::raw(&content).unwrap();
    let prefix_owner = Dictionary::raw(&content[2048..]).unwrap();
    let mut encoder = CompiledStreamEncoder::new(
        CompiledDictionary::new(dictionary.view(), &request(5, &content), std::alloc::Global)
            .unwrap(),
    )
    .unwrap();
    encoder.set_prefix(prefix_owner.view()).unwrap().unwrap();
    encoder.reset();
    assert!(encoder.selection().is_some());
    encoder
        .reset_with_options(CompiledStreamOptions {
            checksum: true,
            ..CompiledStreamOptions::default()
        })
        .unwrap();
    assert!(encoder.selection().is_some());
    let frame = pump(&mut encoder, &content[..1500], 401, 23);
    let header = FrameHeader::parse(&frame).unwrap().0;
    assert!(header.has_checksum);
    assert_eq!(header.dictionary_id, 0);
    assert_eq!(
        decompress_with_dictionary_in(
            &frame,
            &prefix_owner.view(),
            DecodeOptions::new(1500),
            std::alloc::Global,
        )
        .unwrap(),
        &content[..1500]
    );
    // The prefix was consumed by that single frame.
    assert!(encoder.selection().is_none());
}

#[test]
fn oversized_prefix_stages_only_its_window_suffix() {
    let content = pattern(0x3a11_7076, 1 << 16);
    let dictionary = Dictionary::raw(&content).unwrap();
    // The whole 64 KiB content is a prefix candidate; every level resolves a
    // smaller window, so only the in-window suffix can stage.
    let prefix_owner = Dictionary::raw(&content).unwrap();
    let mut input = Vec::new();
    for _ in 0..6 {
        input.extend_from_slice(&content[content.len() - 2048..]);
        input.extend_from_slice(&content[4096..5000]);
    }
    for level in [1, 3, 4, 5, 7, 9] {
        let mut encoder = CompiledStreamEncoder::new(
            CompiledDictionary::new(
                dictionary.view(),
                &request(level, &input),
                std::alloc::Global,
            )
            .unwrap(),
        )
        .unwrap();
        encoder.set_prefix(prefix_owner.view()).unwrap().unwrap();
        let frame = pump(&mut encoder, &input, 1023, 37);
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &prefix_owner.view(),
                DecodeOptions::new(input.len()),
                std::alloc::Global,
            )
            .unwrap(),
            input,
            "level {level}"
        );
    }
}

#[test]
fn empty_prefix_detaches_and_the_stream_continues_dictionary_free() {
    let content = pattern(0xd37a_1e55, 4096);
    let dictionary = Dictionary::raw(&content).unwrap();
    let mut encoder = CompiledStreamEncoder::new(
        CompiledDictionary::new(dictionary.view(), &request(3, &content), std::alloc::Global)
            .unwrap(),
    )
    .unwrap();
    let empty = Dictionary::raw(b"").unwrap();
    assert!(encoder.set_prefix(empty.view()).unwrap().is_some());
    assert!(encoder.selection().is_none());
    let frame = pump(&mut encoder, &content[..900], 311, 19);
    assert_eq!(FrameHeader::parse(&frame).unwrap().0.dictionary_id, 0);
    assert_eq!(
        zstdoxide::decompress(frame.as_slice(), 900).unwrap(),
        &content[..900]
    );
    encoder.reset();
    assert!(encoder.set_prefix(empty.view()).unwrap().is_none());
    assert!(encoder.selection().is_none());
}
