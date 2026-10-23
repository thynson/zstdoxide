# Caller-owned one-shot decompression output

## Scope and order

Module 22 retained Ultra2 qualification is complete. Module 23 now exposes the
four direct destination APIs described below; its final ownership model was
recorded in `docs/data-structures.md` before implementation. Focused API/native
evidence and remaining qualification gates are in `caller-output-validation.md`.
This does not claim completed upstream migration.

The next bounded capability is direct one-shot decoding into a caller-owned
mutable slice. It must write format output directly through checked history
operations: collecting into a `Vec` and copying, or routing through an owned
streaming ring and pending-output block, does not satisfy this scope.
The existing allocating APIs must retain their behavior, allocation ownership,
error precedence and format coverage.

Stable incremental sessions are a later scope. They require persistent borrow
and progress contracts beyond a one-shot destination. In-place overlapping
input/output, raw-block APIs, prepared dictionaries, static entropy workspaces,
legacy formats, numeric streaming, LDM and workers are also separate work.

## Public contract

`decompress_into(input, output)` returns the number of decoded bytes in
`output[..written]`. Its default combined output limit is the destination length;
the existing default advertised-window limit remains independent. An explicit
`DecodeOptions` variant uses the smaller of the destination capacity and
`max_output_size` as the effective combined output allowance.

An allocator-bearing variant owns only scratch allocation domains. It accepts
an allocator implementing the existing `Allocator + Clone` contract, including
a reference to a nonclone allocator. The destination allocation always belongs
to the caller and is never adopted, resized, freed or returned through that
allocator. Final exported names should follow the crate's existing `_in`
convention and be fixed with the implementation decomposition.

Success preserves all destination bytes after the returned prefix. Each write
is bounded by the effective output allowance. An error may leave provisional
bytes in that region; callers must discard the attempted result. The API does
not promise transactional rollback of a whole frame or expose progress on error.
Bytes beyond the effective allowance remain untouched, including on error.

The initial API supports the same dictionary-free standard, skippable,
concatenated, checksum, raw, RLE and compressed frame paths as the existing
one-shot decoder. It rejects dictionary IDs as the existing API does. A raw
external dictionary is not implied by a zero dictionary ID. All frame/window,
content-size and checksum checks remain active.

Empty encoded input retains the existing `UnexpectedEof` contract. An actual
empty standard frame or valid skippable-only stream may succeed with zero output
and an empty destination. This is a deliberate Rust API contract; do not infer
unrestricted parity with every upstream empty-input convenience behavior.

## Ownership and invariants

| Structure | Ownership and invariant | Pinned upstream counterpart |
| --- | --- | --- |
| Borrowed output history | One exclusive `&mut [u8]`; checked initialized-output position, frame start, window and effective limit; `frame_start <= position <= limit <= slice.len()` | Destination bounds and prefix history in `ZSTD_decompressFrame` / `ZSTD_decompressBlock_internal` |
| Shared frame-output contract | Private monomorphized extension of `SequenceOutput`; starts a frame, exposes only current frame bytes, and appends RLE output without changing allocation ownership | `ZSTD_decompressMultiFrame` / `ZSTD_decompressFrame` orchestration |
| Existing owned history | Original `Vec<u8, A>`, allocator, output limit, frame start and window; remains the allocating API result owner | Existing whole-output prefix representation |
| Entropy and literal scratch | Existing `SequenceState<A>`, optional Huffman table and fallible literals; no borrowed destination stored inside these owners | Frame-local entropy tables and literal workspace |
| Options, returned count and indices | Plain values shared by both backends; no artificial unsafe operations | Destination capacity, decoded size and frame-local pointer differences |

The borrowed sink does not own or allocate a history copy. Every valid history
reference resolves into previously written bytes of the current frame in the
same output slice. Previous concatenated-frame bytes are excluded by frame
start. Match distance must be nonzero, no greater than current-frame output,
and no greater than the declared window. Append end must fit the effective
limit before mutation. Validate lengths with checked arithmetic or subtraction
from established invariants, including zero and `usize::MAX` boundaries.

LZ overlap uses forward byte-by-byte expansion: each source byte precedes its
destination and may have been produced by an earlier iteration. Safe Rust
indexing is the default. A dedicated `unsafe-slice-history` feature selects an
equivalent checked wrapper and pointer loop for this performance-sensitive
storage family. The public contract and error ordering are identical. Every
unsafe block must prove allocation validity, bounds, initialization, alignment,
aliasing and borrow lifetime for each operation. No unchecked access is justified
solely by assertions or presumed valid format input.

The entire destination slice already contains initialized `u8` values; logical
output length is tracked separately. The pointer implementation must not change
slice/Vec lengths or ownership. Input/output nonaliasing follows from the safe
shared-input/exclusive-output borrows. The API does not accept an unsafe in-place
escape hatch.

## Shared orchestration decision

Prefer extracting the existing one-shot frame loop into a private generic sink
function, with `History<A>` and borrowed history as implementations. The private
contract should add only `start_frame(window)`, `frame_bytes()` and
`repeat(byte, count)` to existing `SequenceOutput` operations. Avoid dynamic
trait objects and duplicated frame parsing logic.

Preserve the existing wrapper's early empty-input rejection, scratch allocator
clone ordering, fresh output owner and `into_bytes()` return path. Raw and RLE
borrowed decoding should make no codec payload allocation. Allocator cloning
still has the allocator implementation's own semantics and is not described as
infallible or allocation-free. Compressed blocks retain their existing fallible
literal and entropy scratch, with errors dropping all temporary owners.

The generic loop must preserve resource and malformed-input precedence. For
example, existing header content-size checks occur before block decoding, while
raw payload truncation is detected by the cursor before history append. Keep
these observable contracts unless an explicitly justified change receives its
own work item and regression evidence.

## Independently verifiable work items

1. Finalize ownership/invariants and public names in the central data-structure
   record; decompose implementation and qualification in `TODO.md`.
2. Add the safe borrowed history and independent overlap/capacity/frame-boundary
   tests. Add the equivalent opt-in unsafe backend and its local safety proofs.
3. Extract the private generic frame loop while preserving every existing owned
   output contract. Run focused old one-shot, malformed and allocator tests.
4. Admit public slice-output wrappers, exact capacity and options semantics,
   scratch allocator identity and all failure cleanup points. Add usage examples
   that demonstrate success prefix, unchanged tail and explicit resource limits.
5. Qualify pinned native caller-destination behavior and meaningful malformed
   inputs; port only the matching caller-owned assertions and corpus branches.
6. Complete safe/each-isolated/all/release tests, all true no_std products,
   formatting, strict Clippy, foreign Miri and concrete no-atomics code generation.
7. Compare safe/unsafe behavior and allocation ledgers on identical deterministic
   workloads, then measure direct output and the owned predecessor with exact
   executable/source retention. Do not infer a speedup from allocation reduction.
8. Reconcile the bounded scope and remaining migration obligations, update
   progress and decompose stable borrowed incremental sessions separately.

## Required format and ownership tests

Borrowed history tests cover all small overlapping offsets; append, match, empty
and RLE operations; exact-fit, one-byte-short and zero capacity; frame isolation;
window limits; invalid offsets and extreme lengths; no writes on rejected
individual operations; and initialized suffix sentinels.

Public tests cover known/unknown content sizes, all block kinds, cross-block
history, concatenated and skippable frames, empty frames, exact destination
capacity, destination/output-option disagreement, window limits, truncation,
malformed entropy/sequences, dictionary-required frames and corrupt checksums.
They compare exact valid output against the existing owned decoder and pinned
native decoding without relying solely on same-implementation round trips.

Allocator tests use owned tracking allocators, borrowed nonclone allocators and
failure injection at every compressed scratch allocation. The destination
pointer/capacity and allocator identity remain unchanged, scratch is released
on every exit, and raw/RLE output uses no codec payload allocation. Compile-fail
examples or focused borrow checks should demonstrate that safe callers cannot
alias input/output or retain conflicting destination access during the call.

## Pinned upstream assertions and limits

The reference revision is facebook/zstd v1.5.7,
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Reference sources remain outside this
repository. Native helpers are test oracles only; no C implementation is linked
into codec production code.

- `tests/fuzzer.c:885-907` exercises missing/trailing source and null/zero output.
  The existing Rust zero-output-limit adaptation can be strengthened with an
  actual empty destination while preserving its existing explicit-limit check.
- `tests/fuzz/decompress_dstSize_tooSmall.c:30` consumes output capacity from the
  tail producer, then uses remaining input as source and clamps capacity below
  nonempty source size. Add the actual destination and guard-byte assertions;
  retain separate qualifications for level selection, reusable contexts and
  third-party sequence producers. Rerun all 1173 files of the matching pinned
  external corpus with no skips after changing the adapter.
- `tests/zstreamtest.c:786-863` covers stable-output single-call, incremental,
  too-small and changed-buffer behavior. This module must not mark the complete
  stable incremental contract implemented.
- `lib/decompress/zstd_decompress.c::ZSTD_checkOutBuffer` validates expected
  pointer, position and size across streaming calls. A future borrowed session
  must translate that lifetime/progress contract explicitly.
- `tests/fuzzer.c:4190` concerns the FSE encoder's caller-owned destination. It is
  unrelated to this decoder scope and remains separately qualified.

Native captures must retain complete helper/dependency identities, normal and
sanitized runs, exact generated sources, capacities, successful output and
prefix/suffix guards. Map native `dstSize_tooSmall` to Rust `ResourceLimit` only
where the declared contract agrees. Do not require identical partial output on
error or count synthetic evidence as a native execution.
