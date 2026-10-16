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
