// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Contracts and regression fixtures derive from facebook/zstd v1.5.7.
// Used under the BSD-3-Clause option; see LICENSE.zstd.
//! Dictionary/prefix decoder contracts and independently encoded boundaries.
//!
//! The fixture manifest records C acceptance separately from the format rule:
//! dictionary references at P == W are accepted, while P > W is rejected.

use zstdoxide::{
    DecodeOptions, Decoder, Dictionary, Error, FrameHeader, Result, SharedDictionary, StreamStatus,
    decompress_with_dictionary,
};

const RAW: &[u8] = include_bytes!("upstream/dictionaries/boundary-dictionary.raw");
const FAR: &[u8] = include_bytes!("upstream/dictionaries/p0-far-dictionary.zst");
const FAR_OUTPUT: &[u8] = include_bytes!("upstream/dictionaries/p0-far-dictionary.expected");
const HTTP_DICTIONARY: &[u8] =
    include_bytes!("upstream/data/golden-dictionaries/http-dict-missing-symbols");
const HTTP_FRAME: &[u8] = include_bytes!("upstream/data/generated/http-dictionary.zst");
const HTTP_OUTPUT: &[u8] = include_bytes!("upstream/data/golden-compression/http");
const EMPTY_FRAME: &[u8] = &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0];

fn raw_dictionary() -> SharedDictionary {
    Dictionary::raw(RAW).unwrap().into_shared()
}

fn http_dictionary() -> SharedDictionary {
    Dictionary::from_bytes(HTTP_DICTIONARY)
        .unwrap()
        .into_shared()
}

fn boundary_options(output_limit: usize) -> DecodeOptions {
    DecodeOptions {
        max_output_size: output_limit,
        max_window_size: 1024,
    }
}

/// Decode supplied complete frames, keeping the context open for further input.
/// Guard bytes verify that partial processing touches only reported output.
fn decode_stream(
    decoder: &mut Decoder,
    input: &[u8],
    input_chunk: usize,
    output_chunk: usize,
) -> Result<Vec<u8>> {
    assert!(input_chunk != 0 && output_chunk != 0);
    let before_input = decoder.total_input();
    let before_output = decoder.total_output();
    let mut consumed = 0_usize;
    let mut result = Vec::new();
    let mut scratch = vec![0xa5; output_chunk + 8];
    for _ in 0..200_000 {
        scratch.fill(0xa5);
        let end = consumed.saturating_add(input_chunk).min(input.len());
        let progress = decoder.process(&input[consumed..end], &mut scratch[..output_chunk])?;
        assert!(progress.consumed <= end - consumed);
        assert!(progress.produced <= output_chunk);
        assert!(
            scratch[progress.produced..]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        consumed += progress.consumed;
        result.extend_from_slice(&scratch[..progress.produced]);
        assert_eq!(decoder.total_input(), before_input + consumed as u64);
        assert_eq!(decoder.total_output(), before_output + result.len() as u64);
        if progress.status == StreamStatus::FrameFinished && consumed == input.len() {
            return Ok(result);
        }
        assert_ne!(progress.status, StreamStatus::Flushed);
        if progress.status == StreamStatus::NeedInput && consumed == input.len() {
            decoder.finish()?;
            panic!("decoder sealed an incomplete frame without reporting its boundary");
        }
        assert!(
            progress.consumed != 0 || progress.produced != 0,
            "decoder stalled with available caller output"
        );
    }
    panic!("dictionary frame exceeded the bounded test call count");
}

fn skip_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = 0x184d_2a5f_u32.to_le_bytes().to_vec();
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}

/// Alter only the optional ID field; payload and content checksum stay exact.
fn frame_with_dictionary_id(frame: &[u8], id: Option<u32>) -> Vec<u8> {
    let descriptor = frame[4];
    let id_start = 5 + usize::from(descriptor & 0x20 == 0);
    let previous_size = [0, 1, 2, 4][usize::from(descriptor & 3)];
    let mut changed = frame[..id_start].to_vec();
    changed[4] = (descriptor & !3) | if id.is_some() { 3 } else { 0 };
    if let Some(id) = id {
        changed.extend_from_slice(&id.to_le_bytes());
    }
    changed.extend_from_slice(&frame[id_start + previous_size..]);
    let header = FrameHeader::parse(&changed).unwrap().0;
    assert_eq!(header.dictionary_id, id.unwrap_or(0));
    changed
}

macro_rules! boundary_case {
    ($name:literal) => {
        (
            $name,
            &include_bytes!(concat!("upstream/dictionaries/", $name, ".zst"))[..],
            &include_bytes!(concat!("upstream/dictionaries/", $name, ".expected"))[..],
        )
    };
}

/// Format dictionary-content exception and the ordinary whole-window control.
#[test]
fn raw_dictionary_boundaries_accept_p_at_or_before_window_under_backpressure() {
    for (name, frame, expected) in [
        boundary_case!("p0-far-dictionary"),
        boundary_case!("p-window-minus-one"),
        boundary_case!("p-window-equal"),
        boundary_case!("output-distance-equals-window"),
    ] {
        let header = FrameHeader::parse(frame).unwrap().0;
        assert_eq!(header.window_size, 1024);
        assert_eq!(header.dictionary_id, 0);
        assert!(RAW.len() > header.window_size as usize);
        for (input_chunk, output_chunk) in [(1, 1), (7, 3), (frame.len(), 2048)] {
            let mut decoder =
                Decoder::with_dictionary(boundary_options(expected.len()), raw_dictionary());
            let output = decode_stream(&mut decoder, frame, input_chunk, output_chunk).unwrap();
            assert_eq!(output, expected, "{name}: {input_chunk}/{output_chunk}");
            decoder.finish().unwrap();
        }
    }
}

/// C accepts this physical source, but format dictionary lifetime has expired.
#[test]
fn dictionary_reference_after_window_is_rejected_even_when_c_accepts_it() {
    let (_, frame, expected) = boundary_case!("p-window-plus-one");
    for (input_chunk, output_chunk) in [(1, 1), (7, 3), (frame.len(), 2048)] {
        let mut decoder =
            Decoder::with_dictionary(boundary_options(expected.len()), raw_dictionary());
        assert_eq!(
            decode_stream(&mut decoder, frame, input_chunk, output_chunk),
            Err(Error::CorruptData)
        );
        assert_eq!(decoder.process(&[], &mut [0]), Err(Error::InvalidState));
    }
}

#[test]
fn dictionary_reference_before_content_start_is_rejected() {
    let (_, frame, _) = boundary_case!("dictionary-out-of-bounds");
    for (input_chunk, output_chunk) in [(1, 1), (frame.len(), 2048)] {
        let mut decoder = Decoder::with_dictionary(boundary_options(4096), raw_dictionary());
        assert_eq!(
            decode_stream(&mut decoder, frame, input_chunk, output_chunk),
            Err(Error::CorruptData)
        );
    }
}

/// P=900, d=1100: 200 dictionary bytes followed by frame[0..100].
#[test]
fn dictionary_to_frame_match_preserves_prefix_until_the_block_is_committed() {
    let (_, frame, expected) = boundary_case!("dictionary-to-frame-eviction");
    let overwritten =
        include_bytes!("upstream/dictionaries/dictionary-to-frame-eviction.naive-ring");
    assert_eq!(&expected[..1100], &overwritten[..1100]);
    assert_ne!(&expected[1100..], &overwritten[1100..]);
    for output_chunk in [1, 7, 64, 1024] {
        let mut decoder =
            Decoder::with_dictionary(boundary_options(expected.len()), raw_dictionary());
        assert_eq!(
            decode_stream(&mut decoder, frame, 1, output_chunk).unwrap(),
            expected
        );
        decoder.finish().unwrap();
    }
}

/// Dictionary content is not frame output and does not consume output budget.
#[test]
fn dictionary_larger_than_window_does_not_count_as_decoded_content() {
    let mut decoder = Decoder::with_dictionary(boundary_options(3), raw_dictionary());
    assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
    assert_eq!(decoder.total_output(), 3);
    assert_eq!(decoder.total_input(), FAR.len() as u64);
    decoder.finish().unwrap();
}

/// `playTests.sh` golden dictionary data and `zstreamtest.c:1120` prepared decode.
#[test]
fn prepared_http_dictionary_decodes_the_official_golden_frame() {
    let dictionary = http_dictionary();
    assert!(dictionary.is_formatted());
    assert_eq!(
        dictionary.id(),
        FrameHeader::parse(HTTP_FRAME).unwrap().0.dictionary_id
    );
    for (input_chunk, output_chunk) in [(1, 1), (7, 13), (HTTP_FRAME.len(), 4096)] {
        let mut decoder =
            Decoder::with_dictionary(DecodeOptions::new(HTTP_OUTPUT.len()), dictionary.clone());
        assert_eq!(
            decode_stream(&mut decoder, HTTP_FRAME, input_chunk, output_chunk).unwrap(),
            HTTP_OUTPUT
        );
        decoder.finish().unwrap();
    }
    assert_eq!(dictionary.as_bytes(), HTTP_DICTIONARY);
}

/// `zstreamtest.c:1344`: a masked dictionary ID does not turn dictionary use off.
#[test]
fn absent_and_explicit_zero_frame_ids_still_use_the_prepared_dictionary() {
    for id in [None, Some(0)] {
        let frame = frame_with_dictionary_id(HTTP_FRAME, id);
        let mut decoder =
            Decoder::with_dictionary(DecodeOptions::new(HTTP_OUTPUT.len()), http_dictionary());
        assert_eq!(
            decode_stream(&mut decoder, &frame, 1, 1).unwrap(),
            HTTP_OUTPUT
        );
        decoder.finish().unwrap();
    }
}

/// `zstreamtest.c:1198`: dictionary absence and a mismatched ID are distinct.
#[test]
fn nonzero_frame_id_distinguishes_missing_and_mismatched_dictionaries() {
    let mut missing = Decoder::new(DecodeOptions::new(HTTP_OUTPUT.len()));
    assert_eq!(
        decode_stream(&mut missing, HTTP_FRAME, 1, 1),
        Err(Error::DictionaryRequired)
    );
    for wrong in [
        raw_dictionary(),
        Dictionary::from_bytes(include_bytes!(
            "upstream/inline-regressions/fixed-training.dict"
        ))
        .unwrap()
        .into_shared(),
    ] {
        let mut decoder = Decoder::with_dictionary(DecodeOptions::new(HTTP_OUTPUT.len()), wrong);
        assert_eq!(
            decode_stream(&mut decoder, HTTP_FRAME, 1, 1),
            Err(Error::DictionaryMismatch)
        );
    }
}

/// `zstreamtest.c:1216,1328`: configured dictionaries persist across frames.
#[test]
fn persistent_dictionary_decodes_concatenated_frames_with_cumulative_limits() {
    let mut decoder = Decoder::with_dictionary(boundary_options(9), raw_dictionary());
    for count in 1..=3 {
        assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
        assert_eq!(decoder.total_input(), (FAR.len() * count) as u64);
        assert_eq!(decoder.total_output(), (3 * count) as u64);
    }
    assert_eq!(
        decode_stream(&mut decoder, FAR, 1, 1),
        Err(Error::ResourceLimit)
    );
}

/// Prepared tables are immutable defaults, restored independently each frame.
#[test]
fn shared_prepared_dictionary_reinitializes_entropy_for_each_context_and_frame() {
    let dictionary = http_dictionary();
    for _ in 0..2 {
        let mut decoder = Decoder::with_dictionary(
            DecodeOptions::new(HTTP_OUTPUT.len() * 3),
            dictionary.clone(),
        );
        for _ in 0..3 {
            assert_eq!(
                decode_stream(&mut decoder, HTTP_FRAME, 7, 11).unwrap(),
                HTTP_OUTPUT
            );
        }
        decoder.finish().unwrap();
    }
    assert_eq!(dictionary.as_bytes(), HTTP_DICTIONARY);
}

/// `zstreamtest.c:1222,1251`: a session reset retains its persistent dictionary.
#[test]
fn reset_and_reset_with_options_preserve_the_persistent_dictionary() {
    let mut decoder = Decoder::with_dictionary(boundary_options(3), raw_dictionary());
    for change_options in [false, true] {
        assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
        decoder.finish().unwrap();
        if change_options {
            decoder.reset_with_options(DecodeOptions {
                max_output_size: 3,
                max_window_size: 2048,
            });
        } else {
            decoder.reset();
        }
        assert_eq!(decoder.total_input(), 0);
        assert_eq!(decoder.total_output(), 0);
    }
    assert_eq!(decoder.options().max_window_size, 2048);
    assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
}

/// `zstreamtest.c:1299,1390,1403`: prefixes are used by one standard frame.
#[test]
fn prefix_is_consumed_by_one_frame_and_does_not_persist_after_reset() {
    let mut decoder = Decoder::new(boundary_options(6));
    decoder.set_prefix(raw_dictionary()).unwrap();
    assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
    assert_eq!(
        decode_stream(&mut decoder, FAR, 1, 1),
        Err(Error::CorruptData)
    );
    decoder.reset();
    assert_eq!(
        decode_stream(&mut decoder, FAR, 1, 1),
        Err(Error::CorruptData)
    );
}

#[test]
fn skippable_frames_do_not_claim_a_pending_prefix() {
    let mut decoder = Decoder::new(boundary_options(3));
    decoder.set_prefix(raw_dictionary()).unwrap();
    for payload in [&b""[..], &b"prefix remains pending"[..]] {
        let skip = skip_frame(payload);
        assert!(decode_stream(&mut decoder, &skip, 1, 1).unwrap().is_empty());
    }
    assert_eq!(decoder.total_output(), 0);
    assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
    decoder.finish().unwrap();
}

#[test]
fn reset_preserves_an_unclaimed_prefix_including_after_a_skippable_frame() {
    for change_options in [false, true] {
        let mut decoder = Decoder::new(boundary_options(3));
        decoder.set_prefix(raw_dictionary()).unwrap();
        decoder.reset();
        assert!(
            decode_stream(&mut decoder, &skip_frame(b"skip"), 1, 1)
                .unwrap()
                .is_empty()
        );
        if change_options {
            decoder.reset_with_options(boundary_options(3));
        } else {
            decoder.reset();
        }
        assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
    }
}

#[test]
fn reset_drops_a_prefix_claimed_by_an_unfinished_standard_frame() {
    let header_size = FrameHeader::parse(FAR).unwrap().1;
    for change_options in [false, true] {
        let mut decoder = Decoder::new(boundary_options(3));
        decoder.set_prefix(raw_dictionary()).unwrap();
        let progress = decoder.process(&FAR[..header_size], &mut []).unwrap();
        assert_eq!(progress.consumed, header_size);
        assert_eq!(progress.status, StreamStatus::NeedInput);
        if change_options {
            decoder.reset_with_options(boundary_options(3));
        } else {
            decoder.reset();
        }
        assert_eq!(
            decode_stream(&mut decoder, FAR, 1, 1),
            Err(Error::CorruptData)
        );
    }
}

#[test]
fn even_an_empty_standard_frame_consumes_the_pending_prefix() {
    let mut decoder = Decoder::new(boundary_options(3));
    decoder.set_prefix(raw_dictionary()).unwrap();
    assert!(
        decode_stream(&mut decoder, EMPTY_FRAME, 1, 1)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        decode_stream(&mut decoder, FAR, 1, 1),
        Err(Error::CorruptData)
    );
}

#[test]
fn setting_a_prefix_replaces_the_persistent_dictionary() {
    let mut decoder = Decoder::with_dictionary(
        DecodeOptions::new(HTTP_OUTPUT.len() + FAR_OUTPUT.len()),
        raw_dictionary(),
    );
    decoder.set_prefix(http_dictionary()).unwrap();
    assert_eq!(
        decode_stream(&mut decoder, HTTP_FRAME, 1, 1).unwrap(),
        HTTP_OUTPUT
    );
    assert_eq!(
        decode_stream(&mut decoder, FAR, 1, 1),
        Err(Error::CorruptData)
    );
}

#[test]
fn setting_or_clearing_a_dictionary_replaces_any_pending_prefix() {
    let mut decoder = Decoder::new(boundary_options(6));
    decoder.set_prefix(http_dictionary()).unwrap();
    decoder.set_dictionary(Some(raw_dictionary())).unwrap();
    for _ in 0..2 {
        assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
    }
    decoder.reset();
    decoder.set_prefix(raw_dictionary()).unwrap();
    decoder.set_dictionary(None).unwrap();
    assert_eq!(
        decode_stream(&mut decoder, FAR, 1, 1),
        Err(Error::CorruptData)
    );
}

#[test]
fn boundary_configuration_preserves_cumulative_counters_and_limits() {
    let mut decoder = Decoder::with_dictionary(boundary_options(6), raw_dictionary());
    assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
    let replacement: Vec<_> = RAW.iter().rev().copied().collect();
    decoder
        .set_dictionary(Some(Dictionary::raw(&replacement).unwrap().into_shared()))
        .unwrap();
    assert_eq!(decoder.total_input(), FAR.len() as u64);
    assert_eq!(decoder.total_output(), 3);
    assert_eq!(decoder.options().max_output_size, 6);
    assert_eq!(decoder.options().max_window_size, 1024);
    assert_eq!(
        decode_stream(&mut decoder, FAR, 1, 1).unwrap(),
        &replacement[..3]
    );
    decoder.set_prefix(raw_dictionary()).unwrap();
    assert_eq!(decoder.total_input(), (FAR.len() * 2) as u64);
    assert_eq!(decoder.total_output(), 6);
    assert_eq!(
        decode_stream(&mut decoder, FAR, 1, 1),
        Err(Error::ResourceLimit)
    );
}

#[test]
fn rejected_configuration_during_partial_headers_or_pending_output_preserves_state() {
    let header_size = FrameHeader::parse(FAR).unwrap().1;
    for offered in [1, header_size, FAR.len()] {
        let mut decoder = Decoder::with_dictionary(boundary_options(3), raw_dictionary());
        let progress = decoder.process(&FAR[..offered], &mut []).unwrap();
        assert_eq!(progress.consumed, offered);
        assert_eq!(progress.produced, 0);
        assert_eq!(decoder.set_dictionary(None), Err(Error::InvalidState));
        assert_eq!(
            decoder.set_prefix(http_dictionary()),
            Err(Error::InvalidState)
        );
        assert_eq!(decoder.total_input(), offered as u64);
        assert_eq!(decoder.total_output(), 0);
        assert_eq!(
            decode_stream(&mut decoder, &FAR[offered..], 1, 1).unwrap(),
            FAR_OUTPUT
        );
        decoder.finish().unwrap();
    }
}

#[test]
fn ended_context_rejects_configuration_without_changing_its_sealed_state() {
    let mut decoder = Decoder::with_dictionary(boundary_options(3), raw_dictionary());
    assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
    decoder.finish().unwrap();
    assert_eq!(decoder.set_dictionary(None), Err(Error::InvalidState));
    assert_eq!(
        decoder.set_prefix(http_dictionary()),
        Err(Error::InvalidState)
    );
    decoder.finish().unwrap();
    decoder.reset();
    assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
}

#[test]
fn failed_context_requires_reset_before_configuration_and_retains_its_dictionary() {
    let mut decoder = Decoder::with_dictionary(boundary_options(3), raw_dictionary());
    assert_eq!(decoder.process(&[0xff], &mut []), Err(Error::InvalidMagic));
    assert_eq!(decoder.set_dictionary(None), Err(Error::InvalidState));
    assert_eq!(
        decoder.set_prefix(http_dictionary()),
        Err(Error::InvalidState)
    );
    assert_eq!(decoder.process(FAR, &mut [0; 3]), Err(Error::InvalidState));
    decoder.reset();
    assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
}

#[test]
fn one_shot_dictionary_helper_matches_streaming_and_preserves_shared_handles() {
    let dictionary = raw_dictionary();
    for (name, frame, expected) in [
        boundary_case!("p0-far-dictionary"),
        boundary_case!("p-window-minus-one"),
        boundary_case!("p-window-equal"),
        boundary_case!("dictionary-to-frame-eviction"),
        boundary_case!("output-distance-equals-window"),
    ] {
        assert_eq!(
            decompress_with_dictionary(frame, &dictionary, boundary_options(expected.len()))
                .unwrap(),
            expected,
            "{name}"
        );
    }
    let prepared = http_dictionary();
    assert_eq!(
        decompress_with_dictionary(HTTP_FRAME, &prepared, DecodeOptions::new(HTTP_OUTPUT.len()))
            .unwrap(),
        HTTP_OUTPUT
    );
    assert_eq!(dictionary.as_bytes(), RAW);
    assert_eq!(prepared.as_bytes(), HTTP_DICTIONARY);
}

#[test]
fn one_shot_dictionary_helper_rejects_every_incomplete_frame_prefix() {
    let (_, spanning, expected) = boundary_case!("dictionary-to-frame-eviction");
    for (frame, dictionary, options) in [
        (FAR, raw_dictionary(), boundary_options(3)),
        (spanning, raw_dictionary(), boundary_options(expected.len())),
        (
            HTTP_FRAME,
            http_dictionary(),
            DecodeOptions::new(HTTP_OUTPUT.len()),
        ),
    ] {
        for end in 0..frame.len() {
            assert_eq!(
                decompress_with_dictionary(&frame[..end], &dictionary, options),
                Err(Error::UnexpectedEof),
                "frame length {}, prefix {end}",
                frame.len()
            );
        }
        assert!(decompress_with_dictionary(frame, &dictionary, options).is_ok());
    }
}

#[test]
fn one_shot_dictionary_helper_reports_mismatch_and_malformed_match_errors() {
    assert_eq!(
        decompress_with_dictionary(
            HTTP_FRAME,
            &raw_dictionary(),
            DecodeOptions::new(HTTP_OUTPUT.len())
        ),
        Err(Error::DictionaryMismatch)
    );
    for (_, frame, _) in [
        boundary_case!("p-window-plus-one"),
        boundary_case!("dictionary-out-of-bounds"),
    ] {
        assert_eq!(
            decompress_with_dictionary(frame, &raw_dictionary(), boundary_options(4096)),
            Err(Error::CorruptData)
        );
    }
    assert_eq!(
        decompress_with_dictionary(
            FAR,
            &Dictionary::raw(&[]).unwrap().into_shared(),
            boundary_options(3)
        ),
        Err(Error::CorruptData)
    );
}

#[test]
fn dictionaries_do_not_relax_advertised_window_or_output_limits() {
    let dictionary = raw_dictionary();
    for options in [
        DecodeOptions {
            max_output_size: 3,
            max_window_size: 1023,
        },
        boundary_options(2),
        boundary_options(0),
    ] {
        assert_eq!(
            decompress_with_dictionary(FAR, &dictionary, options),
            Err(Error::ResourceLimit)
        );
        let mut decoder = Decoder::with_dictionary(options, dictionary.clone());
        assert_eq!(
            decode_stream(&mut decoder, FAR, 1, 1),
            Err(Error::ResourceLimit)
        );
        assert_eq!(decoder.finish(), Err(Error::InvalidState));
    }
    // The dictionary is twice the allowed output window, but the frame needs
    // only three output bytes and is legal with the exact advertised cap.
    assert_eq!(
        decompress_with_dictionary(FAR, &dictionary, boundary_options(3)).unwrap(),
        FAR_OUTPUT
    );
}

#[test]
fn one_shot_dictionary_helper_reuses_dictionary_across_skips_and_concatenated_frames() {
    let mut stream = skip_frame(b"before");
    stream.extend_from_slice(FAR);
    stream.extend_from_slice(&skip_frame(b"between"));
    stream.extend_from_slice(FAR);
    stream.extend_from_slice(&skip_frame(b"after"));
    let expected = [FAR_OUTPUT, FAR_OUTPUT].concat();
    let dictionary = raw_dictionary();
    assert_eq!(
        decompress_with_dictionary(&stream, &dictionary, boundary_options(6)).unwrap(),
        expected
    );
    assert_eq!(
        decompress_with_dictionary(&stream, &dictionary, boundary_options(5)),
        Err(Error::ResourceLimit)
    );
    stream.push(0x28);
    assert_eq!(
        decompress_with_dictionary(&stream, &dictionary, boundary_options(6)),
        Err(Error::UnexpectedEof)
    );
    assert!(
        decompress_with_dictionary(
            &skip_frame(b"no standard frame"),
            &dictionary,
            boundary_options(0)
        )
        .unwrap()
        .is_empty()
    );
}

/// The boundary probe normally omits FCS. Add a four-byte FCS field without
/// changing its window, blocks, match, or optional dictionary-ID semantics.
#[test]
fn dictionary_content_is_excluded_from_declared_frame_content_size() {
    let (header, header_size) = FrameHeader::parse(FAR).unwrap();
    assert_eq!(header.content_size, None);
    assert_eq!(FAR[4] & 0xe0, 0);
    for content_size in [2_u32, 3, 4] {
        let mut frame = FAR[..header_size].to_vec();
        frame[4] |= 0x80;
        frame.extend_from_slice(&content_size.to_le_bytes());
        frame.extend_from_slice(&FAR[header_size..]);
        assert_eq!(
            FrameHeader::parse(&frame).unwrap().0.content_size,
            Some(u64::from(content_size))
        );
        let result = decompress_with_dictionary(&frame, &raw_dictionary(), boundary_options(16));
        if content_size == 3 {
            assert_eq!(result.unwrap(), FAR_OUTPUT);
        } else {
            assert_eq!(result, Err(Error::ContentSizeMismatch));
        }
    }
}

#[test]
fn prepared_dictionary_does_not_disable_content_checksum_validation() {
    assert!(FrameHeader::parse(HTTP_FRAME).unwrap().0.has_checksum);
    let mut damaged = HTTP_FRAME.to_vec();
    *damaged.last_mut().unwrap() ^= 0x80;
    let dictionary = http_dictionary();
    assert_eq!(
        decompress_with_dictionary(&damaged, &dictionary, DecodeOptions::new(HTTP_OUTPUT.len())),
        Err(Error::ChecksumMismatch)
    );
    let mut decoder =
        Decoder::with_dictionary(DecodeOptions::new(HTTP_OUTPUT.len()), dictionary.clone());
    assert_eq!(
        decode_stream(&mut decoder, &damaged, 1, 1),
        Err(Error::ChecksumMismatch)
    );
    assert_eq!(decoder.finish(), Err(Error::InvalidState));
    decoder.reset();
    assert_eq!(
        decode_stream(&mut decoder, HTTP_FRAME, 1, 7).unwrap(),
        HTTP_OUTPUT
    );
    assert_eq!(dictionary.as_bytes(), HTTP_DICTIONARY);
}

#[test]
fn reset_before_a_complete_standard_header_preserves_the_unclaimed_prefix() {
    let header_size = FrameHeader::parse(FAR).unwrap().1;
    for end in 0..header_size {
        let mut decoder = Decoder::new(boundary_options(3));
        decoder.set_prefix(raw_dictionary()).unwrap();
        let progress = decoder.process(&FAR[..end], &mut []).unwrap();
        assert_eq!(progress.consumed, end);
        assert_eq!(progress.produced, 0);
        decoder.reset();
        assert_eq!(decode_stream(&mut decoder, FAR, 1, 1).unwrap(), FAR_OUTPUT);
    }
}

#[test]
fn reset_after_truncation_preserves_only_unclaimed_prefixes() {
    let header_size = FrameHeader::parse(FAR).unwrap().1;
    for end in 1..FAR.len() {
        let mut decoder = Decoder::new(boundary_options(3));
        decoder.set_prefix(raw_dictionary()).unwrap();
        assert_eq!(
            decode_stream(&mut decoder, &FAR[..end], 1, 1),
            Err(Error::UnexpectedEof)
        );
        decoder.reset();
        let result = decode_stream(&mut decoder, FAR, 1, 1);
        if end < header_size {
            assert_eq!(result.unwrap(), FAR_OUTPUT);
        } else {
            assert_eq!(result, Err(Error::CorruptData));
        }
    }
}

#[test]
fn retained_capacity_counts_shared_dictionary_once_and_releases_consumed_prefix() {
    let dictionary = raw_dictionary();
    let mut persistent = Decoder::with_dictionary(boundary_options(6), dictionary.clone());
    let mut prefix = Decoder::new(boundary_options(6));
    prefix.set_prefix(dictionary.clone()).unwrap();
    let dictionary_capacity = persistent.retained_capacity();
    assert!(dictionary_capacity >= dictionary.as_bytes().len());
    assert_eq!(prefix.retained_capacity(), dictionary_capacity);
    let header_size = FrameHeader::parse(FAR).unwrap().1;
    for decoder in [&mut persistent, &mut prefix] {
        decoder.process(&FAR[..header_size], &mut []).unwrap();
        assert_eq!(decoder.history_size(), 0);
    }
    // Active and configured handles reach the same immutable allocation.
    assert_eq!(persistent.retained_capacity(), prefix.retained_capacity());
    for decoder in [&mut persistent, &mut prefix] {
        assert_eq!(
            decode_stream(decoder, &FAR[header_size..], 1, 1).unwrap(),
            FAR_OUTPUT
        );
    }
    assert_eq!(
        persistent.retained_capacity(),
        prefix.retained_capacity() + dictionary_capacity
    );
    persistent.set_dictionary(None).unwrap();
    assert_eq!(persistent.retained_capacity(), prefix.retained_capacity());
    persistent.set_dictionary(Some(dictionary.clone())).unwrap();
    persistent.reset();
    prefix.reset();
    assert_eq!(
        persistent.retained_capacity(),
        prefix.retained_capacity() + dictionary_capacity
    );
    assert_eq!(dictionary.as_bytes(), RAW);
}

#[test]
fn prepared_dictionary_retention_stays_bounded_across_many_frame_resets() {
    let dictionary = http_dictionary();
    let mut decoder = Decoder::with_dictionary(
        DecodeOptions::new(HTTP_OUTPUT.len() * 32),
        dictionary.clone(),
    );
    let dictionary_capacity = decoder.retained_capacity();
    assert!(dictionary_capacity > dictionary.as_bytes().len());
    assert_eq!(
        decode_stream(&mut decoder, HTTP_FRAME, 7, 31).unwrap(),
        HTTP_OUTPUT
    );
    let plateau = decoder.retained_capacity();
    assert!(plateau >= dictionary_capacity);
    for _ in 0..31 {
        assert_eq!(
            decode_stream(&mut decoder, HTTP_FRAME, 7, 31).unwrap(),
            HTTP_OUTPUT
        );
        assert_eq!(decoder.retained_capacity(), plateau);
    }
    assert_eq!(decoder.total_output(), (32 * HTTP_OUTPUT.len()) as u64);
    decoder.reset();
    assert_eq!(decoder.retained_capacity(), plateau);
    decoder.set_dictionary(None).unwrap();
    assert_eq!(decoder.retained_capacity(), plateau - dictionary_capacity);
    assert_eq!(dictionary.as_bytes(), HTTP_DICTIONARY);
}

fn formatted_dictionary() -> SharedDictionary {
    Dictionary::from_bytes(include_bytes!("upstream/dictionaries/formatted.dict"))
        .unwrap()
        .into_shared()
}

fn trained_dictionary() -> SharedDictionary {
    Dictionary::from_bytes(include_bytes!("upstream/dictionaries/trained.dict"))
        .unwrap()
        .into_shared()
}

fn duplicate_repeat_dictionary() -> SharedDictionary {
    Dictionary::from_bytes(include_bytes!("upstream/dictionaries/duplicate-reps.dict"))
        .unwrap()
        .into_shared()
}

/// Official compression paths: raw/prepared dictionary, omitted ID, training,
/// and `fuzzer.c:3265` duplicate legal repcodes with long repeated input.
#[test]
fn official_raw_prepared_trained_and_duplicate_repeat_frames_decode_exactly() {
    assert_eq!(formatted_dictionary().content(), RAW);
    for (dictionary, (name, frame, expected)) in [
        (raw_dictionary(), boundary_case!("raw-small-window")),
        (formatted_dictionary(), boundary_case!("formatted-id")),
        (
            formatted_dictionary(),
            boundary_case!("formatted-omitted-id"),
        ),
        (trained_dictionary(), boundary_case!("trained-sample")),
        (
            duplicate_repeat_dictionary(),
            boundary_case!("duplicate-reps"),
        ),
    ] {
        let header = FrameHeader::parse(frame).unwrap().0;
        assert!(dictionary.content().len() > header.window_size as usize);
        let options = DecodeOptions {
            max_output_size: expected.len(),
            max_window_size: header.window_size,
        };
        assert_eq!(
            decompress_with_dictionary(frame, &dictionary, options).unwrap(),
            expected,
            "{name}"
        );
        for (input_chunk, output_chunk) in [(1, 1), (7, 31)] {
            let mut decoder = Decoder::with_dictionary(options, dictionary.clone());
            assert_eq!(
                decode_stream(&mut decoder, frame, input_chunk, output_chunk).unwrap(),
                expected,
                "{name}"
            );
            decoder.finish().unwrap();
        }
    }
}

/// No earlier frame block can supply the Huffman table: both fixture forms
/// begin with treeless literals and use the dictionary's prepared table.
#[test]
fn first_block_treeless_literals_use_prepared_huffman_in_one_and_four_stream_modes() {
    let dictionary = formatted_dictionary();
    for (four_streams, (name, frame, expected)) in [
        (false, boundary_case!("dictionary-treeless-literals")),
        (true, boundary_case!("dictionary-treeless-four-streams")),
    ] {
        let header_size = FrameHeader::parse(frame).unwrap().1;
        assert_eq!((frame[header_size] >> 1) & 3, 2);
        let literal_header = frame[header_size + 3];
        assert_eq!(literal_header & 3, 3);
        assert_eq!((literal_header >> 2) & 3 != 0, four_streams);
        assert_eq!(expected.len(), 768);
        let mut decoder =
            Decoder::with_dictionary(boundary_options(expected.len() * 2), dictionary.clone());
        for _ in 0..2 {
            assert_eq!(
                decode_stream(&mut decoder, frame, 1, 1).unwrap(),
                expected,
                "{name}"
            );
        }
        decoder.finish().unwrap();
        // Removing only the dictionary's prepared tables keeps identical raw
        // content, but cannot satisfy first-block treeless literal decoding.
        let raw = Dictionary::raw(dictionary.content()).unwrap().into_shared();
        let no_id = frame_with_dictionary_id(frame, None);
        assert_eq!(
            decompress_with_dictionary(&no_id, &raw, boundary_options(expected.len())),
            Err(Error::CorruptData)
        );
    }
}

/// The independently inspected first block uses treeless literals and Repeat
/// mode for LL, OF, and ML; no prior working sequence tables exist.
#[test]
fn first_block_repeat_sequence_tables_are_restored_for_every_prepared_frame() {
    let (_, frame, expected) = boundary_case!("dictionary-repeat-sequences");
    let dictionary = trained_dictionary();
    let header_size = FrameHeader::parse(frame).unwrap().1;
    let literals = header_size + 3;
    assert_eq!(frame[literals] & 3, 3);
    // This fixture's three-byte literal header carries a ten-bit encoded
    // literal size at bit14; one-byte sequence count follows the literal data.
    assert!((frame[literals] >> 2) & 3 <= 1);
    let literal_header =
        u32::from_le_bytes([frame[literals], frame[literals + 1], frame[literals + 2], 0]);
    let encoded_literals = ((literal_header >> 14) & 0x3ff) as usize;
    let sequence_count = literals + 3 + encoded_literals;
    assert_eq!(frame[sequence_count], 54);
    assert_eq!(frame[sequence_count + 1], 0xfc);
    for _ in 0..2 {
        let mut decoder =
            Decoder::with_dictionary(boundary_options(expected.len() * 3), dictionary.clone());
        for _ in 0..3 {
            assert_eq!(decode_stream(&mut decoder, frame, 1, 1).unwrap(), expected);
        }
        decoder.finish().unwrap();
        decoder.reset_with_options(boundary_options(expected.len()));
        assert_eq!(decode_stream(&mut decoder, frame, 7, 31).unwrap(), expected);
    }
    assert_eq!(
        decompress_with_dictionary(frame, &dictionary, boundary_options(expected.len())).unwrap(),
        expected
    );
}

/// LL0/OF0 selects rep2. The duplicate initial values are 10, whereas a raw
/// dictionary would begin with rep2=4 and produce three different bytes.
#[test]
fn duplicate_initial_repeat_offsets_are_used_and_reset_between_frames() {
    let (_, frame, expected) = boundary_case!("duplicate-reps-initial");
    let dictionary = duplicate_repeat_dictionary();
    assert_eq!(expected, &[0x73, 0x22, 0x78]);
    let mut decoder = Decoder::with_dictionary(boundary_options(9), dictionary.clone());
    for _ in 0..3 {
        assert_eq!(decode_stream(&mut decoder, frame, 1, 1).unwrap(), expected);
    }
    decoder.finish().unwrap();
    let raw = Dictionary::raw(dictionary.content()).unwrap().into_shared();
    let no_id = frame_with_dictionary_id(frame, None);
    assert_eq!(
        decompress_with_dictionary(&no_id, &raw, boundary_options(3)).unwrap(),
        b"xxx"
    );
    assert_eq!(
        decompress_with_dictionary(frame, &dictionary, boundary_options(3)).unwrap(),
        expected
    );
}

#[test]
fn official_prepared_dictionary_persists_across_an_unsplit_concatenated_input() {
    let (_, stream, expected) = boundary_case!("persistent-concatenation");
    let dictionary = formatted_dictionary();
    for (input_chunk, output_chunk) in [(1, 1), (stream.len(), 4096)] {
        let mut decoder =
            Decoder::with_dictionary(boundary_options(expected.len()), dictionary.clone());
        assert_eq!(
            decode_stream(&mut decoder, stream, input_chunk, output_chunk).unwrap(),
            expected
        );
        decoder.finish().unwrap();
    }
    assert_eq!(
        decompress_with_dictionary(stream, &dictionary, boundary_options(expected.len())).unwrap(),
        expected
    );
}

/// `zstreamtest.c:1299,1390,1403`: an official prefix frame followed by an
/// independent frame succeeds; repeating a dependent frame requires a new
/// prefix or a persistent dictionary.
#[test]
fn official_prefix_concatenations_consume_the_prefix_exactly_once() {
    let (_, once, once_output) = boundary_case!("prefix-once-concatenation");
    let (_, repeated, repeated_output) = boundary_case!("prefix-reused-concatenation");
    for (input_chunk, output_chunk) in [(1, 1), (once.len(), 4096)] {
        let mut decoder = Decoder::new(boundary_options(once_output.len()));
        decoder.set_prefix(raw_dictionary()).unwrap();
        assert_eq!(
            decode_stream(&mut decoder, once, input_chunk, output_chunk).unwrap(),
            once_output
        );
        decoder.finish().unwrap();
        decoder.reset();
        decoder.set_prefix(raw_dictionary()).unwrap();
        assert_eq!(
            decode_stream(&mut decoder, repeated, input_chunk, output_chunk),
            Err(Error::CorruptData)
        );
    }
    assert_eq!(
        decompress_with_dictionary(
            repeated,
            &raw_dictionary(),
            boundary_options(repeated_output.len())
        )
        .unwrap(),
        repeated_output
    );
    let (_, second, second_output) = boundary_case!("prefix-second");
    let mut decoder = Decoder::new(boundary_options(second_output.len()));
    assert_eq!(
        decode_stream(&mut decoder, second, 1, 1).unwrap(),
        second_output
    );
}
