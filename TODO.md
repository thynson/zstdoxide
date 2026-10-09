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

Module 10 is complete; its checked work items are archived in Git history and
`docs/progress.md`. The immutable final matrix and remaining migration gaps are
recorded in `docs/compression-parameters-validation.md`.

## Module 11: configurable named match search

Design: `docs/match-search-design.md` and `docs/data-structures.md`.
Operational search settings remain separate from reference profile inspection.
Existing default bytes, ownership and reset semantics are compatibility inputs.

- [x] Record the approved API, ownership/resource formulas, width-aware seed boundaries and independent pre-change default baseline.
- [x] Activate the captured default-frame fingerprints as an offline regression before changing match storage.
- [x] Add validated plain search options, key-layout/shape records and checked requested-storage calculations with target-layout tests.
- [x] Freeze attributed pinned-C hash vectors and reproduction/provenance checks before implementing the new hash primitives.
- [x] Add safe exact-width key extraction and unsalted four-through-eight-byte hashing, preserving current four-byte values and rejecting incomplete keys.
- [x] Make MatchTable dimensions and layout configurable, preserving exact owners and logical state on failure, checked seed copying and equivalent safe/unsafe access.
- [x] Make MatchChain heads/nodes configurable with compatible seed transfer, bounded tags/visits and allocator-failure/resize coverage in both backends.
- [x] Route selected key width and explicit Chain/Lazy effort through parser probes, preserving lookahead, insertion, tie and failure contracts.
- [x] Add exact-width borrowed dictionary/history/input key extraction, with checked coordinates and explicit incomplete tails before storage integration.
- [x] Integrate frame-local storage shape with working tables and dictionary seeds, preserving default insertion order and transactional entropy publication.
- [x] Integrate selected output-history retention, including width-aware short Flush tails and dictionary/history/input crossings.
- [x] Expose meaningful Encoder search configuration and capacity reporting, with existing boundary/reset/allocator contracts and runnable documentation.
- [x] Add public default-compatibility and actual-control-effect tests, pinned-C decoding and representative complete allocation-failure campaigns.
- [x] Port supported live search/reset/dictionary-order assertions from relevant official groups, retaining explicit numeric-level and attachment gaps.
- [x] Define deterministic benchmark profiles, work schedules, ownership accounting and separate harness/measurement checkpoints.
- [x] Add a clock-free validated benchmark harness for identical default/tuned workloads, explicit capacity/allocation accounting and complete backend comparisons.
- [ ] Validate bounds, owners, feature combinations, no_std and target layouts; update relevant official coverage without claiming new reference strategies.
- [ ] Measure the frozen default/tuned benchmark matrix across safe, individual unsafe and all-feature builds; record ratios, sample ranges, allocation and retained-capacity effects.

Numeric profile execution, additional reference algorithms, long-distance matches,
threading, static workspaces and the remaining official migration stay open.

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
