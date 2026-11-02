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


## Module 37: reusable prepared-dictionary compression state

An immutable compiled dictionary owns match-finder seed tables, prepared entropy
and a resolved profile while borrowing dictionary content. Design, ownership and
the reuse geometry are recorded before implementation in
`docs/prepared-dictionary-design.md`, `docs/data-structures.md` and
`docs/prepared-dictionary-analysis.md`.

- [x] Analyze pinned CDict table loading, sampling frontier, loaded-end expiry and by-reference/by-copy ownership before implementation.
- [x] Implement immutable compiled-dictionary ownership with content-relative seeded tables, admission, dictionary identifier and explicit optimal-family rejection.
- [x] Implement translated read-only dictionary-candidate consultation for Fast and DoubleFast with per-call input sampling kept separate and prepared-entropy first-block seeding wired through the compiled path.
- [x] Extend compiled Fast/DoubleFast consultation coverage across multiple blocks and a window-capped dictionary.
- [x] Analyze pinned dictionary chain/row search order, shared attempt budgets and index deltas before implementation.
- [ ] Extend translated consultation to the Greedy/Lazy/Lazy2 chain family.
- [ ] Extend translated consultation to the row family.
- [ ] Extend translated consultation to BinaryTreeLazy2 and retained-plan rejection boundaries.
- [ ] Compare compiled-state frames against the Module 36 combined-staging path per family and record every byte difference explicitly.
- [ ] Expose public compiled-dictionary one-shot and coordinator APIs with guards and allocator-failure qualification.
- [ ] Qualify guarded native schedules, window expiry, unsupported-profile rejection and concrete atomic-free ARM code generation.
- [ ] Qualify safe, every isolated feature/all features, release, no_std and strict Clippy.
- [ ] Compare compiled-state and per-call-staging workloads with independent repeats after CPU barriers.
- [ ] Reconcile bounded compiled-dictionary coverage and decompose the next evidenced contract.

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
