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

## Module 10: compression parameters and encoder lifecycle

Design: `docs/compression-parameters-design.md` and
`docs/data-structures.md`. Pure reference profile inspection is distinct from
operational numeric compression; the existing named policies remain unchanged.
Module 9 completion and its exact validation boundaries are recorded in
`docs/progress.md`. Scalar and live-context checkpoints can proceed independently
after oracle capture; each completed item receives its own verified commit.

- [x] Analyze requested/resolved value records, exact pinned semantics, live configuration boundaries, and subsequent search/storage milestones.
- [x] Freeze pinned C oracle fixtures for bounds, presets, scalar resolution, mutation failures, and reset outcomes before implementing their Rust counterparts.
- [x] Add allocation-free validated parameter records, source-size and reference-strategy types, and checked mutation boundaries.
- [x] Add attributed preset tables and internal tier/level/acceleration selection without changing current compression policies.
- [x] Implement complete parameter adjustment and pure level/request queries, preserving explicit overrides, source hints, platform bounds, and checked size arithmetic.
- [x] Add atomic supported encoder configuration replacement at the documented pre-header boundary.
- [x] Add parameter-only and full encoder reset, verifying phase, pledge, dictionary/prefix release, retained workspace, and allocator ownership.
- [x] Port source-anchored official scalar assertions through the completed public inspection API.
- [ ] Port matching official encoder lifecycle assertions and independent C checks while preserving default output fingerprints.
- [ ] Update the official source/case inventory with precise scalar and lifecycle coverage, retaining blocked numeric execution and other missing contracts.
- [x] Validate scalar inspection on 32-bit and big-endian interpreted targets, and compile the no_std library for a target without pointer atomics.
- [ ] Run the complete safe/individual/all-feature, no_std, Clippy, formatting and oracle matrix; record precise completion and remaining gaps.

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
