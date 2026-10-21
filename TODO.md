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

## Module 21: bounded retained Ultra complete frames

Preserve every existing fresh/Optimal frame. The intended additive strategy-8
scope is 16385..32768 bytes within one resolved window. Retained Ultra2 and the
remaining upstream migration obligations stay separate. Design and ownership
requirements are recorded in `docs/data-structures.md` before implementation.

- [x] Analyze policy-bound retained ownership, source isolation, publication/reset contracts and the one intended predecessor admission transition against pinned upstream.
- [x] Freeze an additive policy-tagged retained transport and independent model; reconcile all policy-0 predecessor observations and reject malformed requests/state in ordinary and optimized Python.
- [x] Freeze source-only policy-2 recipes and witnesses for learned retained prices, alternatives, short keys, horizons, source boundaries and all publication kinds.
- [x] Specify and preflight additive native observer adaptation, full dependency/source inventories, sanitizer commands and bounded capture before execution.
- [x] Capture and independently audit retained policy-2 complete states and native frames, including coherent forgery controls and unchanged predecessor observations.
- [x] Project an immutable complete-state fixture and reconstruct every state byte independently in ordinary/optimized Python, with later-block forgery controls.
- [x] Qualify the independent borrowed Rust fixture reader, all native frame decodes, predecessor states and malformed boundaries before parser admission.
- [x] Introduce a constructor-only retained-Ultra parser scope while preserving old constructor ownership and rejecting every existing parse entry.
- [x] Add the checked policy-bound retained-Ultra plan while preserving copied schedules and early-allocation contracts.
- [x] Admit scoped retained-Ultra parser/coordinator execution without widening old fresh, scripted, Ultra2 or Optimal constructors.
- [ ] Qualify private policy-2 retained transitions against every captured state, future-source isolation, owner failures, poisoning and allocation-preserving reset.
- [ ] Integrate block token collection and shared frame publication with exact nine-owner identities, complete output reservation and raw/RLE rollback.
- [ ] Qualify actual Rust wire history by independent replay and native decoding, without assuming native/Rust entropy decisions agree.
- [ ] Complete all storage-feature correctness, foreign Miri and concrete no-atomics code generation before public admission.
- [ ] Freeze additive public recipes declaring only the exact intended retained-Ultra resource-to-success predecessor transition.
- [ ] Qualify public strategy-8 admission, all remaining predecessor identities and bidirectional interoperability with malformed/allocator controls.
- [ ] Complete full safe/isolated-unsafe/all and release/all regression, all true no_std products, formatting, Clippy, usage, allocator and upstream inventory checks.
- [ ] Bind the final public entry point to foreign Miri and concrete no-atomics artifacts with exact native-to-final source reconciliation.
- [ ] Freeze independently decoded complete-frame benchmark inputs and qualify every backend without clocks and with controller forgery controls.
- [ ] Measure every backend and an independent safe/all repeat after completed CPU barriers, retaining all executables, samples and source identities.
- [ ] Reconcile the bounded milestone, clear its completed checklist and decompose the next scope without claiming complete upstream parity.

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
