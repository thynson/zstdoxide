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

## Module 13: bounded reference DoubleFast profile execution

Design: `docs/reference-double-fast-design.md` and `docs/data-structures.md`.
The design, oracle protocol and pre-change compatibility baseline are complete.
Module 12 validation and measurements are complete; implementation follows the
remaining decomposition below. Current Fast/named outputs remain regression
inputs, and historical C observations retain their original admission scope.

- [x] Analyze the two-table ownership, exact source bounds/scheduling, algorithm-specific admission, shared integration and newly admitted official subsets; independently review the design.
- [x] Freeze complete current Fast frame bytes and exact allocator counts from the reviewed clock-free capture alongside the existing named baselines.
- [x] Specify and independently source-review bounded direct DoubleFast C protocol/arenas and tiny input hypotheses before implementation.
- [x] Implement the bounded direct DoubleFast C adapter, review its actual allocations against the protocol proof, then capture tiny observations with scoped sanitizers.
- [x] Capture low-log eight-byte C hashes for 64 fixed keys at logs6..9 with aligned/unaligned loads, freeze provenance, and compare Rust safe/feature backends.
- [x] Specify and pin wider DoubleFast recipe families for widths, unequal tables, step thresholds, collisions, repeat transitions, floors and long markers before capture.
- [x] Freeze the wider direct-parser observations, both full table maps, provenance and offline rejection controls before Rust parsing.
- [x] Add the private checked eight-byte long-key layout while preserving current Fast and named layout domains.
- [x] Add the allocation-free DoubleFast plan, independent table/window caps, target layouts and actual source-tier admission checks.
- [x] Add checked borrowed DoubleFast block/cursor records with exact search-tail transitions and separate ordinary/repeat floors.
- [x] Construct independent long/short table owners with exact-owner allocation-failure, move and reset coverage.
- [x] Add a dependency-free checked Rust adapter for the frozen direct-parser inputs, sequences and both table maps.
- [x] Implement safe single-block DoubleFast parsing against frozen sequences, literals, repeat intents and both complete table observations.
- [x] Pin the ordinary DoubleFast frame observation protocol and independent source/schedule recipes before C execution.
- [x] Implement and review the bounded ordinary-frame helper, two-table observations and offline replay validator without changing frozen direct/Fast assets.
- [x] Freeze normal and sanitizer ordinary-frame publication trajectories and complete frames, retaining failed hypotheses and rejection controls; keep optional collector corroboration separate.
- [x] Add a dependency-free checked Rust adapter for frozen ordinary trajectories and complete C frames.
- [x] Retain both sampled tables across blocks with separate parsed/emitted state, committed repeats and poison semantics.
- [x] Extract common plain frame settings and truthful header/bound writing while preserving Fast/named output and allocator behavior.
- [x] Extract the shared explicit entropy/gain policy while retaining Fast-only raw-literal acceleration and existing allocation order.
- [x] Add small static search/emission dispatch without allocating inactive algorithm state or duplicating committed entropy.
- [x] Emit complete DoubleFast frames with window-log-21 metadata, raw/RLE fallback, transactional publication and every observed allocation-failure ordinal.
- [x] Freeze a versioned successor admission policy over historical selector/interop inputs before changing public execution, preserving original classifications and identifying additive C obligations.
- [x] Add an offline-checked successor interop driver over the unchanged bounded protocol before switching live oracle jobs to the new policy.
- [x] Extend public level/request admission with level0/default3 equivalence, tier-specific levels2/4 and qualified usage documentation.
- [x] Port newly admitted official numeric/fuzz assertions through a successor policy layer while preserving frozen C tuples and all remaining context/dictionary/capacity gaps.
- [x] Specify a separate broader whole-frame interop protocol and pin DoubleFast source recipes for window21, unequal tables, scalar controls and newly admitted original test inputs.
- [x] Implement and review the broader bounded C/Rust frame adapters, with syntax, Rust and malformed-packet checks before C execution.
- [x] Capture the frozen broader bidirectional campaign with normal and fully instrumented C codec builds, preserving failures and exact source identities.
- [x] Verify focused ownership, bounds and public frame cases under interpreted i686/s390x, plus no-atomics no_std caller code generation.
- [ ] Run and reconcile the complete correctness matrix with each unsafe feature and no_std configuration.
- [x] Freeze deterministic DoubleFast benchmark recipes and validate a clock-free harness with separate allocation accounting and unchanged Fast comparisons.
- [ ] Measure the frozen benchmark across safe/individual/all backends after the CPU barrier, independently replay evidence and qualify results.
- [ ] Reconcile inventory and validation evidence, record the bounded milestone, and clear its completed checklist.

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
