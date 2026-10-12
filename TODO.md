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

## Module 12: bounded reference Fast profile execution

Design: `docs/reference-fast-design.md` and `docs/data-structures.md`.
This module follows the completed Module 11 validation and measurements.
Existing named APIs and default frame bytes remain compatibility inputs.

- [x] Analyze reference Fast ownership, sampling, repeat publication, whole-buffer profile admission and independently review the implementation boundaries.
- [x] Specify separate direct-parser, collector and ordinary-frame oracle contracts and the checked explicit-offset interface.
- [x] Specify shared entropy-owner extraction, reconstruction checks, Fast literal/gain policy and truthful known-size headers.
- [x] Add a bounded direct-C Fast helper with independent sequence replay and reviewed source/literal capacity guards.
- [x] Freeze deterministic direct Fast sequence/state recipe families with attributed normal/sanitizer reproduction and offline integrity checks.
- [x] Adapt frozen direct observations into a checked dependency-free Rust fixture reader for later parser comparisons.
- [x] Corroborate direct candidates through the official sequence collector, retaining its distinct publication, tiny-input and delimiter behavior.
- [x] Capture ordinary Fast frame publication witnesses across compressed/raw/RLE transitions and full independent decoding before Rust Fast implementation.
- [x] Decode every captured complete ordinary Fast C frame in normal Cargo tests with dependency-free fixture access.
- [x] Add a plain checked Fast execution plan using existing known-size request resolution and explicit strategy/resource admission.
- [x] Extend private small hash layouts and separate checked table lookup/store for persistent sampled Fast state, preserving named bounds and both access backends.
- [x] Add checked explicit wire-offset value transitions and planning while preserving canonical named plans.
- [x] Add committed-repeat snapshots and transactional preparation of intent-bearing sequences with complete ownership/failure coverage.
- [x] Implement safe single-block Fast sampling, repeat probes, backward/forward matches, tails and acceleration against frozen observations.
- [x] Derive a dependency-free, hash-bound reader of the frozen ordinary-frame candidate trajectories for persistent-state comparisons.
- [x] Preserve sampled Fast state across blocks with checked whole-source ranges and raw/RLE/compressed transition coverage.
- [x] Extract the parser-independent block entropy owner while preserving named bytes, allocator identities, clone/reserve order and capacities.
- [x] Add an allocation-free known-size Fast frame header and checked raw-frame bound, preserving the named header bytes.
- [x] Add Fast raw-only/minimum-gain literal selection while preserving named output and allocator behavior.
- [x] Add transactional explicit block preparation with reconstruction, snapshot and allocation-failure checks.
- [x] Integrate persistent Fast parsing with selected block emission and truthful frame headers without altering named output.
- [x] Publish allocator-aware level/request one-shot entry points with complete failure-ordinal, ownership and no_std coverage.
- [x] Capture the official negative-level assertion's inherited source using a bounded attributed writer replay, preserving its original C size-equality contract.
- [x] Adapt the frozen negative-level size-equality assertion through both public Rust entry points and decode both original C frames, retaining the live-context gap.
- [x] Reproduce complete admitted Fast frames across Rust backends and independent C encoding/decoding with bounded, pinned request recipes.
- [x] Freeze the original simple-round-trip fuzz selector order, tail boundaries and C capacity observations without consuming unimplemented advanced branches.
- [x] Port admitted simple-round-trip profiles and the original empty/30-byte numeric subsets with explicit unsupported counts and remaining context/capacity gaps.
- [x] Port relevant official numeric assertions and run independent complete C/Rust frame comparisons, retaining explicit algorithm/attachment gaps.
- [x] Add a clock-free-by-default nine-case Fast benchmark with separate allocation accounting, independent corpus fingerprints and reviewed safe/unsafe result validation.
- [x] Validate focused Fast ownership/bounds with strict-provenance i686/s390x Miri and thumbv6m no_std library/caller compilation; preserve qualified platform evidence.
- [x] Run the immutable final full feature/no_std/lint/example matrix and reconcile its source identity with the delivered code.
- [ ] Measure and independently validate the frozen nine-case safe/unsafe Fast benchmark after all task-owned correctness jobs finish.
- [ ] Record the bounded Fast milestone and clear its completed checklist while retaining the next module's decomposition.

## Module 13: bounded reference DoubleFast profile execution

Design: `docs/reference-double-fast-design.md` and `docs/data-structures.md`.
Only preparation is complete. Implementation and C capture wait for Module 12
final validation and measurements. Current Fast/named outputs remain regression
inputs, and historical C observations retain their original admission scope.

- [x] Analyze the two-table ownership, exact source bounds/scheduling, algorithm-specific admission, shared integration and newly admitted official subsets; independently review the design.
- [ ] Freeze complete current Fast frame bytes and exact allocator counts alongside the existing named baselines.
- [x] Specify and independently source-review bounded direct DoubleFast C protocol/arenas and tiny input hypotheses before implementation.
- [ ] Implement the bounded direct DoubleFast C adapter, review its actual allocations against the protocol proof, then capture tiny observations with scoped sanitizers.
- [ ] Freeze directed direct-parser families, both full table maps, low-log eight-byte hashes, provenance and offline rejection controls before Rust parsing.
- [ ] Add the private checked eight-byte long-key layout while preserving current Fast and named layout domains.
- [ ] Add the allocation-free DoubleFast plan, independent table/window caps, target layouts and actual source-tier admission checks.
- [ ] Add checked borrowed block/cursor records and two-table owner construction with exact-owner failure/reset coverage.
- [ ] Implement safe single-block DoubleFast parsing against frozen sequences, literals, repeat intents and both complete table observations.
- [ ] Capture ordinary DoubleFast frame publication trajectories; keep collector corroboration and constructed direct states separately qualified.
- [ ] Add a dependency-free checked Rust adapter for frozen ordinary trajectories and complete C frames.
- [ ] Retain both sampled tables across blocks with separate parsed/emitted state, committed repeats and poison semantics.
- [ ] Extract common plain frame settings and truthful header/bound writing while preserving Fast/named output and allocator behavior.
- [ ] Extract the shared explicit entropy/gain policy while retaining Fast-only raw-literal acceleration and existing allocation order.
- [ ] Add small static search/emission dispatch without allocating inactive algorithm state or duplicating committed entropy.
- [ ] Emit complete DoubleFast frames with window-log-21 metadata, raw/RLE fallback, transactional publication and every observed allocation-failure ordinal.
- [ ] Extend public level/request admission with level0/default3 equivalence, tier-specific levels2/4 and qualified usage documentation.
- [ ] Port newly admitted official numeric/fuzz assertions through a successor policy layer while preserving frozen C tuples and all remaining context/dictionary/capacity gaps.
- [ ] Validate complete bidirectional C/Rust frames, all individual unsafe features, no_std and focused interpreted/compile-only platform boundaries.
- [ ] Freeze and measure deterministic DoubleFast workloads with separate allocation accounting after the CPU barrier; preserve Fast comparisons and qualify results.
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
