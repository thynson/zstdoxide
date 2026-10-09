# zstdoxide

An independent, safe-first Rust implementation of the Zstandard format. The
codec has no dependencies and does not call the C library or another Rust codec.
The pinned reference and test oracle is [facebook/zstd v1.5.7](https://github.com/facebook/zstd/tree/v1.5.7),
commit `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.

This is an initial working codec milestone, **not full parity with upstream
zstd**. Its one-shot and streaming decoders handle standard frames with raw or
formatted dictionaries. Its encoder provides independent Greedy, Chain and Lazy
strategies, Huffman literal compression and reuse, and adaptive sequence entropy
tables. Greedy is the default. These policies do not reproduce upstream numeric
compression levels or promise its compression ratios.

## Using the library

```rust
use zstdoxide::{compress, decompress, DecodeOptions, decompress_with_options};

let input = b"checked Rust, checked Rust, checked Rust";
let encoded = compress(input)?;
let decoded = decompress(&encoded, 1024)?;
assert_eq!(decoded, input);

// Output and advertised-window limits are independent. No advertised window
// is allocated eagerly; the one-shot decoder retains its complete output.
let options = DecodeOptions {
    max_output_size: 8 * 1024 * 1024,
    max_window_size: 8 * 1024 * 1024,
};
assert_eq!(decompress_with_options(&encoded, options)?, input);
# Ok::<(), zstdoxide::Error>(())
```

`decompress` requires an explicit combined-output limit and uses a 128 MiB
advertised-window limit. Use `decompress_with_options` for another window limit.
`FrameHeader::parse` exposes validated metadata without decoding the frame.
Allocation failures and corrupt, truncated, or over-limit input return `Error`.
The encoder writes one checksummed single-segment frame, so its advertised window
equals its content size; adjust decoder limits for inputs larger than 128 MiB.

`Encoder` and `Decoder` provide bounded streaming with caller-owned slices and
reusable allocations. `Progress` reports exact input/output prefixes and whether
more input, more output space, or a frame boundary was reached. See
[streaming usage and lifecycle contracts](docs/streaming-usage.md) for complete
examples, Flush/Finish retry rules, pledged sizes, reset, and EOF validation.

`Dictionary` prepares owned raw or formatted content for shared reuse. One-shot
and streaming operations support persistent dictionaries and streaming single-use
prefixes. `train_raw_dictionary`, `train_dictionary`, and `finalize_dictionary`
provide bounded independent Rust content selection and formatted metadata.
See [dictionary use and training](docs/dictionary-usage.md) for runnable examples,
ID selection, reset behavior, capacity limits, and training algorithm boundaries.

## Current support

| Area | Implemented |
| --- | --- |
| Frames | Standard magic, every content-size width, unknown sizes, checksums, concatenated frames, all sixteen skippable magic values |
| Blocks | Raw, RLE, and compressed blocks; block/window/output bounds |
| Literals | Raw, RLE, direct/FSE Huffman weights, one/four streams, treeless reuse |
| Sequences | Predefined, RLE, FSE-compressed and repeated tables; extra bits; all repeat-offset rules; overlapping matches |
| Compression | Greedy, bounded Chain, and one-byte Lazy matching over dictionary content and up to 128 KiB of retained output, raw/RLE or new/reused Huffman literals, adaptive/reused/RLE/predefined sequence tables, forward repeat-offset planning, raw/RLE fallback |
| Streaming | Caller-owned input/output slices, bounded window/block storage, incremental checksums, Continue/Flush/Finish, source pledges, reusable contexts, precise frame boundaries |
| Dictionaries | Raw/auto/formatted preparation, shared immutable entropy, IDs, omitted-ID selection, dictionary-to-frame matches, persistent dictionaries and single-use prefixes |
| Training | Bounded deterministic raw content selection, learned full-alphabet Huffman metadata, predefined sequence priors, custom or automatic IDs, formatted finalization |
| Portability | Rust 2024, pinned `nightly-2026-10-08` for allocator work; `no_std` plus `alloc` core |

Remaining work includes COVER/fastCover and advanced dictionary training,
advanced compression strategies and
levels, long-distance matching, threading, legacy formats, C ABI, and CLI parity.
There is no command-line compressor yet. Nonzero frame dictionary IDs require
a matching selected dictionary; an omitted ID still uses configured content.
Dictionary lifetime boundary behavior is recorded in
[the dictionary design](docs/dictionary-design.md).

The implementation is new and has not completed a long-running fuzz campaign or
cross-platform release qualification. The checked-in tests, oracle checks, and
Miri runs establish the tested cases, not universal compatibility.

## Safe and unsafe data structures

Default builds enforce `forbid(unsafe_code)`. Slice borrows and indices replace
non-owning pointers; `Vec` owns growable storage and `Arc`/`Rc` shares immutable
prepared dictionaries. An opt-in feature selects an alternative access
implementation for exactly one data structure family, preserving its checked API.

| Feature | Data structure family |
| --- | --- |
| `unsafe-byte-cursor` | Validated byte ranges |
| `unsafe-bitstream` | Reverse/forward bit readers and initialized spare-capacity bit writer |
| `unsafe-history` | Overlapping LZ match expansion |
| `unsafe-ring-history` | Bounded wraparound history reads and stores |
| `unsafe-fse` | FSE decoding tables |
| `unsafe-fse-encoder` | Validated inverse FSE encoding tables |
| `unsafe-huffman` | Huffman decoding table |
| `unsafe-huffman-codebook` | Validated canonical encoding codebooks |
| `unsafe-match-table` | Hash-indexed prior-position table |
| `unsafe-match-chain` | Bounded tagged candidate storage for Chain and Lazy matching |
| `unsafe-dictionary` | Validated immutable dictionary-content access |
| `unsafe-training` | Bounded raw-training frequency cells |

Plain scalar records share a representation; they have no pointer access to
replace. Every unsafe block has a local proof, and no unsafe operation is exposed
through the public API. Private unsafe helpers document full `# Safety`
contracts, including the ring's initialized prefix and capacity invariants. Neither malformed input nor a
release-only assumption may bypass range checks.

Read [the data structure inventory](docs/data-structures.md),
[architecture](docs/architecture.md), and module design documents before adding
another backend. Allocation boundaries use fallible reservation. The standard
`Allocator` is supported by `decompress_in`, `decompress_with_options_in`,
`decompress_with_dictionary_in`,
`Decoder::new_in`, `Decoder::with_dictionary_in`, allocator-aware dictionary
construction, `train_raw_dictionary_in`, `train_dictionary_in`, and
`finalize_dictionary_in`. Whole-buffer decoding retains the
original allocator in its returned vector; temporary storage uses fresh clones.
Borrowing `&allocator` supports non-Clone allocator values. Standard dictionary
Arc/Rc sharing still uses Global for the counts and inline object; borrowed
`DictionaryView` values allocate nothing. Custom streaming encoders and decoders accept either
shared owners or borrowed views independently of their workspace allocator.
`Encoder::new_in`, `with_options_in`, and `with_dictionary_in` retain workspace
ownership across frame reset. One-shot compression is being integrated separately.
[The allocator plan](docs/allocators.md) records the exact API and validation
boundaries, including the unavailable Rust 1.100 verification.

## Building and checking

The checked-in toolchain file selects `nightly-2026-10-08` (Rust 1.101.0-nightly,
commit `1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`) for allocator work. The package
declares `rust-version = "1.101"` as a conservative version floor; the supported
compiler is this exact dated toolchain, not an untested stable release. The
allocator implementation uses the standard API without unstable feature gates.
Rust 1.100 could not be installed from the official distribution on 2026-10-09,
so compatibility with that compiler remains unverified. Completed Module 8
validation and benchmark records retain their original Rust 1.89 environment.

```sh
cargo test
cargo check --no-default-features
sh scripts/check.sh

# Independent official v1.5.7 interoperability; optional external test oracle.
ZSTD_BIN=/path/to/zstd cargo test --test interop -- --ignored

# All unsafe families enabled, with release overflow checks still active.
cargo test --release --all-features
python3 scripts/check_miri_filter.py --lib --all-features history::tests
```

The normal tests use checked-in official-codec fixtures and need no external
executable. The explicit oracle tests cover both directions: official compression
decoded in Rust, and Rust compression decoded by the official implementation.
Fixture generation and provenance are in [tests/fixtures](tests/fixtures/README.md).

The official test migration includes exact golden and inline fixtures, the
upstream testcard, deterministic fuzzer inputs, FSE/Huffman oracle vectors,
and generated decoder corpora. The normal suite is offline; its inventory and
fixture-integrity checks require Python 3.10 or newer with only the standard
library. The complete source/case catalog is in
[the upstream test inventory](docs/upstream-test-inventory.md), with executable
coverage and measured runs in [the validation report](docs/upstream-test-validation.md).
Streaming contracts now have direct incremental tests and near-window oracle
fixtures. Advanced dictionary/training, CLI, legacy, advanced encoder, and other unavailable API
contracts remain explicitly open. A successful normal test run does not
mean every official test has been migrated.

```sh
# Requires an external pinned source tree, static library, CLI with legacy
# support 4, and decodecorpus generator. Also runs the 30,000-cycle stress test.
ZSTD_UPSTREAM=/path/to/zstd-v1.5.7 sh scripts/check_upstream_oracles.sh

# Download and verify separately pinned external corpus snapshots.
python3 scripts/fetch_upstream_corpora.py --cache /path/outside/the/repository

# This deliberately fails while any required upstream test contract is open.
python3 scripts/audit_upstream_tests.py --require-complete
```

The external corpora are rolling release snapshots, independently hashed and
kept outside the repository. Replay commands and exact API limitations are in
[FUZZ-TARGETS.md](tests/upstream/FUZZ-TARGETS.md),
[REGRESSION.md](tests/upstream/REGRESSION.md), and
[the entropy corpus notes](tests/upstream/entropy-data/README.md).

## Measuring backend performance

```sh
bash scripts/benchmark.sh
```

The benchmark runs the safe build, each feature independently, and all features
on identical deterministic inputs. It verifies output fingerprints and refuses
to combine builds if source files change during a run. Timing uses five samples
per workload and reports medians and ranges. See
[benchmark design](docs/benchmark-design.md) and [recorded measurements](docs/benchmarks.md).
Unsafe implementations remain opt-in even when a benchmark favors them.

## Development workflow

[AGENTS.md](AGENTS.md) defines English project language, safety documentation,
module decomposition, and small verified commits. [TODO.md](TODO.md) contains
active work; completed module summaries live in [docs/progress.md](docs/progress.md).
Each work item is checked in its implementation commit, and completed module
checklists are cleared at module boundaries.

The project is BSD-3-Clause licensed. [NOTICE](NOTICE) and
[LICENSE.zstd](LICENSE.zstd) retain upstream attribution for algorithms and
format tables. [LICENSE](LICENSE) covers project contributions.
