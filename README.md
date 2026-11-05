# zstdoxide

An independent, safe-first Rust implementation of the Zstandard format. The
codec has no dependencies and does not call the C library or another Rust codec.
The pinned reference and test oracle is [facebook/zstd v1.5.7](https://github.com/facebook/zstd/tree/v1.5.7),
commit `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.

This is an initial working codec milestone, **not full parity with upstream
zstd**. Its one-shot and streaming decoders handle standard frames with raw or
formatted dictionaries. Its encoder provides independent Greedy, Chain and Lazy
strategies, Huffman literal compression and reuse, and adaptive sequence entropy
tables. Greedy is the named API default. Separate whole-buffer entry points
execute bounded reference Fast, DoubleFast, Greedy, Lazy, Lazy2, BinaryTreeLazy2
and bounded Optimal/Ultra profiles,
including levels 0–6 and negative levels across source tiers, without a dictionary.
Higher admitted levels depend on input size and local resource limits. Their parsers follow the pinned
sampling and repeat rules; entropy
choices remain independent, so compressed bytes and ratios may differ from C.

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
`decompress_into(input, &mut destination)` decodes directly into caller-owned
storage and returns the written prefix length, preserving the unused suffix.
`decompress_into_with_options` caps output at the smaller of slice length and
`max_output_size`; the window limit stays independent. On error, bytes inside
that allowance may contain provisional output, while bytes beyond it stay
unchanged. Compressed blocks still need temporary entropy/literal allocations.
The one-shot slice APIs support concatenation; `decompress_into_with_dictionary`
and `decompress_into_with_dictionary_in` additionally borrow raw/prepared
dictionaries, including borrowed views and independent scratch allocators.
Nonzero dictionary IDs must match; omitted IDs still use the selection.
`SliceDecoder` borrows one fixed destination across temporary input fragments,
writing directly into its frame-local history. Resubmit unconsumed input suffixes;
capacity exhaustion returns `ResourceLimit`. Output is provisional until frame
validation, and failure requires reset. See [stable output usage](docs/stable-output-usage.md).
`SliceDecoder::with_dictionary` and `with_dictionary_in` consume a persistent
shared dictionary or borrowed view. Reset preserves the selection and encoded
input capacity while releasing frame-local entropy overrides; dictionary payload
capacity is reported separately from scratch. `with_prefix` and `with_prefix_in` select a single-use
prefix; `set_prefix` and `set_dictionary` replace selections at clean frame
boundaries. Bounded prefix qualification and lifecycle differences are recorded in
[the validation report](docs/stable-prefix-output-validation.md).
`decompress_in_place` and its options/allocator variants decode suffix-encoded
frames in one exclusive mutable buffer. See [in-place usage](docs/in-place-output-usage.md).
Bounded dictionary-free qualification and layout differences are recorded in
[the validation report](docs/in-place-output-validation.md).
Persistent raw/prepared dictionaries are available through
`decompress_in_place_with_dictionary` and its independent-allocator variant; see
[dictionary in-place usage](docs/dictionary-in-place-usage.md). Its bounded qualification is recorded in
[the dictionary in-place report](docs/dictionary-in-place-validation.md).
`FrameHeader::parse` exposes validated metadata without decoding the frame.
`decompression_margin` scans standard/skippable frame structure without allocating
to compute sufficient suffix-input overlap overhead. It does not validate decoded
contents; actual decoding separately validates payloads, sizes and checksums. Native metadata
boundary differences are recorded in [margin qualification](docs/in-place-margin-validation.md).
Allocation failures and corrupt, truncated, or over-limit input return `Error`.
The named `compress` encoder writes one checksummed single-segment frame, so its advertised window
equals its content size; adjust decoder limits for inputs larger than 128 MiB.

`Encoder` and `Decoder` provide bounded streaming with caller-owned slices and
reusable allocations. `Progress` reports exact input/output prefixes and whether
more input, more output space, or a frame boundary was reached. See
[streaming usage and lifecycle contracts](docs/streaming-usage.md) for complete
examples, Flush/Finish retry rules, pledged sizes, reset, and EOF validation.

`Encoder::set_search_options` configures hash and chain dimensions, four- through
eight-byte keys, bounded Chain/Lazy effort and retained output history for the
existing named policies. Settings are applied before frame processing and
storage is prepared on demand. `search_retained_capacity` reports search caches
separately from total reachable codec capacity. Defaults retain existing output
behavior. See [search configuration and reset examples](docs/match-search-usage.md).

`Encoder::set_options` replaces supported settings before frame processing.
Session reset retains persistent configuration; `reset_parameters` restores
defaults at a fresh or completed boundary, while `reset_all` also discards the
session. Parameter resets remove dictionary/prefix selection and keep reusable
codec storage and its allocator owners.

`inspect_compression_level`, `RawCompressionParameters::adjust` and
`CompressionParameterRequest::resolve` inspect the pinned reference's scalar
profiles with checked size arithmetic. Unknown source size is distinct from
known empty input; explicit overrides and source hints have documented resolution
rules. These allocation-free queries can describe algorithms not yet implemented
by the Rust encoder. See [parameter usage](docs/parameter-usage.md) and
[validation scope](docs/compression-parameters-validation.md).

`compress_at_level` and `compress_with_parameters` execute resolved Fast,
DoubleFast, Greedy, Lazy, Lazy2, BinaryTreeLazy2 and bounded Optimal/Ultra/Ultra2 profiles. Fast supports windows up to 1 MiB and hash logs up to 18;
DoubleFast supports windows up to 2 MiB and independent long/short table logs
up to 18. Greedy, Lazy and Lazy2 support windows up to 2 MiB and hash logs up to 19, selecting
chain storage through window log 14 and row storage above it. Levels 0/3 select DoubleFast; levels 1/2 and negative levels are supported
across all source tiers. Level 4 selects DoubleFast for inputs larger than 16 KiB
and no larger than 128 KiB, and for inputs larger than 256 KiB; its remaining
source tiers select Greedy. BinaryTreeLazy2 uses paired tree edges, with window
log at most 21, independent hash/chain logs at most 19 and search log at most 8.
Levels 5/6 are supported across tiers; levels 7–9 through 256 KiB, and levels
10–12 above 16 KiB through 256 KiB. Level 10 at smaller sizes and levels 7–15
above 256 KiB return resource errors. Optimal, Ultra and Ultra2 admit 64–32,768 bytes. Each source must fit its window, with window logs
up to 16, hash/chain logs up to 12, search
logs up to 6 and target lengths up to 4095. Default levels 11/12 (Optimal) and
13/14 (Ultra), and 16/17 (Ultra2) additionally work for 64–2,048 bytes; valid
strategy-7/8/9 requests outside their respective caps return resource errors.
Optimal, Ultra and Ultra2 preserve the existing single-block paths through 16,384 bytes. Larger
admitted sources use three or four balanced blocks of at most 8,192 bytes,
retaining search and price history across raw/RLE fallback while committing
repeat offsets and entropy only for emitted compressed blocks. Window expiry
and dictionaries are outside this reference-profile extension. Ultra2 seeds
statistics with an initial discarded pass over the first block, invalidates
search history and encodes that block again from the original repeat offsets.
Later retained blocks parse once using the learned statistics and retained search. These APIs write
a known-size frame without a checksum and preserve the resolved window. They do not change the named APIs or provide
live numeric streaming settings. See [reference profile usage](docs/reference-fast-usage.md)
for overrides, allocator ownership, frame flags, and algorithm-specific caps.

`Dictionary` prepares owned raw or formatted content for shared reuse. One-shot
and streaming operations support persistent dictionaries and streaming single-use
prefixes. `train_raw_dictionary`, `train_dictionary`, and `finalize_dictionary`
provide bounded independent Rust content selection and formatted metadata.
See [dictionary use and training](docs/dictionary-usage.md) for runnable examples,
ID selection, reset behavior, capacity limits, and training algorithm boundaries.

Checked frame metadata, exact/unknown concatenated sizes, decompression bounds
and borrowed skippable payload queries are available without decoding. See
[frame query usage](docs/frame-query-usage.md). Structural success does not prove
compressed content or checksums valid. Bounded qualification and explicit native differences are recorded in
[the frame query report](docs/frame-query-validation.md).

Checked `compression_bound` and `write_skippable_frame` utilities support one-pass
size planning and allocation-free caller-owned skippable output. See
[frame utility usage](docs/frame-utility-usage.md). Bounded qualification and native differences
are recorded in [the frame utility report](docs/frame-utility-validation.md).

`OneShotDecoder` owns persistent or next-call dictionary selection for caller-owned
direct and in-place decoding. A next-call prefix covers every frame in that call
and is consumed even on errors. See [one-shot selection usage](docs/one-shot-selection-usage.md).
Bounded qualification and native differences are recorded in
[the one-shot selection report](docs/one-shot-selection-validation.md).

Caller-owned ordinary compression is available through `compress_into` and
`compress_into_in`, preserving the unused success suffix and using one-block
staging. Its completed bounded qualification and limitations are documented in
[the validation report](docs/slice-encode-validation.md).

Dictionary-backed caller-owned compression is available through
`compress_into_with_dictionary` and `compress_into_with_dictionary_in`. Its
completed bounded qualification and remaining scope are recorded in
[the dictionary output validation report](docs/dictionary-slice-encode-validation.md).

Reusable call-scoped selection is available through `OneShotEncoder`: a
persistent dictionary selection survives calls and errors, while
`with_prefix`/`set_prefix` arm exactly the next attempted call, including
empty input, short output and allocation failure. Its completed bounded
qualification and remaining scope are recorded in
[the selection validation report](docs/one-shot-encode-selection-validation.md).

Parameterized dictionary-backed calls are available through
`compress_with_parameters_and_dictionary`, its caller-owned variants and
`OneShotEncoder::compress_into_with_parameters`. A resolved reference profile
and an immutable dictionary view combine in one call over Fast, DoubleFast,
Greedy, Lazy, Lazy2 or BinaryTreeLazy2; Optimal/Ultra/Ultra2 reject nonzero
dictionary content explicitly. In-window dictionary content, prepared entropy
and the dictionary ID follow the existing dictionary contract. See
[parameterized dictionary usage](docs/one-shot-encode-parameters-usage.md) and
[the validation report](docs/one-shot-encode-parameters-validation.md).

Reusable compiled dictionary state is available through `CompiledDictionary`,
`compress_with_compiled(_in)`, `compress_into_with_compiled(_in)` and
`CompiledOneShotEncoder`. Construction fixes one resolved profile and owns the
match-finder tables over the in-window dictionary suffix; calls re-resolve their
request against the actual source and dictionary sizes and reject any profile
difference before writing. Fast, DoubleFast, chain, row and BinaryTreeLazy2
consult the immutable tables read-only beside per-call sampling; formatted
entropy seeds the first block. `CompiledStreamEncoder` adds engineered streaming
over the same immutable state: each sealed block re-binds a reference parser over
the compiled suffix, retained contiguous frame history and the new block, so
cross-block and compiled distances stay exact without reseeding compiled tables.
The selection lifecycle mirrors upstream references: `set_prefix` swaps the
compiled state for a borrowed one-frame prefix and returns it intact, frame
initialization claims that prefix exactly once, `set_compiled` restores a
same-profile state, and parameter-only resets remove the selection while frames
continue dictionary-free under the fixed profile.
See [compiled dictionary usage](docs/prepared-dictionary-usage.md),
[the streaming design](docs/compiled-streaming-design.md),
[the selection lifecycle design](docs/streaming-selection-design.md),
[the validation report](docs/prepared-dictionary-validation.md) and
[the compiled-frame comparison](docs/prepared-dictionary-analysis.md#compiled-frame-comparison-item-9).

`ReferenceStreamEncoder` constructs reference streaming frames directly from a
compression parameter request, with no compiled dictionary. Each frame resolves
its profile from the explicit pledge and optional one-frame prefix before
writing its window descriptor. Six reference families are admitted; reset can
shrink or grow the next frame's geometry within their existing bounds.
`with_prefix` also supports direct raw or formatted prefix construction.
See [request-backed streaming usage](docs/reference-streaming-usage.md) and
[its design](docs/reference-streaming-design.md).

## Current support

| Area | Implemented |
| --- | --- |
| Frames | Standard magic, every content-size width, unknown sizes, checksums, concatenated frames, all sixteen skippable magic values |
| Blocks | Raw, RLE, and compressed blocks; block/window/output bounds |
| Literals | Raw, RLE, direct/FSE Huffman weights, one/four streams, treeless reuse |
| Sequences | Predefined, RLE, FSE-compressed and repeated tables; extra bits; all repeat-offset rules; overlapping matches |
| Compression | Greedy, bounded Chain, and one-byte Lazy matching over dictionary content and up to 1 MiB of configured retained output (128 KiB by default), raw/RLE or new/reused Huffman literals, adaptive/reused/RLE/predefined sequence tables, forward repeat-offset planning, raw/RLE fallback |
| Reference Fast, DoubleFast, Greedy, Lazy, Lazy2 and BinaryTreeLazy2 | Known-size one-shot level/request execution, dictionary-free or with an in-window borrowed raw/formatted dictionary (Optimal/Ultra/Ultra2 reject nonzero dictionaries); sampled hash, chain, row or deferred binary-tree tables, explicit repeat intent, Fast-only negative-level acceleration/raw literal policy, strict gain and transactional block selection; independent entropy output |
| Compiled dictionaries | Immutable resolved profile and read-only seed tables over the in-window suffix; Fast/DoubleFast/chain/row/fully-sorted-tree consultation with per-call sampling and shared attempt budgets; prepared first-block entropy; allocating, caller-owned and coordinator APIs with fixed-profile guards |
| Streaming | Caller-owned input/output slices, bounded window/block storage, incremental checksums, Continue/Flush/Finish, source pledges, reusable contexts, precise frame boundaries |
| Streaming compiled dictionaries | Borrowed immutable compiled state, per-block reference parsing over the compiled suffix plus retained contiguous history, cross-block repeats, session reset, fixed-profile admission, frame flags and dictionary identifiers |
| Parameter inspection | Pinned reference presets, scalar bounds, raw adjustment, explicit request overrides, source hints and checked source/dictionary size arithmetic; independent of live numeric execution |
| Dictionaries | Raw/auto/formatted preparation, shared immutable entropy, IDs, omitted-ID selection, dictionary-to-frame matches, persistent dictionaries and single-use prefixes |
| Training | Bounded deterministic raw content selection, learned full-alphabet Huffman metadata, predefined sequence priors, custom or automatic IDs, formatted finalization |
| Portability | Rust 2024, pinned `nightly-2026-10-08`; `no_std` plus `alloc` core |

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
| `unsafe-slice-history` | Checked match expansion directly in caller-owned output |
| `unsafe-in-place-buffer` | Checked overlapping raw-block movement in one caller buffer |
| `unsafe-frame-sink` | Checked append to caller-owned encoded storage; private compression foundation |
| `unsafe-ring-history` | Bounded wraparound history reads and stores |
| `unsafe-fse` | FSE decoding tables |
| `unsafe-fse-encoder` | Validated inverse FSE encoding tables |
| `unsafe-huffman` | Huffman decoding table |
| `unsafe-huffman-codebook` | Validated canonical encoding codebooks |
| `unsafe-match-table` | Hash-indexed prior-position table |
| `unsafe-match-chain` | Bounded tagged candidate storage for Chain and Lazy matching |
| `unsafe-reference-chain` | Checked cyclic heads/links for the reference lazy family |
| `unsafe-reference-tree` | Checked deferred-tree head/edge storage for bounded BinaryTreeLazy2 requests |
| `unsafe-row-table` | Checked row position/tag storage for the reference lazy family |
| `unsafe-dictionary` | Validated immutable dictionary-content access |
| `unsafe-training` | Bounded raw-training frequency cells |
| `unsafe-optimal-paths` | Initialized Optimal path cells with checked phase and slot access |
| `unsafe-optimal-matches` | Bounded Optimal candidate storage with checked live-prefix access |
| `unsafe-optimal-short-key` | Optional Optimal short-key heads with checked cell access |
| `unsafe-optimal-frequencies` | Checked policy-bound frequency cells for bounded public strategies 7/8 |

Plain scalar records share a representation; they have no pointer access to
replace. Every unsafe block has a local proof, and no unsafe operation is exposed
through the public API. Private unsafe helpers document full `# Safety`
contracts, including the ring's initialized prefix and capacity invariants. Neither malformed input nor a
release-only assumption may bypass range checks.

Read [the data structure inventory](docs/data-structures.md),
[architecture](docs/architecture.md), and module design documents before adding
another backend. Allocation boundaries use fallible reservation. The standard
`Allocator` is supported by `compress_in`, `compress_with_dictionary_in`,
`compress_at_level_in`, `compress_with_parameters_in`,
`decompress_in`, `decompress_with_options_in`,
`decompress_into_in`, `decompress_into_with_options_in`,
`decompress_with_dictionary_in`, `decompress_into_with_dictionary_in`,
`Decoder::new_in`, `Decoder::with_dictionary_in`, `SliceDecoder::new_in`,
`SliceDecoder::with_dictionary_in`, `SliceDecoder::with_prefix_in`, allocator-aware dictionary
construction, `train_raw_dictionary_in`, `train_dictionary_in`, and
`finalize_dictionary_in`. Whole-buffer decoding retains the
original allocator in its returned vector; temporary storage uses fresh clones.
The slice-output variants use the allocator only for scratch and never adopt
the caller's destination.
Borrowing `&allocator` supports non-Clone allocator values. Standard dictionary
Arc/Rc sharing still uses Global for the counts and inline object; borrowed
`DictionaryView` values allocate nothing. Custom streaming encoders and decoders accept either
shared owners or borrowed views independently of their workspace allocator.
`Encoder::new_in`, `with_options_in`, and `with_dictionary_in` retain workspace
ownership across frame reset. One-shot compression retains the original
allocator in its result and releases all temporary workspace before returning.
[Allocator usage](docs/allocator-usage.md) demonstrates caller-owned dictionaries
and output lifetimes. [The allocator plan](docs/allocators.md) records the exact API and validation
boundaries, including the unavailable Rust 1.100 verification.
The completed [allocator feature/oracle matrix](docs/allocator-final-validation.md)
and [allocation/performance comparisons](docs/allocator-benchmarks.md) retain
exact scope, raw evidence and the separate unfinished upstream-migration gate.

## Building and checking

The checked-in toolchain file selects `nightly-2026-10-08` (Rust 1.101.0-nightly,
commit `1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`) for allocator support. The package
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

Configurable named search has a separate 27-case public Encoder benchmark,
with clock-free validation by default, explicit Global timing and an independent
allocation survey. See [search measurements](docs/match-search-benchmarks.md)
for every profile/backend comparison and reproduction commands. This driver
requires Python 3.11 or newer and the pinned Rust toolchain.

Bounded public Optimal has a separate 24-case complete-frame benchmark with
native-audited immutable frames, exact allocation surveys and explicit timing
barriers. See [public qualification](docs/reference-optimal-public-qualification.md)
and [complete-frame measurements](docs/reference-optimal-frame-benchmark.md)
for supported bounds, all backend comparisons and the separate safe/all repeat.

## Development workflow

[AGENTS.md](AGENTS.md) defines English project language, safety documentation,
module decomposition, and small verified commits. [TODO.md](TODO.md) contains
active work; completed module summaries live in [docs/progress.md](docs/progress.md).
Each work item is checked in its implementation commit, and completed module
checklists are cleared at module boundaries.

The project is BSD-3-Clause licensed. [NOTICE](NOTICE) and
[LICENSE.zstd](LICENSE.zstd) retain upstream attribution for algorithms and
format tables. [LICENSE](LICENSE) covers project contributions.
