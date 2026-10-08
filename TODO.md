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
- [ ] Parse raw/auto/formatted dictionaries and validate IDs, table limits, and repeat offsets.
- [ ] Add a separately gated unsafe dictionary-content accessor with complete proofs.
- [ ] Generate pinned dictionary/prefix interoperability and boundary fixtures.
- [x] Extend bounded ring history with immutable absolute reads and deferred commits.
- [x] Read compressed-block history from immutable prior history plus pending output.
- [ ] Initialize first-block Huffman/FSE tables and repeat offsets from prepared dictionaries.
- [ ] Decode raw dictionary references beyond the ordinary frame window.
- [ ] Handle dictionary-to-frame spanning matches without evicting source bytes.
- [ ] Add persistent dictionary and single-use prefix context configuration.
- [ ] Enforce nonzero frame dictionary IDs while supporting omitted IDs.
- [ ] Add one-shot dictionary decode helpers using the same streaming core.
- [ ] Extend indexed match search to retained dictionary/history prefixes.
- [ ] Write dictionary frame IDs and interoperable raw/prepared dictionary compression.
- [ ] Add dictionary-aware encoding, reset, and single-use prefix configuration.
- [ ] Analyze sample selection and decompose dictionary training before implementation.
- [ ] Implement bounded training sample selection and dictionary-content construction.
- [ ] Serialize validated trained dictionary entropy metadata, ID, and repeat offsets.
- [ ] Port applicable official dictionary, prefix, preparation, and training assertions.
- [ ] Replay matching external dictionary corpora with explicit coverage mappings.
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
