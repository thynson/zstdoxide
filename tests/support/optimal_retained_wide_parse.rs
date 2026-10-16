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

fn wide_layout() -> OptimalSearchLayout {
    OptimalSearchLayout::new(16, 12, 12, 6, 3, 0, Some(16)).unwrap()
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
fn wider_retained_moved_nonclone_owner_failures_release_every_allocation() {
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    let source = [b'a'; 16384];
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
        let result =
            pair(&source, wide_layout(), 3, 0, false, owners).and_then(|(parser, search)| {
                RetainedOptimal::from_parts(parser, search, &[8192, 8192])
            });
        if rejected < 7 {
            assert!(matches!(result, Err(Error::AllocationFailed)));
        } else {
            let mut state = result.unwrap();
            let before = monitors.each_ref().map(|m| m.snapshot().attempts);
            for _ in 0..2 {
                for kind in [Publication::Compressed, Publication::Rle] {
                    assert_eq!(
                        state.parse_next(&mut Observe::default()).unwrap().sequences,
                        1
                    );
                    state.publish(kind).unwrap();
                }
                state.reset().unwrap();
            }
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
fn wider_retained_borrowed_owners_reuse_pending_failed_and_finished_storage() {
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    let source = [b'a'; 16384];
    let owners: [_; 7] = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let (parser, search) = pair(&source, wide_layout(), 3, 0, false, owners.each_ref()).unwrap();
    let mut state = RetainedOptimal::from_parts(parser, search, &[8192, 8192]).unwrap();
    let before = monitors.each_ref().map(|m| m.snapshot().attempts);
    assert_eq!(before, [1; 7]);
    for _ in 0..2 {
        state.parse_next(&mut Observe::default()).unwrap();
        assert_eq!(state.phase, Phase::Pending);
        state.reset().unwrap();
        assert!(state.pending.is_none());
        assert_eq!(state.owners, [[1, 4, 8], [0, 0, 0]]);
        let mut fail = Observe {
            fail_stage: Some(9),
            ..Observe::default()
        };
        assert_eq!(state.parse_next(&mut fail), Err(Error::ResourceLimit));
        state.reset().unwrap();
        for kind in [Publication::Raw, Publication::Compressed] {
            state.parse_next(&mut Observe::default()).unwrap();
            state.publish(kind).unwrap();
        }
        assert!(state.is_finished());
        state.reset().unwrap();
    }
    assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), before);
    drop(state);
    for monitor in &monitors {
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
    }
    drop(owners);
    for monitor in monitors {
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
}

#[test]
fn rejected_wider_retained_schedule_releases_all_moved_parts() {
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    let source = [b'a'; 16384];
    let owners: [_; 7] = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let (parser, search) = pair(&source, wide_layout(), 3, 0, false, owners).unwrap();
    assert!(matches!(
        RetainedOptimal::from_parts(parser, search, &[16384]),
        Err(Error::InvalidParameter)
    ));
    for monitor in monitors {
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
}

#[test]
fn wide_schedule_and_source_limits_reject_and_release_every_owner() {
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    let source = [b'a'; 16385];
    for (size, schedule) in [
        (16384, &[8193, 8191][..]),
        (16384, &[8192, 8193][..]),
        (16384, &[8192, 8129, 63][..]),
        (16384, &[4096, 4096, 4096, 4032, 64][..]),
        (16385, &[8192, 8192][..]),
        (127, &[64, 63][..]),
    ] {
        let owners: [_; 7] = core::array::from_fn(|_| {
            TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 1,
                fail_at: None,
            })
            .unwrap()
            .into_non_clone()
        });
        let monitors = owners.each_ref().map(|a| a.monitor());
        let (parser, search) = pair(&source[..size], wide_layout(), 3, 0, false, owners).unwrap();
        assert!(matches!(
            RetainedOptimal::from_parts(parser, search, schedule),
            Err(Error::InvalidParameter)
        ));
        for monitor in monitors {
            monitor.assert_released();
            assert_eq!(monitor.snapshot().owners_dropped, 1);
        }
    }
}
