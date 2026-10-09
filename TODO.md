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
- [ ] Corroborate direct candidates through the official sequence collector, retaining its distinct publication, tiny-input and delimiter behavior.
- [ ] Capture ordinary Fast frame publication witnesses across compressed/raw/RLE transitions and full independent decoding before Rust Fast implementation.
- [ ] Decode every captured complete ordinary Fast C frame in normal Cargo tests with dependency-free fixture access.
- [x] Add a plain checked Fast execution plan using existing known-size request resolution and explicit strategy/resource admission.
- [x] Extend private small hash layouts and separate checked table lookup/store for persistent sampled Fast state, preserving named bounds and both access backends.
- [x] Add checked explicit wire-offset value transitions and planning while preserving canonical named plans.
- [x] Add committed-repeat snapshots and transactional preparation of intent-bearing sequences with complete ownership/failure coverage.
- [ ] Implement safe single-block Fast sampling, repeat probes, backward/forward matches, tails and acceleration against frozen observations.
- [ ] Preserve sampled Fast state across blocks with checked whole-source ranges and raw/RLE/compressed transition coverage.
- [x] Extract the parser-independent block entropy owner while preserving named bytes, allocator identities, clone/reserve order and capacities.
- [x] Add an allocation-free known-size Fast frame header and checked raw-frame bound, preserving the named header bytes.
- [x] Add Fast raw-only/minimum-gain literal selection while preserving named output and allocator behavior.
- [x] Add transactional explicit block preparation with reconstruction, snapshot and allocation-failure checks.
- [ ] Integrate persistent Fast parsing with selected block emission and truthful frame headers without altering named output.
- [ ] Publish allocator-aware level/request one-shot entry points with complete failure-ordinal, ownership and no_std coverage.
- [ ] Port relevant official numeric assertions and run independent complete C/Rust frame comparisons, retaining explicit algorithm/attachment gaps.
- [ ] Validate feature/platform boundaries and measure a frozen Fast-specific deterministic safe/unsafe benchmark before recording this bounded module complete.

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
