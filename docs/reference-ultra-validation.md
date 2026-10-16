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
