//! Independent bounds and reconstruction checks for captured coupled-policy trajectories.
#![forbid(unsafe_code)]
#[path = "support/ultra_path_fixture.rs"]
mod fixture;

#[test]
fn all_captured_path_states_are_bounded_and_complete() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 58);
    assert_eq!(cases.iter().filter(|c| c.policy == 0).count(), 29);
    assert_eq!(cases.iter().map(|c| c.events.len()).sum::<usize>(), 65_672);
    assert_eq!(
        cases
            .iter()
            .flat_map(|c| &c.events)
            .filter(|e| e.query.is_some())
            .count(),
        22_798
    );
    assert_eq!(
        cases
            .iter()
            .flat_map(|c| &c.events)
            .map(|e| e.appended.len())
            .sum::<usize>(),
        143
    );
}

#[test]
fn later_queries_distinguish_the_strict_path_horizon() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for end in [4095, 4096, 4097] {
        let name = format!("up_p0_wide_later_{end}");
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
        ("up_p0_wide_initial_4096", 4096),
        ("up_p0_wide_maximum_immediate", 8191),
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
    assert_eq!(word(first + 56), 0); // The initial path is explicitly zero.
    assert!(word(first + 60) > 2);
    for (offset, value) in [
        (8, 0),
        (8, 59),
        (12, 0),
        (16, 8),
        (20, 2),
        (24, 4096),
        (28, 2),
        (32, 0),
        (36, 1),
        (44, 1),
        (first, 13),
        (first + 4, 1),
        (first + 40, 2),
        (first + 44, 2),
        (first + 48, 2),
        (first + 60, 387),
        (first + 64, 386),
        (first + 68, 0),
        (first + 72, word(first + 64) as u32),
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

#[test]
fn complete_signed_alternatives_and_every_operand_reject_forgery() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let alternatives: Vec<_> = cases
        .iter()
        .flat_map(|c| &c.events)
        .filter_map(|e| e.alternative)
        .collect();
    assert_eq!(alternatives.len(), 72);
    assert!(alternatives.iter().any(|a| a[8] < a[9] && a[8] < a[10]));
    assert!(alternatives.iter().any(|a| a[8] >= a[9] || a[8] >= a[10]));
    let witness: [i64; 12] = [4515, 5, 3, 0, 0, 0, 0, -224, 4547, 2168, 4515, 0];
    let bytes: Vec<_> = witness.iter().flat_map(|v| v.to_le_bytes()).collect();
    let at = fixture::PACK
        .windows(bytes.len())
        .position(|s| s == bytes)
        .unwrap();
    for column in 0..12 {
        let mut bad = fixture::PACK.to_vec();
        bad[at + 8 * column..at + 8 * (column + 1)]
            .copy_from_slice(&(witness[column] + 1).to_le_bytes());
        assert!(fixture::read(&bad).is_err(), "column {column}");
    }
    for value in [i64::MIN, i64::MAX, 0, 224] {
        let mut bad = fixture::PACK.to_vec();
        bad[at + 56..at + 64].copy_from_slice(&value.to_le_bytes());
        assert!(fixture::read(&bad).is_err());
    }
}

// Locate fields only after the complete immutable fixture passes its reader.
// This helper never handles caller-supplied malformed data.
fn publication_locations() -> (usize, usize, usize) {
    let cases = fixture::read(fixture::PACK).unwrap();
    let mut at = 12;
    let mut chosen = None;
    for case in cases {
        at += 36 + case.name.len() + case.source.len();
        for script in &case.scripts {
            at += 8 + script.matches.len() * 8;
        }
        let mut accepted = false;
        for event in case.events {
            let start = at;
            at += 40 + 4 + if event.stretch.is_some() { 28 } else { 0 };
            at += 4 + if event.reverse.is_some() { 12 } else { 0 };
            at += 4 + if event.query.is_some() {
                24 + 4 + event.matches.len() * 8
            } else {
                0
            };
            at += 4 + if event.alternative.is_some() { 96 } else { 0 };
            let changes = at;
            at += 4 + event.path.len() * 32;
            let frequency_changes = at;
            at += 4 + event.frequencies.len() * 8 + 8 + event.appended.len() * 16;
            if accepted {
                assert_eq!(event.stage, 4);
                assert_eq!(event.path.len(), 1);
                chosen = Some((start + 20, changes + 4, frequency_changes));
                break;
            }
            accepted = event
                .alternative
                .is_some_and(|a| a[8] < a[9] && a[8] < a[10]);
        }
        if chosen.is_some() {
            break;
        }
    }
    chosen.unwrap()
}

#[test]
fn accepted_alternative_requires_exact_next_cell_and_frontier() {
    let (frontier, change, frequency) = publication_locations();
    for at in [
        frontier,
        change,
        change + 4,
        change + 8,
        change + 12,
        change + 16,
        change + 20,
        change + 24,
        change + 28,
    ] {
        let mut bad = fixture::PACK.to_vec();
        let old = u32::from_le_bytes(bad[at..at + 4].try_into().unwrap());
        bad[at..at + 4].copy_from_slice(&(old + 1).to_le_bytes());
        assert!(fixture::read(&bad).is_err(), "publication field {at}");
    }
    assert_eq!(
        &fixture::PACK[frequency..frequency + 4],
        &0u32.to_le_bytes()
    );
    let mut bad = fixture::PACK.to_vec();
    bad[frequency..frequency + 4].copy_from_slice(&1u32.to_le_bytes());
    // Inject a syntactically canonical cache change at a non-refresh stage.
    bad.splice(
        frequency + 4..frequency + 4,
        [382u32, 999].into_iter().flat_map(|v| v.to_le_bytes()),
    );
    assert!(fixture::read(&bad).is_err());
}
