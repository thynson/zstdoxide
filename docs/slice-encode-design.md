# Caller-owned one-shot compression output

Module 33 follows call-scoped decoding selection. This design and the work items
precede implementation. Promote production changes only after Module 32 closes.
The initial scope is dictionary-free ordinary compression with the existing
Greedy parser, checksummed single-segment frame and known source size. Dictionary,
reference-level, streaming, prefix and reusable encoder selection are separate.

## Reference and existing engine

Pinned facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99` exposes `ZSTD_compress` in `lib/zstd.h`
and implements bounded output through `ZSTD_compressCCtx`/`ZSTD_compress2` in
`lib/compress/zstd_compress.c`. Its destination capacity is a real output bound,
not a requirement to reserve compressBound before attempting compression. Exact
compressed bytes and ratios depend on the independently implemented encoder and
are not expected to match C.

The Rust `compress_frame_in` currently allocates one output Vec sized for raw
fallback, then emits a header, bounded blocks and checksum. `BlockEncoder` owns
match/history/entropy scratch; its `encode` method writes one block into a Vec.
Reuse that block algorithm unchanged. Stage at most one encoded block (including
its three-byte header) in a fallible scratch Vec, then copy that block once into
caller storage. Reuse and clear the staging Vec between blocks. Do not implement
the API by building a complete compressed frame and copying it afterward.

Refactor header serialization only as needed to share its existing exact bytes
between allocating and borrowed output. A small initialized header record or the
same bounded staging Vec may hold the header. Preserve existing one-shot output
byte-for-byte, including empty-frame encoding, content-size field thresholds and
checksum. Existing Vec-returning APIs keep their allocation/ownership contracts.

## Output owner and invariants

Introduce a private `FrameSink<'a>` owning one exclusive initialized output slice
and a written position. Its invariant is position <= output.len(). `append`
checks position + source.len() for overflow and destination capacity before any
write. A rejected append changes neither position nor destination. Successful
append copies the entire source exactly once and advances position. Prior
appends remain visible if a later operation fails. No source reference survives
the append; no buffer allocation is owned by this record.

This is an independently performance-sensitive bounded-copy structure. Provide
equivalent safe and opt-in `unsafe-frame-sink` implementations with the same public
contract. Default uses checked slice copies. Any unchecked copy must be preceded
by retained runtime checked arithmetic and capacity validation. Its nearby SAFETY
proof covers initialized source/destination, u8 alignment, valid extents, exclusive
mutable borrow, non-overlap supplied by safe independent borrows, and lifetimes.
Never derive safety solely from input validity or debug assertions. Empty appends
must be valid without manufacturing out-of-range pointer offsets. The scalar
position itself needs no artificial unsafe representation.

The staging Vec owns its supplied allocator clone and contains only initialized
encoded bytes. Its length is cleared between emissions; its capacity can be
retained up to the raw block bound and allocator growth policy. Scratch owner
clones may differ; each allocation is freed through its exact owning allocator.
Existing BlockEncoder, entropy, match and ring storage remain independent owners
with their already selected safe/unsafe features. The new sink does not replace
those implementations or introduce retained full-frame history/output copies.

## Public contract

Expose `compress_into(input: &[u8], output: &mut [u8]) -> Result<usize>` and
`compress_into_in(input, output, allocator)` with `A: Allocator + Clone`. The
Global wrapper delegates the generic implementation. A borrowed non-Clone
allocator can be supplied by reference. Input/output borrows are independent;
in-place compression is not supported. Neither borrow nor allocator is retained
past the call. All scratch uses fallible allocation; ordinary allocator cloning
retains caller-provided panic/allocation behavior.

Success writes exactly the same single frame as `compress(input)` and returns its
length, preserving every output byte after that length. A destination smaller
than compression_bound may succeed when the actual frame fits. A short output
returns ResourceLimit and can contain a provisional header or earlier complete
blocks. Errors never write outside the supplied slice; do not promise whole-call
atomicity. Each append is checked before mutation, including checksum emission.
Allocation failure returns AllocationFailed, releases all scratch and preserves
outer guards. No retained reusable encoder state is exposed, so a failed attempt
cannot poison a later independent call.

A valid compression_bound is a sufficient worst-case output size for this stated
one-pass framing if scratch allocation succeeds; it is not mandatory capacity or
a scratch estimate. Keep the default block size and selection policy unchanged.
Do not claim native capacity-error precedence, compressed-byte identity, arbitrary
flush bound, dictionary attachment or reference-level support for this new API.

## Qualification before closure

1. Check sink zero/exact/short appends, overflow arithmetic via numeric helpers,
   unchanged failed append and untouched tails. Run equivalent safe/unsafe cases.
2. Compare whole output byte-for-byte with the existing allocating encoder for
   empty, header thresholds, raw/RLE/compressed blocks, block boundaries and
   multiblock inputs. Verify exact-capacity success and smaller-capacity errors,
   including a compressible case that fits below compression_bound.
3. Inject every observed scratch allocation failure and verify owner release,
   outer guards and recovery on a fresh call. Distinguish destination ownership
   from block staging and existing encoder workspace allocation ledgers.
4. Use external normal/sanitized v1.5.7 helpers for complete bidirectional decode
   interoperability and genuine bounded destination guards. Keep each encoder's
   own actual output size/capacity contract separate. Preserve original test
   history before upgrading any upstream group; generated cases are supplemental.
5. Qualify foreign i686/s390x interpretation and actual no-atomics ARM emission,
   every isolated feature including unsafe-frame-sink, all/default, no_std,
   strict Clippy, formatting and complete release suites on frozen sources.
6. Measure allocating versus borrowed output on identical deterministic inputs,
   with caller output setup outside clocks and all per-call scratch inside. Retain
   complete output and independent allocator ledgers, every sample, source-bound
   executables and an independent safe/all repeat after actual CPU barriers.
7. Reconcile the bounded scope and inventory, then decompose dictionary-backed
   caller-owned compression or the next evidenced missing contract separately.
