//! Explicit retained Ultra entries and transactional ownership boundaries.
use super::*;
use crate::RawCompressionParameters;
use crate::optimal_frequencies::OptimalFrequencies;
use crate::optimal_matches::OptimalMatches;
use crate::optimal_parse::PathEvent;
use crate::optimal_paths::OptimalPaths;
use crate::optimal_search_layout::OptimalMatch;
use crate::optimal_short_key::OptimalShortKey;
use crate::reference_tree::ReferenceTree;
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn plan(length: usize) -> RetainedOptimalPlan {
    RetainedOptimalPlan::new_ultra(
        RawCompressionParameters {
            window_log: 10,
            hash_log: 6,
            chain_log: 6,
            search_log: 1,
            min_match: 3,
            target_length: 0,
            strategy: 8,
        }
        .validate()
        .unwrap(),
        length,
        &[length / 2, length / 2],
        false,
    )
    .unwrap()
}
type Pair<'a, A> = (OptimalParser<'a, A, A, A>, OptimalSearch<'a, A, A, A, A>);
fn pair<A: Allocator>(
    source: &[u8],
    plan: RetainedOptimalPlan,
    owners: [A; 7],
    scope: u8,
) -> Result<Pair<'_, A>> {
    let [f, p, m, h, n, s, b] = owners;
    let policy = if scope == 0 {
        PricePolicy::Optimal
    } else {
        PricePolicy::Ultra
    };
    let f = OptimalFrequencies::new_with_policy_in(policy, plan.disabled_literals(), f)?;
    let p = OptimalPaths::new_in(p)?;
    let m = OptimalMatches::new_in(m)?;
    let minimum = plan.parameters().min_match();
    let target = plan.parameters().target_length() as usize;
    let parser = match scope {
        0 => OptimalParser::from_parts(source, minimum, target, f, p, m),
        1 => OptimalParser::from_policy_parts(policy, source, minimum, target, f, p, m),
        2 => OptimalParser::from_ultra_fresh_parts(source, minimum, target, f, p, m),
        3 => OptimalParser::from_ultra2_parts(source, minimum, target, f, p, m),
        _ => OptimalParser::from_ultra_retained_parts(source, minimum, target, f, p, m),
    }?;
    let l = plan.layout();
    let search = OptimalSearch::from_parts(
        source,
        l,
        ReferenceTree::new_in(l.tree(), h, n)?,
        OptimalShortKey::new_in(l.short_log(), s)?,
        OptimalMatches::new_in(b)?,
    )?;
    Ok((parser, search))
}
#[derive(Default)]
struct Observe {
    events: usize,
    fail_stage: Option<u8>,
}
impl PathObserver for Observe {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        _: &OptimalPaths<P>,
        _: &OptimalFrequencies<F>,
        _: &[OptimalMatch],
    ) -> Result<()> {
        self.events += 1;
        if self.fail_stage == Some(event.stage) {
            Err(Error::ResourceLimit)
        } else {
            Ok(())
        }
    }
}
struct NoObserver;
impl PathObserver for NoObserver {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        _: PathEvent,
        _: &OptimalPaths<P>,
        _: &OptimalFrequencies<F>,
        _: &[OptimalMatch],
    ) -> Result<()> {
        panic!("rejected entry called observer")
    }
}

#[test]
fn all_old_scopes_reject_both_new_entries_before_observation() {
    let source = [b'a'; 128];
    let plan = plan(source.len());
    for scope in 0..4 {
        let (mut parser, mut search) = pair(&source, plan, [Global; 7], scope).unwrap();
        let before = search.test_snapshot();
        assert_eq!(
            parser.parse_ultra_retained(0, 64, [1, 4, 8], &mut search, &mut NoObserver),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(search.test_snapshot(), before);
        assert!(matches!(
            RetainedOptimal::from_ultra_parts(parser, search, plan),
            Err(Error::UnsupportedFeature)
        ));
    }
    let (parser, search) = pair(&source, plan, [Global; 7], 4).unwrap();
    assert!(matches!(
        RetainedOptimal::from_parts(parser, search, plan.blocks()),
        Err(Error::UnsupportedFeature)
    ));
}

#[test]
fn malformed_direct_calls_preserve_parser_and_search_before_entry() {
    let source = [b'a'; 128];
    let other = source;
    let plan = plan(source.len());
    let (mut parser, mut search) = pair(&source, plan, [Global; 7], 4).unwrap();
    let (_, mut unrelated) = pair(&other, plan, [Global; 7], 4).unwrap();
    let before = unrelated.test_snapshot();
    assert_eq!(
        parser.parse_ultra_retained(0, 64, [1, 4, 8], &mut unrelated, &mut NoObserver),
        Err(Error::InvalidParameter)
    );
    assert_eq!(unrelated.test_snapshot(), before);
    for (start, end, repeats, error) in [
        (0, 63, [1, 4, 8], Error::InvalidParameter),
        (0, 129, [1, 4, 8], Error::InvalidParameter),
        (1, 65, [1, 4, 8], Error::InvalidState),
        (64, 128, [1, 4, 8], Error::InvalidState),
        (usize::MAX, 64, [1, 4, 8], Error::InvalidParameter),
        (0, 64, [0, 4, 8], Error::InvalidParameter),
        (0, 64, [129, 4, 8], Error::InvalidParameter),
    ] {
        let before = search.test_snapshot();
        assert_eq!(
            parser.parse_ultra_retained(start, end, repeats, &mut search, &mut NoObserver),
            Err(error)
        );
        assert_eq!(search.test_snapshot(), before);
        assert!(parser.ultra_retained_configuration().is_ok());
    }
    parser
        .parse_ultra_retained(0, 64, [1, 4, 8], &mut search, &mut Observe::default())
        .unwrap();
    let before = search.test_snapshot();
    assert_eq!(
        parser.parse_ultra_retained(0, 64, [1, 4, 8], &mut search, &mut NoObserver),
        Err(Error::InvalidState)
    );
    assert_eq!(search.test_snapshot(), before);
    parser
        .parse_ultra_retained(64, 128, [1, 4, 8], &mut search, &mut Observe::default())
        .unwrap();
}

#[test]
fn mismatched_plan_and_used_search_release_moved_owners() {
    let source = [b'a'; 128];
    let correct = plan(source.len());
    for mismatch in 0..6 {
        let owners = core::array::from_fn(|_| {
            TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone()
        });
        let monitors = owners.each_ref().map(|a| a.monitor());
        let (parser, mut search) = pair(&source, correct, owners, 4).unwrap();
        let mut raw = RawCompressionParameters {
            window_log: 10,
            hash_log: 6,
            chain_log: 6,
            search_log: 1,
            min_match: 3,
            target_length: 0,
            strategy: 8,
        };
        match mismatch {
            1 => raw.min_match = 4,
            2 => raw.target_length = 1,
            3 => raw.chain_log = 7,
            _ => {}
        }
        let changed = if mismatch == 4 {
            raw.strategy = 7;
            RetainedOptimalPlan::new(raw.validate().unwrap(), 128, &[64, 64], false).unwrap()
        } else {
            RetainedOptimalPlan::new_ultra(raw.validate().unwrap(), 128, &[64, 64], mismatch == 0)
                .unwrap()
        };
        if mismatch == 5 {
            search.begin_retained(0, 64).unwrap();
        }
        let error = if mismatch == 4 {
            Error::UnsupportedFeature
        } else if mismatch == 5 {
            Error::InvalidState
        } else {
            Error::InvalidParameter
        };
        assert!(
            matches!(RetainedOptimal::from_ultra_parts(parser,search,changed),Err(e) if e==error)
        );
        for m in monitors {
            m.assert_released();
            assert_eq!(m.snapshot().owners_dropped, 1);
            assert!(!m.snapshot().violations.any());
        }
    }
}

#[test]
fn seven_nonclone_owner_failures_and_publication_reset_preserve_allocations() {
    let source = [b'a'; 128];
    let plan = plan(source.len());
    for rejected in 0..=7 {
        let owners = core::array::from_fn(|i| {
            TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 1,
                fail_at: if i == rejected {
                    core::num::NonZeroU64::new(1)
                } else {
                    None
                },
            })
            .unwrap()
            .into_non_clone()
        });
        let monitors = owners.each_ref().map(|a| a.monitor());
        let result = pair(&source, plan, owners, 4)
            .and_then(|(p, s)| RetainedOptimal::from_ultra_parts(p, s, plan));
        if rejected < 7 {
            assert!(matches!(result, Err(Error::AllocationFailed)));
        } else {
            let mut state = result.unwrap();
            let before = monitors.each_ref().map(|m| m.snapshot().attempts);
            for kind in [Publication::Raw, Publication::Rle, Publication::Compressed] {
                assert_eq!(state.publish(kind), Err(Error::InvalidState));
                let pending = state.parse_next(&mut Observe::default()).unwrap();
                assert_eq!(state.parse_next(&mut NoObserver), Err(Error::InvalidState));
                state.publish(kind).unwrap();
                assert_eq!(
                    state.committed_repeats(),
                    if kind == Publication::Compressed {
                        pending.repeats
                    } else {
                        [1, 4, 8]
                    }
                );
                state.parse_next(&mut Observe::default()).unwrap();
                state.publish(Publication::Compressed).unwrap();
                assert!(state.is_finished());
                assert_eq!(state.parse_next(&mut NoObserver), Err(Error::InvalidState));
                state.reset().unwrap();
                assert_eq!(state.policy, PricePolicy::Ultra);
                assert_eq!(state.position(), 0);
                assert_eq!(state.committed_repeats(), [1, 4, 8]);
            }
            assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), before);
            drop(state);
        }
        for m in monitors {
            m.assert_released();
            assert_eq!(m.snapshot().owners_dropped, 1);
            assert!(!m.snapshot().violations.any());
        }
    }
}

#[test]
fn observer_failure_poisoning_and_borrowed_reset_replay_the_same_blocks() {
    let source = [b'a'; 128];
    let plan = plan(source.len());
    let owners: [_; 7] = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let (parser, search) = pair(&source, plan, owners.each_ref(), 4).unwrap();
    let mut state = RetainedOptimal::from_ultra_parts(parser, search, plan).unwrap();
    let before = monitors.each_ref().map(|m| m.snapshot().attempts);
    for failed_block in 0..2 {
        for stage in [0, 9, 10] {
            if failed_block == 1 {
                state.parse_next(&mut Observe::default()).unwrap();
                state.publish(Publication::Raw).unwrap();
            }
            let mut fail = Observe {
                fail_stage: Some(stage),
                ..Observe::default()
            };
            assert_eq!(state.parse_next(&mut fail), Err(Error::ResourceLimit));
            assert!(fail.events > 0);
            assert_eq!(state.phase, Phase::Poisoned);
            assert_eq!(state.parse_next(&mut NoObserver), Err(Error::InvalidState));
            assert_eq!(state.pending_result(), Err(Error::InvalidState));
            assert_eq!(state.publish(Publication::Raw), Err(Error::InvalidState));
            state.reset().unwrap();
            let (p, s) = pair(&source, plan, [Global; 7], 4).unwrap();
            let mut fresh = RetainedOptimal::from_ultra_parts(p, s, plan).unwrap();
            for _ in 0..2 {
                assert_eq!(
                    state.parse_next(&mut Observe::default()).unwrap(),
                    fresh.parse_next(&mut Observe::default()).unwrap()
                );
                assert_eq!(state.search.test_snapshot(), fresh.search.test_snapshot());
                state.publish(Publication::Compressed).unwrap();
                fresh.publish(Publication::Compressed).unwrap();
            }
            state.reset().unwrap();
        }
    }
    assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), before);
    drop(state);
    for m in &monitors {
        m.assert_released();
        assert_eq!(m.snapshot().owners_dropped, 0);
    }
    drop(owners);
    for m in monitors {
        assert_eq!(m.snapshot().owners_dropped, 1);
        assert!(!m.snapshot().violations.any());
    }
}
