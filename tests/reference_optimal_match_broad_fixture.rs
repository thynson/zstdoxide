//! Bounds and semantic boundaries from wider captured C search maps.
#![forbid(unsafe_code)]
#[path = "support/optimal_match_broad_fixture.rs"]
mod fixture;

#[test]
fn complete_broader_inventory_preserves_ordinary_short_key_geometry() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 37);
    assert_eq!(
        cases.iter().map(|c| c.source.len()).sum::<usize>(),
        2_119_568
    );
    assert_eq!(
        cases.iter().map(|c| c.observations.len()).sum::<usize>(),
        82
    );
    for case in &cases {
        assert_eq!(
            case.geometry[6],
            if case.geometry[4] == 3 {
                case.geometry[0].min(17)
            } else {
                0
            }
        );
    }
    let maximum = cases.iter().find(|c| c.name == "om_broad_maximum").unwrap();
    assert_eq!(maximum.source.len(), 524_288);
    assert_eq!(maximum.observations.len(), 16);
    assert_eq!(maximum.observations[0].after.short_heads.length, 131_072);
}

#[test]
fn strict_short_key_distance_is_separate_from_the_inclusive_window_floor() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for window in [18, 19] {
        for distance in [262_143u32, 262_144, 262_145] {
            let name = format!("om_broad_distance_w{window}_d{distance}");
            let case = cases.iter().find(|c| c.name == name).unwrap();
            let wanted = if distance < 262_144 {
                vec![[distance + 3, 3]]
            } else {
                vec![]
            };
            assert_eq!(case.observations[1].matches, wanted, "{name}");
        }
    }
}

#[test]
fn repeated_offsets_observe_strict_target_equality_and_frontier_publication() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for ll0 in [0, 1] {
        for target in [0, 3, 4, 7, 8, 11, 12, 4095, 4096] {
            let name = format!("om_broad_repeat_ll{ll0}_t{target}");
            let case = cases.iter().find(|c| c.name == name).unwrap();
            let observation = &case.observations[0];
            let count = if target < 4 {
                1
            } else if target < 8 {
                2
            } else {
                3
            };
            assert_eq!(observation.matches, [[1, 4], [2, 8], [3, 12]][..count]);
            assert_eq!(observation.after.short_frontier, 2);
            assert_eq!(
                observation.after.frontier,
                if target < 12 { 66 } else { 67 }
            );
        }
    }
}

#[test]
fn sparse_iteration_and_lookup_recover_every_implicit_zero() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for case in &cases {
        for observation in &case.observations {
            for state in [&observation.before, &observation.after] {
                for cells in [&state.heads, &state.nodes, &state.short_heads] {
                    let nonzero = cells
                        .full()
                        .enumerate()
                        .filter(|(_, v)| *v != 0)
                        .collect::<Vec<_>>();
                    assert_eq!(nonzero, cells.entries);
                    assert_eq!(cells.get(cells.length), None);
                    for &(index, value) in &cells.entries {
                        assert_eq!(cells.get(index), Some(value));
                    }
                }
            }
        }
    }
}

fn word(data: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize
}
fn patch(offset: usize, value: u32) {
    let mut data = fixture::PACK.to_vec();
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    assert!(
        fixture::read(&data).is_err(),
        "offset {offset}, value {value}"
    );
}

#[test]
fn malformed_protocol_geometry_and_query_domains_are_rejected() {
    for n in [0, 7, 11, 51, fixture::PACK.len() - 1] {
        assert!(fixture::read(&fixture::PACK[..n]).is_err());
    }
    for (offset, value) in [
        (8, 38),
        (12, 64),
        (16, 524_289),
        (20, 17),
        (24, 22),
        (28, 13),
        (32, 13),
        (36, 9),
        (40, 8),
        (44, 131_073),
        (48, 16),
    ] {
        patch(offset, value);
    }
    let first = 52 + word(fixture::PACK, 12) + word(fixture::PACK, 16);
    for (offset, value) in [(0, 524_288), (4, 3), (16, 2), (20, 2), (24, 0), (28, 0)] {
        patch(first + offset, value);
    }
    let mut extra = fixture::PACK.to_vec();
    extra.push(0);
    assert!(fixture::read(&extra).is_err());
}

#[test]
fn sparse_extents_zero_entries_duplicate_indices_and_hashes_are_rejected() {
    let first = 52 + word(fixture::PACK, 12) + word(fixture::PACK, 16);
    // Query words, a zero initial snapshot, then one primary head in after-state.
    for (offset, value) in [
        (32, 4095),
        (36, 4097),
        (64, 4095),
        (68, 4097),
        (72, 4096),
        (76, 0),
        (76, 1),
        (96, 261),
    ] {
        patch(first + offset, value);
    }
    patch(
        first + 72,
        ((word(fixture::PACK, first + 72) + 1) % 4096) as u32,
    );
    // Find the second query's after-state primary map, which has multiple heads.
    let mut cursor = first + 24;
    for _ in 0..2 {
        cursor += 8;
        for _ in 0..3 {
            let count = word(fixture::PACK, cursor + 4);
            cursor += 8 + count * 8;
        }
    }
    cursor += 4 + word(fixture::PACK, cursor) * 8 + 24;
    cursor += 8;
    for _ in 0..3 {
        let count = word(fixture::PACK, cursor + 4);
        cursor += 8 + count * 8;
    }
    cursor += 8;
    assert!(word(fixture::PACK, cursor + 4) >= 2);
    let first_index = word(fixture::PACK, cursor + 8);
    patch(cursor + 16, first_index as u32);
}
