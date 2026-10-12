# Ordinary Greedy helper review before first execution

The coordinator inspected `scripts/oracles/greedy_frame.c` against the frozen
ordinary-frame protocol and pinned facebook/zstd v1.5.7 source on 2026-10-12,
before executing any helper mode. Reviewed helper SHA256:
`6fe496e3323a994b95923813c73738b055efd835b6c4210bbbc294af790e0b4c`.
Strict C11 syntax checks pass; no ordinary compression, decoding or sanitizer
runtime result is claimed here.

This helper adapts the project's frozen ordinary DoubleFast observation
scaffolding. It invokes the unchanged library and copies no compressor/parser
algorithm. The CLI keeps `frame CASE INPUT SCHEDULE OUTPUT WHOLE_OR_DASH W H C S M T`,
`header PLEDGE W` and `metadata`. The new metadata identifies Greedy strategy 3
and schema 1; parameter arrays use W/H/C/S/M/T/strategy consistently with the new
source manifest. Old helper sources and expectations remain unchanged.

## Selected owners and initialization

Explicit parameter admission precedes context allocation. Chain parameters are
bounded by W<=14, H<=19, C<=18 and S<=8; rows use W15..21, H<=19, C<=29 and S<=29.
M3..7 is clamped to keys4..6 by the upstream Greedy dispatcher. Automatic row
selection must resolve exactly from W, and applied/match-state cParameters must
equal every requested scalar. Dictionary, worker, LDM, external-sequence,
checksum and splitting controls are checked disabled before observations.

`ZSTD_allocateChainTable` (`zstd_compress.c:255`) excludes row search. Both the
workspace estimator and reset function use that decision before shifting the
chain log. Thus row C29 does not request 2^29 chain cells. Its zero-byte reserve
may return a nonnull cursor: this helper never dereferences or copies that
inactive pointer. Chain mode instead checks two nonoverlapping U32 owners.

The fresh dynamic context is zeroed by `ZSTD_initCCtx` at line 105. This proves
that all eight row cache cells are initialized even before a first parser call.
`ZSTD_reset_matchState` clears the hash/link table regions and reserves initialized
row tags using `ZSTD_cwksp_reserve_aligned_init_once`. The helper checks actual
table extents against workspace `objectEnd`/`tableValidEnd` or
`initOnceStart`/`workspaceEnd`, using integer bounds before any table observation.
Both full selected extents are disjoint. Rows additionally require 64-byte hash
alignment, row-width tag alignment and exact `rowHashLog`. The first salt and
zero entropy must match the earlier independently captured fresh-context values.

The sampled-state record is zero initialized before copying nextToUpdate and
skipping; only row mode reads salt, entropy, rowHashLog and cache fields. Chain
records normalize these inactive fields to zero. Record padding is initialized,
so zero/tiny sampling comparisons do not examine indeterminate bytes. No pointer
addresses enter the output.

## Helper allocations and source lifetime

Seven guarded owners are live during scheduled frame capture:

| Owner | Payload bound |
| --- | --- |
| Source | N+64+262656 initialized bytes; N<=4 MiB, source offset64. |
| Source copy | The same complete payload, including all initialized slack. |
| Scratch output | 2 MiB, with a conservative per-call entropy/literal/sequence bound checked first. |
| Complete frame | N+3*(positive calls+1)+maximum header/checksum bytes, checked before each append. |
| Independent replay | max(N,1), initialized before overlap-aware copying. |
| First before-copy | 4*2^H bytes, H<=19, for heads or row positions. |
| Second before-copy | Chain: 4*2^C, C<=18; row: 2^H byte tags. |

Every owner adds two 64-byte guards outside the full allowed initialized extent.
The largest persistent helper bound is 22,546,649 bytes at N=4 MiB, 64 positive
calls, H19 and chain C18. It is a conservative domain bound, not an observed
allocation total. Context/workspace owners are separate. Decoding temporarily
adds max(N,1)+128 bytes; aligned whole-buffer comparison adds its checked
compressBound output, a fresh context and decode buffer. All successful-path
temporary/persistent owners are released. Fatal checks exit the process; this
does not claim error-path leak instrumentation.

Every helper addition/product is checked before allocation. Before-table buffers
keep natural U32 alignment; tag copies require only byte alignment. Native U32
fingerprints are serialized byte by byte in little-endian order; byte-tag
fingerprints retain one byte per entry. Cell comparison uses width-specific
loads. The source allocation remains live and immutable for the entire schedule.
All raw indices fit below 4 MiB+2, so index reduction is excluded.

Greedy's eight-byte physical reads, row cache lookahead, tiny limit formation and
wildcopies remain inside the initialized leading/suffix allowance. All logical
stored keys must fit consumed input using the effective width4..6. The runtime
window check verifies source-minus-two base, contiguous nextSrc, no attached
dictionary and no overflow corrections after each positive call. Row positions
and chain links are source indices, not dereferenced C pointers from output.

## Observations and independent checks

The helper verifies initial zero owners, copies each selected table before each
call and emits full fingerprints plus sorted deltas afterward, including zero
calls. It checks every nonzero index is >=2, below nextToUpdate and has a complete
logical key. Heads must hash to their bucket; row positions/tags must hash to
their row/tag, reserved positions stay zero, heads are in range and occupied,
and empty usable positions retain zero tags. Old links below active window floors
remain permitted. Deltas are final-state comparisons, not an insertion trace.

Zero Continue returns before window/table work in `zstd_compress.c:4819`. Tiny
positive calls return from `ZSTD_buildSeqStore` before reset, frontier adjustment
or parsing. The helper checks unchanged sampled maps/state on both paths and
preserves stale sequence-store generation identity. Ordinary long-gap adjustment
at lines3292..3297 applies to both families; it is deliberately left to the
unchanged C call and captured in before/after progress, not emulated here.

Only initialized previous or freshly produced pending compressed-block owners
are snapshotted. Sequence/literal extent checks precede reads; raw LL/ML-base,
expanded lengths, offBase/distance and long markers are retained. Independent
wire-repeat interpretation and bounded overlap replay reconstruct each candidate.
Publication swaps owners only for compressed output. Raw/RLE keeps sampled state
while retaining committed repeat/coding entropy, including the documented
offset-tag demotion exception. Row salt entropy is recorded separately.

Completed frames are decoded and byte-compared by the unchanged library. Marked
aligned schedules additionally compare complete fresh one-shot block payloads,
accounting for terminal empty blocks/final flags. These operations are specified
but unexecuted at this review. Header-only probes allocate no pledged source and
are explicitly incomplete/undecoded.

Exclusive output creation prevents overwriting an existing file. The separately
planned driver must also reject path overlap before creating its output tree,
bound stdout/stderr/time, preserve failures and verify all pinned source/header
identities. Full normal/ASan/UBSan runtime coverage requires the later capture to
instrument every used translation unit; syntax checking here is not that claim.
