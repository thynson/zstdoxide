//! Checked retained Ultra2 schedules, first-block bias and owner/output bounds.
//! The private lifecycle follows ZSTD_buildSeqStore, ZSTD_initStats_ultra and
//! ZSTD_compressBlock_btultra2 in pinned facebook/zstd v1.5.7.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Retained Ultra2 execution and public admission remain separately gated"
)]

use crate::optimal_frequencies::CELLS;
use crate::optimal_matches::MATCH_CAPACITY;
use crate::optimal_path_layout::{PATH_CAPACITY, PathCell};
use crate::optimal_search_layout::{OptimalMatch, OptimalSearchLayout};
use crate::reference_frame::{ReferenceFrameHeader, ReferenceFrameSettings};
use crate::sequence_context::SequenceWithOffset;
use crate::ultra2_plan::Ultra2Coordinates;
use crate::{Error, ReferenceStrategy, ResolvedCompressionParameters, Result};
use core::alloc::Layout;

/// Plain values shared by every storage backend; no allocation or unsafe variant
/// is needed. Each owner count belongs to one independent parser/search/token owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetainedUltra2Plan {
    parameters: ResolvedCompressionParameters,
    rebased: Ultra2Coordinates,
    layout: OptimalSearchLayout,
    settings: ReferenceFrameSettings,
    blocks: [usize; 4],
    count: usize,
    maximum: usize,
    disabled_literals: bool,
    owner_bytes: [usize; 9],
    frame_bound: usize,
}

impl RetainedUltra2Plan {
    /// Private observed schedules, including the literal-disabled parser controls.
    /// No owner is cloned or reserved before the complete contract is checked.
    pub(crate) fn new(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
        blocks: &[usize],
        disabled_literals: bool,
    ) -> Result<Self> {
        if parameters.strategy() != ReferenceStrategy::Ultra2 {
            return Err(Error::UnsupportedFeature);
        }
        if !(14..=32768).contains(&source_len)
            || parameters.window_log() > 16
            || parameters.hash_log() > 12
            || parameters.chain_log() > 12
            || parameters.search_log() > 6
            || parameters.target_length() > 4095
        {
            return Err(Error::ResourceLimit);
        }
        if !(2..=4).contains(&blocks.len()) || blocks.iter().any(|n| !(7..=8192).contains(n)) {
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
            .validate_retained_ultra2(source_len)
            .map_err(|_| Error::ResourceLimit)?;
        let rebased =
            Ultra2Coordinates::new(if blocks[0] > 8 { blocks[0] } else { 0 }, source_len)?;
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
            rebased,
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

    /// The first actual block alone determines bootstrap bias; the complete
    /// frame source length remains the physical window span. Skipped bootstrap
    /// retains origin zero.
    pub(crate) const fn coordinates_after_seed(self) -> Ultra2Coordinates {
        self.rebased
    }
    pub(crate) fn first_block_len(self) -> usize {
        self.blocks[0]
    }
    pub(crate) fn seeds_first_block(self) -> bool {
        self.first_block_len() > 8
    }
    /// Checked prefix coordinates never expose future source bytes. Only exact
    /// completed schedule boundaries can form a visible borrowed source view.
    pub(crate) fn visible_coordinates(
        self,
        consumed: usize,
        after_seed: bool,
    ) -> Result<Ultra2Coordinates> {
        let mut total = 0usize;
        let mut complete = false;
        for &length in self.blocks() {
            total = total.checked_add(length).ok_or(Error::InvalidParameter)?;
            complete |= total == consumed;
        }
        if !complete || (after_seed && !self.seeds_first_block()) {
            return Err(Error::InvalidParameter);
        }
        if !after_seed && self.seeds_first_block() && consumed != self.first_block_len() {
            return Err(Error::InvalidState);
        }
        Ultra2Coordinates::new(
            if after_seed {
                self.first_block_len()
            } else {
                0
            },
            consumed,
        )
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
    use crate::ultra2_retained_fixture as fixture;
    use alloc::vec::Vec;
    fn raw() -> RawCompressionParameters {
        RawCompressionParameters {
            window_log: 16,
            hash_log: 12,
            chain_log: 12,
            search_log: 6,
            min_match: 3,
            target_length: 4095,
            strategy: 9,
        }
    }
    #[test]
    fn every_additive_length_has_balanced_blocks_exact_reservation_and_first_block_bias() {
        for n in 16385..=32768 {
            let plan = RetainedUltra2Plan::for_frame(raw().validate().unwrap(), n).unwrap();
            assert_eq!(plan.blocks().len(), n.div_ceil(8192));
            assert_eq!(plan.blocks().iter().sum::<usize>(), n);
            assert!(plan.blocks().iter().all(|v| (7..=8192).contains(v)));
            assert!(
                plan.blocks()
                    .windows(2)
                    .all(|v| v[0] >= v[1] && v[0] - v[1] <= 1)
            );
            assert!(plan.seeds_first_block());
            assert_eq!(plan.coordinates_after_seed().origin(), plan.blocks()[0]);
            assert_eq!(plan.coordinates_after_seed().end(), plan.blocks()[0] + n);
            assert!(plan.coordinates_after_seed().end() <= 40960);
            assert_eq!(plan.frame_bound(), 7 + n + 3 * plan.blocks().len());
            let mut consumed = 0;
            for &length in plan.blocks() {
                assert_eq!(plan.next_block_len(consumed), Ok(length));
                assert_eq!(
                    plan.next_block_len(consumed + 1),
                    Err(Error::InvalidParameter)
                );
                consumed += length;
                let view = plan.visible_coordinates(consumed, true).unwrap();
                assert_eq!(view.origin(), plan.first_block_len());
                assert_eq!(view.len(), consumed);
                assert_eq!(view.offset(view.end(), 1), Err(Error::InvalidParameter));
            }
            assert_eq!(plan.next_block_len(n), Err(Error::InvalidParameter));
            assert!(!plan.disabled_literals());
        }
        for (n, expected) in [
            (16385, &[5462, 5462, 5461][..]),
            (24576, &[8192, 8192, 8192][..]),
            (24577, &[6145, 6144, 6144, 6144][..]),
            (32768, &[8192, 8192, 8192, 8192][..]),
        ] {
            assert_eq!(
                RetainedUltra2Plan::for_frame(raw().validate().unwrap(), n)
                    .unwrap()
                    .blocks(),
                expected
            );
        }
    }
    #[test]
    fn every_native_schedule_has_exact_existing_owner_bounds_and_phase_coordinates() {
        for case in fixture::read(fixture::PACK).unwrap() {
            let p = RawCompressionParameters {
                window_log: case.window.into(),
                hash_log: case.hash.into(),
                chain_log: case.chain.into(),
                search_log: case.search.into(),
                min_match: case.minimum,
                target_length: case.target,
                strategy: 9,
            }
            .validate()
            .unwrap();
            let blocks: Vec<_> = case.blocks.iter().map(|b| b.length).collect();
            let plan =
                RetainedUltra2Plan::new(p, case.source.len(), &blocks, case.literal_mode == 1)
                    .unwrap();
            assert_eq!(plan.blocks(), blocks);
            assert_eq!(plan.source_len(), case.source.len());
            assert_eq!(
                plan.owner_bytes()[5],
                case.short_cells() * core::mem::size_of::<usize>()
            );
            let fresh = crate::ultra2_plan::Ultra2ExecutionPlan::new(
                p,
                plan.maximum_block_len(),
                true,
                case.literal_mode == 1,
            )
            .unwrap();
            assert_eq!(plan.owner_bytes(), fresh.owner_bytes());
            let mut consumed = 0;
            for block in &case.blocks {
                consumed += block.length;
                for phase in &block.phases {
                    let after_seed = plan.seeds_first_block() && !phase.seeding;
                    let coordinates = plan.visible_coordinates(consumed, after_seed).unwrap();
                    assert_eq!(coordinates.origin() as u32 + 2, phase.origin);
                    assert_eq!(coordinates.len(), consumed);
                }
            }
        }
    }
    #[test]
    fn skipped_seed_and_non_power_of_two_biases_bound_visible_source_prefixes() {
        for first in [7, 8, 9, 65, 95, 97, 8192] {
            let lengths = [first, 97];
            let n = first + 97;
            let p = RetainedUltra2Plan::new(raw().validate().unwrap(), n, &lengths, false).unwrap();
            let seeded = first > 8;
            assert_eq!(p.seeds_first_block(), seeded);
            let initial = p.visible_coordinates(first, false).unwrap();
            assert_eq!((initial.origin(), initial.len()), (0, first));
            let encoding = p.visible_coordinates(first, seeded).unwrap();
            let later = p.visible_coordinates(n, seeded).unwrap();
            let bias = if seeded { first } else { 0 };
            assert_eq!((encoding.origin(), later.origin()), (bias, bias));
            assert_eq!(later.end(), bias + n);
            assert_eq!(encoding.offset(bias, first), Ok(0));
            assert_eq!(
                encoding.offset(bias + first, 1),
                Err(Error::InvalidParameter)
            );
            if seeded {
                assert_eq!(encoding.offset(bias - 1, 1), Err(Error::InvalidParameter));
                assert_eq!(p.visible_coordinates(n, false), Err(Error::InvalidState));
            } else {
                assert_eq!(p.visible_coordinates(n, true), Err(Error::InvalidParameter));
            }
            for bad in [0, first - 1, first + 1, n - 1, n + 1, usize::MAX] {
                assert_eq!(
                    p.visible_coordinates(bad, seeded),
                    Err(Error::InvalidParameter)
                );
            }
        }
    }
    #[test]
    fn dedicated_layout_does_not_widen_old_fresh_or_retained_validation() {
        let layout = OptimalSearchLayout::new(16, 12, 12, 6, 3, 4095, Some(16)).unwrap();
        for n in [14, 15, 16, 64, 127] {
            assert_eq!(layout.validate_retained_ultra2(n), Ok(()));
            assert_eq!(layout.validate_retained(n), Err(Error::InvalidParameter));
        }
        for n in [16385, 32768] {
            assert_eq!(layout.validate_retained_ultra2(n), Ok(()));
            assert_eq!(layout.validate_ultra2(n), Err(Error::InvalidParameter));
        }
        for n in [0, 13, 32769, usize::MAX] {
            assert_eq!(
                layout.validate_retained_ultra2(n),
                Err(Error::InvalidParameter)
            );
        }
        let small = OptimalSearchLayout::new(10, 6, 6, 1, 4, 0, None).unwrap();
        assert_eq!(small.validate_retained_ultra2(1024), Ok(()));
        assert_eq!(
            small.validate_retained_ultra2(1025),
            Err(Error::InvalidParameter)
        );
    }
    #[test]
    fn invalid_schedules_geometry_strategies_and_old_fresh_sizes_reject() {
        let parameters = raw().validate().unwrap();
        for n in [0, 13, 14, 128, 16384, 32769, usize::MAX] {
            assert_eq!(
                RetainedUltra2Plan::for_frame(parameters, n),
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
            &[usize::MAX, 7],
        ] {
            assert_eq!(
                RetainedUltra2Plan::new(parameters, 32768, blocks, false),
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
            };
            assert_eq!(
                RetainedUltra2Plan::for_frame(p.validate().unwrap(), 32768),
                Err(Error::ResourceLimit)
            );
        }
        for strategy in [6, 7, 8] {
            let mut p = raw();
            p.strategy = strategy;
            assert_eq!(
                RetainedUltra2Plan::for_frame(p.validate().unwrap(), 32768),
                Err(Error::UnsupportedFeature)
            );
        }
    }
}
