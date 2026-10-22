//! Native retained Ultra2 query/state replay and isolated search lifecycle gates.
use super::*;
use crate::RawCompressionParameters;
use crate::ultra2_retained_fixture as fixture;
use crate::ultra2_retained_plan::RetainedUltra2Plan;
use alloc::{vec, vec::Vec};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
fn plan(source_len: usize, blocks: &[usize]) -> RetainedUltra2Plan {
    RetainedUltra2Plan::new(
        RawCompressionParameters {
            window_log: 16,
            hash_log: 6,
            chain_log: 6,
            search_log: 1,
            min_match: 3,
            target_length: 0,
            strategy: 9,
        }
        .validate()
        .unwrap(),
        source_len,
        blocks,
        false,
    )
    .unwrap()
}
fn case_plan(case: &fixture::Case<'_>) -> RetainedUltra2Plan {
    RetainedUltra2Plan::new(
        RawCompressionParameters {
            window_log: case.window.into(),
            hash_log: case.hash.into(),
            chain_log: case.chain.into(),
            search_log: case.search.into(),
            min_match: case.minimum,
            target_length: case.target,
            strategy: 9,
        }
        .validate()
        .unwrap(),
        case.source.len(),
        &case.blocks.iter().map(|b| b.length).collect::<Vec<_>>(),
        case.literal_mode == 1,
    )
    .unwrap()
}
fn state<'a, A: Allocator>(
    source: &'a [u8],
    p: RetainedUltra2Plan,
    owners: [A; 4],
) -> Result<OptimalSearch<'a, A, A, A, A>> {
    let [h, n, s, m] = owners;
    let l = p.layout();
    OptimalSearch::from_ultra2_retained_parts(
        source,
        p,
        ReferenceTree::new_in(l.tree(), h, n)?,
        OptimalShortKey::new_in(l.short_log(), s)?,
        OptimalMatches::new_in(m)?,
    )
}
fn query(q: [u32; 6]) -> OptimalQuery {
    OptimalQuery {
        position: q[0] as usize,
        repeats: [q[1], q[2], q[3]],
        zero_literals: q[4] == 1,
        length_to_beat: q[5] as usize,
    }
}
#[test]
fn every_native_phase_query_and_complete_search_map_matches() {
    let mut events = 0;
    let mut phases = 0;
    let mut seeds = 0;
    let mut queries = 0;
    for case in fixture::read(fixture::PACK).unwrap() {
        let p = case_plan(&case);
        let mut actual = state(case.source, p, [Global; 4]).unwrap();
        let mut expected = vec![0; case.search_cells()];
        for block in &case.blocks {
            for phase in &block.phases {
                actual
                    .begin_ultra2_retained(
                        block.start as usize,
                        block.start as usize + block.length,
                        phase.seeding,
                    )
                    .unwrap();
                let c = actual.ultra2_retained_coordinates().unwrap();
                assert_eq!(c.origin() + 2, phase.origin as usize, "{}", case.name);
                assert_eq!(c.len(), block.start as usize + block.length);
                for (index, event) in phase.events.iter().enumerate() {
                    if let Some(q) = event.query {
                        let found: Vec<_> = actual
                            .query_ultra2_retained(query(q))
                            .unwrap()
                            .iter()
                            .map(|m| [m.off_base(), m.length() as u32])
                            .collect();
                        assert_eq!(
                            found, event.matches,
                            "{} block{} phase{} event{index}",
                            case.name, block.start, phase.seeding
                        );
                        queries += 1;
                    }
                    for &(at, value) in &phase.search_changes[index] {
                        expected[at] = value;
                    }
                    assert_eq!(
                        actual.test_snapshot(),
                        expected,
                        "{} block{} phase{} event{index}",
                        case.name,
                        block.start,
                        phase.seeding
                    );
                    events += 1;
                }
                if phase.seeding {
                    let before = actual.test_snapshot();
                    actual.rebase_ultra2_retained().unwrap();
                    let after = actual.test_snapshot();
                    assert_eq!(&before[..before.len() - 2], &after[..after.len() - 2]);
                    assert_eq!(
                        &after[after.len() - 2..],
                        &[p.first_block_len() as u32 + 2; 2]
                    );
                    seeds += 1;
                }
                phases += 1;
            }
        }
    }
    assert_eq!((events, phases, seeds), (229657, 123, 30));
    assert!(queries > 0);
}
#[test]
fn bad_schedule_phase_query_and_old_entries_reject_without_state_mutation() {
    let source = [b'a'; 194];
    let p = plan(source.len(), &[97, 97]);
    let mut s = state(&source, p, [Global; 4]).unwrap();
    let q = query([1, 1, 4, 8, 0, 3]);
    for _ in 0..2 {
        let before = s.test_snapshot();
        assert!(s.query(q).is_err());
        assert!(s.query_fresh(q).is_err());
        assert!(s.query_retained(q).is_err());
        assert!(s.query_ultra2(q).is_err());
        assert!(s.query_ultra2_retained(q).is_err());
        assert!(s.begin_retained(0, 97).is_err());
        assert!(s.rebase_ultra2().is_err());
        assert!(s.rebase_ultra2_retained().is_err());
        assert!(s.validate_fresh_start(&source, 3, 0).is_err());
        assert!(s.validate_retained_start(&source, 3, 0).is_err());
        assert!(s.validate_ultra2_start(&source, 3, 0).is_err());
        for (start, end, seed) in [
            (0, 96, true),
            (0, 195, true),
            (1, 98, true),
            (97, 194, false),
            (0, 97, false),
            (usize::MAX, 97, true),
        ] {
            assert!(s.begin_ultra2_retained(start, end, seed).is_err());
            assert_eq!(s.test_snapshot(), before);
            assert_eq!(s.source.len(), 194);
        }
        assert_eq!(s.test_snapshot(), before);
        s.validate_ultra2_retained_start(&source, 3, 0).unwrap();
        s.reset();
    }
    s.begin_ultra2_retained(0, 97, true).unwrap();
    for bad in [
        query([0, 1, 4, 8, 0, 3]),
        query([90, 1, 4, 8, 0, 3]),
        query([1, 0, 4, 8, 0, 3]),
        query([1, 195, 4, 8, 0, 3]),
        query([1, 1, 4, 8, 0, 4]),
    ] {
        let before = s.test_snapshot();
        assert!(s.query_ultra2_retained(bad).is_err());
        assert_eq!(s.test_snapshot(), before);
    }
    s.query_ultra2_retained(q).unwrap();
    s.rebase_ultra2_retained().unwrap();
    let before = s.test_snapshot();
    assert!(s.rebase_ultra2_retained().is_err());
    assert!(s.begin_ultra2_retained(0, 97, true).is_err());
    assert_eq!(s.test_snapshot(), before);
    s.begin_ultra2_retained(0, 97, false).unwrap();
    s.query_ultra2_retained(q).unwrap();
    s.begin_ultra2_retained(97, 194, false).unwrap();
    let before = s.test_snapshot();
    assert!(s.query_ultra2_retained(q).is_err());
    assert_eq!(s.test_snapshot(), before);
    s.query_ultra2_retained(query([97, 1, 4, 8, 1, 3])).unwrap();
    s.reset();
    assert!(s.validate_ultra2_retained_start(&source, 3, 0).is_ok());
}
#[test]
fn threshold_and_non_power_biases_do_not_read_future_source_or_stale_cells() {
    for first in [7, 8, 9, 65, 95, 97, 8192] {
        let p = plan(first + 97, &[first, 97]);
        let a = vec![b'a'; first + 97];
        let mut b = a.clone();
        b[first..].fill(b'b');
        let mut x = state(&a, p, [Global; 4]).unwrap();
        let mut y = state(&b, p, [Global; 4]).unwrap();
        let seed = p.seeds_first_block();
        for seeding in if seed {
            &[true, false][..]
        } else {
            &[false][..]
        } {
            for s in [&mut x, &mut y] {
                s.begin_ultra2_retained(0, first, *seeding).unwrap();
            }
            if first > 8 {
                assert_eq!(
                    x.query_ultra2_retained(query([1, 1, 4, 8, 0, 3])).unwrap(),
                    y.query_ultra2_retained(query([1, 1, 4, 8, 0, 3])).unwrap()
                );
            }
            assert_eq!(x.test_snapshot(), y.test_snapshot());
            for s in [&mut x, &mut y] {
                assert!(s.source.byte(s.source.len()).is_err());
                if !*seeding && seed {
                    assert!(s.source.byte(first - 1).is_err());
                }
                if *seeding {
                    s.rebase_ultra2_retained().unwrap();
                }
            }
        }
    }
}
#[test]
fn owned_and_borrowed_allocators_survive_rebase_and_reset_without_new_allocations() {
    let source = [b'a'; 194];
    let p = plan(source.len(), &[97, 97]);
    for rejected in 0..=4 {
        let owners: [_; 4] = core::array::from_fn(|i| {
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
        let result = state(&source, p, owners);
        if rejected < 4 {
            assert!(matches!(result, Err(Error::AllocationFailed)));
        } else {
            let mut s = result.unwrap();
            let before = monitors.each_ref().map(|m| m.snapshot().attempts);
            for _ in 0..3 {
                s.begin_ultra2_retained(0, 97, true).unwrap();
                s.query_ultra2_retained(query([1, 1, 4, 8, 0, 3])).unwrap();
                s.rebase_ultra2_retained().unwrap();
                s.begin_ultra2_retained(0, 97, false).unwrap();
                s.begin_ultra2_retained(97, 194, false).unwrap();
                s.reset();
            }
            assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), before);
            drop(s);
        }
        for m in monitors {
            m.assert_released();
            assert_eq!(m.snapshot().owners_dropped, 1);
            assert!(!m.snapshot().violations.any());
        }
    }
    let owners: [_; 4] = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let mut s = state(&source, p, owners.each_ref()).unwrap();
    s.begin_ultra2_retained(0, 97, true).unwrap();
    s.rebase_ultra2_retained().unwrap();
    s.reset();
    drop(s);
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
fn binding_mismatch_and_failed_search_require_complete_reset() {
    let source = [b'a'; 194];
    let other = source;
    let p = plan(source.len(), &[97, 97]);
    let mut s = state(&source, p, [Global; 4]).unwrap();
    for (bytes, minimum, target) in [
        (&other[..], 3, 0),
        (&source[..193], 3, 0),
        (&source[..], 4, 0),
        (&source[..], 3, 1),
    ] {
        assert_eq!(
            s.validate_ultra2_retained_start(bytes, minimum, target),
            Err(Error::InvalidParameter)
        );
    }
    s.begin_ultra2_retained(0, 97, true).unwrap();
    s.failed = true;
    let before = s.test_snapshot();
    assert!(s.begin_ultra2_retained(0, 97, true).is_err());
    assert!(s.rebase_ultra2_retained().is_err());
    assert!(s.query_ultra2_retained(query([1, 1, 4, 8, 0, 3])).is_err());
    assert_eq!(s.test_snapshot(), before);
    s.reset();
    s.validate_ultra2_retained_start(&source, 3, 0).unwrap();
    let l = p.layout();
    let mut old = OptimalSearch::from_parts(
        &source,
        l,
        ReferenceTree::new_in(l.tree(), Global, Global).unwrap(),
        OptimalShortKey::new_in(l.short_log(), Global).unwrap(),
        OptimalMatches::new_in(Global).unwrap(),
    )
    .unwrap();
    assert!(old.validate_ultra2_retained_start(&source, 3, 0).is_err());
    assert!(old.begin_ultra2_retained(0, 97, true).is_err());
    assert!(old.rebase_ultra2_retained().is_err());
}
