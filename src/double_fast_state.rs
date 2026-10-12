//! Persistent independent long/short tables for one contiguous DoubleFast source.
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "DoubleFast frame integration follows")
)]

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::double_fast_block::DoubleFastBlock;
use crate::double_fast_parse::{DoubleFastParseResult, parse_block};
use crate::double_fast_plan::DoubleFastExecutionPlan;
use crate::double_fast_tables::DoubleFastTables;
use crate::sequence_context::SequenceWithOffset;
use crate::{Error, Result};

/// Search progress is separate from the caller's committed entropy state.
///
/// All positions index this single borrowed source. Raw/RLE selection retains
/// both sampled tables; the caller supplies only published wire repeats on each
/// call. This composite adds no unchecked operation; both table leaves use the
/// existing independently selected `unsafe-match-table` representation.
/// A failed parse requires dropping this state, never retrying partial sampling.
/// Corresponds to the no-dictionary match/window portion of `ZSTD_CCtx`.
pub(crate) struct DoubleFastMatchState<'a, A: Allocator> {
    source: &'a [u8],
    plan: DoubleFastExecutionPlan,
    tables: DoubleFastTables<A>,
    literals: Vec<u8, A>,
    sequences: Vec<SequenceWithOffset, A>,
    position: usize,
    prefix_floor: usize,
    failed: bool,
}

impl<'a, A: Allocator> DoubleFastMatchState<'a, A> {
    /// Bind complete owners to one source, invalidating earlier sampled indices.
    /// Does not clone an allocator, allocate, or borrow temporary source storage.
    pub(crate) fn from_parts(
        source: &'a [u8],
        plan: DoubleFastExecutionPlan,
        mut tables: DoubleFastTables<A>,
        mut literals: Vec<u8, A>,
        mut sequences: Vec<SequenceWithOffset, A>,
    ) -> Result<Self> {
        if source.len() != plan.source_len()
            || tables.layouts() != (plan.long_layout(), plan.short_layout())
        {
            return Err(Error::InvalidParameter);
        }
        tables.reset();
        literals.clear();
        sequences.clear();
        Ok(Self {
            source,
            plan,
            tables,
            literals,
            sequences,
            position: 0,
            prefix_floor: 0,
            failed: false,
        })
    }

    /// Parse the next bounded contiguous extent without publishing its repeats.
    ///
    /// Empty continuations leave state unchanged. Lengths one through six retain
    /// stale scratch exactly as the ordinary C sequence-store early return does.
    /// A caller must emit/select this block before requesting another block.
    /// Argument errors precede mutation; a parser error makes all later calls
    /// fail with InvalidState, including empty continuations.
    /// Corresponds to `ZSTD_window_enforceMaxDist` at the ordinary block start
    /// and `ZSTD_buildSeqStore` in the pinned frame compression loop.
    pub(crate) fn parse_next(
        &mut self,
        length: usize,
        committed_repeats: [u32; 3],
    ) -> Result<Option<DoubleFastParseResult>> {
        if self.failed {
            return Err(Error::InvalidState);
        }
        let end = self
            .position
            .checked_add(length)
            .filter(|&end| end <= self.source.len())
            .ok_or(Error::InvalidParameter)?;
        if length > self.plan.block_limit() || committed_repeats.contains(&0) {
            return Err(Error::InvalidParameter);
        }
        if length == 0 {
            return Ok(None);
        }
        let floor = self
            .prefix_floor
            .max(self.position.saturating_sub(self.plan.window_size()));
        let candidate = if length < 7 {
            None
        } else {
            let block = DoubleFastBlock::new(
                self.source,
                self.position..end,
                floor,
                self.plan.long_layout(),
                self.plan.short_layout(),
                self.plan.window_size(),
            )?;
            match parse_block(
                block,
                &mut self.tables,
                committed_repeats,
                &mut self.literals,
                &mut self.sequences,
            ) {
                Ok(result) => Some(result),
                Err(error) => {
                    self.failed = true;
                    return Err(error);
                }
            }
        };
        self.prefix_floor = floor;
        self.position = end;
        Ok(candidate)
    }

    pub(crate) fn literals(&self) -> &[u8] {
        &self.literals
    }

    pub(crate) fn sequences(&self) -> &[SequenceWithOffset] {
        &self.sequences
    }

    /// Owned payload capacities, excluding the borrowed source and inline state.
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        self.tables.retained_capacity()
            + self.literals.capacity()
            + self.sequences.capacity() * core::mem::size_of::<SequenceWithOffset>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{alloc::Global, vec};
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    use crate::CompressionParameterRequest;

    fn plan(length: usize, window_log: u8) -> DoubleFastExecutionPlan {
        let mut request = CompressionParameterRequest::with_level(3);
        request.set_window_log(window_log).unwrap();
        request.set_hash_log(10).unwrap();
        request.set_chain_log(9).unwrap();
        request.set_min_match(4).unwrap();
        DoubleFastExecutionPlan::new(&request, length).unwrap()
    }

    fn fresh(source: &[u8], window_log: u8) -> DoubleFastMatchState<'_, Global> {
        let plan = plan(source.len(), window_log);
        DoubleFastMatchState::from_parts(
            source,
            plan,
            DoubleFastTables::new_in(plan.long_layout(), plan.short_layout(), Global, Global)
                .unwrap(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap()
    }

    fn noise(mut state: u32, length: usize) -> Vec<u8> {
        (0..length)
            .map(|_| {
                state ^= state.wrapping_shl(13);
                state ^= state >> 17;
                state ^= state.wrapping_shl(5);
                state as u8
            })
            .collect()
    }

    #[test]
    fn all_frozen_ordinary_candidates_follow_c_publication_without_reseeding() {
        use crate::RawCompressionParameters;

        fn raw_index(position: usize) -> u32 {
            if position == usize::MAX {
                0
            } else {
                u32::try_from(position.checked_add(2).unwrap()).unwrap()
            }
        }
        fn table_hash(values: &[usize]) -> u64 {
            values
                .iter()
                .flat_map(|&p| raw_index(p).to_le_bytes())
                .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
                })
        }

        let cases = crate::double_fast_frame_oracle_fixture::cases().unwrap();
        assert_eq!(cases.len(), 51);
        let mut blocks = 0;
        let mut fresh = 0;
        let mut meaningful_raw_rollback = 0;
        let mut meaningful_rle_rollback = 0;
        for case in cases {
            // The internal C begin call applies these explicit values directly.
            // Public known-size request resolution is a separate tested contract.
            let parameters = RawCompressionParameters {
                window_log: u32::from(case.window_log),
                chain_log: u32::from(case.short_log),
                hash_log: u32::from(case.long_log),
                search_log: u32::from(case.search_log),
                min_match: u32::from(case.min_match),
                target_length: case.target_length,
                strategy: 2,
            }
            .validate()
            .unwrap();
            let plan =
                DoubleFastExecutionPlan::from_observed_parameters(parameters, case.source.len())
                    .unwrap();
            let mut state = DoubleFastMatchState::from_parts(
                case.source,
                plan,
                DoubleFastTables::new_in(plan.long_layout(), plan.short_layout(), Global, Global)
                    .unwrap(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
            let mut expected_long = vec![0_u32; plan.long_layout().bucket_count()];
            let mut expected_short = vec![0_u32; plan.short_layout().bucket_count()];
            let mut committed = [1, 4, 8];
            let mut observations = case.blocks.iter();
            for length in &case.schedule {
                if *length == 0 {
                    let before = state.position;
                    assert_eq!(state.parse_next(0, committed), Ok(None));
                    assert_eq!(state.position, before);
                    continue;
                }
                let expected = observations.next().unwrap();
                assert_eq!(state.position, expected.start, "{}", case.name);
                assert_eq!(committed, expected.wire_before, "{}", case.name);
                assert_eq!(
                    table_hash(state.tables.long.test_positions()),
                    expected.long_table.hashes[0]
                );
                assert_eq!(
                    table_hash(state.tables.short.test_positions()),
                    expected.short_table.hashes[0]
                );
                let actual = state.parse_next(*length, committed).unwrap();
                assert_eq!(state.prefix_floor, expected.prefix_floor, "{}", case.name);
                match (actual, &expected.candidate) {
                    (Some(actual), Some(candidate)) => {
                        fresh += 1;
                        assert_eq!(state.literals(), candidate.literals, "{}", case.name);
                        assert_eq!(
                            state.sequences().len(),
                            candidate.sequences.len(),
                            "{}",
                            case.name
                        );
                        let mut trial = committed;
                        for (got, want) in state.sequences().iter().zip(&candidate.sequences) {
                            let sequence = got.sequence;
                            let resolved = got
                                .offset
                                .resolve(sequence.literal_length, sequence.distance, trial)
                                .unwrap();
                            assert_eq!(
                                (
                                    sequence.literal_length,
                                    sequence.match_length,
                                    sequence.distance,
                                    resolved.offset_value
                                ),
                                (
                                    want.literal_length,
                                    want.match_length,
                                    want.distance,
                                    want.offset_value
                                ),
                                "{}",
                                case.name,
                            );
                            trial = resolved.next_repeat;
                        }
                        assert_eq!(actual.trailing_literals, candidate.trailing_literals);
                        assert_eq!(actual.parser_repeats[..2], candidate.parser_repeats[..2]);
                        // DoubleFast never consumes the raw third C cell. Rust carries
                        // the real incoming wire third, and updates the full wire
                        // triple independently for compressed publication.
                        assert_eq!(actual.parser_repeats[2], committed[2]);
                        assert_eq!(actual.wire_repeats, candidate.wire_repeats);
                        assert_eq!(trial, actual.wire_repeats);
                        if expected.kind == 2 {
                            committed = trial;
                        } else if trial != committed {
                            meaningful_raw_rollback += usize::from(expected.kind == 0);
                            meaningful_rle_rollback += usize::from(expected.kind == 1);
                        }
                    }
                    (None, None) => assert!(*length < 7),
                    _ => panic!("{} fresh/stale candidate mismatch", case.name),
                }
                assert_eq!(committed, expected.wire_after, "{}", case.name);
                for (observed, expected_table, actual) in [
                    (
                        &expected.long_table,
                        &mut expected_long,
                        state.tables.long.test_positions(),
                    ),
                    (
                        &expected.short_table,
                        &mut expected_short,
                        state.tables.short.test_positions(),
                    ),
                ] {
                    for &[bucket, old, new] in &observed.changes {
                        assert_eq!(expected_table[bucket as usize], old);
                        expected_table[bucket as usize] = new;
                    }
                    assert!(
                        actual
                            .iter()
                            .zip(expected_table)
                            .all(|(&got, want)| raw_index(got) == *want),
                        "{} complete sampled table",
                        case.name
                    );
                    assert_eq!(table_hash(actual), observed.hashes[1]);
                }
                blocks += 1;
            }
            assert!(observations.next().is_none());
            assert_eq!(state.position, case.source.len());
        }
        assert_eq!((blocks, fresh), (160, 148));
        assert_eq!(meaningful_raw_rollback, 13);
        assert_eq!(meaningful_rle_rollback, 3);
    }

    #[test]
    fn raw_publication_keeps_search_history_without_publishing_candidate_repeats() {
        let prefix = noise(0x1234_5678, 1024);
        let input = [prefix.as_slice(), prefix.as_slice()].concat();
        let mut state = fresh(&input, 12);
        state.parse_next(1024, [1, 4, 8]).unwrap().unwrap();
        assert!(state.sequences().is_empty());
        assert!(state.tables.long.test_positions().iter().any(|&p| p < 1024));
        assert!(
            state
                .tables
                .short
                .test_positions()
                .iter()
                .any(|&p| p < 1024)
        );
        // Emulate raw selection: retain sampled cells, reuse committed repeats.
        let second = state.parse_next(1024, [1, 4, 8]).unwrap().unwrap();
        assert!(
            state
                .sequences()
                .iter()
                .any(|s| { s.sequence.distance == 1024 && s.sequence.match_length > 900 })
        );
        assert_eq!(second.wire_repeats, [1024, 1, 4]);
        assert_eq!(state.position, input.len());
        assert_eq!(state.prefix_floor, 0);
    }

    #[test]
    fn retained_floor_uses_block_start_and_does_not_clear_expired_cells() {
        let input = noise(0x4567_89ab, 3072);
        let mut state = fresh(&input, 10);
        for expected_floor in [0, 0, 1024] {
            state.parse_next(1024, [1, 4, 8]).unwrap().unwrap();
            assert_eq!(state.prefix_floor, expected_floor);
        }
        // A stale sampled cell remains storage, not permission to read a match.
        assert!(state.tables.long.test_positions().iter().any(|&p| p < 1024));
        assert!(
            state
                .tables
                .short
                .test_positions()
                .iter()
                .any(|&p| p < 1024)
        );
        assert!(
            state
                .sequences()
                .iter()
                .all(|s| s.sequence.distance <= 1024)
        );
    }

    #[test]
    fn tiny_and_empty_extents_preserve_stale_scratch_and_sampled_table() {
        let input = vec![b'x'; 147];
        let mut state = fresh(&input, 10);
        let first = state.parse_next(128, [1, 4, 8]).unwrap().unwrap();
        let before = (
            (
                state.tables.long.test_positions().to_vec(),
                state.tables.short.test_positions().to_vec(),
            ),
            state.literals().to_vec(),
            state.sequences().to_vec(),
            state.retained_capacity(),
        );
        for length in [0, 6, 0, 1, 5] {
            assert_eq!(state.parse_next(length, first.wire_repeats), Ok(None));
            assert_eq!(state.tables.long.test_positions(), before.0.0);
            assert_eq!(state.tables.short.test_positions(), before.0.1);
            assert_eq!(state.literals(), before.1);
            assert_eq!(state.sequences(), before.2);
            assert_eq!(state.retained_capacity(), before.3);
        }
        assert_eq!(state.position, 140);
        let last = state.parse_next(7, first.wire_repeats).unwrap().unwrap();
        assert_eq!(state.literals(), &[b'x'; 7]);
        assert!(state.sequences().is_empty());
        assert_eq!(last.trailing_literals, 7);
        assert_eq!(last.wire_repeats, first.wire_repeats);
    }

    #[test]
    fn invalid_extents_preserve_usable_state_before_any_owner_request() {
        let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = owner.monitor();
        let input = [b'z'; 32];
        let plan = plan(input.len(), 10);
        let mut state = DoubleFastMatchState::from_parts(
            &input,
            plan,
            DoubleFastTables::new_in(
                plan.long_layout(),
                plan.short_layout(),
                owner.clone(),
                owner.clone(),
            )
            .unwrap(),
            Vec::new_in(owner.clone()),
            Vec::new_in(owner),
        )
        .unwrap();
        assert!(monitor.fail_next());
        let before = monitor.snapshot();
        for length in [33, 1025, usize::MAX] {
            assert_eq!(
                state.parse_next(length, [1, 4, 8]),
                Err(Error::InvalidParameter)
            );
        }
        assert_eq!(state.parse_next(8, [1, 0, 8]), Err(Error::InvalidParameter));
        assert_eq!(state.position, 0);
        assert_eq!(state.prefix_floor, 0);
        assert!(!state.failed);
        assert_eq!(monitor.snapshot(), before);
        monitor.clear_failure();
        state.parse_next(32, [1, 4, 8]).unwrap().unwrap();
        assert_eq!(
            state.parse_next(usize::MAX, [1, 4, 8]),
            Err(Error::InvalidParameter)
        );
        assert_eq!(state.position, 32);
        drop(state);
        monitor.assert_released();
    }

    #[test]
    fn source_and_each_table_layout_are_checked_before_binding_owners() {
        use crate::match_search::SearchKeyLayout;

        let input = [b'x'; 64];
        let plan = plan(input.len(), 10);
        for invalid in 0..3 {
            let owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = owner.monitor();
            let long = if invalid == 1 {
                SearchKeyLayout::for_double_fast_long(11).unwrap()
            } else {
                plan.long_layout()
            };
            let short = if invalid == 2 {
                SearchKeyLayout::for_fast(8, 4).unwrap()
            } else {
                plan.short_layout()
            };
            let tables =
                DoubleFastTables::new_in(long, short, owner.clone(), owner.clone()).unwrap();
            let literals = Vec::new_in(owner.clone());
            let sequences = Vec::new_in(owner);
            let before = monitor.snapshot();
            assert!(monitor.fail_next());
            let source = if invalid == 0 {
                &input[..63]
            } else {
                &input[..]
            };
            assert!(matches!(
                DoubleFastMatchState::from_parts(source, plan, tables, literals, sequences),
                Err(Error::InvalidParameter)
            ));
            assert_eq!(monitor.snapshot().attempts, before.attempts);
            assert_eq!(monitor.snapshot().failures, 0);
            monitor.assert_released();
            assert_eq!(monitor.snapshot().owners_dropped, before.identities_created);
        }
    }

    #[test]
    fn either_parser_reserve_failure_poisoned_state_releases_exact_owners() {
        for fail_literals in [true, false] {
            let long_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let short_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let literal_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let sequence_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let long_monitor = long_owner.monitor();
            let short_monitor = short_owner.monitor();
            let lm = literal_owner.monitor();
            let sm = sequence_owner.monitor();
            let input = [b'x'; 96];
            let plan = plan(input.len(), 10);
            let mut state = DoubleFastMatchState::from_parts(
                &input,
                plan,
                DoubleFastTables::new_in(
                    plan.long_layout(),
                    plan.short_layout(),
                    long_owner,
                    short_owner,
                )
                .unwrap(),
                Vec::new_in(literal_owner),
                Vec::new_in(sequence_owner),
            )
            .unwrap();
            assert!(if fail_literals {
                lm.fail_next()
            } else {
                sm.fail_next()
            });
            assert_eq!(
                state.parse_next(96, [1, 4, 8]),
                Err(Error::AllocationFailed)
            );
            assert_eq!(state.position, 0);
            lm.clear_failure();
            sm.clear_failure();
            for length in [0, 1, 96] {
                assert_eq!(
                    state.parse_next(length, [1, 4, 8]),
                    Err(Error::InvalidState)
                );
            }
            drop(state);
            for monitor in [long_monitor, short_monitor, lm, sm] {
                monitor.assert_released();
                assert_eq!(
                    monitor.snapshot().owners_dropped,
                    monitor.snapshot().identities_created
                );
            }
        }
    }

    #[test]
    fn binding_new_source_resets_old_indices_with_borrowed_nonclone_owner() {
        let owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = owner.monitor();
        let input = [b'x'; 64];
        let plan = plan(input.len(), 10);
        let mut tables =
            DoubleFastTables::new_in(plan.long_layout(), plan.short_layout(), &owner, &owner)
                .unwrap();
        tables
            .long
            .store(plan.long_layout().read_key(&input).unwrap(), usize::MAX - 1)
            .unwrap();
        tables
            .short
            .store(
                plan.short_layout().read_key(&input).unwrap(),
                usize::MAX - 2,
            )
            .unwrap();
        let mut state = DoubleFastMatchState::from_parts(
            &input,
            plan,
            tables,
            Vec::new_in(&owner),
            Vec::new_in(&owner),
        )
        .unwrap();
        assert!(
            state
                .tables
                .long
                .test_positions()
                .iter()
                .all(|&p| p == usize::MAX)
        );
        assert!(
            state
                .tables
                .short
                .test_positions()
                .iter()
                .all(|&p| p == usize::MAX)
        );
        assert_eq!(monitor.snapshot().identities_created, 1);
        state.parse_next(input.len(), [1, 4, 8]).unwrap().unwrap();
        let before = monitor.snapshot();
        // Move complete owners to a fresh borrowed source without cloning or
        // requesting another allocation; both old tables and scratch reset.
        let next = [b'q'; 64];
        let mut state = DoubleFastMatchState::from_parts(
            &next,
            plan,
            state.tables,
            state.literals,
            state.sequences,
        )
        .unwrap();
        assert_eq!(monitor.snapshot(), before);
        assert!(
            state
                .tables
                .long
                .test_positions()
                .iter()
                .all(|&p| p == usize::MAX)
        );
        assert!(
            state
                .tables
                .short
                .test_positions()
                .iter()
                .all(|&p| p == usize::MAX)
        );
        assert!(state.literals().is_empty() && state.sequences().is_empty());
        state.parse_next(next.len(), [1, 4, 8]).unwrap().unwrap();
        assert_eq!(monitor.snapshot(), before);
        drop(state);
        monitor.assert_released();
    }
}
