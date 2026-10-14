//! Checked copied values for forward Optimal paths and in-place reconstruction.
//! Corresponds to ZSTD_optimal_t and _shortestPath in zstd_opt.c v1.5.7.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Private path parser integration follows these records"
)]
use crate::{Error, Result};

pub(crate) const PATH_HORIZON: usize = 4096;
pub(crate) const PATH_CAPACITY: usize = PATH_HORIZON + 3;
pub(crate) const BLOCK_LIMIT: u32 = 131_072;
const UNREACHABLE: u32 = 1 << 30;

/// A finite nonnegative price or the explicit unreachable-path sentinel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PathPrice(u32);
impl PathPrice {
    pub(crate) const UNREACHABLE: Self = Self(UNREACHABLE);
    pub(crate) fn finite(value: u32) -> Result<Self> {
        if value >= UNREACHABLE {
            return Err(Error::InvalidParameter);
        }
        Ok(Self(value))
    }
    fn from_word(value: u32) -> Result<Self> {
        if value == UNREACHABLE {
            Ok(Self::UNREACHABLE)
        } else {
            Self::finite(value)
        }
    }
    pub(crate) const fn word(self) -> u32 {
        self.0
    }
    pub(crate) fn value(self) -> Result<u32> {
        if self.0 >= UNREACHABLE {
            return Err(Error::CorruptData);
        }
        Ok(self.0)
    }
    /// Signed literal-length price increments can decrease a finite cost.
    pub(crate) fn adjusted(self, delta: i64) -> Result<Self> {
        let value = i64::from(self.value()?)
            .checked_add(delta)
            .ok_or(Error::ResourceLimit)?;
        let value = u32::try_from(value).map_err(|_| Error::ResourceLimit)?;
        if value >= UNREACHABLE {
            return Err(Error::ResourceLimit);
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PathIndex(usize);
impl PathIndex {
    pub(crate) fn new(value: usize) -> Result<Self> {
        if value >= PATH_CAPACITY {
            return Err(Error::InvalidParameter);
        }
        Ok(Self(value))
    }
    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// Fully initialized, copied storage; unused fields are preserved, not interpreted.
/// Plain records share this safe representation under every access backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PathCell([u32; 7]);
impl PathCell {
    pub(crate) const ZERO: Self = Self([0; 7]);
    pub(crate) fn from_words(words: [u32; 7]) -> Result<Self> {
        PathPrice::from_word(words[0])?;
        if words[1] > BLOCK_LIMIT + 3
            || words[2] > BLOCK_LIMIT
            || words[3] > BLOCK_LIMIT
            || words[4..].iter().any(|&v| v > BLOCK_LIMIT)
            || (words[2] > 0 && (words[2] < 3 || words[1] == 0))
        {
            return Err(Error::InvalidParameter);
        }
        Ok(Self(words))
    }
    pub(crate) const fn words(self) -> [u32; 7] {
        self.0
    }
    pub(crate) fn price(self) -> PathPrice {
        if self.0[0] == UNREACHABLE {
            PathPrice::UNREACHABLE
        } else {
            PathPrice(self.0[0])
        }
    }
    pub(crate) const fn off_base(self) -> u32 {
        self.0[1]
    }
    pub(crate) const fn match_length(self) -> u32 {
        self.0[2]
    }
    pub(crate) const fn literal_length(self) -> u32 {
        self.0[3]
    }
    pub(crate) const fn repeats(self) -> [u32; 3] {
        [self.0[4], self.0[5], self.0[6]]
    }
    pub(crate) fn with_price(self, price: PathPrice) -> Result<Self> {
        let mut words = self.0;
        words[0] = price.word();
        Self::from_words(words)
    }
    pub(crate) fn with_literals(self, length: u32) -> Result<Self> {
        let mut words = self.0;
        words[3] = length;
        Self::from_words(words)
    }
    pub(crate) fn with_match(self, off_base: u32, length: u32) -> Result<Self> {
        if off_base == 0 || length < 3 {
            return Err(Error::InvalidParameter);
        }
        let mut words = self.0;
        words[1] = off_base;
        words[2] = length;
        Self::from_words(words)
    }
    pub(crate) fn without_match(self) -> Self {
        let mut words = self.0;
        words[2] = 0;
        Self(words)
    }
    pub(crate) fn with_repeats(self, repeats: [u32; 3]) -> Result<Self> {
        let mut words = self.0;
        words[4..].copy_from_slice(&repeats);
        Self::from_words(words)
    }
    pub(crate) fn forward(self) -> Result<ForwardStretch> {
        self.price().value()?;
        Ok(ForwardStretch(self))
    }
    pub(crate) fn reverse(self) -> ReverseSequence {
        ReverseSequence(self)
    }
}

/// Match followed by literals. Literal-only origins include bytes before index 0.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ForwardStretch(PathCell);
impl ForwardStretch {
    pub(crate) fn cell(self) -> PathCell {
        self.0
    }
    pub(crate) fn predecessor(self, at: PathIndex) -> Result<Option<PathIndex>> {
        if self.0.match_length() == 0 {
            return Ok(None);
        }
        let extent = self
            .0
            .match_length()
            .checked_add(self.0.literal_length())
            .ok_or(Error::CorruptData)? as usize;
        let previous = at.get().checked_sub(extent).ok_or(Error::CorruptData)?;
        // A matched cell has a positive length >= 3, so the predecessor decreases.
        Ok(Some(PathIndex::new(previous)?))
    }
}

/// Literals followed by a match; source advance is independent of slot index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReverseSequence(PathCell);
impl ReverseSequence {
    pub(crate) fn cell(self) -> PathCell {
        self.0
    }
    pub(crate) fn end(self, anchor: usize, source_end: usize) -> Result<usize> {
        if source_end > BLOCK_LIMIT as usize || anchor > source_end {
            return Err(Error::InvalidParameter);
        }
        let extent = self.0.literal_length() as usize + self.0.match_length() as usize;
        let end = anchor
            .checked_add(extent)
            .filter(|&v| v <= source_end)
            .ok_or(Error::CorruptData)?;
        Ok(end)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PathPhase {
    Forward,
    Reconstruction,
}

/// Copied coordinates only: fetch the predecessor before any overlapping write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReverseCursor {
    predecessor: PathIndex,
    destination: PathIndex,
    end: PathIndex,
}
impl ReverseCursor {
    pub(crate) fn new(predecessor: PathIndex) -> Result<Self> {
        let end = PathIndex::new(
            predecessor
                .get()
                .checked_add(2)
                .ok_or(Error::ResourceLimit)?,
        )?;
        Ok(Self {
            predecessor,
            destination: end,
            end,
        })
    }
    pub(crate) const fn predecessor(self) -> PathIndex {
        self.predecessor
    }
    pub(crate) const fn destination(self) -> PathIndex {
        self.destination
    }
    pub(crate) const fn end(self) -> PathIndex {
        self.end
    }
    pub(crate) fn advance(self, copied: ForwardStretch) -> Result<Option<Self>> {
        let Some(previous) = copied.predecessor(self.predecessor)? else {
            return Ok(None);
        };
        let destination = PathIndex::new(
            self.destination
                .get()
                .checked_sub(1)
                .ok_or(Error::CorruptData)?,
        )?;
        if destination.get() <= previous.get() {
            return Err(Error::CorruptData);
        }
        Ok(Some(Self {
            predecessor: previous,
            destination,
            end: self.end,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimal_path_fixture as fixture;

    #[test]
    fn finite_prices_reject_negative_overflow_and_sentinel_arithmetic() {
        assert_eq!(
            PathPrice::finite(256).unwrap().adjusted(-255),
            Ok(PathPrice::finite(1).unwrap())
        );
        assert!(PathPrice::finite(0).unwrap().adjusted(-1).is_err());
        assert!(PathPrice::finite(UNREACHABLE).is_err());
        assert!(
            PathPrice::finite(UNREACHABLE - 1)
                .unwrap()
                .adjusted(1)
                .is_err()
        );
        assert!(PathPrice::finite(1).unwrap().adjusted(i64::MAX).is_err());
        assert!(PathPrice::UNREACHABLE.adjusted(-1).is_err());
        assert!(
            PathCell::ZERO
                .with_price(PathPrice(UNREACHABLE + 1))
                .is_err()
        );
    }
    #[test]
    fn copied_updates_preserve_unconsumed_fields_and_explicit_unreachable_state() {
        let original = PathCell::from_words([42, 7, 4, 2, 1, 4, 8]).unwrap();
        let unreachable = original.with_price(PathPrice::UNREACHABLE).unwrap();
        assert_eq!(unreachable.words(), [UNREACHABLE, 7, 4, 2, 1, 4, 8]);
        assert!(unreachable.forward().is_err());
        assert_eq!(original.without_match().words(), [42, 7, 0, 2, 1, 4, 8]);
        assert_eq!(
            original.with_literals(0).unwrap().words(),
            [42, 7, 4, 0, 1, 4, 8]
        );
        assert!(PathCell::ZERO.with_match(0, 3).is_err());
        assert!(PathCell::ZERO.with_match(1, 2).is_err());
        assert!(PathCell::ZERO.with_literals(BLOCK_LIMIT + 1).is_err());
        assert!(
            PathCell::ZERO
                .with_repeats([0, BLOCK_LIMIT + 1, 0])
                .is_err()
        );
        assert_eq!(
            PathCell::ZERO
                .forward()
                .unwrap()
                .predecessor(PathIndex::new(0).unwrap()),
            Ok(None)
        );
    }
    #[test]
    fn checked_predecessors_decrease_and_long_matches_do_not_bound_slots() {
        let matched = PathCell::ZERO
            .with_match(4, 4)
            .unwrap()
            .with_literals(2)
            .unwrap()
            .forward()
            .unwrap();
        assert_eq!(
            matched
                .predecessor(PathIndex::new(6).unwrap())
                .unwrap()
                .unwrap()
                .get(),
            0
        );
        assert!(matched.predecessor(PathIndex::new(5).unwrap()).is_err());
        assert!(PathIndex::new(PATH_CAPACITY).is_err());
        assert!(PathIndex::new(usize::MAX).is_err());
        assert!(ReverseCursor::new(PathIndex::new(PATH_CAPACITY - 2).unwrap()).is_err());
        assert_eq!(
            ReverseCursor::new(PathIndex::new(PATH_HORIZON).unwrap())
                .unwrap()
                .end()
                .get(),
            4098
        );
        let long = PathCell::ZERO
            .with_match(4, BLOCK_LIMIT - 1)
            .unwrap()
            .with_literals(1)
            .unwrap();
        assert_eq!(
            long.reverse().end(0, BLOCK_LIMIT as usize),
            Ok(BLOCK_LIMIT as usize)
        );
        assert!(long.reverse().end(1, BLOCK_LIMIT as usize).is_err());
    }
    #[test]
    fn every_captured_cell_and_reconstruction_coordinate_matches_checked_records() {
        let cases = fixture::read(fixture::PACK).unwrap();
        let mut reconstructed = 0;
        for case in cases {
            let mut cells = vec![PathCell::ZERO; case.path_cells];
            let mut cursor = None;
            for event in case.events {
                for (index, words) in event.path {
                    let cell = PathCell::from_words(words).unwrap();
                    assert_eq!(cell.words(), words);
                    cells[index] = cell;
                }
                if let Some(words) = event.stretch {
                    assert_eq!(PathCell::from_words(words).unwrap().words(), words);
                }
                if event.stage == 7 {
                    let [start, end, at] = event.reverse.unwrap();
                    let current = ReverseCursor::new(PathIndex::new(at as usize).unwrap()).unwrap();
                    assert_eq!(
                        [current.destination().get(), current.end().get()],
                        [start as usize, end as usize]
                    );
                    cursor = Some(current);
                }
                if event.stage == 8 {
                    let current = cursor.unwrap();
                    let [start, end, at] = event.reverse.unwrap();
                    assert_eq!(
                        [
                            current.destination().get(),
                            current.end().get(),
                            current.predecessor().get()
                        ],
                        [start as usize, end as usize, at as usize]
                    );
                    assert_eq!(
                        cells[start as usize].literal_length(),
                        cells[at as usize].literal_length()
                    );
                    cursor = current
                        .advance(cells[at as usize].forward().unwrap())
                        .unwrap();
                    reconstructed += 1;
                }
            }
        }
        assert!(reconstructed >= 42);
    }
}
