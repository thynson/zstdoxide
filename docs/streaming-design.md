# Bounded streaming and reusable contexts

Module 6 precedes the remaining roadmap. Its implementation and applicable
official tests must pass before dictionary/encoder/allocator expansion begins.
The core remains Rust 1.89 compatible with `no_std` and `alloc`.

## Public contract

`Decoder::new(DecodeOptions)` owns its state. `process(input, output)` borrows
slices only for that call and returns `Progress { consumed, produced, status }`.
`StreamStatus` distinguishes `NeedInput`, `NeedOutput`, `FrameFinished`, and
encoder-only `Flushed`. Counts never exceed the provided slices. Successful
calls may make zero progress only with an explicit resource request or a frame
boundary. Empty output does not prevent parsing an empty/skippable frame.

A decoder returns `FrameFinished` at each standard or skippable frame boundary,
after all output and any checksum have been handled. It leaves concatenated
input unconsumed for the next call. `finish()` verifies end of input: at least
one frame must have completed, and no partial header, block, checksum, skipped
payload, or pending output may remain. A subsequent `process` requires reset.
Decoded bytes are provisional until the frame checksum succeeds. An error
poisons the context; `reset()` restores usability and clears frame-local state
while retaining reusable allocations. No slice or caller pointer is saved.

`Encoder::new()` uses default options. `Encoder::with_options(EncoderOptions)`
validates the window logarithm, checksum option, optional pledged source size,
and content-size emission flag. `process(input, output, EndDirective)` supports
`Continue`, `Flush`, and `Finish`. A flush emits every accepted input byte in
complete non-final blocks and returns `Flushed` only after draining output.
Finish emits a final block and optional checksum, returning `FrameFinished`
only after draining. Repeated flush/finish calls supply unconsumed input and
output space; switching directives before completion is an invalid state.
Pledged size is enforced even when omitted from the wire header. A completed
encoder requires reset before another frame. Reset retains validated options
and reusable allocations; changing options uses an explicit reset method.

The encoder uses a non-single-segment header with a bounded advertised window,
including for unknown source length. Its initial block strategy remains the
existing raw/RLE/greedy compressed strategy; no advanced compression level is
implied by streaming support. Blocks never exceed the smaller of window and
128 KiB. The input/output block staging is independent of total stream length.

## Owned state and invariants

| Component | Ownership and invariant | Upstream counterpart |
| --- | --- | --- |
| Decoder phase | Value enum, header `[u8; 18]`, filled/required indices, block and skip counters | `ZSTD_dStage`, `ZSTD_dStreamStage` |
| Raw/RLE transfer | Remaining block length and optional repeated byte; emits directly into caller slices as input/output arrive | Raw block `ZSTD_decompressStream` shortcut, RLE block decoding |
| Compressed block staging | Owned input `Vec<u8>` at most 128 KiB; decoded pending block at most 128 KiB plus bounded literals | Buffered compressed-block branch of `ZSTD_decompressStream` |
| Ring history | Owned initialized bytes, physical write index, available count and checked logical frame length; keeps only the declared window | `ZSTD_DCtx` prefix/history fields |
| Sequence sink | Borrowed ring plus bounded owned pending output, shared execution contract with one-shot history | `ZSTD_execSequence` output/history operations |
| Entropy/repeat state | Owned Huffman/FSE tables and three repeat offsets; survive blocks, reset at a frame boundary | `ZSTD_entropyDTables_t`, `rep[]` |
| Incremental checksum | Four lanes, checked-independent hash length modulo 2^64, 32-byte tail and index | `XXH64_state_t` |
| Encoder staging | Owned block/pending bytes and drain index; no saved input pointer; explicit flush/finish phase | `ZSTD_cStreamStage`, `ZSTD_inBuffer`/`outBuffer` progress |
| Encoder workspaces | Owned match table and predefined inverse entropy tables reused across blocks/frames | `ZSTD_CCtx` match/entropy workspaces |

Only ring memory access adds a new performance-sensitive family:
`unsafe-ring-history`. Its safe implementation uses checked indices; the
feature uses checked wrappers and local proofs before raw/unchecked operations.
Plain progress records, state enums, checksum lanes, and counters share a safe
representation. Existing cursor/entropy/match backends remain independently
selectable. All retained capacities and peak block scratch are documented and
measured; memory must not grow with already delivered stream output.

## Work sequence

1. Commit the public contract, data structures, and detailed checklist.
2. Implement safe ring history, then its gated backend and independent tests.
3. Implement incremental XXH64 and verify exact upstream vectors/chunking.
4. Generalize sequence execution through a bounded output/history sink.
5. Parse partial frame/block headers and stream raw/RLE/skip payloads.
6. Stage compressed blocks and preserve entropy/history/checksum across calls.
7. Extract reusable block encoding and implement continue/flush/finish.
8. Port applicable upstream streaming/context contracts with exact mappings.
9. Verify backend/no_std/Miri/oracle matrices and retained-memory/performance.

Dictionary, legacy, parameter-control, threading, and C ABI cases remain
explicitly open until their later roadmap modules exist. Streaming tests must
exercise partial processing, context reset, and bounded memory rather than
wrapping whole-frame `compress`/`decompress` in superficial chunking.
