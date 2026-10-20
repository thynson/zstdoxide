use super::*;
use crate::optimal_retained_frames_fixture as fixture;
use alloc::{vec, vec::Vec};
fn state(source: &[u8], layout: OptimalSearchLayout) -> OptimalSearch<'_> {
    OptimalSearch::from_parts(
        source,
        layout,
        ReferenceTree::new_in(layout.tree(), Global, Global).unwrap(),
        OptimalShortKey::new_in(layout.short_log(), Global).unwrap(),
        OptimalMatches::new_in(Global).unwrap(),
    )
    .unwrap()
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
fn every_larger_retained_query_map_and_frontier_matches_without_future_bytes() {
    let mut queries = 0;
    for case in fixture::read(fixture::PACK).unwrap() {
        let layout = OptimalSearchLayout::new(
            case.window,
            case.hash,
            case.chain,
            case.search,
            case.minimum as u8,
            case.target as usize,
            (case.minimum == 3).then_some(case.window),
        )
        .unwrap();
        let mut actual = state(case.source, layout);
        let mut expected = vec![0; case.search_cells()];
        for block in &case.blocks {
            actual
                .begin_retained(block.start as usize, block.start as usize + block.length)
                .unwrap();
            assert_eq!(actual.source.len(), block.start as usize + block.length);
            for (index, event) in block.events.iter().enumerate() {
                if let Some(q) = event.query {
                    assert_eq!(
                        actual.test_snapshot(),
                        expected,
                        "{} before {index}",
                        case.name
                    );
                    let candidates: Vec<_> = actual
                        .query_retained(query(q))
                        .unwrap()
                        .iter()
                        .map(|m| [m.off_base(), m.length() as u32])
                        .collect();
                    assert_eq!(candidates, event.matches, "{} query {index}", case.name);
                    queries += 1;
                }
                for &(at, value) in &block.search_changes[index] {
                    expected[at] = value;
                }
                assert_eq!(
                    actual.test_snapshot(),
                    expected,
                    "{} block {} event {index}",
                    case.name,
                    block.start
                );
            }
        }
    }
    assert_eq!(queries, 33122);
}

#[test]
fn larger_retained_bounds_reject_before_mode_or_visibility_mutation() {
    let source = [b'a'; 32769];
    for (size, w, h, c, attempts, minimum, target, short) in [
        (127, 14, 12, 12, 6, 4, 0, None),
        (32769, 16, 12, 12, 6, 4, 0, None),
        (16384, 13, 12, 12, 6, 4, 0, None),
        (16384, 17, 12, 12, 6, 4, 0, None),
        (16384, 14, 13, 12, 6, 4, 0, None),
        (16384, 14, 12, 13, 6, 4, 0, None),
        (16384, 14, 12, 12, 7, 4, 0, None),
        (16384, 14, 12, 12, 6, 4, 4096, None),
        (16384, 14, 12, 12, 6, 3, 0, Some(13)),
    ] {
        let layout = OptimalSearchLayout::new(w, h, c, attempts, minimum, target, short).unwrap();
        let mut search = state(&source[..size], layout);
        let before = search.test_snapshot();
        assert!(search.begin_retained(0, 64).is_err());
        assert_eq!(search.test_snapshot(), before);
        assert!(search.retained.is_none());
        assert_eq!(search.source.len(), size);
    }
    let layout = OptimalSearchLayout::new(14, 12, 12, 6, 4, 4095, None).unwrap();
    let mut search = state(&source[..16384], layout);
    let before = search.test_snapshot();
    for (start, end) in [
        (0, 8193),
        (0, 63),
        (1, 8192),
        (8192, 16384),
        (0, usize::MAX),
    ] {
        assert!(search.begin_retained(start, end).is_err());
        assert_eq!(search.test_snapshot(), before);
        assert!(search.retained.is_none());
    }
    search
        .validate_fresh_start(&source[..16384], 4, 4095)
        .unwrap();
    search.begin_retained(0, 8192).unwrap();
    assert!(
        search
            .validate_fresh_start(&source[..16384], 4, 4095)
            .is_err()
    );
}
#[test]
fn larger_retained_long_matches_cannot_observe_future_backing_bytes() {
    let source = [b'a'; 32768];
    let mut changed = source;
    changed[24576..].fill(b'b');
    for minimum in [3, 4] {
        let layout =
            OptimalSearchLayout::new(16, 12, 12, 6, minimum, 4095, (minimum == 3).then_some(16))
                .unwrap();
        let mut a = state(&source, layout);
        let mut b = state(&changed, layout);
        for start in [0, 8192] {
            a.begin_retained(start, start + 8192).unwrap();
            b.begin_retained(start, start + 8192).unwrap();
            let q = query([start.max(1) as u32, 1, 4, 8, 0, minimum.clamp(3, 4) as u32]);
            assert_eq!(a.query_retained(q).unwrap(), b.query_retained(q).unwrap());
        }
        a.begin_retained(16384, 24576).unwrap();
        b.begin_retained(16384, 24576).unwrap();
        let q = query([16384, 1, 4, 8, 0, minimum.clamp(3, 4) as u32]);
        assert_eq!(a.query_retained(q).unwrap(), b.query_retained(q).unwrap());
        assert_eq!(a.matches.as_slice().unwrap().last().unwrap().length(), 8192);
        assert_eq!(a.test_snapshot(), b.test_snapshot());
        assert!(a.source.byte(24576).is_err());
        assert!(a.source.key(24575, layout.tree().key()).is_err());
        assert!(a.source.short_bucket(24574, 16).is_err());
        assert_eq!(a.source.match_length(16384, 16383).unwrap(), 8192);
    }
}
