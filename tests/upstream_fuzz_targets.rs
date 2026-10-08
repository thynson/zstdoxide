//! Executable public-API portions of facebook/zstd v1.5.7 libFuzzer targets.
//! Exact coverage, corpus limits, and missing APIs are in upstream/FUZZ-TARGETS.md.
#![forbid(unsafe_code)]

#[path = "upstream/fuzz_support.rs"]
mod fuzz_support;

use fuzz_support::{DataProducer, decompression_case};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use zstdoxide::{DecodeOptions, Error, FrameHeader, compress, decompress_with_options};

const MAX_INPUT: usize = 1024 * 1024;
const MAX_OUTPUT: usize = 10 * MAX_INPUT;
const MAX_WINDOW: u64 = 128 * 1024 * 1024;
const DEFAULT_MAX_FILES: usize = 1024;
const HARD_MAX_FILES: usize = 65_536;
const MAX_DIRECTORY_DEPTH: usize = 64;

fn options(limit: usize) -> DecodeOptions {
    assert!(limit <= MAX_OUTPUT);
    DecodeOptions {
        max_output_size: limit,
        max_window_size: MAX_WINDOW,
    }
}

#[derive(Debug, Eq, PartialEq)]
struct FrameSummary {
    declared_size: Option<u64>,
    frames: usize,
}

/// Independent test-only boundary walk for the success assertion in
/// simple_decompress.c. It does not decode data or implement a production API.
fn summarize_frames(input: &[u8]) -> Option<FrameSummary> {
    fn take<'a>(input: &'a [u8], position: &mut usize, size: usize) -> Option<&'a [u8]> {
        let end = position.checked_add(size)?;
        let bytes = input.get(*position..end)?;
        *position = end;
        Some(bytes)
    }

    if input.is_empty() {
        return None;
    }
    let mut position = 0;
    let mut summary = FrameSummary {
        declared_size: Some(0),
        frames: 0,
    };
    while position < input.len() {
        let prefix = input.get(position..position.checked_add(4)?)?;
        let magic = u32::from_le_bytes(prefix.try_into().ok()?);
        if magic & 0xffff_fff0 == 0x184d_2a50 {
            take(input, &mut position, 4)?;
            let bytes = take(input, &mut position, 4)?;
            let length = u32::from_le_bytes(bytes.try_into().ok()?) as usize;
            take(input, &mut position, length)?;
        } else {
            let (header, consumed) = FrameHeader::parse(&input[position..]).ok()?;
            take(input, &mut position, consumed)?;
            summary.declared_size = match (summary.declared_size, header.content_size) {
                (Some(total), Some(size)) => Some(total.checked_add(size)?),
                _ => None,
            };
            loop {
                let bytes = take(input, &mut position, 3)?;
                let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
                let size = (value >> 3) as usize;
                let stored = match (value >> 1) & 3 {
                    0 | 2 => size,
                    1 => 1,
                    _ => return None,
                };
                take(input, &mut position, stored)?;
                if value & 1 != 0 {
                    break;
                }
            }
            if header.has_checksum {
                take(input, &mut position, 4)?;
            }
        }
        summary.frames += 1;
    }
    Some(summary)
}

/// `simple_decompress.c::LLVMFuzzerTestOneInput`: preserve exact prefix and
/// output-limit selection, accept any codec error, and validate success sizes.
fn simple_decompress(input: &[u8]) -> bool {
    assert!(input.len() <= MAX_INPUT);
    let mut producer = DataProducer::new(input);
    let prefix = producer.reserve_prefix();
    let limit = producer.unsigned_range(0, (10 * prefix) as u32) as usize;
    match decompress_with_options(&input[..prefix], options(limit)) {
        Ok(output) => {
            assert!(output.len() <= limit);
            let summary = summarize_frames(&input[..prefix])
                .expect("successful decode has complete frame metadata");
            assert!(summary.frames > 0);
            if let Some(expected) = summary.declared_size {
                assert_eq!(expected, output.len() as u64);
            }
            true
        }
        Err(_) => false,
    }
}

/// `decompress_dstSize_tooSmall.c::LLVMFuzzerTestOneInput`: the original
/// tail-selected source and too-small destination become an explicit limit.
fn decompress_destination_too_small(input: &[u8]) {
    assert!(input.len() <= MAX_INPUT);
    let mut producer = DataProducer::new(input);
    let selected_limit = producer.unsigned_range(0, input.len() as u32) as usize;
    let source = &input[..producer.remaining()];
    let limit = selected_limit.min(source.len().saturating_sub(1));
    let encoded = compress(source).expect("compression must succeed");
    let decoded = decompress_with_options(&encoded, options(limit));
    if source.is_empty() {
        assert_eq!(decoded.unwrap(), b"");
    } else {
        assert_eq!(decoded, Err(Error::ResourceLimit));
    }
}

/// Public header-parser portion of `zstd_frame_info.c`. Missing frame-scan,
/// bound, magicless, and dictionary helper APIs are recorded in FUZZ-TARGETS.md.
fn frame_information(input: &[u8]) {
    assert!(input.len() <= MAX_INPUT);
    if let Ok((header, consumed)) = FrameHeader::parse(input) {
        assert!((6..=18).contains(&consumed));
        assert!(consumed <= input.len());
        assert_eq!(
            FrameHeader::parse(&input[..consumed]),
            Ok((header, consumed))
        );
        assert!(FrameHeader::parse(&input[..consumed - 1]).is_err());
    }
}

/// One-shot correctness/determinism from `simple_round_trip.c`: reserve the
/// same source prefix and compare complete encoded bytes, then decoded bytes.
fn simple_round_trip(input: &[u8]) {
    assert!(input.len() <= MAX_INPUT);
    let mut producer = DataProducer::new(input);
    let prefix = producer.reserve_prefix();
    let source = &input[..prefix];
    let first = compress(source).expect("first compression must succeed");
    let second = compress(source).expect("second compression must succeed");
    assert_eq!(first, second, "compression must be deterministic");
    let decoded =
        decompress_with_options(&first, options(input.len())).expect("roundtrip must decode");
    assert_eq!(decoded.len(), source.len());
    assert_eq!(decoded, source);
}

struct Case {
    name: String,
    input: Vec<u8>,
}

fn deterministic_corpus() -> Vec<Case> {
    let mut cases = Vec::new();
    let mut add = |name: &str, bytes: &[u8]| {
        cases.push(Case {
            name: name.into(),
            input: bytes.to_vec(),
        });
    };
    add("empty", &[]);
    add("one-byte", &[0]);
    add("partial-magic", &[0x28, 0xb5, 0x2f]);
    add("invalid-magic", &[1, 0xb5, 0x2f, 0xfd]);
    add(
        "truncated-large-skip",
        &[0x50, 0x2a, 0x4d, 0x18, 0xf8, 0xff, 0xff, 0xff],
    );
    add(
        "unknown-size-empty",
        &[0x28, 0xb5, 0x2f, 0xfd, 0, 0, 1, 0, 0],
    );
    add("official-empty", include_bytes!("fixtures/empty-3.zst"));
    add("official-tiny", include_bytes!("fixtures/tiny-1.zst"));
    add("official-random", include_bytes!("fixtures/random-3.zst"));
    add(
        "official-text-unknown-size",
        include_bytes!("fixtures/text-unknown-size.zst"),
    );
    add(
        "official-two-byte-zero-sequence-count",
        include_bytes!("upstream/data/golden-decompression/zeroSeq_2B.zst"),
    );
    add(
        "official-zero-offset",
        include_bytes!("upstream/data/golden-decompression-errors/off0.bin.zst"),
    );
    add(
        "official-truncated-huffman",
        include_bytes!("upstream/data/golden-decompression-errors/truncated_huff_state.zst"),
    );
    add(
        "official-extraneous-zero-sequence",
        include_bytes!("upstream/data/golden-decompression-errors/zeroSeq_extraneous.zst"),
    );
    let original_count = cases.len();
    for index in 0..original_count {
        let input = cases[index].input.clone();
        let name = cases[index].name.clone();
        for offset in (0..input.len()).step_by((input.len() / 8).max(1)) {
            cases.push(Case {
                name: format!("{name}/truncate-{offset}"),
                input: input[..offset].to_vec(),
            });
            for mask in [1, 0x80] {
                let mut changed = input.clone();
                changed[offset] ^= mask;
                cases.push(Case {
                    name: format!("{name}/byte-{offset}-xor-{mask}"),
                    input: changed,
                });
            }
        }
        if !input.is_empty() {
            cases.push(Case {
                name: format!("{name}/preserved-prefix"),
                input: decompression_case(&input, (input.len() * 10) as u32),
            });
        }
    }
    let mut random = 0x1234_5678_u32;
    for length in [3, 4, 5, 7, 8, 15, 16, 31, 32, 127, 128, 255, 256, 1024] {
        let mut input = Vec::with_capacity(length);
        for _ in 0..length {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            input.push((random >> 24) as u8);
        }
        cases.push(Case {
            name: format!("noise-{length}"),
            input,
        });
    }
    cases
}

fn run_named<T>(case: &str, adapter: &str, test: impl FnOnce() -> T + std::panic::UnwindSafe) -> T {
    match std::panic::catch_unwind(test) {
        Ok(value) => value,
        Err(error) => panic!("{adapter} failed for {case}: {error:?}"),
    }
}

#[test]
fn simple_decompress_deterministic_corpus() {
    let mut successful = 0;
    for case in deterministic_corpus() {
        successful += usize::from(run_named(&case.name, "simple_decompress", || {
            simple_decompress(&case.input)
        }));
    }
    assert!(successful >= 4, "valid decoding paths must be exercised");
}

#[test]
fn too_small_destination_deterministic_corpus() {
    for case in deterministic_corpus() {
        run_named(&case.name, "decompress_dstSize_tooSmall", || {
            decompress_destination_too_small(&case.input);
        });
    }
}

#[test]
fn frame_information_deterministic_corpus() {
    for case in deterministic_corpus() {
        run_named(&case.name, "zstd_frame_info", || {
            frame_information(&case.input)
        });
    }
}

#[test]
fn simple_round_trip_deterministic_corpus() {
    for case in deterministic_corpus() {
        run_named(&case.name, "simple_round_trip", || {
            simple_round_trip(&case.input)
        });
    }
}

#[test]
fn successful_decode_checks_known_unknown_and_concatenated_sizes() {
    let known = compress(b"first").unwrap();
    let unknown = [0x28, 0xb5, 0x2f, 0xfd, 0, 0, 1, 0, 0];
    let skip = [0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0];
    for (data, expected) in [
        (known.clone(), Some(5)),
        (unknown.to_vec(), None),
        (skip.to_vec(), Some(0)),
        (
            [known.as_slice(), &skip, known.as_slice()].concat(),
            Some(10),
        ),
        ([known.as_slice(), &skip, &unknown].concat(), None),
    ] {
        assert!(simple_decompress(&decompression_case(
            &data,
            10 * data.len() as u32
        )));
        assert_eq!(summarize_frames(&data).unwrap().declared_size, expected);
    }
}

fn collect_files(roots: &[PathBuf], max_files: usize) -> io::Result<(Vec<PathBuf>, usize)> {
    assert!((1..=HARD_MAX_FILES).contains(&max_files));
    let max_entries = 4 * max_files;
    let mut pending: Vec<_> = roots.iter().cloned().map(|path| (path, 0)).collect();
    pending.sort_by(|a, b| b.cmp(a));
    pending.dedup();
    let mut discovered = pending.len();
    if discovered > max_entries {
        return Err(io::Error::other("too many corpus root paths"));
    }
    let mut files = Vec::new();
    let mut skipped = 0;
    while let Some((path, depth)) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            skipped += 1;
        } else if metadata.is_dir() {
            if depth >= MAX_DIRECTORY_DEPTH {
                return Err(io::Error::other(format!(
                    "directory depth exceeds {MAX_DIRECTORY_DEPTH}: {}",
                    path.display()
                )));
            }
            let mut entries = Vec::new();
            for entry in fs::read_dir(&path)? {
                discovered += 1;
                if discovered > max_entries {
                    return Err(io::Error::other(format!(
                        "corpus traversal exceeds {max_entries} entries"
                    )));
                }
                entries.push(entry?.path());
            }
            entries.sort_by(|a, b| b.cmp(a));
            pending.extend(entries.into_iter().map(|path| (path, depth + 1)));
        } else if metadata.is_file() {
            if metadata.len() > MAX_INPUT as u64 {
                return Err(io::Error::other(format!(
                    "input exceeds {MAX_INPUT} bytes: {}",
                    path.display()
                )));
            }
            files.push(path);
            if files.len() > max_files {
                return Err(io::Error::other(format!(
                    "corpus exceeds {max_files} files"
                )));
            }
        } else {
            skipped += 1;
        }
    }
    files.sort();
    files.dedup();
    Ok((files, skipped))
}

fn read_case(path: &Path) -> io::Result<Vec<u8>> {
    let mut input = Vec::new();
    // The read cap also covers a file that grows after its metadata was read.
    fs::File::open(path)?
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut input)?;
    if input.len() > MAX_INPUT {
        return Err(io::Error::other(format!(
            "input exceeds {MAX_INPUT} bytes: {}",
            path.display()
        )));
    }
    Ok(input)
}

#[derive(Clone, Copy, Debug)]
enum Target {
    All,
    Decompress,
    DestinationTooSmall,
    FrameInformation,
    RoundTrip,
}

impl Target {
    fn parse(value: &str) -> Self {
        match value {
            "all" => Self::All,
            "simple_decompress" => Self::Decompress,
            "decompress_destination_too_small" => Self::DestinationTooSmall,
            "frame_information" => Self::FrameInformation,
            "simple_round_trip" => Self::RoundTrip,
            _ => panic!(
                "invalid ZSTDOXIDE_FUZZ_TARGET {value:?}; expected all, simple_decompress, decompress_destination_too_small, frame_information, or simple_round_trip"
            ),
        }
    }

    fn run(self, path: &str, input: &[u8]) {
        fn run(path: &str, target: &str, operation: impl FnOnce() + std::panic::UnwindSafe) {
            eprintln!("{target}: {path}");
            run_named(path, target, operation);
        }
        if matches!(self, Self::All | Self::Decompress) {
            run(path, "simple_decompress", || {
                simple_decompress(input);
            });
        }
        if matches!(self, Self::All | Self::DestinationTooSmall) {
            run(path, "decompress_destination_too_small", || {
                decompress_destination_too_small(input)
            });
        }
        if matches!(self, Self::All | Self::FrameInformation) {
            run(path, "frame_information", || frame_information(input));
        }
        if matches!(self, Self::All | Self::RoundTrip) {
            run(path, "simple_round_trip", || simple_round_trip(input));
        }
    }
}

/// Compatible with external fuzz corpora. The selected target preserves its
/// original tail-selected prefix/parameter semantics; `all` runs all four.
#[test]
#[ignore = "requires ZSTDOXIDE_FUZZ_CORPUS pointing to files/directories; see FUZZ-TARGETS.md for caps"]
fn external_file_corpus() {
    let value =
        std::env::var_os("ZSTDOXIDE_FUZZ_CORPUS").expect("ZSTDOXIDE_FUZZ_CORPUS is required");
    let roots: Vec<_> = std::env::split_paths(&value).collect();
    assert!(!roots.is_empty(), "at least one corpus path is required");
    let target = match std::env::var("ZSTDOXIDE_FUZZ_TARGET") {
        Ok(value) => Target::parse(&value),
        Err(std::env::VarError::NotPresent) => Target::All,
        Err(error) => panic!("invalid ZSTDOXIDE_FUZZ_TARGET: {error}"),
    };
    let max_files = match std::env::var("ZSTDOXIDE_FUZZ_MAX_FILES") {
        Ok(value) => value
            .parse()
            .expect("ZSTDOXIDE_FUZZ_MAX_FILES must be an integer"),
        Err(std::env::VarError::NotPresent) => DEFAULT_MAX_FILES,
        Err(error) => panic!("invalid ZSTDOXIDE_FUZZ_MAX_FILES: {error}"),
    };
    assert!(
        (1..=HARD_MAX_FILES).contains(&max_files),
        "ZSTDOXIDE_FUZZ_MAX_FILES must be in 1..={HARD_MAX_FILES}"
    );
    let (files, skipped) =
        collect_files(&roots, max_files).expect("corpus paths must satisfy documented limits");
    assert!(!files.is_empty(), "corpus contains no regular files");
    for path in &files {
        let input = read_case(path).expect("corpus file must be readable and within the cap");
        target.run(&path.display().to_string(), &input);
    }
    eprintln!(
        "checked {} corpus files with target {target:?}; skipped {skipped} symlinks/non-regular entries",
        files.len()
    );
}

struct TemporaryCorpus(PathBuf);

impl TemporaryCorpus {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "zstdoxide-fuzz-corpus-{}-{stamp}-{id}",
                std::process::id()
            ));
            // Cleanup owns only a directory that this call created exclusively.
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create temporary corpus: {error}"),
            }
        }
    }
}

impl Drop for TemporaryCorpus {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn external_corpus_traversal_is_sorted_and_skips_symlinks() {
    let corpus = TemporaryCorpus::new();
    let nested = corpus.0.join("nested");
    fs::create_dir(&nested).unwrap();
    let first = corpus.0.join("a-input");
    let second = nested.join("b-input");
    let third = corpus.0.join("z-input");
    for path in [&third, &second, &first] {
        fs::write(path, [0, 1, 2, 255]).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&corpus.0, nested.join("loop")).unwrap();
    let (files, skipped) =
        collect_files(std::slice::from_ref(&corpus.0), DEFAULT_MAX_FILES).unwrap();
    assert_eq!(files, [first, second, third]);
    assert_eq!(skipped, usize::from(cfg!(unix)));
    for path in files {
        assert_eq!(read_case(&path).unwrap(), [0, 1, 2, 255]);
    }
}

#[test]
fn external_corpus_rejects_oversized_files_without_truncating() {
    let corpus = TemporaryCorpus::new();
    let path = corpus.0.join("oversized-input");
    fs::File::create(&path)
        .unwrap()
        .set_len(MAX_INPUT as u64 + 1)
        .unwrap();
    assert!(
        collect_files(std::slice::from_ref(&corpus.0), DEFAULT_MAX_FILES)
            .unwrap_err()
            .to_string()
            .contains("input exceeds")
    );
    assert!(
        read_case(&path)
            .unwrap_err()
            .to_string()
            .contains("input exceeds")
    );
}
