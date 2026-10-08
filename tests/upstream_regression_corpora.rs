//! Content-correctness portion of facebook/zstd v1.5.7 regression-data tests.
//! Compression-ratio/configuration coverage remains explicitly partial;
//! see upstream/REGRESSION.md for the source mapping and runner limits.
#![forbid(unsafe_code)]

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use zstdoxide::{DecodeOptions, compress, decompress_with_options};

const MAX_FILE_SIZE: u64 = 128 * 1024 * 1024;
const MAX_TOTAL_SIZE: u64 = 512 * 1024 * 1024;
const MAX_FILES: usize = 4096;
const MAX_ENTRIES: usize = MAX_FILES * 4;
const MAX_DEPTH: usize = 64;

#[derive(Debug)]
struct CorpusEntry {
    path: PathBuf,
    size: u64,
}

#[derive(Debug)]
struct CorpusPlan {
    entries: Vec<CorpusEntry>,
    total_size: u64,
}

fn failure(path: &Path, reason: &str) -> io::Error {
    io::Error::other(format!("{}: {reason}", path.display()))
}

impl CorpusPlan {
    fn collect(roots: &[PathBuf]) -> io::Result<Self> {
        if roots.is_empty() || roots.len() > MAX_ENTRIES {
            return Err(io::Error::other("expected 1..=16384 corpus root paths"));
        }
        let mut pending: Vec<_> = roots.iter().cloned().map(|path| (path, 0)).collect();
        pending.sort_by(|a, b| b.cmp(a));
        let mut discovered = pending.len();
        let mut plan = Self {
            entries: Vec::new(),
            total_size: 0,
        };
        while let Some((path, depth)) = pending.pop() {
            let metadata =
                fs::symlink_metadata(&path).map_err(|error| failure(&path, &error.to_string()))?;
            if metadata.file_type().is_symlink() {
                return Err(failure(&path, "symbolic links are not accepted"));
            }
            if metadata.is_dir() {
                if depth >= MAX_DEPTH {
                    return Err(failure(&path, "directory depth exceeds 64"));
                }
                let mut children = Vec::new();
                for child in fs::read_dir(&path)? {
                    discovered += 1;
                    if discovered > MAX_ENTRIES {
                        return Err(failure(&path, "corpus traversal exceeds 16384 entries"));
                    }
                    children.push(child?.path());
                }
                children.sort_by(|a, b| b.cmp(a));
                pending.extend(children.into_iter().map(|child| (child, depth + 1)));
            } else if metadata.is_file() {
                if metadata.len() > MAX_FILE_SIZE {
                    return Err(failure(&path, "file exceeds 128 MiB"));
                }
                if plan.entries.len() == MAX_FILES {
                    return Err(failure(&path, "corpus exceeds 4096 files"));
                }
                plan.total_size += metadata.len();
                if plan.total_size > MAX_TOTAL_SIZE {
                    return Err(failure(&path, "corpus exceeds 512 MiB total"));
                }
                plan.entries.push(CorpusEntry {
                    path,
                    size: metadata.len(),
                });
            } else {
                return Err(failure(&path, "non-regular corpus entry"));
            }
        }
        if plan.entries.is_empty() {
            return Err(io::Error::other("corpus contains no regular files"));
        }
        plan.entries.sort_by(|a, b| a.path.cmp(&b.path));
        if plan
            .entries
            .windows(2)
            .any(|pair| pair[0].path == pair[1].path)
        {
            return Err(io::Error::other(
                "overlapping corpus roots contain a duplicate file path",
            ));
        }
        Ok(plan)
    }
}

impl CorpusEntry {
    fn read(&self) -> io::Result<Vec<u8>> {
        let mut source = Vec::new();
        fs::File::open(&self.path)?
            .take(MAX_FILE_SIZE + 1)
            .read_to_end(&mut source)?;
        if source.len() as u64 != self.size {
            return Err(failure(
                &self.path,
                "file length changed after corpus planning",
            ));
        }
        Ok(source)
    }
}

#[derive(Debug)]
struct RunSummary {
    files: usize,
    source_bytes: u64,
    encoded_bytes: u64,
    elapsed: Duration,
}

/// Content equality from method.c::simple_compress/compress_cctx_compress.
/// The Rust API currently offers one owned-output compression strategy.
fn roundtrip_corpus(plan: &CorpusPlan) -> RunSummary {
    let started = Instant::now();
    let mut encoded_bytes = 0;
    for entry in &plan.entries {
        let source = entry
            .read()
            .unwrap_or_else(|error| panic!("read {}: {error}", entry.path.display()));
        let encoded = compress(&source)
            .unwrap_or_else(|error| panic!("compress {}: {error}", entry.path.display()));
        let decoded = decompress_with_options(
            &encoded,
            DecodeOptions {
                max_output_size: source.len(),
                max_window_size: MAX_FILE_SIZE,
            },
        )
        .unwrap_or_else(|error| panic!("decompress {}: {error}", entry.path.display()));
        assert_eq!(
            decoded.len(),
            source.len(),
            "decoded length: {}",
            entry.path.display()
        );
        if let Some(offset) = source.iter().zip(&decoded).position(|(a, b)| a != b) {
            panic!(
                "roundtrip byte mismatch in {} at offset {offset}",
                entry.path.display()
            );
        }
        encoded_bytes += encoded.len() as u64;
        // All three vectors are dropped here before the next file is loaded.
    }
    RunSummary {
        files: plan.entries.len(),
        source_bytes: plan.total_size,
        encoded_bytes,
        elapsed: started.elapsed(),
    }
}

#[test]
#[ignore = "requires ZSTDOXIDE_REGRESSION_CORPUS pointing to regression-data files/directories"]
fn external_regression_corpus_roundtrips() {
    let value = std::env::var_os("ZSTDOXIDE_REGRESSION_CORPUS")
        .expect("ZSTDOXIDE_REGRESSION_CORPUS is required");
    let roots: Vec<_> = std::env::split_paths(&value).collect();
    let plan =
        CorpusPlan::collect(&roots).expect("regression corpus must satisfy documented limits");
    eprintln!(
        "running complete regression corpus: {} files, {} source bytes",
        plan.entries.len(),
        plan.total_size
    );
    let summary = roundtrip_corpus(&plan);
    eprintln!(
        "passed {} files, {} source bytes, {} encoded bytes, elapsed {:.6} seconds",
        summary.files,
        summary.source_bytes,
        summary.encoded_bytes,
        summary.elapsed.as_secs_f64()
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
                "zstdoxide-regression-{}-{stamp}-{id}",
                std::process::id()
            ));
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
fn corpus_roundtrips_every_file_in_sorted_order() {
    let corpus = TemporaryCorpus::new();
    fs::create_dir(corpus.0.join("nested")).unwrap();
    let paths = [
        corpus.0.join("a-empty"),
        corpus.0.join("nested/b-binary"),
        corpus.0.join("z-text"),
    ];
    fs::write(&paths[2], b"regression corpus roundtrip\n").unwrap();
    fs::write(&paths[1], [0, 255, 1, 128]).unwrap();
    fs::write(&paths[0], []).unwrap();
    let plan = CorpusPlan::collect(std::slice::from_ref(&corpus.0)).unwrap();
    assert_eq!(
        plan.entries
            .iter()
            .map(|entry| &entry.path)
            .collect::<Vec<_>>(),
        paths.iter().collect::<Vec<_>>()
    );
    let summary = roundtrip_corpus(&plan);
    assert_eq!(summary.files, 3);
    assert_eq!(summary.source_bytes, 32);
    assert!(summary.encoded_bytes > 0);
}

#[test]
fn corpus_loader_rejects_oversize_totals_and_changed_lengths() {
    let corpus = TemporaryCorpus::new();
    let path = corpus.0.join("oversized");
    fs::File::create(&path)
        .unwrap()
        .set_len(MAX_FILE_SIZE + 1)
        .unwrap();
    assert!(
        CorpusPlan::collect(std::slice::from_ref(&corpus.0))
            .unwrap_err()
            .to_string()
            .contains("128 MiB")
    );
    fs::remove_file(&path).unwrap();
    for index in 0..5 {
        fs::File::create(corpus.0.join(format!("file-{index}")))
            .unwrap()
            .set_len(MAX_FILE_SIZE)
            .unwrap();
    }
    assert!(
        CorpusPlan::collect(std::slice::from_ref(&corpus.0))
            .unwrap_err()
            .to_string()
            .contains("512 MiB")
    );
    fs::write(&path, [1, 2, 3]).unwrap();
    let plan = CorpusPlan::collect(std::slice::from_ref(&path)).unwrap();
    fs::write(&path, [1, 2]).unwrap();
    assert!(
        plan.entries[0]
            .read()
            .unwrap_err()
            .to_string()
            .contains("length changed")
    );
}

#[cfg(unix)]
#[test]
fn corpus_loader_rejects_symlinks_instead_of_skipping_them() {
    let corpus = TemporaryCorpus::new();
    std::os::unix::fs::symlink(&corpus.0, corpus.0.join("loop")).unwrap();
    assert!(
        CorpusPlan::collect(std::slice::from_ref(&corpus.0))
            .unwrap_err()
            .to_string()
            .contains("symbolic links")
    );
}
