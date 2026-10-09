// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Contracts derive from facebook/zstd v1.5.7 tests/fuzzer.c and zstreamtest.c.
// Used under the BSD-3-Clause option; see LICENSE.zstd.
//! Dictionary encoder contracts, effectiveness, and independent compatibility.
//! The encoder exposes its own strategy, not upstream compression levels.
#![forbid(unsafe_code)]

use zstdoxide::{
    DecodeOptions, Dictionary, Encoder, EncoderOptions, EndDirective, Error, FrameHeader,
    SharedDictionary, StreamStatus, decompress, decompress_with_dictionary,
};

const RAW: &[u8] = include_bytes!("upstream/dictionaries/boundary-dictionary.raw");
const FORMATTED: &[u8] = include_bytes!("upstream/dictionaries/formatted.dict");
const TRAINED: &[u8] = include_bytes!("upstream/dictionaries/trained.dict");
const DUPLICATE_REPS: &[u8] = include_bytes!("upstream/dictionaries/duplicate-reps.dict");

fn raw_dictionary() -> SharedDictionary {
    Dictionary::raw(RAW).unwrap().into_shared()
}

fn prepared(bytes: &[u8]) -> SharedDictionary {
    Dictionary::from_bytes(bytes).unwrap().into_shared()
}

fn options(size: usize) -> EncoderOptions {
    EncoderOptions {
        window_log: 10,
        pledged_size: Some(size as u64),
        ..EncoderOptions::default()
    }
}

/// Drain one directive with caller-owned slices and occasional zero output.
fn action(encoder: &mut Encoder, input: &[u8], directive: EndDirective, chunk: usize) -> Vec<u8> {
    assert!(chunk > 0);
    let before_input = encoder.total_input();
    let before_output = encoder.total_output();
    let mut consumed = 0;
    let mut encoded = Vec::new();
    for call in 1..1_000_000 {
        let capacity = if call % 7 == 0 { 0 } else { chunk };
        let mut output = vec![0xa5; capacity + 8];
        let progress = encoder
            .process(&input[consumed..], &mut output[..capacity], directive)
            .unwrap();
        assert!(progress.consumed <= input.len() - consumed);
        assert!(progress.produced <= capacity);
        assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
        consumed += progress.consumed;
        encoded.extend_from_slice(&output[..progress.produced]);
        assert_eq!(encoder.total_input(), before_input + consumed as u64);
        assert_eq!(encoder.total_output(), before_output + encoded.len() as u64);
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
        assert!(progress.consumed != 0 || progress.produced != 0 || capacity == 0);
    }
    panic!("dictionary encoder exceeded bounded call count");
}

fn encode_chunks(
    encoder: &mut Encoder,
    source: &[u8],
    input_chunk: usize,
    output_chunk: usize,
) -> Vec<u8> {
    assert!(input_chunk > 0);
    let mut encoded = Vec::new();
    for fragment in source.chunks(input_chunk) {
        encoded.extend(action(
            encoder,
            fragment,
            EndDirective::Continue,
            output_chunk,
        ));
    }
    encoded.extend(action(encoder, &[], EndDirective::Finish, output_chunk));
    assert_eq!(encoder.total_input(), source.len() as u64);
    assert_eq!(encoder.total_output(), encoded.len() as u64);
    encoded
}

fn encode(source: &[u8], dictionary: SharedDictionary, settings: EncoderOptions) -> Vec<u8> {
    let mut encoder = Encoder::with_dictionary(settings, dictionary).unwrap();
    encode_chunks(&mut encoder, source, 7, 13)
}

fn assert_round_trip(frame: &[u8], source: &[u8], dictionary: &SharedDictionary) {
    assert_eq!(
        decompress_with_dictionary(frame, dictionary, DecodeOptions::new(source.len())).unwrap(),
        source
    );
}

#[test]
fn raw_and_formatted_dictionary_matches_reduce_size_for_independent_source_bytes() {
    let source = &RAW[..512];
    let mut plain = Encoder::with_options(options(source.len())).unwrap();
    let plain = encode_chunks(&mut plain, source, 7, 13);
    assert!(plain.len() > source.len() / 2);
    for dictionary in [raw_dictionary(), prepared(FORMATTED)] {
        let frame = encode(source, dictionary.clone(), options(source.len()));
        assert!(
            frame.len() * 4 < plain.len(),
            "dictionary bytes must produce real useful matches: {} versus {}",
            frame.len(),
            plain.len()
        );
        assert_round_trip(&frame, source, &dictionary);
        assert!(decompress(&frame, source.len()).is_err());
    }
}

#[test]
fn dictionary_encoding_is_identical_across_input_and_output_schedules() {
    let source = RAW.repeat(3);
    let dictionary = raw_dictionary();
    let mut baseline = None;
    for (input_chunk, output_chunk) in [(1, 1), (7, 13), (source.len(), 4096)] {
        let mut encoder =
            Encoder::with_dictionary(options(source.len()), dictionary.clone()).unwrap();
        let frame = encode_chunks(&mut encoder, &source, input_chunk, output_chunk);
        assert_round_trip(&frame, &source, &dictionary);
        if let Some(previous) = &baseline {
            assert_eq!(&frame, previous);
        } else {
            baseline = Some(frame);
        }
    }
}

#[test]
fn formatted_dictionary_ids_use_all_wire_widths_and_can_be_omitted() {
    let source = &RAW[..512];
    for (id, flag) in [
        (0_u32, 0_u8),
        (1, 1),
        (255, 1),
        (256, 2),
        (65_535, 2),
        (65_536, 3),
        (u32::MAX, 3),
    ] {
        let mut bytes = FORMATTED.to_vec();
        bytes[4..8].copy_from_slice(&id.to_le_bytes());
        let dictionary = prepared(&bytes);
        for include in [false, true] {
            let frame = encode(
                source,
                dictionary.clone(),
                EncoderOptions {
                    include_dictionary_id: include,
                    ..options(source.len())
                },
            );
            let header = FrameHeader::parse(&frame).unwrap().0;
            assert_eq!(header.dictionary_id, if include { id } else { 0 });
            assert_eq!(frame[4] & 3, if include { flag } else { 0 });
            assert_eq!(header.content_size, Some(source.len() as u64));
            assert!(header.has_checksum);
            assert_round_trip(&frame, source, &dictionary);
        }
    }
}

#[test]
fn omitted_dictionary_id_preserves_match_effectiveness_and_checksum() {
    let source = &RAW[..512];
    let dictionary = prepared(FORMATTED);
    let included = encode(source, dictionary.clone(), options(source.len()));
    let omitted = encode(
        source,
        dictionary.clone(),
        EncoderOptions {
            include_dictionary_id: false,
            ..options(source.len())
        },
    );
    assert_eq!(included.len(), omitted.len() + 4);
    assert_eq!(FrameHeader::parse(&omitted).unwrap().0.dictionary_id, 0);
    assert!(decompress(&omitted, source.len()).is_err());
    assert_round_trip(&omitted, source, &dictionary);
    let mut damaged = omitted;
    *damaged.last_mut().unwrap() ^= 1;
    assert_eq!(
        decompress_with_dictionary(&damaged, &dictionary, DecodeOptions::new(source.len())),
        Err(Error::ChecksumMismatch)
    );
}

#[test]
fn dictionary_frames_cover_known_unknown_size_checksum_and_window_options() {
    let source = &RAW[..512];
    let dictionary = prepared(FORMATTED);
    for window_log in [10, 17, 31] {
        for checksum in [false, true] {
            for include_content_size in [false, true] {
                let settings = EncoderOptions {
                    window_log,
                    checksum,
                    include_content_size,
                    ..options(source.len())
                };
                let frame = encode(source, dictionary.clone(), settings);
                let header = FrameHeader::parse(&frame).unwrap().0;
                assert_eq!(header.window_size, 1_u64 << window_log);
                assert_eq!(
                    header.content_size,
                    include_content_size.then_some(source.len() as u64)
                );
                assert_eq!(header.has_checksum, checksum);
                assert_eq!(
                    decompress_with_dictionary(
                        &frame,
                        &dictionary,
                        DecodeOptions {
                            max_output_size: source.len(),
                            max_window_size: 1_u64 << window_log
                        }
                    )
                    .unwrap(),
                    source
                );
            }
        }
    }
}

/// `zstreamtest.c:1222,1251`: reset preserves dictionary configuration, while
/// clearing source pledges and counters according to the Rust context contract.
#[test]
fn encoder_reset_and_reset_with_options_preserve_persistent_dictionary() {
    let source = &RAW[..512];
    let dictionary = prepared(FORMATTED);
    let mut encoder = Encoder::with_dictionary(options(source.len()), dictionary.clone()).unwrap();
    let first = encode_chunks(&mut encoder, source, 1, 1);
    encoder.reset();
    assert_eq!(encoder.options().pledged_size, None);
    assert_eq!(encoder.total_input(), 0);
    assert_eq!(encoder.total_output(), 0);
    let second = encode_chunks(&mut encoder, source, 7, 13);
    assert_eq!(
        FrameHeader::parse(&second).unwrap().0.dictionary_id,
        dictionary.id()
    );
    encoder.reset_with_options(options(source.len())).unwrap();
    assert_eq!(encode_chunks(&mut encoder, source, 7, 13), first);
    assert_round_trip(&second, source, &dictionary);
}

/// `zstreamtest.c:1299,1390,1403`: after one frame, reset cannot restore a prefix.
#[test]
fn encoder_prefix_is_single_use_and_reset_preserves_only_unclaimed_prefix() {
    let source = &RAW[..512];
    let dictionary = prepared(FORMATTED);
    let mut encoder = Encoder::with_options(options(source.len())).unwrap();
    encoder.set_prefix(dictionary.clone()).unwrap();
    encoder.reset_with_options(options(source.len())).unwrap();
    let first = encode_chunks(&mut encoder, source, 1, 1);
    assert_eq!(
        FrameHeader::parse(&first).unwrap().0.dictionary_id,
        dictionary.id()
    );
    assert_round_trip(&first, source, &dictionary);
    encoder.reset_with_options(options(source.len())).unwrap();
    let second = encode_chunks(&mut encoder, source, 7, 13);
    assert_eq!(FrameHeader::parse(&second).unwrap().0.dictionary_id, 0);
    assert_eq!(decompress(&second, source.len()).unwrap(), source);
    assert!(first.len() * 4 < second.len());
}

#[test]
fn header_start_claims_prefix_even_before_any_input_or_output_is_accepted() {
    let dictionary = prepared(FORMATTED);
    let mut encoder = Encoder::with_options(options(0)).unwrap();
    encoder.set_prefix(dictionary).unwrap();
    let progress = encoder
        .process(&[], &mut [], EndDirective::Continue)
        .unwrap();
    assert_eq!(progress.consumed, 0);
    assert_eq!(progress.produced, 0);
    encoder.reset_with_options(options(0)).unwrap();
    let frame = encode_chunks(&mut encoder, &[], 1, 1);
    assert_eq!(FrameHeader::parse(&frame).unwrap().0.dictionary_id, 0);
    assert_eq!(decompress(&frame, 0).unwrap(), b"");
}

#[test]
fn empty_standard_frame_consumes_the_encoder_prefix() {
    let mut encoder = Encoder::with_options(options(0)).unwrap();
    encoder.set_prefix(prepared(FORMATTED)).unwrap();
    let frame = encode_chunks(&mut encoder, &[], 1, 1);
    assert_eq!(
        FrameHeader::parse(&frame).unwrap().0.dictionary_id,
        prepared(FORMATTED).id()
    );
    assert_round_trip(&frame, &[], &prepared(FORMATTED));
    encoder.reset_with_options(options(0)).unwrap();
    let empty = encode_chunks(&mut encoder, &[], 1, 1);
    assert_eq!(decompress(&empty, 0).unwrap(), b"");
}

#[test]
fn prefix_and_dictionary_configuration_replace_one_another() {
    let source = &RAW[..512];
    let mut encoder = Encoder::with_dictionary(options(source.len()), prepared(FORMATTED)).unwrap();
    encoder.set_prefix(raw_dictionary()).unwrap();
    let raw = encode_chunks(&mut encoder, source, 7, 13);
    assert_eq!(FrameHeader::parse(&raw).unwrap().0.dictionary_id, 0);
    assert_round_trip(&raw, source, &raw_dictionary());
    encoder.reset_with_options(options(source.len())).unwrap();
    encoder.set_prefix(raw_dictionary()).unwrap();
    encoder.set_dictionary(Some(prepared(FORMATTED))).unwrap();
    let formatted = encode_chunks(&mut encoder, source, 7, 13);
    assert_eq!(
        FrameHeader::parse(&formatted).unwrap().0.dictionary_id,
        prepared(FORMATTED).id()
    );
    encoder.reset_with_options(options(source.len())).unwrap();
    encoder.set_prefix(raw_dictionary()).unwrap();
    encoder.set_dictionary(None).unwrap();
    let plain = encode_chunks(&mut encoder, source, 7, 13);
    assert_eq!(decompress(&plain, source.len()).unwrap(), source);
}

#[test]
fn rejected_mid_frame_configuration_preserves_dictionary_and_pending_output() {
    let source = &RAW[..512];
    let dictionary = prepared(FORMATTED);
    let mut encoder = Encoder::with_dictionary(options(source.len()), dictionary.clone()).unwrap();
    let first = encoder
        .process(source, &mut [], EndDirective::Continue)
        .unwrap();
    assert_eq!(first.consumed, source.len());
    assert_eq!(first.produced, 0);
    assert_eq!(encoder.set_dictionary(None), Err(Error::InvalidState));
    assert_eq!(
        encoder.set_prefix(raw_dictionary()),
        Err(Error::InvalidState)
    );
    assert_eq!(encoder.total_input(), source.len() as u64);
    let frame = action(&mut encoder, &[], EndDirective::Finish, 1);
    assert_round_trip(&frame, source, &dictionary);
    assert_eq!(encoder.set_dictionary(None), Err(Error::InvalidState));
    assert_eq!(
        encoder.set_prefix(raw_dictionary()),
        Err(Error::InvalidState)
    );
    assert_eq!(
        encoder
            .process(&[], &mut [], EndDirective::Finish)
            .unwrap()
            .status,
        StreamStatus::FrameFinished
    );
}

#[test]
fn processing_failure_requires_reset_and_keeps_persistent_dictionary() {
    let source = &RAW[..512];
    let dictionary = prepared(FORMATTED);
    let mut encoder =
        Encoder::with_dictionary(options(source.len() + 1), dictionary.clone()).unwrap();
    assert_eq!(
        encoder.process(source, &mut [0; 32], EndDirective::Finish),
        Err(Error::ContentSizeMismatch)
    );
    assert_eq!(encoder.set_dictionary(None), Err(Error::InvalidState));
    assert_eq!(
        encoder.set_prefix(raw_dictionary()),
        Err(Error::InvalidState)
    );
    encoder.reset_with_options(options(source.len())).unwrap();
    let frame = encode_chunks(&mut encoder, source, 1, 1);
    assert_round_trip(&frame, source, &dictionary);
}

#[test]
fn invalid_options_leave_an_existing_dictionary_encoder_unchanged() {
    let source = &RAW[..512];
    let dictionary = prepared(FORMATTED);
    let settings = options(source.len());
    let mut encoder = Encoder::with_dictionary(settings, dictionary.clone()).unwrap();
    for window_log in [0, 9, 32, 255] {
        let invalid = EncoderOptions {
            window_log,
            ..settings
        };
        assert!(matches!(
            Encoder::with_dictionary(invalid, dictionary.clone()),
            Err(Error::InvalidParameter)
        ));
        assert_eq!(
            encoder.reset_with_options(invalid),
            Err(Error::InvalidParameter)
        );
        assert_eq!(encoder.options(), settings);
    }
    let frame = encode_chunks(&mut encoder, source, 7, 13);
    assert_round_trip(&frame, source, &dictionary);
}

#[test]
fn raw_dictionary_short_lengths_and_formatted_looking_content_are_valid() {
    for bytes in [
        b"".as_slice(),
        b"x",
        b"xy",
        b"xyz",
        b"xyzw",
        b"abcdefg",
        b"abcdefgh",
        FORMATTED,
    ] {
        let dictionary = Dictionary::raw(bytes).unwrap().into_shared();
        for source in [b"".as_slice(), b"xyzwxyzwabcdefghabcdefgh", &RAW[..128]] {
            let frame = encode(source, dictionary.clone(), options(source.len()));
            assert_eq!(FrameHeader::parse(&frame).unwrap().0.dictionary_id, 0);
            assert_round_trip(&frame, source, &dictionary);
        }
    }
}

#[test]
fn prepared_dictionaries_with_nondefault_duplicate_repcodes_roundtrip() {
    for dictionary in [prepared(TRAINED), prepared(DUPLICATE_REPS)] {
        let content = dictionary.content();
        let source = [
            &content[..256],
            &content[content.len() - 256..],
            &content[..256],
        ]
        .concat();
        let frame = encode(&source, dictionary.clone(), options(source.len()));
        assert_round_trip(&frame, &source, &dictionary);
    }
}

#[test]
fn dictionary_is_eligible_at_the_window_boundary_but_expires_after_it() {
    // At P==W only the first block position can use dictionary content. Use a
    // collision-controlled seed so its first key remains a candidate: the
    // random RAW fixture replaces position0 with a collision at position35.
    // The independent D>W effectiveness test uses the full random dictionary.
    let content: Vec<u8> = (0..=255).collect();
    let dictionary = Dictionary::raw(&content).unwrap().into_shared();
    for prefix_length in [1024, 1025] {
        let mut source = vec![b'r'; prefix_length];
        source.extend_from_slice(&content);
        let frame = encode(&source, dictionary.clone(), options(source.len()));
        assert_round_trip(&frame, &source, &dictionary);
        if prefix_length == 1024 {
            assert!(
                decompress(&frame, source.len()).is_err(),
                "P==W should use a useful dictionary match"
            );
        } else {
            assert_eq!(
                decompress(&frame, source.len()).unwrap(),
                source,
                "P>W must not require dictionary content"
            );
        }
    }
}

#[test]
fn flushes_and_automatic_blocks_preserve_dictionary_frame_positions() {
    let dictionary = raw_dictionary();
    let source = RAW.repeat(3);
    for fragment_size in [31, 257, 1024, 131_072] {
        let mut encoder =
            Encoder::with_dictionary(options(source.len()), dictionary.clone()).unwrap();
        let mut frame = Vec::new();
        for fragment in source.chunks(fragment_size) {
            frame.extend(action(&mut encoder, fragment, EndDirective::Flush, 7));
        }
        frame.extend(action(&mut encoder, &[], EndDirective::Finish, 7));
        assert_round_trip(&frame, &source, &dictionary);
    }
}

#[test]
fn large_stream_roundtrip_stays_bounded_after_dictionary_lifetime_expires() {
    let dictionary = raw_dictionary();
    let source = RAW.repeat(129);
    let mut encoder = Encoder::with_dictionary(
        EncoderOptions {
            window_log: 17,
            ..options(source.len())
        },
        dictionary.clone(),
    )
    .unwrap();
    let frame = encode_chunks(&mut encoder, &source, 1021, 251);
    assert_round_trip(&frame, &source, &dictionary);
    let capacity = encoder.retained_capacity();
    assert!(capacity < 4 * 1024 * 1024);
    encoder.reset();
    for _ in 0..2 {
        let frame = encode_chunks(&mut encoder, &source, 1021, 251);
        assert_round_trip(&frame, &source, &dictionary);
        assert_eq!(encoder.retained_capacity(), capacity);
        encoder.reset();
    }
}

/// Independent decode of Rust-generated dictionary frames. No official codec
/// participates in the production implementation or ordinary Rust tests.
#[test]
#[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN or put zstd on PATH"]
fn official_codec_decodes_raw_prepared_and_omitted_id_dictionary_frames() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let executable = std::env::var_os("ZSTD_BIN").unwrap_or_else(|| "zstd".into());
    let version = Command::new(&executable).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("v1.5.7"));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/upstream/dictionaries");
    let mut cases = 0;
    for (name, dictionary) in [
        ("boundary-dictionary.raw", raw_dictionary()),
        ("formatted.dict", prepared(FORMATTED)),
        ("trained.dict", prepared(TRAINED)),
        ("duplicate-reps.dict", prepared(DUPLICATE_REPS)),
    ] {
        let path = root.join(name);
        let content = dictionary.content();
        let sources = [Vec::new(), content[..512].to_vec(), content.repeat(3)];
        for source in sources {
            for window_log in [10, 17] {
                for checksum in [false, true] {
                    for include_dictionary_id in [false, true] {
                        let settings = EncoderOptions {
                            window_log,
                            checksum,
                            include_dictionary_id,
                            ..options(source.len())
                        };
                        let frame = encode(&source, dictionary.clone(), settings);
                        assert_round_trip(&frame, &source, &dictionary);
                        let mut child = Command::new(&executable)
                            .args(["-q", "-d", "-c", "-D"])
                            .arg(&path)
                            .stdin(Stdio::piped())
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .spawn()
                            .expect("build the official v1.5.7 CLI and set ZSTD_BIN");
                        let mut stdin = child.stdin.take().unwrap();
                        let writer = std::thread::spawn(move || stdin.write_all(&frame));
                        let result = child.wait_with_output().unwrap();
                        let written = writer.join().unwrap();
                        assert!(
                            result.status.success(),
                            "dictionary={name}, length={}, window={window_log}, checksum={checksum}, ID={include_dictionary_id}: {}",
                            source.len(),
                            String::from_utf8_lossy(&result.stderr)
                        );
                        written.unwrap();
                        assert_eq!(
                            result.stdout, source,
                            "dictionary={name}, window={window_log}, checksum={checksum}, ID={include_dictionary_id}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 96);
    eprintln!("Official zstd v1.5.7 decoded {cases} dictionary frames byte-exactly");
}
