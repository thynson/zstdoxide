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

## Module 8: entropy encoding and bounded compression strategies

Design and source counterparts: `docs/entropy-encoder-design.md` and
`docs/data-structures.md`. Each checked item belongs to its own verified commit.

- [x] Record entropy encoder ownership, wire boundaries, safety contracts, and ordered checkpoints.
- [x] Extract the reusable forward bit writer with byte-identical baseline output.
- [x] Add the checked canonical codebook and exact serialized-description pairing.
- [ ] Add unsafe-huffman-codebook with bounds, missing-symbol, and Miri checks.
- [ ] Encode one Huffman stream and verify exact C streams, including dictionary log twelve.
- [ ] Encode four streams and checked jump tables, including minimum and size boundaries.
- [ ] Build sparse literal tables while preserving formatted dictionary-training output.
- [ ] Add checked literal headers and zero-sequence compressed-block assembly.
- [ ] Publish Huffman choices transactionally with emitted and pending entropy state.
- [x] Generalize inverse FSE construction with absent-symbol rejection and explicit RLE.
- [x] Port exact unit-count normalization and sparse normalized-header regressions.
- [ ] Add unsafe-fse-encoder independently from the decoder unsafe-fse backend.
- [x] Plan repeat-offset values forward with exact triples and checked arithmetic.
- [ ] Encode sequences using supplied predefined, new, reused, and RLE tables.
- [ ] Select adaptive per-channel distributions with deterministic complete cost comparisons.
- [ ] Publish FSE and repeat updates only for accepted blocks; verify dictionary/reset lifetimes.
- [ ] Add bounded multi-candidate storage with checked stale-link and tag validation.
- [ ] Add unsafe-match-chain with equivalent candidate order and search limits.
- [ ] Implement bounded multi-candidate greedy matching with documented budgets.
- [ ] Implement one-byte lazy lookahead with ratio and source-boundary witnesses.
- [ ] Expose completed named strategies and port supported upstream assertions and corpora.
- [ ] Extend backend/strategy benchmarks and validation; record remaining upstream gaps.

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
