//! Every native query and physical owner state across the unchanged two-pass reset.
use super::*;
use crate::ultra2_fixture as fixture;
use alloc::{alloc::Global, vec};
fn layout(case: &fixture::Case<'_>) -> OptimalSearchLayout {
    OptimalSearchLayout::new(
        case.window,
        case.hash,
        case.chain,
        case.search,
        case.minimum as u8,
        case.target as usize,
        (case.minimum == 3).then_some(case.window),
    )
    .unwrap()
}
fn state<'a, A: Allocator>(
    source: &'a [u8],
    l: OptimalSearchLayout,
    owners: [A; 4],
) -> Result<OptimalSearch<'a, A>> {
    let [h, n, s, m] = owners;
    OptimalSearch::from_ultra2_parts(
        source,
        l,
        ReferenceTree::new_in(l.tree(), h, n)?,
        OptimalShortKey::new_in(l.short_log(), s)?,
        OptimalMatches::new_in(m)?,
    )
}
fn query(q: [u32; 6]) -> OptimalQuery {
    OptimalQuery {
        position: q[0] as usize,
        repeats: [q[1], q[2], q[3]],
        zero_literals: q[4] != 0,
        length_to_beat: q[5] as usize,
    }
}
fn compare_search(search: &OptimalSearch<'_>, expected: &[usize], context: (&str, usize, usize)) {
    let (heads, nodes) = search.tree.test_maps();
    let short = search.short.test_cells();
    assert_eq!(heads, &expected[..heads.len()], "{context:?} heads");
    let at = heads.len();
    assert_eq!(nodes, &expected[at..at + nodes.len()], "{context:?} nodes");
    let at = at + nodes.len();
    assert_eq!(
        short,
        &expected[at..at + short.len()],
        "{context:?} short keys"
    );
    assert_eq!(
        [search.frontier, search.short_frontier],
        expected[expected.len() - 2..],
        "{context:?} frontiers"
    );
}
#[test]
fn every_native_two_pass_query_and_complete_search_snapshot() {
    let mut queries = 0;
    let mut resets = 0;
    let mut stale_cases = 0;
    for case in fixture::read(fixture::PACK).unwrap() {
        let mut search = state(case.source, layout(&case), [Global; 4]).unwrap();
        search
            .validate_ultra2_start(
                case.source,
                if case.minimum == 3 { 3 } else { 4 },
                case.target as usize,
            )
            .unwrap();
        let mut expected = vec![usize::MAX; case.search_cells()];
        for (phase_index, phase) in case.phases.iter().enumerate() {
            if phase_index == 1 {
                let before = search.test_snapshot();
                search.rebase_ultra2().unwrap();
                let after = search.test_snapshot();
                assert_eq!(
                    before[..before.len() - 2],
                    after[..after.len() - 2],
                    "{} physical reset",
                    case.name
                );
                assert_eq!(after[after.len() - 2..], [phase.origin, phase.origin]);
                resets += 1;
                if after[..after.len() - 2]
                    .iter()
                    .any(|&v| v > 0 && v < phase.origin)
                {
                    stale_cases += 1;
                }
            }
            for (index, (event, changes)) in
                phase.events.iter().zip(&phase.search_changes).enumerate()
            {
                if let Some(q) = event.query {
                    let actual = search.query_ultra2(query(q)).unwrap_or_else(|e| {
                        panic!("{} phase {phase_index} query {q:?}: {e:?}", case.name)
                    });
                    assert!(
                        actual
                            .iter()
                            .map(|m| [m.off_base(), m.length() as u32])
                            .eq(event.matches.iter().copied()),
                        "{} phase {phase_index} query {q:?}",
                        case.name
                    );
                    queries += 1;
                }
                for &(at, value) in changes {
                    expected[at] = if value == 0 {
                        usize::MAX
                    } else {
                        (value - 2) as usize
                    };
                }
                if event.query.is_some() || index == 0 {
                    compare_search(&search, &expected, (case.name, phase_index, index));
                } else {
                    assert!(changes.is_empty());
                }
            }
        }
        compare_search(&search, &expected, (case.name, case.phases.len(), 0));
    }
    assert!(queries > 100000);
    assert_eq!(resets, 29);
    assert!(stale_cases >= 6);
}
#[test]
fn stale_source_reads_fail_before_access_even_when_tree_slots_alias() {
    let bytes = b"abacabadabacabaeabacabadabacabaeabacabadabacabaeabacabadabacabae";
    let mut source = OptimalSource::for_ultra2(bytes).unwrap();
    let key = crate::greedy_key::GreedyKeyLayout::new(6, 3).unwrap();
    let original_key = source.key(1, key).unwrap();
    let original_short = source.short_bucket(1, 10).unwrap();
    source.rebase_ultra2().unwrap();
    let origin = bytes.len();
    for old in 0..origin {
        assert!(source.byte(old).is_err());
        assert!(source.key(old, key).is_err());
        assert!(source.short_bucket(old, 10).is_err());
        assert!(source.match_length(origin + 1, old).is_err());
    }
    assert_eq!(source.key(origin + 1, key).unwrap(), original_key);
    assert_eq!(source.short_bucket(origin + 1, 10).unwrap(), original_short);
    assert_eq!(source.byte(origin).unwrap(), bytes[0]);
    assert!(source.byte(source.len()).is_err());
    assert!(source.key(source.len() - 1, key).is_err());
    let before = (source.origin(), source.len());
    assert!(source.rebase_ultra2().is_err());
    assert_eq!((source.origin(), source.len()), before);
    assert!(source.set_end(bytes.len()).is_err());
    source.restore_end();
    assert_eq!(source.origin(), 0);
    assert_eq!(source.byte(0).unwrap(), bytes[0]);
}
#[test]
fn invalid_phase_requests_are_transactional_and_whole_reset_restores_entry() {
    let bytes = [b'a'; 64];
    let l = OptimalSearchLayout::new(10, 6, 6, 2, 3, 128, Some(10)).unwrap();
    let mut s = state(&bytes, l, [Global; 4]).unwrap();
    s.query_ultra2(query([1, 1, 4, 8, 0, 3])).unwrap();
    for q in [
        [0, 1, 4, 8, 0, 3],
        [57, 1, 4, 8, 0, 3],
        [1, 0, 4, 8, 0, 3],
        [1, 1, 4, 65, 0, 3],
        [1, 1, 4, 8, 0, 4],
    ] {
        let before = s.test_snapshot();
        let matches = s.matches.as_slice().unwrap().to_vec();
        assert!(s.query_ultra2(query(q)).is_err());
        assert_eq!(s.test_snapshot(), before);
        assert_eq!(s.matches.as_slice().unwrap(), matches);
        assert!(!s.failed);
    }
    assert!(s.query_fresh(query([1, 1, 4, 8, 0, 3])).is_err());
    assert!(s.query(query([1, 1, 4, 8, 0, 3])).is_err());
    assert!(s.begin_retained(0, 64).is_err());
    s.rebase_ultra2().unwrap();
    let before = s.test_snapshot();
    assert!(s.rebase_ultra2().is_err());
    assert_eq!(s.test_snapshot(), before);
    s.query_ultra2(query([1, 1, 4, 8, 0, 3])).unwrap();
    s.reset();
    s.validate_ultra2_start(&bytes, 3, 128).unwrap();
    assert_eq!(s.source.origin(), 0);
    assert!(
        s.test_snapshot()[..s.test_snapshot().len() - 2]
            .iter()
            .all(|&v| v == 0)
    );
    let separate = [b'a'; 64];
    assert!(s.validate_ultra2_start(&separate, 3, 128).is_err());
    assert!(s.validate_ultra2_start(&bytes, 4, 128).is_err());
}
#[test]
fn both_passes_and_reset_retain_four_distinct_nonclone_search_owners() {
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    let owners = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let bytes = [b'a'; 64];
    let l = OptimalSearchLayout::new(10, 6, 6, 2, 3, 128, Some(10)).unwrap();
    let mut s = state(&bytes, l, owners).unwrap();
    for m in &monitors {
        assert_eq!(m.snapshot().attempts, 1);
        assert!(m.fail_next());
    }
    for _ in 0..3 {
        s.query_ultra2(query([1, 1, 4, 8, 0, 3])).unwrap();
        s.rebase_ultra2().unwrap();
        s.query_ultra2(query([1, 1, 4, 8, 0, 3])).unwrap();
        s.reset();
    }
    for m in &monitors {
        assert_eq!(m.snapshot().attempts, 1);
        assert_eq!(m.snapshot().owners_dropped, 0);
    }
    drop(s);
    for m in monitors {
        m.assert_released();
        assert_eq!(m.snapshot().owners_dropped, 1);
    }
}

#[test]
fn corrupt_live_links_poison_the_second_pass_until_whole_reset() {
    let bytes = [b'a'; 64];
    let l = OptimalSearchLayout::new(10, 6, 6, 2, 3, 128, Some(10)).unwrap();
    let mut s = state(&bytes, l, [Global; 4]).unwrap();
    s.query_ultra2(query([1, 1, 4, 8, 0, 3])).unwrap();
    s.rebase_ultra2().unwrap();
    let origin = s.source.origin();
    let key = s.source.key(origin, l.tree().key()).unwrap();
    s.tree.replace_head(key, origin + 9).unwrap();
    assert_eq!(
        s.query_ultra2(query([8, 1, 4, 8, 0, 3])),
        Err(Error::CorruptData)
    );
    assert!(s.failed);
    assert!(s.rebase_ultra2().is_err());
    assert_eq!(
        s.query_ultra2(query([1, 1, 4, 8, 0, 3])),
        Err(Error::CorruptData)
    );
    s.reset();
    s.validate_ultra2_start(&bytes, 3, 128).unwrap();
    s.query_ultra2(query([1, 1, 4, 8, 0, 3])).unwrap();
}
