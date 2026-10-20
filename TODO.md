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

## Module 19: bounded reference Ultra2

Strategy 9 now admits the independently qualified bounded fresh-source envelope.
Its discarded pass, history invalidation and public frame path are qualified;
performance closure remains open. Preserve every earlier
recipe, source-size selector, complete frame and allocator baseline.

- [x] Review pinned Ultra2 eligibility, sequence-store reset, copied repeats and history-coordinate changes against the ownership design.
- [x] Freeze an independent eligibility/coordinate model and reconcile skipped-seeding cases with ordinary Ultra.
- [x] Freeze source-only two-pass recipes covering short-key tables, stale ring aliases, threshold predicates and changed final paths.
- [x] Specify bounded pass-tagged transport and complete frequency/search/path/reset observations before native instrumentation.
- [x] Review and preflight read-only instrumentation, full sanitizer coverage and exact command/source inventories.
- [x] Capture and independently audit both passes, malformed requests and coherent forged lifecycle states.
- [x] Specify and project an immutable two-pass fixture with independent reconstruction controls.
- [x] Qualify the independent borrowed Rust fixture reader and all reset/publication boundaries.
- [x] Design a private checked Ultra2 plan and copied phase/coordinate records before changing parser/search state.
- [x] Implement checked index rebasing and stale-history exclusion while retaining exact search owners.
- [x] Implement the private discarded-pass coordinator with preserved frequencies, original repeats and reset token lengths.
- [x] Qualify complete private two-pass paths, nine owner identities and transactional failures against every captured state.
- [x] Complete safe/isolated-unsafe/all correctness, foreign Miri and no-atomics code generation.
- [x] Freeze additive source-aware admission and public recipes without rewriting any Ultra predecessor input.
- [x] Qualify bounded public Ultra2 dispatch and complete shared-entropy frames before public admission.
- [x] Capture and independently audit bidirectional public interoperability and all predecessor frame identities.
- [x] Complete full regression, true no_std, formatting, Clippy, usage and migration-inventory checks.
- [x] Freeze preselected native-audited complete Ultra2 frames with independent immutable baseline checks.
- [x] Verify complete frame bytes and owner ledgers across every backend without clocks.
- [ ] Measure every backend plus an independent repeat after CPU barriers.
- [ ] Reconcile the bounded Ultra2 milestone and decompose the next roadmap scope without claiming complete upstream parity.

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
