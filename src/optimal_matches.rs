//! Initialized bounded candidates for private Optimal match enumeration.
#![cfg_attr(not(feature = "unsafe-optimal-matches"), forbid(unsafe_code))]
#![allow(
    dead_code,
    reason = "Private sorted search integration follows ownership"
)]
use crate::optimal_search_layout::OptimalMatch;
use crate::{Error, Result};
use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

pub(crate) const MATCH_CAPACITY: usize = 4099;
pub(crate) struct OptimalMatches<A: Allocator = Global> {
    cells: Vec<OptimalMatch, A>,
    live: usize,
}
impl<A: Allocator> OptimalMatches<A> {
    pub(crate) fn new_in(allocator: A) -> Result<Self> {
        let mut cells = Vec::new_in(allocator);
        cells
            .try_reserve_exact(MATCH_CAPACITY)
            .map_err(|_| Error::AllocationFailed)?;
        cells.resize(MATCH_CAPACITY, OptimalMatch::EMPTY);
        Ok(Self { cells, live: 0 })
    }
    pub(crate) fn clear(&mut self) {
        self.live = 0;
    }
    pub(crate) fn as_slice(&self) -> Result<&[OptimalMatch]> {
        if self.live > self.cells.len() || self.live > MATCH_CAPACITY {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-optimal-matches"))]
        {
            Ok(&self.cells[..self.live])
        }
        #[cfg(feature = "unsafe-optimal-matches")]
        {
            // SAFETY: The runtime guard proves the entire range lies within
            // initialized aligned live Vec storage. Shared borrowing excludes
            // mutation/reallocation for the returned slice's lifetime, which
            // is tied to this owner; empty ranges are valid too.
            Ok(unsafe { self.cells.get_unchecked(..self.live) })
        }
    }
    pub(crate) fn push(&mut self, value: OptimalMatch) -> Result<()> {
        if value.off_base() == 0 || value.length() < 3 {
            return Err(Error::InvalidParameter);
        }
        let previous = self.as_slice()?.last().copied();
        if previous.is_some_and(|p| p.length() >= value.length()) {
            return Err(Error::InvalidParameter);
        }
        if self.live >= MATCH_CAPACITY || self.live >= self.cells.len() {
            return Err(Error::ResourceLimit);
        }
        #[cfg(not(feature = "unsafe-optimal-matches"))]
        {
            self.cells[self.live] = value;
        }
        #[cfg(feature = "unsafe-optimal-matches")]
        {
            // SAFETY: Both logical capacity and initialized length were checked
            // at runtime; no resizing/callback intervenes. Vec provides aligned
            // valid storage and exclusive borrowing excludes every alias during
            // this assignment. Copy replacement preserves initialization and
            // the temporary reference ends before publishing the new length.
            unsafe {
                *self.cells.get_unchecked_mut(self.live) = value;
            }
        }
        self.live += 1;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    #[test]
    fn strict_lengths_capacity_and_clear() {
        let mut m = OptimalMatches::new_in(Global).unwrap();
        assert!(m.as_slice().unwrap().is_empty());
        assert_eq!(m.push(OptimalMatch::EMPTY), Err(Error::InvalidParameter));
        for i in 0..MATCH_CAPACITY {
            m.push(OptimalMatch::new(1, i + 3).unwrap()).unwrap();
        }
        let before = m.as_slice().unwrap().to_vec();
        assert_eq!(
            m.push(OptimalMatch::new(2, MATCH_CAPACITY + 3).unwrap()),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            m.push(OptimalMatch::new(2, 3).unwrap()),
            Err(Error::InvalidParameter)
        );
        assert_eq!(m.as_slice().unwrap(), before);
        m.clear();
        assert!(m.as_slice().unwrap().is_empty());
        m.push(OptimalMatch::new(3, 524280).unwrap()).unwrap();
        assert_eq!(m.as_slice().unwrap().len(), 1);
    }
    #[test]
    fn malformed_lengths_and_shortened_storage_are_checked() {
        let mut m = OptimalMatches::new_in(Global).unwrap();
        m.cells.truncate(1);
        m.push(OptimalMatch::new(1, 3).unwrap()).unwrap();
        assert_eq!(
            m.push(OptimalMatch::new(1, 4).unwrap()),
            Err(Error::ResourceLimit)
        );
        m.live = 2;
        assert!(m.as_slice().is_err());
        assert_eq!(
            m.push(OptimalMatch::new(1, 5).unwrap()),
            Err(Error::CorruptData)
        );
        m.live = usize::MAX;
        assert!(m.as_slice().is_err());
        m.clear();
        assert!(m.as_slice().unwrap().is_empty());
    }
    #[test]
    fn moved_nonclone_success_and_failure_release() {
        for reject in [false, true] {
            let a = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let monitor = a.monitor();
            if reject {
                assert!(monitor.fail_next());
            }
            let result = OptimalMatches::new_in(a);
            if reject {
                assert!(matches!(result, Err(Error::AllocationFailed)));
            } else {
                let mut m = result.unwrap();
                assert_eq!(
                    monitor.snapshot().live_requested_bytes,
                    (MATCH_CAPACITY * core::mem::size_of::<OptimalMatch>()) as u128
                );
                m.clear();
                drop(m);
            }
            assert_eq!(monitor.snapshot().attempts, 1);
            monitor.assert_released();
            assert_eq!(monitor.snapshot().owners_dropped, 1);
        }
    }
    #[test]
    fn borrowed_allocator_and_no_allocation_after_construction() {
        let a = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = a.monitor();
        let mut m = OptimalMatches::new_in(&a).unwrap();
        assert!(monitor.fail_next());
        for i in 0..MATCH_CAPACITY {
            m.push(OptimalMatch::new(1, i + 3).unwrap()).unwrap();
        }
        m.clear();
        m.push(OptimalMatch::new(2, 9).unwrap()).unwrap();
        assert_eq!(monitor.snapshot().attempts, 1);
        drop(m);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        assert!(matches!(
            OptimalMatches::new_in(&a),
            Err(Error::AllocationFailed)
        ));
        drop(a);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
}
