# Tiny sorted-tree and all-match observation protocol

This source-only preparation targets `ZSTD_btGetAllMatches_internal` with
`ZSTD_noDict` in pinned v1.5.7 zstd_opt.c, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It does not execute C, seed arbitrary
maps, select an optimal path, publish frames or enable strategy 7. Private
frequency ownership is independently qualified on the same integration branch.

## Owned request and checked wire format

`ZOPMT01\0` is followed by nine little-endian u32 header words: source length,
query count, window log, hash log, chain log, search log, minimum match, applied
target length and optional hash3 log. Immutable source bytes follow, then six
u32 words per query: logical source position, rep0, rep1, rep2, ll0 and length to
beat. Source length is 8..16,384; query count is 1..64; packet size is exactly
44 + source length + 24 * query count, at most 17,964 bytes. Unknown magic,
truncation, trailing bytes, bool/float recipe integers and narrowing are rejected.

Window log is 10..17, hash and chain logs 6..12, search log 1..8, minimum match
3..7 and applied target 0..131,072. Effective hash width clamps to 3..6. For
minimum match 3, hash3 log is explicitly 6..12; otherwise it must be zero and no
short-key table is owned. This is direct primitive geometry, not a CCtx-applied
parameter claim: the ordinary CCtx hash3 log min(window log, 17) must be qualified
separately. Applied zero target is literal zero, distinct from public zero's
preset inheritance.

Positions strictly increase and are at most source length minus eight. This
allows safe physical hash loads and leaves a four-byte comparison footprint for
three-byte matching. ll0 is exactly 0 or 1. Length to beat is at least the actual
minimum (3 for minMatch3, otherwise 4) and at most min(4096, remaining source). This upper bound is a required
primitive precondition: a source-end match must exceed the previous best length
so the sorted-tree branch exits before comparing a byte at the end pointer.
A threshold one greater than the remaining source is therefore rejected.

Each supplied repeat is 0..min(4096, position + 2). Zero remains a deliberately
ineligible candidate. A source arena with native prefix bias two and an actual
leading extent keeps every pointer expression formed by this direct helper
inside its allocation, including discarded pre-prefix candidates and rep0-minus-
one wrapping to an ineligible code. This transport restriction excludes huge
invalid repeat values whose upstream no-dictionary branch may form an unused
out-of-allocation pointer. It is not an unsupported-format or public-repeat claim.
Ordinary valid fresh repeat state is a later gate.

## Ownership and initialization

The helper owns initialized zero hash heads, paired tree edges and, only for
minimum match 3, short-key heads. Main and short-key frontiers start at native
prefix position two. Window lowLimit and dictLimit equal two, dictBase is a valid
pointer into the same owned arena, and nextSrc denotes the complete logical end.
The candidate buffer owns 4099 initialized `ZSTD_match_t` cells with guards; only
the returned prefix is observed as candidates, and unused sentinel cells/guards
are checked separately. All positions/maps are emitted as native numeric indices
with bias recorded, never as addresses.

Before/after snapshots preserve every head, edge and short-key cell plus both
frontiers. Each candidate records encoded offset intent and match length; lengths
strictly increase from lengthToBeat minus one. Source and guard checks, operation
identity, parameters, padding pass and request/source fingerprints are explicit.
Run two distinct arena padding patterns and compare normal/ASan+UBSan builds.
All helper-controlled storage, output line/stream/aggregate sizes and process
runtime must be capped before execution. A digest alone cannot replace a map.

## Branch distinctions to qualify independently

The outer skipped-frontier return must preserve both maps and both frontiers.
Sorted catch-up uses the target position to compute its window floor, can sample
ahead internally, and finally sets the main frontier to the requested target.
Node expiry and window eligibility are distinct: prefix-boundary candidates can
be eligible even where the deferred Lazy2 policy would exclude them.

Repeat order depends on ll0: rep0/rep1/rep2 when false, rep1/rep2/rep0-minus-one
when true. Repeats may return before head insertion or short-key catch-up. The
hash3 branch runs only when minimum match is three and no length-three candidate
has already won; its own update frontier excludes the query position. A hash3
early return advances the main frontier to current + 1. The normal sorted-tree
path has different publication and repetitive-pattern skipping rules.

Repeat/hash3 sufficient-length early exits use strict greater-than against the
target capped at 4095, or equality with the source end. The sorted-tree branch
instead breaks for a match strictly longer than 4096, or source-end equality.
Do not collapse these three thresholds. Zero/one/4095/4096 targets, full tails,
node reuse, window expiry, all three repeat intents and minMatch3's four-byte
physical footprint need separate tiny hypotheses. Larger source capture must
later cover hash3's strict 2^18 distance cutoff and ordinary CCtx geometry.

## Rollout gates

Freeze source/query-only recipes and malformed transport controls first. Then
review the helper, pinned full dependency closure and exact command inventory.
Independent replay must validate source match bytes, encoded repeat intent,
strict candidate ordering, both frontier lifecycles and complete map continuity;
selection/scoring must not be inferred from an unverified digest. Capture and
reject forged records before projecting fixtures or implementing Rust sorted
insertion/all-match enumeration. Existing deferred-tree and frequency observations
remain immutable predecessor evidence.
