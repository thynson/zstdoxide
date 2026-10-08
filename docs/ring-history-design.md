# Bounded streaming history

## Purpose and upstream correspondence

`RingHistory` retains only the last advertised window of the active frame.
It replaces the full-output ownership of the initial `History` for streaming
execution. It corresponds to the prefix/window maintenance in `ZSTD_DCtx` and
the match expansion performed by `ZSTD_execSequence`; its representation and
resumable slice API are independent of the upstream C layout.

## Ownership and invariants

The object owns one `Vec<u8>` and plain integer metadata. Caller output and
literal input are borrowed for a single method call; no borrow or pointer is
stored between calls. A caller retains its own remaining literal or match
length after output exhaustion.

The vector's initialized length is the available history, bounded by the active
window. Before the ring fills, the next-write index equals that length. Once
full, the next-write index lies within the vector and cyclic overwrites discard
the oldest byte. At each emitted byte, match distance `d` identifies the ring
slot `head - d` when nonnegative, otherwise `window - (d - head)`. These forms
avoid adding two potentially large indices. Each source byte is read before
the destination slot is overwritten; distance equal to the window may identify
the very slot being overwritten and is valid. Distance one and other short
matches can use bytes emitted earlier in the same call or a previous resume.

The produced-frame counter is `u64`, checked before every emitting call.
Physical ring positions are updated independently, so long frames do not rely
on an ever-growing `usize` address or a narrowing cast. Logical counter overflow
returns `ResourceLimit` before modifying either history or caller output.

A configured maximum window is immutable. Starting a frame validates its window
against that limit and reserves the required capacity with `try_reserve_exact`
before replacing active-frame metadata. Reservation failure preserves the old
logical history. Starting a frame or clearing a context retains capacity;
shrinking the next advertised window does not require shrinking the allocation.
Only initialized bytes up to the current active window are addressable history.
The retained allocation is determined by the largest successfully requested
window so far, subject to the allocator's reported vector capacity.

A zero window permits an empty frame and zero-output calls. Emitting a byte
with zero window is corrupt input. Invalid match distances are rejected even
when the output slice is empty. Plain counters and indices share their safe
representation in both backends; they do not need artificial unsafe variants.

## Slice progress contract

- `append(input, output)` copies at most the smaller slice length and returns
  the number of consumed literal bytes and emitted output bytes.
- `repeat(byte, remaining, output)` and
  `copy_match(distance, remaining, output)` return at most
  `min(remaining, output.len())` emitted bytes.
- The caller advances its pending operation by the returned count. A zero-length
  output slice makes no progress and saves no reference to that slice.
- Validation and checked frame-length advancement happen before emission.
  An error leaves history bytes, logical positions, and caller output unchanged.
- Frame-level content-size, total-output, checksum, and block-size checks belong
  to the streaming decoder; the history object enforces its window and arithmetic
  contracts independently.

## Backend boundary

The first implementation uses indexed vector access only. The subsequent
`unsafe-ring-history` feature will replace byte reads and stores with equivalent
unchecked access after the same validation. The entire vector length remains
initialized. Pointer arithmetic, lifetime, aliasing, and overlap obligations
must be justified at each unsafe block; unchecked access may not depend on
format assumptions or debug-only assertions. Allocation occurs only while
starting a frame, before any transient pointer exists.

## Work decomposition

1. Specify slice progress, storage ownership, and ring/counter invariants.
2. Implement safe storage and partial literal, RLE, and match emission.
3. Verify wraparound, distance one, whole-window distances, resumptions, frame
   resets, window caps, retained allocation, and artificial counter overflow.
4. Commit the safe checkpoint before adding unchecked access.
5. Add the independently gated equivalent backend with complete local proofs.
6. Run matching safe/unsafe correctness, no_std, Clippy, Miri where available,
   and deterministic throughput comparisons integrated by the coordinator.
