//! Position-indexed storage for a simple greedy match finder.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::{Allocator, Layout};

use crate::match_key::MatchKey;
use crate::match_search::SearchKeyLayout;
use crate::{Error, Result};

const EMPTY: usize = usize::MAX;

/// An owned table of candidate positions; input bytes are never owned or pointed
/// to by this table. Callers verify candidate bytes to resolve hash collisions.
///
/// The initialized length equals the checked layout's bucket count. Each vector
/// retains its exact allocator through moves and reshaping; spare capacity is
/// never an initialized candidate. Corresponds to the indexed hash-table role
/// of `ZSTD_MatchState_t`.
pub(crate) struct MatchTable<A: Allocator = Global> {
    positions: Vec<usize, A>,
    layout: SearchKeyLayout,
}

impl MatchTable<Global> {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "convenience entry point retained for direct callers"
        )
    )]
    pub(crate) fn new() -> Result<Self> {
        Self::new_in(Global)
    }
}

impl<A: Allocator> MatchTable<A> {
    /// Allocate the existing 16-bit, four-byte default through this exact owner.
    pub(crate) fn new_in(allocator: A) -> Result<Self> {
        Self::with_layout_in(SearchKeyLayout::new(16, 4)?, allocator)
    }

    /// Allocate initialized positions for a checked key layout.
    /// A single vector owner requires no allocator clone or clone equivalence.
    pub(crate) fn with_layout_in(layout: SearchKeyLayout, allocator: A) -> Result<Self> {
        Layout::array::<usize>(layout.bucket_count()).map_err(|_| Error::ResourceLimit)?;
        let mut positions = Vec::new_in(allocator);
        positions
            .try_reserve_exact(layout.bucket_count())
            .map_err(|_| Error::AllocationFailed)?;
        // Individual pushes fit the successful reservation and cannot allocate.
        while positions.len() < layout.bucket_count() {
            positions.push(EMPTY);
        }
        Ok(Self { positions, layout })
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "dynamic context integration follows this leaf checkpoint"
        )
    )]
    pub(crate) const fn layout(&self) -> SearchKeyLayout {
        self.layout
    }

    /// Start empty storage with a new active layout, retaining its exact owner.
    ///
    /// Validate and reserve before changing initialized entries or metadata.
    /// A failed reserve preserves the prior logical table. Shrinking retains
    /// capacity, and a later regrow within that capacity makes no request.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "dynamic context integration follows this leaf checkpoint"
        )
    )]
    pub(crate) fn reconfigure(&mut self, layout: SearchKeyLayout) -> Result<()> {
        let length = layout.bucket_count();
        Layout::array::<usize>(length).map_err(|_| Error::ResourceLimit)?;
        if length > self.positions.capacity() {
            // Vec's additional count is relative to initialized length, not
            // capacity. Here length > capacity >= len proves subtraction safe.
            self.positions
                .try_reserve_exact(length - self.positions.len())
                .map_err(|_| Error::AllocationFailed)?;
        }
        self.positions.truncate(length);
        self.positions.fill(EMPTY);
        // All pushes fit the existing or newly reserved allocation.
        while self.positions.len() < length {
            self.positions.push(EMPTY);
        }
        self.layout = layout;
        Ok(())
    }

    /// Starts an independent block without retaining candidate positions.
    pub(crate) fn reset(&mut self) {
        self.positions.fill(EMPTY);
    }

    /// Copy a compatible dictionary seed without adopting its allocation owner.
    ///
    /// Check width, hash log and both active lengths before mutation. Equal
    /// capacities do not establish seed compatibility or initialized storage.
    pub(crate) fn reset_from<B: Allocator>(&mut self, seed: &MatchTable<B>) -> Result<()> {
        if self.layout != seed.layout
            || self.positions.len() != self.layout.bucket_count()
            || seed.positions.len() != seed.layout.bucket_count()
        {
            return Err(Error::InvalidParameter);
        }
        self.positions.copy_from_slice(&seed.positions);
        Ok(())
    }

    /// Owned allocation size in bytes, excluding the inline vector handle.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.positions.capacity() * core::mem::size_of::<usize>()
    }

    /// Inspect initialized Rust positions for frozen oracle comparisons only.
    /// `usize::MAX` is invalid; other cells are absolute borrowed-source indices.
    /// C's valid-index bias of two is applied only by the comparison code.
    #[cfg(test)]
    pub(crate) fn test_positions(&self) -> &[usize] {
        &self.positions
    }

    /// Read a sampled candidate without updating the table or borrowing it.
    ///
    /// A nonempty position is a hash collision, possibly at or beyond the
    /// current cursor. Callers check source/window bounds and candidate bytes.
    /// Separate reads and writes preserve Fast's delayed-store collision order.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "Fast parser integration follows")
    )]
    pub(crate) fn lookup(&self, key: impl Into<MatchKey>) -> Result<Option<usize>> {
        let index = self.layout.bucket(key.into())?;
        if index >= self.positions.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-match-table"))]
        let position = self.positions[index];
        #[cfg(feature = "unsafe-match-table")]
        let position = {
            // SAFETY: The runtime check proves index is within the initialized
            // Vec<usize> length, not just its capacity. The retained allocator
            // owns valid aligned storage of initialized usize values. The shared
            // self borrow excludes mutation/reallocation for this read, and the
            // temporary reference cannot escape: only its Copy value is returned.
            unsafe { *self.positions.get_unchecked(index) }
        };
        Ok((position != EMPTY).then_some(position))
    }

    /// Store a sampled position without observing the previous collision.
    ///
    /// The reserved usize::MAX sentinel clears this bucket. Other positions
    /// need not index a source until a caller validates a later lookup. Width
    /// or initialized-bound errors preserve the table, and no allocation occurs.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "Fast parser integration follows")
    )]
    pub(crate) fn store(&mut self, key: impl Into<MatchKey>, position: usize) -> Result<()> {
        let index = self.layout.bucket(key.into())?;
        if index >= self.positions.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-match-table"))]
        {
            self.positions[index] = position;
        }
        #[cfg(feature = "unsafe-match-table")]
        {
            // SAFETY: The runtime check proves index is below initialized length;
            // the exact allocator owns aligned, valid usize storage. Exclusive
            // self excludes aliases, mutation and reallocation until this access
            // ends. The temporary reference does not escape. Assigning a Copy
            // usize preserves initialization and cannot allocate or run a drop.
            unsafe { *self.positions.get_unchecked_mut(index) = position };
        }
        Ok(())
    }

    /// Record the current position and return an earlier candidate, if any.
    ///
    /// Incompatible key widths and invalid live indices fail before mutation.
    /// A candidate only denotes a hash collision: callers must verify bytes and
    /// history/window bounds. Corresponds to the lookup/update role in
    /// `ZSTD_compressBlock_fast`.
    pub(crate) fn replace(
        &mut self,
        key: impl Into<MatchKey>,
        position: usize,
    ) -> Result<Option<usize>> {
        let index = self.layout.bucket(key.into())?;
        if index >= self.positions.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-match-table"))]
        let previous = core::mem::replace(&mut self.positions[index], position);
        #[cfg(feature = "unsafe-match-table")]
        let previous = {
            // SAFETY: The runtime check proves index < the live initialized
            // Vec<usize> length, independently of the layout or spare capacity.
            // Its exact retained allocator owns valid, properly aligned storage
            // containing initialized usize values. The exclusive self borrow
            // excludes aliases and concurrent reset, reshape or reallocation.
            // get_unchecked_mut's temporary reference stays within this block;
            // replace reads one initialized value and writes another Copy value,
            // without allocation, ownership transfer or an escaping reference.
            unsafe { core::mem::replace(self.positions.get_unchecked_mut(index), position) }
        };
        Ok((previous != EMPTY && previous < position).then_some(previous))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampled_reads_preserve_delayed_store_collision_order() {
        for log in 6..=9 {
            let layout = SearchKeyLayout::for_fast(log, 4).unwrap();
            let mut table = MatchTable::with_layout_in(layout, Global).unwrap();
            let first = 0_u32.to_le_bytes();
            let colliding = 0x0e8b_2f51_u32.to_le_bytes();
            assert_eq!(layout.bucket(first.into()), layout.bucket(colliding.into()));
            assert_eq!(table.lookup(first), Ok(None));
            table.store(first, 0).unwrap();
            let saved = table.lookup(colliding).unwrap();
            assert_eq!(saved, Some(0));
            assert_eq!(table.lookup(first), Ok(Some(0)));
            table.store(colliding, 19).unwrap();
            assert_eq!(saved, Some(0));
            assert_eq!(table.lookup(first), Ok(Some(19)));
            // Lookup has no cursor and must not silently apply replace's prior
            // position filter; the parser performs that contextual check.
            table.store(first, usize::MAX - 1).unwrap();
            assert_eq!(table.lookup(first), Ok(Some(usize::MAX - 1)));
            table.store(first, EMPTY).unwrap();
            assert_eq!(table.lookup(first), Ok(None));
        }
    }

    #[test]
    fn sampled_access_rejects_width_and_live_length_before_mutation() {
        let layout = SearchKeyLayout::for_fast(6, 4).unwrap();
        let mut table = MatchTable::with_layout_in(layout, Global).unwrap();
        table.store([0; 4], 13).unwrap();
        let wrong = SearchKeyLayout::for_fast(6, 5)
            .unwrap()
            .read_key(b"12345")
            .unwrap();
        let before = table.positions.to_vec();
        assert_eq!(table.lookup(wrong), Err(Error::InvalidParameter));
        assert_eq!(table.store(wrong, 17), Err(Error::InvalidParameter));
        assert_eq!(table.positions.as_slice(), before);
        let last = u32::MAX.wrapping_mul(0x0e8b_2f51).to_le_bytes();
        assert_eq!(layout.bucket(last.into()), Ok(63));
        table.positions.truncate(63);
        let before = table.positions.to_vec();
        assert!(table.positions.capacity() >= 64);
        assert_eq!(table.lookup(last), Err(Error::CorruptData));
        assert_eq!(table.store(last, 17), Err(Error::CorruptData));
        assert_eq!(table.positions.as_slice(), before);
        table.positions.clear();
        assert_eq!(table.lookup([0; 4]), Err(Error::CorruptData));
        assert_eq!(table.store([0; 4], 1), Err(Error::CorruptData));
        assert!(table.positions.is_empty());
    }

    #[test]
    fn sampled_small_layouts_reuse_exact_owner_without_allocation() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let mut table =
            MatchTable::with_layout_in(SearchKeyLayout::for_fast(9, 7).unwrap(), &owner).unwrap();
        let pointer = table.positions.as_ptr();
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        for log in 6..=9 {
            for width in 4..=7 {
                let layout = SearchKeyLayout::for_fast(log, width).unwrap();
                table.reconfigure(layout).unwrap();
                let key = layout.read_key(b"1234567").unwrap();
                assert_eq!(table.lookup(key), Ok(None));
                table.store(key, 7).unwrap();
                assert_eq!(table.lookup(key), Ok(Some(7)));
                assert_eq!(table.positions.as_ptr(), pointer);
                assert_eq!(monitor.snapshot(), before);
            }
        }
        let old_layout = table.layout();
        let before_entries = table.positions.to_vec();
        assert_eq!(
            table.reconfigure(SearchKeyLayout::new(18, 4).unwrap()),
            Err(Error::AllocationFailed)
        );
        assert_eq!(table.positions.as_slice(), before_entries);
        assert_eq!(table.layout(), old_layout);
        assert_eq!(monitor.snapshot().failures, 1);
        drop(table);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(owner);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }

    #[test]
    fn returns_only_prior_candidates_and_reset_discards_them() {
        let mut table = MatchTable::new().unwrap();
        assert_eq!(table.replace(*b"abcd", 0).unwrap(), None);
        assert_eq!(table.replace(*b"abcd", 9).unwrap(), Some(0));
        assert_eq!(table.replace(*b"abcd", 9).unwrap(), None);
        assert_eq!(table.replace(*b"abcd", 8).unwrap(), None);
        assert_eq!(table.replace(*b"abcd", 10).unwrap(), Some(8));
        table.reset();
        assert_eq!(table.replace(*b"abcd", 11).unwrap(), None);
    }

    #[test]
    fn collisions_remain_candidates_that_need_byte_verification() {
        let mut table = MatchTable::new().unwrap();
        // Multiplication maps these two distinct words to hash values 0 and 1,
        // whose high sixteen bits are equal.
        assert_eq!(table.replace(0_u32.to_le_bytes(), 1).unwrap(), None);
        assert_eq!(
            table.replace(0x0e8b_2f51_u32.to_le_bytes(), 5).unwrap(),
            Some(1)
        );
    }

    #[test]
    fn empty_sentinel_cannot_escape_as_a_candidate() {
        let mut table = MatchTable::new().unwrap();
        assert_eq!(table.replace([255; 4], usize::MAX).unwrap(), None);
        assert_eq!(table.replace([255; 4], usize::MAX).unwrap(), None);
    }

    #[test]
    fn copying_seed_indices_retains_independent_working_storage() {
        let mut seed = MatchTable::new().unwrap();
        let mut working = MatchTable::new().unwrap();
        seed.replace(*b"seed", 7).unwrap();
        working.replace(*b"seed", 19).unwrap();
        let capacity = working.retained_capacity();
        working.reset_from(&seed).unwrap();
        assert_eq!(working.replace(*b"seed", 23).unwrap(), Some(7));
        assert_eq!(seed.replace(*b"seed", 31).unwrap(), Some(7));
        working.reset_from(&seed).unwrap();
        assert_eq!(working.replace(*b"seed", 43).unwrap(), Some(31));
        assert_eq!(working.retained_capacity(), capacity);
        seed.reset();
        working.reset_from(&seed).unwrap();
        assert_eq!(working.replace(*b"seed", 47).unwrap(), None);
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
        seed.replace(*b"seed", 7).unwrap();
        custom_seed.replace(*b"seed", 11).unwrap();
        let pointer = working.positions.as_ptr();
        let identity = working.positions.allocator().identity();
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        working.reset_from(&seed).unwrap();
        assert_eq!(working.replace(*b"seed", 19).unwrap(), Some(7));
        assert_eq!(seed.replace(*b"seed", 23).unwrap(), Some(7));
        working.reset_from(&custom_seed).unwrap();
        assert_eq!(working.replace(*b"seed", 29).unwrap(), Some(11));
        assert_eq!(custom_seed.replace(*b"seed", 31).unwrap(), Some(11));
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
        first.replace(*b"move", 7).unwrap();
        second.replace(*b"move", 11).unwrap();
        let before = monitor.snapshot().attempts;
        drop(allocator);
        core::mem::swap(&mut first, &mut second);
        assert_eq!(first.positions.allocator().identity(), second_id);
        assert_eq!(second.positions.allocator().identity(), first_id);
        assert_eq!(first.replace(*b"move", 19).unwrap(), Some(11));
        assert_eq!(second.replace(*b"move", 23).unwrap(), Some(7));
        assert_eq!(monitor.snapshot().attempts, before);
        let moved = first;
        drop(second);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(moved);
        monitor.assert_released();
    }
    #[test]
    fn dynamic_layouts_initialize_and_reconfigure_every_supported_dimension() {
        let mut table =
            MatchTable::with_layout_in(SearchKeyLayout::new(10, 4).unwrap(), Global).unwrap();
        for log in 10..=18 {
            for width in 4..=8 {
                let layout = SearchKeyLayout::new(log, width).unwrap();
                table.reconfigure(layout).unwrap();
                assert_eq!(table.layout(), layout);
                assert_eq!(table.positions.len(), layout.bucket_count());
                assert!(table.positions.iter().all(|&entry| entry == EMPTY));
                let key = layout.read_key(&[0; 8]).unwrap();
                assert_eq!(table.replace(key, usize::MAX - 1), Ok(None));
                assert_eq!(table.replace(key, usize::MAX), Ok(Some(usize::MAX - 1)));
                assert_eq!(table.replace(key, usize::MAX), Ok(None));
                table.replace(key, 7).unwrap();
            }
        }
    }

    #[test]
    fn double_fast_long_and_short_tables_keep_independent_initialized_owners() {
        for log in [6, 7, 8, 9, 18] {
            let long_layout = SearchKeyLayout::for_double_fast_long(log).unwrap();
            let short_layout = SearchKeyLayout::for_fast(log, 4).unwrap();
            let mut long = MatchTable::with_layout_in(long_layout, Global).unwrap();
            let mut short = MatchTable::with_layout_in(short_layout, Global).unwrap();
            let long_key = long_layout.read_key(b"same-key").unwrap();
            let short_key = short_layout.read_key(b"same-key").unwrap();
            assert_eq!(long.lookup(long_key), Ok(None));
            assert_eq!(short.lookup(short_key), Ok(None));
            long.store(long_key, 0).unwrap();
            short.store(short_key, 19).unwrap();
            assert_eq!(long.lookup(long_key), Ok(Some(0)));
            assert_eq!(short.lookup(short_key), Ok(Some(19)));
            assert_eq!(long.store(short_key, 31), Err(Error::InvalidParameter));
            assert_eq!(short.store(long_key, 31), Err(Error::InvalidParameter));
            assert_eq!(long.lookup(long_key), Ok(Some(0)));
            assert_eq!(short.lookup(short_key), Ok(Some(19)));
            long.reset();
            assert!(long.positions.iter().all(|&entry| entry == EMPTY));
            assert_eq!(short.lookup(short_key), Ok(Some(19)));
        }
    }

    #[test]
    fn dynamic_key_and_seed_mismatches_preserve_working_candidates() {
        let layout = SearchKeyLayout::new(10, 4).unwrap();
        let mut working = MatchTable::with_layout_in(layout, Global).unwrap();
        working.replace(*b"seed", 7).unwrap();
        let before = working.positions.to_vec();
        let pointer = working.positions.as_ptr();
        let capacity = working.retained_capacity();
        for other in [
            SearchKeyLayout::new(10, 5).unwrap(),
            SearchKeyLayout::new(11, 4).unwrap(),
        ] {
            let seed = MatchTable::with_layout_in(other, Global).unwrap();
            assert_eq!(working.reset_from(&seed), Err(Error::InvalidParameter));
            assert_eq!(working.positions.as_slice(), before);
            assert_eq!(working.layout(), layout);
            assert_eq!(working.positions.as_ptr(), pointer);
            assert_eq!(working.retained_capacity(), capacity);
        }
        let wrong = SearchKeyLayout::new(10, 8)
            .unwrap()
            .read_key(b"seedtail")
            .unwrap();
        assert_eq!(working.replace(wrong, 11), Err(Error::InvalidParameter));
        assert_eq!(working.positions.as_slice(), before);
        let mut shortened_seed = MatchTable::with_layout_in(layout, Global).unwrap();
        shortened_seed.positions.pop();
        assert_eq!(
            working.reset_from(&shortened_seed),
            Err(Error::InvalidParameter)
        );
        assert_eq!(working.positions.as_slice(), before);
        assert_eq!(working.replace(*b"seed", 19), Ok(Some(7)));
    }

    #[test]
    fn dynamic_bounds_use_initialized_length_even_with_large_spare_capacity() {
        let layout = SearchKeyLayout::new(10, 4).unwrap();
        let mut table = MatchTable::with_layout_in(layout, Global).unwrap();
        // This inverse maps to u32::MAX, so it selects the final bucket.
        let last = u32::MAX.wrapping_mul(0x0e8b_2f51).to_le_bytes();
        assert_eq!(layout.bucket(last.into()), Ok(layout.bucket_count() - 1));
        table.replace(last, 7).unwrap();
        table.positions.truncate(layout.bucket_count() - 1);
        let before = table.positions.to_vec();
        let capacity = table.retained_capacity();
        assert_eq!(table.replace(last, 11), Err(Error::CorruptData));
        assert_eq!(table.positions.as_slice(), before);
        assert_eq!(table.retained_capacity(), capacity);
        let valid_seed = MatchTable::with_layout_in(layout, Global).unwrap();
        assert_eq!(table.reset_from(&valid_seed), Err(Error::InvalidParameter));
        assert_eq!(table.positions.as_slice(), before);
        table.positions.clear();
        assert_eq!(table.replace([0; 4], 13), Err(Error::CorruptData));
        assert!(table.positions.capacity() >= layout.bucket_count());
        // Reconfiguration repairs a shortened internal test state by explicitly
        // initializing the active entries; allocation capacity alone never does.
        table.reconfigure(layout).unwrap();
        assert_eq!(table.positions.len(), layout.bucket_count());
        assert_eq!(table.replace(last, 17), Ok(None));
    }

    #[test]
    fn dynamic_owner_grow_failure_preserves_state_and_shrink_regrow_make_no_requests() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = allocator.monitor();
        let identity = allocator.identity();
        let small = SearchKeyLayout::new(10, 4).unwrap();
        let large = SearchKeyLayout::new(12, 8).unwrap();
        let mut table = MatchTable::with_layout_in(small, allocator).unwrap();
        table.replace(*b"seed", 7).unwrap();
        let pointer = table.positions.as_ptr();
        let before = table.positions.to_vec();
        let capacity = table.retained_capacity();
        assert_eq!(monitor.snapshot().attempts, 1);
        assert!(monitor.fail_next());
        assert_eq!(table.reconfigure(large), Err(Error::AllocationFailed));
        assert_eq!(table.positions.as_slice(), before);
        assert_eq!(table.positions.as_ptr(), pointer);
        assert_eq!(table.positions.allocator().identity(), identity);
        assert_eq!(table.layout(), small);
        assert_eq!(table.retained_capacity(), capacity);
        assert_eq!(monitor.snapshot().attempts, 2);
        assert_eq!(monitor.snapshot().failures, 1);
        assert_eq!(table.replace(*b"seed", 11), Ok(Some(7)));
        table.reconfigure(large).unwrap();
        assert_eq!(monitor.snapshot().attempts, 3);
        assert_eq!(monitor.snapshot().grow_calls, 2);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert_eq!(table.positions.allocator().identity(), identity);
        let high_water = table.retained_capacity();
        let pointer = table.positions.as_ptr();
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        for layout in [
            SearchKeyLayout::new(10, 8).unwrap(),
            SearchKeyLayout::new(12, 5).unwrap(),
            SearchKeyLayout::new(11, 4).unwrap(),
        ] {
            table.reconfigure(layout).unwrap();
            assert_eq!(table.layout(), layout);
            assert_eq!(table.positions.len(), layout.bucket_count());
            assert!(table.positions.iter().all(|&value| value == EMPTY));
            assert_eq!(table.retained_capacity(), high_water);
            assert_eq!(table.positions.as_ptr(), pointer);
            let key = layout.read_key(b"keybytes").unwrap();
            table.replace(key, 3).unwrap();
            assert_eq!(monitor.snapshot(), before);
        }
        // The fail-next request remains armed across every in-capacity change.
        let old = table.positions.to_vec();
        let old_layout = table.layout();
        let beyond_capacity = SearchKeyLayout::new(18, 4).unwrap();
        assert!(beyond_capacity.bucket_count() > table.positions.capacity());
        assert_eq!(
            table.reconfigure(beyond_capacity),
            Err(Error::AllocationFailed)
        );
        assert_eq!(table.positions.as_slice(), old);
        assert_eq!(table.layout(), old_layout);
        assert_eq!(monitor.snapshot().attempts, before.attempts + 1);
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(table);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
        monitor.assert_released();
    }

    #[test]
    fn dynamic_borrowed_and_distinct_seed_owners_survive_independent_resizing() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let source_owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let destination_owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let source_monitor = source_owner.monitor();
        let destination_monitor = destination_owner.monitor();
        let small = SearchKeyLayout::new(10, 6).unwrap();
        let large = SearchKeyLayout::new(11, 8).unwrap();
        let mut seed = MatchTable::with_layout_in(small, &source_owner).unwrap();
        let mut working = MatchTable::with_layout_in(small, destination_owner).unwrap();
        seed.reconfigure(large).unwrap();
        working.reconfigure(large).unwrap();
        let key = large.read_key(b"seedtail").unwrap();
        seed.replace(key, 7).unwrap();
        working.replace(key, 11).unwrap();
        let source_before = source_monitor.snapshot();
        let destination_before = destination_monitor.snapshot();
        let pointer = working.positions.as_ptr();
        assert!(source_monitor.fail_next());
        assert!(destination_monitor.fail_next());
        working.reset_from(&seed).unwrap();
        assert_eq!(working.replace(key, 17), Ok(Some(7)));
        assert_eq!(seed.replace(key, 19), Ok(Some(7)));
        assert_eq!(working.positions.as_ptr(), pointer);
        assert_eq!(source_monitor.snapshot(), source_before);
        assert_eq!(destination_monitor.snapshot(), destination_before);
        let moved = working;
        drop(seed);
        source_monitor.assert_released();
        assert_eq!(destination_monitor.snapshot().live_blocks, 1);
        drop(source_owner);
        drop(moved);
        destination_monitor.assert_released();
    }

    #[test]
    fn dynamic_allocation_ordinals_release_constructor_and_growth_failures() {
        use core::num::NonZeroU64;
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        for fail_at in 1..=3 {
            let allocator = TrackingAllocator::new(AllocatorConfig {
                fail_at: NonZeroU64::new(fail_at),
                ..AllocatorConfig::default()
            })
            .unwrap()
            .into_non_clone();
            let monitor = allocator.monitor();
            let result = (|| {
                let mut table =
                    MatchTable::with_layout_in(SearchKeyLayout::new(10, 4).unwrap(), allocator)?;
                table.reconfigure(SearchKeyLayout::new(11, 5).unwrap())?;
                table.reconfigure(SearchKeyLayout::new(12, 8).unwrap())?;
                Ok::<_, Error>(table)
            })();
            assert!(matches!(result, Err(Error::AllocationFailed)));
            assert_eq!(monitor.snapshot().attempts, fail_at);
            assert_eq!(monitor.snapshot().failures, 1);
            assert_eq!(monitor.snapshot().owners_dropped, 1);
            monitor.assert_released();
        }
    }
}
