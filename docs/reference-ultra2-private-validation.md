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

Discarded-pass parser/coordinator state, complete path/frequency comparisons,
nine-owner failure controls and public admission remain subsequent gates.
