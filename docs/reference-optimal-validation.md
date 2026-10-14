# Bounded reference Optimal validation

Strategy 7 remains unsupported. The current work qualifies the optLevel=0
statistics and scalar price primitives before search/path implementation. It is
not an Optimal frame implementation or a complete upstream migration.

## Independent price capture

The 147 source-only requests cover 9,835,338 source bytes and 2,018 transitions.
Normal and ASan/UBSan builds each run every transition and reject all 17 malformed
packets before producing observation records. The instrumented build includes
all 26 used codec source units plus the adapter; assembly and workers are disabled,
leak sanitizer is not claimed, and upstream sanitizer annotations are retained.
All complete 377-entry frequency tables, sums, cached prices and query results
match independent replay. Both C build transcripts are byte-identical.

Normal and optimized offline audits each reject 30 modified envelopes, commands,
flags, environments, artifacts or coherently rehashed state/price records without
child execution. The captured auditor remains immutable; an additive strict v2
wrapper also rejects evidence-schema numeric substitutions, unknown envelope
fields and a saved self-report inconsistent with independently computed results.
These checks do not authenticate arbitrary coherent replacement of an entire
campaign; the immutable archived bytes and recorded execution provenance are the
basis for fixture trust.

`evidence/2026-10-13-optimal-price-capture.json` binds the 390 commands and a
9,484,451-byte lossless archive. All 1,017 retained files / 35,639,560 logical bytes
reconstruct exactly, including raw logs, packets, local source, C objects and both
executables. Upstream reference source itself remains outside the repository.
No production Rust price implementation or public admission is claimed here.

## Bounded Rust fixture reader

The audited records project to a 13,036,992-byte pack with exact case extents and
a SHA256-bound catalog. Normal and optimized Python reconstruct it from the fixed
archive and independently recheck all C observations. Four reader tests pass in
safe/all/release-all (12 tests), validating source/update budgets, table sums,
cache phases, price modes, fixed record extents, malformed input and selected
hand-calculated prices. Safe/all strict Clippy and formatting pass.

`evidence/2026-10-13-optimal-price-fixture.json` retains the reader, projector, pack
and complete logs. This additive test-only reader postdates the running immutable
Module 16 matrix; its checks are recorded separately. It is not production Optimal
price/search support.

## Allocation-free scalar prices

The preparatory scalar module matches all 1,420 captured C query triples in
safe/all/release-all configurations. Nine focused tests also reject invalid
lengths, zero encoded offsets, overflowing statistics and unreachable prices,
while preserving empty literal and maximum finite boundaries. Strict safe/all
Clippy, formatting and a standalone no_std compilation pass. The copied scalar
price record contains no allocation or table ownership and has one safe value
representation.

Exact source and logs are in `evidence/2026-10-13-optimal-scalar.json`. At that checkpoint the module
was compiled through the focused harness, outside the library module graph and
public dispatch. Private frequency integration is recorded separately below.
It is an additive Module 17 checkpoint outside the running Module 16 matrix.

## Isolated private frequency integration

The `codex/optimal-frequency` branch adds the scalar and frequency modules to the
private library graph and registers `unsafe-optimal-frequencies` as an independent
opt-in backend. One moved allocator owns 377 initialized counters. Updates stage
checked candidate counts before publication; invalid requests and overflow leave
the model unchanged. Dirty caches reject queries until explicit refresh/rescale.
Reset reuses the allocation, and disabled literals retain zero literal statistics.
Public Optimal remains unsupported.

Nineteen focused configurations (safe, every one of sixteen isolated leaves, all
and release/all) pass eight tests each: 152 tests, with all 2,018 captured state
transitions and 1,420 query triples matched per configuration. Eighteen genuine
no_std configurations, safe/all strict Clippy and formatting pass. Safe/all public
and frozen-tree regressions contribute 54 additional tests. Forty-two selected
strict-provenance Miri tests pass on i686/s390x in safe/isolated/all modes. Three
thumbv6m no_std library configurations and two concrete Global-owner caller builds
pass; both caller artifacts are retained. These are interpretation/compilation,
not native foreign execution.

Initial checks caught a missing new-feature entry in the crate's unsafe permission
condition; default builds retain their unsafe prohibition after the correction.
The initial Miri runner attempted an unavailable default cache path; its successor
uses the existing pinned temporary sysroots. Initial failures, corrected runners,
complete source/log evidence and caller artifacts are retained in
`evidence/2026-10-13-optimal-frequency.json` and its archive. No full library-wide
feature matrix or performance claim is made for this private checkpoint. Merge
waits for Module 16's independent matrix and fixed-source measurement closure.

## Source-only tiny all-match preparation

Twenty-nine bounded input recipes contain 71,072 source bytes and 273 queries.
Normal and optimized Python round-trip every packet, admit minimum/maximum
packet boundaries and reject 172 malformed variants per mode without children.
The protocol separates repeat early exits, short-key catch-up and ordinary tree
publication. A length-to-beat greater than the remaining source is rejected
before execution to preserve the upstream sorted-branch end-pointer precondition.

`evidence/2026-10-13-optimal-match-inputs.json` retains exact inputs, checker and
logs. No C search has executed; these are hypotheses, not result fixtures.
Ordinary CCtx geometry, the strict 2^18 distance cutoff and valid fresh repeat
state remain separate TODO gates. Public Optimal remains unsupported.

## Independent tiny search model

Direct suffix comparison and checked integer links independently reconstruct
complete heads, paired nodes, short-key heads, both frontiers and ordered matches
for all 29 recipes / 273 queries. Hand-calculated cases cover repeat intent order,
strict target equality, short-key early return, full tails and skipped positions.
Normal and optimized Python each reject 44 forged records without child execution.

`evidence/2026-10-14-optimal-match-model.json` retains exact code and logs. These
are synthetic model transcripts before C capture, not upstream observed fixtures.
The model is a test oracle only and is not part of the Rust codec.

## Tiny C match capture and primary-hash correction

The 164-command capture executed all 29 recipes / 273 operations with two padding
passes in normal and fully ASan/UBSan-instrumented builds. All 22 malformed packets
per build were rejected before output; every C command completed as planned.
The initial independent comparison failed: its primary hash incorrectly used
three bytes for minMatch3. Upstream ZSTD_hashPtr defaults to four-byte hashing;
only the separate short-key table uses three bytes. Earlier preparation text
about a three-byte primary hash is superseded by this explicit correction.

The original failed envelope and all original code/logs remain unchanged. The
additive v2 model independently matches every head, paired edge, short-key head,
frontier and ordered candidate. Both padding passes and both C builds agree
exactly. Normal and optimized Python each reject 33 capture forgeries with two
positive audits and zero child execution. The v2 synthetic controls additionally
reject 44 forged records per mode and distinguish the primary/short-key buckets.

`evidence/2026-10-14-optimal-match-capture.json` binds the full lossless archive,
including every object and executable, all 18,857,576 raw log bytes and the v1
failure. This is a reconciled offline audit of actual C observations, not a
retroactive passing v1 run. Ordinary CCtx geometry and the 2^18 distance boundary
remain open; no Rust sorted-search or public Optimal admission is claimed.

## Bounded Rust match fixture reader

The audited C maps project into a 7,152,786-byte pack retaining every before/after
owner, both frontiers and ordered candidate. Normal and optimized Python exactly
reconstruct all 29 cases / 273 queries from the immutable capture archive.
The independent Rust reader passes five tests in safe/all/release-all (15 total),
strict safe/all Clippy, formatting and no_std compilation. It validates complete
state continuity, source/window bounds, repeat intent and every match byte.
Captured regression checks separate primary hash4 from short hash3 and preserve
skipped query state and clamped hash widths. Malformed extents, counts, geometry,
queries, cells, frontiers and candidate capacities are rejected.

`evidence/2026-10-14-optimal-match-fixture.json` retains source, pack, exact logs
and the initial corrected Clippy diagnostic. These additive fixture tests are
outside the Module 16 immutable matrix; no production search is enabled.

## Broader short-key geometry and distance capture

All 180 commands pass for 37 requests / 82 operations per C build, with two
padding passes and all 26 used codec units instrumented. Complete maps, both
frontiers and candidate intents/lengths match the independent chunked model;
normal and instrumented transcripts are byte-identical. The model first matched
333 scalar predecessor comparisons and rejected 14 synthetic forgeries per mode.
Each C build rejects 22 malformed packets before output. Normal and optimized
Python each reject 33 capture forgeries, accept two positives and spawn no child.

The requests distinguish distances 262143/262144/262145 under windows18/19,
ordinary short-key logs min(window,17), strict repeat thresholds and fresh1/4/8
repeat values at pointer-safe positions. Source/query maxima are524288/16;
primary/node logs remain bounded to12. Ordinary parser startup and larger
production primary/node geometry remain separate gates.

`evidence/2026-10-14-optimal-match-broad-capture.json` preserves every raw command,
source, request, object and executable, including149,995,548log bytes. No Rust
sorted-search or public Optimal admission is implied by this observation gate.

## Private integration after tree closure

After Module16 matrix and measurement closure, the isolated frequency branch
merges into the main library graph privately. Post-merge checks pass102tests:
24frequency checks across safe/frequency/all,54public/predecessor checks and
24fixture/price checks across safe/all. Strict safe/all Clippy, formatting and
two true no_std configurations pass with every source input unchanged.
`evidence/2026-10-14-optimal-integration.json` retains exact logs and source hashes.
Public strategy7 remains unsupported pending sorted search, path parsing, ordinary
retained-state observations and admission/interoperability qualification.

## Canonical sparse broader fixtures

Normal and optimized Python reproduce the same 2,271,158-byte pack from all
37 audited requests / 82 operations. Explicit map extents and strictly ordered
nonzero cells preserve every complete map, including implicit zero cells. The
independent Rust reader validates source matches, state continuity, hash buckets,
frontiers and canonical sparse encoding. Six tests pass in safe, all-features
and release-all builds (18 total); formatting, strict safe/all Clippy and no_std
compilation pass. Exact source and logs are preserved in
`evidence/2026-10-14-optimal-match-broad-fixture.json`.

## Private sorted-search scalar records

Checked geometry, immutable source borrowing, request validation and copied match
records pass four focused tests in safe/all/release-all (12 total), strict
safe/all Clippy over the library and tests, formatting and no_std compilation.
The source comparison handles overlapping suffixes and logical tails without
padding. Primary minMatch3 continues to hash four bytes; optional short keys
hash three. Evidence: `evidence/2026-10-14-optimal-search-layout.json`.

## Private optional short-key owner

Four tests pass in safe, isolated unsafe-short-key, all-features and release-all
(16 total). They cover disabled zero-allocation storage, first/last cells, invalid
positions and shortened storage, exact moved non-Clone allocator failure cleanup,
borrowed ownership and reset without allocation. Strict Clippy passes in safe,
isolated and all modes; formatting and safe/isolated no_std compilation pass.
Evidence: `evidence/2026-10-14-optimal-short-key.json`. This owner does not enable
public Optimal or yet claim an unsafe performance improvement.

## Private bounded candidate owner

Four tests pass in safe, isolated unsafe-matches, all-features and release-all
(16 total), including full capacity, monotonic length rejection, malformed live
length/storage, exact moved allocator failures, borrowed ownership and zero
allocation after construction. Strict safe/isolated/all Clippy, formatting and
safe/isolated no_std checks pass. The live prefix excludes initialized unused
placeholders. Evidence: `evidence/2026-10-14-optimal-matches.json`.

## Private sorted-search implementation

The private immutable-source search matches all 273 tiny and 82 broader captured
operations: every candidate intent/length, complete primary/paired/short-key map
and both frontiers. Focused checks pass across safe, tree-only, short-key-only,
candidate-only, all-features and release-all builds. Strict safe/all Clippy,
formatting and safe/all true no_std pass with source hashes unchanged. Tests also
cover invalid requests without mutation, poisoned internal failure and reset,
every composite allocation failure, exact borrowed owners and no allocation
after construction. Evidence: `evidence/2026-10-14-optimal-search.json`.

The initial integration rejected zero repeat values admitted by the observation
protocol. The corrected record accepts them and search treats them as ineligible;
zero rep0 with zero literals uses explicit wrapping subtraction and fails distance
eligibility. Both original failing runs and the corrected complete comparison
are retained in the integration diagnostic evidence. Public Optimal stays
unsupported; ordinary startup, path parsing and retained block publication remain
separate gates.

## Private search foreign and no-atomics checks

All 48 selected strict-provenance Miri tests pass on interpreted i686 and s390x
in safe, three-search-leaf and all-features modes. They cover short-key/candidate
storage corruption, allocator failure cleanup, invalid request preservation,
poison/reset, source/owner mismatch and zero-repeat eligibility. Source hashes
remain unchanged. This is interpreter evidence, not native foreign execution.

Five true no_std library builds and five concrete Global-allocator private-search
caller builds pass for thumbv6m: safe, tree-only, short-key-only, candidate-only
and all leaves. This checks code generation on a target without atomic pointer
operations; it is not firmware execution. Exact evidence lives in
`evidence/2026-10-14-optimal-search-platform.json` and
`evidence/2026-10-14-optimal-search-thumb.json`.

## Private search isolated matrix and predecessor regression

All 18 isolated unsafe leaf configurations pass the 29 focused Optimal tests
(522 passes), and all 18 separately compile with true no_std. Complete safe and
all-features suites each pass 1,257 tests with 55 ignored, no failures. The ignored
tests retain their existing explicit scope; this result does not claim they ran.
Every frozen Rust/test input remains unchanged. Exact commands and logs:
`evidence/2026-10-14-optimal-search-matrix.json`.

## Private search diagnostic timings

Five deterministic minimum-match workloads return identical candidate checksums
and exact allocator records under safe/tree/short-key/candidate/all builds. Each
round resets and reuses the same owners without allocation. First campaign and
independent safe/all repeat retain 175 samples, 64 rounds each. Timing begins
after the matrix, foreign Miri and no-atomics gates complete. Normal and optimized
offline audits pass; each mode rejects 16 forgeries, accepts two positives and
spawns no child process. Exact binaries, source, logs and gates are archived in
`evidence/2026-10-14-optimal-search-timing.json`.

All/safe median ratios for minimum 3/4/5/6/7 were 1.336/1.397/1.467/1.413/1.171
in the first campaign and 0.942/0.959/0.954/0.946/0.964 in the repeat. The direction
reverses. Fixed order, short samples and uncontrolled desktop clocks/thermal/load
prevent a stable unsafe-speedup claim. Candidate and allocator equivalence here
complements, and does not replace, the full C-map fixture comparisons.
