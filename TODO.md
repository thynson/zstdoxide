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

## Module 23: caller-owned one-shot decompression output

Module 22 is complete. The bounded contract and pinned upstream assertions are in `docs/caller-output-design.md`; stable incremental
sessions, dictionaries and in-place decoding remain separate scopes.

- [x] Analyze direct borrowed output, shared frame orchestration, allocator boundaries and matching upstream assertions without claiming implementation.
- [x] Record final ownership, invariants, upstream counterparts and API names in `docs/data-structures.md` before implementation.
- [x] Implement checked borrowed output history and its independent opt-in unsafe backend, including overlap and rejection-before-write tests.
- [x] Share the one-shot frame loop without changing existing owned-output, allocator or error-precedence contracts.
- [x] Expose caller-owned output APIs with exact destination/options bounds, meaningful malformed-input coverage and scratch allocation failure checks.
- [x] Qualify pinned native caller destinations and port the matching upstream assertions and complete destination-too-small corpus adapter.
- [x] Qualify direct output under every isolated storage feature, true no_std and strict Clippy, plus safe/all full regression and release destination tests.
- [ ] Complete whole-suite regression under every isolated storage feature and all-feature release.
- [x] Qualify borrowed compressed history under foreign Miri and concrete no-atomics code generation.
- [ ] Freeze deterministic direct/owned output workloads, compare allocator ledgers and measure safe/unsafe backends with retained executables.
- [ ] Reconcile this bounded milestone and decompose stable borrowed incremental sessions without claiming complete upstream parity.

## Module 24: stable borrowed incremental decompression output

Design and ownership are recorded before implementation in
`docs/stable-output-design.md` and `docs/data-structures.md`. Develop the
candidate separately while Module 23 completes qualification; promote it only
after that milestone closes. Dictionary-backed sessions remain separate.

- [x] Define the borrowed session API, state ownership, precise reset retention, fixed-capacity error semantics and pinned upstream stable-buffer assertions.
- [ ] Extend checked slice history with session prefix/reset operations and verify unchanged one-shot behavior.
- [ ] Implement bounded fragmented input orchestration with direct raw/RLE/compressed output, frame boundaries, poisoning and reset.
- [ ] Expose the stable borrowed session API with lifetime examples, exact progress/limit tests and allocation failure qualification.
- [ ] Port and independently qualify matching pinned native stable-output assertions, including malformed inputs and real guard checks.
- [ ] Preserve the upstream tail-selected stable streaming fuzz branch and replay its entire pinned external corpus with real fixed destinations and guards.
- [ ] Qualify safe/isolated/all features, true no_std, strict Clippy, full/release regression, foreign Miri and concrete no-atomics output.
- [ ] Freeze identical fragmentation workloads, allocation ledgers and safe/unsafe timing with retained executables and an independent repeat.
- [ ] Reconcile bounded session coverage, update usage/progress and decompose dictionary-backed caller-owned output without claiming complete parity.

## Module 25: dictionary-backed caller-owned one-shot output

Design is recorded before candidate implementation in
`docs/dictionary-output-design.md` and `docs/data-structures.md`. Keep predecessor
validation sources frozen; promote this candidate only after Module 24 closes.

- [x] Define dictionary/output ownership, prepared entropy fallback, ID/error contracts and crossing-match invariants before implementation.
- [ ] Implement checked dictionary-aware slice matches with safe and independent unsafe combinations, overlap/window boundaries and rejection-before-write checks.
- [ ] Extend the shared one-shot frame helper with borrowed dictionary metadata and entropy without changing no-dictionary behavior.
- [ ] Expose dictionary destination APIs with borrowed/nonclone allocators, exact bounds, malformed-input and allocation-failure coverage.
- [ ] Independently qualify pinned native raw/prepared fixed destinations and map matching official dictionary assertions conservatively.
- [ ] Extend the exact dictionary fuzz adapter's persistent branches to real direct output and replay the entire pinned corpus with explicit prefix/training qualifications.
- [ ] Qualify safe/isolated/all full and release regression, true no_std, strict Clippy, foreign Miri and concrete no-atomics output.
- [ ] Compare identical direct/owned dictionary workloads and allocation ledgers with retained safe/unsafe executables and independent repeat timings.
- [ ] Reconcile the bounded milestone and decompose dictionary-backed stable sessions without claiming complete upstream parity.

## Module 26: persistent dictionaries in stable borrowed output

Design and ownership are recorded before implementation in
`docs/stable-dictionary-output-design.md` and `docs/data-structures.md`. Develop
a separate candidate; promote only after Module 25 closes.

- [x] Define generic attachment ownership, reset/lifetime behavior and separate dictionary/scratch capacity reporting.
- [ ] Implement the ephemeral borrowed dictionary sequence sink with safe, independent and combined storage checks.
- [ ] Integrate private per-frame dictionary IDs, repeat offsets and prepared entropy while preserving dictionary-free sessions.
- [ ] Expose persistent shared/view session constructors with borrowed allocators, lifetime examples and failure/reset qualification.
- [ ] Independently qualify normal/sanitized native stable dictionary destinations and exact guarded boundary cases.
- [ ] Replay the full pinned dictionary corpus as an explicitly labelled fixed-fragment persistent-session supplement, retaining buffered prefix branches.
- [ ] Qualify safe/isolated/all full and release regression, true no_std, strict Clippy, foreign Miri and concrete no-atomics code generation.
- [ ] Compare frozen whole/fragmented workloads and allocation ledgers with retained safe/unsafe binaries and an independent timing repeat.
- [ ] Reconcile bounded session coverage and decompose single-use borrowed prefixes without claiming complete upstream parity.

## Module 27: single-use prefixes in stable borrowed output

Design precedes implementation in `docs/stable-prefix-design.md` and
`docs/data-structures.md`. Keep predecessors frozen and promote only after
Module 26 closes.

- [x] Define one-owner attachment states, exact claim/release timing, reset and transactional configuration boundaries.
- [ ] Implement and independently verify plain attachment ownership transitions without cloning persistent handles.
- [ ] Integrate private prefix claim, frame completion and reset with existing dictionary-free/persistent behavior.
- [ ] Expose shared/view prefix constructors and boundary replacement with lifetime, allocator and malformed-input qualification.
- [ ] Independently qualify pinned normal/sanitized stable-prefix outputs and preserve documented lifecycle differences.
- [ ] Replay all dictionary corpus branches with a labelled fixed-fragment supplement and unchanged tail selectors.
- [ ] Qualify safe/isolated/all full and release regression, no_std, Clippy, foreign Miri and concrete no-atomics code generation.
- [ ] Compare frozen whole/fragmented prefix workloads and ledgers using retained safe/unsafe executables and independent repeats.
- [ ] Reconcile bounded coverage and decompose the next missing caller-output contract without claiming complete parity.

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
