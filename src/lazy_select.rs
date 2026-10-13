//! Copied selection records for the no-dictionary v1.5.7 lazy family.
//! These values introduce no allocation, aliasing or unsafe backend.
#![forbid(unsafe_code)]

use crate::greedy_block::GreedyBlock;
use crate::greedy_search::GreedyMatch;
use crate::sequence_plan::OffsetIntent;
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LazyDepth {
    Greedy,
    One,
    Two,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LazyCandidate {
    position: usize,
    found: GreedyMatch,
    intent: OffsetIntent,
    offbase: u32,
}

impl LazyCandidate {
    /// Bounds-check a match already compared by the selected source search.
    /// This validates geometry, not byte equality; emission remains checked.
    pub(crate) fn new(
        block: GreedyBlock<'_>,
        position: usize,
        found: GreedyMatch,
        intent: OffsetIntent,
    ) -> Result<Self> {
        let distance = position
            .checked_sub(found.previous)
            .ok_or(Error::CorruptData)?;
        if position < block.range().start
            || distance == 0
            || distance > block.window_size()
            || found.previous < block.prefix_floor()
            || found.length < 4
            || found.length
                > block
                    .range()
                    .end
                    .checked_sub(position)
                    .ok_or(Error::CorruptData)?
        {
            return Err(Error::CorruptData);
        }
        let offbase = match intent {
            OffsetIntent::Repeat1 => 1,
            OffsetIntent::Explicit => u32::try_from(distance)
                .ok()
                .and_then(|d| d.checked_add(3))
                .ok_or(Error::ResourceLimit)?,
            _ => return Err(Error::InvalidParameter),
        };
        Ok(Self {
            position,
            found,
            intent,
            offbase,
        })
    }

    pub(crate) const fn parts(self) -> (usize, GreedyMatch, OffsetIntent) {
        (self.position, self.found, self.intent)
    }

    fn score(self, weight: i32) -> Result<i32> {
        // Positive offbase and bounded source extents are constructor invariants.
        // Signed arithmetic is intentional: short distant matches can score < 0.
        let length = i32::try_from(self.found.length).map_err(|_| Error::ResourceLimit)?;
        length
            .checked_mul(weight)
            .and_then(|s| s.checked_sub(self.offbase.ilog2() as i32))
            .ok_or(Error::ResourceLimit)
    }
}

/// Exact gains in ZSTD_compressBlock_lazy_generic, not a generic price model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LazyGain {
    RepeatOne,
    ExplicitOne,
    RepeatTwo,
    ExplicitTwo,
}

impl LazyGain {
    pub(crate) fn improves(
        self,
        candidate: LazyCandidate,
        incumbent: LazyCandidate,
    ) -> Result<bool> {
        let (weight, penalty, intent) = match self {
            Self::RepeatOne => (3, 1, OffsetIntent::Repeat1),
            Self::ExplicitOne => (4, 4, OffsetIntent::Explicit),
            Self::RepeatTwo => (4, 1, OffsetIntent::Repeat1),
            Self::ExplicitTwo => (4, 7, OffsetIntent::Explicit),
        };
        if candidate.intent != intent {
            return Err(Error::InvalidParameter);
        }
        let threshold = incumbent
            .score(weight)?
            .checked_add(penalty)
            .ok_or(Error::ResourceLimit)?;
        Ok(candidate.score(weight)? > threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::greedy_key::GreedyKeyLayout;
    use alloc::vec;

    #[test]
    fn frozen_thresholds_reject_below_and_equal_and_accept_above() {
        let source = vec![0; 4096];
        let block = GreedyBlock::new(
            &source,
            2048..4096,
            0,
            GreedyKeyLayout::new(12, 4).unwrap(),
            4096,
            false,
        )
        .unwrap();
        for (gain, old_off, new_off, equal_len, intent) in [
            (LazyGain::ExplicitOne, 2048, 128, 8, OffsetIntent::Explicit),
            (LazyGain::ExplicitTwo, 2048, 256, 9, OffsetIntent::Explicit),
            (LazyGain::RepeatOne, 1024, 1, 5, OffsetIntent::Repeat1),
            (LazyGain::RepeatTwo, 512, 1, 6, OffsetIntent::Repeat1),
        ] {
            let old = LazyCandidate::new(
                block,
                2048,
                GreedyMatch {
                    previous: 2048 - (old_off - 3),
                    length: 8,
                },
                OffsetIntent::Explicit,
            )
            .unwrap();
            for length in equal_len - 1..=equal_len + 1 {
                let distance = if intent == OffsetIntent::Repeat1 {
                    1
                } else {
                    new_off - 3
                };
                let new = LazyCandidate::new(
                    block,
                    2049,
                    GreedyMatch {
                        previous: 2049 - distance,
                        length,
                    },
                    intent,
                )
                .unwrap();
                assert_eq!(gain.improves(new, old), Ok(length > equal_len));
            }
        }
    }

    #[test]
    fn maximum_geometry_and_negative_scores_are_checked_without_sentinels() {
        let source = vec![0; 2_097_152 + 131_072];
        let block = GreedyBlock::new(
            &source,
            2_097_152..source.len(),
            0,
            GreedyKeyLayout::new(19, 6).unwrap(),
            2_097_152,
            false,
        )
        .unwrap();
        let short = LazyCandidate::new(
            block,
            2_097_152,
            GreedyMatch {
                previous: 0,
                length: 4,
            },
            OffsetIntent::Explicit,
        )
        .unwrap();
        assert_eq!(short.score(3), Ok(-9));
        let long = LazyCandidate::new(
            block,
            2_097_152,
            GreedyMatch {
                previous: 0,
                length: 131_072,
            },
            OffsetIntent::Explicit,
        )
        .unwrap();
        assert_eq!(long.score(4), Ok(524_267));
        for (position, previous, length) in [
            (2_097_151, 0, 4),
            (2_097_152, 2_097_152, 4),
            (2_097_153, 0, 4),
            (2_097_152, 0, 3),
            (2_097_152, 0, 131_073),
            (usize::MAX, 0, 4),
        ] {
            assert!(
                LazyCandidate::new(
                    block,
                    position,
                    GreedyMatch { previous, length },
                    OffsetIntent::Explicit
                )
                .is_err()
            );
        }
        assert_eq!(
            LazyCandidate::new(
                block,
                2_097_152,
                GreedyMatch {
                    previous: 0,
                    length: 4
                },
                OffsetIntent::Repeat2
            ),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            LazyGain::RepeatOne.improves(short, long),
            Err(Error::InvalidParameter)
        );
    }
}
