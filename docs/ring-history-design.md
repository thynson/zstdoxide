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

## Absolute reads and deferred block commits

Dictionary-to-frame matches can need early frame bytes after their output would
overwrite those bytes in the ring. Module 7 therefore decodes a compressed block
against unchanged previous-block history and the block's own pending output,
then commits the complete block. Dictionary content remains separate from this
ring and is never counted as produced frame output.

`byte_at(frame_position)` returns a copied byte from an absolute `u64` frame
position. Its valid interval is `[produced - bytes.len(), produced)`. The method
first rejects the current/future positions, then converts the positive backward
distance to `usize` and checks it against initialized history length. Only this
validated distance determines the physical slot using the existing head mapping;
the absolute position is never narrowed or reduced modulo the window. Reads
borrow the ring immutably, allocate nothing, and leave every field unchanged.
Expired positions and positions in an empty or cleared frame are corrupt input.

`commit(input)` records every input byte without copying to a caller output
slice. It checks advancement of the entire logical count before writing any
byte, then uses the existing safe or independently selected unsafe store. It
does not allocate: frame initialization already reserved the active window.
Empty input succeeds even for a zero window or exhausted logical counter;
nonempty input with a zero window fails without mutation. Counter overflow also
leaves the whole history unchanged, rather than committing a partial prefix.

This extends the existing `unsafe-ring-history` family. Absolute unsafe reads
require the same initialized-slot and live shared-owner proof as relative match
reads. Deferred unsafe stores retain the existing exclusive-owner, capacity,
alignment and initialization contract. Neither operation creates a new pointer
field, borrow stored across calls, or unsafe family.

The extension is decomposed into these focused items, coordinated in `TODO.md`:

1. Record the absolute retained interval and atomic deferred-commit contract.
2. Implement checked lookup and commit with equivalent safe/unsafe access.
3. Verify interval endpoints, partial/full rings, repeated wraps, old positions,
   counts above `u32::MAX`, zero windows, and failure without partial mutation.
4. Verify immutable source reads before committing a block that evicts them.
5. Run focused safe/unsafe/all-feature, no_std, Clippy and Miri checks.

## Backend boundary

The default implementation uses indexed vector access only. The independent
`unsafe-ring-history` feature replaces byte reads and stores with equivalent
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


## Implemented API and validation checkpoints

`new(maximum_window)` allocates nothing. `start_frame(window)` validates and
fallibly reserves; `clear()` resets active-frame state while retaining capacity.
`frame_len()` returns the checked `u64` count, `available()` returns initialized
history length, `window_size()` reports the active bound, and
`retained_capacity()` exposes the allocation high-water mark for measurement.
`append`, `repeat`, and `copy_match` implement the slice progress contract above.

The safe checkpoint was committed before introducing unsafe operations. Ten
unit tests cover literal/RLE partial output, every match distance in windows
1 through 19, whole-window matches, distance-one overlap, partially initialized
windows, irregular resumes including zero-size output, clear/reset reuse,
invalid distances/windows, zero windows, allocation failure preserving state,
logical counts above `u32::MAX`, atomic `u64` overflow, and bounded retained
storage over thousands of chunks.

The unchecked helper is an `unsafe fn` with an explicit `# Safety` contract
covering nonzero window, capacity, initialized prefix, indices, ownership,
alignment, aliasing, and lifetime. Every caller is inside a nonempty emission
loop after complete count validation. A local proof accompanies every call,
every raw match read, and the combined raw store/vector-length update. Growing
stores initialize exactly one new slot before extending the vector length;
full stores overwrite an existing initialized byte.

The ten tests passed in default, independently enabled `unsafe-ring-history`,
all-feature, no-default-feature plus unsafe-ring-history, and optimized unsafe
builds. The safe no-default-feature tests also passed before the unsafe change.
A standalone Clippy test harness with warnings denied and both unsafe lints
enabled passed for this module. The workspace-wide Clippy invocation was
blocked at this checkpoint by the independently added incremental checksum
object not yet having a non-test consumer; this is recorded separately from
the ring result. Integrated streaming validation, Miri, and comparative
throughput remain coordinator-owned checks before the streaming milestone.

The Module 7 extension adds four tests, bringing the focused ring suite to
fourteen. It verifies each retained position over partial fills and multiple
wraps, both retained-interval endpoints, absolute positions above `u32::MAX`
and near `u64::MAX`, commit overflow without partial mutation, empty commits
with zero windows, and immutable reads before a staged block evicts the old
prefix. The spanning-match test also verifies that committed output remains
usable through the existing resumable match API.

All fourteen tests passed in safe, independently enabled unsafe-ring-history,
all-feature, safe no-default-feature, and unsafe no-default-feature builds.
The four new deferred-history tests passed nightly Miri with unsafe-ring-history
enabled. Standalone safe/unsafe Clippy test harnesses passed with warnings denied
and the unsafe-operation, safety-documentation and unsafe-block lints enabled.
The preserved `copy_match` API remains covered by tests and backend benchmarks;
a documented non-test dead-code allowance keeps it available while production
compressed-block execution uses immutable reads followed by bulk commits.
