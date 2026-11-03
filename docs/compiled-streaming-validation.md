# Streaming compiled-dictionary validation

This report covers Module 38: the bounded streaming coordinator over one
immutable compiled dictionary. Design and ownership invariants are in
[compiled-streaming-design.md](compiled-streaming-design.md); usage is in
[prepared-dictionary-usage.md](prepared-dictionary-usage.md).

## What was implemented

`CompiledStreamEncoder<'a, A, B>` borrows a `CompiledDictionary<'a, B>` and owns
the streaming workspace in `A`. At every sealed block it builds one contiguous
buffer of

```text
compiled loaded suffix | retained frame history | current block
```

and re-binds a reference search state whose parse start is
`compiled_len + history_len`. The compiled candidate views stay read-only and
suffix-relative. Fast, DoubleFast, chain and row owners receive a seeding pass
over only the retained history; the Greedy tree keeps its lazy back-fill from
the combined-buffer start when history is present. Compiled tables are never
reseeded, copied or mutated.

The coordinator keeps the last `min(frame_position, window)` frame bytes
contiguously. This is the correctness invariant behind all emitted distances:

- a candidate in retained history has the same local and decoder distance;
- a compiled candidate has the true distance while the frame has not wrapped;
- after the frame exceeds the window, every compiled candidate's local
  distance is larger than the window, so the ordinary parser window floor
  rejects it exactly where the decoder would.

Entropy publication is transactional: a compressed block commits its literal
tree and sequence tables, raw/RLE fallback discards only the candidate, and
committed wire repeats cross block boundaries. The first block of every frame
installs the compiled formatted-dictionary entropy seed.

## Incremental and public coverage

- Nine focused unit tests cover all six admitted families with multi-block
  inputs beyond and below the window, checksums, pledges, empty/tiny frames,
  `Flush` followed by `Finish`, session reset, omitted frame flags, directive
  and pledge guards, allocator failures, and a parser-level comparison of
  compiled-plus-history binding against full-prefix seeding for the hash and
  chain/row families. The tree is excluded from that equality because its
  eagerly sorted compiled tree can add candidates the lazy per-call back-fill
  has not inserted; this is a measured family property, not a history-binding
  difference.
- Seven public integration tests cover the same families through
  caller-owned pumping, a formatted dictionary and identifier, single-block
  body equality with the one-shot compiled frame, multi-block body equality
  below window wrap, session/parameter reset, frame-flag omission and workspace
  allocator release.
- The public `CompiledStreamEncoder` doc example runs as a doctest.
- The ignored guarded native schedule exports Rust frames and replays native
  frames.

A single-block streaming body is byte-identical to the one-shot compiled body
under equal profiles. Below window wrap, two-block bodies are also identical
for Fast, DoubleFast, Greedy, Lazy, Lazy2 and the tree, even though the
streaming header is non-single-segment while the one-shot header is
single-segment. Beyond wrap, only correctness and decodability are claimed.

## Guarded native streaming schedules

Twelve typed schedules run against normal and sanitized pinned v1.5.7
`ZSTD_CCtx_refCDict` + `ZSTD_compressStream2` controls:

| Schedule | Profile | Frames | Result |
| --- | --- | --- | --- |
| `stream-raw-fast` | Fast, raw dictionary | 1 | supported |
| `stream-raw-double-fast` | DoubleFast | 1 | supported |
| `stream-raw-chain` | Greedy chain | 1 | supported |
| `stream-raw-tree` | BinaryTreeLazy2 | 1 | supported |
| `stream-prepared-fast` | Fast, formatted entropy | 1 | supported |
| `stream-prepared-chain` | Greedy chain, formatted | 1 | supported |
| `stream-prepared-tree` | BinaryTreeLazy2, formatted | 1 | supported |
| `stream-raw-window` | Greedy, window log 10 | 1 | supported |
| `stream-raw-large` | Greedy, multi-block | 1 | supported |
| `stream-raw-multiframe` | Greedy, two frames sharing the CDict | 2 | supported |
| `stream-optimal` | Optimal CDict | 1 | native-only; Rust construction rejected `UnsupportedFeature` |
| `stream-ultra2` | Ultra2 CDict | 1 | native-only; Rust construction rejected `UnsupportedFeature` |

Every case uses a caller output window between one and eleven bytes, so native
and Rust both drain pending output incrementally. The sanitized and normal
oracles produce identical twelve rows and thirteen frame files under scoped
ASan/UBSan. Rust streaming frames decode natively and every native streaming
frame decodes in Rust with the dictionary; the multi-frame case proves the
session-only frame boundary retains the compiled selection. Destination guard
bytes stay unchanged and each native frame decodes to the exact input.

Evidence: `evidence/2026-11-03-compiled-streaming-native-platform.json` and its
campaign archive.

## Concrete atomic-free ARM and static checks

`cargo check -Zbuild-std=core,alloc --target thumbv6m-none-eabi
--no-default-features` passes for safe and for all 22 opt-in unsafe features
together. Release LLVM IR and object emission pass for both configurations and
the artifacts are retained. Safe and all-feature host Clippy with `-D warnings`
and `cargo fmt --check` pass. No Miri ran because this module added no unsafe
code, per the standing direction.

## Complete feature and release regression

All 75 commands pass on the frozen Module38 source: safe/all and every one of
the 22 isolated unsafe features run the complete test suite, true no_std
compilation and strict Clippy; safe/all release and formatting also pass. Every
complete/release suite reports 125 result groups, 1848 passed and 79 ignored
with zero failures; ignored tests are not credited. The benchmark harness is
retained only in its own campaign archive and is not part of this matrix.
Evidence: `evidence/2026-11-03-compiled-streaming-feature-matrix.json`.
One earlier matrix attempt stopped at `unsafe-dictionary-full` because the
host disk filled during Rust's post-link object copy; the rerun on the same
source completed all 75 commands after campaign workspaces were removed.

## Streaming timing

Eighteen deterministic case/level pairs cover Fast, DoubleFast, chain, the tree
and formatted inputs, including an 80 KiB multi-block input. Both roles stream
in 4 KiB input fragments through the same caller output buffer. The compiled
role constructs the compiled state and stream once, then resets between samples.
The per-frame-staging role constructs the compiled state inside every timed
sample and then streams one frame with it. Per-block parser owners and stream
workspace allocation are included in both roles.

Each role runs two rounds of forty samples after an in-test CPU barrier; safe
and all-feature configurations are each run twice. All four qualification
ledgers are byte-identical: 18 cases, equal frame bytes across roles, one decode
check per case, and 11,520 timed samples. Primary safe
staging/compiled geometric-mean ratio is 2.0064; all-feature configuration
repeats are 2.0633 and 2.0901. A ratio above one means per-frame staging took
longer on these workloads. Construction dominates most small cases; the
multi-block cases still show a gain because staging rebuilds the tables and the
compiled role reuses them. No universal, tree-shape, allocation, native
workspace or large-throughput benefit is claimed. Raw samples and the complete
per-case table are in `evidence/2026-11-03-compiled-streaming-benchmarks.json`.

## Remaining scope

Parameter resets that clear a compiled selection, one-frame prefix replacement,
pledged-source window adjustment, native attach-versus-load workspace reuse,
LDM, workers, shared compiled handles and complete upstream parity remain open.
Native byte/workspace identity is not claimed.
