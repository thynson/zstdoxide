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

## Module 9: allocator ownership and platform validation

Design and source contracts: `docs/allocator-design.md`,
`docs/data-structures.md`, and `docs/upstream-allocation-plan.md`.
The Rust 1.100 distribution probe returned HTTP 404; actual 1.101 nightly
compiler evidence is recorded separately. Do not claim a tested 1.100 build.

- [x] Record every allocation owner, standard API boundaries, failure invariants, and exact upstream allocator/workspace test scope.
- [x] Verify and pin the actual implementation compiler, document its minimum-version policy, and check the unchanged codec before generic migration.
- [x] Add a separate counting/failing test allocator with layout, lifetime, distinct-clone identity, zero-size, growth, and non-unwinding contracts.
- [x] Parameterize complete output history while preserving Global return ownership, overlap behavior, limits, and failed-growth state.
- [x] Parameterize ring history and verify wrap, reserve-before-reset, window changes, and retained capacity.
- [x] Parameterize forward/count bit writers with identical bits and reservation-failure guarantees.
- [x] Parameterize FSE decoding entries and weight scratch, including fallible table copies.
- [x] Parameterize inverse FSE tables, normalization, descriptions, and channel-selection temporaries.
- [x] Parameterize Huffman decoding, literal buffers, and paired-description validation storage.
- [x] Replace the Huffman builder's Global-only priority queue with checked allocator-compatible storage and preserve all tree/tie vectors.
- [x] Parameterize transactional literal contexts and codebook streams, preserving selected descriptions and failure publication.
- [x] Parameterize supplied-table sequence bit emission with independent table and writer allocation.
- [x] Parameterize transactional sequence contexts, including default/current/spare owners and every temporary.
- [x] Add allocator-independent validated FSE/Huffman table views without duplicating checked access algorithms.
- [x] Parameterize decoding sequence state with borrowed dictionary-table fallback and explicit reset after partial failures.
- [x] Expose allocator-aware whole-buffer decoding with original output ownership and unchanged frame limits.
- [x] Expose allocator-aware streaming decoding contexts with borrowed/shared dictionary attachments and poisoned-context recovery.
- [x] Add allocator-aware dictionary decoding collection with independent output ownership and complete scratch cleanup.
- [x] Parameterize greedy match tables and seed copies with allocator-independent source storage and identical safe/unsafe candidates.
- [x] Parameterize bounded match chains, candidate borrows, and dictionary seeds with identical safe/unsafe ordering and failure cleanup.
- [x] Parameterize chain/lazy parser table and output storage while preserving resolver and ordering contracts.
- [x] Expose allocator-aware block encoding with dictionary handles and transactional entropy publication.
- [x] Expose allocator-aware stream encoding with independent dictionary handles and unchanged reset/directive contracts.
- [x] Add allocator-aware one-shot compression with original output ownership and no allocator-changing copies.
- [x] Parameterize dictionary bytes and prepared tables, explicitly documenting any Global shared-handle allocation.
- [x] Define allocation-free borrowed dictionary views and a sealed immutable handle contract.
- [x] Add a caller-owned dictionary contract for contexts that require all codec allocation through the chosen allocator, preserving prefix and reset lifetimes.
- [x] Replace the training priority queue with checked storage and preserve ties and stale-score ordering.
- [x] Parameterize raw-training sample windows, frequencies, candidates, ranges, and output without changing selected bytes.
- [x] Parameterize formatted finalization and all validation copies, preserving dictionary bytes, IDs, and C acceptance.
- [x] Inject failure at each observed codec allocation, including HUF/FSE candidates, history growth, dictionary preparation, reset, and final drop.
- [x] Adapt the original 89,456,640-byte custom-context source campaign with explicit seeds and supported strategy/API qualifications.
- [x] Validate standard/custom allocation under safe, each individual unsafe feature, and all features with no_std and independent C checks.
- [x] Execute focused 32-bit and big-endian tests, compile a no-pointer-atomics target, and distinguish runner limits from passing execution.
- [x] Recheck final encoder/trainer public APIs with exact 32-bit/big-endian Miri selectors and no-atomics caller code generation.
- [x] Build a deterministic allocator benchmark harness and validate every workload and backend without timing.
- [ ] Compare deterministic output, allocation counts, peak live bytes, retained capacity, and time on identical Global/custom workloads.
- [x] Record the final compiler/platform evidence, allocator safety review, and remaining static-workspace/estimator/shrinking/1.100 verification gaps.

## Module 10: compression parameters and encoder lifecycle

Design: `docs/compression-parameters-design.md` and
`docs/data-structures.md`. Pure reference profile inspection is distinct from
operational numeric compression; the existing named policies remain unchanged.
Implementation starts after Module 9 validation and measurement finish.

- [x] Analyze requested/resolved value records, exact pinned semantics, live configuration boundaries, and subsequent search/storage milestones.
- [ ] Freeze pinned C oracle fixtures for bounds, presets, scalar resolution, mutation failures, and reset outcomes before implementing their Rust counterparts.
- [ ] Add allocation-free validated parameter records, source-size and reference-strategy types, and checked mutation boundaries.
- [ ] Add attributed preset tables and internal tier/level/acceleration selection without changing current compression policies.
- [ ] Implement complete parameter adjustment and pure level/request queries, preserving explicit overrides, source hints, platform bounds, and checked size arithmetic.
- [ ] Add atomic supported encoder configuration replacement at the documented pre-header boundary.
- [ ] Add parameter-only and full encoder reset, verifying phase, pledge, dictionary/prefix release, retained workspace, and allocator ownership.
- [ ] Port matching official scalar/lifecycle assertions, preserve default output fingerprints, and update the inventory without promoting blocked numeric execution.
- [ ] Run the safe/individual/all-feature, no_std, Clippy, formatting, oracle, and appropriate width checks; record precise completion and remaining gaps.

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
