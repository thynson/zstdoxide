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
- [ ] Verify and pin the actual implementation compiler, document its minimum-version policy, and check the unchanged codec before generic migration.
- [ ] Add a separate counting/failing test allocator with layout, lifetime, distinct-clone identity, zero-size, growth, and non-unwinding contracts.
- [ ] Parameterize complete output history while preserving Global return ownership, overlap behavior, limits, and failed-growth state.
- [ ] Parameterize ring history and verify wrap, reserve-before-reset, window changes, and retained capacity.
- [ ] Parameterize forward/count bit writers with identical bits and reservation-failure guarantees.
- [ ] Parameterize FSE decoding entries and weight scratch, including fallible table copies.
- [ ] Parameterize inverse FSE tables, normalization, descriptions, and channel-selection temporaries.
- [ ] Parameterize Huffman decoding, literal buffers, and paired-description validation storage.
- [ ] Replace the Huffman builder's Global-only priority queue with checked allocator-compatible storage and preserve all tree/tie vectors.
- [ ] Parameterize transactional literal and sequence contexts, including default/current/spare owners and every temporary.
- [ ] Expose allocator-aware decoding contexts and collecting output APIs with unchanged framing and poisoned-context recovery.
- [ ] Parameterize match tables/chains and dictionary seeds with identical safe/unsafe candidate ordering.
- [ ] Expose allocator-aware block/stream encoding and one-shot output without allocator-changing copies.
- [ ] Parameterize dictionary bytes and prepared tables, explicitly documenting any Global shared-handle allocation.
- [ ] Add a caller-owned dictionary contract for contexts that require all codec allocation through the chosen allocator, preserving prefix and reset lifetimes.
- [ ] Replace the training priority queue and parameterize sample windows, frequencies, candidates, ranges, and output without changing selection ties.
- [ ] Parameterize formatted finalization and all validation copies, preserving dictionary bytes, IDs, and C acceptance.
- [ ] Inject failure at each observed codec allocation, including HUF/FSE candidates, history growth, dictionary preparation, reset, and final drop.
- [ ] Adapt the original 89,456,640-byte custom-context source campaign with explicit seeds and supported strategy/API qualifications.
- [ ] Validate standard/custom allocation under safe, each individual unsafe feature, and all features with no_std and independent C checks.
- [ ] Execute focused 32-bit and big-endian tests, compile a no-pointer-atomics target, and distinguish runner limits from passing execution.
- [ ] Compare deterministic output, allocation counts, peak live bytes, retained capacity, and time on identical Global/custom workloads.
- [ ] Record the final compiler/platform evidence, allocator safety review, and remaining static-workspace/estimator/shrinking/1.100 verification gaps.

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
