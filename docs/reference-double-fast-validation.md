# Bounded DoubleFast validation

## Private long-key layout

The copied `SearchKeyLayout::for_double_fast_long` admits eight-byte keys at
logs 6..18 without expanding named API limits or Fast's four-through-seven-byte
key domain. Existing MatchTable owners and their independently gated access
backends serve both table roles; this scalar descriptor adds no unsafe operation.

Three focused tests pass in five configurations: safe, unsafe-match-table, all,
no-default, and no-default plus all twelve unsafe features. They check every
u8 log, exact key-length rejection, 576 existing C width-eight hash observations,
256 high-bit-derived smaller-log expectations, and independent long/short table
read/write/reset behavior including valid source index zero. The smaller-log
expectations are mathematical consequences of existing log-ten observations;
they are not new direct C captures. Dedicated DoubleFast fixtures remain open.

The frozen Fast and named baselines pass eleven tests each in safe/all, checking
unchanged complete Fast frames/allocator counts and the original named records.
An existing named/Fast domain guard also passes. All fourteen commands succeed,
including strict all-target Clippy safe/all, no_std safe/all and formatting.
The shared filter also executes an existing Fast tier test in each configuration.
There are 43 passing test executions and no ignored tests in these focused
commands; filter-based unit selections intentionally exclude unrelated cases.
See [evidence](evidence/2026-10-12-double-fast-layout.json) and the
[logs and source archive](evidence/2026-10-12-double-fast-layout.tar.gz).

This checkpoint implements no DoubleFast parser or public compression admission,
and runs no new C capture, Miri campaign or performance measurement. The copied
plan/ownership foundation, direct oracle and parser are separate work items.


## Allocation-free execution plan

`DoubleFastExecutionPlan` resolves the actual known source size with no
dictionary, checks the effective strategy and independently limits windowLog
to 21 and both hashLog/chainLog to 18. It records two checked usize-array Layout
costs and their checked sum, selected block size and source domain without
allocating or cloning an allocator. At both maximum table logs the requested
payload is 4 MiB on 64-bit or 2 MiB on 32-bit, excluding other workspace owners.
The existing Fast cap remains 1 MiB. Complete output-bound checks remain the
later shared framing preflight's responsibility.

Six tests pass in five safe/feature/no_std configurations, thirty executions.
On this 64-bit host each configuration checks 190 existing pinned C parameter
queries, admitting all 29 whose effective strategy is DoubleFast and rejecting
other algorithms. Further cases cover asymmetric/maximal table dimensions,
known-size normalization before resource rejection, strategy-first errors,
minMatch3-to4 short-key dispatch with fixed long width8, unused search/target
fields, known empty input despite a source hint and the excluded size sentinel.
They expressly keep public DoubleFast compression unsupported until execution
is implemented. No fresh C observations are generated for this scalar layer.

Twenty-two unchanged Fast/named baseline executions, strict all-target Clippy
safe/all, no_std safe/all and formatting pass. All thirteen recorded commands
succeed; no tests are ignored. The explicit source catalog and fixed query
fixture are preserved in [evidence](evidence/2026-10-12-double-fast-plan.json)
and its [archive](evidence/2026-10-12-double-fast-plan.tar.gz). No foreign-target
execution or new algorithm performance is attributed to this checkpoint.

## Initial constructed direct observations

The ZDFST01 helper was checked against the twelve-owner allocation and
same-arena pointer proof before its first execution. The exact reviewed helper
identity and reasoning are recorded in
[the adapter review](reference-double-fast-adapter-review.md). Both builds
explicitly compile unchanged `zstd_double_fast.c`, then link the recorded pinned
archive for support. The driver verifies all 89 inventoried lib C/header files,
records compiler/dependency/command identities, checks its inputs before and
after every command, and limits command time and both output streams.

All 37 capture commands succeed. Fifteen recipes run with two padding patterns
under normal and scoped ASan/UBSan builds: 60 C case executions. The complete
normal/sanitized outputs agree, and each pair of padding observations agrees
after removing only its padding field. C and independent Python replay verify
the complete source, expanded sequences, stored literals, raw parser repeats,
full wire repeats and both complete table maps. Sanitizers cover the helper,
explicitly compiled DoubleFast and inline headers; the remaining support
archive is not instrumented and LeakSanitizer is disabled.

The measured consequences of the isolated recipes are:

| Recipe | Observed sequence (LL, ML, distance) | Trailing literals |
| --- | --- | ---: |
| fresh-seven / eight / nine | none; no table changes | 7 / 8 / 9 |
| fresh-ten | none; one changed cell in each table | 10 |
| prefix-nine / ten | (0, 9 / 10, 16) | 0 |
| repeat-one | (2, 62, 1), repeat intent | 0 |
| repeat-four / dispatch-three / ignored-controls | (4, 60, 4) | 0 |
| current-long-floor | (0, 16, 16) | 0 |
| current-short-floor | (0, 4, 16) | 12 |
| later-floor | (0, 4, 8) | 12 |
| later-above-floor | (1, 15, 25) | 0 |
| extend-to-floor | (0, 32, 16) | 0 |

The minMatch3 and unused search/target controls match the complete period4
observation after removing only their differing declared input scalar fields.
Final table deltas do not reveal intermediate write chronology. These manually
seeded cases do not claim realistic whole-context publication, ordinary-frame
compression, dictionary behavior or complete branch coverage.

The frozen sixteen-file collection has an independently pinned catalog and a
compiler-free checker that verifies its recipe module before importing it.
Normal and optimized Python each pass 17 positive/equivalence controls and
102 rejection controls. These include both-build malformed packet rejection,
invalid sequence/long-marker/repeat/table observations, duplicate JSON fields,
repaired but forged fixture catalogs, forbidden destination overlap, offline
live options, and bounded process timeout/output/OSError evidence. All packet
rejections occur before observation output and emit no sanitizer diagnostic.

The new checker and complete fixture registry pass in normal and optimized
Python. Registry totals are now 545 files in 17 collections. Both standard
check scripts include this collection. C/Rust formatting and diff whitespace
checks pass. No Rust codec source changes accompany this oracle checkpoint;
no new production-code tests, Clippy, no_std execution, platform run or timing
claim is attributed to it. Those foundations were verified separately above.
The [capture evidence](evidence/2026-10-12-double-fast-tiny.json) and
[source/log/control archive](evidence/2026-10-12-double-fast-tiny.tar.gz) retain
the observations and qualifications. Directed families, explicit low-log hash
vectors and the Rust parser remain separate open items.

## Direct small-log hash checkpoint

The subsequent [small-log hash capture](reference-double-fast-hashes.md) adds
512 unchanged-C width8 observations at logs6..9, with aligned and unaligned
loads. Normal and scoped sanitized outputs agree. Rust compares the direct
buckets and full table state under five configurations; the selected foundation
tests and old Fast/named baselines total 77 passing executions. Clippy safe/all,
no_std safe/all and formatting pass. The registry now contains 546 files in18
collections. This completes the explicit low-log primitive item; wider parser
families and public execution remain open.

## Wider direct families

The wider [family report](reference-double-fast-families.md) now freezes the
original 168 inputs plus one separately recorded long-literal refinement. All
676 C padding-case executions agree between normal/scoped sanitized builds,
and independent replay checks every sequence and both entire tables. The
original zero-match long-literal candidate remains preserved. The refinement
observes LL65554 with the correct raw-field extension. Offline verification
also checks 22 equivalent-record pairs and 20 isolated sequence witnesses.

Normal and optimized Python both pass all 338 observations and 32 rejection
controls plus 2 positive controls. The registry now contains 715 files in19
collections. Complete provenance, command failures policy, native metadata,
sanitizer limits and the distinction between final tables and chronological
tracing are retained in the report/evidence. The Rust block/owner foundation,
direct-parser implementation and ordinary-frame publication remain open.

## Borrowed block and search cursor

The allocation-free `DoubleFastBlock` checks the complete source range, retained
prefix, independent table roles, block cap and window10..21. Reads end at the
logical block end even if the borrowed source contains later blocks. Ordinary
candidate and entry repeat floors remain separate. `DoubleFastCursor` uses
checked integer thresholds and exposes only pairs with complete eight-byte
keys. Frozen seven-through-ten-byte entry behavior, step3/step4 coordinates and
the zero-match long-literal candidate's last sampled coordinate all agree.

Four focused tests pass under safe, table-feature, all-feature, no_std safe and
no_std all-feature configurations:20 executions. The unchanged Fast/named
baseline suites contribute22 more. Strict all-target Clippy safe/all, no_std
compilation safe/all and formatting pass; all13 final commands succeed. An
initial Clippy rejection of intentionally reversed range syntax in a negative
test was fixed using explicit Range fields; the original failure and final
logs are retained in the [evidence](evidence/2026-10-12-double-fast-block.json)
and [archive](evidence/2026-10-12-double-fast-block.tar.gz). These shared scalar
records introduce no unsafe operation. This checkpoint does not implement
matching, ordinary frame publication or public DoubleFast admission.

## Independent table owners

`DoubleFastTables<L,S>` owns two existing MatchTable values, with separately
checked roles and allocator values. Both layouts and their summed payload cost
are checked before the first allocation. Four tests cover invalid role rejection
without allocator requests, either allocation failure with exact-owner release,
unequal minimum/maximum dimensions at every short-key width, borrowed non-clone
ownership, allocation-free reset and moves/swaps among distinct clone identities.
The composite adds no unsafe operation; its table leaves use the existing
independent `unsafe-match-table` feature.

All four tests pass in the same five configurations above:20 executions, plus
22 unchanged Fast/named baseline executions. All13 commands succeed, including
strict all-target Clippy safe/all, no_std compilation safe/all and formatting.
The [evidence](evidence/2026-10-12-double-fast-tables.json) and
[archive](evidence/2026-10-12-double-fast-tables.tar.gz) record exact source and
command identities. Direct parsing and public DoubleFast execution remain open.

## Rust-readable direct observations

The test-only [projection](../tests/fixtures/reference-double-fast/README.md)
contains all15 tiny and169 wider recipes, representing368 padding observations.
It preserves source bytes, ordered seeds, raw and expanded sequences, entry
bounds, raw/full repeat triples and both independent final table observations.
The converter checks fixed importer identities before importing them, then
verifies the original pinned collections before projecting one representative
of each padding-equivalent pair. The original observations remain unchanged.

Four dependency-free Rust tests validate184 source replays and368 complete
table reconstructions per configuration, cross-check the borrowed block bounds,
and reject malformed records and forged length/repeat/intent/table metadata.
They pass under safe, table-feature, all-feature, no_std safe and no_std
all-feature configurations:20 test executions. The unchanged Fast/named suites
add22 passing executions. Strict all-target Clippy safe/all, no_std compilation
safe/all and formatting pass. Normal/optimized offline projection and fixture
registry checks also pass:all17 final commands succeed. The original registry
remains715 files in19 collections; this derived projection is checked separately
by both standard check scripts. No official source unit is newly completed.

The [evidence](evidence/2026-10-12-double-fast-adapter.json) and
[archive](evidence/2026-10-12-double-fast-adapter.tar.gz) preserve the binary,
reader/converter identities and logs. A bootstrap shift-literal inference error
was corrected to explicit usize before the passing checks. This fixture adapter
does not implement the matching loop, ordinary frame publication or public
DoubleFast encoding, and introduces no production dependency.

## Single-block DoubleFast parser

The private indexed parser now matches all184 frozen constructed cases, each
using both an exact source slice and a slice exposing32 initialized later bytes
outside the logical block. Each configuration checks368 parser calls and736
complete table comparisons, alongside every expanded LL/ML/distance/offBase,
literal byte, trailing length, raw parser repeat and full wire repeat. The
frozen cases exercise both candidate floors, collisions, cached/deferred stores,
sampling, short-versus-later-long preference, immediate repeats and long markers.
The parser preserves full lengths; the test adapter independently checks their
raw C field representation. No oracle expectations were changed for Rust.

Five tests also cover output reuse with a borrowed non-clone allocator,
invalid layout/repeat rejection before requests, preservation of both logical
outputs and tables on either reserve failure, and expired/future/out-of-range
candidate rejection. They pass in five configurations:25 executions, plus22
unchanged Fast/named baseline executions. Clippy safe/all, no_std compilation
safe/all and formatting pass. Two bounded candidate/failure tests additionally
pass under strict-provenance i686 Miri in safe and table-feature builds:four
interpreted executions. This is not native i686 or full platform validation.

The [evidence](evidence/2026-10-12-double-fast-parse.json) and
[archive](evidence/2026-10-12-double-fast-parse.tar.gz) retain all18 successful
final commands, exact source identities and initial diagnostics. The first
bootstrap already passed all frozen cases; a separate negative test incorrectly
assumed a positive leading literal despite legal backward extension. Its
expectation was corrected. Native-host Miri cache setup was initially denied
before tests ran; the successful campaign used the existing external i686
sysroot. No new C capture, timing, ordinary-frame publication or public
DoubleFast encoding is claimed at this checkpoint.

## Ordinary-frame input protocol

The separate [ordinary-frame protocol](reference-double-fast-frame-oracle.md)
now specifies context-owned storage, both independent table observations,
candidate/committed state boundaries, fresh/tiny/empty behavior, full replay,
frame decoding and complete used-C sanitizer scope before helper execution.
The fixed [source-only recipes](../tests/fixtures/reference-double-fast-frame-recipes/README.md)
contain51 cases and6,348,944 source bytes:164 scheduled calls,160 positive blocks,
seven requested aligned whole-buffer comparisons and13 incomplete header probes.
The larger window hypotheses cross1MiB and2MiB; no outcome is inferred from
the existing Rust parser or from an intended witness name.

Normal and optimized Python independently regenerate the manifest and each
pass two positive/39 rejection controls, including source/schedule/parameter
bounds, bool scalars, JSON duplicates/nonfinite values, changed manifest data
and verification of generator identity before import. Both standard check
scripts include the compiler-free input check. The original fixture registry
and official-source completion counts are unchanged. The
[evidence](evidence/2026-10-12-double-fast-frame-recipes.json) and
[archive](evidence/2026-10-12-double-fast-frame-recipes.tar.gz) preserve the fixed
protocol, input manifest, generator and controls. Helper implementation/review,
C capture and ordinary-frame outcomes remain open.
