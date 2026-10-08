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
Input fills any available staging space before pending output is drained. This
preserves the upstream zero-output contract: `Continue(b"aa", [])` accepts both
bytes, and a subsequent empty `Finish` can wait for output capacity without
requiring the caller to retain the already accepted source.
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

## Implemented workspace bounds

Let `B = min(1 << window_log, 131072)`. The input staging vector holds at most
`B` bytes; the pending vector holds at most a header (14 bytes) or one encoded
block with its three-byte header and optional four-byte checksum (`B + 7`).
Reset retains capacities but clears all frame-local counters, staging lengths,
checksum state, phase, and the latched directive.

The shared `BlockEncoder` owns the following reusable allocations:

| Allocation | Bound and ownership | Upstream role |
| --- | --- | --- |
| Match positions | 65536 initialized `usize` entries; previous positions are block-local indices | `ZSTD_matchState_t` hash table |
| Literals | At most one input block, owned bytes | `seqStore_t` literals |
| Sequences | At most `B / 4` typed records because every greedy match consumes at least four bytes | `seqStore_t` sequences |
| Inverse entropy tables | Fixed predefined distributions; initialized transition and initial-state vectors | Predefined `FSE_CTable` distributions |
| Sequence bitstream | At most 66 bits per sequence plus 18 final-state/end bits, with bounded vector growth | `BIT_CStream_t` storage |
| Candidate payload | Literals plus sequence bitstream and at most seven section-header bytes | Encoded compressed-block workspace |

Every block resets match positions and scratch lengths. Only the predefined
tables survive as logical state. The safe and opt-in match/FSE/bitstream
implementations keep their existing contracts; sequence records, options,
latched byte counts, and phase records use a shared safe value representation.
No allocation depends on the number of blocks already emitted. The low 32 bits
of incremental XXH64 cover exactly the accepted source bytes, including across
flush boundaries.

## Verification evidence

Extraction and scratch reuse each preserved all 48 complete pre-refactor
one-shot frame byte strings (16 size boundaries, random/periodic/RLE inputs).
The reusable-block test alternates different blocks and confirms both exact
encoded bytes and stable retained capacity. Eight streaming unit tests cover
zero-output input acceptance; waiting for the last checksum byte; every content
size field boundary, including a `u64::MAX` header; window endpoints; smaller
latched input fragments and empty drains; directive/extra-input rejection;
pledges omitted from the wire; failed option changes; reset; and checksum
corruption. Safe, all-feature, release all-feature, and `no_std` builds pass
their focused checks, as does strict Clippy for the library and integration
tests with all features.

A separate temporary Rust driver generated 192 streaming frames for the pinned
official v1.5.7 CLI to decode. Inputs covered random/periodic/RLE bytes, lengths
0, 1, 255, 256, 1024, 4097, 131072, and 262161; windows 10 and 17; checksum on/off;
pledged/unknown lengths; content-size omission; flush/continue before finish;
and output slices of 1, 31, or 4096 bytes. Every decoded result exactly matched
its source. The largest retained encoder capacity observed in this workload was
1,481,805 bytes. This is a measured workload value, not an allocator-independent
maximum or a claim of parity with upstream compression levels.
