//! Independent bounds and selected identities for captured Optimal prices.
#![forbid(unsafe_code)]
#[path = "support/optimal_price_fixture.rs"]
mod fixture;

#[test]
fn complete_capture_reads_all_sources_and_price_transitions() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 147);
    assert_eq!(
        cases.iter().map(|c| c.source.len()).sum::<usize>(),
        9_835_338
    );
    assert_eq!(
        cases.iter().map(|c| c.observations.len()).sum::<usize>(),
        2018
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
        .find(|c| c.name == "op_init_single_9_m0")
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
        .find(|c| c.name == "op_init_single_8_m0")
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
        (8, 148),
        (12, 64),
        (16, 131_073),
        (20, 129),
        (24, 2),
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
    let first = 28 + name_length + source_length;
    for (relative, value) in [
        (0, 4),
        (4, 1),
        (12, 1),
        (24, 2),
        (28, u32::MAX),
        (24 + 386 * 4, 1),
        (24 + 387 * 4, 1),
    ] {
        let mut bad = fixture::PACK.to_vec();
        bad[first + relative..first + relative + 4].copy_from_slice(&value.to_le_bytes());
        assert!(fixture::read(&bad).is_err());
    }
}
