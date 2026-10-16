//! Independent complete fresh C frames and sparse horizon admission.
#![forbid(unsafe_code)]
#[path = "support/optimal_fresh_wide_fixture.rs"]
mod fixture;
#[path = "support/optimal_path_fixture.rs"]
mod optimal_path_fixture;

#[test]
fn complete_fresh_frames_and_full_event_totals_are_independent() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 20);
    for case in &cases {
        assert_eq!(case.blocks.len(), 1);
        assert_eq!(case.blocks[0].length, case.source.len());
        assert_eq!(
            zstdoxide::decompress(case.frame, 16384).unwrap(),
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
    assert_eq!(events.len(), 64610);
    assert_eq!(events.iter().filter(|e| e.query.is_some()).count(), 30314);
    assert_eq!(events.iter().map(|e| e.appended.len()).sum::<usize>(), 48);
    assert!(events.iter().flat_map(|e| &e.matches).any(|m| m[1] > 8192));
}
#[test]
fn full_source_horizon_and_publication_preserve_complete_state() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let horizon = cases
        .iter()
        .find(|c| c.name == "fresh_wide_horizon")
        .unwrap();
    assert!(
        horizon.blocks[0]
            .events
            .iter()
            .any(|e| e.last == Some(4195))
    );
    assert!(
        horizon.blocks[0]
            .events
            .iter()
            .flat_map(|e| &e.path)
            .any(|(at, _)| *at == 4096)
    );
    let noise = cases.iter().find(|c| c.name == "fresh_noise16k").unwrap();
    let raw = &noise.blocks[0];
    assert_eq!(raw.kind, 0);
    assert_eq!(raw.incoming, [1, 4, 8]);
    assert_eq!(raw.previous_before, raw.previous_after);
    assert_eq!(raw.owners[raw.previous_after], [1, 4, 8]);
    let uniform = cases
        .iter()
        .find(|c| c.name == "fresh_wide_uniform")
        .unwrap();
    let compressed = &uniform.blocks[0];
    assert_eq!(compressed.kind, 2);
    assert_ne!(compressed.previous_before, compressed.previous_after);
    assert_eq!(
        compressed.owners[compressed.previous_after],
        compressed.candidate
    );
    let largest = cases
        .iter()
        .find(|c| c.name == "fresh_wide_window16")
        .unwrap();
    assert_eq!(largest.short_cells(), 65536);
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
fn malformed_full_fresh_counts_window_horizon_and_publication_fail() {
    for n in [0, 7, 11, 12, 83, fixture::PACK.len() - 1] {
        assert!(fixture::read(&fixture::PACK[..n]).is_err());
    }
    for (at, value) in [
        (8, 0),
        (8, 21),
        (12, 129),
        (16, 63),
        (16, 16385),
        (20, 9),
        (20, 17),
        (24, 13),
        (28, 13),
        (32, 7),
        (36, 2),
        (40, 4096),
        (44, 2),
        (48, 0),
        (48, 2),
        (52, 32769),
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
        if case.name == "fresh_wide_uniform" {
            reject_word(at + 8, 13);
        }
        at += 72 + case.name.len() + case.source.len() + case.frame.len();
        for block in &case.blocks {
            if case.name == "fresh_startup_uniform" {
                for (word, value) in [
                    (0, 1),
                    (1, 16385),
                    (2, 3),
                    (3, 2),
                    (4, 2),
                    (5, 1),
                    (6, 65537),
                    (7, 0),
                    (10, 0),
                    (13, u32::MAX),
                    (19, 65),
                ] {
                    reject_word(at + word * 4, value);
                }
            }
            at += 80;
            for (e, s) in block.events.iter().zip(&block.search_changes) {
                let size = event_bytes(e, s);
                if !horizon
                    && case.name == "fresh_wide_horizon"
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
fn strict_full_fresh_frame_prefixes_and_changed_termination_fail() {
    for case in fixture::read(fixture::PACK).unwrap() {
        for n in 0..case.frame.len() {
            assert!(
                zstdoxide::decompress(&case.frame[..n], 16384).is_err(),
                "{} prefix {n}",
                case.name
            );
        }
        let mut bad = case.frame.to_vec();
        *bad.last_mut().unwrap() ^= 0x80;
        if let Ok(decoded) = zstdoxide::decompress(&bad, 16384) {
            assert_ne!(decoded, case.source);
        }
    }
}
