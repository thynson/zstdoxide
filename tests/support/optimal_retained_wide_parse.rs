//! Retained parser state, publication and independent allocator lifecycle checks.
use super::*;
use crate::optimal_frequencies::OptimalFrequencies;
use crate::optimal_matches::OptimalMatches;
use crate::optimal_parse::{PathEvent, PathObserver};
use crate::optimal_path_layout::{PATH_CAPACITY, PathCell};
use crate::optimal_paths::OptimalPaths;
use crate::optimal_retained_wide_fixture as fixture;
use crate::optimal_search_layout::{OptimalMatch, OptimalSearchLayout};
use crate::optimal_short_key::OptimalShortKey;
use crate::reference_tree::ReferenceTree;
use alloc::{alloc::Global, vec, vec::Vec};
struct Compare<'a> {
    case: &'a fixture::Case<'a>,
    block: usize,
    index: usize,
    cells: Vec<PathCell>,
    actual: Vec<PathCell>,
    frequencies: [u32; 386],
}
impl PathObserver for Compare<'_> {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        paths: &OptimalPaths<P>,
        frequencies: &OptimalFrequencies<F>,
        matches: &[OptimalMatch],
    ) -> Result<()> {
        let expected = &self.case.blocks[self.block].events[self.index];
        let context = (self.case.name, self.index, event.stage);
        assert_eq!(
            event.stage,
            u8::try_from(expected.stage).unwrap(),
            "{context:?}"
        );
        assert_eq!(event.reverse_phase, expected.reverse_phase, "{context:?}");
        assert_eq!(
            [event.ip, event.anchor],
            [expected.ip as usize, expected.anchor as usize],
            "{context:?}"
        );
        assert_eq!(
            [event.cur, event.last, event.tail],
            [
                expected.cur.map(|v| v as usize),
                expected.last.map(|v| v as usize),
                expected.tail.map(|v| v as usize)
            ],
            "{context:?}"
        );
        assert_eq!(event.repeats, expected.repeats, "{context:?}");
        assert_eq!(
            event.stretch.map(PathCell::words),
            expected.stretch,
            "{context:?}"
        );
        assert_eq!(
            event.reverse,
            expected.reverse.map(|r| r.map(|v| v as usize)),
            "{context:?}"
        );
        assert_eq!(
            event.query.map(|q| [
                q.position as u32,
                q.repeats[0],
                q.repeats[1],
                q.repeats[2],
                u32::from(q.zero_literals),
                q.length_to_beat as u32
            ]),
            expected.query,
            "{context:?}"
        );
        assert_eq!(
            matches
                .iter()
                .map(|m| [m.off_base(), m.length() as u32])
                .collect::<Vec<_>>(),
            expected.matches,
            "{context:?}"
        );
        assert_eq!(event.sequences, expected.sequence_count, "{context:?}");
        assert_eq!(event.appended.as_slice(), expected.appended, "{context:?}");
        for &(at, words) in &expected.path {
            self.cells[at] = PathCell::from_words(words).unwrap();
        }
        paths.snapshot(&mut self.actual)?;
        assert_eq!(self.actual, self.cells, "{context:?}");
        for &(at, value) in &expected.frequencies {
            self.frequencies[at] = value;
        }
        let snapshot = frequencies.snapshot()?;
        assert_eq!(snapshot.counts, self.frequencies[..377], "{context:?}");
        assert_eq!(snapshot.sums, self.frequencies[377..381], "{context:?}");
        assert_eq!(
            snapshot.base_prices,
            self.frequencies[381..385],
            "{context:?}"
        );
        assert_eq!(
            u32::from(snapshot.predefined),
            self.frequencies[385],
            "{context:?}"
        );
        assert!(snapshot.initialized);
        self.index += 1;
        Ok(())
    }
}

type Pair<'a, A> = (OptimalParser<'a, A, A, A>, OptimalSearch<'a, A, A, A, A>);
fn pair<A: Allocator>(
    source: &[u8],
    l: OptimalSearchLayout,
    minimum: u8,
    target: usize,
    disabled: bool,
    owners: [A; 7],
) -> Result<Pair<'_, A>> {
    let [f, p, m, h, n, s, b] = owners;
    let parser = OptimalParser::from_parts(
        source,
        minimum,
        target,
        OptimalFrequencies::new_in(disabled, f)?,
        OptimalPaths::new_in(p)?,
        OptimalMatches::new_in(m)?,
    )?;
    let search = OptimalSearch::from_parts(
        source,
        l,
        ReferenceTree::new_in(l.tree(), h, n)?,
        OptimalShortKey::new_in(l.short_log(), s)?,
        OptimalMatches::new_in(b)?,
    )?;
    Ok((parser, search))
}
#[test]
fn every_wider_retained_c_path_frequency_sequence_and_publication_matches() {
    let mut events = 0;
    let mut sequences = 0;
    for case in fixture::read(fixture::PACK).unwrap() {
        let l = OptimalSearchLayout::new(
            case.window,
            case.hash,
            case.chain,
            case.search,
            case.minimum as u8,
            case.target as usize,
            (case.minimum == 3).then_some(case.window),
        )
        .unwrap();
        let (parser, search) = pair(
            case.source,
            l,
            case.minimum as u8,
            case.target as usize,
            case.literal_mode == 1,
            [Global; 7],
        )
        .unwrap();
        let blocks: Vec<_> = case.blocks.iter().map(|b| b.length).collect();
        let mut state = RetainedOptimal::from_parts(parser, search, &blocks).unwrap();
        let mut compare = Compare {
            case: &case,
            block: 0,
            index: 0,
            cells: vec![PathCell::ZERO; PATH_CAPACITY],
            actual: vec![PathCell::ZERO; PATH_CAPACITY],
            frequencies: [0; 386],
        };
        let mut expected_search = vec![0; case.search_cells()];
        for (index, block) in case.blocks.iter().enumerate() {
            compare.block = index;
            compare.index = 0;
            assert_eq!(state.previous, block.previous_before);
            assert_eq!(state.owners[state.previous], block.incoming);
            let result = state.parse_next(&mut compare).unwrap();
            assert_eq!(
                result.repeats, block.candidate,
                "{} block {index}",
                case.name
            );
            assert_eq!(result.tail, block.tail as usize);
            assert_eq!(compare.index, block.events.len());
            assert_eq!(state.phase, Phase::Pending);
            assert_eq!(state.previous, block.previous_before);
            assert_eq!(state.owners[state.previous], block.incoming);
            for changes in &block.search_changes {
                for &(at, value) in changes {
                    expected_search[at] = value;
                }
            }
            assert_eq!(
                state.search.test_snapshot(),
                expected_search,
                "{} block {index}",
                case.name
            );
            state
                .publish(Publication::try_from(block.kind as u8).unwrap())
                .unwrap();
            assert_eq!(state.previous, block.previous_after);
            assert_eq!(state.owners, block.owners);
            events += compare.index;
            sequences += result.sequences;
        }
        assert!(state.is_finished());
    }
    assert_eq!(events, 37977);
    assert_eq!(sequences, 31);
}
