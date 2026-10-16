//! Full native scripted-policy comparisons; independent fixture reader.
use super::*;
use crate::optimal_path_layout::PATH_CAPACITY;
use crate::ultra_path_fixture as fixture;
use alloc::alloc::Global;
struct Script<'a>(&'a [fixture::Script]);
impl PathProvider for Script<'_> {
    fn query<A: Allocator>(
        &mut self,
        query: OptimalQuery,
        matches: &mut OptimalMatches<A>,
    ) -> Result<()> {
        if let Some(script) = self
            .0
            .iter()
            .find(|s| s.position as usize == query.position)
        {
            for &[off, length] in &script.matches {
                matches.push(OptimalMatch::new(off, length as usize)?)?;
            }
        }
        Ok(())
    }
}
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
        assert_eq!(
            snapshot.policy,
            if self.case.policy == 0 {
                PricePolicy::Optimal
            } else {
                PricePolicy::Ultra
            }
        );
        self.index += 1;
        Ok(())
    }
}
#[test]
fn every_captured_scripted_decision_matches_complete_path_frequency_and_output_state() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let mut events = 0;
    let mut sequences = 0;
    for case in &cases {
        let policy = if case.policy == 0 {
            PricePolicy::Optimal
        } else {
            PricePolicy::Ultra
        };
        let freq =
            OptimalFrequencies::new_with_policy_in(policy, case.literal_mode == 1, Global).unwrap();
        let paths = OptimalPaths::new_in(Global).unwrap();
        let matches = OptimalMatches::new_in(Global).unwrap();
        let mut parser = OptimalParser::from_policy_parts(
            policy,
            case.source,
            case.minimum as u8,
            case.target as usize,
            freq,
            paths,
            matches,
        )
        .unwrap();
        let mut compare = Compare {
            case,
            index: 0,
            cells: vec![PathCell::ZERO; PATH_CAPACITY],
            actual: vec![PathCell::ZERO; PATH_CAPACITY],
            frequencies: [0; 386],
        };
        let result = parser
            .parse(&mut Script(&case.scripts), &mut compare)
            .unwrap();
        assert_eq!(compare.index, case.events.len(), "{}", case.name);
        events += compare.index;
        sequences += result.sequences;
        let last = case.events.last().unwrap();
        assert_eq!(result.tail, last.tail.unwrap() as usize);
        assert_eq!(result.repeats, last.repeats);
        assert_eq!(parser.status, Status::Finished);
    }
    assert_eq!(events, 65_672);
    assert_eq!(sequences, 143);
}

struct Observe {
    fail_alternative: bool,
    alternatives: usize,
}
impl PathObserver for Observe {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        _: &OptimalPaths<P>,
        _: &OptimalFrequencies<F>,
        _: &[OptimalMatch],
    ) -> Result<()> {
        if event.alternative.is_some() {
            self.alternatives += 1;
            if self.fail_alternative {
                return Err(Error::ResourceLimit);
            }
        }
        Ok(())
    }
}
struct Immediate;
impl PathProvider for Immediate {
    fn query<A: Allocator>(
        &mut self,
        query: OptimalQuery,
        matches: &mut OptimalMatches<A>,
    ) -> Result<()> {
        if query.position == 1 {
            matches.push(OptimalMatch::new(4, 15)?)?;
        }
        Ok(())
    }
}
#[test]
fn borrowed_ultra_owners_preserve_policy_without_parse_or_reset_allocations() {
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    let owner = TrackingAllocator::new(AllocatorConfig::default())
        .unwrap()
        .into_non_clone();
    let monitor = owner.monitor();
    let mut parser = OptimalParser::from_policy_parts(
        PricePolicy::Ultra,
        b"aaaaaaaaaaaaaaaa",
        4,
        0,
        OptimalFrequencies::new_with_policy_in(PricePolicy::Ultra, false, &owner).unwrap(),
        OptimalPaths::new_in(&owner).unwrap(),
        OptimalMatches::new_in(&owner).unwrap(),
    )
    .unwrap();
    assert_eq!(parser.fresh_configuration(), Err(Error::UnsupportedFeature));
    assert_eq!(
        parser.retained_configuration(),
        Err(Error::UnsupportedFeature)
    );
    assert_eq!(parser.status, Status::Ready);
    assert!(monitor.fail_next());
    let before = monitor.snapshot();
    let mut observe = Observe {
        fail_alternative: false,
        alternatives: 0,
    };
    for _ in 0..3 {
        let result = parser.parse(&mut Immediate, &mut observe).unwrap();
        assert_eq!(result.sequences, 1);
        assert_eq!(result.tail, 0);
        assert_eq!(
            parser.parse(&mut Immediate, &mut observe),
            Err(Error::InvalidState)
        );
        parser.reset().unwrap();
        assert_eq!(parser.frequencies.policy(), PricePolicy::Ultra);
        assert_eq!(
            parser.frequencies.snapshot().unwrap().policy,
            PricePolicy::Ultra
        );
    }
    assert_eq!(monitor.snapshot().attempts, before.attempts);
    drop(parser);
    monitor.assert_released();
    assert_eq!(monitor.snapshot().owners_dropped, 0);
    assert!(matches!(
        OptimalPaths::new_in(&owner),
        Err(Error::AllocationFailed)
    ));
    drop(owner);
    assert_eq!(monitor.snapshot().owners_dropped, 1);
}
#[test]
fn explicit_ultra_mismatch_and_bounds_release_exact_moved_owners() {
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
    for (policy, frequency_policy, length, minimum, target) in [
        (PricePolicy::Ultra, PricePolicy::Optimal, 16, 4, 0),
        (PricePolicy::Optimal, PricePolicy::Ultra, 16, 4, 0),
        (PricePolicy::Ultra, PricePolicy::Ultra, 8, 4, 0),
        (PricePolicy::Ultra, PricePolicy::Ultra, 8193, 4, 0),
        (PricePolicy::Ultra, PricePolicy::Ultra, 16, 2, 0),
        (PricePolicy::Ultra, PricePolicy::Ultra, 16, 4, 4096),
    ] {
        let source = vec![b'a'; length];
        let a = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let ma = a.monitor();
        let b = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let mb = b.monitor();
        let c = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let mc = c.monitor();
        let result = OptimalParser::from_policy_parts(
            policy,
            &source,
            minimum,
            target,
            OptimalFrequencies::new_with_policy_in(frequency_policy, false, a).unwrap(),
            OptimalPaths::new_in(b).unwrap(),
            OptimalMatches::new_in(c).unwrap(),
        );
        assert!(matches!(result, Err(Error::InvalidParameter)));
        for monitor in [ma, mb, mc] {
            monitor.assert_released();
            assert_eq!(monitor.snapshot().attempts, 1);
            assert_eq!(monitor.snapshot().owners_dropped, 1);
            assert!(!monitor.snapshot().violations.any());
        }
    }
}
#[test]
fn alternative_observer_failure_poisoning_requires_reset_before_complete_replay() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for case in cases
        .iter()
        .filter(|case| case.events.iter().any(|e| e.alternative.is_some()))
    {
        let mut parser = OptimalParser::from_policy_parts(
            PricePolicy::Ultra,
            case.source,
            case.minimum as u8,
            case.target as usize,
            OptimalFrequencies::new_with_policy_in(
                PricePolicy::Ultra,
                case.literal_mode == 1,
                Global,
            )
            .unwrap(),
            OptimalPaths::new_in(Global).unwrap(),
            OptimalMatches::new_in(Global).unwrap(),
        )
        .unwrap();
        let mut observe = Observe {
            fail_alternative: true,
            alternatives: 0,
        };
        assert_eq!(
            parser.parse(&mut Script(&case.scripts), &mut observe),
            Err(Error::ResourceLimit)
        );
        assert_eq!(observe.alternatives, 1);
        assert_eq!(parser.status, Status::Poisoned);
        assert_eq!(
            parser.parse(&mut Script(&case.scripts), &mut observe),
            Err(Error::InvalidState)
        );
        assert_eq!(observe.alternatives, 1);
        parser.reset().unwrap();
        let mut compare = Compare {
            case,
            index: 0,
            cells: vec![PathCell::ZERO; PATH_CAPACITY],
            actual: vec![PathCell::ZERO; PATH_CAPACITY],
            frequencies: [0; 386],
        };
        let result = parser
            .parse(&mut Script(&case.scripts), &mut compare)
            .unwrap();
        assert_eq!(compare.index, case.events.len());
        assert_eq!(result.repeats, case.events.last().unwrap().repeats);
    }
}
