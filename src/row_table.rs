//! Independent position/tag owners for reference Greedy's scalar row search.
//! Row zero cells retain the head byte and never store a candidate position.
//! Candidate snapshots own copied indices so insertion cannot overwrite them.
#![cfg_attr(not(feature = "unsafe-row-table"), forbid(unsafe_code))]

use alloc::{alloc::Global, vec::Vec};
use core::alloc::{Allocator, Layout};

use crate::greedy_key::{RowKey, RowLayout};
use crate::{Error, Result};

const EMPTY: usize = usize::MAX;

/// Two complete Vec owners; no SIMD alignment, raw allocation or allocator
/// cloning is required. Position cells start invalid, tag/head bytes start zero.
/// Corresponds to `hashTable`/`tagTable` in `ZSTD_RowFindBestMatch`.
pub(crate) struct RowTable<P: Allocator = Global, T: Allocator = P> {
    positions: Vec<usize, P>,
    tags: Vec<u8, T>,
    layout: RowLayout,
}

/// At most 64 initialized source indices. This plain value has no allocation or
/// unsafe backend. Source/window byte bounds still belong to the parser.
pub(crate) struct CandidateSnapshot {
    values: [usize; 64],
    length: usize,
}

impl CandidateSnapshot {
    pub(crate) fn as_slice(&self) -> &[usize] {
        &self.values[..self.length]
    }
}

impl<P: Allocator, T: Allocator> RowTable<P, T> {
    pub(crate) fn new_in(layout: RowLayout, positions: P, tags: T) -> Result<Self> {
        let count = layout.cells();
        let position_layout = Layout::array::<usize>(count).map_err(|_| Error::ResourceLimit)?;
        let tag_layout = Layout::array::<u8>(count).map_err(|_| Error::ResourceLimit)?;
        position_layout
            .size()
            .checked_add(tag_layout.size())
            .ok_or(Error::ResourceLimit)?;
        let mut position_values = Vec::new_in(positions);
        position_values
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        while position_values.len() < count {
            position_values.push(EMPTY);
        }
        let mut tag_values = Vec::new_in(tags);
        tag_values
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        while tag_values.len() < count {
            tag_values.push(0);
        }
        Ok(Self {
            positions: position_values,
            tags: tag_values,
            layout,
        })
    }

    pub(crate) const fn layout(&self) -> RowLayout {
        self.layout
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        self.positions.capacity() * core::mem::size_of::<usize>() + self.tags.capacity()
    }
    pub(crate) fn reset(&mut self) {
        self.positions.fill(EMPTY);
        self.tags.fill(0);
    }

    /// Check both initialized owner extents, including the complete row, before
    /// reading its head. A valid geometry alone never authorizes an owner access.
    fn checked_row(&self, key: RowKey) -> Result<(usize, usize)> {
        if key.layout() != self.layout {
            return Err(Error::InvalidParameter);
        }
        let base = key.row_start();
        let end = base
            .checked_add(self.layout.row_len())
            .ok_or(Error::CorruptData)?;
        if end > self.positions.len() || end > self.tags.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-row-table"))]
        let (head, reserved) = (usize::from(self.tags[base]), self.positions[base]);
        #[cfg(feature = "unsafe-row-table")]
        let (head, reserved) = {
            // SAFETY: The runtime checks prove base + row_len <= both live
            // initialized lengths; row_len is a nonzero 16/32/64. Thus base is
            // in both owners. Vec retains valid naturally aligned initialized
            // u8/usize storage. Shared self prevents mutation/reallocation and
            // keeps both allocations live. Both references are immediately
            // copied and cannot escape their owner borrow.
            unsafe {
                (
                    usize::from(*self.tags.get_unchecked(base)),
                    *self.positions.get_unchecked(base),
                )
            }
        };
        if head >= self.layout.row_len() || reserved != EMPTY {
            return Err(Error::CorruptData);
        }
        if head != 0 {
            #[cfg(not(feature = "unsafe-row-table"))]
            let newest = self.positions[base + head];
            #[cfg(feature = "unsafe-row-table")]
            let newest = {
                // SAFETY: The preceding runtime checks established head <
                // row_len and the full row's initialized extent, so base+head
                // cannot overflow and is below positions.len(). The Vec owns
                // aligned initialized usize values. Shared self retains the
                // allocation and excludes writes/reallocation; the temporary
                // reference is only copied, with no escaping alias.
                unsafe { *self.positions.get_unchecked(base + head) }
            };
            if newest == EMPTY {
                return Err(Error::CorruptData);
            }
        }
        Ok((base, head))
    }

    /// Collect equal-tag candidates in head-relative cyclic order before the
    /// current position is inserted. Skip reserved cell zero; an invalid/expired
    /// candidate stops collection and a future candidate rejects the state.
    /// Corresponds to `ZSTD_row_getMatchMask` and `matchBuffer` collection, without
    /// vector masks, prefetches, or speculative source-byte accesses.
    pub(crate) fn candidates(
        &self,
        key: RowKey,
        current: usize,
        floor: usize,
    ) -> Result<CandidateSnapshot> {
        if floor > current {
            return Err(Error::InvalidParameter);
        }
        let (base, head) = self.checked_row(key)?;
        let width = self.layout.row_len();
        let mut result = CandidateSnapshot {
            values: [EMPTY; 64],
            length: 0,
        };
        for step in 0..width {
            let relative = (head + step) & (width - 1);
            if relative == 0 {
                continue;
            }
            #[cfg(not(feature = "unsafe-row-table"))]
            let (tag, position) = (self.tags[base + relative], self.positions[base + relative]);
            #[cfg(feature = "unsafe-row-table")]
            let (tag, position) = {
                // SAFETY: checked_row verified this entire row against both
                // initialized owner lengths at runtime. Masking bounds relative
                // below width, and base+width was checked without overflow. The
                // Vec owners retain aligned initialized u8/usize cells. Shared
                // self prevents changes to either length/allocation or values
                // throughout this loop. Both short-lived references are copied;
                // no reference or source pointer escapes these reads.
                unsafe {
                    (
                        *self.tags.get_unchecked(base + relative),
                        *self.positions.get_unchecked(base + relative),
                    )
                }
            };
            if tag != key.tag() {
                continue;
            }
            if position == EMPTY {
                break;
            }
            if position >= current {
                return Err(Error::CorruptData);
            }
            if position < floor {
                break;
            }
            // RowLayout caps attempts at 64. The previous iteration stopped
            // exactly at that cap, so this initialized local slot is available.
            result.values[result.length] = position;
            result.length += 1;
            if result.length == self.layout.attempts() {
                break;
            }
        }
        Ok(result)
    }

    /// Insert one position at the preceding usable slot, cycling through
    /// 1..row_len and skipping the reserved zero cell. Callers provide increasing
    /// positions and prove the hashed bytes. Complete checks precede mutation;
    /// no allocation or externally retained reference occurs in this update.
    /// Corresponds to `ZSTD_row_nextIndex` and one row-table insertion.
    pub(crate) fn insert(&mut self, key: RowKey, position: usize) -> Result<()> {
        if position == EMPTY {
            return Err(Error::InvalidParameter);
        }
        let (base, head) = self.checked_row(key)?;
        if head != 0 {
            #[cfg(not(feature = "unsafe-row-table"))]
            let newest = self.positions[base + head];
            #[cfg(feature = "unsafe-row-table")]
            let newest = {
                // SAFETY: checked_row established head < row_len and both
                // complete initialized extents without overflow. positions owns
                // aligned initialized usize cells, and exclusive self prevents
                // aliases, mutation or reallocation by another access. The
                // temporary shared reference is copied and ends before writes.
                unsafe { *self.positions.get_unchecked(base + head) }
            };
            if newest >= position {
                return Err(Error::CorruptData);
            }
        }
        let next = if head <= 1 {
            self.layout.row_len() - 1
        } else {
            head - 1
        };
        #[cfg(not(feature = "unsafe-row-table"))]
        {
            self.positions[base + next] = position;
            self.tags[base + next] = key.tag();
            self.tags[base] = next as u8;
        }
        #[cfg(feature = "unsafe-row-table")]
        {
            // SAFETY: checked_row validated both full initialized owner extents
            // and head < row_len. Both branches select next in 1..row_len, so
            // base+next and base are in bounds and cannot overflow. No allocation,
            // resizing or callbacks have occurred since those runtime checks.
            // The Vec owners retain separate valid aligned initialized usize/u8
            // allocations. Exclusive self excludes aliases or concurrent access;
            // each temporary mutable reference ends at its assignment. Copy
            // writes preserve initialization, invoke no destructor/allocator,
            // and never publish a reference or reinterpret an allocation.
            unsafe {
                *self.positions.get_unchecked_mut(base + next) = position;
                *self.tags.get_unchecked_mut(base + next) = key.tag();
                *self.tags.get_unchecked_mut(base) = next as u8;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn test_maps(&self) -> (&[usize], &[u8]) {
        (&self.positions, &self.tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greedy_key::{FRESH_ROW_SALT, GreedyKeyLayout};
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn layout(search: u8) -> RowLayout {
        RowLayout::new(GreedyKeyLayout::new(8, 4).unwrap(), search).unwrap()
    }
    fn key(layout: RowLayout) -> RowKey {
        layout
            .key(
                layout.key_layout().read_key(b"abcd").unwrap(),
                FRESH_ROW_SALT,
            )
            .unwrap()
    }

    #[test]
    fn full_row_maps_wrap_reserved_heads_and_keep_snapshots_independent() {
        // The frozen C families select the newest equal-length match after
        // width-1, width and width+1 insertions with these exact row inputs.
        for search in [4, 5, 6] {
            let layout = layout(search);
            let key = key(layout);
            let width = layout.row_len();
            for count in [width - 1, width, width + 1] {
                let mut table = RowTable::new_in(layout, Global, Global).unwrap();
                let mut expected_positions = alloc::vec![EMPTY; layout.cells()];
                let mut expected_tags = alloc::vec![0; layout.cells()];
                for n in 0..count {
                    table.insert(key, n * 20).unwrap();
                    let slot = width - 1 - n % (width - 1);
                    expected_positions[key.row_start() + slot] = n * 20;
                    expected_tags[key.row_start() + slot] = key.tag();
                    expected_tags[key.row_start()] = slot as u8;
                }
                assert_eq!(
                    table.test_maps(),
                    (&expected_positions[..], &expected_tags[..])
                );
                let before = table.candidates(key, count * 20, 0).unwrap();
                let expected: Vec<_> = ((count - (width - 1))..count)
                    .rev()
                    .map(|n| n * 20)
                    .collect();
                assert_eq!(before.as_slice(), expected);
                table.insert(key, count * 20).unwrap();
                assert_eq!(before.as_slice(), expected);
                assert_eq!(
                    table.candidates(key, count * 20 + 1, 0).unwrap().as_slice()[0],
                    count * 20
                );
                assert!(table.test_maps().0.chunks(width).all(|r| r[0] == EMPTY));
                assert_eq!(table.layout(), layout);
            }
        }
    }

    #[test]
    fn zero_tags_empty_sentinels_budgets_and_expired_rows_have_distinct_roles() {
        for search in [1, 2, 3, 4, 5, 6, 7, 29, 30] {
            let layout = layout(search);
            let zero = layout
                .key(layout.key_layout().read_key(&[0; 4]).unwrap(), 0)
                .unwrap();
            let mut table = RowTable::new_in(layout, Global, Global).unwrap();
            assert!(table.candidates(zero, 1, 0).unwrap().as_slice().is_empty());
            for p in 0..15 {
                table.insert(zero, p).unwrap();
            }
            let expected: Vec<_> = (0..15).rev().take(layout.attempts()).collect();
            assert_eq!(table.candidates(zero, 15, 0).unwrap().as_slice(), expected);
            assert_eq!(table.candidates(zero, 15, 14).unwrap().as_slice(), [14]);
            assert!(
                table
                    .candidates(zero, 15, 15)
                    .unwrap()
                    .as_slice()
                    .is_empty()
            );
            table.reset();
            assert!(table.test_maps().0.iter().all(|&p| p == EMPTY));
            assert!(table.test_maps().1.iter().all(|&t| t == 0));
            table.insert(zero, 0).unwrap();
            assert_eq!(
                table
                    .candidates(zero, 2 * 1024 * 1024, 0)
                    .unwrap()
                    .as_slice(),
                [0]
            );
        }
    }

    #[test]
    fn mixed_tags_follow_retained_source_history_in_cyclic_order() {
        let layout = layout(6);
        let mut table = RowTable::new_in(layout, Global, Global).unwrap();
        let mut history = Vec::new();
        for p in 0..1024 {
            let value = (p % 97) as u32;
            let key = layout
                .key(
                    layout.key_layout().read_key(&value.to_le_bytes()).unwrap(),
                    FRESH_ROW_SALT,
                )
                .unwrap();
            table.insert(key, p).unwrap();
            history.push((key, p));
        }
        for value in 0_u32..97 {
            let key = layout
                .key(
                    layout.key_layout().read_key(&value.to_le_bytes()).unwrap(),
                    FRESH_ROW_SALT,
                )
                .unwrap();
            for floor in [0, 950, 1024] {
                let expected: Vec<_> = history
                    .iter()
                    .rev()
                    .filter(|(k, _)| k.row_start() == key.row_start())
                    .take(layout.row_len() - 1)
                    .filter(|(k, p)| k.tag() == key.tag() && *p >= floor)
                    .map(|(_, p)| *p)
                    .collect();
                assert_eq!(
                    table.candidates(key, 1024, floor).unwrap().as_slice(),
                    expected
                );
            }
        }
    }

    #[test]
    fn malformed_heads_future_indices_and_truncated_owners_reject_before_updates() {
        let layout = layout(4);
        let key = key(layout);
        let base = key.row_start();
        let mut table = RowTable::new_in(layout, Global, Global).unwrap();
        table.insert(key, 7).unwrap();
        for bad in [16, 255] {
            table.tags[base] = bad;
            assert!(matches!(
                table.candidates(key, 10, 0),
                Err(Error::CorruptData)
            ));
            let before = (table.positions.clone(), table.tags.clone());
            assert_eq!(table.insert(key, 8), Err(Error::CorruptData));
            assert_eq!(table.test_maps(), (&before.0[..], &before.1[..]));
        }
        table.tags[base] = 15;
        table.positions[base + 15] = 10;
        assert!(matches!(
            table.candidates(key, 10, 0),
            Err(Error::CorruptData)
        ));
        table.positions[base + 15] = EMPTY;
        assert!(matches!(
            table.candidates(key, 10, 0),
            Err(Error::CorruptData)
        ));
        table.positions[base + 15] = 7;
        table.positions[base] = 0;
        assert!(matches!(
            table.candidates(key, 10, 0),
            Err(Error::CorruptData)
        ));
        table.positions[base] = EMPTY;
        for short_tags in [true, false] {
            let mut short = RowTable::new_in(layout, Global, Global).unwrap();
            if short_tags {
                short.tags.truncate(base + 15);
            } else {
                short.positions.truncate(base + 15);
            }
            let before = (short.positions.clone(), short.tags.clone());
            assert_eq!(short.insert(key, 8), Err(Error::CorruptData));
            assert!(matches!(
                short.candidates(key, 10, 0),
                Err(Error::CorruptData)
            ));
            assert_eq!(short.test_maps(), (&before.0[..], &before.1[..]));
        }
    }

    #[test]
    fn invalid_requests_and_nonincreasing_positions_leave_both_owners_unchanged() {
        let layout = layout(4);
        let key = key(layout);
        let mut table = RowTable::new_in(layout, Global, Global).unwrap();
        table.insert(key, 9).unwrap();
        let before = (table.positions.clone(), table.tags.clone());
        for p in [0, 9, EMPTY] {
            assert!(table.insert(key, p).is_err());
        }
        let other = RowLayout::new(GreedyKeyLayout::new(8, 5).unwrap(), 4).unwrap();
        let other = other
            .key(
                other.key_layout().read_key(b"abcde").unwrap(),
                FRESH_ROW_SALT,
            )
            .unwrap();
        assert_eq!(table.insert(other, 10), Err(Error::InvalidParameter));
        assert!(matches!(
            table.candidates(key, 10, 11),
            Err(Error::InvalidParameter)
        ));
        assert_eq!(table.test_maps(), (&before.0[..], &before.1[..]));
    }

    #[test]
    fn exact_independent_owners_release_on_failures_moves_and_reset() {
        for reject in 0..=2 {
            let p = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let t = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let pm = p.monitor();
            let tm = t.monitor();
            if reject == 1 {
                assert!(pm.fail_next());
            }
            if reject == 2 {
                assert!(tm.fail_next());
            }
            let layout = RowLayout::new(GreedyKeyLayout::new(19, 6).unwrap(), 6).unwrap();
            let result = RowTable::new_in(layout, p, t);
            if reject == 0 {
                let mut table = result.unwrap();
                let bytes = (1 << 19) * (core::mem::size_of::<usize>() + 1);
                assert_eq!(table.retained_capacity(), bytes);
                assert_eq!(
                    pm.snapshot().live_requested_bytes + tm.snapshot().live_requested_bytes,
                    bytes as u128
                );
                let before = (pm.snapshot(), tm.snapshot());
                table.reset();
                assert_eq!((pm.snapshot(), tm.snapshot()), before);
                drop(table);
            } else {
                assert!(matches!(result, Err(Error::AllocationFailed)));
            }
            assert_eq!(pm.snapshot().attempts, 1);
            assert_eq!(tm.snapshot().attempts, u64::from(reject != 1));
            for m in [pm, tm] {
                m.assert_released();
                assert_eq!(m.snapshot().owners_dropped, 1);
            }
        }
        let a = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let b = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let am = a.monitor();
        let bm = b.monitor();
        let mut first = RowTable::new_in(layout(4), &a, &b).unwrap();
        let mut second = RowTable::new_in(layout(6), &b, &a).unwrap();
        first.insert(key(first.layout()), 0).unwrap();
        core::mem::swap(&mut first, &mut second);
        assert_eq!(
            second
                .candidates(key(second.layout()), 1, 0)
                .unwrap()
                .as_slice(),
            [0]
        );
        drop((first, second));
        am.assert_released();
        bm.assert_released();
        assert_eq!(
            (am.snapshot().owners_dropped, bm.snapshot().owners_dropped),
            (0, 0)
        );
        drop((a, b));
        assert_eq!(
            (am.snapshot().owners_dropped, bm.snapshot().owners_dropped),
            (1, 1)
        );
    }
}
