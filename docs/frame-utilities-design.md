# Compression size bounds and skippable serialization

Module 31 follows checked structural queries. Record this design and its TODO
before implementation; keep preceding source snapshots frozen and promote only
after Module 30 closes. The utilities allocate nothing and introduce no context.

## Reference and scope

Pinned facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99` defines `ZSTD_MAX_INPUT_SIZE` and
`ZSTD_COMPRESSBOUND` in `lib/zstd.h:248-249`, implements `ZSTD_compressBound` in
`lib/compress/zstd_compress.c:69`, and writes skippable frames with
`ZSTD_writeSkippableFrame` in that file at line 4751. Preserve attribution for
the bound constants/formula and skippable magic/header representation.

Expose `compression_bound(source_size: usize) -> Result<usize>` with the pinned
one-pass formula. Reject the native width-specific maximum and larger sizes,
including usize::MAX, as ResourceLimit rather than returning zero or encoded
native errors. On supported 32/64-bit targets the maximum is the repeating
0xff00 pattern; a width-independent construction from usize::MAX avoids invalid
large literals on narrower targets. Check each addition even after admission.
The small-input margin switches at 128 KiB and decreases at 2 KiB steps.

The bound covers the ordinary one-shot compress/compress_in and dictionary
variants: they emit blocks no larger than the original input plus three-byte
headers at 128 KiB boundaries, the largest frame header and checksum. Public
reference one-shot profiles use blocks of at least 1 KiB; verify their raw-block
fallback overhead also fits before claiming them. Do not claim this bound covers
arbitrarily flushed streaming output, multiple independent frames or caller-
chosen tiny blocks. It is a size estimate, not an allocator/workspace requirement
or a promise that allocation/compression will succeed. Keep encoder reservation
policies unchanged in this module.

## Borrowed output and error invariants

Expose `write_skippable_frame(output: &mut [u8], payload: &[u8], variant: u8)
-> Result<usize>`. One exclusive borrow owns output access for the call; payload
is an independent immutable borrow. Safe Rust forbids overlapping these borrows.
The result is payload length plus eight bytes. The first four bytes encode magic
0x184d2a50 + variant and the next four encode the u32 little-endian payload length.
The payload is copied exactly once; all bytes after the returned extent remain
unchanged. No input or output reference survives the call.

Validate variant, u32 payload representability, checked usize header addition and
actual destination capacity before any write. Invalid variant is InvalidParameter;
length overflow, unrepresentable payload or insufficient capacity is ResourceLimit.
Every error leaves the entire output unchanged. This explicit Rust precedence can
differ from native's destination-first checks when multiple arguments are invalid;
record it rather than pretending native error-code parity. The internal checked
length helper is independently tested at u32/usize limits without fabricating
impossible huge slices or passing undersized native pointers.

These are scalar arithmetic and transient slice serialization, with no independent
owned performance-sensitive data structure. Scalars, arrays and slice copies share
one safe representation in all feature selections. Use standard checked slice
copy operations, not an artificial unchecked backend. Existing separately opted
codec structures retain their safe/unsafe implementations. Keep this module
core-only and forbid unsafe code; no allocator or filesystem dependency is needed.

## Verification

Port the original test_compressBound powers 0..29 and too-large-source assertion,
plus zero, small-margin transitions, 128 KiB, pointer-width maximum and checked
addition boundaries. Check the raw-framing upper-bound inequality with independent
wide arithmetic over all small inputs and representative large boundaries. Use
actual compress/dictionary/reference outputs on deterministic repetitive and
incompressible inputs to validate any documented encoder scope.

Test all 16 skippable variants, empty payload, real payload sizes around header,
page and block boundaries, exact/short/extra capacity, all error-before-write paths,
borrow-lifetime/alias rejection, unchanged suffix and roundtrip through metadata
and decompression. Compare actual encoded bytes and genuine guarded destination
layouts with external normal/ASan+UBSan native helpers. Capture original writer
assertions only with their actual preceding input history when required. Generated
layout cases remain explicitly supplemental. Never allocate gigantic buffers or
invoke native code with invalid pointer extents merely to exercise numeric limits.

Qualify safe/cursor/all focused checks, all isolated feature/full/release checks,
no_std and strict Clippy. Use actual i686/s390x Miri and core-only thumbv6m artifacts
for width/endian and rejection-before-write behavior. Measure identical arithmetic
and slice-writing workloads separately, retaining all backends, guard/result
qualification, equal clock overhead and independent safe/all repeat after actual
CPU barriers. Distinguish sub-clock-scale arithmetic costs from broad performance
claims. Reconcile original coverage only after verified evidence is integrated.
