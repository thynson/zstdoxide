# Streaming selection lifecycle validation

This report covers Module 39: the streaming selection lifecycle and frame-flag
parameters on the compiled streaming coordinator. Design and ownership
invariants are in [streaming-selection-design.md](streaming-selection-design.md);
the pinned upstream analysis is in
[streaming-selection-analysis.md](streaming-selection-analysis.md); usage is in
[prepared-dictionary-usage.md](prepared-dictionary-usage.md).

## What was implemented

`CompiledStreamEncoder<'a, A, B>` now owns a selection instead of a single
compiled state:

```text
compiled:       Option<CompiledDictionary<'a, B>>   // selected compiled dictionary
prefix:         Option<DictionaryView<'a>>          // pending, unclaimed prefix
active_prefix:  Option<DictionaryView<'a>>          // claimed at frame initialization
```

The invariant keeps `compiled` and `prefix` mutually exclusive. The seven-field
profile, window and block geometry stay fixed from construction: every compiled
frame revalidates the supported window range and the loaded suffix bounds at
frame initialization instead of re-resolving parameters, and a replacement
compiled state must carry exactly the construction profile or it is rejected
with `InvalidParameter` before any mutation.

- `set_compiled` corresponds to `ZSTD_CCtx_refCDict`: clean-boundary only, it
  clears a pending prefix and returns the previously selected compiled state.
- `set_prefix` corresponds to `ZSTD_CCtx_refPrefix_advanced`: clean-boundary
  only, it clears the compiled selection and returns it intact; an empty
  prefix detaches the selection like the upstream empty reference call.
- Frame initialization claims a pending prefix exactly once, mirroring the
  upstream single usage of `prefixDict` in `ZSTD_CCtx_init_compressStream2`.
  A raw prefix contributes no identifier and no entropy seed; a formatted
  prefix contributes both through the same `start_frame` path as the compiled
  seed. A completed frame releases the claimed prefix and leaves no selection.
- Prefix blocks stage the last `min(prefix.len(), window)` bytes ahead of the
  retained history and seed the per-call tables over the staged suffix plus
  history through `ReferenceSearchState::new_with_prefix`; no-selection blocks
  stage only the history. Compiled tables are still never reseeded.
- `reset` stays session-only and keeps the compiled selection and an unclaimed
  prefix. `reset_parameters` corresponds to `ZSTD_reset_parameters`: at a clean
  boundary or after a completed frame it restores default frame options and
  removes the selection, returning the compiled value; an active, finishing or
  failed session is refused with `InvalidState` without mutation.
  `reset_with_options` replaces frame flags over an intact selection.
  `reset_all` corresponds to `ZSTD_reset_session_and_parameters` and accepts
  every phase including a poisoned one. No table is ever reseeded by or after
  a selection change.

Replacing a compiled selection moves it by value back to the caller, so no
table, entropy view or content borrow is cloned on any path and no expensive
state is silently dropped. Frames after selection removal continue
dictionary-free under the fixed profile, which is the upstream post-prefix
state. `dictionary()` was replaced by `selection() -> Option<DictionaryView>`,
which reports the compiled view, the pending prefix, the active prefix, or
`None`.

## Incremental and public coverage

- Twenty focused unit tests cover replacement with intact compiled state,
  detach by empty prefix, claim-once semantics across session and flag resets,
  boundary refusals for every non-clean phase, profile-mismatch rejection,
  prefix frames across all six admitted families including window wrap,
  formatted-prefix identifier and entropy seeding, single-block body equality
  with the one-shot parameterized dictionary frame, parameter-reset boundaries
  and restored defaults, reset-all recovery from a poisoned session with a
  reusable returned state, and allocator-failure release across selection
  transitions.
- Eight public integration tests cover replacement sequences with cross-role
  frame reuse, compiled reselection with profile enforcement, formatted prefix
  frames, prefix body equality with the one-shot parameterized dictionary
  frame, pending-prefix survival across session and flag resets, oversized
  prefix staging caps, detach continuation, and parameter reset with state
  reuse.
- The ignored guarded native schedule exports Rust frames and replays native
  frames through the pinned decompressor.

## Guarded native selection lifecycle schedules

Twelve typed schedules run identical lifecycle sequences against normal and
sanitized pinned v1.5.7 `ZSTD_compressStream2` controls, tracking each frame's
attachment on both sides:

| Schedule | Level | Sequence | Frames | Result |
| --- | --- | --- | --- | --- |
| `select-cdict-then-prefix-raw` | 1 | CDict, frame, prefix frame, dictionary-free frame | 3 | supported |
| `select-cdict-then-prefix-prepared` | 4 | same with a formatted dictionary | 3 | supported |
| `select-prefix-then-cdict-tree` | 9 | prefix frame, then CDict reselection | 2 | supported |
| `select-prefix-twice-then-cdict` | 4 | two single-use prefix frames, then CDict reuse | 4 | supported |
| `select-empty-prefix-detaches` | 1 | prefix frame, empty-prefix detach, dictionary-free frame | 2 | supported |
| `select-parameter-reset-dict-free` | 4 | parameter-only reset to a dictionary-free frame | 2 | supported |
| `select-session-reset-reuses-cdict` | 3 | session-only reset retaining the CDict | 2 | supported |
| `select-prefix-error-midstream` | 1 | mid-frame prefix reference fails, frame completes | 2 | supported |
| `select-parameter-error-midstream` | 4 | mid-frame parameter reset fails, frame completes | 2 | supported |
| `select-session-and-parameters-reset` | 4 | session-and-parameters reset | 2 | supported |
| `select-optimal-prefix` | 13 | Optimal prefix control | 1 | native-only; Rust construction rejected `UnsupportedFeature` |
| `select-ultra2-prefix` | 17 | Ultra2 prefix control | 1 | native-only; Rust construction rejected `UnsupportedFeature` |

The mid-frame `p` and `q` ops assert the upstream `stage_wrong` behavior: the
native `ZSTD_CCtx_refPrefix_advanced` and `ZSTD_CCtx_reset(parameters)` calls
fail while a frame is in progress, exactly where the Rust coordinator returns
`InvalidState`, and the interrupted frame still completes with its original
selection. Caller output windows stay between one and eleven bytes on both
sides. The sanitized and normal oracles produce identical rows and frame files
under scoped ASan/UBSan. Every Rust frame decodes natively and every native
frame decodes in Rust with the attachment that produced it, including the
native-only Optimal/Ultra2 prefix controls whose frames decode in Rust but
whose Rust construction is rejected. Evidence:
`evidence/2026-11-04-streaming-selection-native.json` and its campaign archive.

## Selection lifecycle timing

Eighteen deterministic case/level pairs cover Fast, DoubleFast, chain, the tree
and formatted inputs, including an 80 KiB multi-block input. Both roles stream
in 4 KiB input fragments through the same caller output buffer and the same
encoder. The session role resets the session and streams one compiled frame per
sample, retaining the compiled selection. The replacement role swaps the
compiled selection for a borrowed one-frame prefix, streams that frame, resets
the session, and restores the compiled state through the returned value per
sample; both roles begin each sample from the same clean boundary. Frames are
not byte-identical across roles because the selections differ; the qualification
ledger records one decode check per role per case and all pass.

Each role runs two rounds of forty samples after an in-test CPU barrier; safe
and all-feature configurations are each run twice, for 11,520 timed samples
with byte-identical ledgers. The primary safe replacement/session geometric-mean
ratio is 1.4078; the independent repeats and the all-feature configuration are
1.4354, 1.4208 and 1.4629. A ratio above one means the per-frame replacement
cycle — selection swap, per-block prefix seeding and the compiled-state restore
— took longer than session reset with retained compiled tables on these
workloads, which is expected: the replacement role additionally seeds the
prefix into per-call tables every block. No universal, tree-shape, allocation or
native workspace claim follows. Raw samples and the complete per-case table are
in `evidence/2026-11-04-streaming-selection-benchmarks.json`.
