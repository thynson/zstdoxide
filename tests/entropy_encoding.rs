//! Public encoder regressions for literal sections and frame-scoped entropy.
//!
//! Wire inspection follows the Zstandard format's literals and sequence-count
//! fields. These checks do not require identity with an upstream compression
//! strategy. The optional executable is an independent v1.5.7 decode oracle.
#![forbid(unsafe_code)]

use zstdoxide::{FrameHeader, compress, decompress};

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

fn little_endian(bytes: &[u8]) -> usize {
    assert!(bytes.len() <= 5);
    bytes.iter().enumerate().fold(0, |value, (index, &byte)| {
        value | (usize::from(byte) << (8 * index))
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
        let size = value >> 3;
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
                let regenerated = little_endian(&payload[..width]) >> shift;
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
                    (value >> 4) & ((1 << bits) - 1),
                    value >> (4 + bits),
                    format != 0,
                )
            };
            assert!(regenerated <= 128 * 1024);
            let section_size = header_size + stored;
            let tail = &payload[section_size..];
            let sequence_count = match tail[0] {
                count @ 0..=127 => usize::from(count),
                count @ 128..=254 => ((usize::from(count) - 128) << 8) + usize::from(tail[1]),
                _ => little_endian(&tail[1..3]) + 0x7f00,
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
        let executable = upstream.join("programs/zstd");
        let version = Command::new(&executable).arg("--version").output().unwrap();
        assert!(version.status.success());
        assert!(String::from_utf8_lossy(&version.stdout).contains("v1.5.7"));
        executable
    }

    fn decode(executable: &std::path::Path, frame: &[u8], expected: &[u8]) {
        let mut child = Command::new(executable)
            .args(["-q", "-d", "-c"])
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
            decode(&executable, &frame, source);
        }
    }
}
