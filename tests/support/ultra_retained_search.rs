use super::*;
use crate::ultra_retained_fixture as fixture;
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
fn every_retained_ultra_query_map_and_frontier_matches_without_future_bytes() {
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
    assert_eq!(queries, 102307);
}
