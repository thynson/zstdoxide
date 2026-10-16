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

## Module 18 preparation: bounded reference Ultra

Design and ownership are recorded in `docs/data-structures.md`. Keep strategy 8
unsupported until complete coupled optLevel=2 behavior is independently qualified.
Strategy 9 and its initial discarded pass belong to a subsequent module.
Preserve every Optimal fixture, source request, complete frame and owner baseline.

- [x] Review pinned fractional pricing, coupled policy, cache lifecycle and thorough path relaxation against the recorded design.
- [x] Decompose scalar boundaries and freeze an independent integer fractional-price model before native execution.
- [x] Specify bounded scalar transport and adapter ownership before native helper implementation.
- [x] Review and preflight the exact scalar adapter, full instrumentation and malformed request controls.
- [x] Project independently audited scalar observations into an immutable fixture and reject forged projections.
- [x] Qualify the independent borrowed Rust scalar fixture reader.
- [x] Freeze source-only price recipes, observation protocol and exact normal/instrumented C command inventory.
- [x] Capture and independently audit fractional prices, signed increments and malformed/overflow boundaries.
- [x] Specify policy-dependent checked price records and frequency owner/reset invariants before Rust implementation.
- [x] Implement checked scalar policy/price values and compare all independent/native observations.
- [x] Specify immutable frequency policy, fractional publication bounds and parser composition rejection before implementation.
- [x] Freeze the policy-tagged frequency transport/model and reconstruct every original whole-mode C fixture before execution.
- [x] Freeze source-only frequency lifecycle recipes and exact fractional-boundary negative classifications.
- [x] Review and preflight the complete policy-tagged frequency observer and native command inventory.
- [x] Capture and independently audit complete frequency lifecycles and forged-evidence controls.
- [x] Specify policy-tagged frequency fixture ownership and independent cache/lifecycle checks.
- [x] Project the audited policy-tagged frequency fixture and reject forged reconstructions.
- [x] Qualify the bounded independent Rust frequency fixture reader.
- [x] Implement policy-bound frequency initialization, scaling, updates and caches without changing Optimal baselines.
- [x] Qualify exact borrowed/non-cloning owners, policy mismatch rejection and allocation-failure cleanup.
- [x] Complete the policy-bound frequency feature matrix and full safe/all regression checkpoint.
- [x] Qualify a clock-free complete-state frequency measurement harness and exact owners.
- [x] Measure paired safe/frequency-unsafe/all frequency lifecycles and an independent safe/all repeat after CPU barriers.
- [x] Specify coupled scripted-path ownership, saved-match alternative observations and predecessor invariants.
- [x] Reconstruct the complete original 27-case path fixture with policy-0 successor replay.
- [x] Freeze independent Ultra path replay and source-only branch coverage before observer execution.
- [x] Specify policy-tagged path transport, complete alternative observations and bounded native storage.
- [x] Review and preflight policy-tagged path transport and read-only native instrumentation.
- [ ] Capture complete match-plus-literal alternatives, thorough relaxation, predecessor/repeat and frontier states.
- [ ] Implement bounded private Ultra path composition and compare every intermediate state and sequence.
- [ ] Qualify safe/isolated-unsafe/all behavior, foreign Miri, no-atomics codegen and exact owners.
- [ ] Design and implement allocation-free Ultra execution planning and shared-entropy token publication.
- [ ] Freeze source-aware admission successors without rewriting historical selectors or request inputs.
- [ ] Enable only the independently qualified bounded public Ultra envelope and preserve unsupported/resource distinctions.
- [ ] Capture and independently audit complete bidirectional public interoperability and forged-evidence controls.
- [ ] Run complete safe/isolated-unsafe/all/release, true no_std, formatting, Clippy and usage checks.
- [ ] Freeze complete frames/owners and measure all backends plus an independent safe/all repeat after CPU barriers.
- [ ] Reconcile the bounded Ultra milestone, official migration gaps and the separate Ultra2 lifecycle decomposition.

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
