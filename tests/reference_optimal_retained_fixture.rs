//! Independent complete-frame decode and retained publication fixture admission.
#![forbid(unsafe_code)]
#[path = "support/optimal_retained_fixture.rs"]
mod fixture;
#[path = "support/optimal_path_fixture.rs"]
mod optimal_path_fixture;

#[test]
fn complete_retained_c_frames_decode_to_every_original_source() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 16);
    for case in &cases {
        assert_eq!(
            zstdoxide::decompress(case.frame, 1024).unwrap(),
            case.source,
            "{}",
            case.name
        );
    }
    let events: Vec<_> = cases
        .iter()
        .flat_map(|c| &c.blocks)
        .flat_map(|b| &b.events)
        .collect();
    assert_eq!(events.len(), 4486);
    assert_eq!(events.iter().filter(|e| e.query.is_some()).count(), 1523);
    assert_eq!(events.iter().map(|e| e.appended.len()).sum::<usize>(), 41);
}

#[test]
fn raw_candidate_rollback_rle_and_strict_catchup_boundaries_are_observed() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let raw = cases
        .iter()
        .find(|c| c.name == "retained_raw_candidate")
        .unwrap();
    let middle = &raw.blocks[1];
    assert_eq!(middle.kind, 0);
    assert_ne!(middle.candidate, middle.incoming);
    assert_eq!(middle.previous_before, middle.previous_after);
    assert_eq!(raw.blocks[2].incoming, middle.incoming);
    assert_eq!(middle.owners[1 - middle.previous_after], middle.candidate);
    for (name, frontier) in [
        ("retained_gap_385", 3),
        ("retained_gap_386", 387),
        ("retained_gap_512", 387),
    ] {
        let case = cases.iter().find(|c| c.name == name).unwrap();
        let block = &case.blocks[1];
        assert_eq!(block.pre_frontier, 3);
        assert_eq!(block.kind, 1);
        let mut search = vec![0; case.search_cells()];
        for b in &case.blocks {
            for (index, changes) in b.search_changes.iter().enumerate() {
                for &(at, value) in changes {
                    search[at] = value;
                }
                if b.start == block.start && index == 0 {
                    assert_eq!(&search[search.len() - 2..], &[frontier, frontier]);
                }
            }
        }
    }
}

fn event_bytes(e: &optimal_path_fixture::Event, search: &[(usize, u32)]) -> usize {
    40 + 12
        + e.stretch.map_or(0, |_| 28)
        + e.reverse.map_or(0, |_| 12)
        + e.query.map_or(0, |_| 24 + 4 + 8 * e.matches.len())
        + 4
        + 32 * e.path.len()
        + 4
        + 8 * e.frequencies.len()
        + 8
        + 16 * e.appended.len()
        + 4
        + 8 * search.len()
}
fn reject_word(offset: usize, value: u32) {
    let mut bad = fixture::PACK.to_vec();
    bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    assert!(
        fixture::read(&bad).is_err(),
        "accepted offset {offset} value {value}"
    );
}
#[test]
fn malformed_envelopes_publication_and_search_boundaries_are_rejected() {
    for n in [0, 7, 8, 11, 12, 59, 83, fixture::PACK.len() - 1] {
        assert!(fixture::read(&fixture::PACK[..n]).is_err());
    }
    for (at, value) in [
        (8, 0),
        (8, 17),
        (12, 129),
        (16, 1025),
        (20, 9),
        (24, 5),
        (28, 10),
        (32, 5),
        (36, 2),
        (40, 4096),
        (44, 2),
        (48, 1),
        (52, 2049),
        (56, 514),
        (60, 0),
        (68, 0),
        (76, 0),
    ] {
        reject_word(at, value);
    }
    let mut at = 12;
    for case in fixture::read(fixture::PACK).unwrap() {
        at += 72 + case.name.len() + case.source.len() + case.frame.len();
        for block in &case.blocks {
            for (word, value) in [
                (0, u32::MAX),
                (1, 63),
                (2, 3),
                (3, 2),
                (4, 2),
                (5, 1),
                (6, 1),
                (7, 0),
                (10, 0),
                (13, u32::MAX),
                (19, 513),
            ] {
                reject_word(at + word * 4, value);
            }
            if block.kind != 2 {
                reject_word(at + 16, (1 - block.previous_after) as u32);
            }
            at += 80;
            for (index, (event, search)) in
                block.events.iter().zip(&block.search_changes).enumerate()
            {
                let size = event_bytes(event, search);
                if index == 0 {
                    let delta = at + size - 4 - 8 * search.len();
                    reject_word(delta, 5123);
                    if !search.is_empty() {
                        reject_word(delta + 8, 1);
                    }
                }
                at += size;
            }
        }
    }
    assert_eq!(at, fixture::PACK.len());
    let mut bad = fixture::PACK.to_vec();
    bad.push(0);
    assert!(fixture::read(&bad).is_err());
}

#[test]
fn incomplete_and_corrupted_retained_frames_cannot_reproduce_sources() {
    for case in fixture::read(fixture::PACK).unwrap() {
        for end in 0..case.frame.len() {
            assert!(
                zstdoxide::decompress(&case.frame[..end], 1024).is_err(),
                "{} prefix {end}",
                case.name
            );
        }
        for at in [4, case.frame.len() - 1] {
            let mut bad = case.frame.to_vec();
            bad[at] ^= 0x80;
            if let Ok(decoded) = zstdoxide::decompress(&bad, 1024) {
                assert_ne!(decoded, case.source, "{} byte {at}", case.name);
            }
        }
    }
}
