//! Persistent sampled history for one borrowed, contiguous Fast source.
#![forbid(unsafe_code)]

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::fast_parse::{FastBlock, FastParseResult, parse_block};
use crate::fast_plan::FastExecutionPlan;
use crate::matchfinder::MatchTable;
use crate::sequence_context::SequenceWithOffset;
use crate::{Error, Result};

/// Search progress is separate from the caller's committed entropy state.
///
/// All positions index this single borrowed source. Raw/RLE selection retains
/// sampled cells; the caller supplies only published wire repeats on each call.
/// A failed parse requires dropping this state, never retrying partial sampling.
/// Corresponds to the no-dictionary match/window portion of `ZSTD_CCtx`.
pub(crate) struct FastMatchState<'a, A: Allocator> {
    source: &'a [u8],
    plan: FastExecutionPlan,
    table: MatchTable<A>,
    literals: Vec<u8, A>,
    sequences: Vec<SequenceWithOffset, A>,
    position: usize,
    prefix_floor: usize,
    failed: bool,
}

impl<'a, A: Allocator> FastMatchState<'a, A> {
    /// Bind complete owners to one source, invalidating earlier sampled indices.
    /// Does not clone an allocator, allocate, or borrow temporary source storage.
    pub(crate) fn from_parts(
        source: &'a [u8],
        plan: FastExecutionPlan,
        mut table: MatchTable<A>,
        mut literals: Vec<u8, A>,
        mut sequences: Vec<SequenceWithOffset, A>,
    ) -> Result<Self> {
        if source.len() != plan.source_len() || table.layout() != plan.layout() {
            return Err(Error::InvalidParameter);
        }
        table.reset();
        literals.clear();
        sequences.clear();
        Ok(Self {
            source,
            plan,
            table,
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
    ) -> Result<Option<FastParseResult>> {
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
            let block = FastBlock::new(
                self.source,
                self.position..end,
                floor,
                self.plan.layout(),
                self.plan.window_size(),
                self.plan.initial_step(),
            )?;
            match parse_block(
                block,
                &mut self.table,
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
        self.table.retained_capacity()
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

    fn plan(length: usize, window_log: u8) -> FastExecutionPlan {
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_window_log(window_log).unwrap();
        request.set_hash_log(10).unwrap();
        request.set_min_match(4).unwrap();
        FastExecutionPlan::new(&request, length).unwrap()
    }

    fn fresh(source: &[u8], window_log: u8) -> FastMatchState<'_, Global> {
        let plan = plan(source.len(), window_log);
        FastMatchState::from_parts(
            source,
            plan,
            MatchTable::with_layout_in(plan.layout(), Global).unwrap(),
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

        let cases = crate::fast_frame_oracle_fixture::cases().unwrap();
        assert_eq!(cases.len(), 23);
        let mut blocks = 0;
        let mut fresh = 0;
        let mut meaningful_raw_rollback = 0;
        let mut meaningful_rle_rollback = 0;
        for case in cases {
            // The internal C begin call applies these explicit values directly.
            // Public known-size request resolution is a separate tested contract.
            let parameters = RawCompressionParameters {
                window_log: u32::from(case.window_log),
                chain_log: 12,
                hash_log: u32::from(case.hash_log),
                search_log: 1,
                min_match: u32::from(case.min_match),
                target_length: case.target_length,
                strategy: 1,
            }
            .validate()
            .unwrap();
            let plan =
                FastExecutionPlan::from_observed_parameters(parameters, case.source.len()).unwrap();
            let mut state = FastMatchState::from_parts(
                &case.source,
                plan,
                MatchTable::with_layout_in(plan.layout(), Global).unwrap(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
            let mut expected_table = vec![0_u32; plan.bucket_count()];
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
                    table_hash(state.table.test_positions()),
                    expected.table_hashes[0]
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
                        // Fast never consumes the raw third C cell. Rust carries
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
                for &[bucket, old, new] in &expected.table_changes {
                    assert_eq!(expected_table[bucket as usize], old);
                    expected_table[bucket as usize] = new;
                }
                assert!(
                    state
                        .table
                        .test_positions()
                        .iter()
                        .zip(&expected_table)
                        .all(|(&actual, &expected)| raw_index(actual) == expected),
                    "{} complete sampled table",
                    case.name
                );
                assert_eq!(
                    table_hash(state.table.test_positions()),
                    expected.table_hashes[1]
                );
                blocks += 1;
            }
            assert!(observations.next().is_none());
            assert_eq!(state.position, case.source.len());
        }
        assert_eq!((blocks, fresh), (64, 62));
        assert!(meaningful_raw_rollback >= 10);
        assert_eq!(meaningful_rle_rollback, 2);
    }

    #[test]
    fn raw_publication_keeps_search_history_without_publishing_candidate_repeats() {
        let prefix = noise(0x1234_5678, 1024);
        let input = [prefix.as_slice(), prefix.as_slice()].concat();
        let mut state = fresh(&input, 12);
        state.parse_next(1024, [1, 4, 8]).unwrap().unwrap();
        assert!(state.sequences().is_empty());
        assert!(state.table.test_positions().iter().any(|&p| p < 1024));
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
        assert!(state.table.test_positions().iter().any(|&p| p < 1024));
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
            state.table.test_positions().to_vec(),
            state.literals().to_vec(),
            state.sequences().to_vec(),
            state.retained_capacity(),
        );
        for length in [0, 6, 0, 1, 5] {
            assert_eq!(state.parse_next(length, first.wire_repeats), Ok(None));
            assert_eq!(state.table.test_positions(), before.0);
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
        let mut state = FastMatchState::from_parts(
            &input,
            plan,
            MatchTable::with_layout_in(plan.layout(), owner.clone()).unwrap(),
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
    fn either_parser_reserve_failure_poisoned_state_releases_exact_owners() {
        for fail_literals in [true, false] {
            let table_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let literal_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let sequence_owner = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let tm = table_owner.monitor();
            let lm = literal_owner.monitor();
            let sm = sequence_owner.monitor();
            let input = [b'x'; 96];
            let plan = plan(input.len(), 10);
            let mut state = FastMatchState::from_parts(
                &input,
                plan,
                MatchTable::with_layout_in(plan.layout(), table_owner).unwrap(),
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
            for monitor in [tm, lm, sm] {
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
        let mut table = MatchTable::with_layout_in(plan.layout(), &owner).unwrap();
        let key = plan.layout().read_key(&input).unwrap();
        table.store(key, usize::MAX - 1).unwrap();
        let mut state = FastMatchState::from_parts(
            &input,
            plan,
            table,
            Vec::new_in(&owner),
            Vec::new_in(&owner),
        )
        .unwrap();
        assert!(
            state
                .table
                .test_positions()
                .iter()
                .all(|&p| p == usize::MAX)
        );
        assert_eq!(monitor.snapshot().identities_created, 1);
        state.parse_next(input.len(), [1, 4, 8]).unwrap().unwrap();
        drop(state);
        monitor.assert_released();
    }
}
