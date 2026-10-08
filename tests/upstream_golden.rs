//! Port of the official v1.5.7 golden corpus checks in `tests/playTests.sh`.
//!
//! Original assets and generated oracle frames are catalogued in
//! `tests/upstream/data/manifest.json`; see `tests/upstream/GOLDEN.md` for scope.
//! Copyright for upstream fixtures: Meta Platforms, Inc. and affiliates.
//! The upstream BSD license is retained at the repository root in LICENSE.zstd.

use std::io::Write;
use std::process::{Command, Stdio};

use zstdoxide::{Error, compress, decompress};

macro_rules! fixture {
    ($path:literal) => {
        include_bytes!(concat!("upstream/data/", $path)).as_slice()
    };
}

#[test]
fn rle_first_block_decodes_to_one_mebibyte_of_zeros() {
    // playTests.sh compares this output with `dd bs=1048576 count=1 if=/dev/zero`.
    let output = decompress(
        fixture!("golden-decompression/rle-first-block.zst"),
        1 << 20,
    )
    .expect("official RLE-first-block fixture must decode");
    assert_eq!(output.len(), 1 << 20);
    assert!(output.iter().all(|&byte| byte == 0));
}

#[test]
fn empty_block_decodes_to_empty_output() {
    assert_eq!(
        decompress(fixture!("golden-decompression/empty-block.zst"), 0).unwrap(),
        b""
    );
}

#[test]
fn block_at_128k_encoded_size_limit_decodes() {
    // Encoded block size is 128 KiB; official CLI output has 131,068 zero bytes.
    let output = decompress(fixture!("golden-decompression/block-128k.zst"), 131_068).unwrap();
    assert_eq!(output.len(), 131_068);
    assert!(output.iter().all(|&byte| byte == 0));
}

#[test]
fn zero_sequence_count_with_two_byte_encoding_decodes() {
    assert_eq!(
        decompress(fixture!("golden-decompression/zeroSeq_2B.zst"), 13).unwrap(),
        b"Hello World!\n"
    );
}

macro_rules! rejects_golden {
    ($test:ident, $path:literal) => {
        #[test]
        fn $test() {
            assert!(
                decompress(fixture!($path), 1 << 20).is_err(),
                "official malformed fixture {} was accepted",
                $path
            );
        }
    };
}

rejects_golden!(
    rejects_zero_offset,
    "golden-decompression-errors/off0.bin.zst"
);
rejects_golden!(
    rejects_truncated_huffman_state,
    "golden-decompression-errors/truncated_huff_state.zst"
);
rejects_golden!(
    rejects_extraneous_zero_sequence_bytes,
    "golden-decompression-errors/zeroSeq_extraneous.zst"
);

macro_rules! compression_case {
    ($name:ident, $source:literal, $encoded:literal) => {
        mod $name {
            use super::*;

            #[test]
            fn decodes_official_compression() {
                let expected = fixture!($source);
                let actual = decompress(fixture!($encoded), expected.len()).unwrap();
                assert_eq!(actual.len(), expected.len());
                assert_eq!(actual, expected);
            }

            #[test]
            fn rust_encoder_round_trip() {
                let expected = fixture!($source);
                let encoded = compress(expected).unwrap();
                let actual = decompress(&encoded, expected.len()).unwrap();
                assert_eq!(actual.len(), expected.len());
                assert_eq!(actual, expected);
            }

            #[test]
            fn rejects_insufficient_output_limit() {
                let expected = fixture!($source);
                assert_eq!(
                    decompress(fixture!($encoded), expected.len() - 1),
                    Err(Error::ResourceLimit)
                );
            }
        }
    };
}

compression_case!(
    block_splitter_corruption,
    "golden-compression/PR-3517-block-splitter-corruption-test",
    "generated/PR-3517-block-splitter-corruption-test.zst"
);
compression_case!(
    huffman_compressed_larger,
    "golden-compression/huffman-compressed-larger",
    "generated/huffman-compressed-larger.zst"
);
compression_case!(http, "golden-compression/http", "generated/http.zst");
compression_case!(
    large_literal_and_match_lengths,
    "golden-compression/large-literal-and-match-lengths",
    "generated/large-literal-and-match-lengths.zst"
);

#[test]
fn dictionary_fixture_reports_dictionary_requirement() {
    // This tests the current public API contract. It does not stand in for the
    // upstream dictionary round-trip, which needs a future dictionary API.
    assert_eq!(
        decompress(fixture!("generated/http-dictionary.zst"), 1189),
        Err(Error::DictionaryRequired)
    );
}

const COMPRESSION_INPUTS: &[(&str, &[u8])] = &[
    (
        "PR-3517-block-splitter-corruption-test",
        fixture!("golden-compression/PR-3517-block-splitter-corruption-test"),
    ),
    ("http", fixture!("golden-compression/http")),
    (
        "huffman-compressed-larger",
        fixture!("golden-compression/huffman-compressed-larger"),
    ),
    (
        "large-literal-and-match-lengths",
        fixture!("golden-compression/large-literal-and-match-lengths"),
    ),
];

#[test]
#[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN or put zstd on PATH"]
fn official_decoder_accepts_rust_compression_of_all_golden_inputs() {
    let executable = std::env::var_os("ZSTD_BIN").unwrap_or_else(|| "zstd".into());
    let version = Command::new(&executable).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("v1.5.7"));
    for &(name, input) in COMPRESSION_INPUTS {
        let encoded = compress(input).unwrap();
        let mut child = Command::new(&executable)
            .args(["-q", "-d", "-c"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        // Drain stdout while feeding stdin: a large incompressible input can
        // otherwise fill both OS pipes and deadlock the test.
        let output = std::thread::scope(|scope| {
            let writer = scope.spawn(move || stdin.write_all(&encoded));
            let output = child.wait_with_output().unwrap();
            writer.join().unwrap().unwrap();
            output
        });
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), input.len(), "{name}");
        assert_eq!(output.stdout, input, "{name}");
    }
}
