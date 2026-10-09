//! Bounded accounting and failure injection around the standard global allocator.
//!
//! Payload allocations use [`Global`]. Setup bookkeeping is separate and is never
//! extended by an allocator callback. Clones have distinct allocation identities;
//! borrowed references retain the identity of the referenced allocator.
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks, clippy::missing_safety_doc)]

use std::alloc::{AllocError, Allocator, Global, Layout};
use std::num::NonZeroU64;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, MutexGuard};

/// Largest supported number of simultaneously live positive-size allocations.
pub const MAX_LEDGER_CAPACITY: usize = 65_536;

/// Setup-time policy shared by an allocator and its distinct-owner clones.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocatorConfig {
    /// Maximum simultaneous positive-size allocations; must be in `1..=65_536`.
    pub max_live_allocations: usize,
    /// Absolute, one-based positive-size request ordinal to reject once.
    pub fail_at: Option<NonZeroU64>,
}

impl Default for AllocatorConfig {
    fn default() -> Self {
        Self {
            max_live_allocations: 4096,
            fail_at: None,
        }
    }
}

/// Failure to initialize the bookkeeping ledger, before any payload operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupError {
    /// The ledger capacity is zero or exceeds [`MAX_LEDGER_CAPACITY`].
    InvalidCapacity,
    /// Reserving the ledger failed.
    AllocationFailed,
}

/// Sticky diagnostics, inspected outside allocation callbacks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Violations {
    pub poisoned_lock: bool,
    pub counter_overflow: bool,
    pub unknown_pointer: bool,
    pub wrong_owner: bool,
    pub invalid_layout: bool,
    pub inconsistent_accounting: bool,
    pub owner_dropped_with_live_blocks: bool,
}

impl Violations {
    /// Whether any contract or accounting diagnostic was recorded.
    pub fn any(self) -> bool {
        self != Self::default()
    }
}

/// A consistent snapshot of one campaign, including all allocator clones.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AllocationSnapshot {
    pub allocate_calls: u64,
    pub allocate_zeroed_calls: u64,
    pub grow_calls: u64,
    pub grow_zeroed_calls: u64,
    pub shrink_calls: u64,
    pub deallocate_calls: u64,
    /// Requests with positive destination size, including rejected requests.
    pub attempts: u64,
    /// Successful outer allocation or resizing calls, including zero-size ones.
    pub successes: u64,
    pub failures: u64,
    /// Allocation/resizing requests whose destination has size zero.
    pub zero_size_requests: u64,
    /// Sum of requested destination sizes, including failed requests.
    pub requested_bytes: u128,
    pub live_blocks: u64,
    pub peak_live_blocks: u64,
    /// Sum of the original requested sizes of currently live blocks.
    pub live_requested_bytes: u128,
    /// Maximum caller-visible live requested bytes, excluding Global's internal
    /// transient old/new storage overlap during resizing.
    pub peak_requested_bytes: u128,
    /// Sum of the actual sizes returned by Global for currently live blocks.
    pub live_bytes: u128,
    /// Maximum caller-visible live returned bytes, excluding Global's internal
    /// resizing overlap and backing-allocator rounding beyond returned sizes.
    pub peak_bytes: u128,
    pub identities_created: u64,
    pub owners_dropped: u64,
    pub violations: Violations,
}

#[derive(Clone, Copy)]
struct Entry {
    address: usize,
    owner: u64,
    requested: usize,
    actual: usize,
    alignment: usize,
}

#[derive(Clone, Copy)]
enum Operation {
    Allocate,
    AllocateZeroed,
    Grow,
    GrowZeroed,
    Shrink,
}

struct Ledger {
    entries: Vec<Option<Entry>>,
    snapshot: AllocationSnapshot,
    fail_at: Option<NonZeroU64>,
    next_identity: u64,
}

fn increment(value: &mut u64, overflow: &mut bool) {
    match value.checked_add(1) {
        Some(next) => *value = next,
        None => *overflow = true,
    }
}

impl Ledger {
    fn begin(&mut self, operation: Operation, layout: Layout) -> bool {
        let stats = &mut self.snapshot;
        let calls = match operation {
            Operation::Allocate => &mut stats.allocate_calls,
            Operation::AllocateZeroed => &mut stats.allocate_zeroed_calls,
            Operation::Grow => &mut stats.grow_calls,
            Operation::GrowZeroed => &mut stats.grow_zeroed_calls,
            Operation::Shrink => &mut stats.shrink_calls,
        };
        increment(calls, &mut stats.violations.counter_overflow);
        if layout.size() == 0 {
            increment(
                &mut stats.zero_size_requests,
                &mut stats.violations.counter_overflow,
            );
            return true;
        }
        increment(&mut stats.attempts, &mut stats.violations.counter_overflow);
        match stats.requested_bytes.checked_add(layout.size() as u128) {
            Some(bytes) => stats.requested_bytes = bytes,
            None => stats.violations.counter_overflow = true,
        }
        !stats.violations.counter_overflow
            && self.fail_at.map(NonZeroU64::get) != Some(stats.attempts)
    }

    fn failed(&mut self) -> AllocError {
        increment(
            &mut self.snapshot.failures,
            &mut self.snapshot.violations.counter_overflow,
        );
        AllocError
    }

    fn succeeded(&mut self) {
        increment(
            &mut self.snapshot.successes,
            &mut self.snapshot.violations.counter_overflow,
        );
    }

    fn add_live(&mut self, entry: Entry) {
        let stats = &mut self.snapshot;
        increment(
            &mut stats.live_blocks,
            &mut stats.violations.counter_overflow,
        );
        // At most 65,536 entries, each bounded by usize::MAX on supported 32/64-bit
        // hosts, fit u128. Saturation also prevents unwinding on exotic targets.
        stats.live_requested_bytes = stats
            .live_requested_bytes
            .saturating_add(entry.requested as u128);
        stats.live_bytes = stats.live_bytes.saturating_add(entry.actual as u128);
        stats.peak_live_blocks = stats.peak_live_blocks.max(stats.live_blocks);
        stats.peak_requested_bytes = stats.peak_requested_bytes.max(stats.live_requested_bytes);
        stats.peak_bytes = stats.peak_bytes.max(stats.live_bytes);
    }

    fn remove_live(&mut self, entry: Entry) {
        let stats = &mut self.snapshot;
        match (
            stats.live_blocks.checked_sub(1),
            stats
                .live_requested_bytes
                .checked_sub(entry.requested as u128),
            stats.live_bytes.checked_sub(entry.actual as u128),
        ) {
            (Some(blocks), Some(requested), Some(actual)) => {
                stats.live_blocks = blocks;
                stats.live_requested_bytes = requested;
                stats.live_bytes = actual;
            }
            _ => stats.violations.inconsistent_accounting = true,
        }
    }

    fn locate(&mut self, owner: u64, ptr: NonNull<u8>, layout: Layout) -> Option<usize> {
        let index = self
            .entries
            .iter()
            .position(|slot| slot.is_some_and(|entry| entry.address == ptr.as_ptr().addr()));
        let Some(index) = index else {
            self.snapshot.violations.unknown_pointer = true;
            return None;
        };
        let Some(Some(entry)) = self.entries.get(index) else {
            self.snapshot.violations.inconsistent_accounting = true;
            return None;
        };
        if entry.owner != owner {
            self.snapshot.violations.wrong_owner = true;
            return None;
        }
        if entry.alignment != layout.align()
            || layout.size() < entry.requested
            || layout.size() > entry.actual
        {
            self.snapshot.violations.invalid_layout = true;
            return None;
        }
        Some(index)
    }
}

struct Shared {
    ledger: Mutex<Ledger>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Ledger> {
        // TrackingAllocator::new initializes this private native lock before
        // constructing any allocator owner. Later calls cannot take std's lazy
        // initialization/Box-allocation path. No guard is exposed or forgotten,
        // no method recursively locks it, and no user callback/destructor runs
        // under it. Guards unlock on their locking thread before Shared drops.
        // The initialized normal pthread mutex therefore meets lock/unlock/drop
        // preconditions; poison recovery never unwraps. See README's exact
        // selected-stdlib source proof and constructor setup qualification.
        match self.ledger.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.snapshot.violations.poisoned_lock = true;
                guard
            }
        }
    }
}

/// Observer and failure controller that cannot allocate or invalidate payloads.
#[derive(Clone)]
pub struct AllocationMonitor {
    shared: Arc<Shared>,
}

impl AllocationMonitor {
    pub fn snapshot(&self) -> AllocationSnapshot {
        self.shared.lock().snapshot
    }

    /// Set an absolute positive-size request ordinal; `None` disables injection.
    /// A past ordinal has no effect. Existing allocations remain untouched.
    pub fn set_fail_at(&self, ordinal: Option<NonZeroU64>) {
        self.shared.lock().fail_at = ordinal;
    }

    /// Reject the next positive-size request, without resetting accounting.
    /// Returns false only if the request counter cannot represent another ordinal.
    pub fn fail_next(&self) -> bool {
        let mut ledger = self.shared.lock();
        match ledger
            .snapshot
            .attempts
            .checked_add(1)
            .and_then(NonZeroU64::new)
        {
            Some(ordinal) => {
                ledger.fail_at = Some(ordinal);
                true
            }
            None => {
                ledger.snapshot.violations.counter_overflow = true;
                false
            }
        }
    }

    pub fn clear_failure(&self) {
        self.set_fail_at(None);
    }

    /// Assert outside allocator operations that all positive-size blocks were
    /// released and no ownership/accounting contract violation was recorded.
    pub fn assert_released(&self) {
        let snapshot = self.snapshot();
        assert_eq!(snapshot.live_blocks, 0, "live allocation: {snapshot:?}");
        assert_eq!(snapshot.live_requested_bytes, 0, "{snapshot:?}");
        assert_eq!(snapshot.live_bytes, 0, "{snapshot:?}");
        assert!(
            !snapshot.violations.any(),
            "allocator violation: {snapshot:?}"
        );
    }
}

/// A Global-backed allocator with a distinct identity and bounded shared ledger.
///
/// Cloning creates a new, non-equivalent owner. Allocate fresh storage with that
/// clone; move or swap whole collections to transfer existing storage. `&Self`
/// remains equivalent to its referent under the standard allocator contract.
/// Zero-size requests use Global's aligned dangling block without a ledger slot.
pub struct TrackingAllocator {
    shared: Arc<Shared>,
    identity: u64,
}

/// The same tracking allocator without a Clone implementation.
///
/// Use this owned type to test leaf APIs that accept only `A: Allocator`.
/// Aggregate APIs requiring `A: Allocator + Clone` can receive `&Self`, which
/// clones the reference and keeps this single allocation identity.
pub struct NonCloneAllocator {
    inner: TrackingAllocator,
}

impl NonCloneAllocator {
    pub fn identity(&self) -> u64 {
        self.inner.identity()
    }

    pub fn monitor(&self) -> AllocationMonitor {
        self.inner.monitor()
    }
}

impl TrackingAllocator {
    /// Reserve all ledger storage and initialize the private native mutex before
    /// exposing an allocator. Arc/native-mutex setup retains std's OOM behavior
    /// and may panic on a native initialization error; no allocator owner exists
    /// yet if setup fails. Ledger reservation itself returns SetupError.
    pub fn new(config: AllocatorConfig) -> Result<Self, SetupError> {
        if !(1..=MAX_LEDGER_CAPACITY).contains(&config.max_live_allocations) {
            return Err(SetupError::InvalidCapacity);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(config.max_live_allocations)
            .map_err(|_| SetupError::AllocationFailed)?;
        entries.resize(config.max_live_allocations, None);
        let shared = Arc::new(Shared {
            ledger: Mutex::new(Ledger {
                entries,
                snapshot: AllocationSnapshot {
                    identities_created: 1,
                    ..AllocationSnapshot::default()
                },
                fail_at: config.fail_at,
                next_identity: 2,
            }),
        });
        // macOS std::Mutex lazily allocates/initializes its pthread mutex on
        // first lock. Do that (and the first unlock) during setup, before any
        // TrackingAllocator exists whose Allocator/Drop must never unwind.
        drop(shared.lock());
        Ok(Self {
            shared,
            identity: 1,
        })
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    /// Move this exact allocator identity into an owner that cannot be cloned.
    pub fn into_non_clone(self) -> NonCloneAllocator {
        NonCloneAllocator { inner: self }
    }

    pub fn monitor(&self) -> AllocationMonitor {
        AllocationMonitor {
            shared: Arc::clone(&self.shared),
        }
    }

    fn allocate_impl(&self, layout: Layout, zeroed: bool) -> Result<NonNull<[u8]>, AllocError> {
        let mut ledger = self.shared.lock();
        let operation = if zeroed {
            Operation::AllocateZeroed
        } else {
            Operation::Allocate
        };
        if !ledger.begin(operation, layout) || self.identity == 0 {
            return Err(ledger.failed());
        }
        let slot = if layout.size() == 0 {
            None
        } else {
            match ledger.entries.iter_mut().find(|slot| slot.is_none()) {
                Some(slot) => Some(slot),
                None => return Err(ledger.failed()),
            }
        };
        let result = if zeroed {
            Global.allocate_zeroed(layout)
        } else {
            Global.allocate(layout)
        };
        match result {
            Ok(block) => {
                if let Some(slot) = slot {
                    let entry = Entry {
                        address: block.cast::<u8>().as_ptr().addr(),
                        owner: self.identity,
                        requested: layout.size(),
                        actual: block.len(),
                        alignment: layout.align(),
                    };
                    *slot = Some(entry);
                    ledger.add_live(entry);
                }
                ledger.succeeded();
                Ok(block)
            }
            Err(_) => Err(ledger.failed()),
        }
    }

    /// Resize one currently allocated block and replace its ledger record only
    /// after Global succeeds. No raw pointer is retained in the bookkeeping.
    ///
    /// # Safety
    /// `ptr` must be a currently allocated block of this exact allocator identity
    /// (or its equivalent borrowed reference), and `old_layout` must fit it.
    /// The caller must have exclusive authority to invalidate that block: no
    /// references or smart pointers may be invalidated beyond the one being
    /// reallocated, including the allocator's own `&self`. `operation` must be
    /// Grow/GrowZeroed with new size >= old size, or Shrink with new size <= old
    /// size. Both valid Layout values describe required alignment; alignment may
    /// change. The old block need not be initialized: Global preserves raw bytes
    /// without forming typed references. On success the old pointer is invalid;
    /// on failure its allocation and bytes remain valid and unchanged.
    unsafe fn resize(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
        operation: Operation,
    ) -> Result<NonNull<[u8]>, AllocError> {
        let mut ledger = self.shared.lock();
        if !ledger.begin(operation, new_layout) || self.identity == 0 {
            return Err(ledger.failed());
        }
        let order_valid = match operation {
            Operation::Grow | Operation::GrowZeroed => new_layout.size() >= old_layout.size(),
            Operation::Shrink => new_layout.size() <= old_layout.size(),
            _ => false,
        };
        if !order_valid {
            ledger.snapshot.violations.invalid_layout = true;
            return Err(ledger.failed());
        }
        let old_index = if old_layout.size() == 0 {
            None
        } else {
            match ledger.locate(self.identity, ptr, old_layout) {
                Some(index) => Some(index),
                None => return Err(ledger.failed()),
            }
        };
        let destination = if new_layout.size() == 0 {
            old_index
        } else {
            match old_index.or_else(|| ledger.entries.iter().position(Option::is_none)) {
                Some(index) => Some(index),
                None => return Err(ledger.failed()),
            }
        };
        // Select the slot before Global can invalidate the old block. It remains
        // exclusively borrowed under this guard throughout the forwarded call.
        let slot = match destination {
            Some(index) => match ledger.entries.get_mut(index) {
                Some(slot) => Some(slot),
                None => {
                    ledger.snapshot.violations.inconsistent_accounting = true;
                    return Err(ledger.failed());
                }
            },
            None => None,
        };
        // SAFETY: The caller supplies a live block of this identity, a fitting
        // old layout and exclusive invalidation authority. Positive-size blocks
        // are checked against the ledger; zero-size blocks came from Global as
        // well. Global owns all forwarded allocations. The checked operation
        // enforces the required size ordering, valid Layouts provide alignment,
        // and Global preserves raw initialized/uninitialized bytes, applying
        // zeroing for GrowZeroed. On Err Global leaves the old block untouched.
        let result = unsafe {
            match operation {
                Operation::Grow => Global.grow(ptr, old_layout, new_layout),
                Operation::GrowZeroed => Global.grow_zeroed(ptr, old_layout, new_layout),
                Operation::Shrink => Global.shrink(ptr, old_layout, new_layout),
                _ => return Err(AllocError),
            }
        };
        match result {
            Ok(block) => {
                let replacement = (new_layout.size() != 0).then_some(Entry {
                    address: block.cast::<u8>().as_ptr().addr(),
                    owner: self.identity,
                    requested: new_layout.size(),
                    actual: block.len(),
                    alignment: new_layout.align(),
                });
                let old = match slot {
                    Some(slot) => std::mem::replace(slot, replacement),
                    None => None,
                };
                if let Some(old) = old {
                    ledger.remove_live(old);
                }
                if let Some(entry) = replacement {
                    ledger.add_live(entry);
                }
                ledger.succeeded();
                Ok(block)
            }
            Err(_) => Err(ledger.failed()),
        }
    }
}

impl Clone for TrackingAllocator {
    fn clone(&self) -> Self {
        let mut ledger = self.shared.lock();
        let identity = match ledger.next_identity.checked_add(1) {
            Some(next) => {
                let identity = ledger.next_identity;
                ledger.next_identity = next;
                let stats = &mut ledger.snapshot;
                increment(
                    &mut stats.identities_created,
                    &mut stats.violations.counter_overflow,
                );
                identity
            }
            None => {
                ledger.snapshot.violations.counter_overflow = true;
                // This owner cannot allocate. Existing owners remain valid and
                // can release their blocks; no identity is reused.
                0
            }
        };
        Self {
            shared: Arc::clone(&self.shared),
            identity,
        }
    }
}

impl Drop for TrackingAllocator {
    fn drop(&mut self) {
        let mut ledger = self.shared.lock();
        if ledger
            .entries
            .iter()
            .any(|slot| slot.is_some_and(|entry| entry.owner == self.identity))
        {
            ledger.snapshot.violations.owner_dropped_with_live_blocks = true;
        }
        let stats = &mut ledger.snapshot;
        increment(
            &mut stats.owners_dropped,
            &mut stats.violations.counter_overflow,
        );
        // No payload is inspected or freed. The guard and final Arc destruction
        // own only standard Mutex/Vec/plain-record storage, with no user Drop.
    }
}

// SAFETY: Every positive-size block is allocated by Global and remains valid
// until explicitly deallocated/resized through its recorded owner. Clones get
// unique, deliberately non-equivalent identities; references retain identity.
// Global provides alignment, disjointness, addressability and required zeroing.
// Constructor setup warms the native lock before exposing any owner. Ledger
// methods use that private, non-recursive lock and bounded initialized storage
// with checked access, no callback allocation, no panic/format/unwrap, and
// non-overflowing counters. Allocating failures preserve old blocks and bytes.
// Shared safe monitoring never invalidates payloads, and Drop never touches them
// or unwinds. No payload reference is created by this implementation.
unsafe impl Allocator for TrackingAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.allocate_impl(layout, false)
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.allocate_impl(layout, true)
    }

    /// # Safety
    /// `ptr` must be a currently allocated block of this exact identity (or an
    /// equivalent borrowed reference), and `layout` must fit its size/alignment.
    /// The caller must permit invalidation, without invalidating outstanding
    /// references or other smart-pointer owners, including this allocator's
    /// `&self`. Initialization is irrelevant; no payload bytes are read. After
    /// this call the pointer must not be used, including for another deallocation.
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let mut ledger = self.shared.lock();
        let stats = &mut ledger.snapshot;
        increment(
            &mut stats.deallocate_calls,
            &mut stats.violations.counter_overflow,
        );
        let entry = if layout.size() == 0 {
            None
        } else {
            let Some(index) = ledger.locate(self.identity, ptr, layout) else {
                return;
            };
            let Some(slot) = ledger.entries.get_mut(index) else {
                ledger.snapshot.violations.inconsistent_accounting = true;
                return;
            };
            slot.take()
        };
        // SAFETY: The caller grants exclusive invalidation of a live block and
        // fitting layout. Every block originates from Global, and the ledger
        // verifies identity, original alignment and fitting size for positive
        // blocks. Zero-size blocks also originate from Global. No payload is
        // read and no borrowed reference survives this authorized invalidation.
        unsafe { Global.deallocate(ptr, layout) };
        if let Some(entry) = entry {
            ledger.remove_live(entry);
        }
    }

    /// # Safety
    /// `ptr` must denote a currently allocated block of this allocator identity;
    /// `old_layout` must fit its original alignment and size range. New size must
    /// be >= old size; new alignment may differ. The caller must own exclusive
    /// authority to invalidate the old block without invalidating this `&self`
    /// or other outstanding references/owners. Bytes need not be initialized.
    /// On success only the returned pointer is valid; on failure old bytes and
    /// the old allocation remain valid and unchanged.
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: This method's caller supplies exactly resize's live-owner,
        // fitting-layout, size-order, aliasing and lifetime preconditions; Grow
        // selects the nonzeroing operation and introduces no extra requirement.
        unsafe { self.resize(ptr, old_layout, new_layout, Operation::Grow) }
    }

    /// # Safety
    /// `ptr` must denote a currently allocated block of this allocator identity;
    /// `old_layout` must fit its original alignment and size range. New size must
    /// be >= old size; new alignment may differ. The caller must own exclusive
    /// invalidation authority without invalidating this `&self` or outstanding
    /// references/owners. Old bytes need not be initialized. Success preserves
    /// old bytes, zeroes every newly exposed byte, and invalidates the old pointer.
    /// Failure leaves the old allocation and bytes valid and unchanged.
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: The caller guarantees resize's valid owner, fitting layout,
        // ordered sizes, exclusive invalidation and lifetimes. GrowZeroed asks
        // Global to preserve raw old bytes and initialize only the new region.
        unsafe { self.resize(ptr, old_layout, new_layout, Operation::GrowZeroed) }
    }

    /// # Safety
    /// `ptr` must denote a currently allocated block of this allocator identity;
    /// `old_layout` must fit its original alignment and size range. New size must
    /// be <= old size; new alignment may differ. The caller must own exclusive
    /// invalidation authority without invalidating this `&self` or outstanding
    /// references/owners. Bytes need not be initialized. Success invalidates the
    /// old pointer and preserves the retained prefix; failure leaves it unchanged.
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: The caller guarantees resize's live-owner/fitting-layout,
        // aliasing/lifetime and Shrink size-order requirements. No payload
        // references are created and Global preserves the surviving raw prefix.
        unsafe { self.resize(ptr, old_layout, new_layout, Operation::Shrink) }
    }
}

// SAFETY: Every operation forwards to the same retained TrackingAllocator. This
// wrapper neither clones nor publicly exposes mutable access to that owner, so
// blocks retain their identity, validity, alignment, zeroing and lifetimes.
// Forwarding adds no allocation, callback, lock or panicking operation; default
// Drop only runs the inner allocator's non-unwinding destructor. The wrapper
// exposes no mutable access that could invalidate a currently allocated block.
unsafe impl Allocator for NonCloneAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.inner.allocate(layout)
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.inner.allocate_zeroed(layout)
    }

    /// # Safety
    /// The pointer must denote a currently allocated block of this owner (or an
    /// equivalent borrowed reference), and layout must fit its size/alignment.
    /// No outstanding reference or other owner, including this allocator's
    /// `&self`, may be invalidated. Initialization is irrelevant because no bytes
    /// are read. The pointer becomes invalid and must not be reused afterward.
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: All wrapper allocations originate from this exact retained
        // inner owner. The caller supplies a fitting layout, valid lifetime and
        // exclusive invalidation authority; forwarding changes none of them.
        unsafe { self.inner.deallocate(ptr, layout) };
    }

    /// # Safety
    /// The pointer must denote this owner's live block and old_layout must fit
    /// its size/alignment. New size must be >= old size; alignment may change.
    /// The caller must permit exclusive invalidation without invalidating this
    /// `&self` or any outstanding reference/other owner. Bytes need not be
    /// initialized. Success invalidates the old pointer; failure preserves it.
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: Forwarding retains the exact inner allocation owner. The
        // caller supplies the live/fitting block, ordered sizes, exclusive
        // invalidation and lifetime requirements of inner.grow unchanged.
        unsafe { self.inner.grow(ptr, old_layout, new_layout) }
    }

    /// # Safety
    /// The pointer must denote this owner's live block and old_layout must fit
    /// its size/alignment. New size must be >= old size; alignment may change.
    /// The caller must permit exclusive invalidation without invalidating this
    /// `&self` or any outstanding reference/other owner. Old bytes need not be
    /// initialized; success preserves them and zeroes newly exposed bytes,
    /// invalidating the old pointer. Failure leaves the old block unchanged.
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: The retained inner owner allocated the live block. Forwarding
        // preserves the caller's fitting layouts, ordered sizes, aliasing and
        // lifetime guarantees; inner.grow_zeroed supplies the required zeroing.
        unsafe { self.inner.grow_zeroed(ptr, old_layout, new_layout) }
    }

    /// # Safety
    /// The pointer must denote this owner's live block and old_layout must fit
    /// its size/alignment. New size must be <= old size; alignment may change.
    /// The caller must permit exclusive invalidation without invalidating this
    /// `&self` or any outstanding reference/other owner. Initialization is not
    /// required. Success preserves the retained prefix and invalidates the old
    /// pointer; failure leaves the allocation and its bytes unchanged.
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: The caller's live block belongs to the same retained owner;
        // fitting layouts, ordered sizes, exclusive invalidation and lifetimes
        // are passed unchanged to inner.shrink, with no payload borrow created.
        unsafe { self.inner.shrink(ptr, old_layout, new_layout) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig::default()).expect("test ledger")
    }

    #[test]
    fn invalid_ledger_capacities_are_rejected_before_payload_requests() {
        for capacity in [0, MAX_LEDGER_CAPACITY + 1] {
            assert!(matches!(
                TrackingAllocator::new(AllocatorConfig {
                    max_live_allocations: capacity,
                    fail_at: None,
                }),
                Err(SetupError::InvalidCapacity)
            ));
        }
    }

    #[test]
    fn aligned_zeroed_blocks_report_requested_and_returned_sizes() {
        let allocator = allocator();
        let monitor = allocator.monitor();
        for (size, alignment) in [(1, 1), (33, 64), (4097, 4096)] {
            let layout = Layout::from_size_align(size, alignment).unwrap();
            let block = allocator.allocate_zeroed(layout).unwrap();
            let ptr = block.cast::<u8>();
            assert_eq!(ptr.as_ptr().addr() % alignment, 0);
            assert!(block.len() >= size);
            // SAFETY: allocate_zeroed initialized every byte in the returned
            // live block. The slice is used only here, before deallocation, and
            // no mutable access occurs during this shared borrow.
            let bytes = unsafe { std::slice::from_raw_parts(ptr.as_ptr(), block.len()) };
            assert!(bytes.iter().all(|&byte| byte == 0));
            let stats = monitor.snapshot();
            assert_eq!(stats.live_requested_bytes, size as u128);
            assert_eq!(stats.live_bytes, block.len() as u128);
            assert_eq!(stats.live_blocks, 1);
            // SAFETY: This exact allocator returned the still-live block for
            // layout. The temporary slice borrow ended, no owner remains, and
            // deallocation uses the original fitting layout exactly once.
            unsafe { allocator.deallocate(ptr, layout) };
        }
        let stats = monitor.snapshot();
        assert_eq!(stats.attempts, 3);
        assert_eq!(stats.allocate_zeroed_calls, 3);
        assert_eq!(stats.requested_bytes, 4131);
        assert_eq!(stats.peak_requested_bytes, 4097);
        assert_eq!(stats.deallocate_calls, 3);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn zero_size_and_zst_requests_do_not_consume_failure_ordinals_or_slots() {
        let allocator = TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: NonZeroU64::new(1),
        })
        .unwrap();
        let monitor = allocator.monitor();
        for alignment in [1, 64, 4096] {
            let layout = Layout::from_size_align(0, alignment).unwrap();
            let block = allocator.allocate(layout).unwrap();
            assert_eq!(block.len(), 0);
            assert_eq!(block.cast::<u8>().as_ptr().addr() % alignment, 0);
            // SAFETY: Global-backed allocation just returned this aligned
            // zero-size block, the identical layout fits it, and no payload or
            // other reference exists. This is its single deallocation.
            unsafe { allocator.deallocate(block.cast::<u8>(), layout) };
        }
        let mut zst = Vec::new_in(&allocator);
        zst.try_reserve(100).unwrap();
        zst.resize(100, ());
        assert_eq!(monitor.snapshot().attempts, 0);
        assert!(allocator.allocate(Layout::new::<u8>()).is_err());
        assert_eq!(monitor.snapshot().attempts, 1);
        assert_eq!(monitor.snapshot().zero_size_requests, 3);
        drop(zst);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn failed_growth_preserves_pointer_capacity_bytes_and_accounting() {
        let allocator = allocator();
        let monitor = allocator.monitor();
        let mut bytes = Vec::new_in(&allocator);
        bytes.try_reserve_exact(17).unwrap();
        bytes.extend(0..17);
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        assert!(bytes.try_reserve_exact(8192).is_err());
        assert_eq!(bytes.as_ptr(), pointer);
        assert_eq!(bytes.capacity(), capacity);
        assert_eq!(bytes.as_slice(), &(0..17).collect::<Vec<_>>());
        let after = monitor.snapshot();
        assert_eq!(after.live_blocks, before.live_blocks);
        assert_eq!(after.live_bytes, before.live_bytes);
        assert_eq!(after.live_requested_bytes, before.live_requested_bytes);
        assert_eq!(after.grow_calls, before.grow_calls + 1);
        assert_eq!(after.failures, before.failures + 1);
        monitor.clear_failure();
        bytes.try_reserve_exact(8192).unwrap();
        assert_eq!(bytes.as_slice(), &(0..17).collect::<Vec<_>>());
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(bytes);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn failed_shrink_and_successful_zeroed_growth_preserve_the_correct_prefix() {
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        let small = Layout::from_size_align(32, 8).unwrap();
        let smaller = Layout::from_size_align(8, 64).unwrap();
        let large = Layout::from_size_align(80, 128).unwrap();
        let initial = allocator.allocate(small).unwrap();
        let ptr = initial.cast::<u8>();
        // SAFETY: The block is live, writable for all 32 requested bytes, and
        // exclusively owned here. Writing bytes requires alignment one, and no
        // references alias this initialization.
        unsafe { ptr.as_ptr().write_bytes(0xa5, 32) };
        assert!(monitor.fail_next());
        // SAFETY: This is the original live block and fitting layout, new size
        // is smaller, and no borrow/other owner prevents invalidation. Failure
        // must leave the pointer live; the changed alignment is permitted.
        assert!(unsafe { allocator.shrink(ptr, small, smaller) }.is_err());
        // SAFETY: The failed shrink preserved this initialized 32-byte block;
        // the temporary immutable slice ends before the next resize.
        let preserved = unsafe { std::slice::from_raw_parts(ptr.as_ptr(), 32) };
        assert!(preserved.iter().all(|&byte| byte == 0xa5));
        // SAFETY: Failure left the old block live with its original layout.
        // Growth is larger and may change alignment; this owner has exclusive
        // invalidation authority and no old reference remains.
        let grown = unsafe { allocator.grow_zeroed(ptr, small, large) }.unwrap();
        assert_eq!(grown.cast::<u8>().as_ptr().addr() % 128, 0);
        // SAFETY: The preserved 32-byte prefix was initialized and grow_zeroed
        // initializes every remaining returned byte. This borrow ends before
        // resizing the block again.
        let contents =
            unsafe { std::slice::from_raw_parts(grown.cast::<u8>().as_ptr(), grown.len()) };
        assert!(contents[..32].iter().all(|&byte| byte == 0xa5));
        assert!(contents[32..].iter().all(|&byte| byte == 0));
        assert_eq!(monitor.snapshot().live_blocks, 1);
        // SAFETY: grown is this allocator's live block with fitting large
        // layout. New size is smaller; all temporary borrows ended and the
        // caller exclusively permits invalidation.
        let shrunk = unsafe { allocator.shrink(grown.cast::<u8>(), large, smaller) }.unwrap();
        // SAFETY: Successful shrink preserves the initialized 8-byte prefix.
        // No mutation occurs during this temporary shared borrow.
        let prefix = unsafe { std::slice::from_raw_parts(shrunk.cast::<u8>().as_ptr(), 8) };
        assert!(prefix.iter().all(|&byte| byte == 0xa5));
        // SAFETY: shrunk is the current live pointer; smaller is its exact
        // allocation layout, and its temporary shared borrow has ended.
        unsafe { allocator.deallocate(shrunk.cast::<u8>(), smaller) };
        let stats = monitor.snapshot();
        assert_eq!(stats.attempts, 4);
        assert_eq!(stats.failures, 1);
        assert_eq!(stats.grow_zeroed_calls, 1);
        assert_eq!(stats.shrink_calls, 2);
        monitor.assert_released();
    }

    #[test]
    fn resizing_to_and_from_zero_replaces_only_positive_size_records() {
        let allocator = allocator();
        let monitor = allocator.monitor();
        let zero = Layout::from_size_align(0, 32).unwrap();
        let positive = Layout::from_size_align(24, 64).unwrap();
        let block = allocator.allocate(zero).unwrap();
        // SAFETY: This allocator returned the live zero-size block; zero fits
        // it, positive is larger, and no payload references exist to invalidate.
        let block = unsafe { allocator.grow_zeroed(block.cast::<u8>(), zero, positive) }.unwrap();
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert!(monitor.fail_next());
        // SAFETY: block is the current positive-size allocation and positive
        // fits. A zero-size shrink is permitted; no payload borrow is live.
        let block = unsafe { allocator.shrink(block.cast::<u8>(), positive, zero) }.unwrap();
        assert_eq!(monitor.snapshot().live_blocks, 0);
        assert_eq!(monitor.snapshot().attempts, 1);
        // SAFETY: This is the live zero-size result of shrink; its exact layout
        // fits and there are no outstanding references or duplicate owners.
        unsafe { allocator.deallocate(block.cast::<u8>(), zero) };
        assert!(allocator.allocate(positive).is_err());
        assert_eq!(monitor.snapshot().attempts, 2);
        monitor.assert_released();
    }

    #[test]
    fn distinct_clones_move_swap_return_and_drop_with_their_own_blocks() {
        fn returned(allocator: TrackingAllocator) -> Vec<u8, TrackingAllocator> {
            let mut value = Vec::new_in(allocator);
            value.try_reserve_exact(6).unwrap();
            value.extend_from_slice(b"return");
            value
        }
        let original = allocator();
        let monitor = original.monitor();
        let first = original.clone();
        let second = original.clone();
        let first_id = first.identity();
        let second_id = second.identity();
        assert_ne!(first_id, original.identity());
        assert_ne!(first_id, second_id);
        let mut left = returned(first);
        let mut right = Vec::new_in(second);
        right.try_reserve_exact(3).unwrap();
        right.extend_from_slice(b"two");
        drop(original);
        assert_eq!(monitor.snapshot().live_blocks, 2);
        assert!(!monitor.snapshot().violations.any());
        std::mem::swap(&mut left, &mut right);
        assert_eq!(left.allocator().identity(), second_id);
        assert_eq!(right.allocator().identity(), first_id);
        assert_eq!(left.as_slice(), b"two");
        assert_eq!(right.as_slice(), b"return");
        drop(left);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(right);
        assert_eq!(monitor.snapshot().identities_created, 3);
        assert_eq!(monitor.snapshot().owners_dropped, 3);
        monitor.assert_released();
    }

    #[test]
    fn non_clone_allocator_works_through_shared_references() {
        fn fresh<A: Allocator + Clone>(allocator: A) -> Vec<u32, A> {
            let mut result = Vec::new_in(allocator);
            result.try_reserve_exact(3).unwrap();
            result.extend_from_slice(&[11, 22, 33]);
            result
        }
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        let left = fresh(&allocator);
        let right = fresh(&allocator);
        assert_eq!(left.as_slice(), &[11, 22, 33]);
        assert_eq!(right.as_slice(), left.as_slice());
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(left);
        drop(right);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn owned_non_clone_allocator_moves_and_returns_without_a_clone_bound() {
        fn output<A: Allocator>(allocator: A) -> Vec<u8, A> {
            let mut bytes = Vec::new_in(allocator);
            bytes.try_reserve_exact(5).unwrap();
            bytes.extend_from_slice(b"owned");
            bytes
        }
        let allocator = allocator().into_non_clone();
        let identity = allocator.identity();
        let monitor = allocator.monitor();
        let mut bytes = output(allocator);
        assert_eq!(bytes.allocator().identity(), identity);
        bytes.try_reserve_exact(128).unwrap();
        assert_eq!(bytes.as_slice(), b"owned");
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().grow_calls, 1);
        drop(bytes);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
        monitor.assert_released();
    }

    #[test]
    fn ledger_exhaustion_is_fallible_and_existing_records_remain_reusable() {
        let allocator = TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap();
        let monitor = allocator.monitor();
        let mut first = Vec::new_in(&allocator);
        first.try_reserve_exact(4).unwrap();
        first.extend_from_slice(b"live");
        let mut second = Vec::<u8, _>::new_in(&allocator);
        assert!(second.try_reserve_exact(1).is_err());
        assert_eq!(first.as_slice(), b"live");
        first.try_reserve_exact(128).unwrap();
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(first);
        second.try_reserve_exact(8).unwrap();
        assert_eq!(monitor.snapshot().peak_live_blocks, 1);
        drop(second);
        monitor.assert_released();
    }

    #[test]
    fn every_observed_failure_ordinal_releases_partially_constructed_owners() {
        fn workload(
            allocator: &TrackingAllocator,
        ) -> Result<(), std::collections::TryReserveError> {
            let mut first = Vec::new_in(allocator.clone());
            first.try_reserve_exact(4)?;
            first.extend_from_slice(b"seed");
            let mut second = Vec::new_in(allocator.clone());
            second.try_reserve_exact(7)?;
            second.extend_from_slice(b"another");
            first.try_reserve_exact(128)?;
            second.try_reserve_exact(256)?;
            assert_eq!(first.as_slice(), b"seed");
            assert_eq!(second.as_slice(), b"another");
            Ok(())
        }
        let baseline = allocator();
        let baseline_monitor = baseline.monitor();
        workload(&baseline).unwrap();
        let attempts = baseline_monitor.snapshot().attempts;
        assert!(attempts >= 4);
        baseline_monitor.assert_released();
        for ordinal in 1..=attempts {
            let allocator = TrackingAllocator::new(AllocatorConfig {
                fail_at: NonZeroU64::new(ordinal),
                ..AllocatorConfig::default()
            })
            .unwrap();
            let monitor = allocator.monitor();
            assert!(workload(&allocator).is_err(), "failure ordinal {ordinal}");
            let snapshot = monitor.snapshot();
            assert_eq!(snapshot.attempts, ordinal);
            assert_eq!(snapshot.failures, 1);
            drop(allocator);
            monitor.assert_released();
        }
    }

    #[test]
    fn clone_id_overflow_and_poisoned_bookkeeping_are_flags_not_callback_panics() {
        let allocator = allocator();
        let monitor = allocator.monitor();
        allocator.shared.lock().next_identity = u64::MAX;
        let exhausted = allocator.clone();
        assert!(exhausted.allocate(Layout::new::<u8>()).is_err());
        assert_eq!(exhausted.identity(), 0);
        drop(exhausted);
        assert!(monitor.snapshot().violations.counter_overflow);
        assert_eq!(monitor.snapshot().live_blocks, 0);

        // Deliberately poison only private test bookkeeping, outside all
        // allocator operations. Recovery must not unwind in allocate or Drop.
        let separate = super::TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let observer = separate.monitor();
        let poisoned = std::panic::catch_unwind(|| {
            let _guard = separate.shared.ledger.lock().unwrap();
            panic!("intentional test-only poison");
        });
        assert!(poisoned.is_err());
        let mut bytes = Vec::new_in(&separate);
        bytes.try_reserve_exact(3).unwrap();
        bytes.extend_from_slice(b"yes");
        drop(bytes);
        drop(separate);
        let snapshot = observer.snapshot();
        assert!(snapshot.violations.poisoned_lock);
        assert_eq!(snapshot.live_blocks, 0);
        assert_eq!(snapshot.live_bytes, 0);
    }

    #[test]
    fn concurrent_distinct_owners_share_only_bookkeeping() {
        let allocator = allocator();
        let monitor = allocator.monitor();
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let mut threads = Vec::new();
        for number in 0..4 {
            let child = allocator.clone();
            let barrier = Arc::clone(&barrier);
            threads.push(std::thread::spawn(move || {
                let mut bytes = Vec::new_in(child);
                bytes.try_reserve_exact(32).unwrap();
                bytes.resize(32, number);
                barrier.wait();
                assert!(bytes.iter().all(|&byte| byte == number));
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        let stats = monitor.snapshot();
        assert_eq!(stats.attempts, 4);
        assert_eq!(stats.peak_live_blocks, 4);
        assert_eq!(stats.identities_created, 5);
        drop(allocator);
        monitor.assert_released();
    }
}
