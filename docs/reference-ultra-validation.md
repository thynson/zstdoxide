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
