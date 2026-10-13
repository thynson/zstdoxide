# Ordinary lazy-family helper v1 review

The coordinator reviewed `scripts/oracles/lazy_frame_v1.c` against the frozen
protocol and pinned v1.5.7 source before executing any helper mode. SHA256:
`e37b2407cfc9a35a82dc69ae148848b2f2832d59c96f0c51c9d2b08f91ffa951`. Strict C11 normal and ASan/UBSan-configured syntax checks pass with empty
diagnostics. These are compilation checks, not sanitizer runtime or frame results.

The diff from immutable `greedy_frame_v3.c` is limited to the introductory
attribution, numeric canonicalization, depth parameter propagation, selector
mapping, command admission and metadata. All arena/table access, replay,
publication observation, guard, frame/decode and whole-comparison statements
remain unchanged. The original helper and previous evidence are preserved.

## Selector and representation review

The selector is first parsed to uint64_t; depth <= 2 is required before narrowing,
context creation or output creation. Frame IDs must agree with their `dN_`
prefix. Header admission occurs before even its small output allocation.
`parameters` repeats the depth bound and selects named upstream enum constants;
the main entry verifies their expected values 3/4/5. Canonical numeric strings
reject leading zeros except the single character zero. Metadata has its own
protocol identifier/schema and explicit complete depth/strategy arrays.

Pinned `zstd_compress.c:225-262` applies the same row selection and inactive-chain
allocation rules to Greedy, Lazy and Lazy2. Initializer-resolved row selection
is retained and checked, as required by the prior v3 correction. Applied and
match-state parameter tuples already include strategy and remain compared to
the entire request. Wrappers in `zstd_lazy.c:1827-1898` enter the same parser
with depths 0/1/2 and the selected chain/row method; no parser is copied here.

## Preserved ownership and access proof

The seven scheduled-capture owners retain their reviewed bounds: source and
copy each N+64+262656, scratch 2 MiB, accumulated frame N plus bounded block/header
overhead, replay max(N,1), first U32 map 4*2^H, and second 4*2^C chain map or 2^H
row tags. Guards remain 64 bytes on either side. Checked additions/products
precede allocation. Context/workspace allocations and optional decode/whole
owners retain their separate bounds and successful-path release discipline.
Fatal helper rejection still exits the process and makes no leak-check claim.

Only initialized selected table extents are copied. Chain U32 owners retain
alignment/nonoverlap checks; row position/tag alignment, initialized cache and
inactive-chain exclusion remain checked. Pending compressed state is read only
once fresh, with zero/tiny freshness and window-floor exceptions preserved.
Raw indices remain below 4 MiB+2, excluding index reduction. Source bytes stay
immutable and live through the whole schedule and optional comparison.

Lazy's speculative increment can reach the logical chain end-8 or row end-16;
the four/eight-byte probes and row cache work remain within the same initialized
suffix. No logical key validation is relaxed. Rejected matches can update tables
and entropy, but the helper only observes actual library before/after states.
Unchanged independent replay checks each candidate's bounds, distance, literals,
wire repeats and publication decision. Complete frames are decoded by the pinned
library only after a later authorized capture passes all remaining gates.

## Validation and reference recovery

The historical `/private/tmp/zstdoxize-upstream-1.5.7` directory had become a
partial source tree without Git metadata; the first syntax attempt stopped at
its missing `zstd_internal.h`. That diagnostic is retained. A fresh external
checkout at `/private/tmp/zstdoxide-reference-lazy-1.5.7` resolves v1.5.7 to the
exact pinned revision, and every cataloged lib C/header hash was verified before
the successful syntax runs. No old temporary content was replaced.

The command record and diagnostic archive are
`docs/evidence/2026-10-13-lazy-frame-helper.json` and its sibling `.tar.gz`.
The independent validator and capture/auditor preflight remain mandatory before
first execution; syntax success does not bypass those TODO gates.
