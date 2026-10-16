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

Public strategy 7 now admits the qualified bounded fresh-block envelope.
Finish full regression, native audit and measurements before milestone closure.
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
- [x] Qualify ordinary parser startup positions and full fresh repeat publication in retained-state capture.
- [x] Specify bounded fresh ordinary startup requests and compose independent search/path replay before C execution.
- [x] Freeze fresh startup recipes, exact CCtx/provider observation hooks and bounded zero-initializing allocator review.
- [x] Specify an exact fresh startup capture command inventory and independent artifact/state audit before C execution.
- [x] Qualify fresh startup plan, dependency/environment gates and malformed controls in normal/optimized Python.
- [x] Correct and preflight sanitizer workspace accounting while preserving the original failed capture.
- [x] Capture fresh ordinary startup/search/path/frame state with unchanged provider selection in normal and sanitized builds.
- [x] Project audited startup frames, path states and complete search deltas into a bounded fixture.
- [x] Validate the independent startup fixture reader and decode each complete frame with safe/all-feature Rust.
- [x] Independently audit fresh ordinary frames and forged state before extending private search query admission.
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
- [x] Capture wider path transitions at the 4096-position horizon before production path integration.
- [x] Capture and independently check bounded optimal path trajectories and malformed predecessor controls.
- [x] Project audited small/wider path states into bounded canonical binary deltas and verify an independent Rust reader.
- [x] Specify and implement finite path records with explicit forward/reconstruction roles.
- [x] Implement initialized allocator-aware path storage with an isolated unsafe feature.
- [x] Replay scripted path decisions in private Rust and compare all captured states.
- [x] Qualify scripted path decisions on every isolated feature, foreign Miri and no-atomics compilation.
- [x] Freeze C-derived path workload outputs and owners before diagnostic backend timing and a separate repeat.
- [x] Add separately bounded private fresh-query admission and compare every captured search map without weakening direct-query guards.
- [x] Integrate private fresh optLevel0 parsing after ordinary startup is qualified.
- [x] Qualify private fresh composition across isolated features, foreign Miri and no-atomics callers before retained production changes.
- [x] Specify bounded retained-block transport, state ownership and frame publication before model or observer implementation.
- [x] Freeze independent retained search/frequency/path predictions and rollback/RLE hypotheses before C execution.
- [x] Review and preflight the retained ordinary CCtx observer and exact dependency/environment command gates.
- [x] Reconcile inter-block frontier adjustment and add strict-boundary/raw-candidate successor predictions after the initial capture audit failed.
- [x] Capture and independently audit retained optimal blocks, complete frames and committed repeat publication.
- [x] Specify retained fixture encoding with persistent deltas and publication owners.
- [x] Project audited retained complete state into a bounded immutable fixture.
- [x] Qualify a checked independent retained Rust fixture reader and complete frame decode.
- [x] Specify retained prefix bounds and transactional publication coordination.
- [x] Implement retained search entry and match every captured query/map/frontier without future-byte access.
- [x] Implement retained path blocks and explicit candidate/committed publication with exact captured states.
- [x] Match private Rust ordinary sequences/state and decode every complete C frame; preserve all predecessor regressions.
- [x] Qualify allocation failures, exact borrowed/non-clone owners, resets and poisoned transactional failures.
- [x] Design additive wider retained sparse observations before broadening ordinary block admission.
- [x] Freeze wider retained transport and independent sparse replay; reconcile every existing small capture.
- [x] Freeze source-only wider branch recipes and preflight the unchanged-provider sparse observer.
- [x] Capture and audit wider ordinary paths, search state, publication and complete frames.
- [x] Specify the wider retained binary fixture and independent horizon/source checks.
- [x] Project audited wider sparse states and complete frames into an immutable fixture.
- [x] Qualify the wider fixture reader and independent complete-frame decoding.
- [x] Extend private retained search admission and match every wider query, complete map and frontier.
- [x] Extend private retained path/coordinator admission and match all wider states and publications.
- [x] Qualify wider retained bounds, future-byte isolation, exact owners and poisoned resets.
- [x] Check complete wide schedule rejection and cleanup at exact block/source limits.
- [x] Design wider complete fresh-block observations separately from retained scheduling.
- [x] Freeze full fresh source transport/model/recipes and reconcile existing startup observations.
- [x] Preflight the complete fresh-block observer and exact native command inventory.
- [x] Capture and independently audit complete wider fresh blocks and frames.
- [x] Specify complete fresh sparse fixture ownership, bounds and validation.
- [x] Project audited complete fresh states and frames into an immutable wider fixture.
- [x] Qualify the complete fresh fixture reader and independent frame decoding.
- [x] Extend private fresh search admission and compare every complete query/map.
- [x] Extend complete fresh path composition and compare all events and sequences.
- [x] Qualify complete fresh allocation owners, failure cleanup and reset contracts.
- [x] Implement a private allocation-free Optimal execution plan for qualified complete fresh sources.
- [x] Implement checked Optimal token collection with exact owners and wire offset intents.
- [x] Integrate the private plan/state with shared frame entropy and independently decode complete frames.
- [x] Freeze source-aware successor mappings and independently reconstruct every historical Optimal tuple.
- [x] Freeze new positive public recipes and independently reconstruct every admitted tuple.
- [x] Qualify explicit three-byte Optimal entropy admission without weakening preceding parser contracts.
- [x] Enable bounded public Optimal dispatch and preserve original selector/numeric assertion inputs.
- [x] Reconcile local admission-category totals with the immutable source-aware successor tables.
- [x] Preflight the exact public interoperability command inventory and reused generic adapters.
- [x] Qualify the additive public audit successor against the complete implemented source inventory.
- [x] Correct the inherited independent admission audit with an additive version and retain its failed predecessor evidence.
- [x] Capture and independently audit bidirectional public interoperability with offline forged-evidence controls.
- [x] Qualify selected public Optimal frames and owners on 32-bit, big-endian and no-atomics targets.
- [x] Register and qualify the committed Optimal offline fixture and admission checks in the standard driver.
- [ ] Run all safe/isolated-unsafe/all/release, true no_std, formatting and Clippy checks.
- [x] Specify complete-frame benchmark records, owner equivalence and measurement barriers.
- [x] Implement and freeze deterministic complete Optimal frame workloads and independent evidence checks.
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
