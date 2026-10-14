//! Independent bounds and reconstruction checks for captured Optimal trajectories.
#![forbid(unsafe_code)]
#[path = "support/optimal_path_fixture.rs"]
mod fixture;

#[test]
fn all_captured_path_states_are_bounded_and_complete() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 27);
    assert_eq!(cases.iter().filter(|c| c.kind == 0).count(), 17);
    assert_eq!(cases.iter().map(|c| c.events.len()).sum::<usize>(), 22_024);
    assert_eq!(
        cases
            .iter()
            .flat_map(|c| &c.events)
            .filter(|e| e.query.is_some())
            .count(),
        966
    );
    assert_eq!(
        cases
            .iter()
            .flat_map(|c| &c.events)
            .map(|e| e.appended.len())
            .sum::<usize>(),
        42
    );
}

#[test]
fn later_queries_distinguish_the_strict_path_horizon() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for end in [4095, 4096, 4097] {
        let name = format!("path_wide_later_{end}");
        let case = cases.iter().find(|c| c.name == name).unwrap();
        assert!(
            case.events
                .iter()
                .any(|e| e.query.is_some_and(|q| q[0] == 2051))
        );
        let selected = case.events.iter().find(|e| e.stage == 6).unwrap();
        assert_eq!(selected.last, Some(end));
        assert_eq!(selected.cur, Some(if end == 4095 { 0 } else { 2050 }));
    }
}

#[test]
fn immediate_long_matches_do_not_use_match_length_as_a_path_index() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for (name, length) in [
        ("path_wide_initial_4096", 4096),
        ("path_wide_maximum_immediate", 8191),
    ] {
        let case = cases.iter().find(|c| c.name == name).unwrap();
        assert!(!case.events.iter().any(|e| e.stage == 3));
        assert_eq!(
            case.events.iter().find(|e| e.stage == 6).unwrap().last,
            Some(length)
        );
        assert!(
            case.events
                .iter()
                .flat_map(|e| &e.path)
                .all(|(at, _)| *at <= 2)
        );
    }
}

#[test]
fn malformed_headers_flags_deltas_and_truncation_are_rejected() {
    for length in [0, 7, 8, 11, 12, 47, fixture::PACK.len() - 1] {
        assert!(fixture::read(&fixture::PACK[..length]).is_err());
    }
    let word =
        |offset| u32::from_le_bytes(fixture::PACK[offset..offset + 4].try_into().unwrap()) as usize;
    assert_eq!(word(40), 0); // First source has no scripted query.
    let first = 48 + word(12) + word(16);
    assert_eq!(word(first + 52), 0); // The initial path is explicitly zero.
    assert!(word(first + 56) > 2);
    for (offset, value) in [
        (8, 0),
        (8, 65),
        (12, 0),
        (16, 8),
        (20, 2),
        (24, 4096),
        (28, 2),
        (32, 0),
        (36, 1),
        (44, 2),
        (first, 12),
        (first + 4, 1),
        (first + 40, 2),
        (first + 44, 2),
        (first + 48, 2),
        (first + 56, 387),
        (first + 60, 386),
        (first + 64, 0),
        (first + 68, word(first + 60) as u32),
    ] {
        let mut bad = fixture::PACK.to_vec();
        bad[offset..offset + 4].copy_from_slice(&u32::to_le_bytes(value));
        assert!(fixture::read(&bad).is_err(), "offset {offset}");
    }
    let mut bad = fixture::PACK.to_vec();
    bad[0] ^= 1;
    assert!(fixture::read(&bad).is_err());
    let mut bad = fixture::PACK.to_vec();
    bad.push(0);
    assert!(fixture::read(&bad).is_err());
}
