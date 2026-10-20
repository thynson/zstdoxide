//! Private copied Ultra2 lifecycle values; no public strategy admission or allocation.
//! Upstream counterparts: ZSTD_compressBlock_btultra2 and ZSTD_initStats_ultra,
//! facebook/zstd v1.5.7, revision f8745da6ff1ad1e7bab384bd1f9d742439278e99.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Private two-pass composition is qualified incrementally"
)]
use crate::optimal_frequencies::CELLS;
use crate::optimal_matches::MATCH_CAPACITY;
use crate::optimal_path_layout::{PATH_CAPACITY, PathCell};
use crate::optimal_search_layout::{OptimalMatch, OptimalSearchLayout};
use crate::sequence_context::SequenceWithOffset;
use crate::{Error, ReferenceStrategy, ResolvedCompressionParameters, Result};
use core::alloc::Layout;

/// Source positions omit the native bias of two. Stale cells stay integers;
/// only this checked current interval can map them to borrowed source bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Ultra2Coordinates {
    origin: usize,
    length: usize,
    end: usize,
}
impl Ultra2Coordinates {
    pub(crate) fn new(origin: usize, length: usize) -> Result<Self> {
        let end = origin.checked_add(length).ok_or(Error::InvalidParameter)?;
        if length > 131072 || end > (u32::MAX - 2) as usize {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            origin,
            length,
            end,
        })
    }
    pub(crate) const fn origin(self) -> usize {
        self.origin
    }
    pub(crate) const fn len(self) -> usize {
        self.length
    }
    pub(crate) const fn end(self) -> usize {
        self.end
    }
    pub(crate) fn absolute(self, offset: usize) -> Result<usize> {
        if offset > self.length {
            return Err(Error::InvalidParameter);
        }
        self.origin
            .checked_add(offset)
            .ok_or(Error::InvalidParameter)
    }
    pub(crate) fn offset(self, index: usize, width: usize) -> Result<usize> {
        let offset = index
            .checked_sub(self.origin)
            .ok_or(Error::InvalidParameter)?;
        if width > self.length || offset > self.length - width {
            return Err(Error::InvalidParameter);
        }
        Ok(offset)
    }
    pub(crate) fn earlier(self, index: usize, current: usize, width: usize) -> Result<usize> {
        self.offset(current, width)?;
        let offset = self.offset(index, width)?;
        if index >= current {
            return Err(Error::InvalidParameter);
        }
        Ok(offset)
    }
    pub(crate) fn rebase(self) -> Result<Self> {
        Self::new(self.end, self.length)
    }
}

/// The five upstream predicates are separate from the helper's update-frontier
/// precondition. These copied records do not authorize nonfresh parser state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Ultra2Eligibility {
    pub(crate) literal_length_sum: u32,
    pub(crate) sequence_count: usize,
    pub(crate) dictionary_limit: usize,
    pub(crate) low_limit: usize,
    pub(crate) current: usize,
    pub(crate) source_len: usize,
    pub(crate) next_to_update: usize,
}
impl Ultra2Eligibility {
    pub(crate) fn predicates(self) -> Result<[bool; 5]> {
        Ultra2Coordinates::new(self.current, self.source_len)?;
        if self.low_limit > self.dictionary_limit
            || self.dictionary_limit > self.current
            || self.next_to_update > (u32::MAX - 2) as usize
            || self.sequence_count > u32::MAX as usize
        {
            return Err(Error::InvalidParameter);
        }
        Ok([
            self.literal_length_sum == 0,
            self.sequence_count == 0,
            self.dictionary_limit == self.low_limit,
            self.current == self.dictionary_limit,
            self.source_len > 8,
        ])
    }
    pub(crate) fn eligible(self) -> Result<bool> {
        Ok(self.predicates()?.into_iter().all(|p| p))
    }
    pub(crate) fn rebased(self) -> Result<Ultra2Coordinates> {
        if !self.eligible()? {
            return Err(Error::InvalidState);
        }
        let dictionary =
            u32::try_from(self.dictionary_limit).map_err(|_| Error::InvalidParameter)?;
        let frontier = u32::try_from(self.next_to_update).map_err(|_| Error::InvalidParameter)?;
        if dictionary.wrapping_sub(frontier) > 1 {
            return Err(Error::InvalidState);
        }
        Ultra2Coordinates::new(self.current, self.source_len)?.rebase()
    }
}

/// A failed operation requires whole-owner reset, never a phase retry. Plain
/// value states have the same representation in safe and opt-in unsafe builds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Ultra2Phase {
    Ready,
    Seeding,
    Rebased,
    Encoding,
    Complete,
    Failed,
}
impl Ultra2Phase {
    pub(crate) fn advance(self, next: Self) -> Result<Self> {
        use Ultra2Phase::*;
        if matches!(
            (self, next),
            (Ready, Seeding | Encoding)
                | (Seeding, Rebased)
                | (Rebased, Encoding)
                | (Encoding, Complete)
                | (Ready | Seeding | Rebased | Encoding, Failed)
        ) {
            Ok(next)
        } else {
            Err(Error::InvalidState)
        }
    }
}

/// Nine existing storage owners, no frame owner or entropy publication. Private
/// threshold controls do not widen the ordinary Optimal/Ultra plan contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Ultra2ExecutionPlan {
    parameters: ResolvedCompressionParameters,
    layout: OptimalSearchLayout,
    coordinates: Ultra2Coordinates,
    seed: bool,
    disabled_literals: bool,
    owner_bytes: [usize; 9],
}
impl Ultra2ExecutionPlan {
    pub(crate) fn new(
        parameters: ResolvedCompressionParameters,
        source_len: usize,
        seed: bool,
        disabled_literals: bool,
    ) -> Result<Self> {
        if parameters.strategy() != ReferenceStrategy::Ultra2 {
            return Err(Error::UnsupportedFeature);
        }
        if !(7..=16384).contains(&source_len)
            || !(10..=16).contains(&parameters.window_log())
            || !(6..=12).contains(&parameters.hash_log())
            || !(6..=12).contains(&parameters.chain_log())
            || !(1..=6).contains(&parameters.search_log())
            || parameters.target_length() > 4095
            || source_len > 1usize << parameters.window_log()
        {
            return Err(Error::ResourceLimit);
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
        let coordinates = Ultra2Coordinates::new(0, source_len)?;
        coordinates.rebase()?;
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
            bytes::<usize>(layout.short_log().map_or(0, |log| 1usize << log))?,
            bytes::<OptimalMatch>(MATCH_CAPACITY)?,
            bytes::<u8>(source_len)?,
            bytes::<SequenceWithOffset>(source_len / 3)?,
        ];
        owner_bytes.iter().try_fold(0usize, |sum, &v| {
            sum.checked_add(v).ok_or(Error::ResourceLimit)
        })?;
        Ok(Self {
            parameters,
            layout,
            coordinates,
            seed,
            disabled_literals,
            owner_bytes,
        })
    }
    pub(crate) const fn parameters(self) -> ResolvedCompressionParameters {
        self.parameters
    }
    pub(crate) const fn layout(self) -> OptimalSearchLayout {
        self.layout
    }
    pub(crate) const fn coordinates(self) -> Ultra2Coordinates {
        self.coordinates
    }
    pub(crate) const fn source_len(self) -> usize {
        self.coordinates.len()
    }
    pub(crate) const fn disabled_literals(self) -> bool {
        self.disabled_literals
    }
    pub(crate) const fn owner_bytes(self) -> [usize; 9] {
        self.owner_bytes
    }
    pub(crate) const fn entry(self) -> Ultra2Eligibility {
        Ultra2Eligibility {
            literal_length_sum: 0,
            sequence_count: if self.seed { 0 } else { 1 },
            dictionary_limit: 0,
            low_limit: 0,
            current: 0,
            source_len: self.source_len(),
            next_to_update: 0,
        }
    }
    pub(crate) fn seeds(self) -> Result<bool> {
        self.entry().eligible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RawCompressionParameters;
    fn parameters() -> RawCompressionParameters {
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
    fn all_five_predicates_and_separate_helper_frontier_contract() {
        for mask in 0..32 {
            let entry = Ultra2Eligibility {
                literal_length_sum: u32::from(mask & 1 == 0),
                sequence_count: usize::from(mask & 2 == 0),
                dictionary_limit: 10,
                low_limit: if mask & 4 != 0 { 10 } else { 9 },
                current: if mask & 8 != 0 { 10 } else { 11 },
                source_len: if mask & 16 != 0 { 9 } else { 8 },
                next_to_update: 10,
            };
            assert_eq!(
                entry.predicates().unwrap(),
                core::array::from_fn(|i| mask & (1 << i) != 0)
            );
            assert_eq!(entry.eligible().unwrap(), mask == 31);
            assert_eq!(entry.rebased().is_ok(), mask == 31);
        }
        let entry = Ultra2Eligibility {
            literal_length_sum: 0,
            sequence_count: 0,
            dictionary_limit: 10,
            low_limit: 10,
            current: 10,
            source_len: 64,
            next_to_update: 10,
        };
        for frontier in [9, 10] {
            assert_eq!(
                Ultra2Eligibility {
                    next_to_update: frontier,
                    ..entry
                }
                .rebased()
                .unwrap()
                .origin(),
                74
            );
        }
        for frontier in [0, 8, 11, usize::MAX] {
            let bad = Ultra2Eligibility {
                next_to_update: frontier,
                ..entry
            };
            if frontier != usize::MAX {
                assert!(bad.eligible().unwrap());
            }
            assert!(bad.rebased().is_err());
        }
        assert!(
            Ultra2Eligibility {
                low_limit: 11,
                ..entry
            }
            .eligible()
            .is_err()
        );
        assert!(
            Ultra2Eligibility {
                current: 9,
                ..entry
            }
            .eligible()
            .is_err()
        );
    }
    #[test]
    fn checked_interval_edges_overflow_and_stale_ring_aliases() {
        for length in [0, 7, 8, 9, 64, 65, 16384, 131072] {
            let first = Ultra2Coordinates::new(0, length).unwrap();
            let second = first.rebase().unwrap();
            assert_eq!(first.absolute(length), Ok(length));
            assert_eq!(first.offset(length, 0), Ok(length));
            assert!(first.offset(length, 1).is_err());
            assert!(first.absolute(length + 1).is_err());
            assert_eq!(second.origin(), length);
            assert_eq!(second.end(), 2 * length);
            for old in 0..length {
                assert!(second.offset(old, 1).is_err());
            }
            if length > 1 {
                assert_eq!(second.earlier(length, length + 1, 1), Ok(0));
            }
            assert!(second.earlier(length, length, 0).is_err());
        }
        for chain in 6..=12 {
            let length = 1usize << (chain - 1);
            let mask = length - 1;
            let second = Ultra2Coordinates::new(0, length).unwrap().rebase().unwrap();
            assert_eq!((1 + 2) & mask, (length + 1 + 2) & mask);
            assert!(second.offset(1, 1).is_err());
            assert_eq!(second.offset(length + 1, 1), Ok(1));
        }
        let limit = (u32::MAX - 2) as usize;
        let last = Ultra2Coordinates::new(limit - 8, 8).unwrap();
        assert_eq!(last.offset(limit, 0), Ok(8));
        assert!(last.rebase().is_err());
        for (origin, length) in [(usize::MAX, 1), (limit, 1), (0, 131073)] {
            assert!(Ultra2Coordinates::new(origin, length).is_err());
        }
    }
    #[test]
    fn phase_graph_cannot_retry_publish_seed_or_reuse_failure() {
        use Ultra2Phase::*;
        for (from, to) in [
            (Ready, Complete),
            (Seeding, Encoding),
            (Seeding, Complete),
            (Rebased, Seeding),
            (Encoding, Seeding),
            (Complete, Ready),
            (Failed, Ready),
            (Failed, Encoding),
        ] {
            assert_eq!(from.advance(to), Err(Error::InvalidState));
        }
        for phase in [Ready, Seeding, Rebased, Encoding] {
            let failed = phase.advance(Failed).unwrap();
            assert!(failed.advance(phase).is_err());
        }
        assert_eq!(
            Ready
                .advance(Seeding)
                .unwrap()
                .advance(Rebased)
                .unwrap()
                .advance(Encoding)
                .unwrap()
                .advance(Complete),
            Ok(Complete)
        );
        assert_eq!(
            Ready.advance(Encoding).unwrap().advance(Complete),
            Ok(Complete)
        );
    }
    #[test]
    fn all_native_requests_have_exact_checked_geometry_and_nine_owner_sizes() {
        for case in crate::ultra2_fixture::read(crate::ultra2_fixture::PACK).unwrap() {
            let raw = RawCompressionParameters {
                window_log: u32::from(case.window),
                hash_log: u32::from(case.hash),
                chain_log: u32::from(case.chain),
                search_log: u32::from(case.search),
                min_match: case.minimum,
                target_length: case.target,
                strategy: 9,
            };
            let plan = Ultra2ExecutionPlan::new(
                raw.validate().unwrap(),
                case.source.len(),
                case.requested_seed,
                case.literal_mode == 1,
            )
            .unwrap();
            assert_eq!(plan.seeds().unwrap(), case.phases.len() == 2);
            assert_eq!(plan.source_len(), case.source.len());
            assert_eq!(plan.layout().tree().key().cells(), 1 << case.hash);
            assert_eq!(
                plan.entry().predicates().unwrap(),
                core::array::from_fn(|i| case.eligibility[i] != 0)
            );
            assert_eq!(
                plan.owner_bytes(),
                [
                    377 * 4,
                    PATH_CAPACITY * core::mem::size_of::<PathCell>(),
                    MATCH_CAPACITY * core::mem::size_of::<OptimalMatch>(),
                    (1 << case.hash) * core::mem::size_of::<usize>(),
                    (1 << case.chain) * core::mem::size_of::<usize>(),
                    case.short_cells() * core::mem::size_of::<usize>(),
                    MATCH_CAPACITY * core::mem::size_of::<OptimalMatch>(),
                    case.source.len(),
                    case.source.len() / 3 * core::mem::size_of::<SequenceWithOffset>()
                ]
            );
        }
    }
    #[test]
    fn private_plan_rejects_unqualified_strategy_source_and_geometry() {
        for length in [0, 6, 16385, usize::MAX] {
            assert_eq!(
                Ultra2ExecutionPlan::new(parameters().validate().unwrap(), length, true, false),
                Err(Error::ResourceLimit)
            );
        }
        for strategy in 1..9 {
            let mut raw = parameters();
            raw.strategy = strategy;
            assert_eq!(
                Ultra2ExecutionPlan::new(raw.validate().unwrap(), 64, true, false),
                Err(Error::UnsupportedFeature)
            );
        }
        for field in 0..6 {
            let mut raw = parameters();
            match field {
                0 => raw.window_log = 17,
                1 => raw.hash_log = 13,
                2 => raw.chain_log = 13,
                3 => raw.search_log = 7,
                4 => raw.target_length = 4096,
                _ => raw.window_log = 13,
            };
            assert_eq!(
                Ultra2ExecutionPlan::new(raw.validate().unwrap(), 16384, true, false),
                Err(Error::ResourceLimit)
            );
        }
    }
}
