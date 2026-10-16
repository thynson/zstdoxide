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
