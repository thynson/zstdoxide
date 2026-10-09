// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Derived regression bytes and contracts from facebook/zstd v1.5.7.
// Used under the BSD-3-Clause option; see LICENSE.zstd.
//! Bounded streaming contracts from `zstreamtest.c` and streaming fuzz targets.
//!
//! See `docs/upstream-streaming-plan.md` for exact source groups and exclusions.

use zstdoxide::{
    DecodeOptions, Decoder, Encoder, EncoderOptions, EndDirective, Error, FrameHeader,
    StreamStatus, compress, decompress,
};

#[path = "upstream/fuzz_support.rs"]
#[allow(dead_code)]
mod fuzz_support;

const SMALL_SEQUENCES: &[u8] = &[
    0x28, 0xb5, 0x2f, 0xfd, 0x24, 0x3c, 0x35, 0x01, 0x00, 0xf0, 0x85, 0x08, 0xc2, 0xc4, 0x70, 0xcf,
    0xd7, 0xc0, 0x96, 0x7e, 0x4c, 0x6b, 0xa9, 0x8b, 0xbc, 0xc5, 0xb6, 0xd9, 0x7f, 0x4c, 0xf1, 0x05,
    0xa6, 0x54, 0xef, 0xac, 0x69, 0x94, 0x89, 0x1c, 0x03, 0x44, 0x0a, 0x07, 0x00, 0xb4, 0x04, 0x80,
    0x40, 0x0a, 0xa4,
];
const SMALL_SEQUENCES_OUTPUT: &[u8] = &[
    0x85, 0x08, 0xc2, 0xc4, 0x70, 0xcf, 0xd7, 0xc0, 0x96, 0x7e, 0x85, 0x08, 0xc2, 0xc4, 0x70, 0xcf,
    0xd7, 0xc0, 0x96, 0x7e, 0x4c, 0x6b, 0xa9, 0x8b, 0xbc, 0xc5, 0xb6, 0xd9, 0x7f, 0x4c, 0x4c, 0x6b,
    0xa9, 0x8b, 0xbc, 0xc5, 0xb6, 0xd9, 0x7f, 0x4c, 0xf1, 0x05, 0xa6, 0x54, 0xef, 0xac, 0x69, 0x94,
    0x89, 0x1c, 0xf1, 0x05, 0xa6, 0x54, 0xef, 0xac, 0x69, 0x94, 0x89, 0x1c,
];
const HISTORY_OVERWRITE: &[u8] = &[
    0x28, 0xb5, 0x2f, 0xfd, 0x04, 0x00, 0x4c, 0x00, 0x00, 0x10, 0x61, 0x61, 0x01, 0x00, 0x00, 0x2a,
    0x80, 0x05, 0x44, 0x00, 0x00, 0x08, 0x62, 0x01, 0x00, 0x00, 0x2a, 0x20, 0x04, 0x5d, 0x00, 0x00,
    0x00, 0x03, 0x40, 0x00, 0x00, 0x64, 0x60, 0x27, 0xb0, 0xe0, 0x0c, 0x67, 0x62, 0xce, 0xe0,
];

fn raw_frame(source: &[u8], window_log: u8) -> Vec<u8> {
    assert!((10..=17).contains(&window_log));
    let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0, (window_log - 10) << 3];
    let block_size = 1_usize << window_log;
    if source.is_empty() {
        frame.extend_from_slice(&[1, 0, 0]);
    } else {
        let block_count = source.len().div_ceil(block_size);
        for (index, block) in source.chunks(block_size).enumerate() {
            let header = ((block.len() as u32) << 3) | u32::from(index + 1 == block_count);
            frame.extend_from_slice(&header.to_le_bytes()[..3]);
            frame.extend_from_slice(block);
        }
    }
    frame
}

fn skip_frame(source: &[u8], variant: u8) -> Vec<u8> {
    assert!(variant < 16);
    let mut frame = (0x184d_2a50_u32 + u32::from(variant))
        .to_le_bytes()
        .to_vec();
    frame.extend_from_slice(&(source.len() as u32).to_le_bytes());
    frame.extend_from_slice(source);
    frame
}

/// Resubmit every unconsumed suffix and regularly insert empty input/output.
fn decode_chunks(
    decoder: &mut Decoder,
    encoded: &[u8],
    input_chunk: usize,
    output_chunk: usize,
) -> (Vec<u8>, Vec<usize>) {
    decode_scheduled(decoder, encoded, input_chunk, output_chunk, None)
}

fn decode_scheduled(
    decoder: &mut Decoder,
    encoded: &[u8],
    input_chunk: usize,
    output_chunk: usize,
    mut random_seed: Option<u32>,
) -> (Vec<u8>, Vec<usize>) {
    assert!(input_chunk != 0 && output_chunk != 0);
    let mut input_position = 0;
    let mut recovered = Vec::new();
    let mut frame_ends = Vec::new();
    let mut calls = 0;
    loop {
        calls += 1;
        assert!(calls < 4_000_000, "stream did not make bounded progress");
        let input_size = if calls % 11 == 0 {
            0
        } else if let Some(seed) = random_seed.as_mut() {
            1 + next_random(seed) as usize % input_chunk
        } else {
            input_chunk
        };
        let output_size = if calls % 7 == 0 {
            0
        } else if let Some(seed) = random_seed.as_mut() {
            1 + next_random(seed) as usize % output_chunk
        } else {
            output_chunk
        };
        let input_end = (input_position + input_size).min(encoded.len());
        let input = &encoded[input_position..input_end];
        let mut output = vec![0xa5; output_size + 8];
        let progress = decoder.process(input, &mut output[..output_size]).unwrap();
        assert!(progress.consumed <= input.len());
        assert!(progress.produced <= output_size);
        assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
        assert_ne!(progress.status, StreamStatus::Flushed);
        if progress.consumed == 0 && progress.produced == 0 {
            assert!(
                input.is_empty()
                    || output_size == 0
                    || progress.status == StreamStatus::FrameFinished,
                "no progress with input and output: {:?}",
                progress.status
            );
        }
        input_position += progress.consumed;
        recovered.extend_from_slice(&output[..progress.produced]);
        if progress.status == StreamStatus::FrameFinished {
            frame_ends.push(input_position);
            if input_position == encoded.len() {
                break;
            }
        }
        assert!(
            input_position != encoded.len() || progress.status != StreamStatus::NeedInput,
            "valid complete stream requested unavailable input"
        );
    }
    decoder.finish().unwrap();
    assert_eq!(decoder.total_input(), encoded.len() as u64);
    assert_eq!(decoder.total_output(), recovered.len() as u64);
    (recovered, frame_ends)
}

/// `zstreamtest.c:1743`: three blocks preserve repeat tables in tiny sections.
#[test]
fn exact_small_sequence_sections_survive_every_chunk_boundary() {
    for input_chunk in [1, 2, 3, 7, 51] {
        for output_chunk in [1, 2, 3, 7, 60] {
            let mut decoder = Decoder::new(DecodeOptions::new(60));
            let (actual, boundaries) =
                decode_chunks(&mut decoder, SMALL_SEQUENCES, input_chunk, output_chunk);
            assert_eq!(actual, SMALL_SEQUENCES_OUTPUT);
            assert_eq!(boundaries, [SMALL_SEQUENCES.len()]);
        }
    }
}

/// `zstreamtest.c:1710`: an old match must survive a ring wrap and short copies.
#[test]
fn exact_history_overwrite_regression_survives_output_fragmentation() {
    let mut expected = vec![b'a'; 517];
    expected.extend_from_slice(&vec![b'b'; 543]);
    expected.extend_from_slice(b"aaa");
    for output_chunk in [1, 3, 24, 517, 1024] {
        let mut decoder = Decoder::new(DecodeOptions::new(expected.len()));
        let (actual, _) = decode_chunks(&mut decoder, HISTORY_OVERWRITE, 7, output_chunk);
        assert_eq!(actual, expected);
    }
}

/// The dictionary-free `decodecorpus.c::runFrameTest` streaming contract.
#[test]
fn every_official_generated_frame_survives_independent_input_output_chunks() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/upstream/decodecorpus");
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "zst"))
        .collect();
    paths.sort();
    // The manifest contains 96 complete frames plus 16 header-only .zst files.
    assert_eq!(paths.len(), 112, "every generated asset must be classified");
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    let mut complete_frames = 0;
    let mut standalone_headers = 0;
    for path in paths {
        let encoded = std::fs::read(&path).unwrap();
        if path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("header-")
        {
            standalone_headers += 1;
            decoder.reset_with_options(DecodeOptions::new(usize::MAX));
            let progress = decoder.process(&encoded, &mut []).unwrap();
            assert_eq!(progress.consumed, encoded.len());
            assert_eq!(progress.produced, 0);
            assert_eq!(progress.status, StreamStatus::NeedInput);
            assert_eq!(decoder.finish(), Err(Error::UnexpectedEof));
            continue;
        }
        complete_frames += 1;
        let expected = std::fs::read(path.with_extension("raw")).unwrap();
        for (input_chunk, output_chunk) in [(1, 17), (127, 3)] {
            decoder.reset_with_options(DecodeOptions::new(expected.len()));
            let (actual, boundaries) =
                decode_chunks(&mut decoder, &encoded, input_chunk, output_chunk);
            assert_eq!(actual, expected, "{}", path.display());
            assert_eq!(boundaries, [encoded.len()], "{}", path.display());
        }
    }
    assert_eq!(complete_frames, 96);
    assert_eq!(standalone_headers, 16);
}

/// `zstreamtest.c:1794`: output must precede receipt of the whole raw block.
#[test]
fn raw_block_outputs_immediately_under_ten_byte_backpressure() {
    let source: Vec<_> = (0..10_000).map(|index| (index * 73) as u8).collect();
    let encoded = raw_frame(&source, 17);
    let mut decoder = Decoder::new(DecodeOptions::new(source.len()));
    let mut available = 0;
    let mut consumed = 0;
    let mut actual = Vec::new();
    let mut status = StreamStatus::NeedInput;
    while available < encoded.len() {
        available = (available + 100).min(encoded.len());
        while consumed < available {
            let mut output = [0_u8; 10];
            let progress = decoder
                .process(&encoded[consumed..available], &mut output)
                .unwrap();
            assert!(
                progress.produced > 0,
                "raw payload was buffered without output"
            );
            assert!(progress.consumed <= available - consumed);
            assert!(progress.produced <= output.len());
            consumed += progress.consumed;
            actual.extend_from_slice(&output[..progress.produced]);
            status = progress.status;
        }
        assert_eq!(actual.len(), available.saturating_sub(9));
    }
    assert_eq!(status, StreamStatus::FrameFinished);
    assert_eq!(actual, source);
    decoder.finish().unwrap();
}

/// `zstreamtest.c:526`, `:702`, and the skip-frame boundary in `:385`.
#[test]
fn empty_and_skippable_frames_finish_with_zero_output() {
    let frames = [
        raw_frame(b"", 10),
        skip_frame(b"payload", 0),
        skip_frame(b"", 15),
    ];
    let encoded: Vec<_> = frames.concat();
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    let mut position = 0;
    for frame in frames {
        let progress = decoder.process(&encoded[position..], &mut []).unwrap();
        assert_eq!(progress.status, StreamStatus::FrameFinished);
        assert_eq!(progress.consumed, frame.len());
        assert_eq!(progress.produced, 0);
        position += progress.consumed;
    }
    assert_eq!(position, encoded.len());
    decoder.finish().unwrap();
}

/// `zstreamtest.c:458`: boundaries remain exact with bytewise input/output.
#[test]
fn concatenated_frames_report_each_boundary_before_the_next_magic() {
    let frames = [
        raw_frame(b"first", 10),
        skip_frame(b"ignored", 2),
        raw_frame(b"", 10),
        SMALL_SEQUENCES.to_vec(),
        raw_frame(b"last", 10),
    ];
    let encoded = frames.concat();
    let expected = [b"first".as_slice(), SMALL_SEQUENCES_OUTPUT, b"last"].concat();
    let mut position = 0;
    let ends: Vec<_> = frames
        .iter()
        .map(|frame| {
            position += frame.len();
            position
        })
        .collect();
    for input_chunk in [1, 7, encoded.len()] {
        let mut decoder = Decoder::new(DecodeOptions::new(expected.len()));
        let (actual, boundaries) = decode_chunks(&mut decoder, &encoded, input_chunk, 3);
        assert_eq!(actual, expected);
        assert_eq!(boundaries, ends);
    }
}

#[test]
fn frame_completion_leaves_an_invalid_following_prefix_unconsumed() {
    let frame = raw_frame(b"valid frame", 10);
    let mut joined = frame.clone();
    joined.extend_from_slice(&[0, 0, 0]);
    let mut decoder = Decoder::new(DecodeOptions::new(32));
    let mut output = [0; 32];
    let progress = decoder.process(&joined, &mut output).unwrap();
    assert_eq!(progress.status, StreamStatus::FrameFinished);
    assert_eq!(progress.consumed, frame.len());
    assert_eq!(&output[..progress.produced], b"valid frame");
    assert_eq!(
        decoder.process(&joined[progress.consumed..], &mut output),
        Err(Error::InvalidMagic)
    );
}

/// `zstreamtest.c:429`: an impossible short prefix already proves bad magic.
#[test]
fn invalid_magic_prefix_errors_early_and_requires_reset() {
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    assert_eq!(
        decoder.process(&[0, 0, 0], &mut []),
        Err(Error::InvalidMagic)
    );
    assert_eq!(
        decoder.process(&raw_frame(b"", 10), &mut []),
        Err(Error::InvalidState)
    );
    decoder.reset();
    let progress = decoder.process(&raw_frame(b"", 10), &mut []).unwrap();
    assert_eq!(progress.status, StreamStatus::FrameFinished);
    decoder.finish().unwrap();
}

#[test]
fn checksum_bytes_are_required_after_plaintext_has_been_delivered() {
    let expected = b"checksum split after provisional plaintext";
    let encoded = compress(expected).unwrap();
    let mut decoder = Decoder::new(DecodeOptions::new(expected.len()));
    let mut output = [0_u8; 128];
    let before_checksum = encoded.len() - 4;
    let first = decoder
        .process(&encoded[..before_checksum], &mut output)
        .unwrap();
    assert_eq!(first.consumed, before_checksum);
    assert_eq!(&output[..first.produced], expected);
    assert_eq!(first.status, StreamStatus::NeedInput);
    for index in 0..4 {
        let progress = decoder
            .process(
                &encoded[before_checksum + index..before_checksum + index + 1],
                &mut [],
            )
            .unwrap();
        assert_eq!(progress.consumed, 1);
        assert_eq!(progress.produced, 0);
        assert_eq!(
            progress.status,
            if index == 3 {
                StreamStatus::FrameFinished
            } else {
                StreamStatus::NeedInput
            }
        );
    }
    decoder.finish().unwrap();
}

#[test]
fn checksum_failure_poisoning_can_be_cleared_by_reset() {
    let expected = b"checksum failure after produced bytes";
    let mut corrupted = compress(expected).unwrap();
    *corrupted.last_mut().unwrap() ^= 1;
    let mut decoder = Decoder::new(DecodeOptions::new(expected.len()));
    let mut output = [0_u8; 128];
    let checksum_start = corrupted.len() - 4;
    let progress = decoder
        .process(&corrupted[..checksum_start], &mut output)
        .unwrap();
    assert_eq!(&output[..progress.produced], expected);
    assert_eq!(
        decoder.process(&corrupted[checksum_start..], &mut output),
        Err(Error::ChecksumMismatch)
    );
    assert_eq!(decoder.process(&[], &mut []), Err(Error::InvalidState));
    decoder.reset();
    let (actual, _) = decode_chunks(&mut decoder, &raw_frame(expected, 10), 1, 7);
    assert_eq!(actual, expected);
}

#[test]
fn eof_rejects_every_proper_header_block_payload_and_checksum_prefix() {
    let cases = [
        raw_frame(b"abcdef", 10),
        compress(b"checksummed").unwrap(),
        skip_frame(b"skip payload", 3),
        SMALL_SEQUENCES.to_vec(),
    ];
    for frame in cases {
        for end in 0..frame.len() {
            let mut decoder = Decoder::new(DecodeOptions::new(1024));
            let mut output = [0_u8; 1024];
            let progress = decoder.process(&frame[..end], &mut output).unwrap();
            assert_eq!(progress.consumed, end);
            assert_ne!(progress.status, StreamStatus::FrameFinished);
            assert_eq!(
                decoder.finish(),
                Err(Error::UnexpectedEof),
                "prefix {end}/{}",
                frame.len()
            );
        }
    }
}

#[test]
fn eof_seals_a_completed_decoder_until_reset() {
    let frame = raw_frame(b"", 10);
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    assert_eq!(
        decoder.process(&frame, &mut []).unwrap().status,
        StreamStatus::FrameFinished
    );
    decoder.finish().unwrap();
    assert_eq!(decoder.process(&frame, &mut []), Err(Error::InvalidState));
    decoder.reset();
    assert_eq!(decoder.total_input(), 0);
    assert_eq!(decoder.total_output(), 0);
    assert_eq!(
        decoder.process(&frame, &mut []).unwrap().status,
        StreamStatus::FrameFinished
    );
}

#[test]
fn reset_discards_each_partial_frame_phase_and_pending_output() {
    let frames = [
        raw_frame(b"partial raw payload", 10),
        compress(&vec![b'z'; 4096]).unwrap(),
        skip_frame(b"partial skipped payload", 1),
        SMALL_SEQUENCES.to_vec(),
    ];
    let clean = raw_frame(b"independent frame", 10);
    for frame in frames {
        for end in 0..frame.len() {
            let mut decoder = Decoder::new(DecodeOptions::new(8192));
            let mut output = [0_u8; 1];
            let progress = decoder.process(&frame[..end], &mut output).unwrap();
            assert!(progress.consumed <= end);
            decoder.reset();
            assert_eq!(decoder.total_input(), 0);
            assert_eq!(decoder.total_output(), 0);
            let (actual, _) = decode_chunks(&mut decoder, &clean, 3, 2);
            assert_eq!(actual, b"independent frame");
        }
    }
}

/// `zstreamtest.c:1139`: limits persist across session reset.
#[test]
fn window_limit_rejection_and_reconfiguration_are_explicit() {
    let options = DecodeOptions {
        max_output_size: 32,
        max_window_size: 1024,
    };
    let frame = raw_frame(b"larger advertised window", 17);
    let mut decoder = Decoder::new(options);
    assert_eq!(
        decoder.process(&frame, &mut [0; 32]),
        Err(Error::ResourceLimit)
    );
    decoder.reset();
    assert_eq!(
        decoder.process(&frame, &mut [0; 32]),
        Err(Error::ResourceLimit)
    );
    decoder.reset_with_options(DecodeOptions::new(32));
    let (actual, _) = decode_chunks(&mut decoder, &frame, 2, 3);
    assert_eq!(actual, b"larger advertised window");
}

#[test]
fn cumulative_output_limit_applies_across_frame_boundaries() {
    let frame = raw_frame(b"12345", 10);
    let mut decoder = Decoder::new(DecodeOptions::new(9));
    let first = decoder.process(&frame, &mut [0; 16]).unwrap();
    assert_eq!(first.status, StreamStatus::FrameFinished);
    assert_eq!(
        decoder.process(&frame, &mut [0; 16]),
        Err(Error::ResourceLimit)
    );
    decoder.reset();
    assert_eq!(
        decoder.process(&frame, &mut [0; 16]).unwrap().status,
        StreamStatus::FrameFinished
    );
}

#[test]
fn rle_output_drains_without_more_payload_input() {
    // Unknown-size frame, 1 KiB window, final RLE block of 1024 'r' bytes.
    let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0, 0];
    frame.extend_from_slice(&((1024_u32 << 3) | 3).to_le_bytes()[..3]);
    frame.push(b'r');
    let mut decoder = Decoder::new(DecodeOptions::new(1024));
    let first = decoder.process(&frame, &mut []).unwrap();
    assert!(first.consumed <= frame.len());
    assert_eq!(first.produced, 0);
    assert_eq!(first.status, StreamStatus::NeedOutput);
    let mut position = first.consumed;
    let mut actual = Vec::new();
    loop {
        let mut output = [0; 7];
        let progress = decoder.process(&frame[position..], &mut output).unwrap();
        assert!(progress.produced > 0);
        position += progress.consumed;
        actual.extend_from_slice(&output[..progress.produced]);
        if progress.status == StreamStatus::FrameFinished {
            break;
        }
        assert_eq!(progress.status, StreamStatus::NeedOutput);
    }
    assert_eq!(position, frame.len());
    assert_eq!(actual, vec![b'r'; 1024]);
    decoder.finish().unwrap();
}

#[test]
fn retained_decoder_memory_does_not_grow_with_delivered_output() {
    let block: Vec<_> = (0..1024).map(|index| (index * 53) as u8).collect();
    let source = block.repeat(4096);
    let frame = raw_frame(&source, 10);
    let mut decoder = Decoder::new(DecodeOptions::new(source.len()));
    let (actual, _) = decode_chunks(&mut decoder, &frame, 8192, 4096);
    assert_eq!(actual, source);
    assert!(
        decoder.retained_capacity() < 1024 * 1024,
        "retained memory grew with a 4 MiB output and 1 KiB window"
    );
    let capacity = decoder.retained_capacity();
    decoder.reset();
    assert_eq!(decoder.retained_capacity(), capacity);
    let (again, _) = decode_chunks(&mut decoder, &frame, 8192, 4096);
    assert_eq!(again, source);
    assert_eq!(decoder.retained_capacity(), capacity);
}

#[test]
fn huge_skippable_length_is_a_counter_not_a_payload_allocation() {
    let mut prefix = skip_frame(b"", 0);
    prefix[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    let progress = decoder.process(&prefix, &mut []).unwrap();
    assert_eq!(progress.consumed, prefix.len());
    assert_eq!(progress.status, StreamStatus::NeedInput);
    assert!(decoder.retained_capacity() < 1024 * 1024);
    assert_eq!(decoder.finish(), Err(Error::UnexpectedEof));
}

fn next_random(state: &mut u32) -> u32 {
    *state = state
        .wrapping_mul(2_654_435_761)
        .wrapping_add(2_246_822_519)
        .rotate_left(13);
    *state >> 5
}

/// Preserve the upstream prefix reservation while choosing its parameter tail.
fn streaming_fuzz_case(frame: &[u8], parameters: &[u8]) -> Vec<u8> {
    for selector_bytes in 0..=4 {
        let length = frame.len() + parameters.len() + selector_bytes;
        let width = ((u32::BITS - (length as u32).leading_zeros()) as usize).div_ceil(8);
        if width != selector_bytes {
            continue;
        }
        let mut input = frame.to_vec();
        input.extend_from_slice(parameters);
        input.extend_from_slice(&(parameters.len() as u32).to_le_bytes()[..selector_bytes]);
        let mut check = fuzz_support::DataProducer::new(&input);
        assert_eq!(check.reserve_prefix(), frame.len());
        return input;
    }
    panic!("could not encode a consistent streaming parameter tail");
}

/// Buffered-output branch of `tests/fuzz/stream_decompress.c:68`.
fn stream_decompression_target(decoder: &mut Decoder, input: &[u8]) {
    stream_decompression_target_with_limits(decoder, input, 65536, 1024 * 1024, 8 * 1024 * 1024);
}

fn stream_decompression_target_with_limits(
    decoder: &mut Decoder,
    input: &[u8],
    max_input: usize,
    max_output: usize,
    max_window: u64,
) {
    assert!(input.len() <= max_input);
    let mut producer = fuzz_support::DataProducer::new(input);
    let source_size = producer.reserve_prefix();
    let source = &input[..source_size];
    let buffer_size = (10 * source_size).max(128 * 1024);
    // Consume the original selector even though stable-output mode is pending.
    let _stable_output_requested = producer.unsigned_range(0, 10) == 5;
    decoder.reset_with_options(DecodeOptions {
        max_output_size: max_output,
        max_window_size: max_window,
    });
    let mut output = vec![0xa5; buffer_size + 8];
    let mut output_size = if producer.is_empty() {
        buffer_size
    } else {
        producer.unsigned_range(0, buffer_size as u32) as usize
    };
    let mut output_position = 0;
    let mut source_position = 0;
    let mut calls = 0;
    while source_position < source.len() {
        let remaining = source.len() - source_position;
        let selected = if producer.is_empty() {
            remaining
        } else {
            producer.unsigned_range(0, remaining as u32) as usize
        };
        let chunk = &source[source_position..source_position + selected];
        source_position += selected;
        let mut chunk_position = 0;
        loop {
            calls += 1;
            assert!(
                calls < 2_000_000,
                "malformed-input target did not terminate"
            );
            output[output_position..].fill(0xa5);
            let result = decoder.process(
                &chunk[chunk_position..],
                &mut output[output_position..output_size],
            );
            let progress = match result {
                Ok(progress) => progress,
                Err(_) => {
                    assert_eq!(decoder.process(&[], &mut []), Err(Error::InvalidState));
                    return;
                }
            };
            assert!(progress.consumed <= chunk.len() - chunk_position);
            assert!(progress.produced <= output_size - output_position);
            assert!(
                output[output_position + progress.produced..]
                    .iter()
                    .all(|&byte| byte == 0xa5)
            );
            assert_ne!(progress.status, StreamStatus::Flushed);
            if progress.consumed == 0 && progress.produced == 0 {
                assert!(
                    chunk_position == chunk.len()
                        || output_position == output_size
                        || progress.status == StreamStatus::FrameFinished
                );
            }
            chunk_position += progress.consumed;
            output_position += progress.produced;
            assert!(decoder.total_output() <= max_output as u64);
            if output_position == output_size {
                output_position = 0;
                output_size = if producer.is_empty() {
                    buffer_size
                } else {
                    producer.unsigned_range(0, buffer_size as u32) as usize
                };
            }
            if chunk_position == chunk.len() {
                break;
            }
        }
    }
    // Unlike temporary input exhaustion, EOF may expose an incomplete frame.
    let _ = decoder.finish();
    assert!(decoder.retained_capacity() <= max_window as usize + 2 * 1024 * 1024);
}

/// Safe arbitrary-input branch, with explicit test resource caps.
#[test]
fn streaming_fuzz_target_consumes_tail_selected_chunks_without_panics() {
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    for seed in 0..4096 {
        let mut state = seed;
        let length = next_random(&mut state) as usize % 513;
        let input: Vec<_> = (0..length).map(|_| next_random(&mut state) as u8).collect();
        stream_decompression_target(&mut decoder, &input);
    }
    let frames = [
        raw_frame(b"valid raw payload", 10),
        SMALL_SEQUENCES.to_vec(),
        HISTORY_OVERWRITE.to_vec(),
        compress(&vec![b'r'; 4096]).unwrap(),
        skip_frame(b"payload to skip", 15),
    ];
    for frame in frames {
        for seed in 0..128 {
            let mut state = seed;
            let parameters: Vec<_> = (0..64).map(|_| next_random(&mut state) as u8).collect();
            stream_decompression_target(&mut decoder, &streaming_fuzz_case(&frame, &parameters));
            for end in [0, frame.len() / 2, frame.len() - 1] {
                stream_decompression_target(
                    &mut decoder,
                    &streaming_fuzz_case(&frame[..end], &parameters),
                );
            }
            let mut corrupted = frame.clone();
            let index = next_random(&mut state) as usize % corrupted.len();
            corrupted[index] ^= 1 << (next_random(&mut state) % 8);
            stream_decompression_target(
                &mut decoder,
                &streaming_fuzz_case(&corrupted, &parameters),
            );
        }
    }
}

/// Complete one directive, retaining exactly the unconsumed input suffix.
fn encoder_action(
    encoder: &mut Encoder,
    input: &[u8],
    directive: EndDirective,
    output_chunk: usize,
) -> Vec<u8> {
    assert!(output_chunk > 0);
    let mut consumed = 0;
    let mut encoded = Vec::new();
    for call in 1..4_000_000 {
        let capacity = if call % 7 == 0 { 0 } else { output_chunk };
        let mut output = vec![0xa5; capacity + 8];
        let progress = encoder
            .process(&input[consumed..], &mut output[..capacity], directive)
            .unwrap();
        assert!(progress.consumed <= input.len() - consumed);
        assert!(progress.produced <= capacity);
        assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
        consumed += progress.consumed;
        encoded.extend_from_slice(&output[..progress.produced]);
        let complete = match directive {
            EndDirective::Continue => progress.status == StreamStatus::NeedInput,
            EndDirective::Flush => progress.status == StreamStatus::Flushed,
            EndDirective::Finish => progress.status == StreamStatus::FrameFinished,
        };
        if complete {
            assert_eq!(consumed, input.len());
            return encoded;
        }
        assert_eq!(progress.status, StreamStatus::NeedOutput);
        assert!(
            progress.consumed > 0 || progress.produced > 0 || capacity == 0,
            "encoder stalled with available output"
        );
    }
    panic!("encoder did not finish the directive within the call budget");
}

fn encode_chunks(
    encoder: &mut Encoder,
    source: &[u8],
    input_chunk: usize,
    output_chunk: usize,
) -> Vec<u8> {
    let mut encoded = Vec::new();
    for fragment in source.chunks(input_chunk) {
        encoded.extend(encoder_action(
            encoder,
            fragment,
            EndDirective::Continue,
            output_chunk,
        ));
    }
    encoded.extend(encoder_action(
        encoder,
        &[],
        EndDirective::Finish,
        output_chunk,
    ));
    assert_eq!(encoder.total_input(), source.len() as u64);
    assert_eq!(encoder.total_output(), encoded.len() as u64);
    encoded
}

/// Consume an available non-final encoded prefix without declaring EOF.
fn decode_available(decoder: &mut Decoder, encoded: &[u8]) -> (Vec<u8>, Vec<StreamStatus>) {
    let mut consumed = 0;
    let mut actual = Vec::new();
    let mut statuses = Vec::new();
    for _ in 0..1_000_000 {
        let mut output = [0; 17];
        let progress = decoder.process(&encoded[consumed..], &mut output).unwrap();
        assert!(progress.consumed <= encoded.len() - consumed);
        assert!(progress.produced <= output.len());
        consumed += progress.consumed;
        actual.extend_from_slice(&output[..progress.produced]);
        statuses.push(progress.status);
        if consumed == encoded.len()
            && matches!(
                progress.status,
                StreamStatus::NeedInput | StreamStatus::FrameFinished
            )
        {
            return (actual, statuses);
        }
        assert!(progress.consumed > 0 || progress.produced > 0);
    }
    panic!("decoder did not consume available flush output");
}

/// `zstreamtest.c:526`: empty source/output followed by a fully drained End.
#[test]
fn empty_encoder_accepts_zero_capacity_continue_and_finish() {
    let mut encoder = Encoder::new();
    let progress = encoder
        .process(&[], &mut [], EndDirective::Continue)
        .unwrap();
    assert_eq!(progress.consumed, 0);
    assert_eq!(progress.produced, 0);
    assert_ne!(progress.status, StreamStatus::FrameFinished);
    let pending = encoder.process(&[], &mut [], EndDirective::Finish).unwrap();
    assert_eq!(pending.consumed, 0);
    assert_eq!(pending.produced, 0);
    assert_eq!(pending.status, StreamStatus::NeedOutput);
    let encoded = encoder_action(&mut encoder, &[], EndDirective::Finish, 1);
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    let complete = decoder.process(&encoded, &mut []).unwrap();
    assert_eq!(complete.status, StreamStatus::FrameFinished);
    assert_eq!(complete.consumed, encoded.len());
    assert_eq!(complete.produced, 0);
    decoder.finish().unwrap();
}

/// `zstreamtest.c:554`: bounded small-source buffering with no output space.
#[test]
fn encoder_accepts_two_source_bytes_before_any_output_is_available() {
    let mut encoder = Encoder::new();
    let progress = encoder
        .process(b"aa", &mut [], EndDirective::Continue)
        .unwrap();
    assert_eq!(progress.consumed, 2);
    assert_eq!(progress.produced, 0);
    let pending = encoder.process(&[], &mut [], EndDirective::Finish).unwrap();
    assert_eq!(pending.status, StreamStatus::NeedOutput);
    let encoded = encoder_action(&mut encoder, &[], EndDirective::Finish, 1);
    assert_eq!(decompress(&encoded, 2).unwrap(), b"aa");
    let mut decoder = Decoder::new(DecodeOptions::new(2));
    assert_ne!(
        decoder.process(&encoded, &mut []).unwrap().status,
        StreamStatus::FrameFinished
    );
}

/// `zstreamtest.c:584`, `:641`: pledge metadata and emission are independent.
#[test]
fn encoder_header_reports_pledge_window_and_checksum_options() {
    let source = b"metadata stays independent of caller chunk boundaries".repeat(13);
    for checksum in [false, true] {
        for include_content_size in [false, true] {
            for pledged_size in [None, Some(source.len() as u64)] {
                let options = EncoderOptions {
                    window_log: 10,
                    checksum,
                    pledged_size,
                    include_content_size,
                    include_dictionary_id: true,
                    ..EncoderOptions::default()
                };
                let mut encoder = Encoder::with_options(options).unwrap();
                let encoded = encode_chunks(&mut encoder, &source, 7, 3);
                let (header, _) = FrameHeader::parse(&encoded).unwrap();
                assert_eq!(header.window_size, 1024);
                assert_eq!(header.has_checksum, checksum);
                assert_eq!(
                    header.content_size,
                    if include_content_size {
                        pledged_size
                    } else {
                        None
                    }
                );
                assert_eq!(decompress(&encoded, source.len()).unwrap(), source);
            }
        }
    }
}

/// `zstreamtest.c:607`, `:625`, `:641`: both pledge mismatch directions fail.
#[test]
fn encoder_enforces_pledges_even_when_content_size_is_not_written() {
    for include_content_size in [false, true] {
        let mut short = Encoder::with_options(EncoderOptions {
            pledged_size: Some(4),
            include_content_size,
            ..EncoderOptions::default()
        })
        .unwrap();
        let mut scratch = [0; 128];
        let accepted = short
            .process(b"abc", &mut scratch, EndDirective::Continue)
            .unwrap();
        assert_eq!(accepted.consumed, 3);
        assert_eq!(
            short.process(&[], &mut scratch, EndDirective::Finish),
            Err(Error::ContentSizeMismatch)
        );
        assert_eq!(
            short.process(&[], &mut scratch, EndDirective::Finish),
            Err(Error::InvalidState)
        );

        let mut excess = Encoder::with_options(EncoderOptions {
            pledged_size: Some(2),
            include_content_size,
            ..EncoderOptions::default()
        })
        .unwrap();
        assert_eq!(
            excess.process(b"abc", &mut scratch, EndDirective::Continue),
            Err(Error::ContentSizeMismatch)
        );
        excess.reset();
        assert_eq!(excess.options().pledged_size, None);
        let expected = b"recovered after pledge error";
        let encoded = encode_chunks(&mut excess, expected, 3, 1);
        assert_eq!(decompress(&encoded, expected.len()).unwrap(), expected);
    }
}

/// `zstreamtest.c:1443`: a zero pledge is not unknown source length.
#[test]
fn zero_pledge_accepts_only_an_empty_frame() {
    let options = EncoderOptions {
        pledged_size: Some(0),
        ..EncoderOptions::default()
    };
    let mut encoder = Encoder::with_options(options).unwrap();
    let encoded = encoder_action(&mut encoder, &[], EndDirective::Finish, 1);
    assert_eq!(
        FrameHeader::parse(&encoded).unwrap().0.content_size,
        Some(0)
    );
    assert!(decompress(&encoded, 0).unwrap().is_empty());
    encoder.reset_with_options(options).unwrap();
    assert_eq!(
        encoder.process(b"x", &mut [0; 128], EndDirective::Continue),
        Err(Error::ContentSizeMismatch)
    );
}

/// `fuzzer.c:1577`: flush an unknown-size empty frame before ending it.
#[test]
fn flush_of_empty_unknown_size_frame_does_not_close_the_frame() {
    let mut encoder = Encoder::new();
    let flushed = encoder_action(&mut encoder, &[], EndDirective::Flush, 1);
    assert_eq!(FrameHeader::parse(&flushed).unwrap().0.content_size, None);
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    let progress = decoder.process(&flushed, &mut []).unwrap();
    assert_eq!(progress.consumed, flushed.len());
    assert_eq!(progress.status, StreamStatus::NeedInput);
    let ending = encoder_action(&mut encoder, &[], EndDirective::Finish, 1);
    let progress = decoder.process(&ending, &mut []).unwrap();
    assert_eq!(progress.consumed, ending.len());
    assert_eq!(progress.status, StreamStatus::FrameFinished);
    decoder.finish().unwrap();
}

/// Directive portion of `zstreamtest.c:1008`, without stable-input mode.
#[test]
fn flush_exposes_all_source_to_a_live_decoder_before_finish() {
    let source: Vec<_> = (0..518).map(|index| (index * 83) as u8).collect();
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new(DecodeOptions::new(500));
    let mut encoded = Vec::new();
    encoded.extend(encoder_action(
        &mut encoder,
        &source[18..100],
        EndDirective::Continue,
        2,
    ));
    encoded.extend(encoder_action(
        &mut encoder,
        &source[100..200],
        EndDirective::Continue,
        2,
    ));
    encoded.extend(encoder_action(&mut encoder, &[], EndDirective::Flush, 2));
    let (mut actual, statuses) = decode_available(&mut decoder, &encoded);
    assert_eq!(actual, source[18..200]);
    assert!(!statuses.contains(&StreamStatus::FrameFinished));
    assert_eq!(statuses.last(), Some(&StreamStatus::NeedInput));

    let repeated_flush = encoder_action(&mut encoder, &[], EndDirective::Flush, 2);
    let (additional, statuses) = decode_available(&mut decoder, &repeated_flush);
    assert!(additional.is_empty());
    assert!(!statuses.contains(&StreamStatus::FrameFinished));
    encoded.extend_from_slice(&repeated_flush);

    let mut tail = encoder_action(&mut encoder, &source[200..], EndDirective::Continue, 2);
    tail.extend(encoder_action(&mut encoder, &[], EndDirective::Finish, 2));
    let (additional, statuses) = decode_available(&mut decoder, &tail);
    actual.extend(additional);
    encoded.extend(tail);
    assert_eq!(statuses.last(), Some(&StreamStatus::FrameFinished));
    assert_eq!(actual, source[18..]);
    assert_eq!(decompress(&encoded, 500).unwrap(), source[18..]);
    decoder.finish().unwrap();
}

/// `stream_round_trip.c:59`: Finish may need to consume a source suffix later.
#[test]
fn finish_resubmits_unconsumed_source_until_every_output_byte_is_drained() {
    let source: Vec<_> = (0..4097)
        .map(|index| ((index * 19) ^ (index >> 3)) as u8)
        .collect();
    let mut encoder = Encoder::with_options(EncoderOptions {
        window_log: 10,
        ..EncoderOptions::default()
    })
    .unwrap();
    let encoded = encoder_action(&mut encoder, &source, EndDirective::Finish, 1);
    assert_eq!(encoder.total_input(), source.len() as u64);
    assert_eq!(encoder.total_output(), encoded.len() as u64);
    assert_eq!(decompress(&encoded, source.len()).unwrap(), source);
    assert_eq!(
        encoder.process(b"next frame", &mut [0; 64], EndDirective::Continue),
        Err(Error::InvalidState)
    );
}

#[test]
fn switching_a_pending_flush_or_finish_directive_requires_reset() {
    for pending in [EndDirective::Flush, EndDirective::Finish] {
        for replacement in [
            EndDirective::Continue,
            EndDirective::Flush,
            EndDirective::Finish,
        ] {
            if pending == replacement {
                continue;
            }
            let mut encoder = Encoder::new();
            let progress = encoder
                .process(b"pending source", &mut [], pending)
                .unwrap();
            assert_eq!(progress.status, StreamStatus::NeedOutput);
            assert_eq!(
                encoder.process(&[], &mut [0; 128], replacement),
                Err(Error::InvalidState)
            );
            encoder.reset();
            let encoded = encode_chunks(&mut encoder, b"reset source", 3, 1);
            assert_eq!(decompress(&encoded, 12).unwrap(), b"reset source");
        }
    }
}

/// Source-size branches of `zstreamtest.c:657`; level 19 remains unimplemented.
#[test]
fn reset_clears_pledge_and_frame_state_but_retains_supported_options() {
    let initial = EncoderOptions {
        window_log: 10,
        checksum: false,
        pledged_size: Some(513),
        include_content_size: false,
        include_dictionary_id: true,
        ..EncoderOptions::default()
    };
    let mut encoder = Encoder::with_options(initial).unwrap();
    let first = vec![b'a'; 513];
    let encoded = encode_chunks(&mut encoder, &first, 7, 1);
    assert_eq!(decompress(&encoded, 513).unwrap(), first);
    let capacity = encoder.retained_capacity();
    encoder.reset();
    assert_eq!(encoder.total_input(), 0);
    assert_eq!(encoder.total_output(), 0);
    assert_eq!(encoder.retained_capacity(), capacity);
    assert_eq!(encoder.options().window_log, initial.window_log);
    assert_eq!(encoder.options().checksum, initial.checksum);
    assert_eq!(
        encoder.options().include_content_size,
        initial.include_content_size
    );
    assert_eq!(encoder.options().pledged_size, None);
    let second = vec![b'b'; 1025];
    let encoded = encode_chunks(&mut encoder, &second, 127, 3);
    assert_eq!(decompress(&encoded, 1025).unwrap(), second);
    let (header, _) = FrameHeader::parse(&encoded).unwrap();
    assert_eq!(header.content_size, None);
    assert!(!header.has_checksum);

    encoder
        .reset_with_options(EncoderOptions {
            pledged_size: Some(1025),
            ..EncoderOptions::default()
        })
        .unwrap();
    let encoded = encode_chunks(&mut encoder, &second, 127, 3);
    let (header, _) = FrameHeader::parse(&encoded).unwrap();
    assert_eq!(header.content_size, Some(1025));
    assert!(header.has_checksum);
}

/// Deterministic context reuse from `fuzzer.c:1637`, supported strategy only.
#[test]
fn reset_repeats_identical_directive_and_chunk_schedules_deterministically() {
    let source: Vec<_> = (0..20).chain(0..10).collect();
    let mut encoder = Encoder::new();
    let first = encode_chunks(&mut encoder, &source, 3, 1);
    for _ in 0..8 {
        encoder.reset();
        assert_eq!(encode_chunks(&mut encoder, &source, 3, 1), first);
    }
}

#[test]
fn encoder_rejects_window_logs_outside_the_supported_range() {
    for window_log in [0, 9, 32, 255] {
        assert!(matches!(
            Encoder::with_options(EncoderOptions {
                window_log,
                ..EncoderOptions::default()
            }),
            Err(Error::InvalidParameter)
        ));
    }
    for window_log in [10, 17, 31] {
        let mut encoder = Encoder::with_options(EncoderOptions {
            window_log,
            ..EncoderOptions::default()
        })
        .unwrap();
        let encoded = encoder_action(&mut encoder, b"", EndDirective::Finish, 1);
        assert_eq!(
            FrameHeader::parse(&encoded).unwrap().0.window_size,
            1_u64 << window_log
        );
    }
}

/// Directive/reset branches of `tests/fuzz/stream_round_trip.c:59`.
#[test]
fn mixed_directive_schedules_produce_exact_concatenated_frames() {
    let source: Vec<_> = (0..8192)
        .map(|index| ((index * 47) ^ (index >> 5)) as u8)
        .collect();
    let mut encoder = Encoder::with_options(EncoderOptions {
        window_log: 10,
        ..EncoderOptions::default()
    })
    .unwrap();
    let mut encoded = Vec::new();
    let mut ends = Vec::new();
    let mut state = 0x1234_5678_u32;
    let mut position = 0;
    let mut directives = [0; 3];
    while position < source.len() {
        state = state
            .wrapping_mul(2_654_435_761)
            .wrapping_add(2_246_822_519)
            .rotate_left(13);
        let end = (position + 1 + (state as usize % 1031)).min(source.len());
        let mode = (state >> 5) % 10;
        let directive = match mode {
            0..=2 => {
                directives[1] += 1;
                EndDirective::Flush
            }
            3 => {
                directives[2] += 1;
                EndDirective::Finish
            }
            _ => {
                directives[0] += 1;
                if mode == 4 {
                    let progress = encoder
                        .process(&[], &mut [], EndDirective::Continue)
                        .unwrap();
                    assert_eq!(progress.consumed, 0);
                    assert_eq!(progress.produced, 0);
                }
                EndDirective::Continue
            }
        };
        encoded.extend(encoder_action(
            &mut encoder,
            &source[position..end],
            directive,
            1 + ((state >> 16) as usize % 31),
        ));
        position = end;
        if directive == EndDirective::Finish {
            ends.push(encoded.len());
            encoder.reset();
        }
    }
    encoded.extend(encoder_action(&mut encoder, &[], EndDirective::Finish, 3));
    ends.push(encoded.len());
    assert!(directives.iter().all(|&count| count > 0));
    let mut decoder = Decoder::new(DecodeOptions::new(source.len()));
    let (actual, boundaries) = decode_chunks(&mut decoder, &encoded, 7, 5);
    assert_eq!(actual, source);
    assert_eq!(boundaries, ends);
}

/// Dictionary-free/parameter-fixed portions of `zstreamtest.c:2579`, `:2847`.
#[test]
fn many_seeded_directive_schedules_preserve_data_and_compression_determinism() {
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    let mut exercised = [0; 3];
    for seed in 0..128_u32 {
        let mut state = seed;
        let size = match seed {
            0..=5 => [0, 1, 1023, 1024, 1025, 131073][seed as usize],
            _ => next_random(&mut state) as usize % 16385,
        };
        let source: Vec<_> = (0..size)
            .map(|position| {
                let random = next_random(&mut state) as u8;
                match seed % 5 {
                    0 => random,
                    1 => b'a',
                    2 => (position % 31) as u8,
                    3 => random & 3,
                    _ => {
                        if position % 997 == 0 {
                            random
                        } else {
                            0
                        }
                    }
                }
            })
            .collect();
        let options = EncoderOptions {
            window_log: 10 + (seed % 8) as u8,
            checksum: seed & 1 != 0,
            pledged_size: None,
            include_content_size: seed & 2 != 0,
            include_dictionary_id: true,
            ..EncoderOptions::default()
        };
        let schedule_seed = state;
        let mut reference = None;
        for repeat in 0..2 {
            encoder.reset_with_options(options).unwrap();
            state = schedule_seed;
            let mut encoded = Vec::new();
            let mut boundaries = Vec::new();
            let mut position = 0;
            while position < source.len() {
                let chunk_size = 1 + next_random(&mut state) as usize % 4096;
                let end = (position + chunk_size).min(source.len());
                let directive = match next_random(&mut state) % 10 {
                    0..=2 => {
                        exercised[1] += 1;
                        EndDirective::Flush
                    }
                    3 => {
                        exercised[2] += 1;
                        EndDirective::Finish
                    }
                    _ => {
                        exercised[0] += 1;
                        EndDirective::Continue
                    }
                };
                let output_chunk = 1 + next_random(&mut state) as usize % 67;
                encoded.extend(encoder_action(
                    &mut encoder,
                    &source[position..end],
                    directive,
                    output_chunk,
                ));
                position = end;
                if directive == EndDirective::Finish {
                    boundaries.push(encoded.len());
                    encoder.reset();
                }
            }
            encoded.extend(encoder_action(&mut encoder, &[], EndDirective::Finish, 3));
            boundaries.push(encoded.len());
            if let Some(previous) = &reference {
                assert_eq!(&encoded, previous, "seed {seed}: reset changed compression");
            } else {
                reference = Some(encoded.clone());
            }
            decoder.reset_with_options(DecodeOptions::new(source.len()));
            let (actual, actual_boundaries) =
                decode_scheduled(&mut decoder, &encoded, 131, 97, Some(seed ^ 0x9e37_79b9));
            assert_eq!(actual, source, "seed {seed}, repeat {repeat}");
            assert_eq!(
                actual_boundaries, boundaries,
                "seed {seed}, repeat {repeat}"
            );
            assert!(encoder.retained_capacity() < 2 * 1024 * 1024);

            if repeat == 0 {
                let parameters: Vec<_> = (0..48).map(|_| next_random(&mut state) as u8).collect();
                let mutation_size = 1 + next_random(&mut state) as usize % 7;
                let mut corrupted = encoded.clone();
                for _ in 0..mutation_size {
                    let index = next_random(&mut state) as usize % corrupted.len();
                    corrupted[index] ^= 1 << (next_random(&mut state) % 8);
                }
                if corrupted.len() + parameters.len() + 4 <= 65536 {
                    stream_decompression_target(
                        &mut decoder,
                        &streaming_fuzz_case(&corrupted, &parameters),
                    );
                    stream_decompression_target(
                        &mut decoder,
                        &streaming_fuzz_case(&encoded[..encoded.len() - 1], &parameters),
                    );
                }
            }
        }
    }
    assert!(exercised.iter().all(|&count| count > 100));
}

#[test]
fn encoder_retained_memory_is_independent_of_prior_source_length() {
    let source: Vec<_> = (0..1024)
        .map(|index| (index * 43) as u8)
        .collect::<Vec<_>>()
        .repeat(4096);
    let mut encoder = Encoder::with_options(EncoderOptions {
        window_log: 10,
        ..EncoderOptions::default()
    })
    .unwrap();
    let encoded = encode_chunks(&mut encoder, &source, 4096, 4096);
    assert_eq!(decompress(&encoded, source.len()).unwrap(), source);
    assert!(
        encoder.retained_capacity() < 2 * 1024 * 1024,
        "encoder retained a 4 MiB stream instead of bounded block workspaces"
    );
    let capacity = encoder.retained_capacity();
    encoder.reset();
    assert_eq!(encoder.retained_capacity(), capacity);
    let repeated = encode_chunks(&mut encoder, &source, 4096, 4096);
    assert_eq!(repeated, encoded);
    assert_eq!(encoder.retained_capacity(), capacity);
}

#[test]
#[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN or put zstd on PATH"]
fn official_codec_accepts_streamed_frames_and_stream_decoder_accepts_official_frames() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn run(executable: &std::ffi::OsStr, arguments: &[&str], input: &[u8]) -> Vec<u8> {
        let mut child = Command::new(executable)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("build the official v1.5.7 CLI and set ZSTD_BIN");
        let mut stdin = child.stdin.take().unwrap();
        let source = input.to_vec();
        let writer = std::thread::spawn(move || stdin.write_all(&source));
        let result = child.wait_with_output().unwrap();
        writer.join().unwrap().unwrap();
        assert!(
            result.status.success(),
            "official oracle failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        result.stdout
    }

    let executable = std::env::var_os("ZSTD_BIN").unwrap_or_else(|| "zstd".into());
    let version = Command::new(&executable).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("v1.5.7"));
    let inputs = [
        Vec::new(),
        (0..20).chain(0..10).collect(),
        vec![b'r'; 131_073],
        (0..262_147)
            .map(|index| ((index * 79) ^ (index >> 7)) as u8)
            .collect(),
        include_bytes!("upstream/data/golden-compression/http").to_vec(),
    ];
    for source in inputs {
        for window_log in [10, 17] {
            for checksum in [false, true] {
                let mut encoder = Encoder::with_options(EncoderOptions {
                    window_log,
                    checksum,
                    ..EncoderOptions::default()
                })
                .unwrap();
                let mut encoded = Vec::new();
                for fragment in source.chunks(257) {
                    encoded.extend(encoder_action(
                        &mut encoder,
                        fragment,
                        EndDirective::Flush,
                        31,
                    ));
                }
                encoded.extend(encoder_action(&mut encoder, &[], EndDirective::Finish, 31));
                assert_eq!(run(&executable, &["-q", "-d", "-c"], &encoded), source);
            }
        }
        for checksum_flag in ["--check", "--no-check"] {
            let official = run(&executable, &["-q", "-c", "-3", checksum_flag], &source);
            let mut decoder = Decoder::new(DecodeOptions::new(source.len()));
            let (actual, boundaries) = decode_chunks(&mut decoder, &official, 13, 29);
            assert_eq!(actual, source);
            assert_eq!(boundaries, [official.len()]);
        }
    }
}

/// Supported-parameter adapter of `tests/fuzz/stream_round_trip.c:59`.
fn stream_round_trip_target(encoder: &mut Encoder, decoder: &mut Decoder, input: &[u8]) {
    assert!(input.len() <= 2 * 1024 * 1024);
    let mut producer = fuzz_support::DataProducer::new(input);
    let source_size = producer.reserve_prefix();
    let source = &input[..source_size];
    // ZSTD_COMPRESSBOUND from v1.5.7, used only to reproduce the C test's
    // selected output capacities. It is not a production compression API.
    let bound = source_size
        + (source_size >> 8)
        + if source_size < 128 * 1024 {
            (128 * 1024 - source_size) >> 11
        } else {
            0
        };
    let capacity = bound * 15;
    let mut scratch = vec![0xa5; capacity + 8];
    let mut encoded = Vec::new();
    let mut position = 0;
    let mut calls = 0;
    let call_limit = 32 * (input.len() + 1) + 1024;
    encoder
        .reset_with_options(EncoderOptions::default())
        .unwrap();
    // FUZZ_setRandomParameters and external sequence producers are not
    // invoked: the Rust API exposes only its documented initial strategy.
    while position < source_size {
        let length = producer.unsigned_range(1, (source_size - position) as u32) as usize;
        let end = position + length;
        let mut mode = None;
        while position < end || mode.is_some() {
            calls += 1;
            assert!(
                calls <= call_limit,
                "round-trip fuzz target exceeded its progress budget"
            );
            assert!(
                encoded.len() < capacity,
                "original fuzz compression bound exhausted"
            );
            let output_size =
                producer.unsigned_range(1, (capacity - encoded.len()) as u32) as usize;
            let selected = *mode.get_or_insert_with(|| producer.unsigned_range(0, 9));
            let directive = match selected {
                0..=2 => EndDirective::Flush,
                3 => EndDirective::Finish,
                _ => EndDirective::Continue,
            };
            if selected == 4 {
                let progress = encoder
                    .process(&[], &mut [], EndDirective::Continue)
                    .unwrap();
                assert_eq!(progress.consumed, 0);
                assert_eq!(progress.produced, 0);
            }
            scratch[..output_size + 8].fill(0xa5);
            let progress = encoder
                .process(
                    &source[position..end],
                    &mut scratch[..output_size],
                    directive,
                )
                .unwrap();
            assert!(progress.consumed <= end - position);
            assert!(progress.produced <= output_size);
            assert!(
                scratch[progress.produced..output_size + 8]
                    .iter()
                    .all(|&byte| byte == 0xa5)
            );
            position += progress.consumed;
            encoded.extend_from_slice(&scratch[..progress.produced]);
            assert!(
                progress.consumed > 0
                    || progress.produced > 0
                    || matches!(
                        progress.status,
                        StreamStatus::Flushed | StreamStatus::FrameFinished
                    )
            );
            match directive {
                EndDirective::Continue => mode = None,
                EndDirective::Flush => {
                    if progress.status == StreamStatus::Flushed {
                        mode = None;
                    }
                }
                EndDirective::Finish => {
                    if progress.status == StreamStatus::FrameFinished {
                        encoder.reset();
                        // Preserve the branch selector; its unsupported
                        // random-parameter function is deliberately omitted.
                        let _change_parameters = producer.unsigned_range(0, 7) == 0;
                        mode = None;
                    }
                }
            }
        }
    }
    loop {
        calls += 1;
        assert!(calls <= call_limit);
        assert!(encoded.len() < capacity);
        let output_size = producer.unsigned_range(1, (capacity - encoded.len()) as u32) as usize;
        scratch[..output_size + 8].fill(0xa5);
        let progress = encoder
            .process(&[], &mut scratch[..output_size], EndDirective::Finish)
            .unwrap();
        assert_eq!(progress.consumed, 0);
        assert!(progress.produced <= output_size);
        assert!(
            scratch[progress.produced..output_size + 8]
                .iter()
                .all(|&byte| byte == 0xa5)
        );
        encoded.extend_from_slice(&scratch[..progress.produced]);
        if progress.status == StreamStatus::FrameFinished {
            break;
        }
        assert_eq!(progress.status, StreamStatus::NeedOutput);
        assert!(progress.produced > 0);
    }
    // The original helper consumes this selector for optional maxBlockSize.
    let _set_max_block_size = producer.unsigned_range(0, 1);
    decoder.reset_with_options(DecodeOptions::new(source.len()));
    let mut recovered = vec![0; source.len()];
    let mut consumed = 0;
    let mut produced = 0;
    while consumed < encoded.len() {
        let progress = decoder
            .process(&encoded[consumed..], &mut recovered[produced..])
            .unwrap();
        assert!(progress.consumed <= encoded.len() - consumed);
        assert!(progress.produced <= recovered.len() - produced);
        assert_eq!(progress.status, StreamStatus::FrameFinished);
        assert!(progress.consumed > 0);
        consumed += progress.consumed;
        produced += progress.produced;
    }
    decoder.finish().unwrap();
    assert_eq!(produced, source.len());
    assert_eq!(recovered, source);
}

#[test]
#[ignore = "requires ZSTDOXIDE_STREAM_CORPUS and ZSTDOXIDE_STREAM_TARGET; see streaming conformance plan"]
fn external_official_streaming_corpus() {
    use std::path::PathBuf;
    let paths = std::env::var_os("ZSTDOXIDE_STREAM_CORPUS")
        .expect("ZSTDOXIDE_STREAM_CORPUS must name files/directories");
    let target = std::env::var("ZSTDOXIDE_STREAM_TARGET")
        .expect("ZSTDOXIDE_STREAM_TARGET must be stream_decompress or stream_round_trip");
    assert!(matches!(
        target.as_str(),
        "stream_decompress" | "stream_round_trip"
    ));
    let mut pending: Vec<(PathBuf, usize)> = std::env::split_paths(&paths)
        .map(|path| (path, 0))
        .collect();
    let mut files = Vec::new();
    while let Some((path, depth)) = pending.pop() {
        assert!(depth <= 64, "corpus directory depth limit exceeded");
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        assert!(
            !metadata.file_type().is_symlink(),
            "corpus symlinks are not followed: {}",
            path.display()
        );
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                pending.push((entry.unwrap().path(), depth + 1));
            }
        } else {
            assert!(
                metadata.is_file(),
                "unsupported corpus file type: {}",
                path.display()
            );
            assert!(
                metadata.len() <= 2 * 1024 * 1024,
                "corpus input exceeds 2 MiB: {}",
                path.display()
            );
            files.push(path);
            assert!(files.len() <= 65536, "corpus file count exceeds 65,536");
        }
    }
    files.sort();
    assert!(!files.is_empty(), "empty corpus selection");
    let mut decoder = Decoder::new(DecodeOptions::new(0));
    let mut encoder = Encoder::new();
    let mut total_bytes = 0;
    for (index, path) in files.iter().enumerate() {
        let input = std::fs::read(path).unwrap();
        assert!(input.len() <= 2 * 1024 * 1024);
        total_bytes += input.len();
        if target == "stream_decompress" {
            stream_decompression_target_with_limits(
                &mut decoder,
                &input,
                2 * 1024 * 1024,
                10 * 1024 * 1024,
                128 * 1024 * 1024,
            );
        } else {
            stream_round_trip_target(&mut encoder, &mut decoder, &input);
        }
        if (index + 1) % 1000 == 0 {
            eprintln!("{target}: {} / {} files", index + 1, files.len());
        }
    }
    eprintln!(
        "{target}: replayed {} files, {total_bytes} bytes; no skipped inputs",
        files.len()
    );
}
