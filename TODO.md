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

## Module 17 preparation: bounded reference Optimal

Keep strategy 7 unsupported while qualifying the independent optimal parser.
Preserve the completed BinaryTreeLazy2 frames, owners and every earlier baseline.

- [x] Review optimal ownership, frequency lifecycle, sorted-tree eligibility, short-key frontier, finite prices and path reconstruction before implementation.
- [x] Freeze all twenty-four BinaryTreeLazy2 frames and qualified allocator records as permanent predecessor regressions.
- [x] Specify and validate bounded frequency/price requests, independent observation replay and malformed transport controls.
- [x] Freeze source-only initialization, predefined/dynamic, scaling, symbol and arithmetic-boundary recipes.
- [x] Implement independent complete frequency/price replay with lifecycle and forged-observation controls.
- [x] Review and preflight the pinned normal/instrumented price helper and complete source closure before execution.
- [x] Capture complete frequency/price transitions and reject forged records offline in normal and optimized Python.
- [x] Project audited price records into bounded fixtures and add an independent checked Rust reader.
- [x] Implement checked allocation-free scalar prices and compare every captured query before frequency ownership integration.
- [x] Specify contiguous frequency ownership, checked phase transitions and isolated safe/unsafe access before implementation.
- [x] Implement explicitly initialized frequency ownership and lifecycle updates with an isolated safe/unsafe contract.
- [x] Merge the isolated private frequency branch after Module 16 matrix and measurement closure.
- [x] Specify tiny sorted-tree/all-match transport and freeze source-only repeat, short-key and frontier hypotheses with malformed-input controls.
- [x] Extend direct match requests to ordinary CCtx short-key geometry, the strict 2^18 distance boundary and fresh repeat values at pointer-safe positions.
- [ ] Qualify ordinary parser startup positions and full fresh repeat publication in retained-state capture.
- [x] Validate chunked independent suffix replay and preflight the broader guarded helper before C execution.
- [x] Implement independent tiny sorted-tree/all-match replay with exact maps, both frontiers and forged-record controls before C execution.
- [x] Review the guarded direct sorted-tree/all-match helper and preflight pinned dependencies and bounded capture before C execution.
- [x] Capture tiny direct operations and qualify exact all-match maps, short-key tails and forged evidence.
- [x] Project audited tiny match observations into bounded fixtures and add an independent Rust reader.
- [x] Capture broader ordinary short-key geometry, distance and repeat operations before production sorted search.
- [x] Project broader complete maps into canonical sparse fixtures and verify an independent Rust reader.
- [x] Specify checked sorted-search records, immutable source lifetime and independent storage contracts.
- [x] Implement allocation-free sorted-search geometry, request and candidate records.
- [x] Implement optional short-key heads with initialized fallible ownership and an isolated unsafe feature.
- [x] Implement bounded candidate storage with initialized fallible ownership and an isolated unsafe feature.
- [x] Implement checked sorted insertion and bounded match enumeration over immutable source.
- [x] Compare every tiny and broader captured candidate, complete map and both frontiers across search backends.
- [x] Qualify private search on every isolated feature and complete predecessor suites.
- [x] Qualify private search ownership and malformed-state handling with foreign strict-provenance Miri.
- [x] Compile private search libraries and concrete allocator callers for a no-atomics target.
- [x] Compare deterministic private search outputs and allocator records before safe/unsafe diagnostic timing and a separate repeat.
- [x] Specify path transition, sufficient-length, tie, early-abort and backtracking observations before parser implementation.
- [x] Define bounded scripted-candidate path transport and validate source/offset/event capacities.
- [x] Implement independent path replay and prove source-only recipe branch coverage before C execution.
- [x] Review unique-anchor observer substitutions and preflight the pinned normal/instrumented helper.
- [x] Capture bounded path transitions and reject forged phase/predecessor/frequency evidence offline.
- [x] Specify canonical complete-state deltas for wider scripted path observations.
- [x] Validate wider path requests/replay against every small trace and prove horizon branch coverage.
- [x] Preflight the wider path adapter and exact bounded capture plan.
- [ ] Capture wider path transitions at the 4096-position horizon before production path integration.
- [ ] Capture and independently check bounded optimal path trajectories and malformed predecessor controls.
- [ ] Project audited small/wider path states into bounded canonical binary deltas and verify an independent Rust reader.
- [ ] Specify and implement finite path records with explicit forward/reconstruction roles.
- [ ] Implement initialized allocator-aware path storage with an isolated unsafe feature.
- [ ] Replay scripted path decisions in private Rust and compare all captured states.
- [ ] Integrate private optLevel0 parsing after ordinary startup/search history is qualified.
- [ ] Freeze and capture ordinary retained optimal blocks with full search/frequency/path and repeat publication state.
- [ ] Match private Rust ordinary sequences/state and decode every complete C frame; preserve all predecessor regressions.
- [ ] Qualify allocation failures, exact borrowed/non-clone owners, resets and poisoned transactional failures.
- [ ] Freeze successor public admission mappings and independently reconstruct every newly admitted tuple.
- [ ] Enable bounded public Optimal dispatch and preserve original selector/numeric assertion inputs.
- [ ] Capture and independently audit bidirectional public interoperability with offline forged-evidence controls.
- [ ] Run all safe/isolated-unsafe/all/release, true no_std, formatting, Clippy and selected foreign/no-atomics checks.
- [ ] Freeze equivalent complete frames/owners, then measure all backends and a separate safe/all repeat after CPU barriers.
- [ ] Reconcile the bounded milestone, outstanding official migration obligations and the next decomposition.

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
