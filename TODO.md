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

Module 12 is complete within its bounded no-dictionary reference Fast scope.
Completed items are archived in Git history and `docs/progress.md`. The complete
correctness/platform evidence and migration gaps are recorded in
`docs/reference-fast-validation.md`; both timing campaigns and their inconclusive
unsafe-speedup comparison are retained in `docs/reference-fast-benchmarks.md`.

Module 13 is complete within its bounded no-dictionary reference DoubleFast
scope. Completed work items are archived in Git history and `docs/progress.md`.
The final matrix, interoperability and platform qualifications are recorded in
`docs/reference-double-fast-validation.md`; all measured backends, raw samples
and the inconclusive unsafe comparison are retained in
`docs/reference-double-fast-benchmarks.md`.

## Module 14: bounded reference Greedy execution

Begin with source review and data-structure design before implementation.
Preserve all named/Fast/DoubleFast outputs, owner contracts and frozen policies.
The reviewed scope and ownership design are in `docs/reference-greedy-design.md`.

- [x] Analyze source-dependent chain/row selection, ownership, salted hashes, exact parser floors and newly admitted official subsets before implementation.
- [x] Freeze complete current DoubleFast frames and qualified allocation records from the reviewed clock-free benchmark as permanent compatibility regressions.
- [x] Specify a bounded direct Greedy C protocol with separate chain/row arenas, full owner maps and short-input safety proof before execution.
- [x] Implement and source-review the bounded direct/fresh-salt C adapter; complete syntax checks before its first execution.
- [x] Implement the independent packet/record validator and freeze tiny chain/row recipes without generating expected output.
- [x] Capture and independently verify fresh-context salt and tiny chain/row observations with all used C units instrumented.
- [x] Pin broader source/initial-state recipes for row widths, hash/chain collisions, cyclic bias, gaps, skipping, floors, repeats and malformed packets.
- [x] Capture normal/sanitized direct observations and independently reject changed or malformed evidence offline before Rust parsing.
- [x] Add checked salted row-key/layout value records without changing existing Fast/DoubleFast key domains.
- [x] Add initialized safe reference-chain storage with exact independent owners, cyclic slot translation and bounded traversal.
- [x] Add the equivalent opt-in unsafe-reference-chain backend with complete access proofs and corruption/owner equivalence tests.
- [x] Add initialized safe row-table storage with reserved heads, cyclic tag ordering, candidate snapshots and exact owners.
- [x] Add the equivalent opt-in unsafe-row-table backend with complete access proofs and full-map equivalence tests.
- [x] Add allocation-free Greedy admission planning with actual source-tier chain/row selection and independent resource caps.
- [x] Add borrowed Greedy block/cursor records with per-probe floors and distinct chain/row search-tail transitions.
- [x] Project frozen direct Greedy inputs/observations into bounded little-endian records with independently checked provenance.
- [x] Add a dependency-free checked Rust adapter for frozen direct Greedy inputs and complete table/sequence observations.
- [x] Implement safe reference chain search against full frozen heads/links and update/skipping transitions.
- [x] Implement safe reference row search against frozen salted hashes, full positions/tags, head wrap and long-gap updates.
- [x] Implement Greedy repeat-first parsing, tie selection, backward extension and immediate-repeat cleanup against frozen candidates.
- [x] Specify and pin ordinary Greedy frame/schedule recipes before C execution.
- [x] Implement and source-review the bounded ordinary Greedy frame helper; complete syntax checks before execution.
- [x] Version the unexecuted ordinary helper to distinguish tiny window-floor clamping from long-gap/parser updates.
- [x] Pin additive ordinary chain/row tiny-window-floor recipes before C execution.
- [x] Implement the ordinary Greedy offline replay validator with frozen hash and synthetic structural rejection controls.
- [x] Implement the bounded ordinary Greedy capture controller and malformed C-input campaign before execution.
- [x] Preserve the failed unresolved-auto capture and correct the ordinary helper initialization before retrying.
- [x] Capture normal/instrumented ordinary publication trajectories and complete C frames, then independently reject forged evidence offline.
- [x] Project frozen ordinary Greedy trajectories and frames into bounded little-endian records with checked provenance.
- [x] Add a dependency-free checked Rust adapter for frozen ordinary Greedy trajectories and frames.
- [x] Retain selected chain/row state across blocks with ordinary next-to-update adjustment, tiny bypass and failure poisoning.
- [x] Extend static private search dispatch while preserving existing owner counts and complete frame bytes.
- [x] Emit private complete Greedy frames with transactional entropy/repeat publication and all observed allocation-failure ordinals.
- [x] Freeze successor v3 admission mappings over unchanged selectors, numeric loops and both historical interop sets before changing public behavior.
- [x] Extend public request admission and truthful documentation with resolved Greedy scope and unchanged unsupported/resource precedence.
- [x] Port newly admitted official numeric/fuzz assertions without rewriting historical C tuples or closing unrelated obligations.
- [x] Specify the successor public-frame protocol, ownership bounds and source-family plan before implementation.
- [x] Freeze successor public-frame recipes over both unchanged historical sets and additive Greedy sources before codec execution.
- [x] Implement and review separately versioned public C/Rust adapters, checked transport and malformed-packet controls before first C execution.
- [x] Implement the bounded public Greedy capture controller and independent offline auditor with preflight rejection controls.
- [x] Capture broader bidirectional public Greedy interoperability with normal/instrumented C, reject forged evidence offline and preserve the complete campaign.
- [x] Verify focused chain/row ownership and parser bounds on interpreted i686/s390x and compile-only no-atomics callers.
- [ ] Run the full safe/each-feature/all/no_std correctness matrix with unchanged Fast/DoubleFast/named regression baselines.
- [x] Freeze the Greedy benchmark sources, request profiles and measurement/ownership contract before implementation.
- [x] Implement the separate clock-free Greedy harness and independent evidence validator with historical frame/owner controls.
- [x] Validate all fourteen individual unsafe backends, safe and all together on the frozen clock-free workloads.
- [ ] Measure every backend after a completed CPU barrier and repeat safe/all with all raw samples retained.
- [ ] Reconcile tests, inventory, evidence and qualified measurements, record the bounded milestone and clear its completed checklist.

## Module 15 preparation: bounded reference Lazy and Lazy2

Module 14's immutable full matrix and measurements remain the current milestone
gate. Source review and fixture preparation may proceed without changing its
measured codec inputs. Do not enable new public strategies before their own
successor admission mapping and independent oracle checks.

- [x] Review depth-one/two parsing, inclusive lookahead bounds, score arithmetic and shared ownership before implementation.
- [x] Freeze complete Greedy frames and qualified allocator records as permanent regressions from the reviewed clock-free archive.
- [x] Specify bounded direct Lazy/Lazy2 packets, arena bounds and independent observation validation before C execution.
- [x] Implement and review the versioned direct C adapter, completing normal/instrumented syntax checks before execution.
- [ ] Add checked Python transport and independent replay with malformed depth/schema controls and no child execution.
- [ ] Freeze tiny depth/branch recipes and predecessor controls before C execution.
- [ ] Capture normal/instrumented tiny observations and fresh-context salt, then independently reject changed evidence offline.
- [ ] Freeze broader recipes covering repeat ties, gain thresholds, successive lookahead, tail equality, skipping, floors and table trajectories.
- [ ] Capture and independently validate the broader direct observations before Rust parsing.
- [ ] Project direct observations into checked Rust records with complete maps, sequence replay and source provenance.
- [ ] Add shared copied depth/candidate/score records and explicit inclusive lookahead coordinates, preserving Greedy's strict main-search contract.
- [ ] Implement depth-one selection and repeated improvement against frozen observations.
- [ ] Implement depth-two selection, repeat bias and exact speculative sampling against frozen observations.
- [ ] Specify and freeze ordinary frame/schedule recipes and versioned C observation protocol before execution.
- [ ] Review, capture and independently validate ordinary Lazy/Lazy2 trajectories and complete frames with instrumented C.
- [ ] Retain shared lazy-family state across blocks with exact owners, tiny handling and failure poisoning.
- [ ] Integrate private frame dispatch and transactional entropy/repeat publication while retaining every prior complete frame and allocation baseline.
- [ ] Freeze successor admission mappings over all unchanged selector, numeric-loop and historical interoperability inputs.
- [ ] Enable bounded public Lazy/Lazy2 requests and document exact source-dependent level/resource behavior.
- [ ] Port newly admitted original selector and numeric assertions without closing unrelated upstream obligations.
- [ ] Capture and independently validate broader bidirectional public interoperability with normal/instrumented C.
- [ ] Verify exact owner release, allocation failures, malformed state, no_std and interpreted i686/s390x bounds plus no-atomics caller compilation.
- [ ] Run safe, each unsafe feature and all-feature correctness checks with every predecessor regression intact.
- [ ] Freeze and verify clock-free workloads, then compare all backends and repeat safe/all after a CPU barrier.
- [ ] Reconcile the bounded milestone, official-test gaps and measurements; clear completed items and retain the next decomposition.

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
