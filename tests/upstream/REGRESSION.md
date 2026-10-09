# Regression corpus content checks

Reference: facebook/zstd v1.5.7 `tests/regression/data.c`, `method.c`,
`config.c`, `result.c`, and `README.md`. Upstream-derived behavior uses the
BSD-3-Clause licensing option preserved in root `LICENSE.zstd`.

## Design recorded before implementation

`CorpusEntry` owns a `PathBuf` and a validated file length. `CorpusPlan` owns the
sorted entry vector and its total byte count. Non-owning state is represented by
indices or temporary borrowed slices. A bounded directory stack carries path
and depth; symlinks and non-regular entries fail explicitly. File data, encoded
bytes, and decoded bytes are owned vectors that are released after each file.
This avoids loading every source file at once, unlike upstream's buffer-state
setup. These test-only filesystem/value records have no meaningful unsafe
backend requirement.

The loader never truncates or skips an input. It validates per-file and total
limits before codec execution and rechecks the exact length during reading.
Roundtrip checks compare output length and every byte against the original
source. A failure identifies the source path. Elapsed time and byte/file totals
are reported only after all assertions pass; these are workload measurements,
not upstream compression-ratio baselines.

## Work decomposition

1. Implement bounded deterministic file/directory planning and exact per-file
   loading, including explicit symlink/size/total/depth/file-count failures.
2. Add an ignored environment-driven one-shot roundtrip runner and verify its
   normal filesystem/roundtrip checks.
3. Execute all supplied Silesia and GitHub regression inputs in release mode,
   record counts/bytes/time, and map the remaining upstream method/config gaps.

## Scope and source mapping

The upstream regression suite primarily protects compressed byte counts across
a matrix of methods and configurations. This adapter ports its **content
correctness checks on the official directory datasets**, not that full size
regression matrix. The suite therefore remains partial.

| Pinned source / upstream behavior | Rust coverage / remaining work |
| --- | --- |
| `data.c`: `silesia` and `github` directory datasets | All regular files in the extracted official release assets are loaded byte-for-byte and individually roundtripped. The loader owns one input at a time. |
| `data.c`: `silesia_tar` and `github_tar` file datasets | Not executed in the recorded run. The extracted payload files are different inputs from the complete tar byte streams. |
| `data.c`: `data_filenames_get`, `data_buffers_get`, `data_buffer_read`, `data_buffer_compare` | Deterministic recursive file collection, exact reads, and complete length/content comparisons in `CorpusPlan`, `CorpusEntry::read`, and `roundtrip_corpus`. Rust rejects symlinks explicitly and applies documented resource caps. |
| `method.c:94` `simple_compress` | Compression/decompression/content equality adapted to Rust's current one-shot API. Upstream's simple method intentionally skips directory inputs; the Rust directory loop instead corresponds to the per-file content checks of the next method. Compression level selection remains absent. |
| `method.c:144` `compress_cctx_compress` | Per-file non-dictionary content assertions run with reused Rust contexts for Greedy, Chain and Lazy, resetting between pledged frames. The original C method variants, dictionary branch, numeric levels and advanced parameters remain incomplete. |
| `method.c:259` `cli_compress` | Pending a Rust CLI and its configuration matrix. An official executable is not used as the implementation. |
| `method.c:333` `advanced_one_pass_compress_output_adjustment`, wrappers at `:379`/`:385` | Pledged-size streaming APIs exist; the original advanced-parameter/output-adjustment branch and atomic caller-owned compression destination, including the one-byte-smaller bound case, remain unported. |
| `method.c:391` `advanced_streaming_compress` | Incremental compression and flush/finish/reset exist; this regression method's exact 4096-byte input/1024-byte output and parameter schedule remains unported. |
| `method.c:444` `init_cstream`, `:509` `old_streaming_compress_internal`, wrappers at `:585`/`:593`/`:601`/`:609` | Streaming contexts and shared dictionaries exist; the original initialization variants, method adapters and old/new C API equivalence matrix remain unported. |
| `config.c`, `levels.h` | Typed window, frame flags and source pledges exist. The full original level, fast-level, row-matchfinder, dictionary search/attachment/copy/load, hash/chain/search, strategy and advanced-parameter matrix remains unimplemented. |
| `result.c` | Rust test failures distinguish read, compression, decompression, length and byte mismatch. No upstream numeric error/result ABI is claimed. |
| `test.c:249` `run_all`, `:292` `diff_results`, `results.csv` | Full method/configuration Cartesian execution and exact compressed-size CSV comparison remain pending. The reported encoded-byte total is observational, not a golden compression ratio assertion. |

The downloaded GitHub dictionary is preserved separately in the external-corpus
manifest. Passing dictionary bytes through ordinary compression would not test
dictionary support, so it is not substituted for the missing dictionary branch.
No input file within either selected directory is omitted.

## Running and limits

Archive provenance and pinned hashes are in
[`external-corpora.json`](external-corpora.json). The externally cached corpus
is not copied into the repository.

```sh
ZSTDOXIDE_REGRESSION_CORPUS=/tmp/zstdoxide-external-corpora/extracted/silesia:/tmp/zstdoxide-external-corpora/extracted/github cargo test --release --all-features --test upstream_regression_corpora external_regression_corpus_roundtrips -- --ignored --nocapture
```

Select `external_regression_corpus_named_strategies` instead to execute every
file with each named Rust strategy. That adapter reuses a context, resets and
pledges each exact source length, then drains Finish through a guarded 64 KiB
caller buffer. It checks consumed/produced counts and every decoded byte.
Its `STRATEGY` records contain files, source bytes, encoded bytes, complete
length-delimited source and frame FNV-1a hashes, peak reported retained heap
capacity, and skipped count. Compare each strategy's records between backend
builds; strategies may legitimately produce different frames. Retained memory
excludes caller buffers, inline context state and transient allocation overhead.
This functional corpus replay is not a throughput benchmark.

`ZSTDOXIDE_REGRESSION_CORPUS` accepts an operating-system path list of files or
directories: colon-separated on Unix/macOS, semicolon-separated on Windows.
Directory traversal and file execution are sorted. Empty regular files are
included. Missing paths, unreadable entries, symlinks, non-regular entries,
overlapping roots containing duplicate file paths, and an entirely empty corpus
fail explicitly. Oversized inputs are never silently truncated or skipped.

| Resource | Fixed limit |
| --- | --- |
| One source file | 128 MiB |
| Total source data | 512 MiB |
| Regular files | 4096 |
| Total paths discovered | 16384 |
| Directory depth | 64 |
| Decoder output | Exact original file length |
| Advertised decoder window | 128 MiB |

The plan validates every file length and the complete total before starting the
codec. The bounded reader verifies that each actual length still matches its
planned length. Each iteration releases its source, compressed, and decoded
vectors before processing the next file. Normal tests verify sorted processing
of empty/binary/text files, oversize and total-size rejection, changed-length
rejection, and explicit symlink rejection on Unix.

## Recorded complete data run

On 2026-10-08, the release build with all six unsafe features enabled passed all
content assertions for both complete extracted directory assets:

| Dataset | Files | Original bytes |
| --- | ---: | ---: |
| Official regression-data Silesia asset | 12 | 12,000,000 |
| Official regression-data GitHub asset | 500 | 407,963 |
| Combined | 512 | 12,407,963 |

The Silesia release asset contains twelve 1,000,000-byte inputs; it is not the
complete, larger general-purpose Silesia corpus. Every byte present in the
pinned regression asset was tested. Combined compressed size was **6,620,322
bytes**. The measured complete read/compress/decompress/compare loop took
**0.215587 seconds**, excluding directory planning and compilation. This is one
local functional-test timing, not a stable throughput comparison or an upstream
compression-ratio baseline.

The normal driver checks also pass (three tests on Unix, with the external
workload explicitly ignored unless selected), and Clippy with all features and
`-D warnings` passes. No complete regression-matrix parity is claimed.
