//! Complete policy-bound frequency lifecycle measurements; clock-free by default.
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
extern crate alloc;
#[allow(dead_code)]
#[path = "../src/constants.rs"]
mod constants;
#[path = "../tests/support/ultra_frequency_fixture.rs"]
mod fixture;
#[path = "../src/optimal_frequencies.rs"]
mod optimal_frequencies;
#[allow(dead_code)]
#[path = "../src/optimal_price.rs"]
mod optimal_price;
use optimal_frequencies::{FrequencySnapshot, OptimalFrequencies};
use optimal_price::PricePolicy;
use std::{
    alloc::{Allocator, Global},
    hint::black_box,
    time::Instant,
};
use zstdoxide::{Error, Result};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

const SELECTED: [&str; 6] = [
    "init_single_9_m0",
    "init_cycle_131072_m0",
    "counts_8192_m0",
    "symbols_n9_m1_2",
    "grouped_dirty_cache",
    "fraction_boundary_30",
];
fn policy(case: &fixture::Case<'_>) -> PricePolicy {
    if case.policy == 0 {
        PricePolicy::Optimal
    } else {
        PricePolicy::Ultra
    }
}
fn replay<A: Allocator>(
    case: &fixture::Case<'_>,
    model: &mut OptimalFrequencies<A>,
    verify: bool,
) -> FrequencySnapshot {
    for expected in &case.observations {
        let [tag, a, b, c, d, e] = expected.operation;
        match tag {
            0 | 3 => model
                .begin_block(black_box(&case.source[a as usize..(a + b) as usize]))
                .unwrap(),
            1 | 5 => {
                for _ in 0..if tag == 5 { e } else { 1 } {
                    model
                        .update(black_box(&case.source[a as usize..(a + b) as usize]), c, d)
                        .unwrap();
                }
            }
            2 => model.refresh().unwrap(),
            4 => {
                let prices = black_box(
                    model
                        .prices(
                            black_box(&case.source[a as usize..(a + b) as usize]),
                            c,
                            d,
                            e,
                        )
                        .unwrap(),
                );
                if verify {
                    assert_eq!(prices, expected.query.unwrap());
                }
            }
            _ => unreachable!(),
        }
        if verify {
            let actual = model.snapshot().unwrap();
            assert_eq!(actual.counts, expected.frequencies, "{}", case.name);
            assert_eq!(actual.sums, expected.sums, "{}", case.name);
            assert_eq!(actual.base_prices, expected.base_prices, "{}", case.name);
            assert_eq!(actual.predefined, expected.predefined, "{}", case.name);
            assert_eq!(actual.policy, policy(case));
            assert!(actual.initialized);
        }
    }
    black_box(model.snapshot().unwrap())
}
fn global_run(case: &fixture::Case<'_>, verify: bool) -> FrequencySnapshot {
    let mut model =
        OptimalFrequencies::new_with_policy_in(policy(case), case.disabled_literals, Global)
            .unwrap();
    replay(case, &mut model, verify)
}
fn state_json(state: FrequencySnapshot) -> String {
    format!(
        "{{\"counts\":{:?},\"sums\":{:?},\"base_prices\":{:?},\"predefined\":{},\"initialized\":{},\"dirty\":{}}}",
        state.counts,
        state.sums,
        state.base_prices,
        state.predefined,
        state.initialized,
        state.dirty
    )
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let timed = match args.as_slice() {
        [] => false,
        [arg] if arg == "--timed" => true,
        _ => panic!("usage: ultra_frequency_benchmark [--timed]"),
    };
    let cases = fixture::read(fixture::PACK).unwrap();
    let mut expected = Vec::with_capacity(cases.len());
    let mut transitions = 0;
    let mut queries = 0;
    for case in &cases {
        expected.push(global_run(case, true));
        transitions += case.observations.len();
        queries += case
            .observations
            .iter()
            .filter(|o| o.query.is_some())
            .count();
    }
    assert_eq!((cases.len(), transitions, queries), (298, 4065, 2848));
    println!(
        "{{\"record\":\"qualification\",\"cases\":298,\"transitions\":4065,\"queries\":2848,\"frequency_unsafe\":{},\"timed\":{}}}",
        cfg!(feature = "unsafe-optimal-frequencies"),
        timed
    );
    let mut selected = 0;
    for (case, &final_state) in cases.iter().zip(&expected) {
        let suffix = case
            .name
            .strip_prefix("uf_p0_")
            .or_else(|| case.name.strip_prefix("uf_p2_"))
            .unwrap();
        if !SELECTED.contains(&suffix) {
            continue;
        }
        selected += 1;
        let owner = TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 1,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone();
        let monitor = owner.monitor();
        {
            let mut model = OptimalFrequencies::new_with_policy_in(
                policy(case),
                case.disabled_literals,
                &owner,
            )
            .unwrap();
            assert_eq!(replay(case, &mut model, true), final_state);
            let live = monitor.snapshot();
            assert_eq!(
                (live.attempts, live.live_blocks, live.live_requested_bytes),
                (1, 1, 1508)
            );
            assert_eq!(
                (live.grow_calls, live.shrink_calls, live.owners_dropped),
                (0, 0, 0)
            );
        }
        monitor.assert_released();
        drop(owner);
        let state = monitor.snapshot();
        assert_eq!(
            (
                state.identities_created,
                state.owners_dropped,
                state.deallocate_calls
            ),
            (1, 1, 1)
        );
        assert!(!state.violations.any());
        let primitive_operations: u64 = case
            .observations
            .iter()
            .map(|o| {
                if o.operation[0] == 5 {
                    u64::from(o.operation[5])
                } else {
                    1
                }
            })
            .sum();
        println!(
            "{{\"record\":\"case\",\"name\":\"{}\",\"policy\":{},\"primitive_operations\":{},\"allocation_attempts\":{},\"requested_bytes\":{},\"owners_dropped\":{},\"final_state\":{}}}",
            case.name,
            case.policy,
            primitive_operations,
            state.attempts,
            state.requested_bytes,
            state.owners_dropped,
            state_json(final_state)
        );
        if timed {
            // Warmups and equality checks are outside measured regions.
            for _ in 0..3 {
                assert_eq!(global_run(case, false), final_state);
            }
            // Heavy literal and 8192-update schedules get fewer identical loops.
            let iterations = if suffix == "fraction_boundary_30" || suffix == "counts_8192_m0" {
                8
            } else {
                128
            };
            for sample in 0..9 {
                let mut results = Vec::with_capacity(iterations);
                let started = Instant::now();
                for _ in 0..iterations {
                    results.push(global_run(black_box(case), false));
                }
                let nanoseconds = started.elapsed().as_nanos();
                for state in results {
                    assert_eq!(state, final_state);
                }
                println!(
                    "{{\"record\":\"sample\",\"name\":\"{}\",\"sample\":{},\"iterations\":{},\"nanoseconds\":{}}}",
                    case.name, sample, iterations, nanoseconds
                );
            }
        }
    }
    assert_eq!(selected, 12);
    println!(
        "{{\"record\":\"end\",\"selected\":12,\"samples\":{}}}",
        if timed { 108 } else { 0 }
    );
}
