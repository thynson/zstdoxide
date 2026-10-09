//! Position-indexed storage for a simple greedy match finder.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::{Error, Result};

const HASH_BITS: u32 = 16;
const TABLE_SIZE: usize = 1 << HASH_BITS;
const EMPTY: usize = usize::MAX;

/// An owned table of candidate positions; input bytes are never owned or pointed
/// to by this table. Callers verify candidate bytes to resolve hash collisions.
///
/// Corresponds to the indexed hash-table role of `ZSTD_MatchState_t`.
pub(crate) struct MatchTable<A: Allocator = Global> {
    positions: Vec<usize, A>,
}

impl MatchTable<Global> {
    pub(crate) fn new() -> Result<Self> {
        Self::new_in(Global)
    }
}

impl<A: Allocator> MatchTable<A> {
    /// Allocate the initialized table through this exact retained owner.
    /// A single allocation needs no allocator clone or clone equivalence.
    pub(crate) fn new_in(allocator: A) -> Result<Self> {
        let mut positions = Vec::new_in(allocator);
        positions
            .try_reserve_exact(TABLE_SIZE)
            .map_err(|_| Error::AllocationFailed)?;
        // All pushes fit the successful reservation. Unlike bulk insertion,
        // individual pushes into spare capacity promise not to allocate.
        while positions.len() < TABLE_SIZE {
            positions.push(EMPTY);
        }
        Ok(Self { positions })
    }

    /// Starts an independent block without retaining candidate positions.
    pub(crate) fn reset(&mut self) {
        self.positions.fill(EMPTY);
    }

    /// Copy a prepared dictionary's candidate indices into working storage.
    ///
    /// Both vectors have the same fixed initialized length. The owned working
    /// allocation is retained and no reference to the seed table is stored.
    pub(crate) fn reset_from<B: Allocator>(&mut self, seed: &MatchTable<B>) {
        self.positions.copy_from_slice(&seed.positions);
    }

    /// Owned allocation size in bytes, excluding the inline vector handle.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.positions.capacity() * core::mem::size_of::<usize>()
    }

    /// Records the current position and returns an earlier candidate, if any.
    ///
    /// A returned candidate only denotes a hash collision; the caller must
    /// verify source bytes and its own history/window bounds before matching.
    /// Corresponds to table lookup/update in `ZSTD_compressBlock_fast`.
    pub(crate) fn replace(&mut self, word: [u8; 4], position: usize) -> Option<usize> {
        let hash = u32::from_le_bytes(word).wrapping_mul(0x9e37_79b1);
        let index = (hash >> (32 - HASH_BITS)) as usize;
        #[cfg(not(feature = "unsafe-match-table"))]
        let previous = core::mem::replace(&mut self.positions[index], position);
        #[cfg(feature = "unsafe-match-table")]
        let previous = {
            // SAFETY: HASH_BITS is 16, so shifting the u32 hash right by 16 proves
            // index < 2^16 = TABLE_SIZE. Construction initializes TABLE_SIZE
            // properly aligned usize entries; no method changes the vector
            // length or allocation, which retains its exact allocator owner.
            // The exclusive self borrow grants unique
            // access to the element for this call. The temporary reference
            // cannot escape; neither operation reallocates or reads
            // uninitialized storage.
            unsafe { core::mem::replace(self.positions.get_unchecked_mut(index), position) }
        };
        (previous != EMPTY && previous < position).then_some(previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_only_prior_candidates_and_reset_discards_them() {
        let mut table = MatchTable::new().unwrap();
        assert_eq!(table.replace(*b"abcd", 0), None);
        assert_eq!(table.replace(*b"abcd", 9), Some(0));
        assert_eq!(table.replace(*b"abcd", 9), None);
        assert_eq!(table.replace(*b"abcd", 8), None);
        assert_eq!(table.replace(*b"abcd", 10), Some(8));
        table.reset();
        assert_eq!(table.replace(*b"abcd", 11), None);
    }

    #[test]
    fn collisions_remain_candidates_that_need_byte_verification() {
        let mut table = MatchTable::new().unwrap();
        // Multiplication maps these two distinct words to hash values 0 and 1,
        // whose high sixteen bits are equal.
        assert_eq!(table.replace(0_u32.to_le_bytes(), 1), None);
        assert_eq!(table.replace(0x0e8b_2f51_u32.to_le_bytes(), 5), Some(1));
    }

    #[test]
    fn empty_sentinel_cannot_escape_as_a_candidate() {
        let mut table = MatchTable::new().unwrap();
        assert_eq!(table.replace([255; 4], usize::MAX), None);
        assert_eq!(table.replace([255; 4], usize::MAX), None);
    }

    #[test]
    fn copying_seed_indices_retains_independent_working_storage() {
        let mut seed = MatchTable::new().unwrap();
        let mut working = MatchTable::new().unwrap();
        seed.replace(*b"seed", 7);
        working.replace(*b"seed", 19);
        let capacity = working.retained_capacity();
        working.reset_from(&seed);
        assert_eq!(working.replace(*b"seed", 23), Some(7));
        assert_eq!(seed.replace(*b"seed", 31), Some(7));
        working.reset_from(&seed);
        assert_eq!(working.replace(*b"seed", 43), Some(31));
        assert_eq!(working.retained_capacity(), capacity);
        seed.reset();
        working.reset_from(&seed);
        assert_eq!(working.replace(*b"seed", 47), None);
    }
    #[test]
    fn custom_constructor_failure_drops_non_clone_owner_without_live_blocks() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = allocator.monitor();
        assert!(monitor.fail_next());
        assert!(matches!(
            MatchTable::new_in(allocator),
            Err(Error::AllocationFailed)
        ));
        assert_eq!(monitor.snapshot().attempts, 1);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
        monitor.assert_released();
    }

    #[test]
    fn custom_non_clone_table_matches_global_without_requests_after_setup() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let identity = allocator.identity();
        let monitor = allocator.monitor();
        let mut custom = MatchTable::new_in(allocator).unwrap();
        let mut global = MatchTable::new().unwrap();
        assert_eq!(custom.positions.allocator().identity(), identity);
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        for round in 0..3 {
            for index in 0..1024_usize {
                let word = ((index % 31) as u32)
                    .wrapping_mul(0x0e8b_2f51)
                    .to_le_bytes();
                let position = match index % 17 {
                    0 => usize::MAX,
                    1 => 0,
                    _ => index + round * 1024,
                };
                assert_eq!(
                    custom.replace(word, position),
                    global.replace(word, position)
                );
            }
            custom.reset();
            global.reset();
        }
        assert_eq!(custom.positions.as_slice(), global.positions);
        assert_eq!(monitor.snapshot(), before);
        drop(custom);
        monitor.assert_released();
    }

    #[test]
    fn custom_seed_copies_accept_independent_allocator_types_without_allocation() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = allocator.monitor();
        let mut seed = MatchTable::new().unwrap();
        let mut custom_seed = MatchTable::new_in(&allocator).unwrap();
        let mut working = MatchTable::new_in(allocator.clone().into_non_clone()).unwrap();
        seed.replace(*b"seed", 7);
        custom_seed.replace(*b"seed", 11);
        let pointer = working.positions.as_ptr();
        let identity = working.positions.allocator().identity();
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        working.reset_from(&seed);
        assert_eq!(working.replace(*b"seed", 19), Some(7));
        assert_eq!(seed.replace(*b"seed", 23), Some(7));
        working.reset_from(&custom_seed);
        assert_eq!(working.replace(*b"seed", 29), Some(11));
        assert_eq!(custom_seed.replace(*b"seed", 31), Some(11));
        assert_eq!(working.positions.as_ptr(), pointer);
        assert_eq!(working.positions.allocator().identity(), identity);
        assert_eq!(monitor.snapshot(), before);
        // Seed copying retains destination ownership and never adopts a source
        // allocation. A subsequent setup consumes the still-armed request.
        assert!(matches!(
            MatchTable::new_in(&allocator),
            Err(Error::AllocationFailed)
        ));
        drop(working);
        drop(custom_seed);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn custom_distinct_owners_survive_moves_and_swaps() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = allocator.monitor();
        let mut first = MatchTable::new_in(allocator.clone()).unwrap();
        let mut second = MatchTable::new_in(allocator.clone()).unwrap();
        let first_id = first.positions.allocator().identity();
        let second_id = second.positions.allocator().identity();
        assert_ne!(first_id, second_id);
        first.replace(*b"move", 7);
        second.replace(*b"move", 11);
        let before = monitor.snapshot().attempts;
        drop(allocator);
        core::mem::swap(&mut first, &mut second);
        assert_eq!(first.positions.allocator().identity(), second_id);
        assert_eq!(second.positions.allocator().identity(), first_id);
        assert_eq!(first.replace(*b"move", 19), Some(11));
        assert_eq!(second.replace(*b"move", 23), Some(7));
        assert_eq!(monitor.snapshot().attempts, before);
        let moved = first;
        drop(second);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(moved);
        monitor.assert_released();
    }
}
