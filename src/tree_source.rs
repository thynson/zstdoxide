//! Borrowed logical source for direct and retained-prefix tree queries.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Private tree parser integration follows query conformance"
    )
)]
use crate::greedy_block::GreedyBlock;
use crate::greedy_key::{GreedyKey, GreedyKeyLayout};
use crate::{Error, Result};
use core::ops::Range;
#[derive(Clone, Copy)]
pub(crate) struct TreeSource<'a> {
    source: &'a [u8],
    start: usize,
    end: usize,
    prefix_floor: usize,
    key: GreedyKeyLayout,
    window_size: usize,
}
impl<'a> TreeSource<'a> {
    /// Direct search may inspect more than one window of source; window expiry
    /// is checked per candidate, independently of the immutable source extent.
    pub(crate) fn new(
        source: &'a [u8],
        range: Range<usize>,
        prefix_floor: usize,
        key: GreedyKeyLayout,
        window_size: usize,
    ) -> Result<Self> {
        let size = range
            .end
            .checked_sub(range.start)
            .ok_or(Error::InvalidParameter)?;
        if range.end > source.len()
            || !(8..=crate::frame::MAX_BLOCK_SIZE).contains(&size)
            || prefix_floor > range.start
            || !(1024..=2097152).contains(&window_size)
            || !window_size.is_power_of_two()
            || range.end.checked_add(2).is_none()
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
        })
    }
    pub(crate) fn from_block(block: GreedyBlock<'a>) -> Result<Self> {
        if block.row_search() {
            return Err(Error::InvalidParameter);
        }
        Self::new(
            block.source(),
            block.range(),
            block.prefix_floor(),
            block.key_layout(),
            block.window_size(),
        )
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
    pub(crate) fn key(self, position: usize) -> Result<GreedyKey> {
        if position < self.prefix_floor || position.checked_add(8).is_none_or(|p| p > self.end) {
            return Err(Error::CorruptData);
        }
        self.key
            .read_key(
                self.source
                    .get(position..self.end)
                    .ok_or(Error::CorruptData)?,
            )
            .ok_or(Error::CorruptData)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_extent_and_retained_prefix_keep_window_expiry_independent() {
        let source = [0; 4096];
        let key = GreedyKeyLayout::new(6, 4).unwrap();
        let direct = TreeSource::new(&source, 0..4096, 0, key, 1024).unwrap();
        assert!(direct.key(4088).is_ok());
        assert_eq!(direct.key(4089), Err(Error::CorruptData));
        let block = GreedyBlock::new(&source, 2048..3072, 1024, key, 1024, false).unwrap();
        let retained = TreeSource::from_block(block).unwrap();
        assert!(retained.key(1024).is_ok());
        assert_eq!(retained.key(1023), Err(Error::CorruptData));
        assert_eq!(retained.range(), 2048..3072);
        for (start, end, floor, window) in [
            (0, 7, 0, 1024),
            (0, 4097, 0, 1024),
            (0, 4096, 1, 1024),
            (0, 4096, 0, 1025),
            (10, 9, 0, 1024),
        ] {
            assert!(TreeSource::new(&source, start..end, floor, key, window).is_err());
        }
        assert!(
            TreeSource::from_block(GreedyBlock::new(&source, 0..1024, 0, key, 1024, true).unwrap())
                .is_err()
        );
    }
}
