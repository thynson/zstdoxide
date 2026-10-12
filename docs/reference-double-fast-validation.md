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

## Ordinary-frame helper before execution

The new C helper and independent Python validator are implemented. The
[pre-execution review](reference-double-fast-frame-helper-review.md) checks the
actual seven guarded helper buffers, distinct context-owned table dimensions,
source/pointer extents, initialized-only state snapshots and independent wire
replay. Strict C11 syntax/diagnostic checks, formatting and frozen source recipe
checks pass. Normal/optimized Python each pass four positive controls (including
512 unchanged-C hash vectors) and22 rejection controls using explicitly synthetic
table/header structures. These controls are not ordinary DoubleFast observations.

The [evidence](evidence/2026-10-12-double-fast-frame-helper.json) and
[archive](evidence/2026-10-12-double-fast-frame-helper.tar.gz) bind the reviewed
helper and validator identities. No helper mode has been executed at this
checkpoint. Full trajectory validation, normal/sanitizer agreement and complete
frame decoding follow in the separate capture item; unchanged Fast/direct
fixtures remain separate.

## Frozen ordinary-frame publication

The reviewed helper now completes192 capture commands without failure. Normal
and ASan/UBSan builds agree byte-for-byte on all51 scheduled frames, seven aligned
whole-buffer frames and their logical observations. Each build independently
decodes58 complete frames and12,382,352 source bytes. All26 used C translation
units and the helper are instrumented in the sanitizer build; assembly, legacy
support and LeakSanitizer are disabled, and no support archive is linked.
Thirteen additional header probes are intentionally incomplete and undecoded.

The160 positive blocks contain88 raw,6 RLE and66 compressed blocks, with148 fresh
candidates and12 stale-scratch tiny blocks. Thirteen raw and three RLE blocks
discard changed candidate repeats while retaining sampled tables. The frozen
observations demonstrate window20 expiry versus window21 retention for the
A/1MiB-noise/A source, and expiry at window21 with2MiB of intervening noise.
Four complete observation/frame equivalence pairs cover target length, search
log and minimum-match3/4. The original hypotheses and every outcome are retained.

Normal and optimized Python each pass one positive and51 rejection controls,
including26 invalid C setup commands across both binaries. The offline checker
validates source replay, state publication and both complete table maps without
executing C. Both fixture registry modes pass838 files in20 collections. The
[collection](../tests/upstream/double-fast-frames/README.md),
[evidence](evidence/2026-10-12-double-fast-frames.json) and
[archive](evidence/2026-10-12-double-fast-frames.tar.gz) preserve123 data artifacts,
source/command identities and all raw logs. The archive deduplicates4096-byte
chunks; its file map and reconstruction script retain all150,333,464 logical
bytes across1,062 files, verified against the originals after archive creation.

This checkpoint adds ordinary internal-begin C oracles only. Public parameter
resolution, collector corroboration, new Rust checks, platform interpretation
and timing are not claimed. Persistent Rust state and complete DoubleFast frame
emission remain open; no official source unit is newly completed.

## Rust-readable ordinary trajectories and complete frames

The dependency-free [adapter](../tests/fixtures/reference-double-fast-frames/README.md)
projects all51 sources and160 positive blocks into14,663,500 checked binary
bytes, with both independent table deltas and separate raw/wire publication
triples. It borrows58 original complete C frames without regenerating them.
Independent source replay validates all148 fresh candidates,320 complete table
transitions,12 tiny bypasses and the13 raw/3 RLE repeat-discard witnesses.
Source-size caps, record extents, malformed counts, fingerprints, table changes,
offset selectors, publication and stale-candidate forgeries are rejected.

All58 frames decode to12,382,352 expected bytes using the existing Rust decoder
in each of five configurations: safe, table-feature, all-feature, no_std safe
and no_std all-feature. Five adapter tests pass in each configuration, adding25
executions to22 unchanged Fast/named baseline executions. Strict Clippy safe/all,
no_std compilation safe/all, formatting, normal/optimized projection replay and
the838-file/20-collection registry all pass:17 final commands succeed.

The [evidence](evidence/2026-10-12-double-fast-frame-adapter.json) and
[archive](evidence/2026-10-12-double-fast-frame-adapter.tar.gz) retain source
identities, logs and the derived records. Initial tests passed; Clippy requested
a named alias for the generated frame-file tuple type. That correction changes
no oracle data, and all final checks were rerun successfully. This adapter adds
no production dependency or new C capture. Persistent parsing, public
DoubleFast emission, Miri and timing are not claimed at this checkpoint.

## Persistent DoubleFast search state

The private borrowed-source state now retains both complete sampled tables
across ordinary blocks. It resets both owners once when binding a source,
accepts only the caller's committed incoming wire repeats and keeps parsed
position separate from future frame publication. Empty calls leave state
unchanged; lengths1..6 preserve old literal/sequence scratch. Retained floors
advance from block starts without clearing expired table entries. Parser errors
poison the state, including subsequent empty calls; invalid arguments leave it
usable before any allocator request.

All51 frozen ordinary trajectories match160 blocks,148 fresh candidates and320
complete table comparisons per configuration, including all13 raw and3 RLE
repeat-discard witnesses. Exact literals, sequences, offset intents, trailing
lengths and first-two raw/full wire repeats agree. The unused raw C third cell
is explicitly distinct from Rust's real incoming wire third. Tests additionally
cover independent source/table binding rejection, either output reserve failure,
exact owner release and allocation-free source rebinding with a borrowed
non-clone allocator.

Eight tests pass in each of five configurations:40 executions, plus22 unchanged
Fast/named baseline executions. All13 final commands pass, including Clippy
safe/all, no_std compilation safe/all and formatting. The
[evidence](evidence/2026-10-12-double-fast-state.json) and
[archive](evidence/2026-10-12-double-fast-state.tar.gz) bind the source and logs.
The initial seven-test bootstrap also passed without a diagnostic; the final
suite adds explicit constructor rejection coverage. This checkpoint implements
search state only; shared framing, entropy publication and public DoubleFast
encoding remain open. No new C capture, Miri or timing is claimed.

## Shared plain frame metadata

The former Fast-specific frame writer now accepts algorithm-independent checked
`ReferenceFrameSettings`. Source-size sentinel rejection, window10..21 metadata,
block limits and raw-frame bounds precede output allocation. Fast first passes
its unchanged algorithm/resource admission, so common window21 metadata does
not broaden Fast's window20 limit. Header construction keeps the reference's
0xffffffff promotion to an eight-byte content-size field.

Seven frame tests pass in five configurations, including exact comparisons with
all13 original incomplete C header probes on this64-bit host:35 executions.
Nine existing Fast workspace tests pass safe/all, including fallback publication
and every small-frame allocation-failure ordinal:18 executions. The unchanged
Fast/named byte/allocation baselines add22 executions. Clippy safe/all, no_std
compilation safe/all and formatting pass; all15 final commands succeed.

The [evidence](evidence/2026-10-12-reference-frame.json) and
[archive](evidence/2026-10-12-reference-frame.tar.gz) preserve exact sources and
logs. Initial tests passed; Clippy requested fixed-array chunk iteration in the
test-only hex reader, which was corrected before the final checks. Partial
header comparisons do not claim huge-frame decoding or native32-bit execution.
No new C capture, Miri, timing or public DoubleFast encoding is claimed.

## Shared reference entropy policy

Literal and block entropy no longer import the Fast execution plan. Shared
scalar policy supplies strict gain and automatic raw-only literal selection;
the latter requires both the actual Fast strategy and a positive target.
`prepare_reference` and `select_reference` preserve their previous explicit
flag, validation, allocation, section ordering and transactional publication.
Named selection retains its existing distinct policy and ownership.

The new policy integration test demonstrates Huffman and literal-RLE candidates
under DoubleFast target0/4/131072, while positive Fast targets keep raw literals.
All14 literal and9 block entropy tests plus the policy test pass in five
configurations:120 executions. Safe/all Fast/DoubleFast plans, workspace and frozen
byte/allocation baselines add78 executions, for198 total. The plan substring
filter includes13 Fast and6 DoubleFast tests in each configuration. All27 commands pass,
including strict Clippy safe/all, no_std compilation safe/all and formatting.
The bootstrap also passed without diagnostics.

The [evidence](evidence/2026-10-12-reference-policy.json) and
[archive](evidence/2026-10-12-reference-policy.tar.gz) preserve the exact source
and command identities. This checkpoint shares policy and existing entropy
owners; static algorithm dispatch and complete DoubleFast frame emission remain
open. No new C capture, Miri, timing or public DoubleFast support is claimed.

## Static search and shared frame dispatch

`ReferenceSearchPlan` holds one already admitted Fast or DoubleFast plan;
`ReferenceSearchState` constructs only that parser's complete table/vector owners.
The common frame workspace keeps one committed entropy owner and separate
parsed/emitted progress. It consumes the admitted plan without resolving the
request again. The public facade still selects only its previously supported
Fast path; private DoubleFast frame qualification follows separately.

Three dispatch tests pass in five configurations:15 executions. They verify
exactly one Fast or two DoubleFast table allocations, child identities, rejection
before cloning, borrowed non-clone support and a frozen unequal-table ordinary
DoubleFast trajectory. All nine existing Fast workspace tests pass in the same
five configurations:45 executions. The frozen safe/all Fast/named byte and
allocator baselines add22, for82 total. All18 commands pass, including strict
Clippy, no_std compilation and formatting.

The [evidence](evidence/2026-10-12-reference-dispatch.json) and
[archive](evidence/2026-10-12-reference-dispatch.tar.gz) bind the source and logs.
The first workspace bootstrap passed with a temporary unconstructed-variant
warning before the dispatch tests were added; final checks have no diagnostic.
Complete DoubleFast frame/failure qualification and public admission remain
open. No new C capture, Miri or timing is claimed.

## Private complete DoubleFast frames

The shared frame loop now executes complete private DoubleFast frames over all51
fixed source recipes. Each configuration decodes101 main whole/scheduled frames
with the existing Rust decoder and validates truthful window21/content metadata.
The scheduled160 blocks currently select88 raw,6 RLE and66 compressed blocks,
including13 raw and3 RLE changed-repeat discards. These observed counts agree
with the C snapshot, but the tests preserve independent entropy selection and
do not require C-identical payload bytes. Target/search/minimum-match equivalence
pairs and window20 expiry versus window21 retention also pass.

Seven bounded allocation workloads inject failure at every observed request,
including both unequal maximum-table directions. Safe/table/all std logs each
record271 rejected ordinals:1 empty,25 first-uniform,50 literal-tree,56 mixed,
39 raw-then-copy and50 for each unequal-table direction. Both no_std configurations
also inject every observed ordinal without std counter logging. All errors
release through the exact owners. Empty frames allocate only output; tiny frames
allocate output and their two selected tables. Output append failure preserves
existing bytes and committed repeats, discards pending entropy and poisons the
workspace. Invalid scheduling precedes allocator requests; borrowed non-clone
results outlive temporary source storage.

Six new tests pass in five configurations:30 executions. Existing safe/all Fast
workspace and frozen Fast/named baselines add40, for70 total. All15 commands
pass, including strict Clippy, no_std compilation and formatting. The
[evidence](evidence/2026-10-12-double-fast-frame-encode.json) and
[archive](evidence/2026-10-12-double-fast-frame-encode.tar.gz) retain exact source
identities and logs. The initial four-test bootstrap passed without diagnostics.
Public admission, independent C decoding of new Rust frames, full platform
qualification and timing remain open. No new C capture or Miri is claimed.

## Successor admission policy before public execution

The separately versioned [policy fixture](../tests/fixtures/reference-admission-v2/README.md)
preserves all81 original selector tuples and92 historical interop inputs/results.
Fixed C presets establish selector strategy identity without pretending to be
applied scalar observations. Historical selector counts57/20/4 become intended
successor counts61/16/4; historical interop79/11/2 becomes85/4/3. Six formerly
rejected interop cases require additive positive C execution. The oversized
DoubleFast/hash19 request becomes a resource error, not a success. Original
empty and30-byte fuzzer loops predict5/19 and3/16 admitted/unsupported profiles.

Normal and optimized Python each pass8 positive and35 rejection controls,
including scalar types/bounds, strategy-before-resource precedence, malformed
artifact identities and verification before importer access. Child process
creation is disabled in the controls. Both projection and838-file/20-collection
registry modes pass; original artifacts are unchanged. The
[evidence](evidence/2026-10-12-reference-admission-v2.json) and
[archive](evidence/2026-10-12-reference-admission-v2.tar.gz) preserve the source
and controls. These are intended admission expectations frozen before changing
public execution, not successful new codec or C interoperability runs. No new
Rust/Clippy/no_std, Miri or timing result is claimed at this source-only checkpoint.

The additive successor driver reuses the immutable original bounded protocol
and92 source recipes, replacing only their expected admission. Its source
manifest includes the successor policy and historical pins. It checks every
completed Rust scalar observation against the historical tuple before saving
evidence and restores the original driver even on rejection. Normal/optimized
Python pass offline plan checks and in-process valid/rejected observation
controls with child processes disabled. The [driver evidence](evidence/2026-10-12-reference-interop-v2-driver.json)
and [archive](evidence/2026-10-12-reference-interop-v2-driver.tar.gz) make no new
codec, C or sanitizer execution claim. The live job switches with public admission.

## Public reference admission and bounded successor interoperability

The public level/request APIs now resolve actual known input with dictionary
size zero once and dispatch the resulting Fast or DoubleFast profile. Default
levels 0/3 are equivalent; level 2 executes all tiers. Level 4 executes DoubleFast
for 16 KiB < size <=128 KiB and size >256 KiB; the other two tiers remain
unsupported Greedy. Effective limits remain Fast window20/hash18 and DoubleFast
window21/hash18/chain18, checked after resolution and before allocator activity.
A source hint cannot override known input. The output retains its original
allocator, and inactive search storage is never allocated.

Five public DoubleFast tests cover all four source tiers, source hints, window21,
unequal maximum table directions, minimum-match dispatch, nonoperative controls,
independent resource failures before allocator clones, every observed request
failure on a bounded public workload, and borrowed non-clone output lifetimes.
Together with existing public Fast and successor official tests, 27 tests pass
in five configurations: default, unsafe-match-table, all, no_std, no_std/all.
Safe/all plan, workspace, frozen Fast/named baselines and doc examples bring the
final total to229 executions across24 commands. Clippy, no_std compilation,
formatting and inventory checks pass. The [public evidence](evidence/2026-10-12-reference-public.json)
and [archive](evidence/2026-10-12-reference-public.tar.gz) retain initial test
corrections, including the initially omitted level4 >256KiB admission tier.
No production algorithm change was needed for those corrections.

The successor live job independently completes all92 original inputs with
85 admitted,4 unsupported and3 resource errors. Safe/all Rust frames agree;
340 C decodes of Rust frames and340 Rust decodes of C frames reproduce70,212,360
source bytes. All six additive positive C obligations pass, all historical Rust
resolved tuples remain unchanged, and32 malformed protocol inputs are rejected.
The [interop evidence](evidence/2026-10-12-reference-interop-v2.json) and
[archive](evidence/2026-10-12-reference-interop-v2.tar.gz) retain1,171 commands and
the exact capture source. One obsolete cfg(test) unsupported-level3 assertion
was removed after that capture; production code was unchanged. The C helper
alone is ASan/UBSan instrumented; its pinned prebuilt library is not. This
bounded successor campaign does not establish broader DoubleFast workload,
full C instrumentation, foreign-target or timing coverage.

## Successor official numeric and fuzz assertions

The official port now explicitly checks every81-row successor mapping against
its original C selector/input hash, prefix and level, with unchanged preset
strategy identity. Exactly tier-44, tier-49, unsupported-55 and unsupported-56
add DoubleFast success. Original43-row fuzzer loops add only empty levels0/3
and30-byte level3. Existing execution assertions preserve deterministic bytes,
the original compression-size inequality and exact decoded content. Historical
classifications remain57/20/4,3/21 and2/17; current expectations are61/16/4,5/19
and3/16. Advanced selectors retain their unconsumed tails.

Eleven official tests pass in five configurations, and the frozen Fast/named
baselines pass in safe/all builds:77 executions across19 commands. Focused
Clippy, no_std and formatting pass. Normal/optimized inventory audits verify
the complete pinned Git checkout and source anchors; the fixture audit verifies
838 files in20 collections. The completion gate rejects188 unfinished source
units and incomplete external target coverage as expected. No status is promoted
to complete, and caller-buffer, live numeric context, dictionary, decoder-setting,
forced-streaming and in-place gaps remain explicit. The
[evidence](evidence/2026-10-12-double-fast-official.json) and
[archive](evidence/2026-10-12-double-fast-official.tar.gz) retain exact logs/source.
This checkpoint adds no C capture, Miri or timing claim.

## Broader public-frame input plan

A separate versioned [protocol](reference-double-fast-interop-protocol.md) and
source-only fixture freeze159 requests,38,855,519 source bytes,4MiB per-source
and5MiB frame bounds. Expected113 successes,42 unsupported profiles,4 resource
errors and16 Rust frame-equivalence groups remain hypotheses before execution.
The inputs include all51 ordinary source recipes as whole public calls, all four
numeric tiers, independent maximum table directions, window21 retention/expiry,
controls, the43 original fuzzer-loop entries and four exact new selector prefixes.
Original internal schedules, applied tuples and historical observations remain
unchanged. The protocol requires26 independently built C units under ASan/UBSan,
with no prebuilt archive; implementation/review/capture are separate next work.

Normal and optimized Python each pass two positive and34 rejection controls
with child process creation disabled. Both source regeneration checks, the
existing838-file/20-collection registry and successor policy check pass. The
[evidence](evidence/2026-10-12-double-fast-interop-recipes.json) and
[archive](evidence/2026-10-12-double-fast-interop-recipes.tar.gz) preserve inputs,
source identities and controls. No new codec, Clippy/no_std, C, Miri or timing
execution is claimed at this source-only checkpoint.

## Broader adapters before C execution

The separately named C/Rust adapters implement the frozen packet version and
bounded initialized-buffer contract. The [implementation review](reference-double-fast-interop-adapter-review.md)
records exact source identities and checks C allocation arithmetic, initialization,
nonoverlapping owners, header reads, signed conversion, codec lifetimes, guarded
extents and frees. C receives strict syntax checking only at this checkpoint;
no C oracle executable is run. The Rust example forbids unsafe code and validates
metadata plus exact outer frame extent before encode output or decode input.

Safe/all Rust each completes159 requests with113 success,42 unsupported and4
resource errors, preserving identical status, resolved tuple and full frame bytes.
Each configuration self-decodes27,959,660 source bytes, satisfies all16 equivalence
groups and rejects38 malformed packets with exit2 and no successful response.
Focused Clippy, no_std compilation, formatting and22 frozen Fast/named baseline
executions pass. The [evidence](evidence/2026-10-12-double-fast-interop-adapters.json)
and [archive](evidence/2026-10-12-double-fast-interop-adapters.tar.gz) retain630
commands, all raw responses/logs, reviewed source and exact binary identities.
This is not C interoperability or full platform qualification; the instrumented
C build/capture remains the next work item.

## Broader bidirectional public-frame capture

All159 frozen requests complete with113 admitted profiles,42 unsupported and4
resource errors. Safe/all Rust agree on status, resolved parameters and every
frame byte. Normal/sanitized C frames agree. The campaign performs452 C decodes
of Rust frames and452 Rust decodes of C frames, reproducing223,677,280 source
bytes. All16 equivalence groups hold. Each of four adapters rejects38 malformed
packets with exit2, no response and the expected diagnostic prefix.

Both C builds compile all26 used common/compression/decompression translation
units from the pinned clean v1.5.7 checkout; the sanitizer build instruments
all26 plus the adapter with ASan/UBSan. No prebuilt archive is linked. Assembly,
legacy, workers and leak detection are disabled. Recorded dependency hashes,
compiler identity/macros, commands and executable identities qualify the build.
The C response deliberately makes no applied-scalar claim, and independent
entropy decisions do not imply C-identical Rust output.

The independent offline checker verifies every raw artifact and replays all1,600
protocol exchanges among1,666 recorded commands. Normal and optimized Python
both pass; each also accepts two positive controls and rejects24 corruptions
with child process creation disabled. Controls include source replacement,
omitted instrumentation, archive linkage, duplicate objects, response/hash
changes, path escape and a forged oversized window rejected before shifting.
Original raw evidence is unchanged after the controls.

The [evidence](evidence/2026-10-12-double-fast-interop-full.json) and
[reconstruction archive](evidence/2026-10-12-double-fast-interop-full.tar.gz)
retain5,016 files/693,004,628 raw bytes through4KiB chunk deduplication; all6,576
archive members and reconstructed file digests were verified. Compiled objects
and executables are omitted, with their source/build commands and executable
hashes retained. The archive is20,795,123 bytes. The standard oracle job now
runs this bounded capture and its offline replay. This establishes the specified
public-frame scope, not full upstream parity, Miri coverage or performance.

## Focused interpreted and no-atomics platform checks

The fixed b505237 snapshot plus the new `reference_double_fast_platform` target
passes32 strict-provenance Miri executions: eight selected tests in each of
safe/all on i686 (32-bit little-endian) and s390x (64-bit big-endian). Six existing
unit tests check invalid/future/expired parser candidates, both parser reserve
failures, either table-owner failure, exact frame size-field boundaries,
0xffffffff promotion and usize output-bound overflow. Two public tests check
empty/tiny exact allocation ordinals and borrowed non-clone output lifetimes,
then complete96-byte frames with unequal tables and effective widths4/7.
The public tests also pass a two-test native smoke run. All selectors require
exact names and nonzero counts; this is focused interpretation, not full native
foreign-target execution or a rerun of the large frozen parser corpus.

Four thumbv6m checks pass: safe/all no_std library compilation and caller rlib
code generation through both allocator-aware numeric APIs at level3. A separate
Rc type assertion covers the existing no-pointer-atomics dictionary fallback;
it introduces no reference dictionary capability. This does not prove target
runtime behavior, final firmware linking, hardware memory fit or stable-toolchain
support. The temporary caller uses the pinned compiler's available Allocator API
without an unnecessary feature attribute.

All19 recorded commands pass. Production/dependency inputs and the delivered
public test match their snapshot, both cached Miri sysroots are hashed, and
caller artifact identities are verified. The [platform evidence](evidence/2026-10-12-double-fast-platform.json)
and [archive](evidence/2026-10-12-double-fast-platform.tar.gz) retain exact source
catalogs, reconstructable test overlay, commands, environments, selected names,
logs and caller sources. This platform checkpoint contains no timing result.
