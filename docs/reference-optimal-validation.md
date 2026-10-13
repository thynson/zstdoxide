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
