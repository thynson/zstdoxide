# Checked frame metadata and decompressed-size bounds

This module follows persistent dictionaries in in-place output. Commit this
ownership analysis and the TODO decomposition before implementation. Preserve
all preceding frozen check/benchmark snapshots and promote only after Module 29
closes. This scope adds useful structural queries; it does not change decoding
acceptance or claim validation of compressed content.

## Evidence and scope decision

Pinned reference: facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Study
`lib/decompress/zstd_decompress.c` functions `ZSTD_findFrameSizeInfo`,
`ZSTD_findFrameCompressedSize`, `ZSTD_findDecompressedSize`,
`ZSTD_decompressBound`, `ZSTD_isSkippableFrame`, and `ZSTD_readSkippableFrame`.
Existing `FrameLayout` already traverses complete frame ranges for margin
calculation. Share its checked traversal with metadata queries instead of
creating a second parser with different malformed-range handling.

A prefix-only one-shot alias is intentionally not the next module: the native
`ZSTD_getDDict` consumes use-once selection once per `ZSTD_decompressDCtx` call,
then `ZSTD_decompressMultiFrame` reuses that dictionary for every frame in the
call. This differs from frame-based streaming prefix consumption. A reusable
one-shot context needs its own ownership/lifecycle design and must not be
implied by a stateless alias or silently inferred from the streaming contract.

## Owned values, borrows and invariants

Add a public plain `FrameInfo` value holding a `FrameKind`, encoded size, and
block count. `FrameKind` distinguishes a standard copied `FrameHeader` from a
skippable variant and payload size. Values own no input bytes or allocations.
They store validated sizes/metadata, not native pointers. Private construction
or documented read-only accessors prevent callers from confusing arbitrary
mutated metadata with parser-validated facts.

Extend the private frame-layout result to preserve its already parsed header or
skippable metadata. The existing cursor proves every read and encoded extent
before constructing a result. On success, encoded size is nonzero and at most
input length; standard frames contain at least one structurally valid block
header and any declared checksum bytes. Reserved block types are rejected.
The parser deliberately does not execute literals/sequences, validate checksum
values, or prove advertised content-size truth. Existing oversize block/window
structural behavior remains consistent with the margin scanner.

A skippable payload query returns an immutable borrow within the supplied input,
plus its 0..15 magic variant. It never copies, allocates or adopts payload storage.
An incomplete length or payload is rejected before returning the borrow. A magic
predicate may inspect four bytes only; it must explicitly distinguish recognizing
the marker from validating the complete frame.

These records and immutable borrowed views have no independent raw-memory
operation and share one representation in safe/unsafe builds. Their hot traversal
uses the existing separately selectable byte-cursor backend. Do not introduce an
artificial unsafe feature for scalar arithmetic or metadata representation.

## Public query contracts

- `frame_info(input)` scans the first complete standard/skippable frame and ignores
  trailing input. Its encoded size is suitable for advancing a checked slice.
- `frame_compressed_size(input)` returns that first encoded extent without decoding.
- `decompressed_size(input)` scans the entire exact concatenation and returns
  `Some(total)` when every standard frame advertises a size, or `None` if any
  standard size is unknown. Skippable frames contribute zero; empty input is
  `Some(0)`. Trailing garbage/truncation is rejected even after an unknown-size
  frame. Keep errors distinct from unknown size instead of reserving u64 values.
- `decompression_bound(input)` scans all frames. A known advertised content size
  is that frame's bound; an unknown-size standard frame contributes block count
  times `min(window_size, MAX_BLOCK_SIZE)`. Skippable/empty input contributes zero.
  The bound is meaningful for semantically valid supported frames only, and is
  not an instruction to allocate an untrusted advertised amount.
- `is_skippable_frame(input)` recognizes a four-byte marker only.
- `skippable_payload(input)` returns the validated first payload and magic variant;
  trailing input belongs to subsequent frames and is not copied.

All count conversion, multiplication and concatenated addition are checked.
Accumulation uses u64 before multiplication so 32-bit hosts can report bounds
above 4 GiB for small encoded streams. Arithmetic overflow is `ResourceLimit`.
Both complete size queries validate every structural extent, including unknown
size inputs; they do not inherit native early-return shortcuts. Existing header
parsing admits structurally representable u64 sizes without native reserved
sentinel values and platform window-log caps. Native differences in sentinel,
window cap, early unknown return and wrapping aggregate bounds must be explicitly
captured, not called matching results. Legacy and magicless frames remain outside
this scope. Successful scans do not guarantee successful decompression.

## Verification and original assertions

Unit tests cover all skippable variants, zero/large payload boundaries, standard
header widths, exact first-frame extent and trailing bytes, known/unknown mixed
concatenations, checksummed structural extent, reserved block types, truncation
at every encoded boundary, u64 overflow and a >4 GiB bound on a small stream.
Add lifetime compile-fail documentation for the borrowed payload and prove no
codec payload allocation with structural queries on caller storage.

Build an external normal and ASan/UBSan v1.5.7 helper for exact structural query
comparison, keeping real error/unknown/sentinel identities separate. Reproduce
the original `test_decompressBound` examples including the `(1 << 15) + 1` flushed
blocks and retain full provenance. Original multi-frame size assertions must use
their actual preceding fuzzer input/frame history before their status is upgraded.
Generated malformed/small-boundary cases remain supplemental. Do not upgrade
compression-bound, context-reset or unrelated metadata groups merely because a
new query exists.

Run focused tests, all isolated features/all/safe full and release regression,
true no_std and strict Clippy. Use i686/s390x Miri for bounded structural cases,
plus actual thumbv6m core+alloc builds and concrete output. Keep the long >4 GiB
logical-bound case in host and i686 execution with a smaller Miri boundary set
if interpretation cost is prohibitive; state precisely which checks ran.
Benchmark deterministic structural scans on identical small/large/concatenated
inputs, including no-entropy traversal. Retain all backends and independently
rebuild/repeat safe/all after actual CPU barriers. Only then reconcile evidence,
clear the completed checklist and decompose the next meaningful contract.
