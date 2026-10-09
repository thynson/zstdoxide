//! Checked allocator-owned priority queue for bounded setup work.
//!
//! Values retain their ordinary `Ord` meaning. Huffman setup uses
//! `Reverse<(weight, minimum_symbol, node_index)>` to preserve its exact ties.
//! All links are indices into initialized Vec elements. This setup queue has
//! no unchecked backend; a separate measured optimization would need its own
//! feature and safety review.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::{Error, Result};

/// A maximum heap whose complete storage stays with its allocator owner.
///
/// Allocation failure leaves existing values and their ordering unchanged.
/// An incoming value is dropped if `try_push` cannot reserve its slot. Ordinary
/// value comparison or destruction panics retain their normal Rust semantics;
/// they are not reported as allocator errors.
pub(crate) struct MaxHeap<T: Ord, A: Allocator = Global> {
    values: Vec<T, A>,
}

impl<T: Ord, A: Allocator> MaxHeap<T, A> {
    pub(crate) fn new_in(allocator: A) -> Self {
        Self {
            values: Vec::new_in(allocator),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }

    pub(crate) fn try_reserve(&mut self, additional: usize) -> Result<()> {
        self.values
            .try_reserve(additional)
            .map_err(|_| Error::AllocationFailed)
    }

    pub(crate) fn try_push(&mut self, value: T) -> Result<()> {
        self.try_reserve(1)?;
        // The reserved spare slot makes this individual push allocation-free.
        self.values.push(value);
        let mut index = self.values.len() - 1;
        while index != 0 {
            let parent = (index - 1) / 2;
            if self.values[parent] >= self.values[index] {
                break;
            }
            self.values.swap(parent, index);
            index = parent;
        }
        Ok(())
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        let last = self.values.pop()?;
        if self.values.is_empty() {
            return Some(last);
        }
        let maximum = core::mem::replace(&mut self.values[0], last);
        let len = self.values.len();
        let mut parent = 0;
        while parent < len / 2 {
            // For odd and even len, this guard proves 2 * parent + 1 < len.
            // Thus neither child arithmetic nor its following access overflows.
            let mut child = parent * 2 + 1;
            let right = child + 1;
            if right < len && self.values[child] <= self.values[right] {
                child = right;
            }
            if self.values[parent] >= self.values[child] {
                break;
            }
            self.values.swap(parent, child);
            parent = child;
        }
        Some(maximum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BinaryHeap;
    use core::cell::Cell;
    use core::cmp::{Ordering, Reverse};
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 4,
            fail_at: None,
        })
        .unwrap()
    }

    #[test]
    fn every_small_heap_shape_matches_the_standard_priority_queue() {
        for len in 0..=257 {
            let mut actual = MaxHeap::new_in(Global);
            let mut expected = BinaryHeap::new();
            actual.try_reserve(len).unwrap();
            for index in 0..len {
                let value = Reverse(((index * 19) % 7, index % 5, index));
                actual.try_push(value).unwrap();
                expected.push(value);
            }
            while let Some(value) = expected.pop() {
                assert_eq!(actual.pop(), Some(value));
                assert_eq!(actual.len(), expected.len());
            }
            assert_eq!(actual.pop(), None);
        }
    }

    #[test]
    fn interleaved_tie_heavy_operations_match_standard_order() {
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        let mut actual = MaxHeap::new_in(&allocator);
        let mut expected = BinaryHeap::new();
        let mut random = 0x632b_e59b_u64;
        for operation in 0..4096 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            if random & 3 == 0 || expected.is_empty() {
                let value = Reverse((random % 11, random % 3, operation % 17));
                actual.try_push(value).unwrap();
                expected.push(value);
            } else {
                assert_eq!(actual.pop(), expected.pop());
            }
            assert_eq!(actual.len(), expected.len());
        }
        while !expected.is_empty() {
            assert_eq!(actual.pop(), expected.pop());
        }
        drop(actual);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().identities_created, 1);
    }

    #[test]
    fn initial_reservation_growth_failure_and_overflow_preserve_values() {
        let allocator = allocator();
        let monitor = allocator.monitor();
        let mut heap = MaxHeap::new_in(allocator);
        assert!(monitor.fail_next());
        assert_eq!(heap.try_reserve(4), Err(Error::AllocationFailed));
        assert_eq!(heap.len(), 0);
        heap.try_reserve(4).unwrap();
        let capacity = heap.values.capacity();
        for value in 0..capacity {
            heap.try_push(value).unwrap();
        }
        let pointer = heap.values.as_ptr();
        assert!(monitor.fail_next());
        assert_eq!(heap.try_push(capacity), Err(Error::AllocationFailed));
        assert_eq!(heap.values.as_ptr(), pointer);
        assert_eq!(heap.len(), capacity);
        assert_eq!(heap.try_reserve(usize::MAX), Err(Error::AllocationFailed));
        for expected in (0..capacity).rev() {
            assert_eq!(heap.pop(), Some(expected));
        }
        assert_eq!(heap.pop(), None);
        drop(heap);
        assert_eq!(monitor.snapshot().failures, 2);
        monitor.assert_released();
    }

    struct OwnedValue<'a> {
        rank: usize,
        drops: &'a Cell<usize>,
    }

    impl PartialEq for OwnedValue<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.rank == other.rank
        }
    }

    impl Eq for OwnedValue<'_> {}

    impl PartialOrd for OwnedValue<'_> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for OwnedValue<'_> {
        fn cmp(&self, other: &Self) -> Ordering {
            self.rank.cmp(&other.rank)
        }
    }

    impl Drop for OwnedValue<'_> {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn non_copy_values_are_dropped_once_on_failure_pop_and_owner_drop() {
        let allocator = allocator().into_non_clone();
        let monitor = allocator.monitor();
        let drops = Cell::new(0);
        let mut heap = MaxHeap::new_in(allocator);
        heap.try_reserve(4).unwrap();
        let capacity = heap.values.capacity();
        for rank in 0..capacity {
            heap.try_push(OwnedValue {
                rank,
                drops: &drops,
            })
            .unwrap();
        }
        assert!(monitor.fail_next());
        assert_eq!(
            heap.try_push(OwnedValue {
                rank: usize::MAX,
                drops: &drops
            }),
            Err(Error::AllocationFailed)
        );
        assert_eq!(drops.get(), 1);
        let largest = heap.pop().unwrap();
        assert_eq!(largest.rank, capacity - 1);
        assert_eq!(drops.get(), 1);
        drop(largest);
        assert_eq!(drops.get(), 2);
        drop(heap);
        assert_eq!(drops.get(), capacity + 1);
        monitor.assert_released();
    }

    #[test]
    fn complete_heap_moves_and_swaps_preserve_distinct_allocator_domains() {
        let first = allocator();
        let monitor = first.monitor();
        let second = first.clone();
        let first_id = first.identity();
        let second_id = second.identity();
        assert_ne!(first_id, second_id);
        let mut left = MaxHeap::new_in(first);
        let mut right = MaxHeap::new_in(second);
        for value in [1, 9, 3] {
            left.try_push(value).unwrap();
            right.try_push(value + 100).unwrap();
        }
        core::mem::swap(&mut left, &mut right);
        assert_eq!(left.values.allocator().identity(), second_id);
        assert_eq!(right.values.allocator().identity(), first_id);
        assert_eq!(left.pop(), Some(109));
        assert_eq!(right.pop(), Some(9));
        let moved = left;
        drop(right);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(moved);
        monitor.assert_released();
    }
}
