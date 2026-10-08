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
