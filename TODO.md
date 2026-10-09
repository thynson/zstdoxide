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
- [x] Add unsafe-huffman-codebook with bounds, missing-symbol, and Miri checks.
- [x] Add an independent bounded C oracle for exact Huffman tables and streams.
- [x] Encode one Huffman stream and verify exact C streams, including dictionary log twelve.
- [x] Encode four streams and checked jump tables, including minimum and size boundaries.
- [x] Build sparse literal tables while preserving formatted dictionary-training output.
- [x] Serialize checked raw, RLE, and Huffman literal headers at every field boundary.
- [x] Select complete raw, RLE, new-Huffman, and reused-Huffman literal sections by serialized cost.
- [x] Assemble zero-sequence compressed blocks without sequence modes or states.
- [x] Publish Huffman choices transactionally with emitted and pending entropy state.
- [x] Verify public literal-only frames and literal size boundaries with independent C decoding.
- [x] Verify treeless lifetime, dictionary replacement, and fragmented output with C.
- [x] Map verified Huffman/FSE encoder contracts and decompose remaining entropy fuzz assertions.
- [x] Pin the official Huffman corpus independently and verify archive metadata without claiming replay.
- [x] Preserve exact Huffman tail selectors with independent C parameter vectors for both BMI layouts.
- [x] Admit weight twelve in standalone Huffman descriptions while preserving existing serialized bytes.
- [x] Build standalone Huffman trees with explicit requested depths through twelve and meaningful optimal-depth selection.
- [x] Encode standalone Huffman sources through 256 KiB with checked four-stream jump limits.
- [x] Implement caller-owned standalone entropy output bounds and zero-result contracts.
- [x] Adapt the complete Huffman target control flow with exact selectors, depth adjustment, and explicit branch outcomes.
- [x] Replay the complete pinned Huffman corpus through its exact target driver and record every branch outcome.
- [x] Generalize inverse FSE construction with absent-symbol rejection and explicit RLE.
- [x] Port exact unit-count normalization and sparse normalized-header regressions.
- [x] Add unsafe-fse-encoder independently from the decoder unsafe-fse backend.
- [x] Plan repeat-offset values forward with exact triples and checked arithmetic.
- [x] Encode sequences using supplied predefined, new, reused, and RLE tables.
- [x] Select adaptive per-channel distributions with deterministic complete cost comparisons.
- [x] Own pending sequence-table replacements and repeat triples in a resettable transactional context.
- [x] Publish FSE and repeat updates only for accepted blocks; verify dictionary/reset lifetimes.
- [x] Verify adaptive sequence modes, repeat-offset lifetime, and dictionary/reset boundaries through public APIs and C decoding.
- [x] Add bounded multi-candidate storage with checked stale-link and tag validation.
- [x] Add unsafe-match-chain with equivalent candidate order and search limits.
- [x] Add the bounded multi-candidate parser with deterministic ties and source-independent resolution.
- [x] Integrate bounded chain matching with dictionary/history eligibility and documented budgets.
- [x] Implement one-byte lazy lookahead with ratio and source-boundary witnesses.
- [x] Expose Lazy through the shared dictionary/history path and verify public ratio/lifetime contracts.
- [x] Port the original raw-block dictionary entropy lifetime schedule at zstreamtest.c:1831.
- [x] Include the exact dictionary lifetime fixture and its oracle target in reproducible audits.
- [x] Port the original six-byte flush and dictionary offset schedule at zstreamtest.c:1884.
- [x] Port the incremental upstream sequence source generator with independent byte and digest vectors.
- [ ] Run the original 971-case dictionary sequence sweep through all named strategies and independent C decoding.
- [ ] Expose completed named strategies and port supported upstream assertions and corpora.
- [x] Extend the regression content adapter to every named strategy with reset, output-bound, and full-byte checks.
- [x] Replay every pinned regression input with each named strategy under safe and all-feature builds.
- [x] Map complete Huffman and named-strategy corpus evidence while preserving upstream API qualifications.
- [x] Extend deterministic benchmarks to every independent entropy/chain backend and each named strategy.
- [x] Include all new entropy and strategy oracle entry points in the reproducible validation script.
- [ ] Run the backend/strategy benchmark matrix and preserve comparable output fingerprints and memory measurements.
- [ ] Run final feature, no_std, Miri, interoperability, and migration checks; record remaining upstream gaps.

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
