# Policy-tagged frequency observation protocol v1

This additive protocol observes no-dictionary frequency initialization, update,
refresh, rescaling and queries from unchanged pinned zstd v1.5.7 functions.
It adds price policy0/2 to the original Optimal v2 schedule while preserving all
source bytes and operation words. Dictionary hints, paths and frames are absent.

## Request ownership and validation

Magic is `ZUFREQ01`, followed by little-endian u32 source length, operation count,
literal mode and price policy. Source bytes follow at offset24; each operation
has the original six u32 words. Total size is at most134168bytes. Source length
is0..131072; operation count1..128; literal mode0/1; policy exactly0/2. Original
operation tags, source extents, nonzero encoded offsets, match and LL bounds,
reserved zeros, dirty-cache query restrictions and update budgets are unchanged.

Original constraints allow at most8192primitive updates and4194304updated
literal bytes. For enabled fractional literals, additionally derive the initial
histogram mass from the first initialization span: sum of `1+(count>>8)` for
present symbols. Require that mass plus twice all updated literal bytes never
exceeds8388606, before any upstream call or output. This conservative transport
bound deliberately does not credit later rescaling. Sequence sums are at most
their initial mass plus8192, already below the fractional maximum. A later Rust
owner can enforce actual next-state sums transactionally rather than this
capture-specific cumulative envelope.

Each original147request is wrapped for both policies without modification. The
fractional wrapper of the original largest literal accumulation violates the
new envelope and is retained as an expected rejection; its whole-mode wrapper
remains a positive observation. Six additional source-only wrappers use an
alternating131072-byte source, initialize the first9bytes, batch32updates of
131071literals, then update29/30/31literals. The exact limit is reached at30;
the fractional31wrapper is an expected rejection. Full input values and both
negative classifications are frozen before native execution.

## Complete observations

The adapter owns separate guarded arrays of256/36/53/32initialized u32cells,
guarded complete source storage, a zeroed entropy-cost record and optState,
fixed input/operation buffers, and bounded output buffering. Pointers never
arrive from input. All rows are validated before C entry. Initial histogram
validation uses separate local counters without invoking a reference function.
Each primitive update in a batch calls the original C update function.

The unchanged rescale, refresh and scalar query calls receive the explicit policy;
the policy-independent update call is untouched. Every scheduled operation emits
all377frequencies, all sums and cached bases, predefined/dynamic mode, literal
mode, policy, and optional three-price result. Request/end records include
source fingerprint, operation count and policy; the request also pins schema and
revision. Guard/source checks retain the preceding observer's write-detection
limits; ASan/UBSan instrument every used codec unit, preserving annotations.

The new independent model reuses the qualified no-dictionary count lifecycle,
uses histogram arithmetic for literal increments, and policy-bound fractional
weights from the scalar model. Before native capture, whole-policy results must
reconstruct all13,036,992bytes of the original147-case C fixture exactly. Full
successor states are later compared with actual normal/instrumented C output.
No synthetic model transcript is described as a native observation.

## Evidence contract

Keep the complete source-only manifest, exact commands, external source hashes,
compiler/dependency/predefine logs, all objects/executables, packets and complete
stdout/stderr. Require clean pinned upstream revision and fully instrumented
common/compress/decompress units. All invalid wrappers/transport controls must
exit1 with a known bounded diagnostic and no stdout. Normal/instrumented
positive transcripts must be byte-identical and independently replayed in full.
Offline audit and forged-evidence controls run without children; capture bounds
remain explicit and full legacy assertions remain unchanged.
