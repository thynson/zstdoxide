//! Small source-derived retained alternatives for foreign interpretation.
use super::*;
use crate::RawCompressionParameters;
use crate::optimal_frequencies::OptimalFrequencies;
use crate::optimal_matches::OptimalMatches;
use crate::optimal_parse::PathEvent;
use crate::optimal_paths::OptimalPaths;
use crate::optimal_search_layout::OptimalMatch;
use crate::optimal_short_key::OptimalShortKey;
use crate::reference_tree::ReferenceTree;

struct Alternatives {
    count: usize,
}
impl PathObserver for Alternatives {
    fn event<P: Allocator, F: Allocator>(
        &mut self,
        event: PathEvent,
        _: &OptimalPaths<P>,
        _: &OptimalFrequencies<F>,
        _: &[OptimalMatch],
    ) -> Result<()> {
        self.count += usize::from(event.alternative.is_some());
        Ok(())
    }
}

#[test]
fn small_retained_alternatives_preserve_absolute_positions_and_reset() {
    // The first two blocks of the frozen source-only alternatives_0202 recipe.
    // All expected counts come from its independent native/model fixture.
    let mut source = [0_u8; 1248];
    for (block, seed) in [0_u32, 2].into_iter().enumerate() {
        let bytes = &mut source[block * 624..(block + 1) * 624];
        for i in 0..48 {
            bytes[i * 9..i * 9 + 8].copy_from_slice(b"abbaabac");
            bytes[i * 9 + 8] = 128 + i as u8;
        }
        let mut state = 0x144f5e_u32 + seed;
        let alphabet = if seed == 0 { 2 } else { 4 };
        for byte in &mut bytes[432..] {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = 97 + (state % alphabet) as u8;
        }
    }
    let plan = RetainedOptimalPlan::new_ultra(
        RawCompressionParameters {
            window_log: 12,
            hash_log: 8,
            chain_log: 8,
            search_log: 6,
            min_match: 3,
            target_length: 128,
            strategy: 8,
        }
        .validate()
        .unwrap(),
        source.len(),
        &[624, 624],
        false,
    )
    .unwrap();
    let l = plan.layout();
    let parser = OptimalParser::from_ultra_retained_parts(
        &source,
        3,
        128,
        OptimalFrequencies::new_with_policy_in(PricePolicy::Ultra, false, Global).unwrap(),
        OptimalPaths::new_in(Global).unwrap(),
        OptimalMatches::new_in(Global).unwrap(),
    )
    .unwrap();
    let search = OptimalSearch::from_parts(
        &source,
        l,
        ReferenceTree::new_in(l.tree(), Global, Global).unwrap(),
        OptimalShortKey::new_in(l.short_log(), Global).unwrap(),
        OptimalMatches::new_in(Global).unwrap(),
    )
    .unwrap();
    let mut actual = RetainedOptimal::from_ultra_parts(parser, search, plan).unwrap();
    let mut results = None;
    for _ in 0..2 {
        let mut pass = alloc::vec::Vec::new();
        for (expected, result) in [
            (
                72,
                PathResult {
                    tail: 0,
                    repeats: [22, 164, 134],
                    sequences: 72,
                },
            ),
            (
                103,
                PathResult {
                    tail: 6,
                    repeats: [61, 140, 12],
                    sequences: 22,
                },
            ),
        ] {
            let mut observer = Alternatives { count: 0 };
            let observed = actual.parse_next(&mut observer).unwrap();
            assert_eq!(observed, result);
            pass.push(observed);
            assert_eq!(observer.count, expected);
            actual.publish(Publication::Compressed).unwrap();
            assert_eq!(actual.committed_repeats(), result.repeats);
        }
        assert!(actual.is_finished());
        if let Some(before) = &results {
            assert_eq!(&pass, before);
        } else {
            results = Some(pass);
        }
        actual.reset().unwrap();
    }
}
