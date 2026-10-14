//! Independent optional short-key owner for private Optimal search.
#![cfg_attr(not(feature = "unsafe-optimal-short-key"), forbid(unsafe_code))]
#![allow(
    dead_code,
    reason = "Private sorted search integration follows ownership"
)]
use crate::{Error, Result};
use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

pub(crate) struct OptimalShortKey<A: Allocator = Global> {
    cells: Vec<usize, A>,
    log: Option<u8>,
}
impl<A: Allocator> OptimalShortKey<A> {
    pub(crate) fn new_in(log: Option<u8>, allocator: A) -> Result<Self> {
        if log.is_some_and(|v| !(6..=17).contains(&v)) {
            return Err(Error::InvalidParameter);
        }
        let count = log.map_or(0, |v| 1usize << v);
        let mut cells = Vec::new_in(allocator);
        cells
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        cells.resize(count, usize::MAX);
        Ok(Self { cells, log })
    }
    pub(crate) const fn log(&self) -> Option<u8> {
        self.log
    }
    pub(crate) fn reset(&mut self) {
        self.cells.fill(usize::MAX);
    }
    pub(crate) fn get(&self, bucket: usize) -> Result<Option<usize>> {
        if bucket >= self.cells.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-optimal-short-key"))]
        let value = self.cells[bucket];
        #[cfg(feature = "unsafe-optimal-short-key")]
        let value = {
            // SAFETY: The runtime check bounds the index by initialized length.
            // Vec supplies aligned live usize cells. Shared borrowing prevents
            // writes/reallocation during this copied read; no reference escapes.
            unsafe { *self.cells.get_unchecked(bucket) }
        };
        if value == usize::MAX {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }
    pub(crate) fn replace(&mut self, bucket: usize, position: usize) -> Result<Option<usize>> {
        if position > (u32::MAX - 3) as usize {
            return Err(Error::InvalidParameter);
        }
        let old = self.get(bucket)?;
        if old.is_some_and(|p| p >= position) {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-optimal-short-key"))]
        {
            self.cells[bucket] = position;
        }
        #[cfg(feature = "unsafe-optimal-short-key")]
        {
            // SAFETY: get checked bucket against initialized length and no
            // resizing or callback intervened. The exclusive Vec borrow ensures
            // valid aligned live storage and excludes aliases. Replacing a usize
            // preserves initialization; the temporary reference ends here.
            unsafe {
                *self.cells.get_unchecked_mut(bucket) = position;
            }
        }
        Ok(old)
    }
    #[cfg(test)]
    pub(crate) fn test_cells(&self) -> &[usize] {
        &self.cells
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    #[test]
    fn enabled_disabled_and_last_cell() {
        for log in [None, Some(6), Some(17)] {
            let mut t = OptimalShortKey::new_in(log, Global).unwrap();
            assert_eq!(t.log(), log);
            if let Some(log) = log {
                let last = (1 << log) - 1;
                assert_eq!(t.get(last), Ok(None));
                assert_eq!(t.replace(last, 0), Ok(None));
                assert_eq!(t.replace(last, 9), Ok(Some(0)));
                let before = t.cells.clone();
                assert_eq!(t.replace(last, 9), Err(Error::CorruptData));
                assert_eq!(t.replace(last, 8), Err(Error::CorruptData));
                assert_eq!(t.replace(last, usize::MAX), Err(Error::InvalidParameter));
                assert_eq!(t.cells, before);
                t.reset();
                assert!(t.cells.iter().all(|&v| v == usize::MAX));
            }
            assert_eq!(t.get(t.cells.len()), Err(Error::CorruptData));
            assert_eq!(t.replace(t.cells.len(), 0), Err(Error::CorruptData));
        }
        for log in [0, 5, 18, 255] {
            assert!(OptimalShortKey::new_in(Some(log), Global).is_err());
        }
    }
    #[test]
    fn shortened_storage_is_checked() {
        let mut t = OptimalShortKey::new_in(Some(6), Global).unwrap();
        t.cells.truncate(3);
        assert_eq!(t.get(3), Err(Error::CorruptData));
        assert_eq!(t.replace(63, 1), Err(Error::CorruptData));
    }
    #[test]
    fn moved_nonclone_failure_and_disabled_allocator() {
        for log in [None, Some(6)] {
            for reject in [false, true] {
                let a = TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone();
                let m = a.monitor();
                if reject {
                    assert!(m.fail_next());
                }
                let result = OptimalShortKey::new_in(log, a);
                if reject && log.is_some() {
                    assert!(matches!(result, Err(Error::AllocationFailed)));
                } else {
                    let mut t = result.unwrap();
                    t.reset();
                    drop(t);
                }
                assert_eq!(m.snapshot().attempts, u64::from(log.is_some()));
                m.assert_released();
                assert_eq!(m.snapshot().owners_dropped, 1);
            }
        }
    }
    #[test]
    fn borrowed_owner_reset_never_allocates() {
        let a = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let m = a.monitor();
        let mut t = OptimalShortKey::new_in(Some(17), &a).unwrap();
        assert_eq!(
            m.snapshot().live_requested_bytes,
            ((1 << 17) * core::mem::size_of::<usize>()) as u128
        );
        assert!(m.fail_next());
        t.replace(131071, 123).unwrap();
        t.reset();
        assert_eq!(t.get(131071), Ok(None));
        assert_eq!(m.snapshot().attempts, 1);
        drop(t);
        m.assert_released();
        assert_eq!(m.snapshot().owners_dropped, 0);
        assert!(matches!(
            OptimalShortKey::new_in(Some(6), &a),
            Err(Error::AllocationFailed)
        ));
        drop(a);
        assert_eq!(m.snapshot().owners_dropped, 1);
    }
}
