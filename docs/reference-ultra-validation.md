# Bounded reference Ultra qualification

Strategy 8 remains unsupported. This module qualifies the coupled optLevel=2
policy before public dispatch. Strategy 9's discarded first pass and history
rewrite are separate work. All prior Optimal fixtures and native observations
remain immutable; additive protocols identify fractional observations explicitly.

## Pinned source review

Reference: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, `lib/compress/zstd_opt.c`.
The external checkout revision was verified before this review.

- Lines 28–65 select whole-bit versus fractional weights and constrain
  `highbit(raw+1)+8 < 31`. Ultra accepts raw statistics 0..8388606; the next
  value violates the upstream debug contract. The fractional range is 256..6143.
- Lines 82–89 refresh all three sequence caches and the enabled literal cache.
  Literal disabling leaves its cache untouched. A policy must stay attached to
  the frequency owner across reset and cache refresh.
- Lines 263–350 define raw literals, LL symbols and match prices. Predefined
  literals stay at six bits and disabled literals at eight bits. Fractional LL
  and ML weights include the 256-unit intercept at raw zero. Ultra removes only
  the large-offset handicap, while retaining the 51-unit dynamic match bias.
- Lines 1204–1245 preserve the displaced match and its predecessor repeat state
  for a match-plus-one-literal alternative when the LL increment is negative.
  The extra destination requires a checked initialized next cell and can extend
  the frontier. Lines 1268 and 1334 limit the half-bit skip and descending-match
  early abort to optLevel=0.
- Lines 1444–1459 couple numeric strategies 7 and 8 to optLevel 0 and 2. A public
  selection of independent price/search shortcuts would not represent this API.

A scalar fixture will first isolate weights, predefined/dynamic LL prices,
match prices, raw literals, and signed adjacent LL increments. Synthetic
frequency inputs identify explicit scalar boundary tests, not observed ordinary
CCtx states. Requests and independent integer expectations are frozen before
native execution. The C adapter reaches unchanged static upstream functions;
normal and fully instrumented builds must agree. Malformed input must be rejected
before any upstream call or stdout record. Frequency lifecycle and whole-parser
captures remain later gates and cannot be inferred from scalar conformance.

## Independent scalar model

The source-only manifest freezes 2470 unique rows: both policies cover weights,
LL prices, match prices, literal clamps/disabled/predefined modes, and signed LL
increments. There are 70 negative-increment cases. Transport size is 98812 bytes,
SHA-256 `9c087f75ca46d26aa5a696e5f0676865f9f8b741e0abd1dd0a08eeb6a01c68bb`.
`check_ultra_scalar_model.py` passes under ordinary and optimized Python, checking
manual interpolation examples, power boundaries, exact packet roundtrips,
malformed transport, strict JSON types and duplicate/nonfinite keys. Its generated
transcript is a self-consistency check, not an upstream observation. No C helper
has been executed at this checkpoint.

## Native adapter preflight

The reviewed native plan contains exactly 134 commands: pinned reference/compiler
identity, full dependencies/predefines, 52 object compilations, two links, two
2470-row positive runs and 70 malformed runs (35 per build). Every malformed
semantic packet starts with a valid row, proving that the complete-packet gate
must reject it before even the first observation. Ordinary and optimized Python
reject ten altered command plans. Six native syntax/dependency/predefine checks
pass, including fully instrumented adapter syntax. The initial Darwin feature
macro syntax failure and its source are retained separately. No scalar oracle
execution has occurred at this checkpoint; capture is the next work item.

## Audited scalar observations

All 134 native commands completed successfully with unchanged inputs. Normal
and fully ASan/UBSan-instrumented builds produce identical complete transcripts;
2470 scalar observations per build match the independent model exactly. All35
malformed packets per build exit1 with the fixed rejection diagnostic and no
stdout, including late-row invalid statistics, lengths, modes and reserved words.

Both ordinary and optimized Python pass35offline controls: two unchanged
positives and33rejected forgeries, with no child attempts. Controls cover source,
compiler, full instrumentation, exact command order, input/output closure,
coherently changed prices/modes/signed increments with updated log hashes,
malformed-input diagnostics and guard records. Full replay also passes under
optimized Python; no assertion-only audit gate is relied on.

`evidence/2026-10-16-ultra-scalar-capture.tar.gz` retains the original capture and
both complete control directories using file-level content deduplication:
26351 reconstructed files, including all native objects/executables. Its161archive
members and every reconstructed file were verified. SHA-256 is
`f387de6aecd19d5a8cad68364bcfbdf761e9ff665c307e020e7af4a72c1f4fa9`.
This establishes synthetic scalar conformance, not ordinary Ultra lifecycle,
path parsing, public admission or a complete frame implementation.

## Immutable scalar fixture

Ordinary and optimized projection independently audit the entire native capture
before emitting byte-identical fixture and provenance files. The118572-byte
`ZUSFIX01` pack contains2470requests and observed signed results; itsSHA-256is
`f5510dff52e9495a257aa78f367a25d1c0b0834ccfc05f3e4ffb099cd944c9b9`.
The separate checker reconstructs every original request and expected result
without C or production Rust. Each Python mode rejects20forged packs, bypassing
the outer root-digest check to exercise exact shape, domains, request/order and
independent arithmetic. Full projection/control evidence is retained in
`evidence/2026-10-16-ultra-scalar-projection.tar.gz` with verified member hashes.

## Borrowed Rust fixture reader

The independent safe reader borrows fixed rows without allocating codec state.
It checks every extent and query domain before exposing copied observations.
Three tests pass in safe debug and release/all-feature configurations, covering
all2470rows,70signed negative results, malformed lengths/header/modes/statistics,
and result sentinel boundaries. Final all-feature test Clippy and formatting
pass. The first Clippy request to use fixed-array chunks and the preceding source
are retained with final logs in `evidence/2026-10-16-ultra-scalar-reader.tar.gz`.
This reader does not depend on production scalar arithmetic.

## Independent frequency lifecycle successor

The policy-tagged model reconstructs all13,036,992bytes of the original147-case
native whole-mode fixture exactly, including all377counts, sums, cached prices
and query results. Both ordinary and optimized Python pass this reconciliation.
All original source/operation bytes remain unchanged.

The source-only successor manifest fixes300wrappers:298positive schedules with
4065complete transitions and2848queries, and2fractional-envelope rejections.
Additional schedules reach literal sums8388604,8388606and8388608; the last is
rejected only for policy2. Predicted post-rescale sums differ at the upper power
boundary (8447versus4351), so full observations must retain these exact cases.
These are independent model expectations until the new native capture executes.

## Frequency native preflight

The exact successor plan contains702commands and320request files per build:
298positive schedules,2fractional-boundary wrapper rejections, and20malformed
transport controls. Both normal and optimized Python reject10changed plans;
all6normal/instrumented syntax/dependency/predefine checks pass. Every negative
command pins its precise diagnostic, excluding internal guard failures from
acceptable input rejections. Source snapshots and logs are retained in
`evidence/2026-10-16-ultra-frequency-preflight.tar.gz`. No frequency oracle
execution has occurred at this checkpoint.

## Checked scalar implementation

The copied policy now selects checked whole or fractional weights, validates
representable cached bases and prevents mixed-policy symbol prices. All 2470
native observations match. The fractional domain is also exhaustively checked
for all 8,388,607 permitted raw statistics against independent integer division.
Original whole-mode scalar and public Optimal fixtures remain unchanged.

Safe, each of 19 isolated unsafe features, and all features pass the selected
scalar/public tests, 86 Optimal library regressions, and true no_std compilation.
Release/all tests, strict safe/all Clippy and formatting pass: 67 successful
commands and 2136 passing test executions. One earlier compiler invocation ran
out of disk space before tests; its complete failed log is retained. Removing
obsolete build caches and disabling incremental compilation allowed only the
unfinished commands to resume against identical source hashes.

Sixteen selected strict-provenance Miri executions pass on i686 and big-endian
s390x, in safe/all configurations. Thumbv6m no_std checks and private scalar caller
codegen pass in safe/all configurations; these are not native foreign firmware
runs. Complete source-bound logs and reconciliation are retained in
`evidence/2026-10-16-ultra-scalar-implementation.tar.gz`. This completes scalar
qualification only; Ultra frequency ownership and path integration remain open.

## Complete native frequency observations

All 702 planned native commands pass against unchanged source inputs. The two
builds agree on 298 positive schedules, 4065 complete state transitions and 2848
queries. Each build rejects both fractional-budget violations and all 20
malformed packets with the exact preflight diagnostic and empty stdout.
All 26 codec translation units are instrumented in the sanitizer build.

Ordinary and optimized Python each pass 38 offline controls: 36 rejected
forgeries and two unchanged positives, with zero child attempts. These cover
policy, every count/sum/cache record, dirty caches, query prices, exact diagnostics,
command inventory and source/output closure. The full capture and both control
directories are retained with content deduplication in
`evidence/2026-10-16-ultra-frequency-capture.tar.gz`: 138379 reconstructed files,
740 verified archive members, and SHA-256
`04adc89aba68fa8883afbe33a13e5e3664c276a9c91a7c5a67570b75bbaccb26`.
This qualifies frequency observations, not Ultra path selection or public frames.

## Immutable frequency projection

The ordinary and optimized projectors independently audit the full native capture
and emit identical 26,646,439-byte packs with 298 cases, 4065 observations and
2848 queries. The complete pack SHA-256 is
`a92871e38d38d30749d7d65ac710e133500ab65348191ba9579e3592f95aa11c`.
Both independent checker modes reconstruct every byte from the frozen model and
reject 20 forged packs with the root digest bypassed. Each forged input is retained
as a lossless delta against the pinned complete fixture, with reconstructed length
and SHA-256 verified. Projection inputs, outputs, checkers and controls are retained
in `evidence/2026-10-16-ultra-frequency-projection.tar.gz`.

## Independent Rust frequency reader

Six reader tests pass in safe debug and release/all configurations; strict test
Clippy and formatting pass. The reader borrows sources/names, fallibly allocates
bounded observation vectors, independently checks all sums and policy-bound
caches, and enforces source spans, dirty/query phases and cumulative fractional
transport limits. Tests cover all 298 cases, unknown and coherently changed
policies, dirty-cache forgeries, malformed extents and exact upper-bound states.

The first boundary test mistakenly exchanged the expected post-rescale sums:
the allowed fractional upper case rescales to 8447, while the whole-mode case
just above it rescales to 4351. The fixture and reader were correct; the assertion
was corrected after inspecting the counts. That failed test/source and the final
passing checks are retained in `evidence/2026-10-16-ultra-frequency-reader.tar.gz`.

## Policy-bound frequency implementation

Frequency owners and copied snapshots carry an immutable pricing policy. The
existing constructor retains Optimal behavior; the private explicit constructor
uses the same single allocation for either policy. Reset preserves policy and
allocator. Fractional publication bounds and policy equality are checked before
any count, sum, cache or phase write. All query components select the same policy.
The existing Optimal parser rejects Ultra frequency owners before resetting parts.

All 4065 native transitions and 2848 queries match complete Rust states, alongside
the original 2018 transitions and 1420 queries. Safe, isolated frequency unsafe and
all-feature configurations each pass 87 Optimal library tests. True no_std safe/all,
strict all-target all-feature Clippy and formatting pass. Source-bound focused logs
are retained in `evidence/2026-10-16-ultra-frequency-implementation.tar.gz`.
Exact fractional owner/error qualification and the full feature/platform matrix
are separate remaining gates. Public Ultra remains unsupported.

## Fractional owner and error boundaries

Seven new tests cover immutable-policy publication, each table's fractional upper
bound, the last permitted literal increment, refresh/reset phase preservation,
borrowed non-cloning allocation reuse, moved-owner allocation failure, malformed
consumed statistics and parser policy rejection. Together with full native replay,
all eight Ultra library tests pass under all features. Rejections compare complete
snapshots, including cached prices and dirty/initialized flags.

All seven selected owner/error tests pass under strict-provenance Miri on i686
and s390x in safe/all configurations: 28 interpreted executions. Safe/all thumbv6m
no_std library checks and concrete safe/frequency-unsafe construction, update,
query, reset and drop code generation pass. These are not native foreign hardware
runs. Complete source-bound evidence is retained in
`evidence/2026-10-16-ultra-frequency-owners.tar.gz`.

## Complete frequency feature checkpoint

All 69 planned commands pass against unchanged Rust/Cargo inputs. Safe, each of
19 isolated unsafe features and all features each pass 94 Optimal library tests,
21 selected scalar/frequency/public integration tests and true no_std compilation.
Complete safe/all suites each pass 1363 tests with 55 ignored migration gaps;
their individual test/outcome inventories match. Release/all passes all 94 Optimal
regressions. Strict all-target Clippy in safe/all modes and formatting pass.
The campaign contains 5235 passing and 110 ignored test executions; ignored cases
are not counted as successes. Full logs, source identities and exact inventory
reconciliation are retained in `evidence/2026-10-16-ultra-frequency-matrix.tar.gz`.

## Complete original path reconciliation

The policy-0 successor path replay, using the new policy-bound frequency model,
reconstructs every byte of the original 27-case, 22,024-event native path fixture.
Both ordinary and optimized Python pass: 3,211,235 bytes, SHA-256
`0defe329f5a50e2add4162fe865c7f1fc314db2c15733485d2cc92f463c7d167`.
This preserves original small and horizon source/parameter/candidate inputs and
all complete path, frequency, repeat and sequence records. Source and both reports
are retained in `evidence/2026-10-16-ultra-path-baseline.tar.gz`.
The new Ultra branch model still requires its own source-only qualification and
successor native capture; this reconciliation alone does not establish those.

## Frozen independent Ultra path model

Both Python modes pass the complete 58-request source-only corpus, retaining every
original request under both policies and adding two deterministic alternative
scripts under each policy. The model predicts 65,672 logical events and 143
sequences per padding pass, with 72 saved-match alternative checks. Coverage
includes accepted/rejected alternatives, frontier extension, continued queries,
continued descending relaxation, and improvement after a rejection.

A hand arithmetic witness checks 4515 + 256 - 224 = 4547 against a competing
2168 cost, preserving the negative increment and complete saved match. Each mode
rejects 20 forged traces and 14 malformed transport/type cases. The original
27-case C-fixture reconciliation also reruns within each full model check.
Requests, branch manifest, deterministic source-only search and complete reports
are retained in `evidence/2026-10-16-ultra-path-model.tar.gz`. These are model
predictions until the reviewed native successor executes; they do not qualify a
production Ultra parser or ordinary match discovery.

## Coupled path native preflight

The frozen native plan has 220 commands, 58 positive packets and 21 malformed
packets per build. Both Python modes reject 14 changed command/instrumentation
plans without child attempts. All 16 unique observation anchors preserve their
original parser statements except the explicitly scripted provider substitution.
Normal and fully instrumented syntax, dependency and predefine checks all pass.
Negative commands pin exact diagnostics, excluding internal guard failures.
Generated source, adapter diffs, original source snapshots and logs are retained
in `evidence/2026-10-16-ultra-path-preflight.tar.gz`. Native path capture is the
next work item; no successor execution result is claimed at this checkpoint.

## Complete scripted path capture

All 220 frozen native commands pass: 58 source-only positive requests and 21
exact-diagnostic negative requests in both normal and instrumented builds. All
26 instrumented translation units are retained. Each padding pass contains
65,672 logical events, including 72 signed saved-match alternatives and 143
sequences; both passes contain 131,344 observed events per build. Normal and
instrumented observations agree byte for byte with independent replay.

Both Python modes reject all 44 forged capture variants and accept two positive
controls without child processes. Lossless modified files, original baseline
identities, all 351,252,305 native log bytes and command/source evidence are
retained in `evidence/2026-10-16-ultra-path-capture.tar.gz` (755 verified members;
archive SHA-256 `8b28f49ad16dd6f821441683a523f4f1b593120ce1c66d2665e4615c8cecab01`).
This qualifies scripted observations, not ordinary match discovery or a Rust
Ultra path implementation. Public strategies 8 and 9 remain unsupported.

## Complete path fixture projection

Both Python modes independently project byte-identical packs and provenance from
the audited native capture. Separate source-only reconstruction reproduces all
8,796,312 bytes (SHA-256
`b5f990623fcc0c09c3d55e86bffa4b3af1a7f9e2decab1a211aaffa70435a7f9`).
The 58 cases contain 65,672 events, 22,798 queries, 72 signed alternatives and
143 sequences. Both modes reject 24 lossless reconstruction forgeries, including
all twelve signed operands, frontier and path-index changes with root digest
checks bypassed. Exact projection outputs, controls and source identities are
retained in `evidence/2026-10-16-ultra-path-projection.tar.gz`.

## Independent path reader

Six independent-reader tests pass in safe, all-feature and release/all builds.
The bounded borrowed reader checks every complete state, policy-specific cache
and frequency bound, all signed alternative operands, and exact accepted or
rejected next-cell/repeat/frontier publication. Malformed extents, flags, deltas,
all twelve alternative operands, signed extremes and dirty-cache changes are
rejected. True no_std safe/all, strict all-target Clippy safe/all and formatting
also pass against unchanged reader/fixture sources. The eight-command record
and source snapshots are retained in
`evidence/2026-10-16-ultra-path-reader.tar.gz`.

## Private scripted Ultra path implementation

The explicit private constructor accepts matching immutable frequency policy and
bounds Ultra sources to 9..8192 bytes. The existing Optimal constructor still
rejects Ultra owners. Ordinary fresh/retained search and planning configurations
reject Ultra before state changes. Reset preserves policy and all three owners.

Ultra now preserves a replaced match for the negative literal-length increment
alternative, resolves its original predecessor repeats before next-cell
publication, and extends the frontier when selected. It continues queries and
descending match relaxation where Optimal takes its existing shortcuts. All
65,672 native events, 72 alternatives and 143 sequences match complete captured
path/frequency/query/repeat state. Both original Optimal scripted and ordinary
parser tests remain unchanged.

Four new tests pass under safe, isolated frequency/path/candidate unsafe, and all
features. Safe/all/release-all each pass all 21 parser tests. Exact non-cloning
borrowed owners incur no allocation during parse/reset; mismatch/bounds rejection
releases all moved owners. Alternative-observer failure poisons the parser,
rejects retry without reset, and permits complete native replay after reset.
True no_std safe/all, strict all-target Clippy safe/all and formatting pass.
Source-bound logs and implementation snapshots are retained in
`evidence/2026-10-16-ultra-path-implementation.tar.gz`. Full feature/platform
qualification is a separate work item; public Ultra remains unsupported.

## Private parser platform and ownership qualification

Both selected Ultra parser owner/boundary tests pass under strict-provenance Miri
on i686 and s390x in safe/all modes: eight interpreted executions. These cover
policy mismatch, source/parameter rejection, exact moved-owner release, borrowed
non-cloning owners, zero parse/reset allocation and immutable policy. Complete
large fixture replay is exercised natively, not claimed as a Miri run.

Safe/all thumbv6m no_std library checks and concrete private Ultra construction,
scripted parsing, observer, reset and drop code generation pass. The probe retains
exact production source copies plus a concrete caller. No foreign hardware or
firmware execution is claimed. Full source-bound logs and probe inputs are retained
in `evidence/2026-10-16-ultra-path-platform.tar.gz`.

## Ordinary-search policy-0 reconciliation

Both Python modes pass exact comparison with all twenty original complete fresh
native captures: 129,220 events across both padding passes, plus all metadata and
repeat-owner publication records. Original packet bytes and normal/instrumented
transcripts remain unchanged. Source-driven successor search supplies every
query result independently; captured matches are never model inputs. Each mode
rejects 38 packet/type/complete-state forgeries without child execution.

The same unchanged old requests produce preliminary Ultra predictions of 126,816
logical events and 19 alternatives per pass. These are model values, not new C
observations or entropy-kind predictions. The exact reports and additive replay
are retained in `evidence/2026-10-16-ultra-fresh-baseline.tar.gz`; ownership and
protocol scope are described in `reference-ultra-fresh-model.md`. Ordinary Rust
Ultra and its larger source envelope remain gated.

## Frozen ordinary Ultra branch recipes

The 44-request manifest preserves all twenty original requests under both policies
and adds two deterministic 624-byte sources under both policies. The source-only
search first tried 64 short alphabet inputs without alternatives, then prefixed
48 repeated fragments with distinct single-byte separators. Seeds 0 and 2 cover
all eight Ultra-specific branches, including selected/rejected alternatives,
frontier extension, continued queries and an improvement after a rejected longer
relaxation. Both searches and all candidate identities are retained.

The four added model cases contain 1482, 1483, 2046 and 2039 events respectively;
the Ultra pair has 72 and 110 alternatives and 72 and 60 sequences. Normal and
optimized Python produce equal manifests, predictions and coverage. Each rejects
17 alternative/publication forgeries, including every signed operand, without
child execution. Exact requests, source search, checks and the immutable manifest
are retained in `evidence/2026-10-16-ultra-fresh-recipes.tar.gz`. The native adapter
and command inventory have not been implemented or executed for these requests.

## Complete private path feature checkpoint

All 69 planned commands pass against unchanged Rust/Cargo inputs. Safe, each of
19 isolated unsafe features and all features each pass 21 parser tests, nine
independent-path/public-Optimal integration tests and true no_std compilation.
Complete safe/all suites each pass 1373 tests with 55 explicit ignored migration
gaps, and their individual test/outcome inventories are identical. Release/all
passes all 21 parser regressions. Strict all-target Clippy safe/all and formatting
also pass. This campaign contains 3397 passing and 110 ignored test executions.

All command/source/log identities and exact outcome reconciliation are retained
in `evidence/2026-10-16-ultra-path-matrix.tar.gz` (74 verified members; SHA-256
`4908b7470b279dd3d15acdd81600aea743a1dc1b313b1ac29af6a2d2c7c5b4cc`).
The full frequency checkpoint remains a prior source-bound checkpoint. This
latest matrix includes the private scripted path implementation and independent
reader; it does not admit ordinary or public Ultra, retained Ultra, Ultra2 or
previously ignored upstream migration cases.

## Ordinary Ultra observer preflight (2026-10-19)

The additive complete fresh observer preserves the original provider and parser;
removing all fifteen read-only hooks recovers the exact pinned source. Outer
policy/nested packet offsets, signed saved-match alternatives and all policy tags
are reviewed in `reference-ultra-fresh-adapter-review.md`. Both strict normal and
ASan/UBSan helper syntax checks pass. The restored external reference checkout
matches the pinned commit and all 89 used source/header identities.

Normal and optimized Python plan checks each pass 216 exact command rows,
26 codec units, 49 source inputs, 29 malformed packets and four descriptor/path
controls. Each rejects 32 command/environment/frame/ledger forgeries without
child execution. Both recipe checks preserve the 44 original requests and reject
17 alternative/publication forgeries. The full source-only prediction manifest
contains 198476 events and 201 alternatives per padding pass; its SHA-256 is
`406d7d8e970f56220d0ed00d55767a4c1e78b07841726a0b3a6ac157a74f6718`.
The exact source hashes, generated identities and complete command inventory are
retained in `evidence/2026-10-19-ultra-fresh-preflight.json`. This checkpoint is
pre-execution qualification; native capture and independent audit follow.

## Ordinary Ultra native capture and successor audit

All 216 frozen native commands completed with expected outcomes: 44 cases,
198476 logical events and 201 alternatives per padding pass, 176 complete frames
and 176 released allocation ledgers. Normal and ASan/UBSan observations agree
exactly. Both builds reject all 29 malformed packets and four filesystem controls.

The original post-capture auditor failed because it applied the 16444-byte valid
packet limit to the intentional 16445-byte oversize negative. Its failed evidence
and exact error remain immutable. Additive auditor v2 reads each exact expected
request length and accepts only that specific prior audit-bound failure; it still
checks every command, source, native state, publication, frame and owner ledger.
Its passed sidecar binds the original failed evidence hash. No native rerun or
semantic relaxation is hidden by the successor.

Normal and optimized successor controls each pass two positive audits and reject
88 forgeries without child execution. Semantic mutations are equal across both
builds and must fail independent state/publication replay, so transcript inequality
cannot stand in for semantic validation. Results and source identities are in
`evidence/2026-10-19-ultra-fresh-controls.json`.

The capture archive retains 970 verified regular members (27943010 compressed
bytes), including original/successor sources, requests, generated instrumentation,
objects, logs, frames, ledgers and both audit outcomes. Its SHA-256 is
`8f75a522e0428169e8e9f1e3da66e8cecca8c0dcc354cf92bfdfb006eb6ed523`;
the archive report documents omitted special filesystem nodes and their retained
creation commands/diagnostics. This qualifies ordinary native observations, not
public Rust Ultra admission.

## Ordinary Ultra immutable projection

The audited native projection contains 44 cases, 198476 events and 201 signed
alternatives in 25429567 bytes (SHA-256
`6844d88c533e56a30fdeac437a63d023cbadb51b0d8a4cb64ef64542460ed0de`).
An independent serializer reconstructs every source-driven search/path/frequency
and publication state without using captured query answers. Normal and optimized
Python reproduce the identical complete pack and provenance and each reject
28 malformed or forged projections. Native frames and allocator ledgers remain
explicit audited opaque inputs with exact identities. Commands, results and
source hashes are retained in `evidence/2026-10-19-ultra-fresh-projection.json`.

## Independent ordinary Rust reader

All ten source-bound reader qualification commands pass. Safe and all-feature
builds each pass fourteen new-reader/original-Optimal/scripted-Ultra tests; the
release build passes all four new-reader tests. Strict all-target Clippy safe/all,
true no_std safe/all and formatting pass. The reader checks complete search and
publication state, all twelve signed alternative operands and their next literal
publication, malformed bounds and all original twenty policy-0 frame/state/owner
baselines. All 44 native frames also decode to their source bytes in Rust.

The 200-member source/log archive is
`evidence/2026-10-19-ultra-fresh-reader.tar.gz`, SHA-256
`3bdcb82d31e958c4625d68390e02f1081e3c8f0e9134fb4bd95930ddb2358325`.
This reader checkpoint precedes private ordinary parser integration.

## Private ordinary Ultra parser implementation

A separately checked private constructor admits Ultra owners for ordinary
64..16384-byte sources. Its admission bit prevents using that larger envelope
with arbitrary scripted providers. The original scripted bound, Optimal entry,
retained entry and public/planning gates remain unchanged. Ordinary parsing uses
the existing checked source provider and seven storage owners.

All twelve implementation qualification commands pass against unchanged named
source inputs: safe/all each pass 28 parser regressions and the complete ordinary
search replay, release/all passes eight new ordinary tests, and strict all-target
Clippy, true no_std and formatting pass. Complete comparisons cover 198476 events,
95581 queries, 201 signed alternatives and 353 sequences. Owner failure controls
cover every moved allocation, borrowed non-cloning reuse, foreign source/geometry,
used state and observer poisoning followed by complete reset/replay.

The 203-member source/log archive is
`evidence/2026-10-19-ultra-fresh-implementation.tar.gz`, SHA-256
`027a3729d5d7bc425aae8f1a6f3202bc90dbb3e3e6877405dc62c37640cbcb72`.
This is private ordinary composition; complete public token/frame admission
remains a separate task.

## Private ordinary platform qualification

Eight strict-provenance Miri executions pass: source/admission and borrowed-owner
parse/reset tests on i686 and big-endian s390x, each with safe and all-feature
builds. These are interpreted foreign-target executions, not native binaries.
Both safe and all-unsafe thumbv6m builds pass true no_std compilation and actual
code generation of a concrete ordinary Ultra construction/parse/reset/drop caller.
The retained LLVM IR contains its definition and the retained objects are checked
32-bit little-endian ARM ELF. No firmware execution is claimed.

All twelve commands bind unchanged source hashes. The 229-member archive
`evidence/2026-10-19-ultra-fresh-platform.tar.gz` retains commands, logs, code,
sysroot setup logs, concrete probe source, LLVM IR and objects (SHA-256
`033063f8d2d2f34e307d4396c0b9d193a4ba38ec3070248f1619ede913721d8e`).

## Private Ultra allocation-free plan

The separate private Ultra plan constructor shares checked source/layout/frame
arithmetic with Optimal while preserving the original Optimal constructor and
public request gate. It checks all nine independent owner sizes before allocation
and keeps unsupported-strategy versus resource-limit precedence. All 22 native
Ultra geometries produce the expected plans. Plain copied plan records are shared
by every backend and need no artificial unsafe representation.

All ten qualification commands pass: seven new/original plan tests under safe,
all features and release/all; strict safe/all-target Clippy; true no_std safe/all;
and formatting. The 201-member source/log archive is
`evidence/2026-10-19-ultra-private-plan.tar.gz`, SHA-256
`0ff25172679c27d8c42ada00b07f85afe8e9afd64275ab5d150642394287286e`.
Token collection and shared entropy remain separate qualification steps.

## Source-only public Ultra inputs

The additive public manifest preserves all 1269 predecessor requests and adds
76 cases: twenty observed source/request pairs, 24 default requests, ten minimum/
target boundaries, eight explicit aliases and fourteen rejection boundaries.
Independent source resolution predicts 1222 successes, 108 resource limits and
15 unsupported cases among 1345 requests, with sixteen equivalence groups.
Normal and optimized controls each reject fifteen forgeries, including semantic
mutations whose outer digest was coherently updated. No children execute.

All 4636 originally successful encoded-response identities are reconstructed
from the unchanged audited predecessor archive. Both Python modes reject four
forgeries each. The original predecessor checker failed before comparison on
its 32 MiB index cap; additive v2 verifies the exact 55635394-byte pinned index.
The initial source/error remains recorded. Commands, source hashes and results
are in `evidence/2026-10-19-ultra-public-inputs.json`. No new native capture or
comparison with these old frames is claimed by this input-freeze checkpoint.

## Private Ultra token publication

The separate Ultra token-state constructor checks matching plan/parser policy,
ordinary admission, complete source identity, effective minimum/target, layout,
literal mode and unused search/parser state before clearing scratch. All nine
allocator identities survive composition. The existing checked collector retains
wire offset intents, three-byte matches, overlap reconstruction, repeat evolution
and complete literal tails. Reservation failures poison the state until reset.

All ten new/original state tests pass under safe, all features and release/all,
including all nineteen automatic-literal native Ultra cases, every moved owner
allocation failure, borrowed capacity reuse and forged plan/parser compositions.
Strict all-target Clippy safe/all, true no_std safe/all and formatting pass.
The ten-command, 201-member source/log archive is
`evidence/2026-10-19-ultra-private-state.tar.gz`, SHA-256
`fa2af89e6ad457d8eb1a9d57759e4a65295e11178ec3b69e4a7b40a807c01aa3`.

## Private Ultra static dispatch and complete frames

The private Ultra plan variant selects the existing inline optimal token owner,
validating variant/strategy correspondence before cloning. Exact factory order
remains nine independent parser/search/scratch identities; borrowed non-cloning
owners introduce no extra identity. The shared entropy owner accepts one complete
source and checked three-byte tokens, commits only a compressed publication and
poisons the workspace after a failed output append.

All sixteen qualification commands pass. Safe, all-feature and release/all each
pass seven dispatch tests, four new Ultra frame tests and two original Optimal
frame tests. Nineteen automatic-literal observed Ultra cases produce deterministic
complete frames that decode exactly; every actual allocation failure releases
its owners, and failed output publication leaves output unchanged. Strict
all-target Clippy, true no_std and formatting pass. The 221-member source/log
archive is `evidence/2026-10-19-ultra-private-frame.tar.gz`, SHA-256
`97415103eb73c0cd1768ec7f3deca9719d6d6b11c51488ceb0b27e5253ff88ef`.
The main checkout's public strategy gate is still closed at this checkpoint.
