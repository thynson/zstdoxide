# Completed modules

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

## Validation and measurement

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
