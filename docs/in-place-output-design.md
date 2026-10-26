# Bounded dictionary-free in-place decompression design

This design precedes implementation. Module 28 follows completion of Module 27;
keep all predecessor production, tests and measurement sources frozen. The
candidate may be studied separately, but no in-place API is currently admitted.

## Scope and public contract

Add dictionary-free, single-pass decompression with one exclusively borrowed
`&mut [u8]`. Encoded standard/skippable/concatenated frames occupy the suffix
`buffer[input_start..]`; decoded output begins at index zero. A single owner and
validated indices express overlap. Do not accept independently borrowed input
and output slices that can refer to the same allocation.

Proposed APIs are `decompression_margin(input: &[u8]) -> Result<usize>`,
`decompress_in_place(buffer: &mut [u8], input_start: usize) -> Result<usize>`,
and allocator/options variants consistent with the caller-owned output API:
`decompress_in_place_in`, `decompress_in_place_with_options`, and
`decompress_in_place_with_options_in`. The options variants take `DecodeOptions`;
allocator variants take `A: Allocator + Clone`. The input end is always the end
of the supplied slice; callers can subslice an allocation to choose that end.

The effective output limit is the smaller of the options limit and buffer length.
The advertised-window limit remains independent. Success returns written length;
bytes outside that written prefix remain unchanged from their initial values,
including encoded bytes not overwritten by output. Failure can leave provisional
writes within the effective allowance, but never alters unread encoded bytes or
bytes beyond that allowance. Invalid initial index is rejected before mutation.
Errors do not promise native numeric codes, native precedence or transactional
rollback of already decoded blocks. Empty input remains UnexpectedEof for the
decoder; empty/skippable frames can produce zero bytes.

Dictionaries, prefixes, incremental overlapping sessions, stable input contracts,
caller-provided scratch workspaces, legacy/magicless frames and C pointer APIs
are outside this milestone. The implementation must not copy the complete input
or allocate a complete output as an overlap workaround. Existing fallible literal
and entropy scratch remains permitted and measured.

## Framing scan and margin

An allocation-free structural scan corresponds to pinned v1.5.7
`ZSTD_findFrameSizeInfo` and `ZSTD_decompressionMargin`. For each standard frame,
add its actual header length, three bytes per block and four bytes if it carries
a checksum. Add the entire size of each skippable frame. Finally add the largest
`min(window_size, 128 KiB)` among standard frames. Use checked arithmetic for
all sums, products, conversions and positions; overflow returns ResourceLimit.
The empty concatenation has margin zero, independently of the decoder's empty
input error. All-skippable input has its complete encoded length as margin.

`FrameLayout` is a plain value record describing validated byte ranges, block
count and advertised maximum block size; the scan never retains a borrowed
subslice in an owning container. A cursor walks input using validated lengths.
A framing scan does not decode literals/sequences, check content checksums or
prove that an advertised content size is truthful. In particular, structural
block length parsing and semantic block/window validation are separate steps;
retain native comparisons for structurally traversable but invalid payloads.
Do not call a successful margin calculation a successful frame validation.

The documented sufficient placement is `buffer.len() >= output_size + margin`
with the encoded input at its end and fitting inside it. Margin is a sufficient
bound for supported valid frames, not a promise that every smaller layout fails.
The decoder still enforces dynamic unread-input fences on each block rather than
trusting the margin or a caller-supplied output size. Unknown content size is
supported through those runtime checks; no guessed full-output allocation is
required. A structural preflight must not substitute for the actual decoder's
bounds, limits, content-size or checksum validation.

## Ownership and block transitions

`InPlaceBuffer<'a>` owns the unique mutable slice and index coordinates:
`frame_start <= output_position <= output_limit <= buffer.len()` and a validated
encoded cursor no later than the input end. The initial source begins at the
caller-supplied index. Header and block metadata are copied into ordinary value
records before any output operation. Never retain a cursor borrow across a
write that could overlap its consumed bytes.

For a compressed block, validate its complete encoded range, then split the
buffer at the payload start. The mutable left half contains all permitted
output/history; the immutable right half contains the still-live compressed
payload and subsequent input. Construct an ephemeral checked `SliceHistory`
view of the left half with existing output/frame positions, current window and
the smaller of the normal output limit and that split boundary. Decode with the
existing literal/sequence owners. Drop all block borrows before advancing the
encoded cursor or reborrowing the complete buffer. Earlier output remains the
history; no history clone or second output allocation is introduced.

The checked history-resumption constructor validates every position and limit
before creating its state, including `frame_start <= position <= effective_limit`.
It does not infer initialized output from malformed input: all slice bytes are
initialized Rust u8 values, and only the coordinator's validated written prefix
is admitted as history. The constructor is private and tested independently.

Raw blocks use overlap-safe `copy_within` on the single mutable slice after
validating source, output end, ordinary output allowance and the unread suffix
boundary. Copying may overlap the current raw payload but must end no later than
its consumed end, so later headers/payload/checksum bytes cannot be overwritten.
RLE first reads its single encoded value, then writes under the conservative
payload-start fence used by the pinned native frame decoder. A tiny layout may
therefore reject even when another implementation could stage that byte and
accept it. Exact-margin and smaller-placement behavior require native evidence.

At frame boundaries, output remains concatenated but match history starts anew.
Checksum reads happen after all relevant block borrows end, and checksum input
is the already written current-frame range. Skippable frames advance only the
encoded cursor. No successful or rejected operation may treat output from an
older frame as current-frame match history.

## Safe and opt-in unsafe implementations

The default overlap operation uses safe slice APIs. Give `InPlaceBuffer` its
own `unsafe-in-place-buffer` feature for the equivalent checked overlapping byte
move, using `ptr::copy` only after proving source and destination bounds,
initialized u8 storage, alignment one, exclusive allocation lifetime and allowed
overlap. No raw pointer survives the operation. Error branches must execute
before pointer construction/access and leave coordinates unchanged.

Compressed match writes reuse the independently selectable
`unsafe-slice-history`; cursor, entropy and dictionary-free existing features
remain independent. `FrameLayout` and position records are ordinary safe values
and need no artificial unsafe variant. Test the new feature separately, combine
it with slice history, and run every isolated feature and all features on the
same final source before closing the module. Allocation-free raw moves do not
imply compressed scratch is allocation-free.

## Pinned reference and qualification

Reference revision: `f8745da6ff1ad1e7bab384bd1f9d742439278e99` (v1.5.7).
Study `lib/zstd.h` lines 1536-1580, `ZSTD_decompressionMargin`,
`ZSTD_findFrameSizeInfo`, `ZSTD_decompressFrame`, `ZSTD_copyRawBlock` and
`ZSTD_allocateLiteralsBuffer`. Upstream `tests/fuzzer.c` cases beginning at
lines 1344 and 1369 cover expanded raw input and two-frame in-place decoding.
Retain original assertions and input construction when mapping those cases;
additional Rust bounds and allocator tests are separate supplements.

Native normal and ASan/UBSan helpers must actually share one allocation between
source and destination. Record layout, source placement, margin, exact output
and both outer guards. Include raw overlap, RLE, compressed literals/matches,
checksums, unknown sizes, multiple frames, leading/interior/trailing skips,
empty blocks, exact margins, smaller gaps, zero capacity, every truncation and
malformed lengths. Do not infer native equivalence from two Rust entrypoints.

Qualify scratch failure ordinals with independent nonclone allocator owners;
raw/RLE/skip paths should require no codec payload allocation. Strict-provenance
Miri must execute actual overlapping raw moves and compressed split borrows on
i686/s390x, safe and relevant unsafe builds. Include concrete thumbv6m core+alloc
IR/object generation, true no_std, strict Clippy and full feature/release tests.
Benchmarks must restore encoded bytes outside the decode clock or explicitly
report restoration cost, use identical deterministic layouts/backends, retain
executables and allocator ledgers, and independently repeat safe/all timings.
