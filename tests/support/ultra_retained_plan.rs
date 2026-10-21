//! Allocation-free policy, schedule, owner and output bounds before execution.
use super::*;
use crate::RawCompressionParameters;
use crate::ultra_retained_fixture as fixture;
use alloc::vec::Vec;

fn raw() -> RawCompressionParameters {
    RawCompressionParameters {
        window_log: 16,
        hash_log: 12,
        chain_log: 12,
        search_log: 6,
        min_match: 3,
        target_length: 4095,
        strategy: 8,
    }
}

#[test]
fn every_ultra_frame_length_preserves_balanced_schedule_and_all_owner_bounds() {
    for n in 16385..=32768 {
        let ultra = RetainedOptimalPlan::for_ultra_frame(raw().validate().unwrap(), n).unwrap();
        let mut ordinary = raw();
        ordinary.strategy = 7;
        let ordinary = RetainedOptimalPlan::for_frame(ordinary.validate().unwrap(), n).unwrap();
        assert_eq!(ultra.policy(), PricePolicy::Ultra);
        assert_eq!(ordinary.policy(), PricePolicy::Optimal);
        assert_eq!(ultra.parameters().strategy(), ReferenceStrategy::Ultra);
        assert_eq!(ultra.blocks(), ordinary.blocks());
        assert_eq!(ultra.layout(), ordinary.layout());
        assert_eq!(ultra.owner_bytes(), ordinary.owner_bytes());
        assert_eq!(ultra.frame_bound(), ordinary.frame_bound());
        assert_eq!(ultra.frame_bound(), n + 7 + 3 * ultra.blocks().len());
        assert!(!ultra.disabled_literals());
        let mut offset = 0;
        for &length in ultra.blocks() {
            assert_eq!(ultra.next_block_len(offset).unwrap(), length);
            assert_eq!(
                ultra.next_block_len(offset + 1),
                Err(Error::InvalidParameter)
            );
            offset += length;
        }
        assert_eq!(offset, n);
        assert_eq!(ultra.next_block_len(n), Err(Error::InvalidParameter));
    }
}

#[test]
fn all_captured_ultra_schedules_bind_policy_geometry_and_independent_owner_sizes() {
    let mut count = 0;
    for case in fixture::read(fixture::PACK)
        .unwrap()
        .into_iter()
        .filter(|c| c.policy == 2)
    {
        let parameters = RawCompressionParameters {
            window_log: case.window.into(),
            hash_log: case.hash.into(),
            chain_log: case.chain.into(),
            search_log: case.search.into(),
            min_match: case.minimum,
            target_length: case.target,
            strategy: 8,
        }
        .validate()
        .unwrap();
        let mut blocks: Vec<_> = case.blocks.iter().map(|b| b.length).collect();
        let plan = RetainedOptimalPlan::new_ultra(
            parameters,
            case.source.len(),
            &blocks,
            case.literal_mode == 1,
        )
        .unwrap();
        assert_eq!(plan.policy(), PricePolicy::Ultra);
        assert_eq!(plan.source_len(), case.source.len());
        assert_eq!(plan.disabled_literals(), case.literal_mode == 1);
        assert_eq!(plan.blocks(), blocks);
        let largest = *blocks.iter().max().unwrap();
        assert_eq!(
            plan.owner_bytes(),
            [
                377 * core::mem::size_of::<u32>(),
                4099 * core::mem::size_of::<PathCell>(),
                MATCH_CAPACITY * core::mem::size_of::<OptimalMatch>(),
                (1 << case.hash) * core::mem::size_of::<usize>(),
                (1 << case.chain) * core::mem::size_of::<usize>(),
                case.short_cells() * core::mem::size_of::<usize>(),
                MATCH_CAPACITY * core::mem::size_of::<OptimalMatch>(),
                largest,
                largest / 3 * core::mem::size_of::<SequenceWithOffset>(),
            ]
        );
        blocks.fill(0);
        assert!(plan.blocks().iter().all(|&n| n >= 64));
        count += 1;
    }
    assert_eq!(count, 21);
}

#[test]
fn strategy_and_resource_errors_preserve_old_admission_boundaries() {
    for strategy in 1..=9 {
        let mut parameters = raw();
        parameters.strategy = strategy;
        let parameters = parameters.validate().unwrap();
        assert_eq!(
            RetainedOptimalPlan::new_ultra(parameters, 128, &[64, 64], false).is_ok(),
            strategy == 8
        );
        assert_eq!(
            RetainedOptimalPlan::new(parameters, 128, &[64, 64], false).is_ok(),
            strategy == 7
        );
        if strategy != 8 {
            assert_eq!(
                RetainedOptimalPlan::for_ultra_frame(parameters, 32768),
                Err(Error::UnsupportedFeature)
            );
        }
    }
    for n in [0, 127, 128, 16384, 32769, usize::MAX] {
        assert_eq!(
            RetainedOptimalPlan::for_ultra_frame(raw().validate().unwrap(), n),
            Err(Error::ResourceLimit)
        );
    }
    for n in [0, 127, 32769, usize::MAX] {
        assert_eq!(
            RetainedOptimalPlan::new_ultra(raw().validate().unwrap(), n, &[64, 64], false),
            Err(Error::ResourceLimit)
        );
    }
    for (field, value) in [(0, 14), (0, 17), (1, 13), (2, 13), (3, 7), (4, 4096)] {
        let mut p = raw();
        match field {
            0 => p.window_log = value,
            1 => p.hash_log = value,
            2 => p.chain_log = value,
            3 => p.search_log = value,
            _ => p.target_length = value,
        }
        assert_eq!(
            RetainedOptimalPlan::for_ultra_frame(p.validate().unwrap(), 32768),
            Err(Error::ResourceLimit)
        );
    }
    for blocks in [
        &[][..],
        &[128],
        &[63, 65],
        &[64, 63],
        &[64, 64, 64],
        &[8193, 8191],
        &[64, 64, 64, 64, 64],
        &[usize::MAX, 64],
    ] {
        assert_eq!(
            RetainedOptimalPlan::new_ultra(raw().validate().unwrap(), 128, blocks, false),
            Err(Error::InvalidParameter)
        );
    }
    let plan =
        RetainedOptimalPlan::new_ultra(raw().validate().unwrap(), 128, &[64, 64], true).unwrap();
    assert_eq!(plan.frame_bound(), 128 + 6 + 6);
    assert!(plan.disabled_literals());
}
