//! Independent same-description Huffman encoder checks for zstd v1.5.7.
//! Fixture views borrow immutable bytes; request/response buffers own their data.
//! C is compiled into a temporary oracle executable only when explicitly run.

use alloc::vec::Vec;

use crate::bitwriter::BitWriter;
use crate::huffman::HuffmanTable;
use crate::huffman_codebook::HuffmanCodebook;

struct Fixture<'a> {
    source: &'a [u8],
    description: &'a [u8],
    single: &'a [u8],
    four: &'a [u8],
}

struct Records<'a> {
    remaining: &'a [u8],
}

impl<'a> Records<'a> {
    fn take(&mut self, size: usize) -> &'a [u8] {
        assert!(size <= self.remaining.len(), "truncated oracle record");
        let (value, remaining) = self.remaining.split_at(size);
        self.remaining = remaining;
        value
    }

    fn number(&mut self) -> usize {
        u32::from_le_bytes(self.take(4).try_into().unwrap()) as usize
    }
}

fn fixtures() -> Vec<Fixture<'static>> {
    let mut records = Records {
        remaining: include_bytes!("entropy-data/huffman.bin"),
    };
    assert_eq!(records.take(8), b"ZHUF0001");
    assert_eq!(records.number(), 30);
    let mut result = Vec::new();
    for _ in 0..30 {
        let source_size = records.number();
        let description_size = records.number();
        let single_size = records.number();
        let four_size = records.number();
        assert!(source_size <= 128 * 1024);
        assert!((1..=512).contains(&description_size));
        assert!((1..=196_640).contains(&single_size));
        assert!((7..=196_640).contains(&four_size));
        result.push(Fixture {
            source: records.take(source_size),
            description: records.take(description_size),
            single: records.take(single_size),
            four: records.take(four_size),
        });
    }
    assert!(records.remaining.is_empty());
    result
}

/// Encoder portion of `tests/fuzz/huf_round_trip.c:94-127` for the exact
/// independently stored tables. Tree-selection and CPU policies are separate.
#[test]
fn one_stream_encoding_matches_all_thirty_official_tables() {
    let mut writer = BitWriter::new();
    let mut weight_modes = [false; 2];
    for (case, fixture) in fixtures().iter().enumerate() {
        let (table, consumed) = HuffmanTable::read(fixture.description).unwrap();
        assert_eq!(consumed, fixture.description.len());
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        codebook.encode_stream(fixture.source, &mut writer).unwrap();
        assert_eq!(writer.as_bytes(), fixture.single, "official table {case}");
        weight_modes[usize::from(fixture.description[0] >= 128)] = true;
    }
    assert_eq!(weight_modes, [true, true]);
}

/// Four-stream encoder assertions from `tests/fuzz/huf_round_trip.c:118-127`
/// with identical table descriptions and byte-for-byte independent C results.
#[test]
fn four_stream_encoding_matches_all_thirty_official_tables() {
    let mut writer = BitWriter::new();
    let mut output = Vec::new();
    for (case, fixture) in fixtures().iter().enumerate() {
        let (table, consumed) = HuffmanTable::read(fixture.description).unwrap();
        assert_eq!(consumed, fixture.description.len());
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        codebook
            .encode_four_streams(fixture.source, &mut writer, &mut output)
            .unwrap();
        assert_eq!(output, fixture.four, "official table {case}");
    }
}

#[cfg(feature = "std")]
mod oracle {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    const REVISION: &str = "f8745da6ff1ad1e7bab384bd1f9d742439278e99";
    const SOURCE_LIMIT: usize = 128 * 1024;
    const STREAM_LIMIT: usize = 196_640;

    struct Directory(PathBuf);

    impl Drop for Directory {
        fn drop(&mut self) {
            // The test exclusively created this directory; no shared cache is removed.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct Oracle {
        directory: Directory,
    }

    impl Oracle {
        fn compile() -> Self {
            let upstream = PathBuf::from(
                std::env::var_os("ZSTD_UPSTREAM").expect("set ZSTD_UPSTREAM to pinned v1.5.7"),
            );
            let revision = Command::new("git")
                .arg("-C")
                .arg(&upstream)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap();
            assert!(revision.status.success());
            assert_eq!(
                std::str::from_utf8(&revision.stdout).unwrap().trim(),
                REVISION
            );
            let clean = Command::new("git")
                .arg("-C")
                .arg(&upstream)
                .args(["diff", "--quiet", "HEAD", "--", "lib"])
                .status()
                .unwrap();
            assert!(clean.success(), "upstream library sources must be clean");
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir().join(alloc::format!(
                "zstdoxide-huffman-stream-oracle-{}-{stamp}",
                std::process::id()
            ));
            std::fs::create_dir(&directory).unwrap();
            let oracle = Self {
                directory: Directory(directory),
            };
            let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
            let compiled = Command::new(compiler)
                .args(["-std=c99", "-O2", "-Wall", "-Wextra", "-Werror"])
                .arg("-I")
                .arg(upstream.join("lib/common"))
                .arg("-I")
                .arg(upstream.join("lib"))
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/scripts/oracles/huffman_streams.c"
                ))
                .arg(upstream.join("lib/libzstd.a"))
                .arg("-o")
                .arg(oracle.executable())
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                std::string::String::from_utf8_lossy(&compiled.stderr)
            );
            oracle
        }

        fn executable(&self) -> PathBuf {
            self.directory.0.join("huffman-streams")
        }

        fn check(
            &self,
            description: &[u8],
            source: &[u8],
            streams: usize,
            encoded: &[u8],
            codebook: &HuffmanCodebook,
        ) -> usize {
            assert!((1..=512).contains(&description.len()));
            assert!(source.len() <= SOURCE_LIMIT);
            assert!(streams == 1 || (streams == 4 && source.len() >= 6));
            assert!((1..=STREAM_LIMIT).contains(&encoded.len()));
            let mut request = Vec::from(&b"ZHENC001"[..]);
            for value in [description.len(), source.len(), streams, encoded.len()] {
                request.extend_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
            }
            request.extend_from_slice(description);
            request.extend_from_slice(source);
            request.extend_from_slice(encoded);
            let mut child = Command::new(self.executable())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let written = child.stdin.take().unwrap().write_all(&request);
            let response = child.wait_with_output().unwrap();
            assert!(
                response.status.success(),
                "source={}, streams={streams}: {}",
                source.len(),
                std::string::String::from_utf8_lossy(&response.stderr)
            );
            written.unwrap();
            assert!(response.stdout.len() <= 536 + STREAM_LIMIT);
            let mut records = Records {
                remaining: &response.stdout,
            };
            assert_eq!(records.take(8), b"ZHOK0001");
            let log = records.number();
            let symbols = records.number();
            let encoded_size = records.number();
            let flags = records.number();
            assert_eq!(log, usize::from(codebook.table_log()));
            assert!((2..=256).contains(&symbols));
            assert_eq!(encoded_size, encoded.len());
            assert_eq!(flags & !3, 0);
            assert_eq!(flags & 2 != 0, source.is_empty());
            if flags & 1 != 0 {
                assert_eq!(log, 12, "unexpected X2 table-log fallback");
            }
            let weights = records.take(256);
            let lengths = records.take(256);
            let mut maximum = 0;
            for symbol in 0..=255_u8 {
                let bits = codebook.code(symbol).map_or(0, |code| code.bits);
                assert_eq!(lengths[usize::from(symbol)], bits);
                let weight = if bits == 0 { 0 } else { log as u8 + 1 - bits };
                assert_eq!(weights[usize::from(symbol)], weight);
                if bits != 0 {
                    maximum = usize::from(symbol);
                }
            }
            assert_eq!(symbols, maximum + 1);
            assert_eq!(records.take(encoded_size), encoded);
            assert!(records.remaining.is_empty());
            flags
        }
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7, built lib/libzstd.a, and a C compiler"]
    fn official_oracle_verifies_one_stream_encoding_and_pinned_four_streams() {
        let oracle = Oracle::compile();
        let mut writer = BitWriter::new();
        let mut verified = 0;
        let mut x2_fallbacks = 0;
        for fixture in fixtures() {
            let (table, _) = HuffmanTable::read(fixture.description).unwrap();
            let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
            codebook.encode_stream(fixture.source, &mut writer).unwrap();
            assert_eq!(writer.as_bytes(), fixture.single);
            for (streams, encoded) in [(1, writer.as_bytes()), (4, fixture.four)] {
                x2_fallbacks += oracle.check(
                    fixture.description,
                    fixture.source,
                    streams,
                    encoded,
                    &codebook,
                ) & 1;
                verified += 1;
            }
        }

        let description = [128, 0x10];
        let (table, _) = HuffmanTable::read(&description).unwrap();
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        for length in [
            0, 1, 5, 6, 7, 9, 11, 12, 255, 256, 1023, 1024, 16383, 16384, 131072,
        ] {
            let source: Vec<_> = (0..length).map(|index| (index & 1) as u8).collect();
            codebook.encode_stream(&source, &mut writer).unwrap();
            oracle.check(&description, &source, 1, writer.as_bytes(), &codebook);
            verified += 1;
        }

        let dictionary = include_bytes!("data/golden-dictionaries/http-dict-missing-symbols");
        let (table, consumed) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        let description = &dictionary[8..8 + consumed];
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        assert_eq!(codebook.table_log(), 12);
        let present: Vec<_> = (0..=255_u8)
            .filter(|&symbol| codebook.code(symbol).is_ok())
            .collect();
        assert_eq!(present.len(), 242);
        assert_eq!(codebook.code(0).unwrap().bits, 12);
        for length in [1, 6, 9, 12, 242, 1023, 1024, 4096, 131072] {
            let source: Vec<_> = present.iter().copied().cycle().take(length).collect();
            codebook.encode_stream(&source, &mut writer).unwrap();
            x2_fallbacks += oracle.check(description, &source, 1, writer.as_bytes(), &codebook) & 1;
            verified += 1;
        }
        // Every source symbol uses all twelve bits, exercising the actual
        // 128-KiB block's maximum 1X payload rather than a mixed-width average.
        let source = alloc::vec![0; SOURCE_LIMIT];
        codebook.encode_stream(&source, &mut writer).unwrap();
        assert_eq!(writer.as_bytes().len(), 196_609);
        x2_fallbacks += oracle.check(description, &source, 1, writer.as_bytes(), &codebook) & 1;
        verified += 1;
        std::eprintln!(
            "C verified {verified} same-table streams; {x2_fallbacks} explicit X2 table-log fallbacks; one empty-stream marker check bypassed C decoding"
        );
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7, built lib/libzstd.a, and a C compiler"]
    fn official_oracle_verifies_sparse_literal_descriptions_and_fallback_codes() {
        use crate::huffman_encode::build_sparse_literal_table;

        let oracle = Oracle::compile();
        let mut histograms = Vec::new();
        for maximum in [1, 2, 15, 127, 128, 129, 254, 255] {
            let mut counts = [0; 256];
            counts[0] = 7;
            counts[maximum] = 11;
            histograms.push(counts);
        }
        let mut skewed = [0; 256];
        for (symbol, count) in skewed.iter_mut().enumerate().take(60) {
            *count = 1_u64 << symbol;
        }
        histograms.push(skewed);
        skewed[255] = 1;
        histograms.push(skewed);
        // An ordinary uniform tree has 255 constant explicit weights. Its
        // selected nonconstant fallback must provide the codes actually sent.
        histograms.push([1; 256]);

        let mut writer = BitWriter::new();
        let mut modes = [0_usize; 2];
        let mut verified = 0;
        for (case, counts) in histograms.iter().enumerate() {
            let built = build_sparse_literal_table(counts).unwrap();
            let codebook = built.codebook();
            let maximum = counts.iter().rposition(|&count| count != 0).unwrap();
            let direct = built.description()[0] >= 128;
            assert_eq!(direct, maximum <= 128, "description kind, case {case}");
            modes[usize::from(direct)] += 1;
            let present: Vec<_> = (0..=255_u8)
                .filter(|&symbol| counts[usize::from(symbol)] != 0)
                .collect();
            if case < 10 {
                for symbol in 0..=255_u8 {
                    assert_eq!(
                        codebook.code(symbol).is_ok(),
                        counts[usize::from(symbol)] != 0,
                        "sparse support, case {case}, symbol {symbol}"
                    );
                }
            } else {
                assert_eq!(codebook.code(0).unwrap().bits, 7);
                assert_eq!(codebook.code(254).unwrap().bits, 9);
                assert_eq!(codebook.code(255).unwrap().bits, 9);
            }
            assert!(codebook.table_log() <= 11);
            for length in [1, 6, 9, 257, 4096] {
                let source: Vec<_> = present.iter().copied().cycle().take(length).collect();
                codebook.encode_stream(&source, &mut writer).unwrap();
                assert_eq!(
                    oracle.check(built.description(), &source, 1, writer.as_bytes(), codebook),
                    0,
                    "nonempty inline table needs neither X2 fallback nor empty-output bypass"
                );
                verified += 1;
            }
        }
        assert_eq!(histograms.len(), 11);
        assert_eq!(modes, [5, 6]);
        std::eprintln!(
            "C verified 11 sparse/fallback descriptions (6 direct, 5 FSE) and {verified} exact 1X streams through X1 and X2 decoding"
        );
    }

    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pinned v1.5.7, built lib/libzstd.a, and a C compiler"]
    fn official_oracle_verifies_four_stream_encoding_and_boundaries() {
        let oracle = Oracle::compile();
        let mut writer = BitWriter::new();
        let mut output = Vec::new();
        let mut verified = 0;
        for fixture in fixtures() {
            let (table, _) = HuffmanTable::read(fixture.description).unwrap();
            let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
            codebook
                .encode_four_streams(fixture.source, &mut writer, &mut output)
                .unwrap();
            assert_eq!(output, fixture.four);
            assert_eq!(
                oracle.check(fixture.description, fixture.source, 4, &output, &codebook),
                0
            );
            verified += 1;
        }

        let description = [128, 0x10];
        let (table, _) = HuffmanTable::read(&description).unwrap();
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        for length in [
            6, 7, 8, 9, 11, 12, 255, 256, 1023, 1024, 16383, 16384, 131071, 131072,
        ] {
            let source: Vec<_> = (0..length).map(|index| (index & 1) as u8).collect();
            codebook
                .encode_four_streams(&source, &mut writer, &mut output)
                .unwrap();
            if length == 6 || length == 9 {
                assert_eq!(output.last(), Some(&1), "empty final stream marker");
                let first_three: usize = output[..6]
                    .chunks_exact(2)
                    .map(|bytes| usize::from(u16::from_le_bytes(bytes.try_into().unwrap())))
                    .sum();
                assert_eq!(output.len(), 6 + first_three + 1);
            }
            assert_eq!(
                oracle.check(&description, &source, 4, &output, &codebook),
                0
            );
            verified += 1;
        }

        // Exercise encoded-size thresholds separately from regenerated size.
        // The one-/two-bit symbols let each quarter gain whole encoded bytes
        // without changing its source size. A one-bit alphabet alone skips
        // some total sizes when all three leading quarters grow together.
        // Literal header selection itself belongs to later assembly tests.
        let size_description = [129, 0x21];
        let (size_table, _) = HuffmanTable::read(&size_description).unwrap();
        let size_codebook = HuffmanCodebook::from_decoding_table(&size_table).unwrap();
        assert_eq!(size_codebook.code(0).unwrap().bits, 1);
        assert_eq!(size_codebook.code(1).unwrap().bits, 2);
        for target in [1023, 1024, 16383, 16384] {
            let quarter = (target - 10_usize).div_ceil(8) * 8;
            let mut source = alloc::vec![0; 4 * quarter];
            let mut extra_bytes = target - (6 + 4 * (quarter / 8 + 1));
            for part in source.chunks_exact_mut(quarter) {
                let add = extra_bytes.min(quarter / 8);
                part[..add * 8].fill(1);
                extra_bytes -= add;
            }
            assert_eq!(extra_bytes, 0);
            size_codebook
                .encode_four_streams(&source, &mut writer, &mut output)
                .unwrap();
            assert_eq!(output.len(), target);
            assert_eq!(
                oracle.check(&size_description, &source, 4, &output, &size_codebook),
                0
            );
            verified += 1;
        }

        let dictionary = include_bytes!("data/golden-dictionaries/http-dict-missing-symbols");
        let (table, consumed) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        let description = &dictionary[8..8 + consumed];
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        assert_eq!(codebook.table_log(), 12);
        let present: Vec<_> = (0..=255_u8)
            .filter(|&symbol| codebook.code(symbol).is_ok())
            .collect();
        for length in [6, 9, 12, 255, 256, 1023, 1024, 16383, 16384, 131072] {
            let source: Vec<_> = present.iter().copied().cycle().take(length).collect();
            codebook
                .encode_four_streams(&source, &mut writer, &mut output)
                .unwrap();
            assert_eq!(oracle.check(description, &source, 4, &output, &codebook), 0);
            verified += 1;
        }
        let source = alloc::vec![0; SOURCE_LIMIT];
        codebook
            .encode_four_streams(&source, &mut writer, &mut output)
            .unwrap();
        assert_eq!(output.len(), 196_618);
        for bytes in output[..6].chunks_exact(2) {
            assert_eq!(u16::from_le_bytes(bytes.try_into().unwrap()), 49_153);
        }
        assert_eq!(oracle.check(description, &source, 4, &output, &codebook), 0);
        verified += 1;
        assert_eq!(verified, 59);
        std::eprintln!(
            "C verified {verified} exact Rust 4X streams through X1 and X2; tiny6/9 independent quarters, encoded-size boundaries, maximum196618-byte payload"
        );
    }
}
