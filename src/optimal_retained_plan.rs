//! Checked copied schedules and exact owner/output bounds for retained Optimal.
//! The private lifecycle follows ZSTD_buildSeqStore and ordinary block publication.
#![forbid(unsafe_code)]
#![allow(dead_code, reason = "Retained complete-frame admission remains gated")]

use crate::optimal_frequencies::CELLS;
use crate::optimal_matches::MATCH_CAPACITY;
use crate::optimal_path_layout::{PATH_CAPACITY, PathCell};
use crate::optimal_search_layout::{OptimalMatch, OptimalSearchLayout};
use crate::reference_frame::{ReferenceFrameHeader, ReferenceFrameSettings};
use crate::sequence_context::SequenceWithOffset;
use crate::{Error, ReferenceStrategy, ResolvedCompressionParameters, Result};
use core::alloc::Layout;

/// Plain values shared by every storage backend; no allocation or unsafe variant
/// is needed. Each owner count belongs to one independent parser/search/token owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetainedOptimalPlan {
    parameters: ResolvedCompressionParameters,
    layout: OptimalSearchLayout,
    settings: ReferenceFrameSettings,
    blocks: [usize; 4],
    count: usize,
    maximum: usize,
    disabled_literals: bool,
    owner_bytes: [usize; 9],
    frame_bound: usize,
}

impl RetainedOptimalPlan {
    /// Private observed schedules, including the literal-disabled parser controls.
    /// No owner is cloned or reserved before the complete contract is checked.
    pub(crate) fn new(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
        blocks: &[usize],
        disabled_literals: bool,
    ) -> Result<Self> {
        if parameters.strategy() != ReferenceStrategy::Optimal {
            return Err(Error::UnsupportedFeature);
        }
        if !(128..=32768).contains(&source_len)
            || parameters.window_log() > 16
            || parameters.hash_log() > 12
            || parameters.chain_log() > 12
            || parameters.search_log() > 6
            || parameters.target_length() > 4095
        {
            return Err(Error::ResourceLimit);
        }
        if !(2..=4).contains(&blocks.len()) || blocks.iter().any(|n| !(64..=8192).contains(n)) {
            return Err(Error::InvalidParameter);
        }
        let total = blocks.iter().try_fold(0_usize, |total, &length| {
            total.checked_add(length).ok_or(Error::InvalidParameter)
        })?;
        if total != source_len {
            return Err(Error::InvalidParameter);
        }
        let layout = OptimalSearchLayout::new(
            parameters.window_log(),
            parameters.hash_log(),
            parameters.chain_log(),
            parameters.search_log(),
            parameters.min_match(),
            parameters.target_length() as usize,
            (parameters.min_match() == 3).then_some(parameters.window_log()),
        )?;
        layout
            .validate_retained(source_len)
            .map_err(|_| Error::ResourceLimit)?;
        let maximum = blocks
            .iter()
            .copied()
            .max()
            .ok_or(Error::InvalidParameter)?;
        fn bytes<T>(count: usize) -> Result<usize> {
            Layout::array::<T>(count)
                .map(|l| l.size())
                .map_err(|_| Error::ResourceLimit)
        }
        let owner_bytes = [
            bytes::<u32>(CELLS)?,
            bytes::<PathCell>(PATH_CAPACITY)?,
            bytes::<OptimalMatch>(MATCH_CAPACITY)?,
            bytes::<usize>(layout.tree().key().cells())?,
            bytes::<usize>(
                layout
                    .tree()
                    .nodes()
                    .checked_mul(2)
                    .ok_or(Error::ResourceLimit)?,
            )?,
            bytes::<usize>(layout.short_log().map_or(0, |log| 1_usize << log))?,
            bytes::<OptimalMatch>(MATCH_CAPACITY)?,
            bytes::<u8>(maximum)?,
            bytes::<SequenceWithOffset>(maximum / 3)?,
        ];
        owner_bytes.iter().try_fold(0_usize, |sum, &n| {
            sum.checked_add(n).ok_or(Error::ResourceLimit)
        })?;
        let settings = ReferenceFrameSettings::new(source_len, parameters.window_log())?;
        // This schedule has more headers than the window's constant block limit.
        // The known-size frame has neither dictionary ID nor checksum trailer.
        let frame_bound = source_len
            .checked_add(ReferenceFrameHeader::new(&settings).as_bytes().len())
            .and_then(|n| n.checked_add(3 * blocks.len()))
            .ok_or(Error::ResourceLimit)?;
        bytes::<u8>(frame_bound)?;
        let mut schedule = [0; 4];
        schedule[..blocks.len()].copy_from_slice(blocks);
        Ok(Self {
            parameters,
            layout,
            settings,
            blocks: schedule,
            count: blocks.len(),
            maximum,
            disabled_literals,
            owner_bytes,
            frame_bound,
        })
    }

    /// Proposed additive frame envelope: preserve all old fresh source sizes,
    /// choose balanced blocks and force automatic literal prices.
    pub(crate) fn for_frame(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
    ) -> Result<Self> {
        if !(16385..=32768).contains(&source_len) {
            return Err(Error::ResourceLimit);
        }
        let count = source_len.div_ceil(8192);
        let quotient = source_len / count;
        let remainder = source_len % count;
        let mut blocks = [0; 4];
        for (index, length) in blocks[..count].iter_mut().enumerate() {
            *length = quotient + usize::from(index < remainder);
        }
        Self::new(parameters, source_len, &blocks[..count], false)
    }

    pub(crate) const fn parameters(self) -> ResolvedCompressionParameters {
        self.parameters
    }
    pub(crate) const fn layout(self) -> OptimalSearchLayout {
        self.layout
    }
    pub(crate) const fn source_len(self) -> usize {
        self.settings.source_len()
    }
    pub(crate) const fn maximum_block_len(self) -> usize {
        self.maximum
    }
    pub(crate) const fn disabled_literals(self) -> bool {
        self.disabled_literals
    }
    pub(crate) const fn owner_bytes(self) -> [usize; 9] {
        self.owner_bytes
    }
    pub(crate) const fn frame_bound(self) -> usize {
        self.frame_bound
    }
    pub(crate) fn blocks(&self) -> &[usize] {
        &self.blocks[..self.count]
    }

    /// Only exact not-yet-completed schedule boundaries identify a next block.
    pub(crate) fn next_block_len(self, emitted: usize) -> Result<usize> {
        let mut position = 0_usize;
        for &length in self.blocks() {
            if emitted == position {
                return Ok(length);
            }
            position = position.checked_add(length).ok_or(Error::CorruptData)?;
        }
        Err(Error::InvalidParameter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RawCompressionParameters;
    use crate::optimal_retained_frames_fixture as fixture;
    use alloc::vec::Vec;
    fn raw() -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: 16,
            hash_log: 12,
            chain_log: 12,
            search_log: 6,
            min_match: 3,
            target_length: 4095,
            strategy: 7,
        }
    }

    #[test]
    fn every_additive_length_has_balanced_blocks_and_exact_raw_reservation() {
        for source_len in 16385..=32768 {
            let plan =
                RetainedOptimalPlan::for_frame(raw().validate().unwrap(), source_len).unwrap();
            let blocks = plan.blocks();
            assert_eq!(blocks.len(), source_len.div_ceil(8192));
            assert_eq!(blocks.iter().sum::<usize>(), source_len);
            assert!(blocks.iter().all(|n| (64..=8192).contains(n)));
            assert!(
                blocks
                    .windows(2)
                    .all(|pair| pair[0] >= pair[1] && pair[0] - pair[1] <= 1)
            );
            let mut offset = 0;
            for &length in blocks {
                assert_eq!(plan.next_block_len(offset).unwrap(), length);
                assert_eq!(
                    plan.next_block_len(offset + 1),
                    Err(Error::InvalidParameter)
                );
                offset += length;
            }
            assert_eq!(
                plan.next_block_len(source_len),
                Err(Error::InvalidParameter)
            );
            assert_eq!(plan.frame_bound(), 7 + source_len + 3 * blocks.len());
            assert_eq!(plan.owner_bytes()[7], plan.maximum_block_len());
            assert_eq!(
                plan.owner_bytes()[8],
                plan.maximum_block_len() / 3 * core::mem::size_of::<SequenceWithOffset>()
            );
            assert!(!plan.disabled_literals());
        }
        for (n, expected) in [
            (16385, &[5462, 5462, 5461][..]),
            (24576, &[8192, 8192, 8192][..]),
            (24577, &[6145, 6144, 6144, 6144][..]),
            (32768, &[8192, 8192, 8192, 8192][..]),
        ] {
            assert_eq!(
                RetainedOptimalPlan::for_frame(raw().validate().unwrap(), n)
                    .unwrap()
                    .blocks(),
                expected
            );
        }
    }

    #[test]
    fn every_native_schedule_and_price_policy_has_the_same_seven_storage_owners() {
        for case in fixture::read(fixture::PACK).unwrap() {
            let parameters = RawCompressionParameters {
                window_log: case.window.into(),
                hash_log: case.hash.into(),
                chain_log: case.chain.into(),
                search_log: case.search.into(),
                min_match: case.minimum,
                target_length: case.target,
                strategy: 7,
            }
            .validate()
            .unwrap();
            let blocks: Vec<_> = case.blocks.iter().map(|b| b.length).collect();
            let plan = RetainedOptimalPlan::new(
                parameters,
                case.source.len(),
                &blocks,
                case.literal_mode == 1,
            )
            .unwrap();
            assert_eq!(plan.blocks(), blocks);
            assert_eq!(plan.source_len(), case.source.len());
            assert_eq!(plan.disabled_literals(), case.literal_mode == 1);
            assert_eq!(
                plan.owner_bytes()[5],
                case.short_cells() * core::mem::size_of::<usize>()
            );
            let fresh = crate::optimal_plan::OptimalExecutionPlan::from_resolved_parameters(
                parameters,
                plan.maximum_block_len(),
            )
            .unwrap();
            assert_eq!(plan.owner_bytes()[..7], fresh.owner_bytes()[..7]);
        }
    }

    #[test]
    fn schedules_geometry_and_fresh_sizes_reject_before_allocation() {
        let parameters = raw().validate().unwrap();
        for n in [0, 127, 128, 16384, 32769, usize::MAX] {
            assert_eq!(
                RetainedOptimalPlan::for_frame(parameters, n),
                Err(Error::ResourceLimit)
            );
        }
        for blocks in [
            &[][..],
            &[32768],
            &[8192, 8192],
            &[8193, 8191, 8192, 8192],
            &[8192, 8192, 8192, 8191],
            &[8192, 8192, 8192, 8128, 64],
            &[usize::MAX, 64],
        ] {
            assert_eq!(
                RetainedOptimalPlan::new(parameters, 32768, blocks, false),
                Err(Error::InvalidParameter)
            );
        }
        for (field, value) in [(0, 14), (0, 17), (1, 13), (2, 13), (3, 7), (4, 4096)] {
            let mut p = raw();
            match field {
                0 => p.window_log = value,
                1 => p.hash_log = value,
                2 => p.chain_log = value,
                3 => p.search_log = value,
                _ => p.target_length = value,
            }
            assert_eq!(
                RetainedOptimalPlan::for_frame(p.validate().unwrap(), 32768),
                Err(Error::ResourceLimit)
            );
        }
        for strategy in [6, 8, 9] {
            let mut p = raw();
            p.strategy = strategy;
            assert_eq!(
                RetainedOptimalPlan::for_frame(p.validate().unwrap(), 32768),
                Err(Error::UnsupportedFeature)
            );
        }
    }
}
