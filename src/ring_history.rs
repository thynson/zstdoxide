//! Bounded, resumable LZ history independent of the frame's logical length.
//!
//! Corresponds to prefix/window retention in upstream `ZSTD_DCtx` and the
//! match-copy part of `ZSTD_execSequence`. No caller buffer is retained.

use alloc::alloc::{Allocator, Global};
use alloc::vec::Vec;

use crate::{Error, Result};

/// Owns the active frame's last window bytes, with reusable allocation.
///
/// Before the ring fills, `head == bytes.len()`. Once full, `head < window`
/// identifies the oldest byte and the next slot to overwrite. The initialized
/// length never exceeds `window`, which never exceeds `maximum_window`.
/// `produced` is independent of the physical index and never wraps.
/// The vector retains the exact allocator that owns its storage. Moving the
/// complete ring moves that owner; no allocator cloning or equivalence is needed.
pub(crate) struct RingHistory<A: Allocator = Global> {
    bytes: Vec<u8, A>,
    head: usize,
    window: usize,
    maximum_window: usize,
    produced: u64,
}

impl RingHistory<Global> {
    /// Create an empty context without allocating the maximum window eagerly.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "convenience entry point retained for direct callers"
        )
    )]
    pub(crate) fn new(maximum_window: usize) -> Self {
        Self::new_in(maximum_window, Global)
    }
}

impl<A: Allocator> RingHistory<A> {
    /// Create an empty ring with its own allocator, without allocating bytes.
    ///
    /// The allocator may be an owned non-Clone value or an external reference.
    /// Reservation, storage retention, and final deallocation use that exact
    /// retained owner. No reference into the ring itself is created.
    pub(crate) fn new_in(maximum_window: usize, allocator: A) -> Self {
        Self {
            bytes: Vec::new_in(allocator),
            head: 0,
            window: 0,
            maximum_window,
            produced: 0,
        }
    }

    /// Start an independent frame, retaining the allocation from earlier ones.
    ///
    /// Validates and fallibly reserves before clearing old state. Bytes are
    /// initialized as they are emitted, not when the advertised window arrives.
    /// Corresponds to resetting the active prefix in `ZSTD_decompressBegin`.
    pub(crate) fn start_frame(&mut self, window: usize) -> Result<()> {
        if window > self.maximum_window {
            return Err(Error::ResourceLimit);
        }
        if self.bytes.capacity() < window {
            // len <= capacity < window, so subtraction cannot underflow. The
            // requested final capacity is window regardless of the old length.
            self.bytes
                .try_reserve_exact(window - self.bytes.len())
                .map_err(|_| Error::AllocationFailed)?;
        }
        self.clear();
        self.window = window;
        Ok(())
    }

    /// Clear frame-local state while retaining allocated storage for reuse.
    ///
    /// A new frame must be started before producing more bytes. Retained
    /// capacity is the high-water allocation of previous successful windows;
    /// selecting a smaller next window does not shrink that allocation.
    pub(crate) fn clear(&mut self) {
        self.bytes.clear();
        self.head = 0;
        self.window = 0;
        self.produced = 0;
    }

    pub(crate) fn frame_len(&self) -> u64 {
        self.produced
    }

    pub(crate) fn available(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn window_size(&self) -> usize {
        self.window
    }

    pub(crate) fn retained_capacity(&self) -> usize {
        self.bytes.capacity()
    }

    /// Read an absolute position in the active frame's retained history.
    ///
    /// Positions must precede `frame_len()` and be no older than `available()`
    /// bytes. The returned value owns no borrow, and this method changes no
    /// history state. Corresponds to reading the retained prefix in upstream
    /// `ZSTD_execSequence`, with checked frame indices replacing pointers.
    pub(crate) fn byte_at(&self, frame_position: u64) -> Result<u8> {
        if frame_position >= self.produced {
            return Err(Error::CorruptData);
        }
        let distance =
            usize::try_from(self.produced - frame_position).map_err(|_| Error::CorruptData)?;
        if distance > self.bytes.len() {
            return Err(Error::CorruptData);
        }
        let source = if distance <= self.head {
            self.head - distance
        } else {
            self.window - (distance - self.head)
        };
        #[cfg(not(feature = "unsafe-ring-history"))]
        let byte = self.bytes[source];
        #[cfg(feature = "unsafe-ring-history")]
        let byte = {
            // SAFETY: frame_position < produced makes distance positive, and
            // conversion plus the explicit length check prove 1 <= distance
            // <= len. Before filling, head == len, so source == len - distance
            // < len. Once full, len == window and head < window: the first
            // branch subtracts into 0..len; the second has
            // 1 <= distance - head <= window and also yields 0..len.
            // The Vec retains the allocator owning its live u8 allocation,
            // correctly aligned with every element below len initialized.
            // The shared borrow of self
            // prevents mutation or reallocation for this read and permits no
            // conflicting exclusive alias. Pointer arithmetic stays inside
            // this allocation; read copies one byte without retaining a
            // reference or pointer beyond this expression.
            unsafe { self.bytes.as_ptr().add(source).read() }
        };
        Ok(byte)
    }

    /// Commit complete staged output without copying to another output slice.
    ///
    /// Checks the entire logical advancement before mutation. Empty input is
    /// accepted even with a zero window; a nonempty commit requires an active
    /// window. Corresponds to retaining decoded block content in the upstream
    /// `ZSTD_DCtx` prefix after block execution.
    pub(crate) fn commit(&mut self, input: &[u8]) -> Result<()> {
        let produced = self.checked_advance(input.len())?;
        for &byte in input {
            #[cfg(not(feature = "unsafe-ring-history"))]
            self.store(byte);
            #[cfg(feature = "unsafe-ring-history")]
            {
                // SAFETY: this loop executes only for nonempty input, so the
                // complete checked_advance proved window > 0 before mutation.
                // start_frame bounded window by maximum_window and reserved
                // capacity >= window. Constructors and every store preserve
                // initialized len <= window, head == len before filling, and
                // head < window once full. All metadata and byte are initialized.
                // self exclusively owns the Vec and its allocator, keeping
                // the allocation live and correctly aligned for u8. No history
                // references or pointers escape.
                // The exclusive borrow prevents conflicting access, including
                // input aliasing this allocation, and no reallocation occurs.
                unsafe { self.store_unchecked(byte) };
            }
        }
        self.produced = produced;
        Ok(())
    }

    /// Emit the literal prefix that fits in `output`, returning its length.
    ///
    /// The caller resumes with `input[written..]`. Errors leave both buffers
    /// and logical state unchanged. Corresponds to literal copying within
    /// upstream `ZSTD_execSequence` and raw-block decoding.
    pub(crate) fn append(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
        let count = input.len().min(output.len());
        let produced = self.checked_advance(count)?;
        for (destination, &byte) in output[..count].iter_mut().zip(&input[..count]) {
            #[cfg(not(feature = "unsafe-ring-history"))]
            self.store(byte);
            #[cfg(feature = "unsafe-ring-history")]
            {
                // SAFETY: this loop is entered only when count > 0, so
                // checked_advance proved window > 0 before any mutation.
                // start_frame bounded window by maximum_window and reserved
                // capacity >= window; constructors and stores preserve
                // len <= window and the head invariant.
                // The Vec and its allocator remain live and uniquely borrowed
                // by self; no history references escape. The prefix is initialized,
                // and the byte value is initialized. No reallocation occurs.
                unsafe { self.store_unchecked(byte) };
            }
            *destination = byte;
        }
        self.produced = produced;
        Ok(count)
    }

    /// Emit an RLE prefix, returning the count to subtract from `remaining`.
    ///
    /// Corresponds to upstream `ZSTD_setRleBlock`, with caller-owned output
    /// and history retained independently across partial writes.
    pub(crate) fn repeat(
        &mut self,
        byte: u8,
        remaining: usize,
        output: &mut [u8],
    ) -> Result<usize> {
        let count = remaining.min(output.len());
        let produced = self.checked_advance(count)?;
        for destination in &mut output[..count] {
            #[cfg(not(feature = "unsafe-ring-history"))]
            self.store(byte);
            #[cfg(feature = "unsafe-ring-history")]
            {
                // SAFETY: this loop is entered only when count > 0, so
                // checked_advance proved window > 0 before any mutation.
                // start_frame bounded window by maximum_window and reserved
                // capacity >= window; constructors and stores preserve
                // len <= window and the head invariant.
                // The Vec and its allocator remain live and uniquely borrowed
                // by self; no history references escape. The prefix is initialized,
                // and the byte value is initialized. No reallocation occurs.
                unsafe { self.store_unchecked(byte) };
            }
            *destination = byte;
        }
        self.produced = produced;
        Ok(count)
    }

    /// Emit a match prefix, retaining overlap semantics across resumptions.
    ///
    /// Distance must be nonzero and within available active-frame history,
    /// even for an empty output slice. Repeating this call with the same
    /// distance and a reduced `remaining` resumes the same LZ match.
    /// Corresponds to the match-copy part of upstream `ZSTD_execSequence`.
    // Retained for direct history tests and backend benchmarks; compressed
    // block decoding now reads immutable history and commits staged output.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn copy_match(
        &mut self,
        distance: usize,
        remaining: usize,
        output: &mut [u8],
    ) -> Result<usize> {
        if distance == 0 || distance > self.bytes.len() {
            return Err(Error::CorruptData);
        }
        let count = remaining.min(output.len());
        let produced = self.checked_advance(count)?;
        for destination in &mut output[..count] {
            let source = if distance <= self.head {
                self.head - distance
            } else {
                self.window - (distance - self.head)
            };
            // Read before overwriting: at distance == window the source and
            // destination ring slots coincide, and the old byte is required.
            #[cfg(not(feature = "unsafe-ring-history"))]
            let byte = self.bytes[source];
            #[cfg(feature = "unsafe-ring-history")]
            let byte = {
                // SAFETY: distance was checked in 1..=bytes.len(). Before the
                // ring fills, head == len, so source == len - distance < len.
                // Once full, len == window and head < window. For distance <=
                // head, subtraction yields source < len; otherwise
                // 1 <= distance - head <= window, so window minus that value
                // lies in 0..len. Stores preserve these invariants each turn.
                // The Vec's initialized u8 prefix is live, aligned for u8, and
                // owned by self through its retained allocator. This read copies
                // the byte and leaves no borrow or pointer alive across the
                // following exclusive store.
                unsafe { self.bytes.as_ptr().add(source).read() }
            };
            #[cfg(not(feature = "unsafe-ring-history"))]
            self.store(byte);
            #[cfg(feature = "unsafe-ring-history")]
            {
                // SAFETY: this loop is entered only when count > 0, so
                // checked_advance proved window > 0 before any mutation.
                // start_frame bounded window by maximum_window and reserved
                // capacity >= window; constructors and stores preserve
                // len <= window and the head invariant.
                // The Vec and its allocator remain live and uniquely borrowed
                // by self; no history references escape. The prefix is initialized,
                // and the byte value is initialized. No reallocation occurs.
                unsafe { self.store_unchecked(byte) };
            }
            *destination = byte;
        }
        self.produced = produced;
        Ok(count)
    }

    /// Validate the complete count before mutating output or history.
    fn checked_advance(&self, count: usize) -> Result<u64> {
        if count != 0 && self.window == 0 {
            return Err(Error::CorruptData);
        }
        let count = u64::try_from(count).map_err(|_| Error::ResourceLimit)?;
        self.produced.checked_add(count).ok_or(Error::ResourceLimit)
    }

    /// Store one byte after an emitting method has validated a nonzero window.
    #[cfg(not(feature = "unsafe-ring-history"))]
    fn store(&mut self, byte: u8) {
        if self.bytes.len() < self.window {
            // start_frame reserved the complete active window; push cannot
            // reallocate, and the initialized prefix grows by exactly one.
            self.bytes.push(byte);
        } else {
            self.bytes[self.head] = byte;
        }
        // head < window before this step, so head + 1 <= window cannot overflow.
        self.head += 1;
        if self.head == self.window {
            self.head = 0;
        }
    }

    /// Store one byte without repeating vector bounds checks.
    ///
    /// # Safety
    ///
    /// - The active window must be nonzero and no greater than the owned Vec's
    ///   capacity or the configured maximum window.
    /// - The initialized vector length must be at most the window. If it is
    ///   smaller, `head` must equal that length; otherwise `head < window`.
    /// - `self` and its Vec allocation must remain live and exclusively owned
    ///   for this call. No other reference or pointer may access that allocation
    ///   during the write. The vector's existing prefix and `byte` must be fully
    ///   initialized; any unused capacity need not be initialized.
    /// - The allocation must be the Vec's own valid u8 allocation, kept live by
    ///   its retained allocator, with its required alignment (one). All struct
    ///   fields must be initialized. No allocator operation occurs in this call.
    ///
    /// These requirements put the write inside the allocation. For a growing
    /// prefix, the newly initialized byte is immediately included in its length.
    /// Scalar metadata uses ordinary checked-invariant arithmetic in both
    /// backends; this helper does not advance the logical frame counter.
    #[cfg(feature = "unsafe-ring-history")]
    unsafe fn store_unchecked(&mut self, byte: u8) {
        let growing = self.bytes.len() < self.window;
        let pointer = self.bytes.as_mut_ptr();
        // SAFETY: the contract gives capacity >= window > 0. If growing,
        // head == len < window <= capacity; otherwise head < window == len.
        // Thus pointer.add(head) addresses one writable u8 inside this live
        // allocation, kept live by the Vec's retained allocator, with alignment
        // one, and write initializes that byte.
        // Exclusive ownership excludes conflicting access or aliases. The
        // pointer is temporary and no allocation or reallocation occurs.
        // In the growing case, len + 1 <= window <= capacity, cannot overflow,
        // and every element below the new length is initialized: the previous
        // prefix already was, and write just initialized the additional byte.
        // Therefore set_len exposes no uninitialized memory. The nongrowing
        // case overwrites a Copy byte and does not change initialized length.
        unsafe {
            pointer.add(self.head).write(byte);
            if growing {
                self.bytes.set_len(self.bytes.len() + 1);
            }
        }
        // head < window at entry, hence incrementing cannot overflow usize.
        self.head += 1;
        if self.head == self.window {
            self.head = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn seed<A: Allocator>(history: &mut RingHistory<A>, bytes: &[u8]) {
        let mut output = vec![0; bytes.len()];
        assert_eq!(history.append(bytes, &mut output).unwrap(), bytes.len());
        assert_eq!(output, bytes);
    }

    #[test]
    fn deferred_lookup_covers_partial_and_wrapped_retained_intervals() {
        for window in 1..=19 {
            let mut history = RingHistory::new(window);
            history.start_frame(window).unwrap();
            let capacity = history.retained_capacity();
            assert_eq!(history.byte_at(0), Err(Error::CorruptData));
            let mut expected = Vec::new();
            for index in 0..window * 4 + 3 {
                let byte = (index * 37) as u8;
                history.commit(&[byte]).unwrap();
                expected.push(byte);
                let start = expected.len().saturating_sub(window);
                for (position, &value) in expected.iter().enumerate().skip(start) {
                    assert_eq!(history.byte_at(position as u64).unwrap(), value);
                }
                if start != 0 {
                    assert_eq!(history.byte_at((start - 1) as u64), Err(Error::CorruptData));
                }
                assert_eq!(
                    history.byte_at(expected.len() as u64),
                    Err(Error::CorruptData)
                );
                assert_eq!(history.byte_at(u64::MAX), Err(Error::CorruptData));
                assert_eq!(history.retained_capacity(), capacity);
            }
            history.start_frame(window).unwrap();
            assert_eq!(history.byte_at(0), Err(Error::CorruptData));
            history.commit(b"x").unwrap();
            assert_eq!(history.byte_at(0).unwrap(), b'x');
            assert_eq!(history.byte_at(1), Err(Error::CorruptData));
        }
    }

    #[test]
    fn deferred_lookup_uses_wide_logical_positions_and_independent_ring_head() {
        let mut history = RingHistory::new(7);
        history.start_frame(7).unwrap();
        history.commit(b"abcdefghijk").unwrap();
        history.produced = u64::from(u32::MAX) + 91;
        let start = history.frame_len() - 7;
        for (index, &byte) in b"efghijk".iter().enumerate() {
            assert_eq!(history.byte_at(start + index as u64).unwrap(), byte);
        }
        for position in [0, start - 1, history.frame_len(), u64::MAX] {
            assert_eq!(history.byte_at(position), Err(Error::CorruptData));
        }
        history.commit(b"LMN").unwrap();
        assert_eq!(history.byte_at(start), Err(Error::CorruptData));
        for (index, &byte) in b"hijkLMN".iter().enumerate() {
            assert_eq!(history.byte_at(start + 3 + index as u64).unwrap(), byte);
        }
        history.produced = u64::MAX;
        for (index, &byte) in b"hijkLMN".iter().enumerate() {
            assert_eq!(history.byte_at(u64::MAX - 7 + index as u64).unwrap(), byte);
        }
        assert_eq!(history.byte_at(u64::MAX), Err(Error::CorruptData));
    }

    #[test]
    fn deferred_commit_checks_entire_advancement_before_mutation() {
        let mut history = RingHistory::new(3);
        assert_eq!(history.commit(b"x"), Err(Error::CorruptData));
        history.commit(&[]).unwrap();
        history.start_frame(3).unwrap();
        history.commit(b"abc").unwrap();
        history.produced = u64::MAX - 2;
        let bytes = history.bytes.clone();
        let head = history.head;
        let capacity = history.retained_capacity();
        assert_eq!(history.commit(b"xyz"), Err(Error::ResourceLimit));
        assert_eq!(history.bytes, bytes);
        assert_eq!(history.head, head);
        assert_eq!(history.frame_len(), u64::MAX - 2);
        assert_eq!(history.window_size(), 3);
        assert_eq!(history.retained_capacity(), capacity);
        history.commit(b"XY").unwrap();
        assert_eq!(history.frame_len(), u64::MAX);
        for (index, &byte) in b"cXY".iter().enumerate() {
            assert_eq!(history.byte_at(u64::MAX - 3 + index as u64).unwrap(), byte);
        }
        history.commit(&[]).unwrap();
        assert_eq!(history.commit(b"Z"), Err(Error::ResourceLimit));
        assert_eq!(history.byte_at(u64::MAX - 1).unwrap(), b'Y');
        history.clear();
        history.commit(&[]).unwrap();
        assert_eq!(history.commit(b"x"), Err(Error::CorruptData));
        assert_eq!(history.available(), 0);
        assert_eq!(history.frame_len(), 0);
        assert_eq!(history.byte_at(0), Err(Error::CorruptData));
        assert_eq!(history.retained_capacity(), capacity);
    }

    #[test]
    fn deferred_block_reads_previous_prefix_before_commit_evicts_it() {
        let mut history = RingHistory::new(8);
        history.start_frame(8).unwrap();
        seed(&mut history, b"abcdef");
        let bytes = history.bytes.clone();
        let head = history.head;
        // A dictionary match at P=6, distance=10, length=8 emits four
        // dictionary bytes, then frame[0..4]. Immediate ring writes would
        // evict frame[0] at output position 8 before reading it at position 10.
        let mut pending = b"1234".to_vec();
        for position in 0..4 {
            pending.push(history.byte_at(position).unwrap());
        }
        assert_eq!(pending, b"1234abcd");
        assert_eq!(history.bytes, bytes);
        assert_eq!(history.head, head);
        assert_eq!(history.frame_len(), 6);
        history.commit(&pending).unwrap();
        assert_eq!(history.frame_len(), 14);
        assert_eq!(history.byte_at(0), Err(Error::CorruptData));
        for (index, &byte) in pending.iter().enumerate() {
            assert_eq!(history.byte_at(6 + index as u64).unwrap(), byte);
        }
        let mut output = [0; 8];
        history.copy_match(4, 8, &mut output).unwrap();
        assert_eq!(&output, b"abcdabcd");
    }

    #[test]
    fn literals_rle_and_zero_output_resume_without_retaining_caller_buffers() {
        let mut history = RingHistory::new(5);
        history.start_frame(5).unwrap();
        assert_eq!(history.append(b"abcdefg", &mut []).unwrap(), 0);
        let mut output = [99; 4];
        assert_eq!(history.append(b"abcdefg", &mut output).unwrap(), 4);
        assert_eq!(&output, b"abcd");
        assert_eq!(history.append(b"efg", &mut output).unwrap(), 3);
        assert_eq!(&output, b"efgd");
        assert_eq!(history.repeat(b'x', 9, &mut output).unwrap(), 4);
        assert_eq!(&output, b"xxxx");
        assert_eq!(history.repeat(b'y', 2, &mut output).unwrap(), 2);
        assert_eq!(&output, b"yyxx");
        assert_eq!(history.frame_len(), 13);
        assert_eq!(history.available(), 5);
        assert_eq!(history.window_size(), 5);
    }

    #[test]
    fn matches_wrap_at_every_distance_and_preserve_whole_window_sources() {
        for window in 1..=19 {
            for distance in 1..=window {
                let mut history = RingHistory::new(window);
                history.start_frame(window).unwrap();
                let prefix: Vec<u8> = (0..window * 3 + 5).map(|n| n as u8).collect();
                seed(&mut history, &prefix);
                let length = window * 4 + 11;
                let mut expected = prefix.clone();
                for _ in 0..length {
                    expected.push(expected[expected.len() - distance]);
                }
                let mut actual = vec![0; length];
                assert_eq!(
                    history.copy_match(distance, length, &mut actual).unwrap(),
                    length
                );
                assert_eq!(actual, expected[prefix.len()..]);
                assert_eq!(history.available(), window);
            }
        }
    }

    #[test]
    fn partially_filled_match_can_overlap_and_resume_with_irregular_outputs() {
        for distance in 1..=17 {
            let mut history = RingHistory::new(17);
            history.start_frame(17).unwrap();
            let prefix: Vec<u8> = (0..distance as u8).collect();
            seed(&mut history, &prefix);
            let mut actual = Vec::new();
            let mut calls = 0;
            while actual.len() < 211 {
                let capacity = [0, 1, 2, 5, 31][calls % 5];
                calls += 1;
                let mut output = vec![0; capacity];
                let count = history
                    .copy_match(distance, 211 - actual.len(), &mut output)
                    .unwrap();
                actual.extend_from_slice(&output[..count]);
            }
            let expected: Vec<u8> = prefix.iter().copied().cycle().take(211).collect();
            assert_eq!(actual, expected);
            assert_eq!(history.frame_len(), (distance + 211) as u64);
        }
    }

    #[test]
    fn resets_reuse_capacity_and_forbid_cross_frame_matches() {
        let mut history = RingHistory::new(32);
        history.start_frame(32).unwrap();
        let capacity = history.retained_capacity();
        seed(&mut history, b"previous frame");
        history.start_frame(3).unwrap();
        assert_eq!(history.available(), 0);
        assert_eq!(history.frame_len(), 0);
        assert_eq!(history.retained_capacity(), capacity);
        assert_eq!(history.copy_match(1, 1, &mut [0]), Err(Error::CorruptData));
        seed(&mut history, b"new frame");
        history.clear();
        assert_eq!(history.available(), 0);
        assert_eq!(history.window_size(), 0);
        assert_eq!(history.frame_len(), 0);
        assert_eq!(history.retained_capacity(), capacity);
    }

    #[test]
    fn rejects_invalid_distances_and_windows_without_mutation() {
        let mut history = RingHistory::new(3);
        history.start_frame(3).unwrap();
        seed(&mut history, b"abcd");
        let bytes = history.bytes.clone();
        let head = history.head;
        let mut output = [99; 4];
        for distance in [0, 4, usize::MAX] {
            assert_eq!(
                history.copy_match(distance, 4, &mut output),
                Err(Error::CorruptData)
            );
            assert_eq!(
                history.copy_match(distance, 0, &mut []),
                Err(Error::CorruptData)
            );
        }
        assert_eq!(history.start_frame(4), Err(Error::ResourceLimit));
        assert_eq!(history.bytes, bytes);
        assert_eq!(history.head, head);
        assert_eq!(history.frame_len(), 4);
        assert_eq!(output, [99; 4]);
    }

    #[test]
    fn zero_window_allows_empty_progress_and_rejects_produced_bytes() {
        let mut history = RingHistory::new(0);
        history.start_frame(0).unwrap();
        let mut output = [99];
        assert_eq!(history.append(&[], &mut output).unwrap(), 0);
        assert_eq!(history.append(b"x", &mut []).unwrap(), 0);
        assert_eq!(history.repeat(0, 0, &mut output).unwrap(), 0);
        assert_eq!(history.append(b"x", &mut output), Err(Error::CorruptData));
        assert_eq!(history.repeat(0, 1, &mut output), Err(Error::CorruptData));
        assert_eq!(history.copy_match(1, 0, &mut []), Err(Error::CorruptData));
        assert_eq!(output, [99]);
        assert_eq!(history.retained_capacity(), 0);
    }

    #[test]
    fn allocation_failure_preserves_the_previous_frame() {
        let mut history = RingHistory::new(usize::MAX);
        history.start_frame(3).unwrap();
        seed(&mut history, b"abc");
        assert_eq!(
            history.start_frame(usize::MAX),
            Err(Error::AllocationFailed)
        );
        let mut output = [0; 6];
        history.copy_match(3, 6, &mut output).unwrap();
        assert_eq!(&output, b"abcabc");
    }

    #[test]
    fn global_count_above_u32_does_not_determine_ring_indices() {
        let mut history = RingHistory::new(7);
        history.start_frame(7).unwrap();
        seed(&mut history, b"123456789abcdef");
        history.produced = u64::from(u32::MAX) + 91;
        let before = history.frame_len();
        let mut output = [0; 15];
        history.copy_match(3, 15, &mut output).unwrap();
        assert_eq!(&output, b"defdefdefdefdef");
        assert_eq!(history.frame_len(), before + 15);
    }

    #[test]
    fn counter_overflow_is_atomic_for_all_emission_modes() {
        let mut history = RingHistory::new(3);
        history.start_frame(3).unwrap();
        seed(&mut history, b"abc");
        history.produced = u64::MAX - 2;
        let bytes = history.bytes.clone();
        let head = history.head;
        let mut output = [99; 3];
        assert_eq!(
            history.append(b"xyz", &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            history.repeat(b'x', 3, &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            history.copy_match(1, 3, &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(history.bytes, bytes);
        assert_eq!(history.head, head);
        assert_eq!(history.frame_len(), u64::MAX - 2);
        assert_eq!(output, [99; 3]);
        assert_eq!(
            history.copy_match(1, usize::MAX, &mut output[..2]).unwrap(),
            2
        );
        assert_eq!(history.frame_len(), u64::MAX);
        assert_eq!(history.copy_match(1, usize::MAX, &mut []).unwrap(), 0);
        assert_eq!(history.repeat(0, 1, &mut output), Err(Error::ResourceLimit));
    }

    #[test]
    fn retained_history_stays_bounded_over_many_output_chunks() {
        let mut history = RingHistory::new(13);
        history.start_frame(13).unwrap();
        let capacity = history.retained_capacity();
        let mut output = [0; 37];
        for index in 0..4096 {
            history
                .repeat(index as u8, output.len(), &mut output)
                .unwrap();
            assert_eq!(history.available(), 13);
            assert_eq!(history.retained_capacity(), capacity);
        }
        assert_eq!(history.frame_len(), 4096 * 37);
    }

    #[test]
    fn custom_owned_and_borrowed_non_clone_allocators_keep_their_identity() {
        fn moved<A: Allocator>(mut history: RingHistory<A>) -> RingHistory<A> {
            history.commit(b"d").unwrap();
            history
        }

        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let mut history = RingHistory::new_in(16, owner);
        assert_eq!(monitor.snapshot().attempts, 0);
        assert_eq!(history.bytes.allocator().identity(), identity);
        history.start_frame(3).unwrap();
        history.commit(b"abc").unwrap();
        let pointer = history.bytes.as_ptr();
        let history = moved(history);
        assert_eq!(history.bytes.as_ptr(), pointer);
        assert_eq!(history.bytes.allocator().identity(), identity);
        assert_eq!(history.byte_at(1).unwrap(), b'b');
        assert_eq!(history.byte_at(3).unwrap(), b'd');
        drop(history);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
        monitor.assert_released();

        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut left = RingHistory::new_in(8, &owner);
        let mut right = RingHistory::new_in(8, &owner);
        left.start_frame(3).unwrap();
        right.start_frame(5).unwrap();
        left.commit(b"one").unwrap();
        right.commit(b"other").unwrap();
        assert_eq!(left.bytes.allocator().identity(), owner.identity());
        assert_eq!(right.bytes.allocator().identity(), owner.identity());
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().live_blocks, 2);
        drop(left);
        drop(right);
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(owner);
        monitor.assert_released();
    }

    #[test]
    fn custom_failed_initial_reservation_preserves_empty_state_and_recovers() {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut history = RingHistory::new_in(64, owner);
        let pointer = history.bytes.as_ptr();
        assert!(monitor.fail_next());
        assert_eq!(history.start_frame(9), Err(Error::AllocationFailed));
        assert_eq!(history.bytes.as_ptr(), pointer);
        assert_eq!(history.retained_capacity(), 0);
        assert_eq!(history.available(), 0);
        assert_eq!(history.head, 0);
        assert_eq!(history.window_size(), 0);
        assert_eq!(history.frame_len(), 0);
        assert_eq!(monitor.snapshot().attempts, 1);
        assert_eq!(monitor.snapshot().failures, 1);
        assert_eq!(monitor.snapshot().live_blocks, 0);
        monitor.clear_failure();
        history.start_frame(9).unwrap();
        assert!(history.retained_capacity() >= 9);
        history.commit(b"recovered").unwrap();
        assert_eq!(history.byte_at(8).unwrap(), b'd');
        drop(history);
        monitor.assert_released();
    }

    #[test]
    fn custom_failed_growth_preserves_wrapped_history_and_previous_matches() {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut history = RingHistory::new_in(usize::MAX, owner);
        history.start_frame(7).unwrap();
        history.commit(b"abcdefghijklmnopqr").unwrap();
        let bytes = history.bytes.to_vec();
        let pointer = history.bytes.as_ptr();
        let capacity = history.retained_capacity();
        let head = history.head;
        let before = monitor.snapshot();
        let larger = capacity.checked_add(1).unwrap();
        assert!(monitor.fail_next());
        assert_eq!(history.start_frame(larger), Err(Error::AllocationFailed));
        assert_eq!(history.bytes.as_slice(), bytes);
        assert_eq!(history.bytes.as_ptr(), pointer);
        assert_eq!(history.retained_capacity(), capacity);
        assert_eq!(history.head, head);
        assert_eq!(history.window_size(), 7);
        assert_eq!(history.frame_len(), 18);
        for (position, &byte) in b"lmnopqr".iter().enumerate() {
            assert_eq!(history.byte_at(11 + position as u64).unwrap(), byte);
        }
        let after = monitor.snapshot();
        assert_eq!(after.attempts, before.attempts + 1);
        assert_eq!(after.grow_calls, before.grow_calls + 1);
        assert_eq!(after.failures, before.failures + 1);
        assert_eq!(after.live_blocks, before.live_blocks);
        assert_eq!(after.live_bytes, before.live_bytes);
        assert_eq!(after.live_requested_bytes, before.live_requested_bytes);

        let mut output = [0; 14];
        assert_eq!(history.copy_match(7, 14, &mut output).unwrap(), 14);
        assert_eq!(&output, b"lmnopqrlmnopqr");
        monitor.clear_failure();
        history.start_frame(larger).unwrap();
        assert!(history.retained_capacity() >= larger);
        assert_eq!(history.window_size(), larger);
        assert_eq!(history.available(), 0);
        assert_eq!(history.frame_len(), 0);
        assert_eq!(history.byte_at(0), Err(Error::CorruptData));
        history.commit(b"new").unwrap();
        assert_eq!(history.byte_at(2).unwrap(), b'w');
        drop(history);
        monitor.assert_released();
    }

    #[test]
    fn custom_emission_clear_and_smaller_windows_reuse_the_reserved_allocation() {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut history = RingHistory::new_in(usize::MAX, owner);
        history.start_frame(17).unwrap();
        let capacity = history.retained_capacity();
        let pointer = history.bytes.as_ptr();
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        let mut output = [99; 5];
        assert_eq!(history.append(b"abcde", &mut output).unwrap(), 5);
        assert_eq!(&output, b"abcde");
        assert_eq!(history.append(b"x", &mut []).unwrap(), 0);
        assert_eq!(history.repeat(b'Z', 3, &mut output).unwrap(), 3);
        assert_eq!(&output, b"ZZZde");
        history.commit(b"0123456789abcdefghijkl").unwrap();
        assert_eq!(history.byte_at(history.frame_len() - 1).unwrap(), b'l');
        assert_eq!(history.copy_match(3, 5, &mut output).unwrap(), 5);
        assert_eq!(&output, b"jkljk");
        for window in [17, 7, 0, 3] {
            history.start_frame(window).unwrap();
            assert_eq!(history.available(), 0);
            assert_eq!(history.frame_len(), 0);
            assert_eq!(history.bytes.as_ptr(), pointer);
            assert_eq!(history.retained_capacity(), capacity);
            assert_eq!(history.copy_match(1, 0, &mut []), Err(Error::CorruptData));
            if window != 0 {
                history.commit(b"more retained bytes").unwrap();
            }
            history.clear();
            history.commit(&[]).unwrap();
            assert_eq!(history.commit(b"x"), Err(Error::CorruptData));
        }
        let after = monitor.snapshot();
        assert_eq!(after.attempts, before.attempts);
        assert_eq!(after.successes, before.successes);
        assert_eq!(after.deallocate_calls, before.deallocate_calls);
        assert_eq!(after.live_bytes, before.live_bytes);
        // The failed request is still armed: none of the previous operations
        // called the allocator, including filling and wrapping the ring.
        assert_eq!(
            history.start_frame(capacity.checked_add(1).unwrap()),
            Err(Error::AllocationFailed)
        );
        assert_eq!(monitor.snapshot().attempts, before.attempts + 1);
        assert_eq!(monitor.snapshot().failures, before.failures + 1);
        drop(history);
        monitor.assert_released();
    }

    #[test]
    fn custom_validation_and_counter_overflow_do_not_request_storage_or_mutate() {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let mut history = RingHistory::new_in(7, owner);
        history.start_frame(3).unwrap();
        history.commit(b"abcdef").unwrap();
        history.produced = u64::MAX - 2;
        let bytes = history.bytes.to_vec();
        let pointer = history.bytes.as_ptr();
        let capacity = history.retained_capacity();
        let head = history.head;
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        assert_eq!(history.start_frame(8), Err(Error::ResourceLimit));
        let mut output = [99; 3];
        assert_eq!(history.commit(b"xyz"), Err(Error::ResourceLimit));
        assert_eq!(
            history.append(b"xyz", &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            history.repeat(b'x', 3, &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            history.copy_match(1, 3, &mut output),
            Err(Error::ResourceLimit)
        );
        for distance in [0, 4, usize::MAX] {
            assert_eq!(
                history.copy_match(distance, 0, &mut []),
                Err(Error::CorruptData)
            );
        }
        assert_eq!(output, [99; 3]);
        assert_eq!(history.bytes.as_slice(), bytes);
        assert_eq!(history.bytes.as_ptr(), pointer);
        assert_eq!(history.retained_capacity(), capacity);
        assert_eq!(history.head, head);
        assert_eq!(history.window_size(), 3);
        assert_eq!(history.frame_len(), u64::MAX - 2);
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        assert_eq!(monitor.snapshot().failures, before.failures);
        assert_eq!(history.start_frame(7), Err(Error::AllocationFailed));
        assert_eq!(monitor.snapshot().attempts, before.attempts + 1);
        assert_eq!(history.frame_len(), u64::MAX - 2);
        assert_eq!(history.bytes.as_slice(), bytes);
        drop(history);
        monitor.assert_released();
    }

    #[test]
    fn custom_wrap_and_fragmented_matches_cover_every_small_window_and_distance() {
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        for window in 1..=19 {
            for distance in 1..=window {
                let mut history = RingHistory::new_in(window, &owner);
                history.start_frame(window).unwrap();
                let prefix: Vec<u8> = (0..window * 3 + 5).map(|n| (n * 37) as u8).collect();
                seed(&mut history, &prefix);
                let mut expected = prefix.clone();
                let length = window * 4 + 11;
                for _ in 0..length {
                    expected.push(expected[expected.len() - distance]);
                }
                let before = monitor.snapshot();
                let mut offset = 0;
                let mut call = 0;
                while offset < length {
                    let capacity = [0, 1, 2, 5, 31][call % 5];
                    call += 1;
                    let mut output = [0xa5; 33];
                    let count = history
                        .copy_match(distance, length - offset, &mut output[1..1 + capacity])
                        .unwrap();
                    assert_eq!(count, capacity.min(length - offset));
                    assert_eq!(
                        &output[1..1 + count],
                        &expected[prefix.len() + offset..prefix.len() + offset + count]
                    );
                    assert_eq!(output[0], 0xa5);
                    assert!(output[1 + count..].iter().all(|&byte| byte == 0xa5));
                    offset += count;
                }
                assert_eq!(history.frame_len(), expected.len() as u64);
                let retained_start = expected.len() - window;
                assert_eq!(
                    history.byte_at((retained_start - 1) as u64),
                    Err(Error::CorruptData)
                );
                for (position, &byte) in expected.iter().enumerate().skip(retained_start) {
                    assert_eq!(history.byte_at(position as u64).unwrap(), byte);
                }
                assert_eq!(monitor.snapshot().attempts, before.attempts);
                drop(history);
                monitor.assert_released();
            }
        }
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(owner);
        monitor.assert_released();
    }

    #[test]
    fn custom_distinct_owner_moves_and_swaps_keep_allocations_with_their_allocators() {
        let original = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = original.monitor();
        let first = original.clone();
        let second = original.clone();
        let first_id = first.identity();
        let second_id = second.identity();
        assert_ne!(first_id, second_id);
        let mut left = RingHistory::new_in(7, first);
        let mut right = RingHistory::new_in(11, second);
        left.start_frame(3).unwrap();
        right.start_frame(5).unwrap();
        left.commit(b"abc").unwrap();
        right.commit(b"123456").unwrap();
        let left_pointer = left.bytes.as_ptr();
        let right_pointer = right.bytes.as_ptr();
        let attempts = monitor.snapshot().attempts;
        drop(original);
        core::mem::swap(&mut left, &mut right);
        assert_eq!(left.bytes.allocator().identity(), second_id);
        assert_eq!(right.bytes.allocator().identity(), first_id);
        assert_eq!(left.bytes.as_ptr(), right_pointer);
        assert_eq!(right.bytes.as_ptr(), left_pointer);
        assert_eq!(left.maximum_window, 11);
        assert_eq!(right.maximum_window, 7);
        assert_eq!(left.byte_at(1).unwrap(), b'2');
        assert_eq!(right.byte_at(0).unwrap(), b'a');
        let mut output = [0; 6];
        right.copy_match(3, 6, &mut output).unwrap();
        assert_eq!(&output, b"abcabc");
        assert_eq!(monitor.snapshot().attempts, attempts);
        let moved = left;
        drop(right);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(moved);
        assert_eq!(monitor.snapshot().identities_created, 3);
        assert_eq!(monitor.snapshot().owners_dropped, 3);
        monitor.assert_released();
    }
}
