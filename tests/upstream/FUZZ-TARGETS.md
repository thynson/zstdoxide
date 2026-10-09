# Public-API fuzz-target adapters

Reference: facebook/zstd v1.5.7 `tests/fuzz/simple_decompress.c`,
`decompress_dstSize_tooSmall.c`, `zstd_frame_info.c`, `simple_round_trip.c`,
and `fuzz_data_producer.c`. Derived test semantics use upstream's BSD-3-Clause
option; the license is preserved in root `LICENSE.zstd`.

## Design recorded before implementation

`DataProducer<'a>` borrows one immutable input slice and stores start/end
indices. It consumes parameter bytes from the tail, shifts their values in the
same order as `FUZZ_dataProducer_uint32Range`, and applies an inclusive modulus.
Exhaustion returns the minimum. Prefix reservation first consumes the upstream
size-selection bytes, then moves the start index to retain the selected tail.
Rollback validates both the original slice bound and the upstream monotonic
remaining-size requirement. These invariants replace the C borrowed pointer.
No unsafe backend is useful for a small test-only parameter record.

Each adapter borrows its source, owns encoded/decoded vectors, and uses explicit
output/window limits. Successful decode metadata is checked by an independent
safe test-only frame boundary walker plus the public `FrameHeader::parse`; this
is not a new production API or a replacement for the absent production frame
scanner. Deterministic corpus mutations own their bytes. External corpus paths
are visited in sorted order with explicit per-file, file-count, output, and
window caps; symbolic links are not followed. Failure reports identify the
adapter and source case. No caller-owned aliasing/in-place or dictionary API is
invented to claim coverage.

## Work decomposition

1. Implement exact tail-driven data-producer semantics and independently verify
   range, exhaustion, prefix reservation, contraction, and rollback vectors.
2. Adapt the four public one-shot fuzz targets and execute a small deterministic
   corpus of valid, malformed, truncated, and mutated frames.
3. Add an opt-in external file/directory corpus runner and document its limits,
   invocation, upstream mapping, and unported parameter/context branches.

## Executable target mapping

All Rust adapters and driver tests live in `tests/upstream_fuzz_targets.rs`.
The tail producer lives in `tests/upstream/fuzz_support.rs`.

| Pinned source / line | Rust mapping | Coverage and remaining work |
| --- | --- | --- |
| `simple_decompress.c:28` `LLVMFuzzerTestOneInput` | `simple_decompress`, `simple_decompress_deterministic_corpus`, `successful_decode_checks_known_unknown_and_concatenated_sizes` | Exact tail-selected source prefix and inclusive `0..10*prefix` output limit. Errors are allowed; success must be bounded and agree with all declared frame sizes. The independent test-only frame walker supplies the assertion because a production `findDecompressedSize` equivalent is absent. Reusable decoder/reset APIs exist; this one-shot adapter does not exercise the original persistent-context branch. |
| `decompress_dstSize_tooSmall.c:30` `LLVMFuzzerTestOneInput` | `decompress_destination_too_small`, `too_small_destination_deterministic_corpus` | Exact tail-selected source length and too-small limit. Empty input succeeds; nonempty input returns typed `ResourceLimit`. Level 1 selection and third-party sequence producers remain unsupported. Reusable contexts exist, but their original target branch and caller-owned one-shot destinations/canaries are not exercised here. |
| `zstd_frame_info.c:22` `LLVMFuzzerTestOneInput` | `frame_information`, `frame_information_deterministic_corpus` | Public `FrameHeader::parse` takes arbitrary input. Successful header parses are bounded, accept exactly the returned header prefix, and reject its final-byte truncation. The metadata record covers content size, dictionary ID, window and checksum. Production frame boundary/decompressed-size/bound/isFrame helpers and magicless configuration are absent. |
| `simple_round_trip.c:141` `LLVMFuzzerTestOneInput`; `:52` `roundTripTest` | `simple_round_trip`, `simple_round_trip_deterministic_corpus` | Exact tail-selected source prefix; byte-identical repeated compression and exact decoded length/content. Full byte comparisons strengthen upstream's encoded size/hash equality. Reusable contexts are available but their original target branch is not exercised here. Compression levels, random advanced parameters, smaller one-shot compression destinations, decoder max-block setting and third-party sequence producers remain unsupported. |
| `simple_round_trip.c:30` `getDecompressionMargin` and in-place roundtrip | No executable substitute | Overlapping caller-owned buffers and decompression-margin APIs do not exist; an owned-output roundtrip does not prove in-place correctness. |
| `fuzz_data_producer.c:19,27,29,51,55,66,70,76,80,90` | `DataProducer::{new,unsigned_range,unsigned,signed_range,remaining,rollback,is_empty,contract,reserve_prefix}` | Original scalar semantics with checked slice indices and ordinary Rust drop. No heap allocation is needed for the producer itself. Signed full-width arithmetic is defined using `i64`; rollback also rejects restoration beyond the borrowed allocation. |

All four complete C fuzz targets remain **partial** ports: their executable
current-API assertions are real, but missing capabilities listed above are not
counted as covered. The upstream disabled superblock expansion assertion remains
unclaimed. These are deterministic adapters, not an LLVM libFuzzer ABI binding
or a substitute for a coverage-guided fuzz campaign.

## Deterministic corpus and producer verification

The default corpus has 347 cases: fourteen initial empty, short, valid, unknown-
size, and malformed inputs; sampled byte mutations and truncations; exact
selector suffixes that preserve original frame prefixes; and fourteen
reproducible noise inputs. It includes existing official tiny/random frames and
all three golden decompression-error fixtures. Every default case runs through all four
adapters. A separate success test covers known and unknown content sizes,
skippable frames, and concatenations; the generic decompression test requires
successful cases so the entire corpus cannot silently exercise only rejection.

The C producer draws parameters from the end, so a raw `.zst` file used directly
as fuzz input will often reserve only part of that frame. This is intentional
upstream behavior. The deterministic corpus additionally appends carefully
encoded producer selectors to preserve full valid frames and exercise success.
The external runner does not reinterpret or append anything to user files.

Seven prefix/output-bound vectors were independently produced by compiling the
pinned upstream `fuzz_data_producer.c` in `/tmp`. Inputs have lengths 0, 1, 2,
255, 256, 65535 and 65536, with byte `i` equal to `(i*17+123) mod 256`. Each vector
records reserved prefix length, remaining parameter bytes, the next
`uint32Range(0,10*prefix)` result, and remaining bytes after that draw. Additional
executable checks cover byte order, equal bounds consuming no bytes, exhaustion,
contraction, rollback, and the entire signed 32-bit range. No C test runtime is
required to run the Rust tests.

## Commands and external corpus limits

```sh
cargo test --test upstream_fuzz_targets
cargo test --test upstream_fuzz_targets --all-features
ZSTDOXIDE_FUZZ_CORPUS=/path/to/corpus ZSTDOXIDE_FUZZ_TARGET=simple_decompress ZSTDOXIDE_FUZZ_MAX_FILES=65536 cargo test --release --test upstream_fuzz_targets external_file_corpus -- --ignored --nocapture
ZSTDOXIDE_FUZZ_CORPUS=tests/upstream/data/golden-decompression-errors:tests/fixtures/tiny-1.zst cargo test --test upstream_fuzz_targets external_file_corpus -- --ignored --nocapture
```

`ZSTDOXIDE_FUZZ_CORPUS` is an operating-system path list: colon-separated on
Unix/macOS and semicolon-separated on Windows. Each root may be a regular file
or a directory; traversal is recursive and sorted. Files run in sorted path
order, and duplicate final paths run once. Symlinks and other non-regular
entries are skipped and reported. A symlink directory loop is never followed.
At least one regular file is required; missing/unreadable paths fail explicitly.

`ZSTDOXIDE_FUZZ_TARGET` selects `simple_decompress`,
`decompress_destination_too_small`, `frame_information`, or `simple_round_trip`;
omitting it or setting `all` runs all four. Use the matching selector for an
upstream target's rolling corpus. The runner prints target and path before each
invocation, including the case that fails or panics.

`ZSTDOXIDE_FUZZ_MAX_FILES` raises the default 1024-file ceiling up to a hard
maximum of 65536. Invalid, zero, and larger values fail explicitly. Total
traversal entries are capped at four times the selected file limit. The input,
output, and window caps remain fixed. Limits are enforced before codec
execution:

| Resource | Limit / behavior |
| --- | --- |
| One fuzz input | 1 MiB. Oversized files fail; they are never silently truncated. |
| Decoded output | Exactly the selected bound, at most 10 MiB. |
| Advertised decoder window | 128 MiB. This is an added resource limit relative to the C target. |
| Regular files encountered | 1024 by default; configurable through 65536, including overlapping supplied paths before deduplication. |
| Total discovered paths | Four times the selected file limit (4096 by default), including directories and skipped entries. |
| Directory depth | 64. |

The bounded file reader also rejects a file that grows beyond the cap between
metadata inspection and reading. Filesystem tests verify sorted nested traversal,
symlink-loop exclusion on Unix, exact byte reads, and oversized-file rejection.
The ignored external runner is an opt-in workload, not a skipped codec feature.
