# Tasks

## Official test migration: remaining work

Completed one-shot and streaming checkpoints are recorded in `docs/progress.md`
and the validation reports. The complete upstream migration remains open.

### Required for complete migration

- [ ] Port advanced context reset, stable-buffer, and parameter assertions.
- [ ] Port remaining dictionary/training controls, optimizations, and prefix parameter assertions.
- [ ] Port encoder level/parameter, entropy encoder, and sequence-producer tests.
- [ ] Port caller-owned output, in-place, allocator, and workspace contracts.
- [ ] Port long-distance, multithreading, thread-pool, and large-index stress tests.
- [ ] Port the CLI, gzip/zlib, seekable, and file-operation behavior suites.
- [ ] Make the preserved legacy v0.4-v0.7 success assertions pass.
- [ ] Run all external fuzz/regression corpora with their matching API adapters.
- [ ] Satisfy `scripts/audit_upstream_tests.py --require-complete`.

The required items remain open. A complete inventory and passing available
one-shot adaptations do not complete the upstream test migration.

Module 11 is complete within its configurable named-search scope; completed
work items are archived in Git history and `docs/progress.md`. Correctness,
platform evidence and remaining migration gaps are recorded in
`docs/match-search-validation.md`; frozen measurements and their limits are in
`docs/match-search-benchmarks.md`.

Module 12 is complete within its bounded no-dictionary reference Fast scope.
Completed items are archived in Git history and `docs/progress.md`. The complete
correctness/platform evidence and migration gaps are recorded in
`docs/reference-fast-validation.md`; both timing campaigns and their inconclusive
unsafe-speedup comparison are retained in `docs/reference-fast-benchmarks.md`.

Module 13 is complete within its bounded no-dictionary reference DoubleFast
scope. Completed work items are archived in Git history and `docs/progress.md`.
The final matrix, interoperability and platform qualifications are recorded in
`docs/reference-double-fast-validation.md`; all measured backends, raw samples
and the inconclusive unsafe comparison are retained in
`docs/reference-double-fast-benchmarks.md`.

Module 14 is complete within its bounded no-dictionary reference Greedy scope.
Completed work items are archived in Git history and `docs/progress.md`.
The final evidence reconciliation is recorded in
`docs/evidence/2026-10-13-greedy-closure.json`; correctness/platform qualifications
and both measured campaigns are in `docs/reference-greedy-validation.md` and
`docs/reference-greedy-benchmarks.md`. Complete upstream migration remains open.

Module 15 is complete within its bounded no-dictionary reference Lazy/Lazy2
scope. Completed items are archived in Git history and `docs/progress.md`.
The final matrix, interoperability and platform qualifications are in
`docs/reference-lazy-validation.md`; both timing campaigns and their limits are
in `docs/reference-lazy-benchmarks.md`. Complete upstream migration remains open.

Module 16 is complete within its bounded no-dictionary BinaryTreeLazy2 scope.
Completed items are archived in Git history and `docs/progress.md`. Matrix,
interoperability and platform qualifications are in `docs/reference-tree-validation.md`;
both timing campaigns and their opposing aggregate directions are recorded in
`docs/reference-tree-benchmarks.md`. Complete upstream migration remains open.

Module 17 is complete within its bounded no-dictionary fresh Optimal scope.
Completed work is archived in Git history and `docs/progress.md`. Public bounds,
interoperability and platform qualification are recorded in
`docs/reference-optimal-public-qualification.md`; complete-frame measurements
and the independent repeat are in `docs/reference-optimal-frame-benchmark.md`.
Private retained state qualification does not admit public retained Optimal.
The full upstream migration obligations above remain open.

Module 18 is complete within its bounded fresh no-dictionary Ultra scope.
Completed work items are archived in Git history and `docs/progress.md`.
The full regression, interoperability and platform evidence is recorded in
`docs/reference-ultra-public-qualification.md`; complete-frame measurements and
the independent repeat are in `docs/reference-ultra-frame-benchmark.md`.
The final evidence reconciliation is `docs/evidence/2026-10-20-ultra-closure.json`.
Ultra2 was outside that milestone; its progress is tracked in Module 19 below.
Complete upstream migration remains open.

Module 19 is complete within its bounded fresh no-dictionary Ultra2 scope.
Completed work items are archived in Git history and `docs/progress.md`.
Correctness and platform evidence are in
`docs/reference-ultra2-public-qualification.md`; complete-frame measurements and
the independent repeat are in `docs/reference-ultra2-frame-benchmark.md`.
The final reconciliation is `docs/evidence/2026-10-20-ultra2-closure.json`.
Retained public blocks and complete upstream migration remain open.

Module 20 is complete within its bounded retained no-dictionary Optimal scope.
Completed work items are archived in Git history and `docs/progress.md`.
The exact public matrix and platform evidence are in
`docs/reference-retained-optimal-public-qualification.md`; measured complete frames
are in `docs/reference-retained-optimal-frame-benchmark.md`. The final receipt is
`docs/evidence/2026-10-21-optimal-retained-closure.json`.
Retained Ultra/Ultra2 and complete upstream migration remain open.

Module 21 is complete within its bounded retained no-dictionary Ultra scope.
Completed work items are archived in Git history and `docs/progress.md`.
The full public regression, interoperability and platform evidence are in
`docs/reference-ultra-retained-public-qualification.md`; complete-frame
measurements are in `docs/reference-ultra-retained-frame-benchmark.md`.
The final reconciliation is `docs/evidence/2026-10-22-ultra-retained-closure.json`.
Retained Ultra2 and complete upstream migration remain separate open work.

Module 23 is complete within its bounded caller-owned one-shot decompression output scope.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification, measured comparisons and remaining limitations are in
`docs/caller-output-validation.md` and `docs/caller-output-benchmarks.md`.
The final receipt is `docs/evidence/2026-10-26-caller-output-closure.json`.
Complete upstream migration remains open.

Module 24 is complete within its bounded stable borrowed incremental output scope.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification, measured comparisons and remaining limitations are in
`docs/stable-output-validation.md` and `docs/stable-output-benchmarks.md`.
The final receipt is `docs/evidence/2026-10-26-stable-output-closure.json`.
Complete upstream migration remains open.

Module 25 is complete within its bounded dictionary-backed caller-owned one-shot output scope.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification, measured comparisons and remaining limitations are in
`docs/dictionary-output-validation.md` and `docs/dictionary-output-benchmarks.md`.
The final receipt is `docs/evidence/2026-10-26-dictionary-output-closure.json`.
Complete upstream migration remains open.

Module 26 is complete within its bounded persistent dictionaries in stable borrowed output scope.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification, measured comparisons and remaining limitations are in
`docs/stable-dictionary-output-validation.md` and `docs/stable-dictionary-output-benchmarks.md`.
The final receipt is `docs/evidence/2026-10-26-stable-dictionary-output-closure.json`.
Complete upstream migration remains open.

Module 27 is complete within its bounded single-use prefixes in stable borrowed output scope.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification, measured comparisons and remaining limitations are in
`docs/stable-prefix-output-validation.md` and `docs/stable-prefix-output-benchmarks.md`.
The final receipt is `docs/evidence/2026-10-26-stable-prefix-output-closure.json`.
Complete upstream migration remains open.

Module 28 is complete within its bounded dictionary-free one-shot in-place scope.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification, measurements and limitations are in
`docs/in-place-output-validation.md` and `docs/in-place-output-benchmarks.md`.
The final receipt is `docs/evidence/2026-10-27-in-place-output-closure.json`.
Complete upstream migration remains open.

Module 29 is complete within its bounded persistent dictionary in-place scope.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification and measurements are in `docs/dictionary-in-place-validation.md`
and `docs/dictionary-in-place-benchmarks.md`. The final receipt is
`docs/evidence/2026-10-27-dictionary-in-place-closure.json`.
Complete upstream migration remains open.

Module 30 is complete within its bounded structural frame-query scope.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification and measurements are in `docs/frame-query-validation.md`
and `docs/frame-query-benchmarks.md`. The final receipt is
`docs/evidence/2026-10-27-frame-query-closure.json`.
Complete upstream migration remains open.

Module 31 is complete within its bounded scalar-bound and skippable-write scope.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification and measurements are in `docs/frame-utility-validation.md`
and `docs/frame-utility-benchmarks.md`. The final receipt is
`docs/evidence/2026-10-27-frame-utility-closure.json`.
Complete upstream migration remains open.

Module 32 is complete within its bounded call-scoped selection contract.
Completed work items are archived in Git history and `docs/progress.md`.
Qualification and measurements are in `docs/one-shot-selection-validation.md`
and `docs/one-shot-selection-benchmarks.md`. The final receipt is
`docs/evidence/2026-10-27-one-shot-selection-closure.json`.
Complete upstream migration remains open.

Module 33 is complete within its ordinary dictionary-free caller-owned output scope.
Completed items are archived in Git history and `docs/progress.md`; contracts,
measurements and limits are in `docs/slice-encode-validation.md` and
`docs/slice-encode-benchmarks.md`. Final source reconciliation is in
`docs/evidence/2026-10-27-slice-encode-closure.json`. Complete upstream parity remains open.

Module 34 is complete within its persistent dictionary caller-owned compression scope.
Completed items are archived in Git history and `docs/progress.md`; contracts,
measurements and limits are in `docs/dictionary-slice-encode-validation.md` and
`docs/dictionary-slice-encode-benchmarks.md`. Final reconciliation is in
`docs/evidence/2026-10-27-dictionary-slice-encode-closure.json`.
Complete upstream parity remains open.

Module 35 is complete within its call-scoped one-shot selection scope.
Completed items are archived in Git history and `docs/progress.md`; contracts,
measurements and limits are in `docs/one-shot-encode-selection-validation.md`
and `docs/one-shot-encode-benchmarks.md`. Final reconciliation is in
`docs/evidence/2026-10-29-one-shot-encode-closure.json`.
Complete upstream parity remains open.

Module 36 is complete within its bounded combined parameterized dictionary
one-shot scope. Completed items are archived in Git history and
`docs/progress.md`; contracts, measurements and limits are in
`docs/one-shot-encode-parameters-validation.md` and
`docs/one-shot-encode-parameters-benchmarks.md`. Final reconciliation is in
`docs/evidence/2026-11-02-one-shot-encode-parameters-closure.json`.
Complete upstream parity remains open.


## Module 37: reusable prepared-dictionary compression state (complete)

Completed within its bounded one-shot compiled-dictionary scope. An immutable
state owns one resolved profile, content-relative seed tables, optional prepared
entropy and a borrowed dictionary view; Fast, DoubleFast, chain, row and a
fully sorted binary tree are consulted read-only beside per-call sampling.
Caller-owned, allocating and coordinator APIs enforce a fixed seven-field
profile, guards and allocator-failure release. Native/sanitized CDict schedules,
concrete atomic-free ARM emission, the 75-command feature/release matrix and
caller-owned timing are recorded in
`docs/prepared-dictionary-validation.md`,
`docs/prepared-dictionary-benchmarks.md` and the 2026-11-03 receipts. Native
CCtx workspace semantics, LDM, workers, streaming reuse and complete upstream
parity remain open.

## Module 38: compiled dictionary reuse in streaming frames (complete)

Completed within its bounded reference-streaming scope. `CompiledStreamEncoder`
borrows one immutable compiled dictionary and re-binds a reference parser for
every sealed block over `compiled suffix | retained contiguous history | block`.
Fast, DoubleFast, chain and row owners seed only the retained history; the tree
keeps its lazy back-fill. Compiled tables are never reseeded, copied or mutated.
Session reset keeps the selection, `reset_with_options` replaces frame flags,
and a completed frame reinstalls the compiled first-block entropy seed.
Caller-owned `process` follows the existing Continue/Flush/Finish contract;
pledge, directive, poison and allocator-failure guards are qualified. Twelve
guarded native streaming schedules, including a two-frame boundary and
native-only Optimal/Ultra2 controls, and concrete `thumbv6m-none-eabi`
safe/all checks are recorded in
`docs/compiled-streaming-validation.md`,
`docs/compiled-streaming-benchmarks.md` and the 2026-11-03 receipts. The 75
command feature/release matrix reports 125 result groups, 1848 passed and 79
ignored. Native CCtx workspace reuse, prefix replacement, parameter resets that
clear a compiled selection, LDM, workers and complete upstream parity remain
open.

## Module 39: streaming selection lifecycle and frame-flag parameters (complete)

Completed within its bounded streaming selection scope. `CompiledStreamEncoder`
owns exactly one selection: an owned `CompiledDictionary` or a borrowed
one-frame `DictionaryView` prefix, mutually exclusive and moved by value so
compiled tables are never cloned. `set_prefix` and `set_compiled` replace the
selection at a clean boundary and return the displaced compiled state intact;
an empty prefix detaches it; frame initialization claims a pending prefix
exactly once, installs the active selection's entropy seed and identifier, and
releases the claim at frame completion. `set_compiled` requires the
construction profile, and every compiled frame revalidates that fixed profile
instead of re-resolving parameters. `reset_parameters` restores default frame
flags and removes the selection at a clean boundary or completed frame while
returning the compiled value; `reset_all` accepts every phase; frames after
removal continue dictionary-free under the fixed profile; no table is ever
reseeded. Twenty unit tests and eight public integration tests cover
replacement, detach, claim-once, profile enforcement, prefix families and
window wrap, formatted-prefix seeding, body equality with the one-shot
parameterized dictionary frame, reset boundaries and allocator release. Twelve
guarded native lifecycle schedules, including mid-frame stage errors and
native-only Optimal/Ultra2 controls, plus the 79-command matrix and the
11,520-sample timing campaign are recorded in
`docs/streaming-selection-validation.md`, `docs/evidence/2026-11-04-*` and the
2026-11-04 receipts. Prefix-only construction, pledged-source window
adjustment, native attach/load workspace reuse, LDM, workers and complete
upstream parity remain open.

## Module 40: dictionary-free reference streaming frames (in progress)

Analysis and design reviewed before implementation; qualify each item independently.

- [x] Analyze pinned dictionary-free streaming frame initialization, per-frame
  cParams re-resolution from the pledged source size, and window descriptor
  selection without a dictionary.
- [x] Design a streaming coordinator constructed from a compression parameter
  request without compiled tables, with per-frame plan rebuilds from the
  resolved record and a pledged-source window contract.
- [x] Implement dictionary-free streaming frames across the six admitted
  reference families with the existing Continue/Flush/Finish, pledge and poison
  contracts.
- [x] Implement prefix-only construction and pledged window descriptor updates
  that the compiled coordinator deferred.
- [x] Qualify guarded native dictionary-free streaming schedules, including
  pledged-size and window-descriptor interop.
- [x] Qualify atomic-free ARM, every isolated feature/all features, release,
  no_std and strict Clippy.
- [x] Compare dictionary-free streaming workloads after CPU barriers with
  independent repeats against the named-policy encoder.
- [x] Verify automatic request-tier family switches across reused frames against frozen native presets.
- [x] Document request-backed construction, size knowledge, prefix lifetime and reset usage.
- [ ] Reconcile bounded streaming coverage and decompose the next contract.

## Roadmap

Execution order requested on 2026-10-08: finish Module 6, migrate its related
official tests, then implement and validate every following roadmap module in
order. Decompose each module before implementation and preserve small commits.

1. Bounded streaming and reusable contexts.
2. Raw and prepared dictionaries, dictionary IDs, and training.
3. Huffman/adaptive FSE encoding and improved compression strategies.
4. Allocator parameterization and broader platform validation.
5. Advanced compression levels, long-distance matches, threading, and adapters.

Each module must be decomposed before implementation. Completion of an initial
codec milestone does not mean parity with the upstream library or CLI.
