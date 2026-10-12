# Broader public-frame adapter review before C execution

Reviewed against `reference-double-fast-interop-protocol.md` after implementation
and before executing the new C oracle. The native C compiler has checked syntax
with `-Wall -Wextra -Werror`; no new C helper executable has been run here.

| Owned source | SHA256 |
| --- | --- |
| `scripts/oracles/reference_double_fast_frames.c` | `dca0eb63b4eed76324de5d4ae9134b0f7dc7a5a7868479fa7eb9bec310171b2c` |
| `examples/reference_double_fast_oracle.rs` | `ed455584173111e8186b41e7eec16645fa6c267828a0935831e74196ba87895b` |
| `scripts/oracles/double_fast_interop_protocol.py` | `af9562efe67846530ab364ce6c97388af7f407147040b67e330643517e5ad543` |

## C allocation, initialization and lifetime

The finished helper has exactly three explicit `Buffer` allocations. Every
capacity is positive, at most5MiB, and checked against `SIZE_MAX - 128` before
addition. `malloc` succeeds before any pointer arithmetic. Each entire region
is initialized with0xa5, then `data = base +64` leaves64 leading guard bytes and
`capacity +64` remaining bytes. The source and compressed-input allocations are
at least one byte even when their logical extents are zero. Independent nonzero
allocations establish nonoverlapping live source, frame and output regions.

The56-byte stack header is fully read before `memcmp` or any scalar load.
Twelve scalar reads start at8 +4*i for0 <= i <12 and read exactly four bytes,
so the last byte is55. `word` uses individual unsigned-byte loads and u32 shifts;
it has no unaligned integer dereference or aliasing cast. Mode, source/frame
caps, encode/decode agreement and all scalar override domains are checked before
buffer allocation. C requires32-bit int. For a negative level wire value,
`UINT32_MAX - value` fits `INT_MAX`, and `-1 - converted` stays in the signed
32-bit range, including `INT_MIN`.

`read_exact` receives only the already bounded logical source/frame extents.
The following EOF check rejects extra input. Encoding checks `compressBound`
for errors, positivity and5MiB before constructing output. The fresh context
owns all opaque compression storage and is freed after `compress2` finishes,
while every input/output buffer remains alive. No pointer is retained or read
from the freed context. Decoding checks complete frame metadata and exact frame
extent first; destination allocation is `max(source_size,1)` and the API receives
the true source-size capacity. `memcmp` occurs only after successful decoding
returns exactly that length. Even zero-length calls receive valid live pointers.

`verify_frame` reads `ZSTD_frameHeader` fields only after `ZSTD_getFrameHeader`
returns zero, which establishes a complete initialized header. It rejects
skippable frames, windows above2MiB, unknown/mismatched content size, nonzero
dictionary ID and checksum. `findFrameCompressedSize` errors are checked before
requiring exact equality with the supplied initialized extent. Concatenated or
skippable tails therefore cannot pass as one frame.

All six guard loops stay within their original complete allocations. Encoded
output length is checked against its capacity; decoded length equals the
bounded source size. Serialization consequently reads only initialized output
and separately checks the u32 response extent. Successful paths free the three
original bases exactly once. Rejected input or codec errors terminate this
short-lived process; no per-error explicit allocator-balance claim is made.
The conservative13,631,872-byte buffer/guard bound excludes opaque C workspace,
stdio buffering and sanitizer runtime, as stated in the protocol.

## Safe Rust and independent wire validation

The Rust example forbids unsafe code. Its input reader stops at maximum packet
length plus one; fixed-header indexing follows the56-byte check. It validates
source/frame bounds before adding their lengths or borrowing either slice.
Both modes validate every parameter setter. The original request is copied
and checked after encoding, while its resolved tuple is reported independently
of the resulting admission status. Metadata and the checked single-frame block
walk run before serializing encoded frames or decoding supplied frames.
The decoder receives explicit source-size and2MiB window limits. Returned
vectors own their bytes and no input borrow escapes.

The Python protocol reader uses bounded little-endian fields and validates exact
response lengths/statuses. C scalar fields must be zero. Its outer-frame reader
is the immutable independently implemented Module12 reader, checked by identity
before import; no historical C driver or admission execution is invoked there.
The38 negative packets exercise header/mode/extent agreement, both modes' scalar
validation, bounded-reader overflow, metadata, single-frame boundaries, reserved
blocks, truncation and a valid-frame/wrong-source comparison. They contain only
ordinary initialized bytes, never invented pointers.

## Pre-C validation result

Safe/all Rust each executes all159 fixed requests:113 success,42 unsupported,
4 resource errors. Every admitted frame self-decodes exactly, both builds agree
on status/tuple/full bytes, and all16 equivalence groups pass. Both builds reject
all38 malformed packets with exit2 and no successful response. Focused Clippy,
formatting, no_std compilation and the existing Fast/named baselines pass.
These are Rust and C-syntax results only. The next capture must compile all26
used C units with instrumentation, verify all source/dependency identities,
repeat negative packets in both C builds, and preserve its full command evidence.
