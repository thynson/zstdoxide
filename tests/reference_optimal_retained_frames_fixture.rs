//! Independent larger ordinary C frame decode and sparse horizon admission.
#![forbid(unsafe_code)]
#[path = "support/optimal_retained_frames_fixture.rs"]
mod fixture;
#[path = "support/optimal_path_fixture.rs"]
mod optimal_path_fixture;

#[test]
fn all_larger_c_frames_and_complete_event_totals_are_independent() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 19);
    for case in &cases {
        assert_eq!(
            zstdoxide::decompress(case.frame, 32768).unwrap(),
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
    assert_eq!(events.len(), 67321);
    assert_eq!(events.iter().filter(|e| e.query.is_some()).count(), 33122);
    assert_eq!(events.iter().map(|e| e.appended.len()).sum::<usize>(), 84);
    assert!(events.iter().flat_map(|e| &e.matches).any(|m| m[1] > 4096));
}
#[test]
fn actual_horizon_and_raw_rle_publication_states_survive_projection() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let horizon = cases.iter().find(|c| c.name == "wide_horizon").unwrap();
    assert!(
        horizon
            .blocks
            .iter()
            .flat_map(|b| &b.events)
            .any(|e| e.last == Some(4195))
    );
    assert!(
        horizon
            .blocks
            .iter()
            .flat_map(|b| &b.events)
            .flat_map(|e| &e.path)
            .any(|(at, _)| *at == 4096)
    );
    let noise = cases.iter().find(|c| c.name == "wide_noise_copy").unwrap();
    assert_eq!(noise.blocks[0].kind, 0);
    assert_eq!(noise.blocks[1].incoming, [1, 4, 8]);
    let uniform = cases.iter().find(|c| c.name == "wide_uniform").unwrap();
    let rle = &uniform.blocks[1];
    assert_eq!(rle.kind, 1);
    assert_ne!(rle.incoming, rle.candidate);
    assert_eq!(rle.previous_before, rle.previous_after);
    assert_eq!(rle.owners[rle.previous_after], rle.incoming);
    assert_eq!(rle.owners[1 - rle.previous_after], rle.candidate);
    let largest = cases.iter().find(|c| c.name == "wide_window16").unwrap();
    assert_eq!(largest.short_cells(), 65536);
    assert_eq!(largest.blocks.len(), 4);
}
fn event_bytes(e: &optimal_path_fixture::Event, s: &[(usize, u32)]) -> usize {
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
        + 8 * s.len()
}
fn reject_word(offset: usize, value: u32) {
    let mut bad = fixture::PACK.to_vec();
    bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    assert!(
        fixture::read(&bad).is_err(),
        "offset {offset} value {value}"
    );
}
#[test]
fn malformed_larger_counts_visibility_horizon_and_publication_are_rejected() {
    for n in [0, 7, 11, 12, 83, fixture::PACK.len() - 1] {
        assert!(fixture::read(&fixture::PACK[..n]).is_err());
    }
    for (at, value) in [
        (8, 0),
        (8, 20),
        (12, 129),
        (16, 32769),
        (20, 17),
        (20, 13),
        (24, 13),
        (28, 13),
        (32, 7),
        (36, 2),
        (40, 4096),
        (44, 2),
        (48, 1),
        (48, 5),
        (52, 32833),
        (56, 4098),
        (60, 0),
        (68, 0),
        (76, 0),
    ] {
        reject_word(at, value);
    }
    let mut at = 12;
    let mut horizon = false;
    for case in fixture::read(fixture::PACK).unwrap() {
        at += 72 + case.name.len() + case.source.len() + case.frame.len();
        for (bi, block) in case.blocks.iter().enumerate() {
            if bi == 0 && case.name == "wide_uniform" {
                for (word, value) in [
                    (0, 1),
                    (1, 8193),
                    (2, 3),
                    (3, 2),
                    (4, 2),
                    (5, 1),
                    (6, 32769),
                    (7, 0),
                    (10, 0),
                    (13, u32::MAX),
                    (19, 8193),
                ] {
                    reject_word(at + word * 4, value);
                }
            }
            if block.kind == 1 {
                reject_word(at + 16, (1 - block.previous_after) as u32);
            }
            at += 80;
            for (e, s) in block.events.iter().zip(&block.search_changes) {
                let size = event_bytes(e, s);
                if !horizon
                    && case.name == "wide_horizon"
                    && let Some(index) = e.path.iter().position(|(slot, _)| *slot == 4096)
                {
                    let path_count = at
                        + 40
                        + 12
                        + e.stretch.map_or(0, |_| 28)
                        + e.reverse.map_or(0, |_| 12)
                        + e.query.map_or(0, |_| 24 + 4 + 8 * e.matches.len());
                    reject_word(path_count + 4 + 32 * index, 4099);
                    reject_word(path_count, 4100);
                    horizon = true;
                }
                at += size;
            }
        }
    }
    assert!(horizon);
    assert_eq!(at, fixture::PACK.len());
    let mut bad = fixture::PACK.to_vec();
    bad.push(0);
    assert!(fixture::read(&bad).is_err());
}
#[test]
fn every_strict_larger_frame_prefix_and_changed_termination_fail() {
    for case in fixture::read(fixture::PACK).unwrap() {
        for n in 0..case.frame.len() {
            assert!(
                zstdoxide::decompress(&case.frame[..n], 32768).is_err(),
                "{} prefix {n}",
                case.name
            );
        }
        let mut bad = case.frame.to_vec();
        *bad.last_mut().unwrap() ^= 0x80;
        if let Ok(decoded) = zstdoxide::decompress(&bad, 32768) {
            assert_ne!(decoded, case.source);
        }
    }
}

#[test]
fn new_source_boundaries_preserve_balanced_ranges_and_late_publication() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for (name, lengths) in [
        ("wide_frames_uniform_16385", &[5462, 5462, 5461][..]),
        ("wide_frames_uniform_24576", &[8192, 8192, 8192][..]),
        ("wide_frames_uniform_24577", &[6145, 6144, 6144, 6144][..]),
        ("wide_frames_uniform_32768", &[8192, 8192, 8192, 8192][..]),
    ] {
        let case = cases.iter().find(|c| c.name == name).unwrap();
        assert_eq!(
            case.blocks.iter().map(|b| b.length).collect::<Vec<_>>(),
            lengths
        );
        assert_eq!(case.blocks[0].kind, 2);
        for block in &case.blocks[1..] {
            assert_eq!(block.kind, 1);
            assert_eq!(block.previous_before, block.previous_after);
            assert_eq!(block.owners[block.previous_after], block.incoming);
        }
    }
    let mixed = cases
        .iter()
        .find(|c| c.name == "wide_frames_mixed")
        .unwrap();
    assert_eq!(
        mixed.blocks.iter().map(|b| b.kind).collect::<Vec<_>>(),
        [2, 0, 1, 2]
    );
    assert_eq!(mixed.blocks[1].incoming, mixed.blocks[2].incoming);
    assert_eq!(mixed.blocks[2].incoming, mixed.blocks[3].incoming);
    assert!(
        mixed.blocks[3]
            .events
            .iter()
            .flat_map(|e| &e.matches)
            .any(|m| m[0] > 8192)
    );
}
