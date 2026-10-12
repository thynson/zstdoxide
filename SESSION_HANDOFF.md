# Local session handoff — paused on 2026-10-12

**Local context only. Do not commit this document.** The user requested a pause,
one WIP commit containing all uncommitted project work, and a separate uncommitted
handoff. Do not resume development until the user asks to continue.

- Repository: `/Users/lanxingcan/Workspace/zstdoxize` (directory spelling is
  historical; the project/package name is **zstdoxide**).
- Branch: `master`.
- Last normal commit: `e74ae1b` — `Measure every Greedy backend and retain the safe all repeat`.
- Pause WIP commit: `db6b48e` — `WIP: preserve paused Lazy capture and audited progress`.
- Before pause preservation the worktree was clean: completed work already had
  small commits. The WIP adds the broader Lazy capture archive and its pause
  report; it does not squash or replace the completed commits.
- All capture, test, benchmark, control and packaging processes have exited.
  Process inspection confirmed no project jobs remained. No active child agent,
  goal or recurring automation requires resuming. The one listed child agent
  (`golden_tests`) was already completed.

## User intent and standing requirements

The long-running request is an idiomatic Rust reimplementation of Zstandard,
using facebook/zstd **v1.5.7** as a pinned reference, including migration of all
relevant upstream tests and eventually every roadmap item. Earlier instructions
renamed the project to zstdoxide and requested rebasing its history; do not redo
historical renaming/rebasing merely because it appears in conversation history.
The current request supersedes active development: preserve and pause.

Read `AGENTS.md`, `TODO.md`, `docs/data-structures.md`, and the relevant module
design/validation documents before continuing. Project code, comments, docs,
task tracking and commits are **English**; conversation with the user is Chinese.
Default builds must be entirely safe Rust. Use indices/ranges for non-owning
positions and owned Rust allocations. Each meaningful optimized data structure
has its own opt-in unsafe feature and unchanged safe contract. Every unsafe
function/block needs complete safety requirements/proofs. Preserve no_std+alloc,
fallible allocation and future allocator parameterization. Do not assume a
future stable Rust 1.100 Allocator API. No C bindings/wrappers as implementation;
upstream executables are test oracles only. Downloaded reference source remains
outside the repository. Only the coordinating agent makes small commits, each
with its checked TODO item after focused checks. Clear a completed major-module
checklist and record its milestone in `docs/progress.md`.

The user earlier requested discarding a state-saving commit on resume. If that
preference is applied to this new WIP, first preserve its archive/report in the
working tree or a subsequent real work-item commit. Do not hard-reset away the
only durable copy of captured evidence. Preserve existing completed commits and
any new user edits. The handoff must remain untracked.

## Resume order

1. Inspect Git status and read this file, AGENTS and TODO. Preserve the WIP data.
2. Finish **Module 14 closure**. Implementation, platform/full-feature checks and
   both timing campaigns are complete. Reconcile existing evidence, update the
   stale Greedy validation introduction, append `docs/progress.md`, clear Module
   14's completed checklist and retain Module 15's decomposition. This is the
   only unchecked Module 14 item. No gratuitous full test rerun is needed unless
   code changes or a new concern justify one.
3. Finish **Module 15 broader direct-capture work item**. Actual C capture and
   normal/optimized independent audits passed, but capture forgery controls,
   score-hit reconciliation and permanent fixture freezing are unfinished.
   Keep the corresponding TODO unchecked until those are complete.
4. Project frozen complete observations into checked Rust records; add the Rust
   fixture adapter; implement copied depth/candidate/score records and inclusive
   lookahead coordinates, then Lazy/Lazy2 parsing as separate checked commits.
5. Follow the remaining Module 15 decomposition for ordinary frames, state,
   admission, official assertions, interop, platforms, full matrix and timing.
   The complete upstream migration and roadmap remain open.

## Durable pause archive (current WIP)

- `docs/evidence/2026-10-12-lazy-family-wip.json`
- `docs/evidence/2026-10-12-lazy-family-wip.tar.gz`
- Archive SHA256:
  `8b59851b9138312c2191aa981992749cb9b4b5ab8893e51815ade9cc9aa05bf6`
- 31,222,859 compressed bytes; 73,617,023 uncompressed bytes; 3,703 members.
- Prefix `lazy-family-wip/`. It contains `capture/` with **every one of the
  3,698 actual raw capture files**, `audit-normal.json`, `audit-optimized.json`,
  `pause-report.json`, `pause-packager.py`, and `bundle-manifest.json`.
- Actual objects/executables, requests, logs, metadata and source identities are
  retained. Downloaded C source is not included. Every archive member was read
  back and compared exactly with its source bytes before writing the outer report.
- The manifest maps relative member names to byte sizes/SHA256. Verify hashes
  before restoring to a fresh external directory. The offline auditor accepts
  the restored `capture/` path and validates the original absolute invocation
  paths from saved evidence; it does not execute those recorded commands.
  Archived binaries are for provenance, not instructions to run them.
- Live original capture: `/private/tmp/zstdoxide-lazy-families-2026-10-12`.
- Packager source also remains `/tmp/pause-zstdoxide-lazy-family.py`.
- The WIP report intentionally says `status: paused`, `capture_status: passed`.
  This is preservation, not completion of Module 15 or its fixture work item.

## Environment and source pins

- Reference checkout: `/private/tmp/zstdoxize-upstream-1.5.7`.
- Upstream revision: `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.
- Toolchain: `nightly-2026-10-08`, rustc 1.101 commit
  `1d81eb4ad9cd207e3e638bd32b17ec4fce8412a6`.
- Native target: `aarch64-apple-darwin`; `/tmp` aliases `/private/tmp`.
- Warm build artifacts include `/tmp/zstdoxide-module13-final-matrix/target` and
  the Module 14 immutable snapshot's target. Inspect saved runner/evidence for
  exact invocation arguments before reusing a target.
- Git mutations needed `sandbox_permissions: require_escalated` with `git`
  prefix; commits were authorized. No approval-review rejection is outstanding.
- Do not delete temporary captures/evidence on resume without confirming a
  complete durable archive. Read gzip archives sequentially when verifying;
  random seeking per member/chunk was prohibitively slow.

## Module 14 — complete technical gates, closure pending

Primary documents: `docs/reference-greedy-design.md`,
`docs/reference-greedy-validation.md`, `docs/reference-greedy-benchmarks.md`.
The validation title/introduction still says work is in progress and public
interop/platform/full-feature/measurement gates remain open. Later sections
correctly record their completion. Reconcile the introduction and distinguish
earlier checkpoints during closure. `docs/progress.md` currently ends at Module
13. No Module 14 closure report has been written yet.

### Implemented scope and preserved behavior

Bounded known-source, no-dictionary numeric/request execution supports reference
Fast, DoubleFast and Greedy. Greedy uses chain at window log <=14 and row above
14, with row widths 16/32/64. Bounds are Fast W20/H18; DoubleFast W21/H18/C18;
Greedy W21/H19, chain C18/S8, row candidate visits <=64 (unused chain log is not
artificially capped for row). Greedy M3/4 use key4, M6/7 key6; target length is
ignored as upstream. Level 4 now works in all source tiers, using Greedy through
16,384 and for 131,073..262,144 bytes, DoubleFast otherwise. Named/Fast/DoubleFast
complete frames and exact qualified owner records remain unchanged.

Private frame evidence covers 274 Rust frames, 395 C blocks (181 raw, 38 RLE,
176 compressed), 25 raw and 12 RLE discarded-repeat transitions, and all 6,184
observed allocation-failure ordinals. Direct Greedy has 173 complete map/sequence
cases; ordinary capture has 395 blocks and 367 fresh maps/frontiers/salt/entropy
wire records. Final states are not a chronological cache/probe trace.

Admission v3 retains all original selector identities: 81 selectors =62 admitted,
15 unsupported, 4 advanced; original empty/initial-data loops admit 6/24 and 4/19.
The old 92-case interop set maps 86/3/3 admitted/unsupported/resource; the broader
159-case set maps 121/33/5. Thirteen newly admitted original tests passed safe/all.
The inventory still has **188 unfinished source units: 149 blocked, 34 partial,
5 planned**. No complete source unit was promoted by these bounded extensions.

### Public C interoperability

422 recipes =372 successful, 37 unsupported, 13 resource rejects; 25 equivalence
groups. 4,786 commands, 1,488 C decodes of Rust frames plus 1,488 Rust decodes of C
frames, 413,994,400 reproduced source bytes, 156 malformed rejects. Both builds
compile all 26 used C units plus helper directly. ASan/UBSan covers linked units
with original upstream per-function annotations retained; LeakSanitizer is off.

- Raw: `/private/tmp/zstdoxide-greedy-public-2026-10-12`.
- Permanent: `docs/evidence/2026-10-12-greedy-public-interop.json/.tar.gz`.
- Archive 35,003,394 bytes; SHA256
  `3440015eaf7d12504729b6cd225de36dec0543062c78bddb09381cf4deeefee8`.
  14,414 raw files (~1.305 GB), 15,414 archive members. Its report uses a nested
  `archive` object. Initial random-seek archive verification was stopped after
  writing; separate sequential exact verification passed. This was not a C failure.

### Final full matrix (commit 219f0ef)

- Raw `/tmp/zstdoxide-module14-final-matrix/evidence.json`, SHA256
  `376a56e690bc73f1a8a9537e19c514f74780dcb4ce65638877ace0af3ff5602b`.
- 88 commands passed. 17 full suites (safe, 14 individual unsafe features,
  all debug, all release), each **1,098 passed /0 failed /54 ignored**.
  Aggregate 18,666 passing executions and 918 explicit ignores.
- 16 no_std configurations, formatting, strict safe/all Clippy, 13 allocator
  helper tests, 22 usage-documentation executions per safe/all passed.
- `--require-complete` returned the expected failure for 188 unfinished units.
- Immutable snapshot based on `097e842` plus two platform tests; snapshot report
  SHA256 `abbc7a66109a5f6a8a80f19658b127c8f848da7c3446fbf9e49b45fecf36fecf`.
  Later additive baseline tests, benchmark harness and Module 15 preparation
  were validated separately, not retroactively included in this matrix.
- Permanent `docs/evidence/2026-10-12-greedy-matrix.json/.tar.gz`:
  625,624 bytes and 99 members. Runner `/tmp/zstdoxide-module14-matrix-runner.py`;
  packager `/tmp/package-zstdoxide-greedy-matrix.py`.

### Platform checks (commit de0aa23)

44 strict-provenance Miri executions: ten unit tests plus one public test,
safe/all, interpreted i686/s390x. Nineteen commands, one native smoke, four
thumbv6m no-atomics library/caller compilation checks. Do not describe these as
native foreign hardware tests or proof of target memory fit.

- Raw `/tmp/zstdoxide-module14-platform/evidence.json`, SHA256
  `d1fa6ae2588808c81769b1bb5d6bf7a8875511264e3d5c8439b42ef0b77d346e`.
- Permanent `docs/evidence/2026-10-12-greedy-platform.json/.tar.gz`:
  1,531,205 bytes, 34 members, archive SHA256
  `27731f43af5873bc292e7e4ba1f191d2c246b370fc07a803b926dd80f5c099a4`.
- New tests are in `tests/support/greedy_frame_encode.rs` and
  `tests/reference_greedy_platform.rs`. Saved safe/all caller rlib paths were
  preserved even though Cargo's canonical caller rlib was overwritten by all.

### Complete benchmark campaigns (commit e74ae1b)

Frozen harness: `examples/greedy_benchmark.rs`, `scripts/benchmark_greedy.py`.
Do not edit these casually; fixture and evidence identities pin their bytes.
Four deterministic corpora, eight profiles (Fast, DoubleFast, chain4, chain8,
row16, row32, row64, row-capped), 32 cases per configuration, 24 Greedy cases.
All 16 configurations passed clock-free frame/owner checks before timing.

- Clock-free: `/tmp/zstdoxide-greedy-benchmark-clock-free`, 37 commands, 512 cases.
  Permanent `2026-10-12-greedy-clock-free.json/.tar.gz` (20,672,495 bytes,
  669 raw files, 5,282 members). Harness rejection controls: each normal/optimized
  Python 43 rejected, 2 positive, zero children. Harness archive is
  `2026-10-12-greedy-benchmark-harness.tar.gz` (30,121,924 bytes, 5,499 members);
  its filemap is 57.8 MB, so do not apply a 32 MB metadata cap to that file.
- First timing: `/tmp/zstdoxide-greedy-benchmark-timed/evidence.json`, SHA256
  `91d9055c1a0baa0f0618fe25d8afe93385da1a9e612fbd2d7b294dec226f6b40`.
  All 16 configurations, 37 commands, 512 cases, 2,560 samples.
- Repeat: `/tmp/zstdoxide-greedy-benchmark-repeat/evidence.json`, SHA256
  `f701b6f6badb124998c0da0e86dd372e5a006056ed7c1eb5ca8f0ba8fc3ac2bc`.
  Safe/all, 9 commands, 64 cases, 320 samples. Same executable identities.
- Total 46 commands, 576 cases, 2,880 samples; five samples >=100 ms each.
  Only Global one-shot resolve/allocate/compress/drop is timed. Decode, frame
  hashing and TrackingAllocator surveys are outside clocks. All builds precede
  timing. Project CPU jobs were stopped at recorded barriers before both runs.
  Broader Lazy C capture started only after the repeat exited.
- Barriers `/tmp/zstdoxide-greedy-cpu-barrier-{first,repeat}.json` (09:55:08 and
  10:00:49 UTC). Offline optimized replay passed both campaigns. Timing controls
  each normal/optimized: 2 positive, 22 rejected, zero children; see
  `/tmp/zstdoxide-greedy-timing-controls-{normal,optimized}/summary.json` and
  `/tmp/zstdoxide-greedy-timing-controls.py`.
- Permanent `docs/evidence/2026-10-12-greedy-measurements.json/.csv/.tar.gz`:
  27,911,263 bytes, 6,599 members, 4,745 raw files, exact reconstruction verified.
  Packager `/tmp/package-zstdoxide-greedy-measurements.py`.
- Equal-weight geometric mean of all/safe median throughput over 24 Greedy
  cases: 1.0480 first, 1.0588 repeat. Ranges 0.9709..1.1394 and 0.8546..1.2181.
  Isolated chain backend active cases mean 0.9944; row backend 1.0484. Per-profile
  ratios, all unfavorable results, raw samples and owner scope are in
  `docs/reference-greedy-benchmarks.md`. One host, fixed order, thermal and
  unrelated desktop activity uncontrolled. Individual flags were not repeated;
  inactive flags do not establish causal performance effects. No universal
  speedup, C throughput comparison or foreign-native claim. Safe stays default.

### Closure inputs

Use existing artifact chains rather than restarting campaigns. Reports include
`2026-10-12-greedy-tiny`, `greedy-families`, `greedy-projection`,
`reference-greedy-plan`, `greedy-frames`, `greedy-frame-projection`,
`reference-admission-v3`, `greedy-public-interop`, `greedy-platform`,
`greedy-matrix`, `greedy-clock-free`, `greedy-measurements`, `greedy-baseline`
under `docs/evidence/`. Most archives are flat string/hash fields; public interop
uses a nested object. Greedy frame archive metadata is `archive_verification`:
55,094,419 bytes, 1,384 members, SHA256
`9ac132b6a452fd545acc68e914e26b3a79479efadb84593b59d592be10e4ced3`.
Projection/plan/admission reports may lack archive/status fields by design;
check fixture provenance directly. Prior closure example:
`docs/evidence/2026-10-12-double-fast-closure.json`.

## Module 15 — preparation and C observations, no Rust Lazy implementation yet

Design: `docs/reference-lazy-design.md` and additions to
`docs/data-structures.md`; work items are already decomposed in TODO.
**`docs/reference-lazy-oracle.md` is pinned: do not edit it further.** Put new
qualifications in `docs/reference-lazy-validation.md`. Direct helper/transport
and recipe/controller/auditor pin relationships are frozen. Use explicit new
versions if a real correction becomes necessary; do not silently refresh hashes.
Old Greedy helpers/checkers/docs/fixtures remain unchanged. `scripts/check.sh`
does not yet include the new Greedy baseline/Lazy checkers. Add registration in
an appropriate later work item after Module 14 closure; do not claim those new
checks ran in its immutable full matrix.

### Permanent Greedy baseline (commit 87b361b)

`scripts/check_reference_greedy_baseline.py`,
`tests/reference_greedy_defaults.rs` (nine tests), and
`tests/fixtures/reference-greedy-baseline/{README.md,expected.tsv,frames.bin,provenance.json}`.
32 logical/24 distinct complete frames, 13,723,905 bytes, from all 16 clock-free
configurations with exact qualified allocator records. Thirty-six focused test
executions across safe/chain/row/all plus strict Clippy safe/all and formatting
passed. Each normal/optimized Python: 15 forgery rejects, 2 positive, zero children.
`2026-10-12-greedy-baseline.json/.tar.gz` preserves this separate qualification
(25,587 archive bytes, 20 members). Raw `/tmp/zstdoxide-greedy-baseline-checks/evidence.json`.

### Reviewed implementation details for later Rust work

Reference `lib/compress/zstd_lazy.c:1519–1775`, wrappers depth1 chain/row at
1832/1853 and depth2 at 1876/1897; row auto-selection in zstd_compress.c:225–250.
Lazy/Lazy2 share ReferenceChain/RowTable owners and unsafe leaves. Plain copied
depth/candidate/score records need no artificial unsafe representation. Planned
bounds W21/H19; chain C18/S8; row visits64; unused chain log uncapped; match key
clamped4..6; target ignored. Freeze actual successor admission before enabling
public strategies. Levels5/6 are expected to fit; large-tier7/8 H20 may resource
reject. These are design expectations, not enabled public behavior.

- Baseline repeat1 at q+1 still queries the table at q for depth1/2; table wins
  only strictly longer, ties retain the repeat.
- Lookahead increments after checking q<limit, so a query **at limit is valid**:
  chain N-8, row N-16. Existing Greedy strict wrappers are correct and must stay
  strict. Add a checked inclusive lookahead path.
- Depth1 repeat gain: 3*new versus 3*old-highbit(offBase)+1.
- Depth1 explicit gain: 4*new-highbit(newOff) versus
  4*old-highbit(oldOff)+4.
- Depth2 repeat uses multiplier4 and penalty1; explicit multiplier4 penalty7.
- All improvements are strict. An explicit winner restarts depth1 searching;
  a repeat improvement alone does not restart. Rejected speculative searches
  still mutate maps/frontier/row entropy. Preserve skipping until publication.
- offBase1 is repeat; explicit distance+3. Use bounded checked signed score
  arithmetic for valid matches >=4; do not transliterate C sentinel underflow.
- Preserve backward extension, immediate-repeat cleanup, repeat rotation and
  raw/RLE transactional discard behavior.
- Relevant current Rust: `src/greedy_parse.rs` (borrowed GreedySearch,
  GreedyProgress next/entropy/skipping, depth0 parser), `src/greedy_block.rs`
  (strict search_from, inclusive cleanup_allowed), `src/greedy_search.rs`
  (strict find_chain/find_row). No new Rust implementation edits have been made.

### Tiny helper/protocol/fixtures complete

152-byte `ZLAZY01\0` packet, depth0/1/2 at144, reserved148 zero, branch72;
strategies3/4/5, six unchanged private upstream chain/row entrypoints. Fresh salt
has separate depth switches, two fresh contexts per depth/build. Owner alignment
64; source leading64/suffix2048; sequence capacity floor(B/3)+1; owner64MiB,
stdout128MiB/stderr1MiB; 120s command/30s negatives/1800s campaign. All 26 used C
units syntax-checked before first execution. Shared ownership/hash/replay
functions were reviewed against unchanged Greedy predecessors. Python projects
checked Lazy selectors into the common full-map/sequence validator, never
generating algorithm expectations from Rust.

138 tiny recipes (46/depth): 36 old Greedy cases times3 plus30 tail geometry;
4,449 source bytes, 25,581 packet bytes, 39 malformed. Checker
`scripts/check_lazy_tiny_recipes.py`, generator `scripts/oracles/lazy_tiny.py`;
manifest SHA256 `f1bc0ab9185648892ae9c56f1b5ebd2c8f744edef9a1c87d160aa7e313eedd9c`.
Controller `scripts/capture_lazy_tiny.py`; auditor `scripts/check_lazy_tiny_capture.py`;
permanent checker `scripts/check_lazy_tiny_fixture.py`.

Raw `/private/tmp/zstdoxide-lazy-tiny-2026-10-12`: 422 commands, 552 direct
observations, 12 fresh contexts, 78 malformed rejects. Normal/sanitized complete
direct bytes identical; all 36 Greedy controls and qualified owners exact.
Each normal/optimized Python rejected50 capture forgeries and15 fixture forgeries
with2 positives and zero children. Permanent
`tests/fixtures/reference-lazy-tiny/{capture.json,observations.ndjson}`:
576,102-byte pack SHA256 `5d25e610ba0d6bb1379d38fce19208c558873e9cfd5a1696e0e8534b1179997d`;
capture manifest SHA256 `5f7cf83d440e22cf581b59b7e1b592b9b2db4b2d522ac9efcc57b7753f5736dc`.
`docs/evidence/2026-10-12-lazy-tiny.json/.tar.gz`: 7,039,369 bytes/1,113 members.
One temporary negative-control name typo was corrected in the control runner,
then both complete modes reran; no C/helper/fixture change resulted.

Observed logical final frontiers (C bias2 subtracted), depth0/1/2:
chain fresh first0/2/2, second0/2/3; row fresh first0/3/3, second0/3/4;
chain prefix first8/9/9, second8/9/10; row prefix first8/10/10, second8/10/11.
Strict-limit cases have zero sequences/frontier; others above have one sequence.
These establish final state, not a chronological probe trace.

### Broader recipes and preflight complete

`scripts/oracles/lazy_families.py`, `scripts/check_lazy_family_recipes.py`,
`tests/fixtures/reference-lazy-families/{README.md,recipes.json}`.
645 recipes:303 chain/342 row,215 per depth =138 tiny +137 old broader Greedy
cases times3 +96 constructed score cases. All173 depth0 predecessor controls
retained. 18,900,771 source bytes;19,019,595 packet bytes;55 malformed packets.
Manifest609,872 bytes;19 static pins plus recursive predecessor/salt checks.
Each normal/optimized Python:2 positives,33 forgery rejects,zero children.
`2026-10-12-lazy-family-inputs.json/.tar.gz`:162,983 bytes/29 members.
Temporary runners `/tmp/check-zstdoxide-lazy-family-inputs.py`,
`/tmp/freeze-zstdoxide-lazy-families.py`, `/tmp/package-zstdoxide-lazy-family-inputs.py`.

96 score inputs use4096-byte prefix filled241, 96-byte body32..127, disjoint
prefix copies with preceding243/terminator242, explicit seeds and repeat
distance253. Sixteen families per branch/depth:
depth1 explicit base length8/offBase2048 versus q+1 off128 lengths7/8/9;
depth2 explicit base8/off2048 versus q+2 off256 lengths8/9/10;
depth1 repeat base8/off1024 versus q+1 lengths4/5/6;
depth2 repeat base8/off512 versus q+2 lengths5/6/7;
baseline table8/off2048 versus repeat q+1 lengths7/8/9;
successive q0..3 lengths4/8/12/16, offBases2048/1024/512/256.
These are **source arithmetic hypotheses, not established parser hit claims**.
Inspect actual sequences/full maps for coverage. Later repeat bias can change
the emitted first sequence after an initial table choice; do not infer initial
selection solely from that emission.

Broader infrastructure (commit c84f259): `scripts/capture_lazy_families.py` and
`scripts/check_lazy_family_capture.py`, reviewed before execution. Exactly1,468
ordered commands:4 setup +58 build +2*(3 salts+645 cases+55 negatives).
Auditor requires all138 complete tiny observations and173 depth0 Greedy controls
including qualified owners, full source/command/environment/dependency/object/
executable hashes. ASan/UBSan diagnostics are fatal even in malformed probes.
Each normal/optimized preflight:2 positives,23 rejects,zero children.
`2026-10-12-lazy-family-preflight.json/.tar.gz`:15,300 bytes/9 members.
Runner `/tmp/check-zstdoxide-lazy-family-preflight.py`.

### Actual broader capture passed; next work starts here

Capture completed after the safe/all benchmark repeat exited:
`/private/tmp/zstdoxide-lazy-families-2026-10-12/evidence.json`, SHA256
`028af18ffbae1f600ad1b5e0b6f44cf53d3fa808e4657534d57de27e1525c962`.
1,468 commands,645 recipes,2,580 direct observations,12 fresh contexts,
110 malformed rejects. All26 used units compiled for normalO2 and sanitizedO1
ASan/UBSan with original per-function annotations retained and LSAN off.
Every normal/sanitized direct observation byte agrees.

Independent offline normal and optimized audits both passed, with138 exact tiny
observations,173 predecessor controls and all173 exact qualified owner layouts:
`/tmp/zstdoxide-lazy-family-audit-{normal,optimized}.json` (identical bytes),
corresponding `.stderr` files empty. These audit outputs and the entire capture
are now durable in the WIP archive above. Repository sources matched capture
before/after and were rechecked during pause packaging.

**Still unfinished:** broad actual-capture forgery controls; detailed96-input
score-hit reconciliation; permanent `capture.json`/observation pack and checker;
final archive qualification; Rust projection/adapter; Lazy/Lazy2 implementation.
The TODO capture item correctly remains unchecked. Adapt the tiny50-forgery
control approach, including coherently tampered tiny/predecessor observations,
before freezing the broader fixture. Do not fabricate chronological trace
claims or promote intended score hypotheses without actual evidence.

## Recent history anchors

- `e74ae1b` Measure every Greedy backend and retain the safe all repeat
- `219f0ef` Verify the complete Greedy feature and no_std matrix
- `c84f259` Validate broader Lazy capture plans and predecessor controls
- `0392249` Freeze broader Lazy score and history input families
- `8d0a083` Freeze audited Lazy lookahead observations and predecessor parity
- `3b43b83` Validate bounded Lazy capture preflight before C execution
- `ab23425` Freeze tiny Lazy lookahead inputs and predecessor controls
- `c47253a` Validate Lazy transport selectors and common replay without execution
- `2e6e322` Review versioned Lazy C helper before oracle execution
- `87b361b` Freeze complete Greedy frames and owner regressions before Lazy changes
- `923dc83` Specify bounded Lazy oracle packets and lookahead arena proof
- `55000c6` Analyze reference Lazy lookahead and decompose implementation
- `6bc6e85` Validate all Greedy benchmark backends without clocks
- `de0aa23` Verify Greedy ownership and bounds across interpreted targets
- `93bee66` Validate clock-free Greedy benchmark and historical controls
- `5445b7c` Freeze Greedy benchmark inputs and measurement contract

All official tests have **not** been migrated and the roadmap is **not** complete.
Report bounded milestones and unrun/unsupported behavior accurately.
