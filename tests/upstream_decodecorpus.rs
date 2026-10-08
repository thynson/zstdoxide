//! Replay official zstd v1.5.7 `tests/decodecorpus.c` against the Rust decoder.
//!
//! Both compressed and original buffers come from the official generator.
//! The fixture importer additionally verifies originals with the official CLI.
//! See `tests/upstream/decodecorpus/README.md` for provenance and mode coverage.

use zstdoxide::{DecodeOptions, Error, FrameHeader, decompress, decompress_with_options};

struct FrameFixture {
    name: &'static str,
    encoded: &'static [u8],
    original: &'static [u8],
    window_size: u64,
    content_size: Option<u64>,
    checksum: bool,
    max_block_size: usize,
    max_content_size: usize,
}

struct HeaderFixture {
    name: &'static str,
    encoded: &'static [u8],
    window_size: u64,
    content_size: Option<u64>,
    checksum: bool,
}

include!("upstream/decodecorpus/cases.rs");

#[test]
fn decodes_every_generated_frame_byte_for_byte() {
    for fixture in FRAMES {
        let actual = decompress(fixture.encoded, fixture.original.len())
            .unwrap_or_else(|error| panic!("{}: {error:?}", fixture.name));
        assert_eq!(actual, fixture.original, "{}", fixture.name);
    }
}

#[test]
fn generated_headers_and_block_sizes_match_recorded_metadata() {
    let mut block_kinds = [false; 3];
    let mut content_size_modes = [false; 2];
    let mut checksum_modes = [false; 2];
    for fixture in FRAMES {
        let (header, mut cursor) = FrameHeader::parse(fixture.encoded).unwrap();
        assert_eq!(header.window_size, fixture.window_size, "{}", fixture.name);
        assert_eq!(
            header.content_size, fixture.content_size,
            "{}",
            fixture.name
        );
        assert_eq!(header.has_checksum, fixture.checksum, "{}", fixture.name);
        assert_eq!(header.dictionary_id, 0, "{}", fixture.name);
        assert!(fixture.original.len() <= fixture.max_content_size);
        if let Some(size) = header.content_size {
            assert_eq!(size, fixture.original.len() as u64, "{}", fixture.name);
        }
        content_size_modes[usize::from(header.content_size.is_some())] = true;
        checksum_modes[usize::from(header.has_checksum)] = true;
        loop {
            let bytes = &fixture.encoded[cursor..cursor + 3];
            let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
            let kind = ((value >> 1) & 3) as usize;
            let size = (value >> 3) as usize;
            assert!(kind < 3, "{}: reserved block kind", fixture.name);
            assert!(size <= fixture.max_block_size, "{}", fixture.name);
            block_kinds[kind] = true;
            cursor += 3 + if kind == 1 { 1 } else { size };
            if value & 1 != 0 {
                break;
            }
        }
        cursor += if header.has_checksum { 4 } else { 0 };
        assert_eq!(cursor, fixture.encoded.len(), "{}", fixture.name);
    }
    assert_eq!(block_kinds, [true; 3]);
    assert_eq!(content_size_modes, [true; 2]);
    // decodecorpus.c deliberately sets the frame checksum flag for every frame.
    assert_eq!(checksum_modes, [false, true]);
}

#[test]
fn rejects_incomplete_generated_frames() {
    for fixture in FRAMES {
        let (_, header_size) = FrameHeader::parse(fixture.encoded).unwrap();
        let first_ends = 0..(header_size + 3).min(fixture.encoded.len());
        let last_ends = fixture.encoded.len().saturating_sub(16)..fixture.encoded.len();
        for end in first_ends.chain(last_ends) {
            assert!(
                decompress(&fixture.encoded[..end], fixture.original.len()).is_err(),
                "{}: accepted truncated frame ending at {end}",
                fixture.name
            );
        }
    }
}

#[test]
fn generated_frames_respect_output_and_window_limits() {
    for fixture in FRAMES {
        if !fixture.original.is_empty() {
            assert!(
                decompress(fixture.encoded, fixture.original.len() - 1).is_err(),
                "{}: ignored output limit",
                fixture.name
            );
        }
        if fixture.window_size != 0 {
            assert_eq!(
                decompress_with_options(
                    fixture.encoded,
                    DecodeOptions {
                        max_output_size: fixture.original.len(),
                        max_window_size: fixture.window_size - 1,
                    },
                ),
                Err(Error::ResourceLimit),
                "{}: ignored window limit",
                fixture.name
            );
        }
    }
}

#[test]
fn generated_checksums_reject_changed_content_digest() {
    let mut checked = 0;
    for fixture in FRAMES.iter().filter(|fixture| fixture.checksum) {
        let mut encoded = fixture.encoded.to_vec();
        *encoded.last_mut().unwrap() ^= 0x80;
        assert_eq!(
            decompress(&encoded, fixture.original.len()),
            Err(Error::ChecksumMismatch),
            "{}",
            fixture.name
        );
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn concatenated_generated_frames_reset_decoder_state() {
    let encoded: Vec<u8> = FRAMES
        .iter()
        .flat_map(|fixture| fixture.encoded.iter().copied())
        .collect();
    let original: Vec<u8> = FRAMES
        .iter()
        .flat_map(|fixture| fixture.original.iter().copied())
        .collect();
    assert_eq!(decompress(&encoded, original.len()).unwrap(), original);
}

#[test]
fn generated_standalone_headers_parse_but_are_not_complete_frames() {
    for fixture in HEADERS {
        let (header, consumed) = FrameHeader::parse(fixture.encoded).unwrap();
        assert_eq!(consumed, fixture.encoded.len(), "{}", fixture.name);
        assert_eq!(header.window_size, fixture.window_size, "{}", fixture.name);
        assert_eq!(
            header.content_size, fixture.content_size,
            "{}",
            fixture.name
        );
        assert_eq!(header.has_checksum, fixture.checksum, "{}", fixture.name);
        assert_eq!(header.dictionary_id, 0, "{}", fixture.name);
        assert!(
            decompress(fixture.encoded, 1 << 20).is_err(),
            "{}: header without blocks accepted",
            fixture.name
        );
        for end in 0..fixture.encoded.len() {
            assert!(FrameHeader::parse(&fixture.encoded[..end]).is_err());
        }
    }
}

#[test]
#[ignore = "requires official v1.5.7 decodecorpus; set ZSTD_DECODECORPUS (default: 4096 new frames)"]
fn replays_configurable_official_generator_run() {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TemporaryCorpus(PathBuf);

    impl Drop for TemporaryCorpus {
        fn drop(&mut self) {
            // This directory was created exclusively for this invocation.
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn parameter(name: &str, default: u32) -> u32 {
        std::env::var(name).map_or(default, |value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("{name} must be a nonnegative 32-bit integer"))
        })
    }

    let count = parameter("ZSTD_DECODECORPUS_CASES", 4096);
    let seed = parameter("ZSTD_DECODECORPUS_SEED", 1);
    assert!(count >= 2, "ZSTD_DECODECORPUS_CASES must be at least 2");
    let executable = std::env::var_os("ZSTD_DECODECORPUS").unwrap_or_else(|| "decodecorpus".into());
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "zstdoxide-decodecorpus-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let temporary = TemporaryCorpus(directory);
    let encoded_dir = temporary.0.join("encoded");
    let original_dir = temporary.0.join("original");
    fs::create_dir(&encoded_dir).unwrap();
    fs::create_dir(&original_dir).unwrap();
    let result = Command::new(executable)
        .args([
            format!("-s{seed}"),
            format!("-n{count}"),
            "--max-content-size-log=16".to_owned(),
            "--max-block-size-log=17".to_owned(),
            format!("-p{}", encoded_dir.display()),
            format!("-o{}", original_dir.display()),
        ])
        .output()
        .expect("build v1.5.7 tests/decodecorpus and set ZSTD_DECODECORPUS");
    assert!(
        result.status.success(),
        "official generator failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    for index in 0..count {
        let encoded = fs::read(encoded_dir.join(format!("z{index:06}.zst"))).unwrap();
        let original = fs::read(original_dir.join(format!("z{index:06}"))).unwrap();
        assert!(original.len() <= 1 << 16);
        let decoded = decompress(&encoded, original.len())
            .unwrap_or_else(|error| panic!("seed {seed}, frame {index}: {error:?}"));
        assert_eq!(decoded, original, "seed {seed}, frame {index}");
    }
}
