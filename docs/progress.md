# Development progress

Detailed completed work items are preserved in Git history. `TODO.md` contains
only active module checklists and the remaining roadmap.

## Foundation

Project instructions, the pinned v1.5.7 data structure inventory, architecture,
and a dependency-free Rust 2024 crate are complete. Both safe default and all
feature combinations compile without `std` using Rust 1.89.0. Codec modules are
being delivered as small work-item commits.

## Entropy primitives

Reverse bitstreams, normalized FSE distributions, bounded two-state Huffman
weights, canonical Huffman tables, and all literal section modes are implemented.
Each access backend has its own feature and checked interface. Focused tests and
the official frame corpus pass in safe and all-feature configurations.

## One-shot frame decoder

Frame/block headers, overlap-aware history, repeat offsets, sequence table reuse,
XXH64 verification, concatenation, and skippable frames are implemented. Output
and advertised-window limits are explicit. Twelve checked-in official v1.5.7
fixtures and 56 runtime oracle cases pass. This milestone excludes dictionaries,
legacy frames, and resumable streaming; full upstream parity remains open.

## Initial compressor

Checksummed frames select raw, RLE, or greedy LZ-compressed blocks. Matches use
position indices, raw literals, explicit offsets, and predefined FSE tables.
Encoding transitions are constructed by inverting the decoding relation.
Thirty-one independent official-decoder cases pass; compression ratio and speed
have not been tuned to match upstream levels. Adaptive entropy, Huffman encoding,
and cross-block matching are still open.

## Project rename

The project is named `zstdoxide`. On 2026-10-08, all 25 existing commits were
rebased from the root to correct the package name, crate references, and script
variables throughout history. All 612 historical file snapshots were checked
against the originals: only the intended textual name changed, and commit
boundaries were preserved. A local recovery branch
`codex/before-zstdoxide-rename` retains the previous history.

## Initial validation and measurement

The safe build, six individual backends, all backends, release mode, and no_std
matrix passed. Targeted Miri checks cover every unsafe access family. The local
suite contains 53 unit tests, five regular interoperability tests, two malformed
input tests, one documentation example, and two separately executed oracle tests.
See [validation.md](validation.md) for exact evidence and remaining gaps.

Every backend and whole-codec workload was measured with five timed samples on
the same source and input. Complete output sizes and FNV-1a checksums match in
all configurations. Isolated unsafe history showed 1.646x throughput; all-feature
text-fixture decompression showed 1.305x while compression was essentially
unchanged. Raw data, ranges, and limitations are recorded in
[benchmarks.md](benchmarks.md). Default builds remain entirely safe.

The initial working codec milestone is complete. Streaming, dictionaries,
advanced compression, custom allocators, and full upstream parity are not.

## Official test migration (in progress)

The pinned upstream tree now has a complete source inventory and 615 named
case-group anchors. Exact golden, inline, testcard, kernel, entropy, and
decodecorpus fixtures have reproducible imports and offline integrity checks.
The executable Rust adaptations include the deterministic upstream generators,
one-shot regressions, four public fuzz targets, entropy decoder targets, and
external regression-data content checks.

At the pre-streaming checkpoint, the suite had 140 ordinary passing tests and nine explicit opt-in/pending
tests. Extended checks include 30,000 one-shot fuzz cycles, 4,096 independently
generated frames, 15,804 fuzz corpus files, and all 512 selected Silesia/GitHub
regression input files. See [the validation report](upstream-test-validation.md)
for the exact distinction between a test, a fixture, and a corpus execution.

This module remains open. At the pre-streaming checkpoint, of the 418 test-related source units, 11 were ported,
25 partial, three planned, 162 blocked by missing capabilities, and 217 are
support/build metadata or C-specific details. These are source dispositions,
not an assertion-coverage percentage. The strict completion gate rejects the
190 unfinished behavioral source units and incomplete external target coverage.
The full-legacy desired-success test still fails with `InvalidMagic` when
explicitly run. `TODO.md` retains the required capability and test work;
completed migration checkpoints are not cleared as a completed major module.


## Module 6: bounded streaming and reusable contexts

Completed on 2026-10-08. Encoder and decoder contexts now accept caller slices,
report exact progress and frame boundaries, retain bounded block/window storage,
and support incremental checksums, Flush/Finish, pledged sizes, poisoned errors,
and reset reuse. Raw output is incremental before the complete block arrives.
The ring history has an independent safe/unsafe backend and full local proofs.
One-shot encoded bytes remained unchanged through workspace extraction/reuse.

All applicable ordinary streaming/context assertions were ported with exact
source mappings, including the two inline regressions and the full near-window
sweep through log 29. Both entire external streaming corpora passed safe and
all-feature release replay: 12,462 decompression and 24,223 round-trip inputs,
with no skipped files. Missing dictionary/parameter/stable-buffer/worker branches
remain explicitly assigned to later work rather than counted as covered.

The full local matrix passed 207 tests in each of ten runs, all nine no_std
configurations, formatting, strict Clippy, source/fixture audits, and independent
oracle checks. Targeted Miri and all-feature compressed streaming regressions
passed. The final external runner was verified separately after the matrix;
the ordinary count remains 207, with 12 deliberate ignored entry points.
Retained-memory plateaus and all nine performance configurations are recorded
in [streaming-validation.md](streaming-validation.md) and [benchmarks.md](benchmarks.md).
Unsafe ring access was slightly slower in isolation and remains opt-in.

The source inventory still has 190 incomplete behavioral source units; completing
streaming does not complete the full upstream migration. Module 7 is decomposed
in `TODO.md` before dictionary implementation begins.


## Module 7: dictionaries, prefixes, and training

Completed on 2026-10-09. Raw and formatted dictionaries now own validated content
and entropy metadata, with immutable shared handles, persistent context selection,
single-use prefixes, exact ID handling, and dictionary-to-frame crossing matches.
Encoder history retains at most 128 KiB of output for cross-block matching.
Bounded raw content selection and independent formatted training produce
interoperable dictionaries with learned literal trees and predefined sequence
priors. COVER/fastCover and advanced training controls remain later obligations.

The supported official parser, lifetime, ID, window, sample-selection, and training
contracts have explicit source mappings. All five dictionary corpora passed safe
and all-feature release replay: 83,358 inputs per backend with no skipped files.
The exact normalized-count target now includes all 512 byte-identical C headers,
10,241 deterministic calls, and both complete 79-input corpus replays.

The complete twelve-run test matrix, eleven no_std configurations, formatting,
strict Clippy, independent C oracles, fixture audits, and targeted Miri passed.
Test-only normalized-count additions landed during the full matrix; the final
suite has 376 passing tests and 22 explicit ignores, and a twelve-configuration
focused follow-up covers every new entropy assertion. Exact per-run counts and
log hashes are retained in [dictionary-validation.md](dictionary-validation.md).
Oracle checks include 30,000 stress cycles and all five exact CNBuffer campaigns.

All eleven backend configurations produced identical deterministic benchmark
outputs. Dictionary access showed a small isolated improvement, while unsafe
training was slightly slower; both remain opt-in. Throughput, sample ranges,
source fingerprints, and retained-memory plateaus are in [benchmarks.md](benchmarks.md).
The full upstream migration remains open: 14 of 418 test-related source units
are ported, 33 partial, 150 blocked, five planned, and 216 non-applicable.

The completed checklist is archived in Git history. Module 8 is decomposed in
`TODO.md` and [entropy-encoder-design.md](entropy-encoder-design.md) before
implementation. It adds Huffman literals, adaptive/reused FSE, repeat planning,
and bounded named compression strategies without upstream level-parity claims.
