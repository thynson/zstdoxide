# Successor public Greedy adapter review before C execution

The completed adapters implement `reference-greedy-interop-protocol.md`. The C
helper has passed `cc -std=c11 -Wall -Wextra -Werror -fsyntax-only` against the
external pinned headers with Apple clang 17.0.0. No new C executable has run at
this checkpoint. The two new adapter sources differ from the reviewed Module 13
sources only in their diagnostic labels, descriptive title and transport magic;
an exact normalized-source comparison confirms this. Historical sources remain
unchanged.

| Source | SHA256 |
| --- | --- |
| `scripts/oracles/reference_greedy_frames.c` | `e49b52366148ab6c1ee19f150cdade770c72318b391c7fce482b1dca09f7430d` |
| `examples/reference_greedy_oracle.rs` | `b982e84fe2df72dcb4ec0b5131d0fa1f88e6dd7c5c0fe29dc17559841da47f7c` |
| `scripts/oracles/greedy_interop_protocol.py` | `cbf5c0763f9c1fbd230f988f2b01c517f6af22f58c4645239303a4c75b66b344` |

## C ownership and bounds

The helper owns three independent nonzero `malloc` regions: source, compressed
input and output. Each capacity is positive, at most 5 MiB and at most
`SIZE_MAX - 128` before guard-size addition. The entire region is initialized
to 0xa5 before `data = base + 64`; both guard regions remain in that same owner.
Even a zero logical input has a one-byte live payload. No source/output/frame
owner overlaps another, and no buffer pointer escapes its allocation lifetime.

All 56 header bytes are read before scalar access. Twelve four-byte reads start
at `8 + 4*i` with `i < 12`, ending at byte 55. Individual unsigned-byte loads
avoid alignment and integer aliasing requirements. Mode, payload agreement,
source/frame caps and scalar domains precede allocation and payload reads.
Signed level decoding preserves `INT_MIN` without an out-of-range signed cast;
the helper explicitly requires 32-bit `int`. Exact EOF rejects trailing input.

Compression checks `compressBound` for errors and a positive capacity no larger
than 5 MiB. A fresh opaque `CCtx` owns its tables; it is freed after `compress2`
returns, while every supplied buffer is still live. The helper uses public
parameter setters and the actual pledged size, with no private pointer seeding,
context copying or forced unresolved row setting. C encode requests will come
only from the frozen admitted recipe set. The packet bound alone does not cap
all opaque workspace or CPU usage for arbitrary valid parameter combinations.

Before decompression, `getFrameHeader` must return zero before any header field
is inspected. Metadata checks reject skippable/unknown-size frames, dictionaries,
checksums, a mismatched known size and windows above 2 MiB. A successful
`findFrameCompressedSize` must equal the complete supplied extent, rejecting
concatenation and trailing skippable data. Output owns `max(source_size, 1)`
bytes, while the API receives its true logical capacity. Only a successful
decode of exactly the expected size reaches `memcmp` over initialized bytes.

Six guard checks remain within their original owners. Serialization checks the
output length against the allocated capacity and u32 wire extent before reading
it. Successful paths free each original base exactly once. Fatal errors exit
the short-lived process; this is not an assertion of explicit per-error C
allocator balance. The conservative 13,631,872-byte explicit buffer/guard bound
excludes opaque workspace, stdio and sanitizer overhead.

## Safe Rust and checked transport

The Rust example forbids unsafe code, reads at most the maximum packet length
plus one and checks the complete header before fixed indexing. It checks both
payload caps before adding lengths or borrowing indexed slices. Both modes
validate public parameter setters. Encoding verifies the unchanged request and
reports its separately resolved tuple even for admission rejection. Complete
single-frame metadata/extents are checked before returning encoded bytes or
decoding input; the decoder receives explicit output/window limits. All returned
bytes have owned vectors and no packet borrow escapes.

The Python reader checks exact bounded response lengths, valid statuses and
zero C scalar fields. It imports only the identity-pinned independent outer
frame reader. New magic is `ZGOI0001` / `ZGOR0001` / `ZGOC0001`. The 39 negative
packets preserve the prior 38 malformed-boundary tests and add explicit rejection
of old `ZDOI0001` transport. Sixteen malformed wire/record controls reject in
normal and optimized Python with child creation blocked and zero attempts.

## Pre-C result

Both safe and all-feature Rust adapters execute all 422 fixed requests: 372
success, 37 unsupported and 13 resource errors. All 251 historical resolved
tuples remain exact. Both builds produce identical statuses, tuples and complete
frames; 744 self-decodes reproduce every expected source byte. All 25 frozen
equivalence groups pass. Each build rejects all 39 malformed packets with exit 2,
a diagnostic and no response. The 1,666 bounded Rust commands, source hashes,
executable hashes and drivers are preserved in the preflight evidence archive.
Focused safe/all Clippy passes. These are Rust and C-syntax results only.

The next campaign must compile all 26 used C units and this helper in normal
and ASan/UBSan builds, keep assembly/legacy/workers disabled, record retained
upstream sanitizer annotations and disabled LeakSanitizer, and independently
replay full command, source, response and decoded-byte evidence.
