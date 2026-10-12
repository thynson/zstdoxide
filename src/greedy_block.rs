//! Borrowed source bounds and copied Greedy search coordinates.
//! Indices replace C pointers; row and chain search retain their distinct tail
//! margins. These allocation-free records are safe under every storage backend.
#![forbid(unsafe_code)]
#![cfg_attr(not(test), allow(dead_code, reason = "Greedy parsing follows"))]

use crate::frame::MAX_BLOCK_SIZE;
use crate::greedy_key::{GreedyKey, GreedyKeyLayout};
use crate::{Error, Result};
use core::ops::Range;

#[derive(Clone, Copy)]
pub(crate) struct GreedyBlock<'a> {
    source: &'a [u8],
    start: usize,
    end: usize,
    prefix_floor: usize,
    key: GreedyKeyLayout,
    window_size: usize,
    row_search: bool,
}

impl<'a> GreedyBlock<'a> {
    /// A direct block has at least seven bytes; the ordinary dispatcher handles
    /// shorter blocks without entering this parser. The entire immutable source
    /// may include later blocks, but every key read stops at this block's end.
    /// Corresponds to source/window bounds in depth-zero
    /// `ZSTD_compressBlock_lazy_generic`, in facebook/zstd v1.5.7.
    pub(crate) fn new(
        source: &'a [u8],
        range: Range<usize>,
        prefix_floor: usize,
        key: GreedyKeyLayout,
        window_size: usize,
        row_search: bool,
    ) -> Result<Self> {
        let length = range
            .end
            .checked_sub(range.start)
            .ok_or(Error::InvalidParameter)?;
        if range.end > source.len()
            || prefix_floor > range.start
            || !(1024..=2_097_152).contains(&window_size)
            || !window_size.is_power_of_two()
            || !(7..=MAX_BLOCK_SIZE.min(window_size)).contains(&length)
        {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            source,
            start: range.start,
            end: range.end,
            prefix_floor,
            key,
            window_size,
            row_search,
        })
    }
    pub(crate) const fn source(self) -> &'a [u8] {
        self.source
    }
    pub(crate) fn range(self) -> Range<usize> {
        self.start..self.end
    }
    pub(crate) const fn prefix_floor(self) -> usize {
        self.prefix_floor
    }
    pub(crate) const fn key_layout(self) -> GreedyKeyLayout {
        self.key
    }
    pub(crate) const fn window_size(self) -> usize {
        self.window_size
    }
    pub(crate) const fn row_search(self) -> bool {
        self.row_search
    }

    pub(crate) fn initial_position(self) -> usize {
        // At least seven admitted bytes prove the optional increment fits.
        self.start + usize::from(self.start == self.prefix_floor)
    }

    /// Search eligibility uses this probe's distance, not the block end.
    /// Equality with the floor is allowed. Retained prefix_floor stays separate
    /// for backward extension, which moves both sides without changing distance.
    pub(crate) fn candidate_floor(self, current: usize) -> Result<usize> {
        if !(self.start..=self.end).contains(&current) {
            return Err(Error::InvalidParameter);
        }
        Ok(self
            .prefix_floor
            .max(current.saturating_sub(self.window_size)))
    }
    pub(crate) fn repeat_floor(self) -> usize {
        self.prefix_floor
            .max(self.initial_position().saturating_sub(self.window_size))
    }

    /// Chain preserves the final eight bytes; rows preserve sixteen (C's eight
    /// byte read plus eight-entry hash-cache lookahead). Even a scalar Rust row
    /// search must retain the same logical tail boundary for reference parity.
    fn search_limit(self) -> Option<usize> {
        self.end.checked_sub(if self.row_search { 16 } else { 8 })
    }

    pub(crate) fn key(self, position: usize) -> Result<GreedyKey> {
        self.source
            .get(position..self.end)
            .and_then(|s| self.key.read_key(s))
            .ok_or(Error::CorruptData)
    }

    pub(crate) fn initial_cursor(self) -> Result<Option<GreedyCursor>> {
        self.search_from(self.initial_position(), self.start)
    }

    /// Main search uses a strict limit. Invalid scalar coordinates reject even
    /// if no search would be possible; later source bytes cannot rescue a tail.
    pub(crate) fn search_from(self, current: usize, anchor: usize) -> Result<Option<GreedyCursor>> {
        if anchor < self.start || anchor > current || current > self.end {
            return Err(Error::InvalidParameter);
        }
        Ok(self
            .search_limit()
            .filter(|&limit| current < limit)
            .map(|limit| GreedyCursor {
                current,
                anchor,
                limit,
            }))
    }

    /// After storing a match, immediate repeat cleanup includes the limit.
    /// This is distinct from entering/restarting main search at the same index.
    pub(crate) fn cleanup_allowed(self, current: usize) -> Result<bool> {
        if !(self.start..=self.end).contains(&current) {
            return Err(Error::InvalidParameter);
        }
        Ok(self.search_limit().is_some_and(|limit| current <= limit))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GreedyCursor {
    current: usize,
    anchor: usize,
    limit: usize,
}

/// Preserve the final skipping flag even when advancing exits main search.
/// This value owns no table, source reference or allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GreedyMiss {
    pub(crate) next: Option<GreedyCursor>,
    pub(crate) skipping: bool,
}

impl GreedyCursor {
    pub(crate) const fn current(self) -> usize {
        self.current
    }
    pub(crate) const fn anchor(self) -> usize {
        self.anchor
    }

    /// Advance by ((current-anchor) >> 8) + 1; skipping starts above eight.
    /// Corresponds to the unsuccessful Greedy probe in
    /// `ZSTD_compressBlock_lazy_generic`. Unlike DoubleFast, the step is derived
    /// directly from the current distance to the literal anchor each time.
    pub(crate) fn miss(self) -> Result<GreedyMiss> {
        if self.current >= self.limit {
            return Err(Error::CorruptData);
        }
        let distance = self
            .current
            .checked_sub(self.anchor)
            .ok_or(Error::CorruptData)?;
        let step = (distance >> 8).checked_add(1).ok_or(Error::ResourceLimit)?;
        let next = self.current.checked_add(step).ok_or(Error::ResourceLimit)?;
        Ok(GreedyMiss {
            next: (next < self.limit).then_some(Self {
                current: next,
                ..self
            }),
            skipping: step > 8,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    fn key() -> GreedyKeyLayout {
        GreedyKeyLayout::new(19, 6).unwrap()
    }

    #[test]
    fn fresh_and_prefixed_search_tails_keep_strict_and_inclusive_limits_separate() {
        let source = [0; 64];
        for row in [false, true] {
            let margin = if row { 16 } else { 8 };
            for prefix in [0, 1, 17] {
                for size in 7..=20 {
                    let block =
                        GreedyBlock::new(&source, prefix..prefix + size, 0, key(), 1024, row)
                            .unwrap();
                    let initial = prefix + usize::from(prefix == 0);
                    assert_eq!(block.initial_position(), initial);
                    assert_eq!(
                        block.initial_cursor().unwrap().is_some(),
                        size > margin + usize::from(prefix == 0)
                    );
                    assert_eq!(block.row_search(), row);
                    assert_eq!(block.range(), prefix..prefix + size);
                    assert_eq!(block.source(), source);
                    assert_eq!(block.key_layout(), key());
                    assert_eq!(block.window_size(), 1024);
                    if size >= margin {
                        let limit = prefix + size - margin;
                        assert!(block.search_from(limit, prefix).unwrap().is_none());
                        assert!(block.cleanup_allowed(limit).unwrap());
                        assert!(!block.cleanup_allowed(limit + 1).unwrap());
                    } else {
                        assert!(!block.cleanup_allowed(prefix).unwrap());
                    }
                }
            }
        }
    }

    #[test]
    fn per_probe_floor_changes_without_replacing_retained_prefix() {
        let source = vec![0; 4096];
        for row in [false, true] {
            let block = GreedyBlock::new(&source, 2000..3024, 900, key(), 1024, row).unwrap();
            assert_eq!(block.prefix_floor(), 900);
            assert_eq!(block.initial_position(), 2000);
            assert_eq!(block.repeat_floor(), 976);
            for (current, floor) in [(2000, 976), (2100, 1076), (3024, 2000)] {
                assert_eq!(block.candidate_floor(current), Ok(floor));
            }
            assert_eq!(block.candidate_floor(1999), Err(Error::InvalidParameter));
            assert_eq!(block.candidate_floor(3025), Err(Error::InvalidParameter));
            let fresh = GreedyBlock::new(&source, 2000..3024, 2000, key(), 1024, row).unwrap();
            assert_eq!(fresh.initial_position(), 2001);
            assert_eq!(fresh.repeat_floor(), 2000);
        }
    }

    #[test]
    fn key_reads_use_logical_end_and_never_later_block_padding() {
        let source = *b"abcdefghijklmnopqrstuvwxyz0123456789";
        for width in 4..=6 {
            let layout = GreedyKeyLayout::new(19, width).unwrap();
            let block = GreedyBlock::new(&source, 8..24, 0, layout, 1024, true).unwrap();
            assert_eq!(
                block.key(0),
                Ok(layout.read_key(&source[..usize::from(width)]).unwrap())
            );
            assert_eq!(
                block.key(24 - usize::from(width)),
                Ok(layout
                    .read_key(&source[24 - usize::from(width)..24])
                    .unwrap())
            );
            for p in [25 - usize::from(width), 24, 25, usize::MAX] {
                assert_eq!(block.key(p), Err(Error::CorruptData));
            }
        }
    }

    #[test]
    fn invalid_ranges_windows_and_cursor_coordinates_reject_before_access() {
        let source = [0; 2048];
        for (start, end, floor, window) in [
            (9, 8, 0, 1024),
            (0, 6, 0, 1024),
            (0, 2049, 0, 2048),
            (0, 8, 1, 1024),
            (0, 8, 0, 0),
            (0, 8, 0, 1023),
            (0, 8, 0, 1025),
            (0, 1025, 0, 1024),
            (0, 8, 0, 4_194_304),
            (0, usize::MAX, 0, 1024),
        ] {
            assert!(matches!(
                GreedyBlock::new(&source, start..end, floor, key(), window, false),
                Err(Error::InvalidParameter)
            ));
        }
        let block = GreedyBlock::new(&source, 20..84, 0, key(), 1024, false).unwrap();
        for (current, anchor) in [(19, 20), (20, 19), (20, 21), (85, 20), (usize::MAX, 20)] {
            assert_eq!(
                block.search_from(current, anchor),
                Err(Error::InvalidParameter)
            );
        }
        assert_eq!(block.cleanup_allowed(19), Err(Error::InvalidParameter));
        assert_eq!(block.cleanup_allowed(85), Err(Error::InvalidParameter));
        for cursor in [
            GreedyCursor {
                current: 2,
                anchor: 3,
                limit: 9,
            },
            GreedyCursor {
                current: 9,
                anchor: 0,
                limit: 9,
            },
        ] {
            assert_eq!(cursor.miss(), Err(Error::CorruptData));
        }
        assert_eq!(
            GreedyCursor {
                current: usize::MAX - 1,
                anchor: 0,
                limit: usize::MAX
            }
            .miss(),
            Err(Error::ResourceLimit)
        );
    }

    #[test]
    fn empty_match_noise_schedules_match_frozen_chain_and_row_final_flags() {
        // Actual no-match C families: chain/row-skip-tail-{2047,2064,4096}.
        // C nextToUpdate is logical last_probe+2 (chain) or +3 (row).
        // These test only the copied schedule; full parser records follow later.
        for (row, size, last, skipping) in [
            (false, 2047, 2038, false),
            (false, 2064, 2054, true),
            (false, 4096, 4075, true),
            (true, 2047, 2030, false),
            (true, 2064, 2046, false),
            (true, 4096, 4075, true),
        ] {
            let source = vec![0; size];
            let block = GreedyBlock::new(&source, 0..size, 0, key(), 8192, row).unwrap();
            let mut cursor = block.initial_cursor().unwrap().unwrap();
            loop {
                assert_eq!(cursor.anchor(), 0);
                let missed = cursor.miss().unwrap();
                if let Some(next) = missed.next {
                    cursor = next;
                } else {
                    assert_eq!(cursor.current(), last);
                    assert_eq!(missed.skipping, skipping);
                    break;
                }
            }
        }
        let source = vec![0; 4096];
        let block = GreedyBlock::new(&source, 0..4096, 0, key(), 8192, true).unwrap();
        // Moving the anchor after a match restores a unit step at that position.
        let next = block
            .search_from(3000, 3000)
            .unwrap()
            .unwrap()
            .miss()
            .unwrap();
        assert_eq!(next.next.unwrap().current(), 3001);
        assert!(!next.skipping);
    }
}
