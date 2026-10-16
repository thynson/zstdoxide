//! Independent complete-frame decode and source/state fixture admission.
#![forbid(unsafe_code)]
#[path = "support/optimal_startup_fixture.rs"]
mod fixture;
#[path = "support/optimal_path_fixture.rs"]
mod optimal_path_fixture;

#[test]
fn complete_c_frames_decode_to_every_original_source() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 12);
    for case in &cases {
        assert_eq!(
            zstdoxide::decompress(case.frame, 512).unwrap(),
            case.path.source,
            "{}",
            case.path.name
        );
    }
    assert_eq!(
        cases.iter().map(|c| c.path.events.len()).sum::<usize>(),
        4409
    );
    assert_eq!(
        cases
            .iter()
            .flat_map(|c| &c.path.events)
            .filter(|e| e.query.is_some())
            .count(),
        1406
    );
    assert_eq!(
        cases
            .iter()
            .flat_map(|c| &c.path.events)
            .map(|e| e.appended.len())
            .sum::<usize>(),
        22
    );
}

#[test]
fn fresh_unavailable_repeats_survive_observation_without_becoming_source_positions() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for case in cases {
        let first = case.path.events.iter().find_map(|e| e.query).unwrap();
        assert_eq!(&first[..4], &[1, 1, 4, 8]);
        assert_eq!(case.allocations[1][1] - case.allocations[0][1], 3584);
        for event in case.path.events {
            if let Some(q) = event.query {
                for [off, _] in event.matches {
                    if off <= 3 && q[4] == 0 {
                        assert!(q[off as usize] <= q[0]);
                    }
                }
            }
        }
    }
}

#[test]
fn malformed_envelopes_allocators_and_search_deltas_are_rejected() {
    for length in [0, 7, 8, 11, 12, 39, 63, fixture::PACK.len() - 1] {
        assert!(fixture::read(&fixture::PACK[..length]).is_err());
    }
    let word = |at| u32::from_le_bytes(fixture::PACK[at..at + 4].try_into().unwrap()) as usize;
    let first_delta = 64 + word(28) + word(32);
    assert_eq!(word(first_delta), 2);
    for (offset, value) in [
        (8, 0),
        (8, 13),
        (12, 9),
        (16, 5),
        (20, 10),
        (24, 5),
        (28, 1025),
        (32, u32::MAX),
        (36, 1),
        (40, 0),
        (48, 0),
        (56, 0),
        (first_delta, 5123),
        (first_delta + 4, u32::MAX),
        (first_delta + 8, 1),
        (first_delta + 12, word(first_delta + 4) as u32),
    ] {
        let mut bad = fixture::PACK.to_vec();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(fixture::read(&bad).is_err(), "offset {offset}");
    }
    let mut bad = fixture::PACK.to_vec();
    bad[0] ^= 1;
    assert!(fixture::read(&bad).is_err());
    let mut bad = fixture::PACK.to_vec();
    bad.push(0);
    assert!(fixture::read(&bad).is_err());
}

#[test]
fn incomplete_c_frames_and_changed_payloads_cannot_match_the_original_source() {
    for case in fixture::read(fixture::PACK).unwrap() {
        for length in 0..case.frame.len() {
            assert!(
                zstdoxide::decompress(&case.frame[..length], 512).is_err(),
                "{} truncated to {length}",
                case.path.name
            );
        }
        let mut changed = case.frame.to_vec();
        *changed.last_mut().unwrap() ^= 0x80;
        if let Ok(decoded) = zstdoxide::decompress(&changed, 512) {
            assert_ne!(decoded, case.path.source, "{}", case.path.name);
        }
    }
}
