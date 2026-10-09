//! Public encoder regressions for literal sections and frame-scoped entropy.
//!
//! Wire inspection follows the Zstandard format's literals and sequence-count
//! fields. These checks do not require identity with an upstream compression
//! strategy. The optional executable is an independent v1.5.7 decode oracle.
#![forbid(unsafe_code)]

use zstdoxide::{
    DecodeOptions, Dictionary, Encoder, EncoderOptions, EndDirective, FrameHeader,
    SharedDictionary, StreamStatus, compress, decompress, decompress_with_dictionary,
};

const HTTP_DICTIONARY: &[u8] =
    include_bytes!("upstream/data/golden-dictionaries/http-dict-missing-symbols");
const HTTP_SOURCE: &[u8] = include_bytes!("upstream/data/golden-compression/http");

#[derive(Debug)]
struct LiteralSection {
    kind: u8,
    four_streams: bool,
    regenerated: usize,
    section_size: usize,
    sequence_count: usize,
}

#[derive(Debug)]
struct Block<'a> {
    kind: u8,
    payload: &'a [u8],
    literals: Option<LiteralSection>,
}

fn little_endian(bytes: &[u8]) -> u64 {
    assert!(bytes.len() <= 5);
    bytes.iter().enumerate().fold(0, |value, (index, &byte)| {
        value | (u64::from(byte) << (8 * index))
    })
}

/// A bounded, test-only wire inspector. It owns scalar metadata and borrows
/// initialized frame slices; it never shares the encoder's private structures.
fn blocks(frame: &[u8]) -> Vec<Block<'_>> {
    let (header, mut position) = FrameHeader::parse(frame).unwrap();
    let mut blocks = Vec::new();
    loop {
        let value = little_endian(&frame[position..position + 3]);
        position += 3;
        let kind = ((value >> 1) & 3) as u8;
        assert!(kind < 3, "reserved block kind");
        let size = usize::try_from(value >> 3).unwrap();
        assert!(size <= 128 * 1024);
        let stored_size = if kind == 1 { 1 } else { size };
        let payload = &frame[position..position + stored_size];
        position += stored_size;
        let literals = (kind == 2).then(|| {
            let kind = payload[0] & 3;
            let format = (payload[0] >> 2) & 3;
            let (header_size, regenerated, stored, four_streams) = if kind < 2 {
                let (width, shift) = match format {
                    0 | 2 => (1, 3),
                    1 => (2, 4),
                    _ => (3, 4),
                };
                let regenerated =
                    usize::try_from(little_endian(&payload[..width]) >> shift).unwrap();
                (
                    width,
                    regenerated,
                    if kind == 1 { 1 } else { regenerated },
                    false,
                )
            } else {
                let (width, bits) = match format {
                    0 | 1 => (3, 10),
                    2 => (4, 14),
                    _ => (5, 18),
                };
                let value = little_endian(&payload[..width]);
                (
                    width,
                    usize::try_from((value >> 4) & ((1 << bits) - 1)).unwrap(),
                    usize::try_from(value >> (4 + bits)).unwrap(),
                    format != 0,
                )
            };
            assert!(regenerated <= 128 * 1024);
            let section_size = header_size + stored;
            let tail = &payload[section_size..];
            let sequence_count = match tail[0] {
                count @ 0..=127 => usize::from(count),
                count @ 128..=254 => ((usize::from(count) - 128) << 8) + usize::from(tail[1]),
                _ => usize::try_from(little_endian(&tail[1..3])).unwrap() + 0x7f00,
            };
            LiteralSection {
                kind,
                four_streams,
                regenerated,
                section_size,
                sequence_count,
            }
        });
        blocks.push(Block {
            kind,
            payload,
            literals,
        });
        if value & 1 != 0 {
            break;
        }
    }
    assert_eq!(
        position + if header.has_checksum { 4 } else { 0 },
        frame.len()
    );
    blocks
}

/// The lexicographic order-four de Bruijn cycle has every four-symbol key
/// exactly once. Its linear form therefore offers no four-byte match to a
/// fresh encoder, while its sixteen-symbol alphabet halves literal bit cost.
fn de_bruijn() -> Vec<u8> {
    fn visit(depth: usize, period: usize, word: &mut [u8; 5], output: &mut Vec<u8>) {
        if depth == word.len() {
            if 4 % period == 0 {
                output.extend_from_slice(&word[1..=period]);
            }
        } else {
            word[depth] = word[depth - period];
            visit(depth + 1, period, word, output);
            for next in word[depth - period] + 1..16 {
                word[depth] = next;
                visit(depth + 1, depth, word, output);
            }
        }
    }
    let mut output = Vec::new();
    visit(1, 1, &mut [0; 5], &mut output);
    assert_eq!(output.len(), 65_536);
    let mut seen = vec![false; 65_536];
    for start in 0..output.len() {
        let key = (0..4).fold(0, |key, offset| {
            (key << 4) | usize::from(output[(start + offset) % output.len()])
        });
        assert!(!seen[key], "repeated four-byte key at {start}");
        seen[key] = true;
    }
    assert!(seen.into_iter().all(|present| present));
    output
}

fn assert_literal_only(frame: &[u8], source: &[u8], four_streams: bool) {
    let parsed = blocks(frame);
    assert_eq!(parsed.len(), 1, "{parsed:?}");
    assert_eq!(parsed[0].kind, 2, "{parsed:?}");
    let literals = parsed[0].literals.as_ref().unwrap();
    assert_eq!(literals.kind, 2, "fresh frame must describe its tree");
    assert_eq!(literals.four_streams, four_streams);
    assert_eq!(literals.regenerated, source.len());
    assert_eq!(literals.sequence_count, 0);
    assert_eq!(
        &parsed[0].payload[literals.section_size..],
        &[0],
        "zero sequences have neither modes nor state bits"
    );
    assert!(
        frame.len() < source.len() * 3 / 4,
        "literal compression must reduce the complete frame"
    );
    assert_eq!(decompress(frame, source.len()).unwrap(), source);
}

#[test]
fn literal_only_de_bruijn_input_emits_a_smaller_zero_sequence_block() {
    let source = de_bruijn();
    let encoded = compress(&source).unwrap();
    assert_literal_only(&encoded, &source, true);
}

#[test]
fn public_literal_encoding_crosses_one_and_four_stream_size_fields() {
    let source = de_bruijn();
    for size in [127, 128, 255, 256, 1023, 1024, 16_383, 16_384, 65_536] {
        let source = &source[..size];
        assert_literal_only(&compress(source).unwrap(), source, size >= 1024);
    }
}

fn random_alphabet(size: usize, mut state: u64, base: u8) -> Vec<u8> {
    (0..size)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            base + ((state >> 32) as u8 % 16)
        })
        .collect()
}

fn http_dictionary() -> SharedDictionary {
    Dictionary::from_bytes(HTTP_DICTIONARY)
        .unwrap()
        .into_shared()
}

/// Keep directive boundaries constant while varying both caller buffer sizes.
/// The source remains borrowed only for each call, and untouched output bytes
/// are canaries. Zero-capacity calls must not lose staged input or pending bits.
fn action(encoder: &mut Encoder, source: &[u8], directive: EndDirective, chunk: usize) -> Vec<u8> {
    let before_input = encoder.total_input();
    let before_output = encoder.total_output();
    let mut consumed = 0;
    let mut encoded = Vec::new();
    for call in 1..1_000_000 {
        let capacity = if call % 7 == 0 { 0 } else { chunk };
        let mut output = vec![0xa5; capacity + 8];
        let progress = encoder
            .process(&source[consumed..], &mut output[..capacity], directive)
            .unwrap();
        assert!(progress.consumed <= source.len() - consumed);
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
            assert_eq!(consumed, source.len());
            return encoded;
        }
        assert_eq!(progress.status, StreamStatus::NeedOutput);
        assert!(progress.consumed != 0 || progress.produced != 0 || capacity == 0);
    }
    panic!("literal encoder failed to finish within the bounded call count");
}

fn encode_blocks(
    encoder: &mut Encoder,
    sources: &[Vec<u8>],
    input_chunk: usize,
    output_chunk: usize,
) -> Vec<u8> {
    assert!(!sources.is_empty() && input_chunk != 0 && output_chunk != 0);
    let mut frame = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        for fragment in source.chunks(input_chunk) {
            frame.extend(action(
                encoder,
                fragment,
                EndDirective::Continue,
                output_chunk,
            ));
        }
        frame.extend(action(
            encoder,
            &[],
            if index + 1 == sources.len() {
                EndDirective::Finish
            } else {
                EndDirective::Flush
            },
            output_chunk,
        ));
    }
    assert_eq!(
        encoder.total_input(),
        sources
            .iter()
            .map(|source| source.len() as u64)
            .sum::<u64>()
    );
    assert_eq!(encoder.total_output(), frame.len() as u64);
    frame
}

fn assert_modes(frame: &[u8], expected: &[(u8, Option<u8>)]) {
    let parsed = blocks(frame);
    let modes: Vec<_> = parsed
        .iter()
        .map(|block| {
            (
                block.kind,
                block.literals.as_ref().map(|section| section.kind),
            )
        })
        .collect();
    assert_eq!(
        modes, expected,
        "literal lifetime witness changed: {parsed:?}"
    );
}

fn preservation_sources(base: u8) -> Vec<Vec<u8>> {
    vec![
        random_alphabet(8192, 1, base),
        (0..=255).collect(),
        vec![0xed; 4096],
        random_alphabet(8192, 0xdead_beef, base),
    ]
}

fn replacement_sources() -> Vec<Vec<u8>> {
    vec![
        random_alphabet(8192, 1, 0),
        random_alphabet(8192, 12_346, 192),
        random_alphabet(8192, 24_691, 192),
    ]
}

#[test]
fn successive_same_distribution_blocks_reuse_the_previous_inline_tree() {
    let sources: Vec<_> = [1, 12_346, 24_691]
        .into_iter()
        .map(|seed| random_alphabet(8192, seed, 0))
        .collect();
    let frame = encode_blocks(&mut Encoder::new(), &sources, 4096, 4096);
    assert_modes(&frame, &[(2, Some(2)), (2, Some(3)), (2, Some(3))]);
    let expected = sources.concat();
    assert_eq!(decompress(&frame, expected.len()).unwrap(), expected);
}

#[test]
fn intervening_raw_and_rle_blocks_preserve_the_emitted_huffman_tree() {
    let sources = preservation_sources(0);
    let frame = encode_blocks(&mut Encoder::new(), &sources, 4096, 4096);
    assert_modes(&frame, &[(2, Some(2)), (0, None), (1, None), (2, Some(3))]);
    let expected = sources.concat();
    assert_eq!(decompress(&frame, expected.len()).unwrap(), expected);
}

#[test]
fn new_inline_trees_replace_the_prepared_dictionary_and_each_other() {
    // The official HTTP dictionary has a twelve-bit tree and absent symbols
    // 192/193. The first inline tree here only covers 0..15; neither that tree
    // nor the dictionary can encode the second source's full alphabet.
    let sources = replacement_sources();
    assert!(sources[1].contains(&192) && sources[1].contains(&193));
    let dictionary = http_dictionary();
    let mut encoder =
        Encoder::with_dictionary(EncoderOptions::default(), dictionary.clone()).unwrap();
    let frame = encode_blocks(&mut encoder, &sources, 4096, 4096);
    assert_modes(&frame, &[(2, Some(2)), (2, Some(2)), (2, Some(3))]);
    let expected = sources.concat();
    assert_eq!(
        decompress_with_dictionary(&frame, &dictionary, DecodeOptions::new(expected.len()))
            .unwrap(),
        expected
    );
}

#[test]
fn frame_reset_discards_inline_state_and_restores_prepared_dictionary_entropy() {
    let source = vec![random_alphabet(8192, 1, 0)];
    let mut plain = Encoder::new();
    let first = encode_blocks(&mut plain, &source, 4096, 4096);
    plain.reset();
    let second = encode_blocks(&mut plain, &source, 7, 13);
    assert_eq!(second, first);
    assert_modes(&second, &[(2, Some(2))]);

    let dictionary = http_dictionary();
    let mut prepared =
        Encoder::with_dictionary(EncoderOptions::default(), dictionary.clone()).unwrap();
    let replaced = encode_blocks(&mut prepared, &replacement_sources(), 4096, 4096);
    assert_modes(&replaced, &[(2, Some(2)), (2, Some(2)), (2, Some(3))]);
    prepared.reset();
    let source = vec![HTTP_SOURCE[..128].to_vec()];
    let restored = encode_blocks(&mut prepared, &source, 7, 13);
    let mut fresh =
        Encoder::with_dictionary(EncoderOptions::default(), dictionary.clone()).unwrap();
    assert_eq!(restored, encode_blocks(&mut fresh, &source, 4096, 4096));
    assert_modes(&restored, &[(2, Some(3))]);
    assert_eq!(
        decompress_with_dictionary(&restored, &dictionary, DecodeOptions::new(128)).unwrap(),
        source[0]
    );
}

#[test]
fn a_claimed_prefix_does_not_leak_its_huffman_tree_into_the_next_frame() {
    let source = vec![HTTP_SOURCE[..128].to_vec()];
    let dictionary = http_dictionary();
    let mut encoder = Encoder::new();
    encoder.set_prefix(dictionary.clone()).unwrap();
    let first = encode_blocks(&mut encoder, &source, 7, 13);
    assert_modes(&first, &[(2, Some(3))]);
    assert_eq!(
        decompress_with_dictionary(&first, &dictionary, DecodeOptions::new(128)).unwrap(),
        source[0]
    );
    encoder.reset();
    let next = encode_blocks(&mut encoder, &source, 1, 1);
    assert_eq!(
        next,
        encode_blocks(&mut Encoder::new(), &source, 4096, 4096)
    );
    assert_eq!(FrameHeader::parse(&next).unwrap().0.dictionary_id, 0);
    assert!(blocks(&next).iter().all(|block| {
        block
            .literals
            .as_ref()
            .is_none_or(|literals| literals.kind != 3)
    }));
    assert_eq!(decompress(&next, 128).unwrap(), source[0]);
}

#[test]
fn fragmented_input_and_output_preserve_complete_entropy_sections() {
    for (base, use_dictionary) in [(0, false), (192, true)] {
        let sources = preservation_sources(base);
        let expected = sources.concat();
        let dictionary = http_dictionary();
        let mut baseline = None;
        for (input_chunk, output_chunk) in [(4096, 4096), (7, 13), (1, 1)] {
            let mut encoder = if use_dictionary {
                Encoder::with_dictionary(EncoderOptions::default(), dictionary.clone()).unwrap()
            } else {
                Encoder::new()
            };
            let frame = encode_blocks(&mut encoder, &sources, input_chunk, output_chunk);
            assert_modes(&frame, &[(2, Some(2)), (0, None), (1, None), (2, Some(3))]);
            if use_dictionary {
                assert_eq!(
                    decompress_with_dictionary(
                        &frame,
                        &dictionary,
                        DecodeOptions::new(expected.len())
                    )
                    .unwrap(),
                    expected
                );
            } else {
                assert_eq!(decompress(&frame, expected.len()).unwrap(), expected);
            }
            if let Some(baseline) = &baseline {
                assert_eq!(&frame, baseline);
            } else {
                baseline = Some(frame);
            }
        }
    }
}

#[cfg(feature = "std")]
mod oracle {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    fn executable() -> PathBuf {
        let upstream = PathBuf::from(
            std::env::var_os("ZSTD_UPSTREAM")
                .expect("set ZSTD_UPSTREAM to the pinned v1.5.7 checkout"),
        );
        let revision = Command::new("git")
            .arg("-C")
            .arg(&upstream)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(revision.status.success());
        assert_eq!(
            String::from_utf8(revision.stdout).unwrap().trim(),
            "f8745da6ff1ad1e7bab384bd1f9d742439278e99"
        );
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&upstream)
                .args(["diff", "--quiet", "HEAD", "--", "lib", "programs"])
                .status()
                .unwrap()
                .success(),
            "upstream library and CLI sources must be clean"
        );
        let executable = upstream.join("programs/zstd");
        let version = Command::new(&executable).arg("--version").output().unwrap();
        assert!(version.status.success());
        assert!(String::from_utf8_lossy(&version.stdout).contains("v1.5.7"));
        executable
    }

    fn decode(executable: &std::path::Path, frame: &[u8], expected: &[u8], dictionary: bool) {
        let mut command = Command::new(executable);
        command.args(["-q", "-d", "-c"]);
        if dictionary {
            command.arg("-D").arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/upstream/data/golden-dictionaries/http-dict-missing-symbols"
            ));
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = std::thread::scope(|scope| {
            let mut input = child.stdin.take().unwrap();
            let writer = scope.spawn(move || input.write_all(frame));
            let output = child.wait_with_output().unwrap();
            writer.join().unwrap().unwrap();
            output
        });
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected);
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7 with its programs/zstd executable"]
    fn official_decoder_accepts_zero_sequence_huffman_blocks_at_stream_boundaries() {
        let executable = executable();
        let source = de_bruijn();
        for size in [127, 1023, 1024, 16_383, 16_384, 65_536] {
            let source = &source[..size];
            let frame = compress(source).unwrap();
            assert_literal_only(&frame, source, size >= 1024);
            decode(&executable, &frame, source, false);
        }
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7 with its programs/zstd executable"]
    fn official_decoder_accepts_treeless_lifetimes_and_dictionary_replacement() {
        let executable = executable();
        let dictionary = http_dictionary();
        let mut checked_frames = 0;
        for prepared in [false, true] {
            let sources = preservation_sources(if prepared { 192 } else { 0 });
            let mut encoder = if prepared {
                Encoder::with_dictionary(EncoderOptions::default(), dictionary.clone()).unwrap()
            } else {
                Encoder::new()
            };
            let frame = encode_blocks(&mut encoder, &sources, 7, 13);
            assert_modes(&frame, &[(2, Some(2)), (0, None), (1, None), (2, Some(3))]);
            decode(&executable, &frame, &sources.concat(), prepared);
            checked_frames += 1;
        }
        let mut encoder =
            Encoder::with_dictionary(EncoderOptions::default(), dictionary.clone()).unwrap();
        let sources = replacement_sources();
        let frame = encode_blocks(&mut encoder, &sources, 7, 13);
        assert_modes(&frame, &[(2, Some(2)), (2, Some(2)), (2, Some(3))]);
        decode(&executable, &frame, &sources.concat(), true);
        checked_frames += 1;

        encoder.reset();
        let sources = vec![HTTP_SOURCE[..128].to_vec()];
        let frame = encode_blocks(&mut encoder, &sources, 1, 1);
        assert_modes(&frame, &[(2, Some(3))]);
        decode(&executable, &frame, &sources.concat(), true);
        checked_frames += 1;

        let mut prefix = Encoder::new();
        prefix.set_prefix(dictionary).unwrap();
        let first = encode_blocks(&mut prefix, &sources, 1, 1);
        assert_modes(&first, &[(2, Some(3))]);
        decode(&executable, &first, &sources.concat(), true);
        prefix.reset();
        let second = encode_blocks(&mut prefix, &sources, 1, 1);
        assert_eq!(
            second,
            encode_blocks(&mut Encoder::new(), &sources, 4096, 4096)
        );
        decode(&executable, &second, &sources.concat(), false);
        checked_frames += 2;
        assert_eq!(checked_frames, 6);
        eprintln!(
            "C decoded {checked_frames} frames with inline/reused trees, raw/RLE preservation, dictionary replacement, reset and prefix isolation"
        );
    }
}
