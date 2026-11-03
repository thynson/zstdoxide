//! Two independently allocated candidate tables for DoubleFast.
//!
//! This composite uses only the checked `MatchTable` contract. Its leaves share
//! the `unsafe-match-table` feature; the pair adds no unchecked representation.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "DoubleFast parser integration follows")
)]

use alloc::alloc::Global;
use core::alloc::{Allocator, Layout};

use crate::match_search::SearchKeyLayout;
use crate::matchfinder::{MatchTable, MatchTableView};
use crate::{Error, Result};

/// Complete owners of the long and short hash tables, with independent layouts
/// and allocator values. The short table contains positions, not chain links.
/// Corresponds to `hashTable` and `chainTable` in DoubleFast's match state.
/// No allocator clone or equivalence between the two owners is required.
pub(crate) struct DoubleFastTables<L: Allocator = Global, S: Allocator = L> {
    pub(crate) long: MatchTable<L>,
    pub(crate) short: MatchTable<S>,
}

impl<L: Allocator, S: Allocator> DoubleFastTables<L, S> {
    /// Validate both table roles and the summed allocation size before the
    /// first request. Each allocation remains with its exact allocator value.
    /// If short-table allocation fails, the complete long owner is dropped;
    /// if the first allocation fails, the unused short allocator is dropped.
    pub(crate) fn new_in(
        long_layout: SearchKeyLayout,
        short_layout: SearchKeyLayout,
        long_allocator: L,
        short_allocator: S,
    ) -> Result<Self> {
        if SearchKeyLayout::for_double_fast_long(long_layout.hash_log())? != long_layout
            || SearchKeyLayout::for_fast(short_layout.hash_log(), short_layout.key_bytes())?
                != short_layout
        {
            return Err(Error::InvalidParameter);
        }
        let long_size = Layout::array::<usize>(long_layout.bucket_count())
            .map_err(|_| Error::ResourceLimit)?
            .size();
        let short_size = Layout::array::<usize>(short_layout.bucket_count())
            .map_err(|_| Error::ResourceLimit)?
            .size();
        long_size
            .checked_add(short_size)
            .ok_or(Error::ResourceLimit)?;
        let long = MatchTable::with_layout_in(long_layout, long_allocator)?;
        let short = MatchTable::with_layout_in(short_layout, short_allocator)?;
        Ok(Self { long, short })
    }

    /// Invalidate all sampled positions without allocating or moving owners.
    pub(crate) fn reset(&mut self) {
        self.long.reset();
        self.short.reset();
    }

    /// Seed every staged key position of both tables so candidates can point
    /// into the dictionary region; parsing never inserts those positions
    /// itself. Later positions win each slot, and the parser's window floor
    /// still bounds every candidate it accepts. This is the combined
    /// dictionary staging contract shared with the Fast profile.
    pub(crate) fn seed_prefix(&mut self, source: &[u8], prefix_len: usize) -> Result<()> {
        self.seed_range(source, 0, prefix_len)
    }

    /// Seed only the positions of `start..end`, leaving the compiled suffix to
    /// its read-only owner. Ascending insertion preserves the newest-wins slot
    /// contract for retained frame history.
    pub(crate) fn seed_range(&mut self, source: &[u8], start: usize, end: usize) -> Result<()> {
        seed_table_range(&mut self.long, source, start, end)?;
        seed_table_range(&mut self.short, source, start, end)
    }

    pub(crate) fn layouts(&self) -> (SearchKeyLayout, SearchKeyLayout) {
        (self.long.layout(), self.short.layout())
    }

    /// Borrow both sampled arrays without allocating or gaining a write path.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "compiled DoubleFast consultation is test-only so far"
        )
    )]
    pub(crate) fn view(&self) -> (MatchTableView<'_>, MatchTableView<'_>) {
        (self.long.view(), self.short.view())
    }

    /// Payload capacity only; excludes this inline pair and borrowed source.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.long.retained_capacity() + self.short.retained_capacity()
    }
}

/// Seed one table with every key position of `start..end` in the combined
/// borrow. Later positions win each slot; the parser's window floor bounds what
/// it accepts from these candidates.
fn seed_table_range<A: Allocator>(
    table: &mut MatchTable<A>,
    source: &[u8],
    start: usize,
    end: usize,
) -> Result<()> {
    let layout = table.layout();
    if let Some(last) = end.checked_sub(usize::from(layout.key_bytes()))
        && start <= last
    {
        for position in start..=last {
            let key = layout
                .read_key(&source[position..])
                .ok_or(Error::InvalidState)?;
            table.replace(key, position)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn layouts(long: u8, short: u8, width: u8) -> (SearchKeyLayout, SearchKeyLayout) {
        (
            SearchKeyLayout::for_double_fast_long(long).unwrap(),
            SearchKeyLayout::for_fast(short, width).unwrap(),
        )
    }

    #[test]
    fn both_roles_are_checked_before_either_allocator_is_requested() {
        let (long, short) = layouts(6, 18, 7);
        for (first, second) in [(short, short), (long, long)] {
            let l = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let s = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let lm = l.monitor();
            let sm = s.monitor();
            assert!(matches!(
                DoubleFastTables::new_in(first, second, l, s),
                Err(Error::InvalidParameter)
            ));
            for monitor in [lm, sm] {
                assert_eq!(monitor.snapshot().attempts, 0);
                assert_eq!(monitor.snapshot().owners_dropped, 1);
                monitor.assert_released();
            }
        }
    }

    #[test]
    fn either_allocation_failure_releases_both_exact_nonclone_owners() {
        for reject_first in [true, false] {
            let l = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let s = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let lm = l.monitor();
            let sm = s.monitor();
            assert!(if reject_first {
                lm.fail_next()
            } else {
                sm.fail_next()
            });
            let (long, short) = layouts(7, 9, 5);
            assert!(matches!(
                DoubleFastTables::new_in(long, short, l, s),
                Err(Error::AllocationFailed)
            ));
            assert_eq!(lm.snapshot().attempts, 1);
            assert_eq!(sm.snapshot().attempts, u64::from(!reject_first));
            assert_eq!(lm.snapshot().successes, u64::from(!reject_first));
            assert_eq!(lm.snapshot().deallocate_calls, u64::from(!reject_first));
            assert_eq!(sm.snapshot().successes, 0);
            for monitor in [lm, sm] {
                assert_eq!(monitor.snapshot().owners_dropped, 1);
                monitor.assert_released();
            }
        }
    }

    #[test]
    fn unequal_dimensions_and_borrowed_owner_reset_without_new_requests() {
        for (long_log, short_log) in [(6, 18), (18, 6)] {
            for width in 4..=7 {
                let l = TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone();
                let s = TrackingAllocator::new(AllocatorConfig::default())
                    .unwrap()
                    .into_non_clone();
                let lm = l.monitor();
                let sm = s.monitor();
                let (long, short) = layouts(long_log, short_log, width);
                let mut tables = DoubleFastTables::new_in(long, short, l, &s).unwrap();
                assert_eq!(tables.layouts(), (long, short));
                let expected_bytes =
                    (long.bucket_count() + short.bucket_count()) * core::mem::size_of::<usize>();
                assert_eq!(tables.retained_capacity(), expected_bytes);
                assert_eq!(tables.long.test_positions().len(), long.bucket_count());
                assert_eq!(tables.short.test_positions().len(), short.bucket_count());
                assert!(
                    tables
                        .long
                        .test_positions()
                        .iter()
                        .all(|&p| p == usize::MAX)
                );
                assert!(
                    tables
                        .short
                        .test_positions()
                        .iter()
                        .all(|&p| p == usize::MAX)
                );
                let lk = long.read_key(b"long-key").unwrap();
                let sk = short.read_key(b"long-key").unwrap();
                tables.long.store(lk, 0).unwrap();
                tables.short.store(sk, 37).unwrap();
                assert_eq!(tables.long.lookup(lk), Ok(Some(0)));
                assert_eq!(tables.short.lookup(sk), Ok(Some(37)));
                tables.reset();
                assert!(
                    tables
                        .long
                        .test_positions()
                        .iter()
                        .all(|&p| p == usize::MAX)
                );
                assert!(
                    tables
                        .short
                        .test_positions()
                        .iter()
                        .all(|&p| p == usize::MAX)
                );
                assert_eq!(tables.retained_capacity(), expected_bytes);
                assert_eq!(lm.snapshot().attempts, 1);
                assert_eq!(sm.snapshot().attempts, 1);
                drop(tables);
                lm.assert_released();
                sm.assert_released();
                assert_eq!(lm.snapshot().owners_dropped, 1);
                assert_eq!(sm.snapshot().owners_dropped, 0);
                drop(s);
                assert_eq!(sm.snapshot().owners_dropped, 1);
            }
        }
    }

    #[test]
    fn moves_and_swaps_keep_distinct_clone_identities_with_their_tables() {
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = allocator.monitor();
        let (long, short) = layouts(6, 9, 4);
        let mut first =
            DoubleFastTables::new_in(long, short, allocator.clone(), allocator.clone()).unwrap();
        let mut second =
            DoubleFastTables::new_in(long, short, allocator.clone(), allocator.clone()).unwrap();
        let lk = long.read_key(b"same-key").unwrap();
        let sk = short.read_key(b"same-key").unwrap();
        first.long.store(lk, 11).unwrap();
        first.short.store(sk, 13).unwrap();
        second.long.store(lk, 17).unwrap();
        second.short.store(sk, 19).unwrap();
        assert_eq!(monitor.snapshot().identities_created, 5);
        drop(allocator);
        core::mem::swap(&mut first, &mut second);
        assert_eq!(first.long.lookup(lk), Ok(Some(17)));
        assert_eq!(first.short.lookup(sk), Ok(Some(19)));
        assert_eq!(second.long.lookup(lk), Ok(Some(11)));
        assert_eq!(second.short.lookup(sk), Ok(Some(13)));
        let moved = first;
        drop(second);
        assert_eq!(monitor.snapshot().live_blocks, 2);
        assert_eq!(monitor.snapshot().owners_dropped, 3);
        drop(moved);
        assert_eq!(monitor.snapshot().attempts, 4);
        assert_eq!(monitor.snapshot().owners_dropped, 5);
        monitor.assert_released();
    }
}
