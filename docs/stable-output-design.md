# Stable borrowed incremental decompression

## Scope

Module 24 adds `SliceDecoder<'output, A = Global>` for dictionary-free standard,
skippable and concatenated frames. One exclusive initialized output slice is
borrowed for the complete session. Calls accept temporary input fragments and
write directly into this slice. Previous frame-local output is the only match
history; no ring history or pending decoded-output vector is allocated.

This translates the stable-output behavior of pinned facebook/zstd v1.5.7
`ZSTD_decompressStream`, `ZSTD_checkOutBuffer` and
`ZSTD_decompressContinueStream`. The Rust type owns the position and borrow,
so safe callers cannot replace the output pointer, capacity or position between
calls. Native changed-buffer rejection is adapted as compile-fail borrow/API
checks, not a claimed identical C error code.

Input stability, overlapping input/output, dictionaries/prefixes, mutable
workspaces, numeric streaming hints, legacy frames and the existing buffered
`Decoder` dictionary machinery remain separate. The existing `Decoder` keeps
its ability to drain into unrelated short output slices.

## Public API and lifetime

- `SliceDecoder::new(output, options)` and `SliceDecoder::new_in(output,
  options, allocator)` create allocation-free codec owners. Allocator cloning
  retains the allocator implementation's own semantics.
- `process(input) -> Result<Progress>` reports consumed input and newly written
  output for that call. Resubmit only the unconsumed suffix. It returns
  `NeedInput` or `FrameFinished`, stopping at each complete frame before the
  next magic. It never returns `NeedOutput`: a fixed destination cannot grow,
  so insufficient output allowance is `ResourceLimit` and poisons the session.
- `finish() -> Result<()>` seals only at a clean boundary after at least one
  completed standard or skippable frame. Repeated successful finish is allowed;
  premature finish returns `UnexpectedEof` and poisons the session.
- `output() -> Result<&[u8]>` borrows the written prefix for the lifetime of
  `&self`. Bytes in an unfinished frame remain provisional until checksum and
  content size validation. Failed sessions return `InvalidState`.
- `total_input()` and `total_output()` report accepted input and physically
  written bytes, including writes made by a failing call. Overflow is rejected.
- `reset()` restarts at output position zero with the same borrow and effective
  options, retaining compressed-input and sequence workspace capacity. It does
  not erase destination bytes. Previously produced bytes are no longer output.
- `options()` exposes the effective limits. The output limit is clamped to the
  destination capacity at construction; window limits stay independent.
- Dropping the session releases scratch owners and returns exclusive access to
  the caller's allocation through the ordinary borrow lifetime. There is no
  mutable output accessor or ownership adoption.

The unused suffix stays unchanged on success. On error, bytes within the
allowance may contain partial output; bytes beyond the allowance stay unchanged.
Neither output rollback nor usable input/output positions on error are promised.
A failed or sealed session rejects `process` until reset. Each successful call
must make progress, complete a frame or request more input; empty-input calls
must terminate promptly.

## Ownership and data structures

| Owner/record | Representation and invariants | Upstream counterpart |
| --- | --- | --- |
| `SliceDecoder<'output, A>` | Owns state, allocator and scratch; owns one exclusive output borrow indirectly through `SliceHistory`; never retains input | `ZSTD_DCtx`, stable out-buffer mode |
| `SliceHistory<'output>` | Existing checked output sink; `frame_start <= position <= limit <= output.len()`; output-only read/reset access added privately | Destination prefix history and stable out position |
| Phase | Plain enum for header, block header, raw, RLE byte, RLE, compressed, skip length, skip, checksum, ended, failed; no unsafe variant needed | Stream stages and decompression stages |
| Header/checksum scratch | Inline `[u8; 18]`, filled count bounded by the selected field size | Header/load-header/input staging |
| Encoded block | Fallible `Vec<u8, A>`, at most validated block payload size; retains no decoded output; reset clears length | Bounded input buffer |
| Entropy state | Existing optional Huffman table and `SequenceState<A>`; reset at every standard frame, retained only within that frame | Literal/sequence tables and repcodes |
| Frame metadata | Validated `FrameHeader`, block bound, last-block flag, remaining payload, RLE byte | Frame/block header and stage records |
| Counters | Checked input total and completed-frame count; output position from `SliceHistory`; no duplicate mutable output position | Stream counters/out-buffer position |

`unsafe-slice-history` selects only the existing checked history operation
backend. Other entropy features remain independent. New state and indices are
plain safe records, not artificial unsafe implementations. Default builds remain
safe Rust; all codec code uses core/alloc and supports allocators by value or a
reference to a nonclone allocator.

## Processing decisions

Headers accumulate only their exact required bytes. Validate magic prefixes,
reserved descriptor bits, dictionary IDs, window limit and remaining content
size allowance before accepting a block. Use the pinned frame parser rather
than reimplementing format field arithmetic where practical.

Raw payloads append as input arrives. RLE consumes its one encoded byte, then
writes its validated decoded length directly. A complete compressed payload already present in the current input call is
decoded directly from that borrowed input without an encoded copy. Fragmented
compressed payloads accumulate at most one block before decoding literals and
executing sequences directly into `SliceHistory`. Sequence execution checks block-size bounds; the sink
checks frame-local offsets, window bounds and the combined destination limit.
No buffering of an entire input stream or replay of previously decoded frames
is allowed.

At a last block, check declared content size. For checksum frames, gather the
four checksum bytes and verify against current frame output. A one-shot hash
of the retained frame prefix avoids a second checksum-position counter and
scans each frame exactly once. Complete-frame processing resets entropy and
starts subsequent history at the current output position. Skippable frames
consume their payload without output or entropy dependencies.

Keep a separate small incremental state machine initially, reusing existing
format parsing, literals, sequence execution and storage. Refactoring the
mature dictionary-capable ring decoder is not required to implement this
bounded contract. Any future orchestration unification needs its own regression
item; avoid accidental changes to existing buffering or error precedence.

## Qualification before milestone closure

Test every input split of representative small frames, bytewise input, complete
input, empty calls, exact/zero/short destinations, raw/RLE/compressed blocks,
cross-block matches, concatenation/skips, checksums, known/unknown content size,
dictionary rejection, invalid entropy/sequences, truncated fields and reset
from every reachable phase. Use independent official frames and originals.
Check failure poisoning, stable suffix sentinels and destination identity.

Use native normal and sanitized stable-output helpers as independent test
oracles. Retain source revision/hash identities, helper code, inputs, executions
and guard checks. Preserve distinctions between matching behavioral assertions
and unsupported upstream branches. Add borrow compile-fail examples and
allocator failure injection at every observed allocation ordinal; demonstrate
that raw/RLE sessions request no codec payload allocation and that compressed
sessions own input/entropy scratch only.

Run focused tests for safe/each isolated/all backends, true no_std, strict
Clippy, formatting, safe/all full regression, isolated full regression and
release checks. Qualify actual direct compressed history with strict-provenance
foreign Miri and concrete no-atomics code generation. Freeze deterministic
fragment schedules and output fingerprints; compare allocation ledgers and
measure stable direct versus existing buffered decoding after correctness
qualification, retaining exact executables and repeat timings. Report limitations
without claiming complete upstream migration or universal speedups.

## Matching streaming fuzz branch

Pinned `tests/fuzz/stream_decompress.c:68-119` selects stable output with
`FUZZ_dataProducer_uint32Range(producer, 0, 10) == 5` after reserving the input
prefix. The older buffered Rust adapter consumes that selector but always
uses the buffered branch; its historical complete corpus runs do not qualify
the newly supported stable-output branch.

Add a separate exact-selector adapter without changing the frozen predecessor
qualification. Stable cases retain one destination of `max(10 * source_size,
128 KiB)` and do not consume the buffered branch's output-size selectors.
Input sizes remain zero-inclusive and tail-selected. Stop when the stable
output fills, as the original target does; malformed-input errors are accepted,
with poisoning and guard preservation checked. The ordinary buffered branch
continues to use its own selected output sizes. Record branch counts and every
input identity; distinguish selector-preserving runs from any forced-stable
supplemental cases. Do not silently force all original inputs into stable mode.

The test adapter owns initialized destination/guard vectors and only plain
indices/counters. `SliceDecoder` exclusively borrows the destination region;
disjoint prefix/suffix guards can be inspected independently and the remainder
is inspected only after that borrow ends. No raw-pointer or artificial unsafe
backend is needed in test bookkeeping. Explicit input/window/work limits and
non-stateful context lifetime remain qualified differences from unrestricted
native fuzzing. Corpus snapshots remain separately hash-pinned rolling assets,
not proof of membership in the v1.5.7 source tag.
