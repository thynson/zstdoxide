# Tasks

## Module 5: official v1.5.7 test migration

- [x] Inventory every official test file, entry point, asset, and API dependency.
- [x] Import all official golden assets with exact hashes and provenance.
- [x] Port golden decoding, error rejection, and adapted compression tests.
- [x] Expose validated FSE count metadata for exact upstream parser assertions.
- [x] Generate pinned FSE/Huffman oracle vectors with exact expected metadata.
- [x] Port entropy decoder fuzz contracts and bounded arbitrary-input checks.
- [x] Port supported one-shot fuzzer cases and deterministic data generation.
- [x] Preserve exact inline binary regression fixtures and legacy expectations.
- [x] Replay current-format inline regressions and retain the failing legacy contract.
- [x] Port bounded public fuzz-target properties and external corpus replay.
- [x] Generate and verify reproducible decodecorpus compatibility fixtures.
- [x] Replay decodecorpus fixtures and configurable official generator campaigns.
- [x] Map CLI, streaming, dictionary, threading, legacy, and build-test contracts.
- [x] Import and replay the exact standalone testcard and kernel empty-frame tests.
- [x] Pin available external corpus snapshots and provide bounded replay tools.
- [x] Replay official Silesia/GitHub regression data through one-shot Rust APIs.
- [x] Replay external Huffman decoder inputs with upstream parameter consumption.
- [x] Verify all checked-in fixture hashes in the offline CI audit.
- [x] Prevent temporary-directory collisions between concurrent corpus tests.
- [x] Audit test coverage and keep unimplemented contracts explicitly open.
- [x] Run migrated tests across safe/unsafe backends and document exact results.
- [x] Add a reproducible pinned-upstream oracle job alongside the offline CI matrix.

### Required for complete migration

- [ ] Port reusable-context and streaming assertions after those APIs exist.
- [ ] Port raw/prepared dictionary, training, and prefix assertions.
- [ ] Port encoder level/parameter, entropy encoder, and sequence-producer tests.
- [ ] Port caller-owned output, in-place, allocator, and workspace contracts.
- [ ] Port long-distance, multithreading, thread-pool, and large-index stress tests.
- [ ] Port the CLI, gzip/zlib, seekable, and file-operation behavior suites.
- [ ] Make the preserved legacy v0.4-v0.7 success assertions pass.
- [ ] Run all external fuzz/regression corpora with their matching API adapters.
- [ ] Satisfy `scripts/audit_upstream_tests.py --require-complete`.

The required items remain open. A complete inventory and passing available
one-shot adaptations do not complete the upstream test migration.

## Module 6: bounded streaming and reusable contexts

- [x] Specify input/output progress, completion, reset, and error contracts.
- [x] Analyze ring-history ownership, wraparound indices, and backend invariants.
- [x] Implement and test safe bounded ring history independently of frame logic.
- [x] Add and verify an independently gated unsafe ring-history backend.
- [x] Implement incremental XXH64 state with arbitrary update boundaries.
- [x] Generalize sequence execution through a bounded history/output sink.
- [x] Add public progress/directive records and explicit invalid-state handling.
- [x] Parse partial frame/block headers with bounded staging and explicit phases.
- [x] Stream raw/RLE/skippable payloads before a complete block is available.
- [x] Resume block decoding with caller-provided output slices and bounded history.
- [x] Preserve entropy/repeat state and checksums across input/output exhaustion.
- [x] Extract reusable block-encoding state without changing one-shot output.
- [x] Retain bounded block-encoding scratch allocations across stream calls.
- [ ] Add a streaming encoder with bounded block staging and finish semantics.
- [ ] Enforce pledged sizes, flush/finish draining, poisoned errors, and reset reuse.
- [ ] Port every applicable official streaming/context test with explicit source mapping.
- [ ] Generate pinned official near-window sequence fixtures for window logs 10–29.
- [ ] Replay the full window-distance sweep with bounded streaming output.
- [ ] Port malformed streaming-input and randomized directive fuzz campaigns.
- [ ] Test arbitrary chunk sizes, concatenation, truncation, reset, and oracle parity.
- [ ] Measure retained memory and safe/unsafe throughput; document stream limits.

Completed foundation, entropy, one-shot decoding, initial encoding, and validation
checklists were cleared at their module boundaries. See `docs/progress.md` and
the small work-item commits for completed work.

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
