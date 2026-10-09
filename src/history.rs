//! Bounded output storage with checked, overlap-aware LZ match expansion.

use crate::{Error, Result};
use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

/// The checked operations needed by sequence execution, independent of storage.
///
/// A sink reports its current block/output position and validates capacity and
/// backward distances before writes. Streaming sinks retain bounded ring
/// history; one-shot history owns the complete result.
pub(crate) trait SequenceOutput {
    fn len(&self) -> usize;
    fn append(&mut self, bytes: &[u8]) -> Result<()>;
    fn copy_match(&mut self, offset: usize, length: usize) -> Result<()>;
}

pub(crate) struct History<A: Allocator = Global> {
    output: Vec<u8, A>,
    limit: usize,
    frame_start: usize,
    window: usize,
}

impl History<Global> {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Global convenience for direct tests and benchmarks"
        )
    )]
    pub(crate) fn new(limit: usize) -> Self {
        Self::new_in(limit, Global)
    }
}

impl<A: Allocator> History<A> {
    /// Retain this allocator with the complete output; construction neither
    /// allocates payload storage nor clones the allocator.
    pub(crate) fn new_in(limit: usize, allocator: A) -> Self {
        Self {
            output: Vec::new_in(allocator),
            limit,
            frame_start: 0,
            window: 0,
        }
    }

    pub(crate) fn start_frame(&mut self, window: usize) {
        self.frame_start = self.output.len();
        self.window = window;
    }

    pub(crate) fn len(&self) -> usize {
        self.output.len()
    }

    pub(crate) fn frame_bytes(&self) -> &[u8] {
        &self.output[self.frame_start..]
    }

    pub(crate) fn into_bytes(self) -> Vec<u8, A> {
        self.output
    }

    fn reserve(&mut self, count: usize) -> Result<()> {
        if count > self.limit - self.output.len() {
            return Err(Error::ResourceLimit);
        }
        self.output
            .try_reserve(count)
            .map_err(|_| Error::AllocationFailed)
    }

    pub(crate) fn append(&mut self, bytes: &[u8]) -> Result<()> {
        self.reserve(bytes.len())?;
        // Individual pushes cannot reallocate within the capacity reserved
        // above; bulk insertion APIs do not make that same general guarantee.
        for &byte in bytes {
            self.output.push(byte);
        }
        Ok(())
    }

    pub(crate) fn repeat(&mut self, byte: u8, count: usize) -> Result<()> {
        self.reserve(count)?;
        for _ in 0..count {
            self.output.push(byte);
        }
        Ok(())
    }

    /// Execute the match-copy part of upstream `ZSTD_execSequence`.
    pub(crate) fn copy_match(&mut self, offset: usize, length: usize) -> Result<()> {
        if offset == 0 || offset > self.frame_bytes().len() || offset > self.window {
            return Err(Error::CorruptData);
        }
        self.reserve(length)?;
        let start = self.output.len();
        #[cfg(not(feature = "unsafe-history"))]
        for index in 0..length {
            let byte = self.output[start + index - offset];
            self.output.push(byte);
        }
        #[cfg(feature = "unsafe-history")]
        {
            let pointer = self.output.as_mut_ptr();
            // SAFETY: offset is nonzero and <= the initialized frame prefix.
            // reserve proved start + length <= limit without overflow and
            // allocated capacity for that end through the Vec's retained A.
            // That allocator and its live allocation remain owned throughout;
            // no reallocation or allocator replacement occurs below.
            // On iteration i, start+i-offset is nonnegative and < start+i:
            // it is in the original initialized prefix or was written by an
            // earlier iteration. Source and destination are within this same
            // live u8 allocation (alignment one). read copies a u8, write fills
            // one slot; their temporary pointers create no aliased references.
            // The exclusive self borrow prevents other access. Sequential reads
            // preserve LZ overlap semantics. After the loop every byte up to the
            // new length is initialized, proving set_len's requirements.
            unsafe {
                for index in 0..length {
                    let byte = pointer.add(start + index - offset).read();
                    pointer.add(start + index).write(byte);
                }
                self.output.set_len(start + length);
            }
        }
        Ok(())
    }
}

impl<A: Allocator> SequenceOutput for History<A> {
    fn len(&self) -> usize {
        History::len(self)
    }

    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        History::append(self, bytes)
    }

    fn copy_match(&mut self, offset: usize, length: usize) -> Result<()> {
        History::copy_match(self, offset, length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_overlapping_patterns_at_every_small_offset() {
        for offset in 1..=16 {
            let mut history = History::new(1000);
            history.start_frame(128);
            let prefix: Vec<_> = (0..offset as u8).collect();
            history.append(&prefix).unwrap();
            history.copy_match(offset, 301).unwrap();
            let expected: Vec<_> = prefix.iter().copied().cycle().take(offset + 301).collect();
            assert_eq!(history.into_bytes(), expected);
        }
    }

    #[test]
    fn rejects_out_of_window_cross_frame_and_oversized_matches() {
        let mut history = History::new(10);
        history.start_frame(2);
        history.append(b"abc").unwrap();
        assert_eq!(history.copy_match(0, 1), Err(Error::CorruptData));
        assert_eq!(history.copy_match(3, 1), Err(Error::CorruptData));
        assert_eq!(history.copy_match(1, usize::MAX), Err(Error::ResourceLimit));
        assert_eq!(history.frame_bytes(), b"abc");
        history.start_frame(10);
        assert_eq!(history.copy_match(1, 1), Err(Error::CorruptData));
        history.repeat(b'd', 2).unwrap();
        assert_eq!(history.len(), 5);
        assert_eq!(history.frame_bytes(), b"dd");
    }

    #[test]
    fn custom_constructor_empty_operations_and_non_clone_return_preserve_owner() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let identity = allocator.identity();
        let monitor = allocator.monitor();
        let mut history = History::new_in(128, allocator);
        history.start_frame(128);
        history.append(&[]).unwrap();
        history.repeat(0, 0).unwrap();
        assert_eq!(history.copy_match(1, 0), Err(Error::CorruptData));
        assert_eq!(monitor.snapshot().attempts, 0);
        history.append(b"owned output").unwrap();
        let pointer = history.output.as_ptr();
        let before = monitor.snapshot();
        let output = history.into_bytes();
        assert_eq!(output.allocator().identity(), identity);
        assert_eq!(output.as_ptr(), pointer);
        assert_eq!(output.as_slice(), b"owned output");
        assert_eq!(monitor.snapshot(), before);
        drop(output);
        monitor.assert_released();
    }

    #[test]
    fn custom_failed_growth_preserves_every_field_and_recovers() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        for operation in 0..3 {
            let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = allocator.monitor();
            let mut history = History::new_in(usize::MAX, &allocator);
            history.append(b"prefix").unwrap();
            history.start_frame(128);
            history.append(b"abc").unwrap();
            // Fill the observed capacity; do not assume an exact allocator size.
            history
                .repeat(b'c', history.output.capacity() - history.len())
                .unwrap();
            let bytes = history.output.to_vec();
            let pointer = history.output.as_ptr();
            let capacity = history.output.capacity();
            let frame_start = history.frame_start;
            let window = history.window;
            let before = monitor.snapshot();
            assert!(monitor.fail_next());
            let attempt = |history: &mut History<&TrackingAllocator>| match operation {
                0 => history.append(b"x"),
                1 => history.repeat(b'y', 1),
                _ => history.copy_match(1, 1),
            };
            assert_eq!(attempt(&mut history), Err(Error::AllocationFailed));
            assert_eq!(history.output.as_slice(), bytes);
            assert_eq!(history.output.as_ptr(), pointer);
            assert_eq!(history.output.capacity(), capacity);
            assert_eq!(history.frame_start, frame_start);
            assert_eq!(history.window, window);
            let after = monitor.snapshot();
            assert_eq!(after.attempts, before.attempts + 1);
            assert_eq!(after.failures, before.failures + 1);
            assert_eq!(after.live_bytes, before.live_bytes);
            monitor.clear_failure();
            attempt(&mut history).unwrap();
            assert_eq!(&history.output[..bytes.len()], bytes);
            assert_eq!(history.len(), bytes.len() + 1);
            assert_eq!(history.output[bytes.len()], b"xyc"[operation]);
            drop(history);
            drop(allocator);
            monitor.assert_released();
        }
    }

    #[test]
    fn custom_validation_and_spare_capacity_preserve_armed_failure() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = allocator.monitor();
        let mut history = History::new_in(4, &allocator);
        history.start_frame(2);
        history.append(b"a").unwrap();
        history.reserve(3).unwrap();
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        history.append(b"b").unwrap();
        history.repeat(b'c', 1).unwrap();
        history.copy_match(1, 1).unwrap();
        assert_eq!(history.copy_match(0, 1), Err(Error::CorruptData));
        assert_eq!(history.copy_match(3, 1), Err(Error::CorruptData));
        assert_eq!(history.copy_match(1, usize::MAX), Err(Error::ResourceLimit));
        assert_eq!(history.append(b"x"), Err(Error::ResourceLimit));
        assert_eq!(history.repeat(b'x', 1), Err(Error::ResourceLimit));
        history.copy_match(1, 0).unwrap();
        assert_eq!(history.frame_bytes(), b"abcc");
        assert_eq!(monitor.snapshot(), before);
        // A second owner using this exact borrowed allocator consumes the
        // still-armed ordinal, proving prior operations made no hidden request.
        let mut second = History::new_in(4, &allocator);
        assert_eq!(second.append(b"x"), Err(Error::AllocationFailed));
        assert_eq!(second.len(), 0);
        drop(second);
        drop(history);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn custom_overlap_matches_global_at_every_small_distance() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = allocator.monitor();
        for offset in 1..=16 {
            let mut custom = History::new_in(1000, &allocator);
            let mut global = History::new(1000);
            custom.start_frame(128);
            global.start_frame(128);
            let prefix: Vec<_> = (0..offset as u8).collect();
            custom.append(&prefix).unwrap();
            global.append(&prefix).unwrap();
            custom.copy_match(offset, 301).unwrap();
            global.copy_match(offset, 301).unwrap();
            assert_eq!(custom.into_bytes().as_slice(), global.into_bytes());
        }
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn custom_distinct_domains_move_swap_and_return_whole_owners() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = allocator.monitor();
        let first_allocator = allocator.clone();
        let second_allocator = allocator.clone();
        let first_id = first_allocator.identity();
        let second_id = second_allocator.identity();
        assert_ne!(first_id, second_id);
        let mut first = History::new_in(128, first_allocator);
        let mut second = History::new_in(256, second_allocator);
        first.start_frame(64);
        second.start_frame(128);
        first.append(b"abc").unwrap();
        first.copy_match(3, 6).unwrap();
        second.append(b"xyz").unwrap();
        drop(allocator);
        let before = monitor.snapshot();
        core::mem::swap(&mut first, &mut second);
        let first_pointer = first.output.as_ptr();
        let second_pointer = second.output.as_ptr();
        let first = first.into_bytes();
        let second = second.into_bytes();
        assert_eq!(first.allocator().identity(), second_id);
        assert_eq!(second.allocator().identity(), first_id);
        assert_eq!(first.as_ptr(), first_pointer);
        assert_eq!(second.as_ptr(), second_pointer);
        assert_eq!(first.as_slice(), b"xyz");
        assert_eq!(second.as_slice(), b"abcabcabc");
        assert_eq!(monitor.snapshot(), before);
        drop(first);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(second);
        monitor.assert_released();
    }
}
