# Bounded streaming encoder design

Reference: facebook/zstd v1.5.7 `ZSTD_CCtx`, `ZSTD_compressStream2`,
`ZSTD_compressBlock_internal`, `ZSTD_writeFrameHeader`, and the public contract
in `streaming-design.md`. Rust retains its independent greedy block strategy.

## Ownership and decomposition recorded before implementation

First extract a `BlockEncoder` owning optional indexed match storage and
predefined inverse entropy tables from the one-shot function. The existing
single-segment frame header, block choices, sequence order, checksums, and exact
one-shot encoded bytes must remain unchanged. Subsequent workspace reuse keeps
literal, sequence, bitstream, and compressed-payload allocations with this
block encoder rather than allocating them for each block.

The streaming `Encoder` owns validated `EncoderOptions`, one input block, one
pending encoded block/header/checksum vector and its drain index, a reusable
`BlockEncoder`, incremental `XxHash64`, and scalar progress/phase records. Input
slices are borrowed for one call; no pointer or caller reference is retained.
Input staging is bounded by `min(1 << window_log, 128 KiB)` independently of total
frame size. Pending output is one encoded block plus bounded format overhead.
Owned capacities may retain their previous maximum across resets, always within
the module's fixed block/workspace bound. Existing optional unsafe access
families retain their proofs; plain encoder state receives no artificial unsafe
variant.

`Continue` accepts bounded input and emits complete blocks. `Flush` and `Finish`
latch their directive and the count of input bytes provided with that operation.
Resumption must use the same directive and may supply at most the still
unconsumed count; empty calls can drain output. No completion status is returned
until all latched input is accepted and all pending output is drained. Flush
emits non-final blocks; finish emits a final block (empty when necessary) and
optional checksum. A processing error poisons the state until reset.

An empty `Finish` after `FrameFinished` is idempotent, returning the same boundary
with zero counts. All other calls after completion fail with `InvalidState`.
`reset()` retains window/checksum/content-size flags and reusable allocations but
clears the per-frame pledge. `reset_with_options(options)` validates before
changing any state; invalid options leave the existing context untouched.

The stream header is non-single-segment. Window logarithms are 10..=31, default
17. Checksum defaults on. A supplied `pledged_size: Option<u64>` is enforced even
when `include_content_size` is false. Content size is emitted only when both a
pledge and the emission flag are present; tiny known sizes use the valid four-
byte non-single-segment content-size field. Actual accepted/emitted byte counts
are checked `u64` values. `retained_capacity()` reports owned heap capacity in
bytes, excluding inline fields and caller buffers.

Work items:

1. Extract reusable block state and verify exact pre-refactor one-shot bytes.
2. Reuse bounded block scratch; retain existing safe/unsafe access contracts.
3. Implement validated options, partial header/output drainage, input staging,
   checksum, latched continue/flush/finish, pledge enforcement, and reset.
4. Test fragmented input/output, empty drainage, flush boundaries, checksums,
   window/content-size fields, short/long pledges, poisoned recovery, idempotent
   finish and retained-capacity stability; run focused no_std/backend checks.
