//! Retained parser state, publication and independent allocator lifecycle checks.
use super::*;
use crate::RawCompressionParameters;
use crate::optimal_frequencies::OptimalFrequencies;
use crate::optimal_matches::OptimalMatches;
use crate::optimal_parse::{PathEvent, PathObserver};
use crate::optimal_path_layout::{PATH_CAPACITY, PathCell};
use crate::optimal_paths::OptimalPaths;
use crate::optimal_search_layout::{OptimalMatch, OptimalSearchLayout};
use crate::optimal_short_key::OptimalShortKey;
use crate::reference_tree::ReferenceTree;
use crate::ultra_retained_fixture as fixture;
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
        assert_eq!(event.alternative, expected.alternative, "{context:?}");
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
    policy: PricePolicy,
    l: OptimalSearchLayout,
    minimum: u8,
    target: usize,
    disabled: bool,
    owners: [A; 7],
) -> Result<Pair<'_, A>> {
    let [f, p, m, h, n, s, b] = owners;
    let constructor = match policy {
        PricePolicy::Optimal => OptimalParser::from_parts,
        PricePolicy::Ultra => OptimalParser::from_ultra_retained_parts,
    };
    let parser = constructor(
        source,
        minimum,
        target,
        OptimalFrequencies::new_with_policy_in(policy, disabled, f)?,
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
fn bound_plan(case: &fixture::Case<'_>) -> RetainedOptimalPlan {
    let p = RawCompressionParameters {
        window_log: case.window.into(),
        hash_log: case.hash.into(),
        chain_log: case.chain.into(),
        search_log: case.search.into(),
        min_match: case.minimum,
        target_length: case.target,
        strategy: if case.policy == 2 { 8 } else { 7 },
    }
    .validate()
    .unwrap();
    let blocks: Vec<_> = case.blocks.iter().map(|b| b.length).collect();
    let constructor = if case.policy == 2 {
        RetainedOptimalPlan::new_ultra
    } else {
        RetainedOptimalPlan::new
    };
    constructor(p, case.source.len(), &blocks, case.literal_mode == 1).unwrap()
}
fn state<'a>(case: &fixture::Case<'_>, source: &'a [u8]) -> RetainedOptimal<'a> {
    let plan = bound_plan(case);
    let (p, s) = pair(
        source,
        plan.policy(),
        plan.layout(),
        case.minimum as u8,
        case.target as usize,
        case.literal_mode == 1,
        [Global; 7],
    )
    .unwrap();
    if case.policy == 2 {
        RetainedOptimal::from_ultra_parts(p, s, plan)
    } else {
        RetainedOptimal::from_parts(p, s, plan.blocks())
    }
    .unwrap()
}
fn compare<'a>(case: &'a fixture::Case<'a>) -> Compare<'a> {
    Compare {
        case,
        block: 0,
        index: 0,
        cells: vec![PathCell::ZERO; PATH_CAPACITY],
        actual: vec![PathCell::ZERO; PATH_CAPACITY],
        frequencies: [0; 386],
    }
}
fn check_prefix(
    state: &mut RetainedOptimal<'_>,
    case: &fixture::Case<'_>,
    count: usize,
) -> (usize, usize, usize) {
    let mut compare = compare(case);
    let mut expected_search = vec![0; case.search_cells()];
    let mut events = 0;
    let mut sequences = 0;
    let mut alternatives = 0;
    for (index, block) in case.blocks.iter().take(count).enumerate() {
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
        alternatives += block
            .events
            .iter()
            .filter(|e| e.alternative.is_some())
            .count();
    }
    (events, sequences, alternatives)
}
#[test]
fn every_retained_policy_state_sequence_alternative_and_publication_matches_native() {
    let mut totals = (0, 0, 0);
    let mut blocks = 0;
    for case in fixture::read(fixture::PACK).unwrap() {
        let mut state = state(&case, case.source);
        let counts = check_prefix(&mut state, &case, case.blocks.len());
        totals.0 += counts.0;
        totals.1 += counts.1;
        totals.2 += counts.2;
        blocks += case.blocks.len();
        assert!(state.is_finished());
    }
    assert_eq!(totals, (207635, 543, 530));
    assert_eq!(blocks, 134);
}
#[test]
fn every_unconsumed_suffix_is_isolated_from_all_completed_ultra_states() {
    let mut boundaries = 0;
    for case in fixture::read(fixture::PACK)
        .unwrap()
        .into_iter()
        .filter(|c| c.policy == 2)
    {
        for count in 1..case.blocks.len() {
            let end = case.blocks[count].start as usize;
            let mut changed = case.source.to_vec();
            for byte in &mut changed[end..] {
                *byte ^= 0xff;
            }
            assert_ne!(changed, case.source);
            let mut actual = state(&case, &changed);
            check_prefix(&mut actual, &case, count);
            assert_eq!(actual.position(), end);
            assert!(!actual.is_finished());
            boundaries += 1;
        }
    }
    assert_eq!(boundaries, 46);
}

struct FailAlternative;
impl PathObserver for FailAlternative {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        _: &OptimalPaths<P>,
        _: &OptimalFrequencies<F>,
        _: &[OptimalMatch],
    ) -> Result<()> {
        if event.alternative.is_some() {
            Err(Error::ResourceLimit)
        } else {
            Ok(())
        }
    }
}
#[test]
fn later_block_alternative_failure_requires_full_reset_and_exact_replay() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let mut failures = 0;
    for case in cases.iter().filter(|c| c.policy == 2) {
        for index in 1..case.blocks.len() {
            if !case.blocks[index]
                .events
                .iter()
                .any(|e| e.alternative.is_some())
            {
                continue;
            }
            let mut actual = state(case, case.source);
            check_prefix(&mut actual, case, index);
            assert_eq!(
                actual.parse_next(&mut FailAlternative),
                Err(Error::ResourceLimit)
            );
            assert_eq!(actual.phase, Phase::Poisoned);
            assert_eq!(actual.pending_result(), Err(Error::InvalidState));
            assert_eq!(actual.publish(Publication::Raw), Err(Error::InvalidState));
            actual.reset().unwrap();
            check_prefix(&mut actual, case, case.blocks.len());
            assert!(actual.is_finished());
            failures += 1;
        }
    }
    assert!(failures >= 4);
}
