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
