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

## Module 16 preparation: bounded reference BinaryTreeLazy2

Module 15 correctness and measurement closure is complete. Preserve its frozen
complete frame and owner baselines while implementing the reviewed tree design.
Public strategy 6 remains unsupported until its own private and public gates pass.

- [x] Review binary-tree ownership, biased paired slots, unsorted-chain reversal, candidate scoring and frontier semantics before implementation.
- [x] Freeze all eighty Lazy-family frames and qualified allocator records as permanent predecessor regressions.
- [x] Specify bounded direct tree operations, deterministic arena ownership and independent observation replay before C execution.
- [x] Freeze tiny source/operation recipes for unsorted insertion, reversal, truncation, branch ordering, equal-to-end and skipped-frontier cases.
- [x] Implement the checked direct request transport and qualify source/packet mutations in normal and optimized Python.
- [x] Implement and review the direct C helper with normal/instrumented syntax checks and pinned dependency identities.
- [x] Implement independent tree observation replay and malformed offline controls before execution.
- [x] Implement and preflight the direct capture controller and offline auditor before execution.
- [x] Capture tiny normal/instrumented observations and reject forged records offline without child execution.
- [x] Freeze broader source/ring/window/offset-cost input recipes while preserving all thirty tiny packets.
- [x] Preflight a broader controller/auditor with pinned tiny observation controls before execution.
- [x] Capture and independently check complete broader tree trajectories with offline forgery controls.
- [x] Project audited direct records into bounded binary fixtures with exact source and predecessor provenance.
- [x] Add an independent test-only Rust fixture reader with boundary rejection and selected-match replay.
- [x] Register frozen Lazy baseline and tree recipe/projection checks in the standard check driver.
- [x] Add checked tree layout and scalar edge/link records with explicit source and node-floor invariants.
- [x] Implement initialized independent hash/node owners with safe storage and an opt-in unsafe-reference-tree leaf.
- [x] Implement unsorted insertion, bounded chain reversal and deferred sorting with malformed-link and synthetic ordering controls.
- [x] Implement checked binary-tree best-match scoring, equal-tail handling and sampling-frontier advancement; compare every complete operation against frozen C maps.
- [x] Integrate private depth-two parser dispatch with all earlier chain/row trajectories and complete frame baselines unchanged.
- [x] Freeze ordinary tree frame source/schedule recipes and protocol with explicit retained-state observations.
- [x] Implement and review the bounded ordinary tree C helper and independent offline replay with forged-record controls.
- [x] Preflight the capture controller, pinned dependencies and normal/instrumented command inventory before execution.
- [x] Capture and audit ordinary retained-state C tree frames with build/padding equality and offline forgery controls.
- [x] Project audited ordinary tree frames into bounded binary fixtures with exact provenance.
- [ ] Add checked tree trajectory reading and match Rust retained sequences, maps and complete frame bytes before public integration.
- [x] Validate malformed retained tree state, allocation failures and transactional frame/repeat publication.
- [ ] Freeze successor admission mappings and independently resolve every new public tuple before enabling bounded strategy 6.
- [ ] Port newly admitted original assertions and capture bidirectional public C interoperability with forged-evidence controls.
- [ ] Run safe, every unsafe leaf, all-feature, no_std, Clippy, formatting and selected foreign/no-atomics checks.
- [ ] Freeze equivalent all-backend workloads, verify clock-free output/owners and record timing plus a separate safe/all repeat after CPU barriers.
- [ ] Reconcile the bounded milestone and remaining official obligations, retaining the next decomposition.

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
