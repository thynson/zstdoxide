# Allocator test support

This standalone, unpublished `std` helper tests the codec's use of the standard
`Allocator` trait. It is not a production allocator or a replacement allocator
API. It needs the real standard allocator API available in the selected Module 9
compiler, `nightly-2026-10-08` (Rust 1.101.0-nightly, commit `1d81eb4ad`);
the unavailable Rust 1.100 distribution has not been tested.

`TrackingAllocator` forwards payload storage to `Global`. A preallocated ledger
contains only numeric addresses, layouts and allocator identities. An `Arc`
owns that ledger and its `Mutex`; neither payload pointers nor references into
payload memory are retained. Safe monitor methods observe accounting or change
future failure injection, and cannot invalidate a live allocation.

Cloning an allocator creates a distinct identity with shared campaign accounting.
An allocation must be freed or resized through its original identity. The helper
deliberately does not implement `AllocatorClone`. Passing `&allocator` provides
the normal equivalent borrowed allocator, without cloning the underlying value.
Moving and swapping complete vectors preserves their allocator identity.
`into_non_clone()` provides the same allocator through a public non-Clone owner
for testing leaves that require only `A: Allocator`; aggregate APIs can borrow
that owner as `&NonCloneAllocator` without cloning it.

All positive-size allocate, allocate-zeroed, grow, grow-zeroed and shrink requests
share one-based failure ordinals across a campaign and its clones. A failure is
injected exactly once at the configured ordinal. Zero-size destinations bypass
failure injection and do not occupy ledger slots; zero-size requests still have
separate counters. Ledger exhaustion is an ordinary allocation failure. The
ledger has a fixed maximum of 65,536 simultaneously live positive-size blocks.

`requested_bytes` is cumulative requested size, including failed requests.
Live/peak requested bytes use the original requested layouts, while live/peak
bytes use returned block sizes. Resizing replaces one live record. Successful
allocation operations and explicit deallocation calls are counted separately;
resizing does not pretend to be an explicit deallocation call. Zero-size blocks
are excluded from live-byte and live-block counts. Peaks describe caller-visible
live payload between outer allocator calls, excluding any internal overlap of
old/new storage during Global resizing and any backing allocator rounding beyond
the returned size. Setup's ledger and Arc allocation (including the inline
Shared/Mutex/Ledger state, not just reference counts) are outside these payload
counters. Ledger reservation is fallible, while `Arc::new` retains the standard
allocation-error behavior. Constructor setup also locks/unlocks the private
mutex before any `TrackingAllocator` exists. Native-mutex initialization may
allocate or panic under std's setup contract, outside allocator callbacks.

Allocator callbacks and allocator destruction perform no assertions, formatting,
dynamic bookkeeping allocation or unchecked arithmetic. Violations become flags
for inspection outside callbacks; poisoned locks are recovered and flagged.
Failed resizing preserves the original pointer, bytes and ledger record. Drop
records an owner that disappears with live blocks but does not touch payload
memory. `AllocationMonitor::assert_released` is deliberately an ordinary test
assertion outside allocator callbacks. Tests never deliberately violate an unsafe
allocator call's preconditions.

The selected compiler's macOS lock proof is concrete: in
`std/src/sys/sync/mutex/pthread.rs:19`, the first lock runs
`OnceBox::get_or_init` and `Box::pin`; `std/src/sys/pal/unix/sync/mutex.rs:50`
initializes a `PTHREAD_MUTEX_NORMAL` with checked OS calls that may panic. Setup
executes that path before constructing the allocator. Later lock calls observe
the initialized pinned mutex. No external guard, recursive call, forgotten guard,
or user callback can violate locking ownership, and every guard unlocks on its
locking thread. The normal pthread mutex therefore meets the selected stdlib's
lock/unlock/destroy preconditions: no error/panic branch is reachable in valid
use. Poisoning is recovered with `into_inner` and flagged. The final Arc can only
destroy an unlocked initialized mutex after all guards and owners are gone.
Other platform checks must review their selected stdlib mutex implementation
alongside actual execution; this source proof does not claim unrun platforms.

Implementation checkpoints:

1. Define the bounded ledger, shared monitor, identity and failure contracts.
2. Forward the complete standard allocation interface with nearby safety proofs.
3. Verify alignment/zeroing, zero-size behavior, failed growth, complete release,
   identity-preserving moves/swaps, borrowed use and deterministic failure sweeps.
4. Run focused tests, strict Clippy and Miri on the coordinator-approved compiler.

The root package registers this crate only as a development dependency. Unsafe
test allocator code therefore does not weaken the codec's safe-default lint.
