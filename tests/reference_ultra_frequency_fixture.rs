//! Independent bounds and selected identities for captured policy-tagged frequencies.
#![forbid(unsafe_code)]
#[path = "support/ultra_frequency_fixture.rs"]
mod fixture;

#[test]
fn complete_capture_reads_all_sources_and_price_transitions() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 298);
    assert_eq!(
        cases.iter().map(|c| c.source.len()).sum::<usize>(),
        20194964
    );
    assert_eq!(
        cases.iter().map(|c| c.observations.len()).sum::<usize>(),
        4065
    );
    assert!(cases.iter().any(|c| c.disabled_literals));
    assert!(
        cases
            .iter()
            .any(|c| c.observations.iter().any(|o| o.operation[0] == 5))
    );
}

#[test]
fn captured_initialization_preserves_hand_checked_price_identities() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let dynamic = cases
        .iter()
        .find(|c| c.name == "uf_p0_init_single_9_m0")
        .unwrap();
    assert_eq!(dynamic.observations[0].sums, [1, 40, 53, 53]);
    assert_eq!(dynamic.observations[0].frequencies[usize::from(b'a')], 1);
    assert!(!dynamic.observations[0].predefined);
    assert_eq!(dynamic.observations[1].query.unwrap()[0], 2304);
    assert_eq!(dynamic.observations[1].query.unwrap()[2], 1843);
    assert_eq!(
        dynamic.observations[2].base_prices,
        dynamic.observations[1].base_prices
    );
    let small = cases
        .iter()
        .find(|c| c.name == "uf_p0_init_single_8_m0")
        .unwrap();
    assert!(small.observations[0].predefined);
    assert_eq!(small.observations[1].query.unwrap()[0], 12_288);
    assert!(!small.observations[5].predefined);
}

#[test]
fn malformed_headers_extents_and_trailing_data_are_rejected() {
    for length in [0, 7, 8, 11, 12, 27, fixture::PACK.len() - 1] {
        assert!(fixture::read(&fixture::PACK[..length]).is_err());
    }
    for (offset, value) in [
        (8, 0),
        (8, 299),
        (12, 81),
        (16, 131_073),
        (20, 129),
        (24, 2),
        (28, 1),
    ] {
        let mut bad = fixture::PACK.to_vec();
        bad[offset..offset + 4].copy_from_slice(&u32::to_le_bytes(value));
        assert!(fixture::read(&bad).is_err());
    }
    let mut extra = fixture::PACK.to_vec();
    extra.push(0);
    assert!(fixture::read(&extra).is_err());
}

#[test]
fn malformed_operation_state_and_price_tags_are_rejected() {
    let name_length = u32::from_le_bytes(fixture::PACK[12..16].try_into().unwrap()) as usize;
    let source_length = u32::from_le_bytes(fixture::PACK[16..20].try_into().unwrap()) as usize;
    let first = 32 + name_length + source_length;
    for (relative, value) in [
        (0, 4),
        (4, 1),
        (12, 1),
        (24, 2),
        (28, 1),
        (28, u32::MAX),
        (24 + 386 * 4, 1),
        (24 + 387 * 4, 1),
    ] {
        let mut bad = fixture::PACK.to_vec();
        bad[first + relative..first + relative + 4].copy_from_slice(&value.to_le_bytes());
        assert!(fixture::read(&bad).is_err());
    }
}

#[test]
fn fractional_policies_caches_and_boundary_observations_are_present() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.iter().filter(|c| c.policy == 0).count(), 150);
    assert_eq!(cases.iter().filter(|c| c.policy == 2).count(), 148);
    assert_eq!(
        cases
            .iter()
            .flat_map(|c| &c.observations)
            .filter(|o| o.query.is_some())
            .count(),
        2848
    );
    let upper = cases
        .iter()
        .find(|c| c.name == "uf_p2_fraction_boundary_30")
        .unwrap();
    assert!(
        upper
            .observations
            .iter()
            .any(|o| o.sums[0] == 8_388_606 && o.base_prices[0] == 6143)
    );
    assert!(upper.observations.iter().any(|o| o.sums[0] == 8447));
    let whole = cases
        .iter()
        .find(|c| c.name == "uf_p0_fraction_boundary_31")
        .unwrap();
    assert!(whole.observations.iter().any(|o| o.sums[0] == 8_388_608));
    assert!(whole.observations.iter().any(|o| o.sums[0] == 4351));
}

#[test]
fn valid_policy_changes_and_dirty_cache_forgeries_are_rejected() {
    let mut offset = 12;
    for case in fixture::read(fixture::PACK).unwrap() {
        let first = offset + 20 + case.name.len() + case.source.len();
        if case.policy == 2 {
            let mut bad = fixture::PACK.to_vec();
            bad[offset + 16..offset + 20].copy_from_slice(&0u32.to_le_bytes());
            assert!(fixture::read(&bad).is_err());
            let mut bad = fixture::PACK.to_vec();
            let cache = first + 24 + 382 * 4;
            bad[cache..cache + 4].copy_from_slice(&0u32.to_le_bytes());
            assert!(fixture::read(&bad).is_err());
            if let Some(i) = case
                .observations
                .iter()
                .position(|o| matches!(o.operation[0], 1 | 5))
            {
                let mut bad = fixture::PACK.to_vec();
                let cache = first + i * 1584 + 24 + 382 * 4;
                bad[cache..cache + 4].copy_from_slice(&999u32.to_le_bytes());
                assert!(fixture::read(&bad).is_err());
                return;
            }
        }
        offset = first + case.observations.len() * 1584;
    }
    panic!("missing fractional dirty state");
}
