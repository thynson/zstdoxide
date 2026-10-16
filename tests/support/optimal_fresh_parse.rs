//! Full ordinary fresh parser comparisons and composite owner contracts.
use super::*;
use crate::optimal_path_fixture as fixture;
use crate::optimal_path_layout::PATH_CAPACITY;
use crate::optimal_search_layout::OptimalSearchLayout;
use crate::optimal_short_key::OptimalShortKey;
use crate::optimal_startup_fixture as startup;
use crate::reference_tree::ReferenceTree;
use alloc::{alloc::Global, vec, vec::Vec};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
struct Compare<'a> {
    case: &'a fixture::Case<'a>,
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
        let expected = &self.case.events[self.index];
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

fn layout(minimum: u8, target: usize) -> OptimalSearchLayout {
    OptimalSearchLayout::new(10, 6, 6, 1, minimum, target, (minimum == 3).then_some(10)).unwrap()
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
fn every_ordinary_c_path_frequency_query_and_sequence_matches() {
    let mut events = 0;
    let mut sequences = 0;
    for case in startup::read(startup::PACK).unwrap() {
        let l = OptimalSearchLayout::new(
            case.window,
            case.hash,
            case.chain,
            case.search,
            case.path.minimum as u8,
            case.path.target as usize,
            (case.path.minimum == 3).then_some(case.window),
        )
        .unwrap();
        let (mut parser, mut search) = pair(
            case.path.source,
            l,
            case.path.minimum as u8,
            case.path.target as usize,
            case.path.literal_mode == 1,
            [Global; 7],
        )
        .unwrap();
        let mut compare = Compare {
            case: &case.path,
            index: 0,
            cells: vec![PathCell::ZERO; PATH_CAPACITY],
            actual: vec![PathCell::ZERO; PATH_CAPACITY],
            frequencies: [0; 386],
        };
        let result = parser.parse_fresh(&mut search, &mut compare).unwrap();
        assert_eq!(compare.index, case.path.events.len(), "{}", case.path.name);
        let mut final_search = vec![0; case.search_cells()];
        for changes in &case.search_changes {
            for &(at, value) in changes {
                final_search[at] = value;
            }
        }
        assert_eq!(
            search.test_snapshot(),
            final_search,
            "{} final search",
            case.path.name
        );
        let last = case.path.events.last().unwrap();
        assert_eq!(result.tail, last.tail.unwrap() as usize);
        assert_eq!(result.repeats, last.repeats);
        events += compare.index;
        sequences += result.sequences;
    }
    assert_eq!(events, 4409);
    assert_eq!(sequences, 22);
}
#[derive(Default)]
struct Observe {
    events: usize,
    sequences: usize,
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
        self.sequences += usize::from(event.appended.is_some());
        if self.fail_stage == Some(event.stage) {
            Err(Error::ResourceLimit)
        } else {
            Ok(())
        }
    }
}
#[test]
fn fresh_pair_rejects_source_parameters_and_used_search_before_parser_mutation() {
    let source = [b'a'; 64];
    let other = [b'a'; 64];
    for (m, t) in [(3, 0), (4, 1)] {
        let (mut parser, _) = pair(&source, layout(4, 0), 4, 0, false, [Global; 7]).unwrap();
        let (_, mut search) = pair(&source, layout(m, t), m, t, false, [Global; 7]).unwrap();
        let before = search.test_snapshot();
        let mut observe = Observe::default();
        assert_eq!(
            parser.parse_fresh(&mut search, &mut observe),
            Err(Error::InvalidParameter)
        );
        assert_eq!(observe.events, 0);
        assert_eq!(parser.status, Status::Ready);
        assert_eq!(search.test_snapshot(), before);
    }
    let (mut parser, mut search) = pair(&source, layout(4, 0), 4, 0, false, [Global; 7]).unwrap();
    let (_, mut other_search) = pair(&other, layout(4, 0), 4, 0, false, [Global; 7]).unwrap();
    assert_eq!(
        parser.parse_fresh(&mut other_search, &mut Observe::default()),
        Err(Error::InvalidParameter)
    );
    search
        .query_fresh(OptimalQuery {
            position: 1,
            repeats: [1, 4, 8],
            zero_literals: false,
            length_to_beat: 4,
        })
        .unwrap();
    assert_eq!(
        parser.parse_fresh(&mut search, &mut Observe::default()),
        Err(Error::InvalidState)
    );
    assert_eq!(parser.status, Status::Ready);
    search.reset();
    parser
        .parse_fresh(&mut search, &mut Observe::default())
        .unwrap();
}
#[test]
fn fresh_observer_failures_require_both_resets_and_preserve_external_output() {
    let source = [b'a'; 64];
    for stage in [0, 1, 6, 8, 9, 10, 11] {
        let (mut parser, mut search) =
            pair(&source, layout(4, 0), 4, 0, false, [Global; 7]).unwrap();
        let mut observe = Observe {
            fail_stage: Some(stage),
            ..Observe::default()
        };
        assert_eq!(
            parser.parse_fresh(&mut search, &mut observe),
            Err(Error::ResourceLimit)
        );
        assert_eq!(parser.status, Status::Poisoned);
        let emitted = observe.sequences;
        assert_eq!(emitted, usize::from(stage >= 9));
        search.reset();
        assert_eq!(
            parser.parse_fresh(&mut search, &mut Observe::default()),
            Err(Error::InvalidState)
        );
        parser.reset().unwrap();
        let result = parser
            .parse_fresh(&mut search, &mut Observe::default())
            .unwrap();
        assert_eq!(result.sequences, 1);
        assert_eq!(observe.sequences, emitted);
        parser.reset().unwrap();
        assert_eq!(
            parser.parse_fresh(&mut search, &mut Observe::default()),
            Err(Error::InvalidState)
        );
        search.reset();
        parser
            .parse_fresh(&mut search, &mut Observe::default())
            .unwrap();
    }
}
#[test]
fn fresh_moved_nonclone_owner_failures_release_every_allocation() {
    let source = [b'a'; 64];
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
        let result = pair(&source, layout(3, 0), 3, 0, false, owners);
        if rejected < 7 {
            assert!(matches!(result, Err(Error::AllocationFailed)));
        } else {
            let (mut parser, mut search) = result.unwrap();
            let before = monitors.each_ref().map(|m| m.snapshot().attempts);
            for _ in 0..2 {
                assert_eq!(
                    parser
                        .parse_fresh(&mut search, &mut Observe::default())
                        .unwrap()
                        .sequences,
                    1
                );
                parser.reset().unwrap();
                search.reset();
            }
            assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), before);
            drop((parser, search));
        }
        for monitor in monitors {
            monitor.assert_released();
            assert_eq!(monitor.snapshot().owners_dropped, 1);
        }
    }
}
#[test]
fn fresh_borrowed_owners_never_allocate_during_parse_or_reset() {
    let source = [b'a'; 64];
    let owners: [_; 7] = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let (mut parser, mut search) =
        pair(&source, layout(3, 0), 3, 0, false, owners.each_ref()).unwrap();
    let before = monitors.each_ref().map(|m| m.snapshot().attempts);
    assert_eq!(before, [1; 7]);
    for _ in 0..3 {
        assert_eq!(
            parser
                .parse_fresh(&mut search, &mut Observe::default())
                .unwrap()
                .sequences,
            1
        );
        parser.reset().unwrap();
        search.reset();
    }
    assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), before);
    drop((parser, search));
    for m in &monitors {
        m.assert_released();
        assert_eq!(m.snapshot().owners_dropped, 0);
    }
    drop(owners);
    for m in monitors {
        m.assert_released();
        assert_eq!(m.snapshot().owners_dropped, 1);
    }
}
