//! Borrowed source bounds and copied search coordinates for DoubleFast.
//!
//! All positions are indices, and thresholds are scalar counters rather than
//! source addresses. These allocation-free records share one safe representation
//! under every feature; the independently owned tables select their own backend.
#![forbid(unsafe_code)]

use core::ops::Range;

use crate::frame::MAX_BLOCK_SIZE;
use crate::match_key::MatchKey;
use crate::match_search::SearchKeyLayout;
use crate::{Error, Result};

#[derive(Clone, Copy)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "DoubleFast parser integration follows")
)]
pub(crate) struct DoubleFastBlock<'a> {
    source: &'a [u8],
    start: usize,
    end: usize,
    prefix_floor: usize,
    long_layout: SearchKeyLayout,
    short_layout: SearchKeyLayout,
    window_size: usize,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "DoubleFast parser integration follows")
)]
impl<'a> DoubleFastBlock<'a> {
    /// Admit a direct no-dictionary parser block without allocating or changing
    /// its borrowed source. The ordinary frame dispatcher handles lengths 0..6
    /// separately. A source may contain later blocks, but key reads stop at this
    /// range's end. The long and short dimensions are independently checked.
    /// Corresponds to the source/window bounds of
    /// `ZSTD_compressBlock_doubleFast_noDict_generic` in zstd v1.5.7.
    pub(crate) fn new(
        source: &'a [u8],
        range: Range<usize>,
        prefix_floor: usize,
        long_layout: SearchKeyLayout,
        short_layout: SearchKeyLayout,
        window_size: usize,
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
            || SearchKeyLayout::for_double_fast_long(long_layout.hash_log())? != long_layout
            || SearchKeyLayout::for_fast(short_layout.hash_log(), short_layout.key_bytes())?
                != short_layout
        {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            source,
            start: range.start,
            end: range.end,
            prefix_floor,
            long_layout,
            short_layout,
            window_size,
        })
    }

    pub(crate) const fn source(&self) -> &'a [u8] {
        self.source
    }

    pub(crate) fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    pub(crate) const fn prefix_floor(&self) -> usize {
        self.prefix_floor
    }

    pub(crate) const fn window_size(&self) -> usize {
        self.window_size
    }

    pub(crate) const fn layouts(&self) -> (SearchKeyLayout, SearchKeyLayout) {
        (self.long_layout, self.short_layout)
    }

    /// Current long/short candidates may equal this block-end-derived floor.
    /// The later long candidate applies its separate strict comparison.
    pub(crate) fn ordinary_floor(&self) -> usize {
        self.prefix_floor
            .max(self.end.saturating_sub(self.window_size))
    }

    pub(crate) fn initial_position(&self) -> usize {
        // An admitted block has at least seven bytes, proving start+1 <= end.
        self.start + usize::from(self.start == self.ordinary_floor())
    }

    /// Repeat availability is decided at entry, not at the block end. A repeat
    /// can therefore refer before ordinary_floor while respecting this floor.
    pub(crate) fn repeat_floor(&self) -> usize {
        self.prefix_floor
            .max(self.initial_position().saturating_sub(self.window_size))
    }

    fn key(&self, position: usize, layout: SearchKeyLayout) -> Result<MatchKey> {
        self.source
            .get(position..self.end)
            .and_then(|bytes| layout.read_key(bytes))
            .ok_or(Error::CorruptData)
    }

    pub(crate) fn long_key(&self, position: usize) -> Result<MatchKey> {
        self.key(position, self.long_layout)
    }

    pub(crate) fn short_key(&self, position: usize) -> Result<MatchKey> {
        self.key(position, self.short_layout)
    }

    pub(crate) fn initial_cursor(&self) -> Result<Option<DoubleFastCursor>> {
        self.search_from(self.initial_position())
    }

    /// Restart the search after a match with step1 and a new relative growth
    /// threshold. A seven/eight-byte tail can be a valid block without any
    /// searchable pair. No key read is attempted on that cleanup path.
    pub(crate) fn search_from(&self, position: usize) -> Result<Option<DoubleFastCursor>> {
        if position < self.start || position > self.end {
            return Err(Error::InvalidParameter);
        }
        let Some(limit) = self.end.checked_sub(8) else {
            return Ok(None);
        };
        if position >= limit {
            return Ok(None);
        }
        // position < limit proves the next coordinate fits as well. The
        // threshold is never used as a source index and may exceed block end.
        Ok(Some(DoubleFastCursor {
            current: position,
            next: position + 1,
            step: 1,
            next_threshold: position.checked_add(256).ok_or(Error::ResourceLimit)?,
            limit,
        }))
    }
}

/// Two complete eight-byte key starts and a positive sampling gap.
/// This copied record owns no source reference or allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "DoubleFast parser integration follows")
)]
pub(crate) struct DoubleFastCursor {
    current: usize,
    next: usize,
    step: usize,
    next_threshold: usize,
    limit: usize,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "DoubleFast parser integration follows")
)]
impl DoubleFastCursor {
    pub(crate) const fn current(&self) -> usize {
        self.current
    }

    pub(crate) const fn next(&self) -> usize {
        self.next
    }

    pub(crate) const fn step(&self) -> usize {
        self.step
    }

    /// Advance after an unsuccessful probe. Growth is tested against the old
    /// next position before that position becomes current, matching the pinned
    /// noDict DoubleFast sampling schedule. Stop before exposing an incomplete
    /// key pair. All scalar additions remain checked even without pointer use.
    pub(crate) fn advance(self) -> Result<Option<Self>> {
        let (step, next_threshold) = if self.next >= self.next_threshold {
            (
                self.step.checked_add(1).ok_or(Error::ResourceLimit)?,
                self.next_threshold
                    .checked_add(256)
                    .ok_or(Error::ResourceLimit)?,
            )
        } else {
            (self.step, self.next_threshold)
        };
        let next = self.next.checked_add(step).ok_or(Error::ResourceLimit)?;
        if next > self.limit {
            return Ok(None);
        }
        Ok(Some(Self {
            current: self.next,
            next,
            step,
            next_threshold,
            limit: self.limit,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn block(
        source: &[u8],
        range: Range<usize>,
        floor: usize,
        window: usize,
    ) -> Result<DoubleFastBlock<'_>> {
        DoubleFastBlock::new(
            source,
            range,
            floor,
            SearchKeyLayout::for_double_fast_long(18).unwrap(),
            SearchKeyLayout::for_fast(6, 7).unwrap(),
            window,
        )
    }

    #[test]
    fn rejects_invalid_ranges_windows_lengths_and_table_roles() {
        let source = vec![0; MAX_BLOCK_SIZE + 32];
        for range in [
            Range { start: 10, end: 9 },
            0..usize::MAX,
            usize::MAX..usize::MAX,
            0..0,
            0..6,
            0..MAX_BLOCK_SIZE + 1,
        ] {
            assert!(matches!(
                block(&source, range, 0, 2_097_152),
                Err(Error::InvalidParameter)
            ));
        }
        assert!(matches!(
            block(&source, 16..32, 17, 1024),
            Err(Error::InvalidParameter)
        ));
        assert!(matches!(
            block(&source, 0..1025, 0, 1024),
            Err(Error::InvalidParameter)
        ));
        for window in [0, 1023, 1025, 2_097_153, 4_194_304, usize::MAX] {
            assert!(matches!(
                block(&source, 0..16, 0, window),
                Err(Error::InvalidParameter)
            ));
        }
        let long = SearchKeyLayout::for_double_fast_long(6).unwrap();
        let short = SearchKeyLayout::for_fast(18, 4).unwrap();
        for (first, second) in [(short, short), (long, long)] {
            assert!(matches!(
                DoubleFastBlock::new(&source, 0..16, 0, first, second, 1024),
                Err(Error::InvalidParameter)
            ));
        }
        let valid = DoubleFastBlock::new(&source, 0..16, 0, long, short, 2_097_152).unwrap();
        assert_eq!(valid.source(), source);
        assert_eq!(valid.range(), 0..16);
        assert_eq!(valid.prefix_floor(), 0);
        assert_eq!(valid.window_size(), 2_097_152);
        assert_eq!(valid.layouts(), (long, short));
    }

    #[test]
    fn frozen_seven_through_ten_byte_entry_boundaries_do_not_read_incomplete_keys() {
        let source = [0; 40];
        for length in 7..=10 {
            let fresh = block(&source, 0..length, 0, 1024).unwrap();
            assert_eq!(fresh.initial_position(), 1);
            assert_eq!(
                fresh
                    .initial_cursor()
                    .unwrap()
                    .map(|c| (c.current(), c.next())),
                (length == 10).then_some((1, 2))
            );
            let prefix = block(&source, 16..16 + length, 0, 1024).unwrap();
            assert_eq!(prefix.initial_position(), 16);
            assert_eq!(
                prefix
                    .initial_cursor()
                    .unwrap()
                    .map(|c| (c.current(), c.next())),
                (length >= 9).then_some((16, 17))
            );
            let expired = block(&source, 16..16 + length, 16, 1024).unwrap();
            assert_eq!(
                expired
                    .initial_cursor()
                    .unwrap()
                    .map(|c| (c.current(), c.next())),
                (length == 10).then_some((17, 18))
            );
        }
    }

    #[test]
    fn ordinary_and_repeat_floors_remain_distinct_and_keys_stop_at_block_end() {
        let source = vec![0x71; 2048];
        let borrowed = block(&source, 1500..1564, 0, 1024).unwrap();
        assert_eq!(borrowed.ordinary_floor(), 540);
        assert_eq!(borrowed.repeat_floor(), 476);
        assert_eq!(borrowed.initial_position(), 1500);
        assert!(borrowed.long_key(0).is_ok());
        assert!(borrowed.long_key(1556).is_ok());
        assert_eq!(borrowed.long_key(1557), Err(Error::CorruptData));
        assert!(borrowed.short_key(1557).is_ok());
        assert_eq!(borrowed.short_key(1558), Err(Error::CorruptData));
        assert_eq!(borrowed.long_key(usize::MAX), Err(Error::CorruptData));
        assert_eq!(borrowed.short_key(usize::MAX), Err(Error::CorruptData));
        assert_eq!(borrowed.search_from(1499), Err(Error::InvalidParameter));
        assert_eq!(borrowed.search_from(1565), Err(Error::InvalidParameter));
        assert_eq!(borrowed.search_from(1564), Ok(None));
        let retained = block(&source, 1500..1564, 600, 1024).unwrap();
        assert_eq!(
            (retained.ordinary_floor(), retained.repeat_floor()),
            (600, 600)
        );
        let full_window = block(&source, 32..1056, 0, 1024).unwrap();
        assert_eq!(
            (
                full_window.ordinary_floor(),
                full_window.initial_position(),
                full_window.repeat_floor()
            ),
            (32, 33, 0)
        );
    }

    #[test]
    fn sampling_growth_and_cleanup_match_the_frozen_no_match_coordinates() {
        let source = vec![0; 65_728];
        // Frozen conditional-store inputs start at prefix32. A current probe
        // at relative512 has step3; relative770 has step4 and next774.
        let borrowed = block(&source, 32..814, 0, 1024).unwrap();
        let mut cursor = borrowed.initial_cursor().unwrap();
        let mut step3 = None;
        let mut step4 = None;
        let mut last = None;
        while let Some(current) = cursor {
            assert!(current.current() < current.next());
            assert!(borrowed.long_key(current.current()).is_ok());
            assert!(borrowed.long_key(current.next()).is_ok());
            if current.current() == 32 + 512 {
                step3 = Some((current.step(), current.next() - 32));
            }
            if current.current() == 32 + 770 {
                step4 = Some((current.step(), current.next() - 32));
            }
            last = Some(current.current() - 32);
            cursor = current.advance().unwrap();
        }
        assert_eq!(step3, Some((3, 515)));
        assert_eq!(step4, Some((4, 774)));
        assert_eq!(last, Some(770));
        let restarted = borrowed.search_from(500).unwrap().unwrap();
        assert_eq!(
            (restarted.current(), restarted.next(), restarted.step()),
            (500, 501, 1)
        );
        // The preserved long-literal-candidate emitted no matches. Its largest
        // final C table position is relative65298; the 128-byte tail is skipped.
        // Only this scalar no-match schedule is exercised here, not parsing.
        let large = block(&source, 64..65_728, 0, 131_072).unwrap();
        let mut cursor = large.initial_cursor().unwrap();
        let mut last = 0;
        while let Some(current) = cursor {
            last = current.current();
            cursor = current.advance().unwrap();
        }
        assert_eq!(last - 64, 65_298);
    }
}
