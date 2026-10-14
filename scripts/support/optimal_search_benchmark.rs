//! Diagnostic deterministic private search workload, built by its Python runner.
#![feature(allocator_api)]
#![allow(dead_code)]
extern crate alloc;
#[path = "@ROOT@/src/error.rs"] mod error;
#[path = "@ROOT@/src/greedy_key.rs"] mod greedy_key;
#[path = "@ROOT@/src/tree_layout.rs"] mod tree_layout;
#[path = "@ROOT@/src/reference_tree.rs"] mod reference_tree;
#[path = "@ROOT@/src/optimal_search_layout.rs"] mod optimal_search_layout;
#[path = "@ROOT@/src/optimal_short_key.rs"] mod optimal_short_key;
#[path = "@ROOT@/src/optimal_matches.rs"] mod optimal_matches;
#[path = "@ROOT@/src/optimal_search.rs"] mod optimal_search;
pub use error::{Error, Result};
use optimal_search_layout::{OptimalQuery, OptimalSearchLayout};
use std::hint::black_box;
use std::time::Instant;
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

fn mix(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
}
fn main() -> Result<()> {
    let verify = std::env::args().nth(1).as_deref() == Some("--verify");
    let mut source = vec![0; 32768];
    let mut random = 0x5eed_0b71_2026_1014_u64;
    for (i, byte) in source.iter_mut().enumerate() {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        *byte = if i % 1024 < 768 { b'a' + (i % 23) as u8 } else { random as u8 };
    }
    for minimum in 3..=7 {
        let layout = OptimalSearchLayout::new(15, 10, 10, 4, minimum, 4096,
            (minimum == 3).then_some(15))?;
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap().into_non_clone();
        let monitor = allocator.monitor();
        let tree = reference_tree::ReferenceTree::new_in(layout.tree(), &allocator, &allocator)?;
        let short = optimal_short_key::OptimalShortKey::new_in(layout.short_log(), &allocator)?;
        let matches = optimal_matches::OptimalMatches::new_in(&allocator)?;
        let mut search = optimal_search::OptimalSearch::from_parts(&source, layout, tree, short, matches)?;
        let expected_bytes = layout.tree().total_bytes()
            + layout.short_log().map_or(0, |log| (1 << log) * core::mem::size_of::<usize>())
            + optimal_matches::MATCH_CAPACITY * core::mem::size_of::<optimal_search_layout::OptimalMatch>();
        let expected_attempts = if minimum == 3 { 4 } else { 3 };
        let mut run = || -> Result<(u64, usize)> {
            search.reset();
            let mut hash = 0xcbf29ce484222325;
            let mut count = 0;
            for position in (0..source.len() - 8).step_by(31) {
                let q = OptimalQuery { position, repeats: if position < 8 { [0; 3] } else { [1, 4, 8] },
                    zero_literals: position % 2 == 0, length_to_beat: if minimum == 3 { 3 } else { 4 } };
                let found = search.query(black_box(q))?;
                mix(&mut hash, position as u64);
                mix(&mut hash, found.len() as u64);
                for m in found { mix(&mut hash, u64::from(m.off_base())); mix(&mut hash, m.length() as u64); }
                count += found.len();
            }
            Ok((black_box(hash), count))
        };
        let expected = run()?;
        let mut samples = Vec::new();
        if !verify {
            for _ in 0..5 {
                let start = Instant::now();
                for _ in 0..64 { assert_eq!(run()?, expected); }
                samples.push(start.elapsed().as_nanos());
            }
        }
        let stats = monitor.snapshot();
        assert_eq!(stats.attempts, expected_attempts);
        assert_eq!(stats.live_requested_bytes, expected_bytes as u128);
        println!("{{\"minimum\":{minimum},\"checksum\":\"{:016x}\",\"candidates\":{},\"allocations\":{},\"owner_bytes\":{},\"rounds\":{},\"samples_ns\":{:?}}}",
            expected.0, expected.1, stats.attempts, stats.live_requested_bytes, if verify { 0 } else { 64 }, samples);
        drop(search);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().owners_dropped, 0);
        drop(allocator);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
    }
    Ok(())
}
