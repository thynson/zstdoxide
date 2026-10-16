//! Checked token collection for one complete, qualified Optimal block.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Internal state diagnostics share the checked composition"
    )
)]
use crate::optimal_frequencies::OptimalFrequencies;
use crate::optimal_parse::{OptimalParser, PathEvent, PathObserver};
use crate::optimal_paths::OptimalPaths;
use crate::optimal_plan::OptimalExecutionPlan;
use crate::optimal_search::OptimalSearch;
use crate::optimal_search_layout::OptimalMatch;
use crate::sequence_context::{LogicalSequence, SequenceWithOffset};
use crate::sequence_plan::{OffsetIntent, repeated_offset};
use crate::{Error, Result};
use alloc::vec::Vec;
use core::alloc::Allocator;

/// All nine allocator identities are preserved. The borrowed source is never
/// copied; scratch owns only literal bytes and checked wire-intent records.
pub(crate) struct OptimalMatchState<'a, A: Allocator> {
    source: &'a [u8],
    plan: OptimalExecutionPlan,
    parser: OptimalParser<'a, A, A, A>,
    search: OptimalSearch<'a, A, A, A, A>,
    literals: Vec<u8, A>,
    sequences: Vec<SequenceWithOffset, A>,
    finished: bool,
    failed: bool,
}
impl<'a, A: Allocator> OptimalMatchState<'a, A> {
    /// Validate complete source identity and effective parser configuration before
    /// clearing scratch. This constructor neither allocates nor clones owners.
    pub(crate) fn from_parts(
        source: &'a [u8],
        plan: OptimalExecutionPlan,
        parser: OptimalParser<'a, A, A, A>,
        search: OptimalSearch<'a, A, A, A, A>,
        mut literals: Vec<u8, A>,
        mut sequences: Vec<SequenceWithOffset, A>,
    ) -> Result<Self> {
        let (original, minimum, target, disabled) = parser.fresh_configuration()?;
        if !core::ptr::eq(original, source)
            || source.len() != plan.source_len()
            || search.layout() != plan.layout()
            || minimum
                != if plan.parameters().min_match() == 3 {
                    3
                } else {
                    4
                }
            || target != plan.parameters().target_length() as usize
            || disabled
        {
            return Err(Error::InvalidParameter);
        }
        search.validate_fresh_start(source, minimum, target)?;
        literals.clear();
        sequences.clear();
        Ok(Self {
            source,
            plan,
            parser,
            search,
            literals,
            sequences,
            finished: false,
            failed: false,
        })
    }
    /// Only the complete source and initial wire repeats are qualified. Argument
    /// rejection is transactional; failures after reservation begins poison it.
    pub(crate) fn parse_next(&mut self, length: usize, committed: [u32; 3]) -> Result<bool> {
        if self.failed || self.finished {
            return Err(Error::InvalidState);
        }
        if length != self.source.len() || committed != [1, 4, 8] {
            return Err(Error::InvalidParameter);
        }
        self.failed = true;
        self.literals
            .try_reserve_exact(self.source.len())
            .map_err(|_| Error::AllocationFailed)?;
        self.sequences
            .try_reserve_exact(self.source.len() / 3)
            .map_err(|_| Error::AllocationFailed)?;
        let mut collect = Collect {
            source: self.source,
            literals: &mut self.literals,
            sequences: &mut self.sequences,
            anchor: 0,
            repeats: committed,
        };
        let result = self.parser.parse_fresh(&mut self.search, &mut collect)?;
        if result.sequences != collect.sequences.len()
            || result.repeats != collect.repeats
            || result.tail != self.source.len() - collect.anchor
        {
            return Err(Error::CorruptData);
        }
        collect.literal_tail()?;
        self.finished = true;
        self.failed = false;
        Ok(true)
    }
    pub(crate) fn literals(&self) -> &[u8] {
        &self.literals
    }
    pub(crate) fn sequences(&self) -> &[SequenceWithOffset] {
        &self.sequences
    }
    #[cfg(test)]
    pub(crate) fn reset(&mut self) -> Result<()> {
        self.failed = true;
        self.parser.reset()?;
        self.search.reset();
        self.literals.clear();
        self.sequences.clear();
        self.finished = false;
        self.failed = false;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        self.plan.owner_bytes()[..7].iter().sum::<usize>()
            + self.literals.capacity()
            + self.sequences.capacity() * core::mem::size_of::<SequenceWithOffset>()
    }
}

struct Collect<'s, 'v, A: Allocator> {
    source: &'s [u8],
    literals: &'v mut Vec<u8, A>,
    sequences: &'v mut Vec<SequenceWithOffset, A>,
    anchor: usize,
    repeats: [u32; 3],
}
impl<A: Allocator> Collect<'_, '_, A> {
    fn append(&mut self, [anchor, literal_length, match_length, off_base]: [u32; 4]) -> Result<()> {
        if anchor as usize != self.anchor || match_length < 3 || off_base == 0 {
            return Err(Error::CorruptData);
        }
        let position = self
            .anchor
            .checked_add(literal_length as usize)
            .ok_or(Error::CorruptData)?;
        let end = position
            .checked_add(match_length as usize)
            .filter(|&v| v <= self.source.len())
            .ok_or(Error::CorruptData)?;
        let (intent, distance) = match off_base {
            1 => (
                OffsetIntent::Repeat1,
                repeated_offset(self.repeats, 1, literal_length == 0)
                    .ok_or(Error::CorruptData)?
                    .0,
            ),
            2 => (
                OffsetIntent::Repeat2,
                repeated_offset(self.repeats, 2, literal_length == 0)
                    .ok_or(Error::CorruptData)?
                    .0,
            ),
            3 => (
                OffsetIntent::Repeat3,
                repeated_offset(self.repeats, 3, literal_length == 0)
                    .ok_or(Error::CorruptData)?
                    .0,
            ),
            _ => (OffsetIntent::Explicit, off_base - 3),
        };
        let reference = position
            .checked_sub(distance as usize)
            .ok_or(Error::CorruptData)?;
        if self.source.get(position..end) != self.source.get(reference..end - distance as usize) {
            return Err(Error::CorruptData);
        }
        let resolved = intent.resolve(literal_length, distance, self.repeats)?;
        if resolved.offset_value != off_base
            || self.sequences.len() >= self.sequences.capacity()
            || literal_length as usize > self.literals.capacity() - self.literals.len()
        {
            return Err(Error::CorruptData);
        }
        self.literals.extend_from_slice(
            self.source
                .get(self.anchor..position)
                .ok_or(Error::CorruptData)?,
        );
        self.sequences.push(SequenceWithOffset {
            sequence: LogicalSequence {
                literal_length,
                match_length,
                distance,
            },
            offset: intent,
        });
        self.anchor = end;
        self.repeats = resolved.next_repeat;
        Ok(())
    }
    fn literal_tail(&mut self) -> Result<()> {
        let tail = self.source.get(self.anchor..).ok_or(Error::CorruptData)?;
        if tail.len() > self.literals.capacity() - self.literals.len() {
            return Err(Error::CorruptData);
        }
        self.literals.extend_from_slice(tail);
        Ok(())
    }
}
impl<A: Allocator> PathObserver for Collect<'_, '_, A> {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        _: &OptimalPaths<P>,
        _: &OptimalFrequencies<F>,
        _: &[OptimalMatch],
    ) -> Result<()> {
        if let Some(row) = event.appended {
            self.append(row)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RawCompressionParameters;
    use crate::optimal_fresh_wide_fixture as fixture;
    use crate::optimal_matches::OptimalMatches;
    use crate::optimal_short_key::OptimalShortKey;
    use crate::reference_tree::ReferenceTree;
    use alloc::{alloc::Global, vec};
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    fn plan(size: usize, minimum: u32) -> OptimalExecutionPlan {
        OptimalExecutionPlan::from_resolved_parameters(
            RawCompressionParameters {
                window_log: 16,
                hash_log: 12,
                chain_log: 12,
                search_log: 6,
                min_match: minimum,
                target_length: 4095,
                strategy: 7,
            }
            .validate()
            .unwrap(),
            size,
        )
        .unwrap()
    }
    fn state<A: Allocator>(
        source: &[u8],
        p: OptimalExecutionPlan,
        owners: [A; 9],
    ) -> Result<OptimalMatchState<'_, A>> {
        let [f, path, m, h, n, short, candidates, literals, sequences] = owners;
        let parser = OptimalParser::from_parts(
            source,
            p.parameters().min_match(),
            p.parameters().target_length() as usize,
            OptimalFrequencies::new_in(false, f)?,
            OptimalPaths::new_in(path)?,
            OptimalMatches::new_in(m)?,
        )?;
        let l = p.layout();
        let search = OptimalSearch::from_parts(
            source,
            l,
            ReferenceTree::new_in(l.tree(), h, n)?,
            OptimalShortKey::new_in(l.short_log(), short)?,
            OptimalMatches::new_in(candidates)?,
        )?;
        OptimalMatchState::from_parts(
            source,
            p,
            parser,
            search,
            Vec::new_in(literals),
            Vec::new_in(sequences),
        )
    }
    #[test]
    fn complete_c_tokens_keep_exact_wire_intents_and_reconstruct_sources() {
        let mut total = 0;
        for case in fixture::read(fixture::PACK)
            .unwrap()
            .into_iter()
            .filter(|c| c.literal_mode != 1)
        {
            let p = OptimalExecutionPlan::from_resolved_parameters(
                RawCompressionParameters {
                    window_log: u32::from(case.window),
                    hash_log: u32::from(case.hash),
                    chain_log: u32::from(case.chain),
                    search_log: u32::from(case.search),
                    min_match: case.minimum,
                    target_length: case.target,
                    strategy: 7,
                }
                .validate()
                .unwrap(),
                case.source.len(),
            )
            .unwrap();
            let mut state = state(case.source, p, [Global; 9]).unwrap();
            assert!(state.parse_next(case.source.len(), [1, 4, 8]).unwrap());
            let expected = case.blocks[0]
                .events
                .iter()
                .filter_map(|event| event.appended.first())
                .collect::<Vec<_>>();
            assert_eq!(state.sequences().len(), expected.len(), "{}", case.name);
            let mut decoded = Vec::new();
            let mut at = 0;
            let mut repeats = [1, 4, 8];
            for (actual, row) in state.sequences().iter().zip(expected) {
                let seq = actual.sequence;
                assert_eq!([seq.literal_length, seq.match_length], [row[1], row[2]]);
                let resolved = actual
                    .offset
                    .resolve(seq.literal_length, seq.distance, repeats)
                    .unwrap();
                assert_eq!(resolved.offset_value, row[3]);
                repeats = resolved.next_repeat;
                let end = at + seq.literal_length as usize;
                decoded.extend_from_slice(&state.literals()[at..end]);
                at = end;
                for _ in 0..seq.match_length {
                    let value = decoded[decoded.len() - seq.distance as usize];
                    decoded.push(value);
                }
                total += 1;
            }
            decoded.extend_from_slice(&state.literals()[at..]);
            assert_eq!(decoded, case.source, "{}", case.name);
            assert_eq!(repeats, case.blocks[0].candidate);
            assert_eq!(
                state.parse_next(case.source.len(), [1, 4, 8]),
                Err(Error::InvalidState)
            );
        }
        assert!(total >= 24);
    }
    #[test]
    fn moved_nonclone_owners_release_every_failed_construction_and_reservation() {
        let source = [b'a'; 16384];
        for failed in 0..=9 {
            let owners = core::array::from_fn(|i| {
                TrackingAllocator::new(AllocatorConfig {
                    max_live_allocations: 1,
                    fail_at: if i == failed {
                        core::num::NonZeroU64::new(1)
                    } else {
                        None
                    },
                })
                .unwrap()
                .into_non_clone()
            });
            let monitors = owners.each_ref().map(|a| a.monitor());
            let result = state(&source, plan(source.len(), 3), owners);
            if failed < 7 {
                assert!(matches!(result, Err(Error::AllocationFailed)));
            } else {
                let mut state = result.unwrap();
                let result = state.parse_next(source.len(), [1, 4, 8]);
                if failed < 9 {
                    assert_eq!(result, Err(Error::AllocationFailed));
                    assert_eq!(
                        state.parse_next(source.len(), [1, 4, 8]),
                        Err(Error::InvalidState)
                    );
                    state.reset().unwrap();
                    state.parse_next(source.len(), [1, 4, 8]).unwrap();
                } else {
                    assert_eq!(result, Ok(true));
                }
                drop(state);
            }
            for m in monitors {
                m.assert_released();
                assert_eq!(m.snapshot().owners_dropped, 1);
            }
        }
    }
    #[test]
    fn borrowed_owners_reuse_capacities_and_reject_partial_or_changed_repeats() {
        let source = [b'a'; 16384];
        let owners: [_; 9] = core::array::from_fn(|_| {
            TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 1,
                fail_at: None,
            })
            .unwrap()
            .into_non_clone()
        });
        let monitors = owners.each_ref().map(|a| a.monitor());
        let mut state = state(&source, plan(source.len(), 3), owners.each_ref()).unwrap();
        for (length, repeats) in [
            (0, [1, 4, 8]),
            (8192, [1, 4, 8]),
            (16385, [1, 4, 8]),
            (16384, [1, 1, 1]),
            (16384, [0, 4, 8]),
        ] {
            assert_eq!(
                state.parse_next(length, repeats),
                Err(Error::InvalidParameter)
            );
        }
        state.parse_next(source.len(), [1, 4, 8]).unwrap();
        let before = monitors.each_ref().map(|a| a.snapshot().attempts);
        assert_eq!(before, [1; 9]);
        let expected = state.sequences().to_vec();
        let capacity = state.retained_capacity();
        for _ in 0..3 {
            state.reset().unwrap();
            state.parse_next(source.len(), [1, 4, 8]).unwrap();
            assert_eq!(state.sequences(), expected);
            assert_eq!(state.retained_capacity(), capacity);
        }
        assert_eq!(monitors.each_ref().map(|a| a.snapshot().attempts), before);
        drop(state);
        for m in &monitors {
            m.assert_released();
            assert_eq!(m.snapshot().owners_dropped, 0);
        }
        drop(owners);
        for m in monitors {
            assert_eq!(m.snapshot().owners_dropped, 1);
        }
    }
    #[test]
    fn malformed_appends_do_not_publish_literals_sequences_or_repeats() {
        let source = [b'a'; 64];
        let mut literals = Vec::with_capacity(64);
        let mut sequences = Vec::with_capacity(21);
        let mut collect = Collect {
            source: &source,
            literals: &mut literals,
            sequences: &mut sequences,
            anchor: 0,
            repeats: [1, 4, 8],
        };
        for row in [
            [1, 1, 3, 4],
            [0, 0, 3, 4],
            [0, 1, 2, 4],
            [0, 1, 64, 4],
            [0, u32::MAX, 3, 4],
            [0, 1, 3, 0],
            [0, 1, 3, u32::MAX],
            [0, 0, 3, 3],
        ] {
            assert_eq!(collect.append(row), Err(Error::CorruptData));
            assert!(collect.literals.is_empty());
            assert!(collect.sequences.is_empty());
            assert_eq!(collect.repeats, [1, 4, 8]);
            assert_eq!(collect.anchor, 0);
        }
        collect.append([0, 1, 3, 4]).unwrap();
        assert_eq!(collect.anchor, 4);
        let mut changed = vec![b'a'; 64];
        changed[2] = b'b';
        let mut l = Vec::with_capacity(64);
        let mut s = Vec::with_capacity(21);
        let mut bad = Collect {
            source: &changed,
            literals: &mut l,
            sequences: &mut s,
            anchor: 0,
            repeats: [1, 4, 8],
        };
        assert_eq!(bad.append([0, 1, 3, 4]), Err(Error::CorruptData));
        assert!(bad.sequences.is_empty());
    }
    #[test]
    fn composition_rejects_foreign_sources_policies_geometry_and_used_owners() {
        fn rebind<'a>(
            original: OptimalMatchState<'a, Global>,
            source: &'a [u8],
            p: OptimalExecutionPlan,
        ) -> Result<OptimalMatchState<'a, Global>> {
            OptimalMatchState::from_parts(
                source,
                p,
                original.parser,
                original.search,
                original.literals,
                original.sequences,
            )
        }
        let source = [b'a'; 64];
        let other = source;
        let p = plan(64, 3);
        assert!(matches!(
            rebind(state(&source, p, [Global; 9]).unwrap(), &other, p),
            Err(Error::InvalidParameter)
        ));
        for wrong in [plan(128, 3), plan(64, 4)] {
            assert!(matches!(
                rebind(state(&source, p, [Global; 9]).unwrap(), &source, wrong),
                Err(Error::InvalidParameter)
            ));
        }
        for (disabled, target) in [(true, 4095), (false, 1)] {
            let mut original = state(&source, p, [Global; 9]).unwrap();
            original.parser = OptimalParser::from_parts(
                &source,
                3,
                target,
                OptimalFrequencies::new_in(disabled, Global).unwrap(),
                OptimalPaths::new_in(Global).unwrap(),
                OptimalMatches::new_in(Global).unwrap(),
            )
            .unwrap();
            assert!(matches!(
                rebind(original, &source, p),
                Err(Error::InvalidParameter)
            ));
        }
        let mut original = state(&source, p, [Global; 9]).unwrap();
        original
            .search
            .query_fresh(crate::optimal_search_layout::OptimalQuery {
                position: 1,
                repeats: [1, 4, 8],
                zero_literals: false,
                length_to_beat: 3,
            })
            .unwrap();
        assert!(matches!(
            rebind(original, &source, p),
            Err(Error::InvalidState)
        ));
        let mut original = state(&source, p, [Global; 9]).unwrap();
        original.parse_next(64, [1, 4, 8]).unwrap();
        assert!(matches!(
            rebind(original, &source, p),
            Err(Error::InvalidState)
        ));
    }
}
