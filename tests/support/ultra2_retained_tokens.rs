//! Retained Ultra2 actual-pass token publication, exact offset intents and nine-owner contracts.

use super::*;
use crate::RawCompressionParameters;
use crate::optimal_frequencies::OptimalFrequencies;
use crate::optimal_matches::OptimalMatches;
use crate::optimal_paths::OptimalPaths;
use crate::optimal_short_key::OptimalShortKey;
use crate::reference_tree::ReferenceTree;
use crate::ultra2_retained_fixture as fixture;
use alloc::alloc::Global;
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn plan(size: usize) -> RetainedUltra2Plan {
    RetainedUltra2Plan::for_frame(
        RawCompressionParameters {
            window_log: 15,
            hash_log: 6,
            chain_log: 6,
            search_log: 1,
            min_match: 3,
            target_length: 0,
            strategy: 9,
        }
        .validate()
        .unwrap(),
        size,
    )
    .unwrap()
}
type Parts<'a, A> = (
    OptimalParser<'a, A, A, A>,
    OptimalSearch<'a, A, A, A, A>,
    Vec<u8, A>,
    Vec<SequenceWithOffset, A>,
);
fn pieces<A: Allocator>(
    source: &[u8],
    plan: RetainedUltra2Plan,
    owners: [A; 9],
) -> Result<Parts<'_, A>> {
    let [f, p, m, h, n, s, b, l, q] = owners;
    let parser = OptimalParser::from_ultra2_retained_parts(
        source,
        plan.parameters().min_match(),
        plan.parameters().target_length() as usize,
        OptimalFrequencies::new_with_policy_in(
            crate::optimal_price::PricePolicy::Ultra,
            plan.disabled_literals(),
            f,
        )?,
        OptimalPaths::new_in(p)?,
        OptimalMatches::new_in(m)?,
    )?;
    let layout = plan.layout();
    let search = OptimalSearch::from_ultra2_retained_parts(
        source,
        plan,
        ReferenceTree::new_in(layout.tree(), h, n)?,
        OptimalShortKey::new_in(layout.short_log(), s)?,
        OptimalMatches::new_in(b)?,
    )?;
    Ok((parser, search, Vec::new_in(l), Vec::new_in(q)))
}
fn build<A: Allocator>(
    source: &[u8],
    plan: RetainedUltra2Plan,
    owners: [A; 9],
) -> Result<RetainedUltra2State<'_, A>> {
    let (parser, search, literals, sequences) = pieces(source, plan, owners)?;
    RetainedUltra2State::from_parts(source, plan, parser, search, literals, sequences)
}
fn publish<A: Allocator>(state: &mut RetainedUltra2State<'_, A>, kind: Publication) {
    let next = if kind == Publication::Compressed {
        state.pending_result().unwrap().repeats
    } else {
        state.coordinator.committed_repeats()
    };
    state.validate_publication(kind, next).unwrap();
    state.publish(kind, next).unwrap();
}

#[test]
fn retained_ultra2_integration_tokens_match_all_native_literals_sequences_and_publications() {
    let mut sequences = 0;
    let mut expected_sequences = 0;
    let mut discarded_sequences = 0;
    for case in fixture::read(fixture::PACK).unwrap().into_iter() {
        let parameters = RawCompressionParameters {
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
        let plan = RetainedUltra2Plan::new(
            parameters,
            case.source.len(),
            &blocks,
            case.literal_mode == 1,
        )
        .unwrap();
        let mut state = build(case.source, plan, [Global; 9]).unwrap();
        for block in &case.blocks {
            assert!(state.parse_next(block.length, block.incoming).unwrap());
            let result = state.pending_result().unwrap();
            assert_eq!(result.repeats, block.candidate);
            assert_eq!(result.tail, block.tail as usize);
            expected_sequences += block.sequence_count;
            discarded_sequences += block
                .phases
                .iter()
                .filter(|p| p.seeding)
                .map(|p| p.events.last().unwrap().sequence_count)
                .sum::<usize>();
            let expected: Vec<_> = block
                .phases
                .last()
                .unwrap()
                .events
                .iter()
                .flat_map(|e| &e.appended)
                .copied()
                .collect();
            let mut literals = Vec::new();
            let mut repeats = block.incoming;
            assert_eq!(state.sequences().len(), expected.len());
            for (actual, row) in state.sequences().iter().zip(expected) {
                let [anchor, literal_length, match_length, off_base] = row;
                literals.extend_from_slice(
                    &case.source[anchor as usize..(anchor + literal_length) as usize],
                );
                let next =
                    crate::optimal_path_fixture::publish(off_base, repeats, literal_length == 0)
                        .unwrap();
                assert_eq!(actual.sequence.literal_length, literal_length);
                assert_eq!(actual.sequence.match_length, match_length);
                assert_eq!(actual.sequence.distance, next[0]);
                let resolved = actual
                    .offset
                    .resolve(literal_length, next[0], repeats)
                    .unwrap();
                assert_eq!(resolved.offset_value, off_base);
                assert_eq!(resolved.next_repeat, next);
                repeats = next;
            }
            let end = block.start as usize + block.length;
            literals.extend_from_slice(&case.source[end - block.tail as usize..end]);
            assert_eq!(
                state.literals(),
                literals,
                "{} block {}",
                case.name,
                block.start
            );
            assert_eq!(repeats, block.candidate);
            sequences += result.sequences;
            state
                .publish(
                    Publication::try_from(block.kind as u8).unwrap(),
                    block.owners[block.previous_after],
                )
                .unwrap();
            assert!(state.literals().is_empty() && state.sequences().is_empty());
        }
        assert!(state.is_finished());
    }
    assert_eq!(sequences, expected_sequences);
    assert!(sequences > 0 && discarded_sequences > 0);
}

#[test]
fn retained_ultra2_integration_tokens_all_nine_nonclone_owner_failures_release_and_recover() {
    let source = [b'a'; 32768];
    let plan = plan(source.len());
    for rejected in 0..=9 {
        let owners = core::array::from_fn(|index| {
            TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 1,
                fail_at: if index == rejected {
                    core::num::NonZeroU64::new(1)
                } else {
                    None
                },
            })
            .unwrap()
            .into_non_clone()
        });
        let monitors = owners.each_ref().map(|a| a.monitor());
        let result = build(&source, plan, owners);
        if rejected < 7 {
            assert!(matches!(result, Err(Error::AllocationFailed)));
        } else {
            let mut state = result.unwrap();
            let first = state.parse_next(8192, [1, 4, 8]);
            if rejected < 9 {
                assert_eq!(first, Err(Error::AllocationFailed));
                assert!(state.literals().is_empty() && state.sequences().is_empty());
                assert_eq!(state.pending_result(), Err(Error::InvalidState));
                assert_eq!(state.parse_next(8192, [1, 4, 8]), Err(Error::InvalidState));
                state.reset().unwrap();
                assert!(state.parse_next(8192, [1, 4, 8]).unwrap());
            } else {
                assert_eq!(first, Ok(true));
            }
            let before = monitors.each_ref().map(|m| m.snapshot().attempts);
            let capacity = state.retained_capacity();
            publish(&mut state, Publication::Raw);
            for kind in [
                Publication::Compressed,
                Publication::Rle,
                Publication::Compressed,
            ] {
                state
                    .parse_next(8192, state.coordinator.committed_repeats())
                    .unwrap();
                publish(&mut state, kind);
            }
            assert!(state.is_finished());
            state.reset().unwrap();
            for kind in [
                Publication::Compressed,
                Publication::Raw,
                Publication::Rle,
                Publication::Compressed,
            ] {
                state
                    .parse_next(8192, state.coordinator.committed_repeats())
                    .unwrap();
                publish(&mut state, kind);
            }
            assert!(state.is_finished());
            assert_eq!(state.retained_capacity(), capacity);
            assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), before);
            drop(state);
        }
        for monitor in monitors {
            monitor.assert_released();
            assert_eq!(monitor.snapshot().owners_dropped, 1);
        }
    }
}

#[test]
fn retained_ultra2_integration_tokens_borrowed_owners_reject_arguments_without_mutation() {
    let source = [b'a'; 16385];
    let plan = plan(source.len());
    let owners: [_; 9] = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let mut state = build(&source, plan, owners.each_ref()).unwrap();
    let initial = monitors.each_ref().map(|m| m.snapshot().attempts);
    assert_eq!(initial, [1, 1, 1, 1, 1, 1, 1, 0, 0]);
    for (length, repeats) in [(0, [1, 4, 8]), (5461, [1, 4, 8]), (5462, [2, 4, 8])] {
        assert_eq!(
            state.parse_next(length, repeats),
            Err(Error::InvalidParameter)
        );
        assert_eq!(state.coordinator.position(), 0);
        assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), initial);
    }
    state.parse_next(5462, [1, 4, 8]).unwrap();
    let literals = state.literals().to_vec();
    let count = state.sequences().len();
    assert_eq!(
        state.publish(Publication::Compressed, [u32::MAX, 0, 0]),
        Err(Error::InvalidParameter)
    );
    assert_eq!(state.literals(), literals);
    assert_eq!(state.sequences().len(), count);
    assert_eq!(state.parse_next(5462, [1, 4, 8]), Err(Error::InvalidState));
    let before = monitors.each_ref().map(|m| m.snapshot().attempts);
    publish(&mut state, Publication::Raw);
    assert_eq!(
        state.publish(Publication::Raw, [1, 4, 8]),
        Err(Error::InvalidState)
    );
    state.parse_next(5462, [1, 4, 8]).unwrap();
    state.reset().unwrap();
    for &length in plan.blocks() {
        state
            .parse_next(length, state.coordinator.committed_repeats())
            .unwrap();
        publish(&mut state, Publication::Compressed);
    }
    assert!(state.is_finished());
    assert_eq!(state.parse_next(5461, [1, 4, 8]), Err(Error::InvalidState));
    assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), before);
    drop(state);
    for monitor in &monitors {
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
    }
    drop(owners);
    for monitor in monitors {
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
}

#[test]
fn retained_ultra2_integration_tokens_collector_requires_empty_reserved_block_local_scratch() {
    let source = [b'a'; 128];
    let mut literals = Vec::with_capacity_in(64, Global);
    let mut sequences = Vec::with_capacity_in(21, Global);
    for (prefix, start, repeats) in [
        (6, 0, [1, 4, 8]),
        (128, 129, [1, 4, 8]),
        (128, 64, [0, 4, 8]),
        (128, 64, [129, 4, 8]),
        (128, 0, [1, 4, 8]),
    ] {
        assert!(matches!(
            Collect::for_ultra2_retained_block(
                &source[..prefix],
                start,
                repeats,
                &mut literals,
                &mut sequences
            ),
            Err(Error::InvalidParameter)
        ));
    }
    literals.push(7);
    assert!(matches!(
        Collect::for_ultra2_retained_block(&source, 64, [1, 4, 8], &mut literals, &mut sequences),
        Err(Error::InvalidParameter)
    ));
    assert_eq!(literals, [7]);
    literals.clear();
    let mut collector =
        Collect::for_ultra2_retained_block(&source, 64, [1, 4, 8], &mut literals, &mut sequences)
            .unwrap();
    collector
        .finish(
            PathResult {
                tail: 64,
                repeats: [1, 4, 8],
                sequences: 0,
            },
            true,
        )
        .unwrap();
    assert_eq!(literals, source[64..]);
    for length in [7, 8, 9, 63] {
        literals.clear();
        sequences.clear();
        assert!(
            Collect::for_block(
                &source[..length],
                0,
                [1, 4, 8],
                &mut literals,
                &mut sequences
            )
            .is_err()
        );
        let mut collector = Collect::for_ultra2_retained_block(
            &source[..length],
            0,
            [1, 4, 8],
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        collector
            .finish(
                PathResult {
                    tail: length,
                    repeats: [1, 4, 8],
                    sequences: 0,
                },
                true,
            )
            .unwrap();
        assert_eq!(literals, &source[..length]);
    }
}

#[test]
fn retained_ultra2_integration_tokens_constructor_rejects_source_and_price_mismatches() {
    let source = [b'a'; 16385];
    let other = [b'a'; 16385];
    let plan = plan(source.len());
    for mismatch in 0..3 {
        let (parser, search, literals, sequences) = pieces(&source, plan, [Global; 9]).unwrap();
        let changed = if mismatch == 1 {
            RetainedUltra2Plan::new(plan.parameters(), source.len(), plan.blocks(), true).unwrap()
        } else if mismatch == 2 {
            let p = RawCompressionParameters {
                window_log: 15,
                hash_log: 6,
                chain_log: 7,
                search_log: 1,
                min_match: 3,
                target_length: 0,
                strategy: 9,
            };
            RetainedUltra2Plan::new(p.validate().unwrap(), source.len(), plan.blocks(), false)
                .unwrap()
        } else {
            plan
        };
        assert!(matches!(
            RetainedUltra2State::from_parts(
                if mismatch == 0 { &other } else { &source },
                changed,
                parser,
                search,
                literals,
                sequences
            ),
            Err(Error::InvalidParameter)
        ));
    }
}
