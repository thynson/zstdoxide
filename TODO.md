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

## Module 28: dictionary-free one-shot in-place decompression

Design and ownership are recorded before implementation in
`docs/in-place-output-design.md` and `docs/data-structures.md`. The input occupies
the end of one exclusively borrowed buffer. Promote only after Module 27 closes;
dictionary/prefix in-place and incremental overlap remain separate contracts.

- [x] Analyze pinned margin/block protection, single-buffer ownership, checked overlap and conservative scope before implementation.
- [x] Implement an allocation-free checked framing/margin scan with structural versus semantic validation boundaries and pinned native comparisons.
- [x] Implement checked resumable slice history and one-buffer raw movement with independent safe/unsafe storage and rejection-before-write invariants.
- [x] Integrate a private frame/block loop using disjoint compressed payload/history borrows, actual raw overlap and unread-input fences.
- [x] Expose suffix-input in-place APIs with exact options, alias/lifetime documentation, malformed bounds and scratch failure qualification.
- [x] Port the original single/two-frame assertions with exact preceding upstream input history and independently qualify normal/sanitized shared-buffer layouts and real guards.
- [x] Qualify actual overlap and split-borrow history on foreign Miri and concrete no-atomics output.
- [ ] Qualify every isolated feature and all features, full/release regression, true no_std and strict Clippy.
- [ ] Freeze identical overlapping workloads and allocation ledgers, retain executables and measure safe/unsafe builds with an independent repeat and explicit source-restoration cost.
- [ ] Reconcile bounded in-place coverage and decompose the next missing contract without claiming complete upstream parity.

## Module 29: persistent dictionaries in one-shot in-place output

Design and ownership precede implementation in `docs/dictionary-in-place-design.md`
and `docs/data-structures.md`. Promote only after Module 28 closes. The single
mutable suffix-input owner remains unchanged; dictionaries are independently
borrowed persistent selections, not overlapping prefix storage.

- [x] Record dictionary/history composition, immutable entropy fallback, independent allocator ownership and native contracts before implementation.
- [ ] Implement checked dictionary-history resumption and private frame selection without handle clones or copied dictionary history.
- [ ] Expose dictionary-backed in-place wrappers with lifetime/alias documentation, exact bounds, frame reset and scratch-failure qualification.
- [ ] Qualify real shared-buffer normal/sanitized native layouts and replay the bounded dictionary corpus with explicit differences.
- [ ] Qualify actual dictionary crossings and overlap under foreign Miri and concrete no-atomics code generation.
- [ ] Qualify safe, every isolated feature and all features, complete/release regression, true no_std and strict Clippy.
- [ ] Compare frozen in-place/direct dictionary workloads, source restoration and independent allocation ledgers across retained backends and independent repeats.
- [ ] Reconcile bounded dictionary in-place coverage and decompose the next missing contract without claiming complete upstream parity.

## Module 30: checked frame metadata and decompressed-size bounds

Ownership and structural-versus-semantic contracts are recorded before code in
`docs/frame-scan-design.md` and `docs/data-structures.md`. Promote only after
Module 29 closes. Reuse the checked margin traversal; do not add a second parser
or infer reusable one-shot prefix lifecycle from streaming semantics.

- [x] Analyze checked frame metadata, borrowed skip payloads, native sentinel/early-return behavior and original assertions before implementation.
- [ ] Refactor the private structural result to retain validated metadata while preserving every existing margin contract.
- [ ] Expose first-frame metadata, exact/unknown concatenated size, checked decompression bound and borrowed skippable queries with allocation/lifetime qualification.
- [ ] Reproduce pinned normal/sanitized structural queries and original bounded assertions, retaining explicit overflow/sentinel/unknown differences.
- [ ] Qualify all deterministic frame corpus queries, foreign width/endian interpretation and concrete no-atomics output.
- [ ] Qualify safe, every isolated feature and all features, complete/release regression, true no_std and strict Clippy.
- [ ] Compare frozen structural workloads across retained safe/unsafe backends and independent repeats after actual CPU barriers.
- [ ] Reconcile bounded query coverage and decompose the next missing contract without claiming complete upstream parity.

## Module 31: compression bounds and skippable frame writing

Design and ownership precede implementation in `docs/frame-utilities-design.md`
and `docs/data-structures.md`. Promote only after Module 30 closes. These scalar
and borrowed-slice utilities add no context, workspace or streaming-bound promise.

- [x] Analyze native bounds, width limits, borrowed serialization and rejection-before-write invariants before implementation.
- [ ] Implement checked scalar compression bounds and skippable serialization with numeric-limit and original bound assertions.
- [ ] Expose documented utilities and qualify encoder-bound scope, output guards, borrowed aliases and structural roundtrips.
- [ ] Qualify normal/sanitized native arithmetic and real writer layouts, retaining original-assertion provenance and error differences.
- [ ] Qualify width/endian arithmetic and real writes under foreign Miri and concrete core-only ARM code generation.
- [ ] Qualify safe, every isolated feature and all features, complete/release regression, true no_std and strict Clippy.
- [ ] Compare deterministic arithmetic and writing workloads across retained safe/unsafe backends and independent repeats after actual CPU barriers.
- [ ] Reconcile bounded utility coverage and decompose the next missing contract without claiming complete upstream parity.

## Module 32: reusable one-shot dictionary and prefix selection

Ownership and call-versus-frame lifecycle are recorded before implementation in
`docs/one-shot-selection-design.md` and `docs/data-structures.md`. Promote only
after Module 31 closes; this coordinator does not retain decoder scratch.

- [x] Analyze native call-level prefix consumption, persistent ownership and transient call borrows before implementation.
- [ ] Implement plain checked selection transitions and call claims without handle clones or copied history.
- [ ] Expose one-shot direct/in-place coordinator methods with exact attempt consumption, lifetime/guard and independent allocator qualification.
- [ ] Qualify persistent/prefix lifecycle schedules using normal/sanitized native contexts and explicit subsequent-call observations.
- [ ] Qualify real ownership transitions and overlapping output under foreign Miri and concrete no-atomics code generation.
- [ ] Qualify safe, every isolated feature and all features, complete/release regression, true no_std and strict Clippy.
- [ ] Compare deterministic coordinator/output workloads with ownership ledgers, restoration/setup accounting and independent repeats after actual CPU barriers.
- [ ] Reconcile bounded one-shot selection coverage and decompose the next missing contract without claiming native workspace or full upstream parity.

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
