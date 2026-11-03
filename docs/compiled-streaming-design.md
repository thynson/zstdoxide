# Streaming compiled-dictionary design

This is the Module 38 implementation decision recorded before code. It follows
[the lifecycle analysis](prepared-dictionary-streaming-analysis.md) and keeps
the pinned v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99` as the reference.

## Decision

The bounded coordinator wraps the **reference block engine**, not the named
`Encoder`/`BlockEncoder` greedy engine. The named workspace owns a single
`CompressionStrategy` parser and its own seed tables; it cannot consume the
compiled Fast, DoubleFast, chain, row or tree candidate views, and extending it
would duplicate the reference family dispatch. The selected shape is a new
`CompiledStreamEncoder` that owns one borrowed `CompiledDictionary` and re-binds
a reference search state for every sealed block.

The coordinator is deliberately narrower than native `ZSTD_compressStream2`:

- Supported families are exactly the six compiled profiles: Fast, DoubleFast,
  Greedy, Lazy, Lazy2 and BinaryTreeLazy2. Optimal, Ultra and Ultra2 are
  rejected at compiled construction, so they cannot reach the stream.
- The frame profile is the compiled seven-field profile. Parameters are not
  re-resolved per frame or per block; the block plan is rebuilt from the same
  resolved record with the actual block length.
- No per-frame table seeding of the compiled suffix happens. The compiled
  candidates are read-only for the whole stream and are never reset.
- A one-frame prefix cannot be selected on the compiled coordinator. Native
  `ZSTD_CCtx_refPrefix_advanced` would clear the CDict; this bounded API omits
  the replacement instead of silently changing frame semantics.

## Block staging and position mapping

At the start of every sealed block the coordinator builds one combined buffer:

```text
compiled loaded suffix | retained frame history | current block
0 .. loaded_len        | loaded_len .. prefix  | prefix ..
```

`prefix = loaded_len + history_len` is the parse start and also the number of
staged bytes the parser may search. `history_len` is the smaller of the frame
bytes already emitted and the resolved window size. History is kept
contiguously from the last window bytes; older bytes are dropped.

This construction keeps every emitted distance exact without a frame-base
translation:

- A candidate inside the retained history has the same local and frame
  distance, because the history is exactly the bytes immediately preceding the
  block.
- A compiled candidate at local position `p < loaded_len` has local distance
  `loaded_len + history_len + offset - p`. While `frame_position == history_len`
  this is the true distance. Once the window has wrapped, the local distance is
  strictly greater than the window and the parser's ordinary window floor
  rejects the candidate even if its bytes match. The decoder would also reject
  that distance.
- Cross-block repeats and matches keep their wire distances because the
  retained history is a contiguous suffix of the frame, not a compacted
  selection.

`history_len` is therefore a *validity invariant*, not a compression heuristic:
the parser must never be given a shorter or non-suffix history, because that
would silently move a candidate while its distance stayed unchanged.

## Owners and lifetimes

- `CompiledDictionary<'a, A>` stays immutably borrowed for the stream. It owns
  the loaded suffix tables and the prepared first-block entropy; the stream
  never clones, reseeds or mutates them.
- The coordinator owns the caller-selected workspace allocator `A`, block
  staging, pending output, a contiguous history vector bounded by the resolved
  window, the streaming checksum and one `BlockEntropyEncoder<A>`.
- Every sealed block creates a per-block `ReferenceSearchState` whose per-call
  tables are bound to the combined buffer. Those owners are dropped at the end
  of the block; only the entropy owners and history persist for the frame.
  Allocating per block is intentional and keeps the source lifetime local. A
  later item may pool them without changing the public contract.
- No borrow of caller input or output survives a `process` call. The combined
  buffer borrows the caller's input block only for the seal operation.

## Entropy and frame state

- First-block prepared entropy comes from the compiled dictionary's
  `first_block_seed`, installed with the same transactional `start_frame` path
  as the one-shot compiled frame.
- Published repeats live in the retained `BlockEntropyEncoder`. A compressed
  block commits its candidate literals and sequence tables; raw and RLE
  fallback discard only the candidate. The next block always receives the
  committed wire repeats, so a repeat may cross a block boundary.
- Block selection copies the one-shot reference gate: compression is
  worthwhile only when the candidate is smaller than the block; block RLE
  requires a non-first block, a parsed candidate and either rejected gain or a
  payload shorter than 25 bytes.
- The frame header is the non-single-segment streaming form used by `Encoder`,
  with the compiled window descriptor, optional content size, optional checksum
  and the compiled dictionary ID. An empty completed frame emits the header,
  one last empty raw block and the optional checksum.

## Lifecycle

- Construction validates the compiled profile, window log and fixed frame
  options before any allocation. It installs the compiled entropy seed but
  performs no table seeding.
- `reset` discards frame-local block, history, counters, checksum and entropy
  but keeps the compiled selection, like `ZSTD_CCtx_reset(session_only)`.
- A completed frame's inherited entropy and repeats are discarded; the next
  frame starts from the compiled first-block seed again.
- Any processing error poisons the stream until `reset`, matching the existing
  `Encoder` contract. Bytes already delivered stay delivered.
- Dropping the stream releases the workspace owners and then the compiled
  borrow; the compiled state itself is dropped by its own owner.

## Explicit non-goals

Native attach-versus-load workspace reuse, pledged-source window adjustment,
prefix replacement, parameter resets that clear a compiled selection, LDM,
workers, arbitrary numeric levels and byte identity with native streaming are
not claimed. Pumped-block output is not expected to be byte-identical to the
one-shot compiled frame because the one-shot path knows the source length, uses
one parser state for all blocks and writes a single-segment header.
