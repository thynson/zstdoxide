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

## Module 15 preparation: bounded reference Lazy and Lazy2

Module 14's immutable full matrix and measurements are the completed predecessor
gate. Preserve its measured codec inputs and permanent frame/owner baselines. Do not enable new public strategies before their own
successor admission mapping and independent oracle checks.

- [x] Review depth-one/two parsing, inclusive lookahead bounds, score arithmetic and shared ownership before implementation.
- [x] Freeze complete Greedy frames and qualified allocator records as permanent regressions from the reviewed clock-free archive.
- [x] Specify bounded direct Lazy/Lazy2 packets, arena bounds and independent observation validation before C execution.
- [x] Implement and review the versioned direct C adapter, completing normal/instrumented syntax checks before execution.
- [x] Add checked Python transport and independent replay with malformed depth/schema controls and no child execution.
- [x] Freeze tiny depth/branch recipes and predecessor controls before C execution.
- [x] Implement the bounded tiny capture controller and independent offline auditor with preflight rejection controls before first C execution.
- [x] Capture normal/instrumented tiny observations and fresh-context salt, then independently reject changed evidence offline.
- [x] Freeze broader recipes covering repeat ties, gain thresholds, successive lookahead, tail equality, skipping, floors and table trajectories.
- [x] Implement and preflight the broader capture controller and offline auditor with exact tiny/predecessor controls before execution.
- [x] Qualify the completed broader capture with offline forgery controls and preserve the original capture archive.
- [x] Reconcile score-family observations and freeze independently checked broader direct fixtures before Rust parsing.
- [x] Register the Greedy baseline and broader Lazy offline recipe, plan and fixture checks in the standard check driver.
- [x] Project direct observations into checked Rust records with complete maps, sequence replay and source provenance.
- [x] Add a dependency-free checked Rust adapter for all frozen direct Lazy inputs and complete observations.
- [x] Add shared copied depth/candidate/score records and explicit inclusive lookahead coordinates, preserving Greedy's strict main-search contract.
- [x] Implement depth-one selection and repeated improvement against frozen observations.
- [x] Implement depth-two selection, repeat bias and exact speculative sampling against frozen observations.
- [x] Specify and freeze ordinary frame/schedule recipes and versioned C observation protocol before execution.
- [x] Implement and review the versioned ordinary C helper with normal/instrumented syntax checks before execution.
- [x] Add independent ordinary Lazy record replay and malformed offline controls.
- [x] Implement and preflight the ordinary capture controller and offline auditor before execution.
- [x] Capture and independently validate ordinary Lazy/Lazy2 trajectories and complete frames with instrumented C.
- [ ] Freeze audited ordinary observations with immutable artifact identities and offline replay.
- [ ] Project ordinary depth-tagged trajectories and complete frames into a bounded binary fixture.
- [ ] Add a checked Rust ordinary fixture adapter with independent replay and predecessor controls.
- [ ] Retain shared lazy-family state across blocks with exact owners, tiny handling and failure poisoning.
- [ ] Integrate private frame dispatch and transactional entropy/repeat publication while retaining every prior complete frame and allocation baseline.
- [ ] Freeze successor admission mappings over all unchanged selector, numeric-loop and historical interoperability inputs.
- [ ] Enable bounded public Lazy/Lazy2 requests and document exact source-dependent level/resource behavior.
- [ ] Port newly admitted original selector and numeric assertions without closing unrelated upstream obligations.
- [ ] Capture and independently validate broader bidirectional public interoperability with normal/instrumented C.
- [ ] Verify exact owner release, allocation failures, malformed state, no_std and interpreted i686/s390x bounds plus no-atomics caller compilation.
- [ ] Run safe, each unsafe feature and all-feature correctness checks with every predecessor regression intact.
- [ ] Freeze and verify clock-free workloads, then compare all backends and repeat safe/all after a CPU barrier.
- [ ] Reconcile the bounded milestone, official-test gaps and measurements; clear completed items and retain the next decomposition.

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
