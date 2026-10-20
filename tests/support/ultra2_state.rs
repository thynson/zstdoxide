//! Complete private two-pass lifecycle, path, frequency, tokens and owner contracts.
use super::*;
use crate::RawCompressionParameters;
use crate::optimal_matches::OptimalMatches;
use crate::optimal_path_layout::{PATH_CAPACITY, PathCell};
use crate::optimal_short_key::OptimalShortKey;
use crate::reference_tree::ReferenceTree;
use crate::ultra2_fixture as fixture;
use alloc::{alloc::Global, vec};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
struct Compare<'a> {
    case: &'a fixture::Case<'a>,
    index: usize,
    phase: usize,
    total: usize,
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
        let expected = &self.case.phases[self.phase].events[self.index];
        let context = (self.case.name, self.phase, self.index, event.stage);
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
        self.total += 1;
        Ok(())
    }
}

fn words(s: FrequencySnapshot) -> [u32; 386] {
    let mut result = [0; 386];
    result[..377].copy_from_slice(&s.counts);
    result[377..381].copy_from_slice(&s.sums);
    result[381..385].copy_from_slice(&s.base_prices);
    result[385] = u32::from(s.predefined);
    result
}
impl Ultra2Observer for Compare<'_> {
    fn entry(&mut self, entry: Ultra2Eligibility) -> Result<()> {
        assert_eq!(
            entry.predicates()?,
            core::array::from_fn(|i| self.case.eligibility[i] != 0)
        );
        Ok(())
    }
    fn begin(
        &mut self,
        stage: Ultra2Phase,
        c: Ultra2Coordinates,
        f: FrequencySnapshot,
    ) -> Result<()> {
        let p = &self.case.phases[self.phase];
        assert_eq!(stage == Ultra2Phase::Seeding, p.seeding);
        assert_eq!(c.origin() + 2, p.origin as usize);
        assert_eq!(c.len(), self.case.source.len());
        assert_eq!(
            words(f),
            p.initial_frequency,
            "{} phase {} initial",
            self.case.name,
            self.phase
        );
        assert_eq!(f.initialized, self.phase != 0);
        assert_eq!(self.index, 0);
        Ok(())
    }
    fn end(
        &mut self,
        stage: Ultra2Phase,
        c: Ultra2Coordinates,
        r: PathResult,
        literals: usize,
        f: FrequencySnapshot,
    ) -> Result<()> {
        let p = &self.case.phases[self.phase];
        assert_eq!(stage == Ultra2Phase::Seeding, p.seeding);
        assert_eq!(c.origin() + 2, p.origin as usize);
        assert_eq!(self.index, p.events.len());
        assert_eq!(r.tail, p.tail as usize);
        assert_eq!(r.repeats, p.candidate);
        assert_eq!(r.sequences, p.events.last().unwrap().sequence_count);
        assert_eq!(
            literals,
            p.events
                .iter()
                .flat_map(|e| &e.appended)
                .map(|v| v[1] as usize)
                .sum::<usize>()
        );
        assert_eq!(words(f), p.final_frequency);
        Ok(())
    }
    fn reset(&mut self, r: Ultra2Reset) -> Result<()> {
        let expected = self.case.phases[self.phase].reset.as_ref().unwrap();
        assert_eq!(r.discarded.sequences, expected.discarded_sequences as usize);
        assert_eq!(r.discarded.tail, expected.discarded_tail as usize);
        assert_eq!(r.discarded.repeats, expected.discarded_repeats);
        assert_eq!(r.discarded_literals, expected.discarded_literals as usize);
        assert_eq!(r.original_repeats, expected.caller_repeats);
        assert_eq!(r.before.origin() + 2, expected.origin_before as usize);
        assert_eq!(r.after.origin() + 2, expected.origin_after as usize);
        assert_eq!(words(r.frequencies), expected.frequency);
        self.phase += 1;
        self.index = 0;
        Ok(())
    }
}
fn plan(size: usize, minimum: u32) -> Ultra2ExecutionPlan {
    Ultra2ExecutionPlan::new(
        RawCompressionParameters {
            window_log: 16,
            hash_log: 12,
            chain_log: 12,
            search_log: 6,
            min_match: minimum,
            target_length: 4095,
            strategy: 9,
        }
        .validate()
        .unwrap(),
        size,
        true,
        false,
    )
    .unwrap()
}
fn state<A: Allocator>(
    source: &[u8],
    p: Ultra2ExecutionPlan,
    owners: [A; 9],
) -> Result<Ultra2MatchState<'_, A>> {
    let [f, path, m, h, n, short, candidates, literals, sequences] = owners;
    let parser = OptimalParser::from_ultra2_parts(
        source,
        p.parameters().min_match(),
        p.parameters().target_length() as usize,
        OptimalFrequencies::new_with_policy_in(
            crate::optimal_price::PricePolicy::Ultra,
            p.disabled_literals(),
            f,
        )?,
        OptimalPaths::new_in(path)?,
        OptimalMatches::new_in(m)?,
    )?;
    let l = p.layout();
    let search = OptimalSearch::from_ultra2_parts(
        source,
        l,
        ReferenceTree::new_in(l.tree(), h, n)?,
        OptimalShortKey::new_in(l.short_log(), short)?,
        OptimalMatches::new_in(candidates)?,
    )?;
    Ultra2MatchState::from_parts(
        source,
        p,
        parser,
        search,
        Vec::new_in(literals),
        Vec::new_in(sequences),
    )
}
#[test]
fn every_native_two_pass_event_frequency_lifecycle_and_final_token() {
    let mut total = 0;
    let mut resets = 0;
    for case in fixture::read(fixture::PACK).unwrap() {
        let p = Ultra2ExecutionPlan::new(
            RawCompressionParameters {
                window_log: u32::from(case.window),
                hash_log: u32::from(case.hash),
                chain_log: u32::from(case.chain),
                search_log: u32::from(case.search),
                min_match: case.minimum,
                target_length: case.target,
                strategy: 9,
            }
            .validate()
            .unwrap(),
            case.source.len(),
            case.requested_seed,
            case.literal_mode == 1,
        )
        .unwrap();
        let mut s = state(case.source, p, [Global; 9]).unwrap();
        let mut compare = Compare {
            case: &case,
            index: 0,
            phase: 0,
            total: 0,
            cells: vec![PathCell::ZERO; PATH_CAPACITY],
            actual: vec![PathCell::ZERO; PATH_CAPACITY],
            frequencies: [0; 386],
        };
        s.parse_observed(case.source.len(), [1, 4, 8], &mut compare)
            .unwrap_or_else(|e| panic!("{}: {e:?}", case.name));
        assert_eq!(s.phase(), Ultra2Phase::Complete);
        assert_eq!(compare.phase + 1, case.phases.len());
        total += compare.total;
        resets += compare.phase;
        let last = case.phases.last().unwrap();
        let expected = last
            .events
            .iter()
            .flat_map(|e| &e.appended)
            .collect::<Vec<_>>();
        assert_eq!(s.sequences().len(), expected.len());
        let mut decoded = Vec::new();
        let mut at = 0;
        let mut repeats = [1, 4, 8];
        for (actual, row) in s.sequences().iter().zip(expected) {
            let seq = actual.sequence;
            assert_eq!([seq.literal_length, seq.match_length], [row[1], row[2]]);
            let resolved = actual
                .offset
                .resolve(seq.literal_length, seq.distance, repeats)
                .unwrap();
            assert_eq!(resolved.offset_value, row[3]);
            repeats = resolved.next_repeat;
            let end = at + seq.literal_length as usize;
            decoded.extend_from_slice(&s.literals()[at..end]);
            at = end;
            for _ in 0..seq.match_length {
                let value = decoded[decoded.len() - seq.distance as usize];
                decoded.push(value);
            }
        }
        decoded.extend_from_slice(&s.literals()[at..]);
        assert_eq!(decoded, case.source, "{}", case.name);
        assert_eq!(repeats, last.candidate);
        let mut final_search = vec![0; case.search_cells()];
        for phase in &case.phases {
            for changes in &phase.search_changes {
                for &(at, value) in changes {
                    final_search[at] = value;
                }
            }
        }
        assert_eq!(
            s.search.test_snapshot(),
            final_search,
            "{} search",
            case.name
        );
        assert_eq!(
            s.parse_next(case.source.len(), [1, 4, 8]),
            Err(Error::InvalidState)
        );
        assert_eq!(s.phase(), Ultra2Phase::Complete);
    }
    assert_eq!(total, 394656);
    assert_eq!(resets, 29);
}
#[test]
fn moved_nonclone_owners_release_every_failed_construction_and_reservation() {
    let source = [b'a'; 16384];
    for failed in 0..=9 {
        let owners = core::array::from_fn(|i| {
            TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 1,
                fail_at: if i == failed {
                    core::num::NonZeroU64::new(1)
                } else {
                    None
                },
            })
            .unwrap()
            .into_non_clone()
        });
        let monitors = owners.each_ref().map(|a| a.monitor());
        let result = state(&source, plan(source.len(), 3), owners);
        if failed < 7 {
            assert!(matches!(result, Err(Error::AllocationFailed)));
        } else {
            let mut state = result.unwrap();
            let result = state.parse_next(source.len(), [1, 4, 8]);
            if failed < 9 {
                assert_eq!(result, Err(Error::AllocationFailed));
                assert_eq!(
                    state.parse_next(source.len(), [1, 4, 8]),
                    Err(Error::InvalidState)
                );
                state.reset().unwrap();
                state.parse_next(source.len(), [1, 4, 8]).unwrap();
            } else {
                assert_eq!(result, Ok(true));
            }
            drop(state);
        }
        for m in monitors {
            m.assert_released();
            assert_eq!(m.snapshot().owners_dropped, 1);
        }
    }
}
#[test]
fn borrowed_owners_reuse_capacities_and_reject_partial_or_changed_repeats() {
    let source = [b'a'; 16384];
    let owners: [_; 9] = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let mut state = state(&source, plan(source.len(), 3), owners.each_ref()).unwrap();
    for (length, repeats) in [
        (0, [1, 4, 8]),
        (8192, [1, 4, 8]),
        (16385, [1, 4, 8]),
        (16384, [1, 1, 1]),
        (16384, [0, 4, 8]),
    ] {
        assert_eq!(
            state.parse_next(length, repeats),
            Err(Error::InvalidParameter)
        );
    }
    state.parse_next(source.len(), [1, 4, 8]).unwrap();
    let before = monitors.each_ref().map(|a| a.snapshot().attempts);
    assert_eq!(before, [1; 9]);
    let expected = state.sequences().to_vec();
    let capacity = state.retained_capacity();
    for _ in 0..3 {
        state.reset().unwrap();
        state.parse_next(source.len(), [1, 4, 8]).unwrap();
        assert_eq!(state.sequences(), expected);
        assert_eq!(state.retained_capacity(), capacity);
    }
    assert_eq!(monitors.each_ref().map(|a| a.snapshot().attempts), before);
    drop(state);
    for m in &monitors {
        m.assert_released();
        assert_eq!(m.snapshot().owners_dropped, 0);
    }
    drop(owners);
    for m in monitors {
        assert_eq!(m.snapshot().owners_dropped, 1);
    }
}

#[derive(Clone, Copy, Debug)]
enum Failure {
    Entry,
    Begin,
    Appended,
    End,
    Reset,
}
struct Fail {
    at: Failure,
    phase: Ultra2Phase,
    target: Ultra2Phase,
}
impl Fail {
    fn hit(&self, at: Failure) -> Result<()> {
        if core::mem::discriminant(&self.at) == core::mem::discriminant(&at)
            && (matches!(at, Failure::Entry | Failure::Reset) || self.phase == self.target)
        {
            Err(Error::ResourceLimit)
        } else {
            Ok(())
        }
    }
}
impl PathObserver for Fail {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        e: PathEvent,
        _: &OptimalPaths<P>,
        _: &OptimalFrequencies<F>,
        _: &[OptimalMatch],
    ) -> Result<()> {
        if e.appended.is_some() {
            self.hit(Failure::Appended)?;
        }
        Ok(())
    }
}
impl Ultra2Observer for Fail {
    fn entry(&mut self, _: Ultra2Eligibility) -> Result<()> {
        self.hit(Failure::Entry)
    }
    fn begin(
        &mut self,
        phase: Ultra2Phase,
        _: Ultra2Coordinates,
        _: FrequencySnapshot,
    ) -> Result<()> {
        self.phase = phase;
        self.hit(Failure::Begin)
    }
    fn end(
        &mut self,
        _: Ultra2Phase,
        _: Ultra2Coordinates,
        _: PathResult,
        _: usize,
        _: FrequencySnapshot,
    ) -> Result<()> {
        self.hit(Failure::End)
    }
    fn reset(&mut self, _: Ultra2Reset) -> Result<()> {
        self.hit(Failure::Reset)
    }
}
#[test]
fn every_lifecycle_failure_hides_discarded_or_partial_output_until_whole_reset() {
    let source = [b'a'; 64];
    for target in [Ultra2Phase::Seeding, Ultra2Phase::Encoding] {
        for at in [
            Failure::Entry,
            Failure::Begin,
            Failure::Appended,
            Failure::End,
            Failure::Reset,
        ] {
            let mut s = state(&source, plan(source.len(), 3), [Global; 9]).unwrap();
            let mut observer = Fail {
                at,
                phase: Ultra2Phase::Ready,
                target,
            };
            assert_eq!(
                s.parse_observed(source.len(), [1, 4, 8], &mut observer),
                Err(Error::ResourceLimit),
                "{at:?} {target:?}"
            );
            assert_eq!(s.phase(), Ultra2Phase::Failed);
            assert!(s.literals().is_empty());
            assert!(s.sequences().is_empty());
            assert_eq!(
                s.parse_next(source.len(), [1, 4, 8]),
                Err(Error::InvalidState)
            );
            s.reset().unwrap();
            assert_eq!(s.phase(), Ultra2Phase::Ready);
            assert!(s.literals().is_empty());
            assert!(s.sequences().is_empty());
            s.parse_next(source.len(), [1, 4, 8]).unwrap();
            assert!(!s.sequences().is_empty());
        }
    }
}
#[test]
fn composition_rejects_foreign_source_geometry_and_used_or_rebased_owners() {
    fn rebind<'a>(
        s: Ultra2MatchState<'a, Global>,
        source: &'a [u8],
        plan: Ultra2ExecutionPlan,
    ) -> Result<Ultra2MatchState<'a, Global>> {
        Ultra2MatchState::from_parts(source, plan, s.parser, s.search, s.literals, s.sequences)
    }
    let source = [b'a'; 64];
    let other = source;
    let p = plan(64, 3);
    assert!(matches!(
        rebind(state(&source, p, [Global; 9]).unwrap(), &other, p),
        Err(Error::InvalidParameter)
    ));
    for wrong in [plan(128, 3), plan(64, 4)] {
        assert!(matches!(
            rebind(state(&source, p, [Global; 9]).unwrap(), &source, wrong),
            Err(Error::InvalidParameter)
        ));
    }
    for (disabled, target) in [(true, 4095), (false, 1)] {
        let mut original = state(&source, p, [Global; 9]).unwrap();
        original.parser = OptimalParser::from_ultra2_parts(
            &source,
            3,
            target,
            OptimalFrequencies::new_with_policy_in(
                crate::optimal_price::PricePolicy::Ultra,
                disabled,
                Global,
            )
            .unwrap(),
            OptimalPaths::new_in(Global).unwrap(),
            OptimalMatches::new_in(Global).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            rebind(original, &source, p),
            Err(Error::InvalidParameter)
        ));
    }
    for step in 0..3 {
        let mut s = state(&source, p, [Global; 9]).unwrap();
        match step {
            0 => {
                s.search
                    .query_ultra2(crate::optimal_search_layout::OptimalQuery {
                        position: 1,
                        repeats: [1, 4, 8],
                        zero_literals: false,
                        length_to_beat: 3,
                    })
                    .unwrap();
            }
            1 => {
                s.search.rebase_ultra2().unwrap();
            }
            _ => {
                s.parse_next(64, [1, 4, 8]).unwrap();
            }
        }
        assert!(rebind(s, &source, p).is_err());
    }
}

#[test]
fn small_two_pass_nonclone_rebase_failure_and_reuse() {
    let source = b"abacabadabacabaeabacabadabacabaeabacabadabacabaeabacabadabacabae";
    let p = Ultra2ExecutionPlan::new(
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
        source.len(),
        true,
        false,
    )
    .unwrap();
    let owners: [_; 9] = core::array::from_fn(|_| {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone()
    });
    let monitors = owners.each_ref().map(|a| a.monitor());
    let mut s = state(source, p, owners.each_ref()).unwrap();
    let mut observer = Fail {
        at: Failure::Reset,
        phase: Ultra2Phase::Ready,
        target: Ultra2Phase::Seeding,
    };
    assert_eq!(
        s.parse_observed(source.len(), [1, 4, 8], &mut observer),
        Err(Error::ResourceLimit)
    );
    assert_eq!(s.phase(), Ultra2Phase::Failed);
    assert!(s.literals().is_empty());
    assert!(s.sequences().is_empty());
    assert_eq!(
        s.search.ultra2_coordinates().unwrap().origin(),
        source.len()
    );
    assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), [1; 9]);
    s.reset().unwrap();
    assert_eq!(s.search.ultra2_coordinates().unwrap().origin(), 0);
    assert_eq!(words(s.parser.ultra2_frequencies().unwrap()), [0; 386]);
    s.parse_next(source.len(), [1, 4, 8]).unwrap();
    assert_eq!(s.phase(), Ultra2Phase::Complete);
    assert!(!s.sequences().is_empty());
    assert_eq!(monitors.each_ref().map(|m| m.snapshot().attempts), [1; 9]);
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
