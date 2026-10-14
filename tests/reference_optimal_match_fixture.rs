//! Independent format and source checks for complete captured match maps.
#![forbid(unsafe_code)]
#[path = "support/optimal_match_fixture.rs"]
mod fixture;

#[test]
fn all_captured_sources_queries_and_owner_geometries_are_bounded() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 29);
    assert_eq!(cases.iter().map(|c| c.source.len()).sum::<usize>(), 71_072);
    assert_eq!(
        cases.iter().map(|c| c.observations.len()).sum::<usize>(),
        273
    );
    let maximum = cases.iter().find(|c| c.name == "om_maximum").unwrap();
    assert_eq!(maximum.source.len(), 16_384);
    assert_eq!(maximum.observations.len(), 64);
    for observation in &maximum.observations {
        assert_eq!(observation.after.heads.len(), 4096);
        assert_eq!(observation.after.nodes.len(), 4096);
        assert_eq!(observation.after.short_heads.len(), 4096);
    }
}

#[test]
fn primary_hash_and_short_key_hash_are_distinct_for_three_byte_matching() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let short = cases
        .iter()
        .find(|c| c.name == "om_three_target_0")
        .unwrap();
    assert_eq!(short.observations[0].after.heads[5], 2);
    assert_eq!(short.observations[0].after.heads[40], 0);
    assert_eq!(short.observations[2].matches, [[7, 3]]);
    assert_eq!(short.observations[2].after.short_heads[40], 2);
    assert_eq!(short.observations[2].after.frontier, 7);
    assert_eq!(short.observations[2].after.short_frontier, 6);
    let six = cases.iter().find(|c| c.name == "om_width_6").unwrap();
    let seven = cases.iter().find(|c| c.name == "om_width_7").unwrap();
    for (a, b) in six.observations.iter().zip(&seven.observations) {
        assert_eq!(a.before, b.before);
        assert_eq!(a.after, b.after);
        assert_eq!(a.matches, b.matches);
    }
}

#[test]
fn repeats_keep_intent_order_and_skipped_queries_preserve_complete_state() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for name in ["om_repeats_0", "om_repeats_1"] {
        let case = cases.iter().find(|c| c.name == name).unwrap();
        assert_eq!(case.observations[0].matches, [[1, 4], [2, 8], [3, 12]]);
    }
    let long = cases.iter().find(|c| c.name == "om_long_m4_t0").unwrap();
    assert_eq!(long.observations[1].matches, [[4, 8191]]);
    let skipped = long
        .observations
        .iter()
        .filter(|o| o.query[0] + 2 < o.before.frontier)
        .collect::<Vec<_>>();
    assert_eq!(skipped.len(), 8);
    for observation in skipped {
        assert_eq!(observation.before, observation.after);
        assert!(observation.matches.is_empty());
    }
}

#[test]
fn malformed_headers_names_extents_and_geometry_are_rejected() {
    for length in [0, 7, 8, 11, 12, 51, fixture::PACK.len() - 1] {
        assert!(fixture::read(&fixture::PACK[..length]).is_err());
    }
    for (offset, value) in [
        (8, 0),
        (8, 30),
        (12, 64),
        (16, 7),
        (16, 16_385),
        (20, 65),
        (24, 9),
        (28, 13),
        (32, 5),
        (36, 0),
        (40, 2),
        (44, 131_073),
        (48, 0),
    ] {
        let mut bad = fixture::PACK.to_vec();
        bad[offset..offset + 4].copy_from_slice(&u32::to_le_bytes(value));
        assert!(fixture::read(&bad).is_err(), "header offset {offset}");
    }
    let mut extra = fixture::PACK.to_vec();
    extra.push(0);
    assert!(fixture::read(&extra).is_err());
    let mut bad_name = fixture::PACK.to_vec();
    bad_name[52] = b'!';
    assert!(fixture::read(&bad_name).is_err());
}

#[test]
fn malformed_queries_frontiers_cells_and_candidate_capacity_are_rejected() {
    let word =
        |offset| u32::from_le_bytes(fixture::PACK[offset..offset + 4].try_into().unwrap()) as usize;
    let first = 52 + word(12) + word(16);
    // The first recipe has 64 primary cells, 64 edge cells and 64 short cells.
    let snapshot_bytes = (2 + 64 + 64 + 64) * 4;
    for (relative, value) in [
        (0, 99),
        (4, 3),
        (8, u32::MAX),
        (12, 3),
        (16, 2),
        (20, 2),
        (24, 0),
        (28, 0),
        (32, 1),
        (32 + 64 * 4, 1),
        (32 + 128 * 4, 1),
        (24 + snapshot_bytes, 0),
        (24 + 2 * snapshot_bytes, 261),
    ] {
        let mut bad = fixture::PACK.to_vec();
        bad[first + relative..first + relative + 4].copy_from_slice(&u32::to_le_bytes(value));
        assert!(fixture::read(&bad).is_err(), "record relative {relative}");
    }
}
