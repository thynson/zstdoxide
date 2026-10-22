//! Complete independent native phase comparison and scoped ownership execution.
use super::*;
use crate::RawCompressionParameters;
use crate::optimal_matches::OptimalMatches;
use crate::optimal_path_layout::{PATH_CAPACITY, PathCell};
use crate::optimal_price::PricePolicy;
use crate::optimal_short_key::OptimalShortKey;
use crate::reference_tree::ReferenceTree;
use crate::ultra2_plan::Ultra2Coordinates;
use crate::ultra2_retained_fixture as fixture;
use alloc::{vec, vec::Vec};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
fn words(f: FrequencySnapshot) -> [u32; 386] {
    let mut result = [0; 386];
    result[..377].copy_from_slice(&f.counts);
    result[377..381].copy_from_slice(&f.sums);
    result[381..385].copy_from_slice(&f.base_prices);
    result[385] = u32::from(f.predefined);
    result
}
struct Compare<'a> {
    case: &'a fixture::Case<'a>,
    block: usize,
    pass: usize,
    index: usize,
    cells: Vec<PathCell>,
    actual: Vec<PathCell>,
    frequencies: [u32; 386],
}
impl Ultra2Observer for Compare<'_> {
    fn entry(&mut self, e: Ultra2Eligibility) -> Result<()> {
        let expected = self.case.blocks[self.block].eligibility;
        let predicates = e.predicates()?;
        assert_eq!(predicates, core::array::from_fn(|i| expected[i] == 1));
        assert_eq!(
            [
                u32::from(e.eligible()?),
                e.current as u32 + 2,
                e.dictionary_limit as u32 + 2,
                e.low_limit as u32 + 2,
                e.next_to_update as u32 + 2,
                e.source_len as u32
            ],
            [
                expected[5],
                expected[6],
                expected[7],
                expected[8],
                expected[9],
                expected[10]
            ],
            "{} block{}",
            self.case.name,
            self.block
        );
        assert_eq!(e.sequence_count, 0);
        Ok(())
    }
    fn begin(&mut self, p: Ultra2Phase, c: Ultra2Coordinates, f: FrequencySnapshot) -> Result<()> {
        let phase = &self.case.blocks[self.block].phases[self.pass];
        assert_eq!(p == Ultra2Phase::Seeding, phase.seeding);
        assert_eq!(c.origin() as u32 + 2, phase.origin);
        assert_eq!(
            c.len(),
            self.case.blocks[self.block].start as usize + self.case.blocks[self.block].length
        );
        assert_eq!(words(f), phase.initial_frequency);
        assert_eq!(self.frequencies, phase.initial_frequency);
        self.index = 0;
        Ok(())
    }
    fn end(
        &mut self,
        p: Ultra2Phase,
        _: Ultra2Coordinates,
        result: PathResult,
        literals: usize,
        f: FrequencySnapshot,
    ) -> Result<()> {
        let phase = &self.case.blocks[self.block].phases[self.pass];
        assert_eq!(self.index, phase.events.len());
        assert_eq!(result.tail, phase.tail as usize);
        assert_eq!(result.repeats, phase.candidate);
        assert_eq!(
            result.sequences,
            phase.events.last().unwrap().sequence_count
        );
        assert_eq!(
            literals,
            phase
                .events
                .iter()
                .flat_map(|e| &e.appended)
                .map(|row| row[1] as usize)
                .sum::<usize>()
        );
        assert_eq!(words(f), phase.final_frequency);
        assert_eq!(self.frequencies, phase.final_frequency);
        if p == Ultra2Phase::Encoding {
            self.block += 1;
            self.pass = 0;
        }
        Ok(())
    }
    fn reset(&mut self, r: Ultra2Reset) -> Result<()> {
        let expected = self.case.blocks[self.block].phases[self.pass]
            .reset
            .as_ref()
            .unwrap();
        assert_eq!(r.discarded.tail, expected.discarded_tail as usize);
        assert_eq!(r.discarded.sequences, expected.discarded_sequences as usize);
        assert_eq!(r.discarded.repeats, expected.discarded_repeats);
        assert_eq!(r.discarded_literals, expected.discarded_literals as usize);
        assert_eq!(r.original_repeats, expected.caller_repeats);
        assert_eq!(r.before.origin() as u32 + 2, expected.origin_before);
        assert_eq!(r.after.origin() as u32 + 2, expected.origin_after);
        assert_eq!(words(r.frequencies), expected.frequency);
        self.pass += 1;
        Ok(())
    }
}
fn bound_plan(case: &fixture::Case<'_>) -> RetainedUltra2Plan {
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
type Pair<'a, A> = (OptimalParser<'a, A, A, A>, OptimalSearch<'a, A, A, A, A>);
fn pair<A: Allocator>(
    source: &[u8],
    plan: RetainedUltra2Plan,
    owners: [A; 7],
) -> Result<Pair<'_, A>> {
    let [f, p, m, h, n, s, b] = owners;
    let l = plan.layout();
    let parser = OptimalParser::from_ultra2_retained_parts(
        source,
        plan.parameters().min_match(),
        plan.parameters().target_length() as usize,
        OptimalFrequencies::new_with_policy_in(PricePolicy::Ultra, plan.disabled_literals(), f)?,
        OptimalPaths::new_in(p)?,
        OptimalMatches::new_in(m)?,
    )?;
    let search = OptimalSearch::from_ultra2_retained_parts(
        source,
        plan,
        ReferenceTree::new_in(l.tree(), h, n)?,
        OptimalShortKey::new_in(l.short_log(), s)?,
        OptimalMatches::new_in(b)?,
    )?;
    Ok((parser, search))
}
fn state<'a, A: Allocator>(
    source: &'a [u8],
    plan: RetainedUltra2Plan,
    owners: [A; 7],
) -> Result<RetainedUltra2<'a, A, A, A, A, A, A, A>> {
    let (p, s) = pair(source, plan, owners)?;
    RetainedUltra2::from_parts(p, s, plan)
}
#[test]
fn every_native_complete_parser_phase_reset_and_publication_state_matches() {
    let mut blocks = 0;
    let mut events = 0;
    for case in fixture::read(fixture::PACK).unwrap() {
        let p = bound_plan(&case);
        let mut actual = state(case.source, p, [Global; 7]).unwrap();
        let mut compare = Compare {
            case: &case,
            block: 0,
            pass: 0,
            index: 0,
            cells: vec![PathCell::ZERO; PATH_CAPACITY],
            actual: vec![PathCell::ZERO; PATH_CAPACITY],
            frequencies: [0; 386],
        };
        for block in &case.blocks {
            assert_eq!(actual.position(), block.start as usize);
            assert_eq!(actual.previous, block.previous_before);
            assert_eq!(actual.committed_repeats(), block.incoming);
            let pending = actual.parse_next(&mut compare).unwrap();
            assert_eq!(actual.pending_result(), Ok(pending));
            assert_eq!(pending.repeats, block.candidate);
            assert_eq!(pending.tail, block.tail as usize);
            assert_eq!(pending.sequences, block.sequence_count);
            actual
                .publish(Publication::try_from(block.kind as u8).unwrap())
                .unwrap();
            assert_eq!(actual.previous, block.previous_after);
            assert_eq!(actual.owners, block.owners);
            blocks += 1;
            events += block.phases.iter().map(|p| p.events.len()).sum::<usize>();
        }
        assert!(actual.is_finished());
        assert_eq!(compare.block, case.blocks.len());
    }
    assert_eq!((blocks, events), (93, 229657));
}
impl PathObserver for Compare<'_> {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        paths: &OptimalPaths<P>,
        frequencies: &OptimalFrequencies<F>,
        matches: &[OptimalMatch],
    ) -> Result<()> {
        let expected = &self.case.blocks[self.block].phases[self.pass].events[self.index];
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
#[derive(Default)]
struct Probe {
    seen: usize,
    fail_at: Option<usize>,
}
impl Probe {
    fn tick(&mut self) -> Result<()> {
        self.seen += 1;
        if self.fail_at == Some(self.seen) {
            Err(Error::ResourceLimit)
        } else {
            Ok(())
        }
    }
}
impl PathObserver for Probe {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        _: PathEvent,
        _: &OptimalPaths<P>,
        _: &OptimalFrequencies<F>,
        _: &[OptimalMatch],
    ) -> Result<()> {
        self.tick()
    }
}
impl Ultra2Observer for Probe {
    fn entry(&mut self, _: Ultra2Eligibility) -> Result<()> {
        self.tick()
    }
    fn begin(&mut self, _: Ultra2Phase, _: Ultra2Coordinates, _: FrequencySnapshot) -> Result<()> {
        self.tick()
    }
    fn end(
        &mut self,
        _: Ultra2Phase,
        _: Ultra2Coordinates,
        _: PathResult,
        _: usize,
        _: FrequencySnapshot,
    ) -> Result<()> {
        self.tick()
    }
    fn reset(&mut self, _: Ultra2Reset) -> Result<()> {
        self.tick()
    }
}
fn small_plan(n: usize) -> RetainedUltra2Plan {
    RetainedUltra2Plan::new(
        RawCompressionParameters {
            window_log: 10,
            hash_log: 6,
            chain_log: 6,
            search_log: 1,
            min_match: 3,
            target_length: 0,
            strategy: 9,
        }
        .validate()
        .unwrap(),
        n,
        &[n / 2, n / 2],
        false,
    )
    .unwrap()
}
#[test]
fn every_observer_boundary_failure_poisoning_and_borrowed_reset_preserve_owners() {
    let source = [b'a'; 194];
    let plan = small_plan(source.len());
    let mut reference = state(&source, plan, [Global; 7]).unwrap();
    let mut probe = Probe::default();
    let mut results = Vec::new();
    for _ in 0..2 {
        results.push(reference.parse_next(&mut probe).unwrap());
        reference.publish(Publication::Compressed).unwrap();
    }
    let final_search = reference.search.test_snapshot();
    let final_frequencies = reference.frequencies().unwrap();
    let final_owners = reference.owners;
    let owners: [_; 7] = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let mut actual = state(&source, plan, owners.each_ref()).unwrap();
    let attempts = monitors.each_ref().map(|m| m.snapshot().attempts);
    for fail_at in 1..=probe.seen {
        let mut failed = Probe {
            seen: 0,
            fail_at: Some(fail_at),
        };
        let mut reached = false;
        for _ in 0..2 {
            match actual.parse_next(&mut failed) {
                Ok(_) => actual.publish(Publication::Compressed).unwrap(),
                Err(e) => {
                    assert_eq!(e, Error::ResourceLimit);
                    reached = true;
                    break;
                }
            }
        }
        assert!(reached);
        assert_eq!(failed.seen, fail_at);
        assert_eq!(actual.phase, Phase::Poisoned);
        let mut untouched = Probe::default();
        assert_eq!(actual.parse_next(&mut untouched), Err(Error::InvalidState));
        assert_eq!(untouched.seen, 0);
        assert!(actual.pending_result().is_err());
        assert!(actual.publish(Publication::Raw).is_err());
        actual.reset().unwrap();
        assert_eq!(actual.position(), 0);
        assert_eq!(actual.committed_repeats(), [1, 4, 8]);
        for expected in &results {
            assert_eq!(actual.parse_next(&mut Probe::default()).unwrap(), *expected);
            actual.publish(Publication::Compressed).unwrap();
        }
        assert_eq!(actual.search.test_snapshot(), final_search);
        assert_eq!(actual.frequencies().unwrap(), final_frequencies);
        assert_eq!(actual.owners, final_owners);
        actual.reset().unwrap();
    }
    assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), attempts);
    drop(actual);
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
#[test]
fn seven_nonclone_owner_failures_and_actual_publication_kinds_are_isolated() {
    let source = [b'a'; 194];
    let plan = small_plan(source.len());
    for rejected in 0..=7 {
        let owners: [_; 7] = core::array::from_fn(|i| {
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
        let result = state(&source, plan, owners);
        if rejected < 7 {
            assert!(matches!(result, Err(Error::AllocationFailed)));
        } else {
            let mut actual = result.unwrap();
            let attempts = monitors.each_ref().map(|m| m.snapshot().attempts);
            for kind in [Publication::Raw, Publication::Rle, Publication::Compressed] {
                assert!(actual.publish(kind).is_err());
                let pending = actual.parse_next(&mut Probe::default()).unwrap();
                let mut uncalled = Probe::default();
                assert_eq!(actual.parse_next(&mut uncalled), Err(Error::InvalidState));
                assert_eq!(uncalled.seen, 0);
                actual.publish(kind).unwrap();
                assert_eq!(
                    actual.committed_repeats(),
                    if kind == Publication::Compressed {
                        pending.repeats
                    } else {
                        [1, 4, 8]
                    }
                );
                actual.parse_next(&mut Probe::default()).unwrap();
                actual.publish(Publication::Compressed).unwrap();
                assert!(actual.is_finished());
                assert!(actual.parse_next(&mut uncalled).is_err());
                actual.reset().unwrap();
            }
            assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), attempts);
            drop(actual);
        }
        for m in monitors {
            m.assert_released();
            assert_eq!(m.snapshot().owners_dropped, 1);
            assert!(!m.snapshot().violations.any());
        }
    }
}
#[test]
fn direct_invalid_phase_and_source_calls_preserve_parser_and_search() {
    let source = [b'a'; 194];
    let other = source;
    let plan = small_plan(source.len());
    let (mut p, mut s) = pair(&source, plan, [Global; 7]).unwrap();
    let (_, mut wrong) = pair(&other, plan, [Global; 7]).unwrap();
    let mut uncalled = Probe::default();
    assert_eq!(
        p.parse_ultra2_retained(0, 97, true, [1, 4, 8], &mut wrong, &mut uncalled),
        Err(Error::InvalidParameter)
    );
    assert_eq!(uncalled.seen, 0);
    for (start, end, seed, repeats) in [
        (0, 96, true, [1, 4, 8]),
        (1, 98, true, [1, 4, 8]),
        (97, 194, false, [1, 4, 8]),
        (0, 97, false, [1, 4, 8]),
        (0, 97, true, [0, 4, 8]),
        (0, 97, true, [2, 4, 8]),
        (usize::MAX, 97, true, [1, 4, 8]),
    ] {
        let before = s.test_snapshot();
        assert!(
            p.parse_ultra2_retained(start, end, seed, repeats, &mut s, &mut uncalled)
                .is_err()
        );
        assert_eq!(uncalled.seen, 0);
        assert_eq!(s.test_snapshot(), before);
        assert!(p.ultra2_retained_configuration().is_ok());
    }
    assert!(p.rebase_ultra2_retained(&mut s).is_err());
    p.parse_ultra2_retained(0, 97, true, [1, 4, 8], &mut s, &mut Probe::default())
        .unwrap();
    assert!(
        p.parse_ultra2_retained(0, 97, false, [1, 4, 8], &mut s, &mut uncalled)
            .is_err()
    );
    p.rebase_ultra2_retained(&mut s).unwrap();
    assert!(p.rebase_ultra2_retained(&mut s).is_err());
    p.parse_ultra2_retained(0, 97, false, [1, 4, 8], &mut s, &mut Probe::default())
        .unwrap();
    assert!(p.rebase_ultra2_retained(&mut s).is_err());
    p.parse_ultra2_retained(97, 194, false, [1, 4, 8], &mut s, &mut Probe::default())
        .unwrap();
    assert_eq!(uncalled.seen, 0);
}
#[test]
fn plan_source_scope_and_used_owner_mismatches_reject_composition() {
    let source = [b'a'; 194];
    let other = source;
    let plan = small_plan(source.len());
    let (mut p, s) = pair(&source, plan, [Global; 7]).unwrap();
    let (_, wrong) = pair(&other, plan, [Global; 7]).unwrap();
    assert!(RetainedUltra2::from_parts(p, wrong, plan).is_err());
    let (q, mut search) = pair(&source, plan, [Global; 7]).unwrap();
    p = q;
    search.begin_ultra2_retained(0, 97, true).unwrap();
    assert!(RetainedUltra2::from_parts(p, search, plan).is_err());
    drop(s);
    let (p, s) = pair(&source, plan, [Global; 7]).unwrap();
    let different = RetainedUltra2Plan::new(plan.parameters(), 194, &[65, 129], false).unwrap();
    assert!(RetainedUltra2::from_parts(p, s, different).is_err());
    for old in 0..4 {
        let frequencies =
            OptimalFrequencies::new_with_policy_in(PricePolicy::Ultra, false, Global).unwrap();
        let paths = OptimalPaths::new_in(Global).unwrap();
        let matches = OptimalMatches::new_in(Global).unwrap();
        let mut p = match old {
            0 => OptimalParser::from_policy_parts(
                PricePolicy::Ultra,
                &source,
                3,
                0,
                frequencies,
                paths,
                matches,
            ),
            1 => OptimalParser::from_ultra_fresh_parts(&source, 3, 0, frequencies, paths, matches),
            2 => OptimalParser::from_ultra2_parts(&source, 3, 0, frequencies, paths, matches),
            _ => {
                OptimalParser::from_ultra_retained_parts(&source, 3, 0, frequencies, paths, matches)
            }
        }
        .unwrap();
        let (_, mut s) = pair(&source, plan, [Global; 7]).unwrap();
        let mut uncalled = Probe::default();
        assert_eq!(
            p.parse_ultra2_retained(0, 97, true, [1, 4, 8], &mut s, &mut uncalled),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(
            p.rebase_ultra2_retained(&mut s),
            Err(Error::UnsupportedFeature)
        );
        assert_eq!(uncalled.seen, 0);
        assert!(RetainedUltra2::from_parts(p, s, plan).is_err());
    }
}
