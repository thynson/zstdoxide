# Private Rust Ultra2 validation

The private lifecycle is qualified incrementally. Public Ultra2 remains
unsupported, and this document does not claim complete strategy-9 frame behavior.
The native lifecycle and immutable fixture are described in
`reference-ultra2-native-qualification.md` and
`reference-ultra2-fixture-qualification.md`.

## Checked plan and copied coordinates

`Ultra2ExecutionPlan` validates resolved strategy-9 parameters, private 7..16384
source lengths, ordinary table geometry, literal-price mode, seed hypothesis and
nine exact owner sizes without allocating. Its copied coordinates reserve the
native bias of two and translate only the current interval. Stale integers and
same-ring aliases cannot authorize source access; end positions are valid only
for zero-width ranges. The five eligibility predicates remain distinct from the
helper's unsigned update-frontier precondition. Copied phase transitions prevent
publishing the seed pass, skipping invalidation or retrying a failed coordinator.

Five focused tests pass in safe and all-feature builds. They cover all 53 native
request geometries, all 32 predicate combinations, separate frontier failures,
7/8/9 and source-size limits, u32/usize overflow, old intervals and ring aliases,
phase failure/retry boundaries and the nine independent owner sizes. Formatting,
strict all-test Clippy and true no_std safe/all compilation pass. No parser or
search implementation change is included in this plan qualification snapshot.

The plan archive is `evidence/2026-10-20-ultra2-plan.tar.gz`, SHA-256
`857b541aef185f6e2f804a55fe321360c80deb11485aabc6c922c8a8f64577ba`.
All 579 compiled input identities were captured before and checked after execution.
The archive directly retains 229 regular files, including all Rust sources,
new fixture inputs, drivers and raw command logs. To avoid duplicating large
unchanged inherited corpora, 384 fixture files are represented by exact committed
Git revision/blob/SHA-256/size records. Each Git blob was read and compared
byte-for-byte with its contemporaneous snapshot before omission; the map explains
reconstruction. The complete original local snapshot and redundant full archive
remain available. An earlier focused run omitted integration sources from its
retained input closure; its logs remain, and the final complete rerun supplies
those inputs for all-test Clippy.

## Checked search rebasing and physical owner preservation

The private search constructor admits only its separately bounded source domain.
All source keys, short hashes, bytes and overlapping matches use checked current
interval translation. Ordinary zero-origin contracts remain intact. The rebase
retains all physical hash/tree/short-key cells and allocator identities, resets
the main/local-short frontiers to the new origin, and clears only live candidates.
The window floor is combined with that origin before any candidate source read.
Parser-relative queries are translated exactly once at the provider boundary.

Every native query and complete physical search state matches both phases of all
53 fixtures. Tests cover 29 actual resets, retained stale table cells, direct stale
read rejection even at aliased ring slots, transactional bad arguments, private/
ordinary mode separation, four distinct nonclone owners with no phase allocations,
and live-link corruption poisoning until whole reset. Safe and all-feature runs
also pass all 29 existing/new search and layout tests. The isolated tree, short-key
and match-buffer unsafe configurations each pass all five new tests. Formatting,
strict safe/all Clippy and all five true no_std configurations pass (13 commands).
The first exploratory full-state comparison passed while allocating converted
snapshots for each query; the final test compares complete borrowed table slices
with incrementally reconstructed expected values and avoids that test-only cost.

The search archive is `evidence/2026-10-20-ultra2-search.tar.gz`, SHA-256
`7762e12a82f14b64e30e4ff574b0b3b24158aba701dc2e2c2bb619acf1dabb2e`.
All 580 source/input identities are unchanged across qualification. As with the
plan archive, unchanged inherited fixture bytes have exact verified Git-blob
references and complete reconstruction instructions; all changed/new inputs and
Rust sources are retained directly. The original full snapshots remain local.
No timing or unsafe-speedup conclusion is drawn from these correctness runs.

## Discarded-pass coordinator and exact output publication

The private coordinator uses the same nine allocator objects for both passes.
It reserves literal and sequence capacity once, collects and validates the seed
pass without appending its tail, preserves the frequency/path/search owners,
rebases checked search coordinates, clears token lengths and restores original
1/4/8 repeat offsets before encoding. Only the completed encoding pass exposes
output. Parameter rejection precedes mutation; failures at any work boundary
hide partial tokens and require whole-owner reset. The shared collector retains
the existing Optimal/Ultra behavior and validates final repeats, counts and tail.

All 53 native cases match every one of 394656 path/frequency/query/publication
events and 29 lifecycle resets in safe and all-feature builds. Complete final
wire offset intents reconstruct every source, including tiny private thresholds,
disabled literal prices, skipped-seed hypotheses, stale rings and changed seeded
paths. Full final search tables match the captured states; every intermediate
search state was independently qualified in the earlier search checkpoint.

Six focused tests cover all nine moved nonclone construction/reservation failures,
borrowed owner identities and allocation-free phase/reset reuse, foreign-source/
geometry/used-owner rejection, and injected entry/begin/append/end/reset failures
in both passes. Completed output survives invalid reuse; failed/seed output is
never exposed. Old Optimal/Ultra state and ordinary Ultra parser regression,
strict safe/all Clippy, formatting and true no_std safe/all checks all pass
(11 commands, 582 unchanged compiled input identities).

The coordinator archive is `evidence/2026-10-20-ultra2-coordinator.tar.gz`, SHA-256
`065d298d98280087c53d571413d6e4507c90cc7dffd3406ecdd715b53eda08c9`.
It retains 227 regular bundle files and exact Git-blob reconstruction records for
unchanged inherited fixture inputs; complete original snapshots remain local.
Independent lossless reconstruction and unchanged-original verification pass.
Exploratory runs passed before the final six-test source snapshot. These are
correctness runs and provide no performance claim.

The full isolated feature matrix, foreign platform reconciliation and public
admission remain subsequent gates.

## Foreign ownership and no-atomics qualification

The six coordinator tests above qualify the complete captured trajectories and
nine-owner failure contracts in safe and all-feature builds. A reduced borrowed
nonclone-owner test also passes under strict-provenance Miri on i686 little-endian
32-bit and s390x big-endian 64-bit targets, in both safe and all-feature modes.
It exercises both passes, whole reset, source lifetime and owner destruction.
These four executions are interpreted foreign-target checks, not native runs.

The thumbv6m target passes core-plus-alloc library checks and concrete private
two-pass/reset/drop release code generation in safe and all-feature modes. LLVM
IR and ARM ELF objects are retained. All eight commands pass with unchanged
qualification inputs. The archive is
`evidence/2026-10-20-ultra2-private-platform.tar.gz`, SHA-256
`93768d12d9e164b5b594df4c2f327cc6d7a644f5867f687a9da0da3b5290a6c9`.
It contains 308 regular files, with independently reconstructed lossless content
and exact Git-blob references for unchanged inherited fixtures. The full
isolated-feature matrix remains a separate pending gate.
