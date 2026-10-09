# Tasks

## Official test migration: remaining work

Completed one-shot and streaming checkpoints are recorded in `docs/progress.md`
and the validation reports. The complete upstream migration remains open.

### Required for complete migration

- [ ] Port advanced context reset, stable-buffer, and parameter assertions.
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

## Module 7: dictionaries, prefixes, and training

- [x] Record dictionary ownership, entropy initialization, lifetime boundaries, and API contracts.
- [x] Parse raw/auto/formatted dictionaries and validate IDs, table limits, and repeat offsets.
- [x] Add a separately gated unsafe dictionary-content accessor with complete proofs.
- [x] Generate pinned raw-dictionary boundary fixtures with independent and C oracles.
- [x] Generate prepared dictionary, prefix, and entropy-reuse interoperability fixtures.
- [x] Extend bounded ring history with immutable absolute reads and deferred commits.
- [x] Read compressed-block history from immutable prior history plus pending output.
- [x] Initialize first-block Huffman/FSE tables and repeat offsets from prepared dictionaries.
- [x] Decode raw dictionary references beyond the ordinary frame window.
- [x] Handle dictionary-to-frame spanning matches without evicting source bytes.
- [x] Add persistent dictionary and single-use prefix context configuration.
- [x] Enforce nonzero frame dictionary IDs while supporting omitted IDs.
- [x] Add one-shot dictionary decode helpers using the same streaming core.
- [x] Add reusable dictionary seed tables and an owned encoder dictionary lifecycle.
- [x] Extend indexed match search to dictionary content and first-block crossing.
- [x] Retain committed encoder block input in a reusable bounded history ring.
- [x] Search indexed retained output and cross-boundary dictionary/history matches.
- [ ] Verify encoder history eviction, reset isolation, and independent interoperability.
- [x] Write dictionary frame IDs with explicit omission and smallest field widths.
- [x] Produce interoperable raw/prepared dictionary compression.
- [x] Expose one-shot dictionary compression with known size and minimal ID width.
- [x] Verify one-shot dictionary compression boundaries and independent C decoding.
- [x] Add dictionary-aware encoding context, reset, and single-use prefix configuration.
- [x] Analyze bounded raw sample selection and dictionary-training ownership.
- [x] Implement safe bounded frequency scoring and deterministic raw content selection.
- [x] Add a separately gated unsafe training-frequency accessor with full proofs.
- [x] Verify raw training quality, malformed parameters, determinism, and bounded selection.
- [x] Decompose entropy construction and formatted dictionary finalization before implementation.
- [ ] Serialize normalized FSE counts with checked forward bit output.
- [ ] Normalize sample frequencies deterministically with bounded integer arithmetic.
- [ ] Encode two-state FSE weight streams using checked transition inversion.
- [ ] Build bounded full-alphabet literal trees and serialize validated dictionary weights.
- [ ] Finalize formatted dictionaries with IDs, sequence priors, repeat offsets and bounded content.
- [x] Implement bounded training sample selection and raw dictionary-content construction.
- [ ] Serialize validated trained dictionary entropy metadata, ID, and repeat offsets.
- [x] Port dictionary decoder, prefix lifetime, prepared entropy, and malformed-input assertions.
- [x] Preserve exact omitted-ID and first-prefix missing-dictionary failure controls.
- [x] Map implemented parser/decoder assertions to pinned upstream source groups.
- [x] Port dictionary encoder lifetime, effectiveness, ID, window, and interoperability assertions.
- [ ] Port applicable official dictionary preparation and training assertions.
- [x] Adapt dictionary loader and raw-dictionary fuzz contracts with exact source selectors.
- [ ] Replay matching external dictionary corpora with explicit coverage mappings.
- [x] Extend deterministic benchmark workloads for dictionary access, codec contexts, and training.
- [ ] Compare safe/unsafe dictionary throughput and memory; run backend/no_std/oracle/Miri checks.

Completed Module 6 work is recorded in `docs/streaming-validation.md`, including
all applicable ordinary streaming assertions and both external streaming corpora.
Its detailed completed checklist is preserved in Git history.

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
