//! Initialized allocator-aware path storage with explicit forward/reverse phases.
#![cfg_attr(not(feature = "unsafe-optimal-paths"), forbid(unsafe_code))]
#![allow(
    dead_code,
    reason = "Private path parser integration follows ownership"
)]
use crate::optimal_path_layout::{
    ForwardStretch, PATH_CAPACITY, PathCell, PathIndex, PathPhase, ReverseSequence,
};
use crate::{Error, Result};
use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

pub(crate) struct OptimalPaths<A: Allocator = Global> {
    cells: Vec<PathCell, A>,
    phase: PathPhase,
}
impl<A: Allocator> OptimalPaths<A> {
    pub(crate) fn new_in(allocator: A) -> Result<Self> {
        let mut cells = Vec::new_in(allocator);
        cells
            .try_reserve_exact(PATH_CAPACITY)
            .map_err(|_| Error::AllocationFailed)?;
        cells.resize(PATH_CAPACITY, PathCell::ZERO);
        Ok(Self {
            cells,
            phase: PathPhase::Forward,
        })
    }
    fn check(&self, index: usize, phase: PathPhase) -> Result<()> {
        PathIndex::new(index)?;
        if self.cells.len() != PATH_CAPACITY || self.phase != phase {
            return Err(Error::CorruptData);
        }
        Ok(())
    }
    pub(crate) const fn phase(&self) -> PathPhase {
        self.phase
    }
    pub(crate) fn read(&self, index: usize, phase: PathPhase) -> Result<PathCell> {
        self.check(index, phase)?;
        #[cfg(not(feature = "unsafe-optimal-paths"))]
        {
            Ok(self.cells[index])
        }
        #[cfg(feature = "unsafe-optimal-paths")]
        {
            // SAFETY: check proves index < PATH_CAPACITY == initialized Vec length.
            // Vec owns valid aligned PathCell storage, and no resize/callback
            // intervenes. Shared borrowing excludes mutation while copying; the
            // returned Copy value has no pointer or lifetime tied to storage.
            Ok(unsafe { *self.cells.get_unchecked(index) })
        }
    }
    pub(crate) fn write(&mut self, index: usize, cell: PathCell, phase: PathPhase) -> Result<()> {
        self.check(index, phase)?;
        #[cfg(not(feature = "unsafe-optimal-paths"))]
        {
            self.cells[index] = cell;
        }
        #[cfg(feature = "unsafe-optimal-paths")]
        {
            // SAFETY: check establishes index in initialized, aligned Vec storage
            // with exact fixed length. Exclusive borrowing excludes aliases and
            // no operation can resize between check and assignment. A complete
            // initialized Copy cell replaces one cell; the reference ends here.
            unsafe {
                *self.cells.get_unchecked_mut(index) = cell;
            }
        }
        Ok(())
    }
    pub(crate) fn forward(&self, index: usize) -> Result<ForwardStretch> {
        self.read(index, PathPhase::Forward)?.forward()
    }
    pub(crate) fn reverse(&self, index: usize) -> Result<ReverseSequence> {
        Ok(self.read(index, PathPhase::Reconstruction)?.reverse())
    }
    pub(crate) fn begin_reconstruction(&mut self) -> Result<()> {
        self.check(0, PathPhase::Forward)?;
        self.phase = PathPhase::Reconstruction;
        Ok(())
    }
    pub(crate) fn resume_forward(&mut self) -> Result<()> {
        self.check(0, PathPhase::Reconstruction)?;
        self.phase = PathPhase::Forward;
        Ok(())
    }
    /// Copy a complete initialized snapshot without allocating or exposing aliases.
    pub(crate) fn snapshot(&self, destination: &mut [PathCell]) -> Result<()> {
        if self.cells.len() != PATH_CAPACITY || destination.len() != PATH_CAPACITY {
            return Err(Error::InvalidParameter);
        }
        destination.copy_from_slice(&self.cells);
        Ok(())
    }
    pub(crate) fn reset(&mut self) -> Result<()> {
        if self.cells.len() != PATH_CAPACITY {
            return Err(Error::CorruptData);
        }
        self.cells.fill(PathCell::ZERO);
        self.phase = PathPhase::Forward;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimal_path_fixture as fixture;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    #[test]
    fn phases_reject_wrong_role_and_reset_preserves_allocation() {
        let mut paths = OptimalPaths::new_in(Global).unwrap();
        let cell = PathCell::from_words([256, 4, 3, 1, 1, 4, 8]).unwrap();
        paths
            .write(PATH_CAPACITY - 1, cell, PathPhase::Forward)
            .unwrap();
        assert_eq!(paths.forward(PATH_CAPACITY - 1).unwrap().cell(), cell);
        assert!(paths.reverse(0).is_err());
        assert!(paths.resume_forward().is_err());
        assert_eq!(paths.phase(), PathPhase::Forward);
        paths.begin_reconstruction().unwrap();
        assert!(paths.begin_reconstruction().is_err());
        assert!(paths.forward(0).is_err());
        assert!(paths.write(0, cell, PathPhase::Forward).is_err());
        assert_eq!(paths.reverse(PATH_CAPACITY - 1).unwrap().cell(), cell);
        paths.write(0, cell, PathPhase::Reconstruction).unwrap();
        paths.resume_forward().unwrap();
        assert_eq!(paths.forward(0).unwrap().cell(), cell);
        paths.begin_reconstruction().unwrap();
        paths.reset().unwrap();
        assert_eq!(paths.phase(), PathPhase::Forward);
        assert!(paths.cells.iter().all(|&c| c == PathCell::ZERO));
    }
    #[test]
    fn invalid_indices_and_corrupted_length_fail_before_access_or_mutation() {
        let mut paths = OptimalPaths::new_in(Global).unwrap();
        for index in [PATH_CAPACITY, usize::MAX] {
            assert!(paths.read(index, PathPhase::Forward).is_err());
            assert!(
                paths
                    .write(index, PathCell::ZERO, PathPhase::Forward)
                    .is_err()
            );
        }
        let mut snapshot = vec![PathCell::ZERO; PATH_CAPACITY];
        paths.snapshot(&mut snapshot).unwrap();
        assert!(paths.snapshot(&mut snapshot[..PATH_CAPACITY - 1]).is_err());
        paths.cells.truncate(PATH_CAPACITY - 1);
        assert!(paths.read(0, PathPhase::Forward).is_err());
        assert!(paths.write(0, PathCell::ZERO, PathPhase::Forward).is_err());
        assert!(paths.snapshot(&mut snapshot).is_err());
        assert!(paths.reset().is_err());
        assert!(paths.begin_reconstruction().is_err());
        assert_eq!(paths.phase(), PathPhase::Forward);
    }
    #[test]
    fn moved_nonclone_allocator_exact_size_and_failure_cleanup() {
        for reject in [false, true] {
            let a = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let monitor = a.monitor();
            if reject {
                assert!(monitor.fail_next());
            }
            let result = OptimalPaths::new_in(a);
            if reject {
                assert!(matches!(result, Err(Error::AllocationFailed)));
            } else {
                let mut paths = result.unwrap();
                assert_eq!(
                    monitor.snapshot().live_requested_bytes,
                    (PATH_CAPACITY * core::mem::size_of::<PathCell>()) as u128
                );
                paths.reset().unwrap();
                drop(paths);
            }
            assert_eq!(monitor.snapshot().attempts, 1);
            monitor.assert_released();
            assert_eq!(monitor.snapshot().owners_dropped, 1);
        }
    }
    #[test]
    fn borrowed_nonclone_owner_reuses_initialized_cells_without_allocation() {
        let a = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = a.monitor();
        let mut paths = OptimalPaths::new_in(&a).unwrap();
        assert!(monitor.fail_next());
        let cell = PathCell::from_words([0, 4, 3, 0, 1, 4, 8]).unwrap();
        for i in 0..PATH_CAPACITY {
            paths.write(i, cell, PathPhase::Forward).unwrap();
        }
        paths.begin_reconstruction().unwrap();
        for i in 0..PATH_CAPACITY {
            assert_eq!(paths.reverse(i).unwrap().cell(), cell);
        }
        paths.reset().unwrap();
        assert_eq!(monitor.snapshot().attempts, 1);
        drop(paths);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        assert!(matches!(
            OptimalPaths::new_in(&a),
            Err(Error::AllocationFailed)
        ));
        drop(a);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
    #[test]
    fn every_captured_complete_path_map_is_identical_through_both_phases() {
        let cases = fixture::read(fixture::PACK).unwrap();
        let mut paths = OptimalPaths::new_in(Global).unwrap();
        let mut expected = vec![PathCell::ZERO; PATH_CAPACITY];
        let mut actual = expected.clone();
        let mut events = 0;
        for case in cases {
            paths.reset().unwrap();
            expected.fill(PathCell::ZERO);
            for event in case.events {
                let phase = if event.reverse_phase {
                    PathPhase::Reconstruction
                } else {
                    PathPhase::Forward
                };
                if phase != paths.phase() {
                    if event.reverse_phase {
                        paths.begin_reconstruction().unwrap();
                    } else {
                        paths.resume_forward().unwrap();
                    }
                }
                for (at, words) in event.path {
                    let cell = PathCell::from_words(words).unwrap();
                    expected[at] = cell;
                    paths.write(at, cell, phase).unwrap();
                    assert_eq!(paths.read(at, phase).unwrap(), cell);
                }
                paths.snapshot(&mut actual).unwrap();
                assert_eq!(actual, expected, "{} event {events}", case.name);
                events += 1;
            }
        }
        assert_eq!(events, 22_024);
    }
}
