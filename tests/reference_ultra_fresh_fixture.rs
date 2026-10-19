//! Independent ordinary Ultra frames, complete states and malformed-fixture checks.
#![forbid(unsafe_code)]
#[path = "support/ultra_fresh_fixture.rs"]
mod fixture;
#[path = "support/optimal_path_fixture.rs"]
mod optimal_path_fixture;
#[path = "support/optimal_fresh_wide_fixture.rs"]
mod original;
#[path = "support/ultra_path_fixture.rs"]
mod ultra_path_fixture;

#[test]
fn every_ordinary_frame_and_complete_state_inventory_is_valid() {
    let cases = fixture::read(fixture::PACK).unwrap();
    assert_eq!(cases.len(), 44);
    assert_eq!(cases.iter().filter(|c| c.policy == 2).count(), 22);
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
    let events: Vec<_> = cases.iter().flat_map(|c| &c.blocks[0].events).collect();
    assert_eq!(events.len(), 198476);
    assert_eq!(events.iter().filter(|e| e.query.is_some()).count(), 95581);
    assert_eq!(
        events.iter().filter(|e| e.alternative.is_some()).count(),
        201
    );
    assert_eq!(events.iter().map(|e| e.appended.len()).sum::<usize>(), 353);
    assert!(events.iter().flat_map(|e| &e.matches).any(|m| m[1] > 8192));
    assert!(cases.iter().any(|c| c.short_cells() == 65536));
}

#[test]
fn every_original_optimal_request_frame_and_state_is_preserved() {
    let cases = fixture::read(fixture::PACK).unwrap();
    for old in original::read(original::PACK).unwrap() {
        let new = cases
            .iter()
            .find(|c| c.name == format!("uf_p0_{}", old.name))
            .unwrap();
        assert_eq!(new.source, old.source);
        assert_eq!(new.frame, old.frame);
        assert_eq!(new.allocations, old.allocations);
        assert_eq!(
            (
                new.window,
                new.hash,
                new.chain,
                new.search,
                new.minimum,
                new.target,
                new.literal_mode
            ),
            (
                old.window,
                old.hash,
                old.chain,
                old.search,
                old.minimum,
                old.target,
                old.literal_mode
            )
        );
        let (a, b) = (&new.blocks[0], &old.blocks[0]);
        assert_eq!(a.search_changes, b.search_changes);
        assert_eq!(
            (
                a.kind,
                a.previous_before,
                a.previous_after,
                a.incoming,
                a.candidate,
                a.owners,
                a.tail
            ),
            (
                b.kind,
                b.previous_before,
                b.previous_after,
                b.incoming,
                b.candidate,
                b.owners,
                b.tail
            )
        );
        assert_eq!(a.events.len(), b.events.len());
        for (a, b) in a.events.iter().zip(&b.events) {
            assert!(a.alternative.is_none());
            assert_eq!(
                (
                    a.stage,
                    a.reverse_phase,
                    a.ip,
                    a.anchor,
                    a.cur,
                    a.last,
                    a.tail,
                    a.repeats
                ),
                (
                    b.stage,
                    b.reverse_phase,
                    b.ip,
                    b.anchor,
                    b.cur,
                    b.last,
                    b.tail,
                    b.repeats
                )
            );
            assert_eq!(
                (a.stretch, a.reverse, a.query),
                (b.stretch, b.reverse, b.query)
            );
            assert_eq!(a.matches, b.matches);
            assert_eq!(a.path, b.path);
            assert_eq!(a.frequencies, b.frequencies);
            assert_eq!(a.sequence_count, b.sequence_count);
            assert_eq!(a.appended, b.appended);
        }
    }
}

fn event_prefix(e: &ultra_path_fixture::Event) -> usize {
    40 + 12
        + e.stretch.map_or(0, |_| 28)
        + e.reverse.map_or(0, |_| 12)
        + e.query.map_or(0, |_| 28 + 8 * e.matches.len())
}
fn event_bytes(e: &ultra_path_fixture::Event, s: &[(usize, u32)]) -> usize {
    event_prefix(e)
        + 4
        + e.alternative.map_or(0, |_| 96)
        + 4
        + 32 * e.path.len()
        + 4
        + 8 * e.frequencies.len()
        + 8
        + 16 * e.appended.len()
        + 4
        + 8 * s.len()
}
fn isolated(name: &str) -> Vec<u8> {
    let mut at = 12;
    let mut found = None;
    for case in fixture::read(fixture::PACK).unwrap() {
        let start = at;
        at += 76 + case.name.len() + case.source.len() + case.frame.len();
        for block in &case.blocks {
            at += 80;
            for (e, s) in block.events.iter().zip(&block.search_changes) {
                at += event_bytes(e, s);
            }
        }
        if case.name == name {
            let mut data = b"ZUFFX01\0".to_vec();
            data.extend(1u32.to_le_bytes());
            data.extend_from_slice(&fixture::PACK[start..at]);
            found = Some(data);
        }
    }
    assert_eq!(at, fixture::PACK.len());
    found.unwrap()
}
fn reject_word(data: &[u8], at: usize, value: u32) {
    let mut bad = data.to_vec();
    bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
    assert!(fixture::read(&bad).is_err(), "offset {at} value {value}");
}

#[test]
fn ordinary_header_search_and_publication_forgeries_are_rejected() {
    let data = isolated("uf_p2_alternative_seed_0");
    let cases = fixture::read(&data).unwrap();
    let case = &cases[0];
    for n in [0, 7, 11, 12, 87, data.len() - 1] {
        assert!(fixture::read(&data[..n]).is_err());
    }
    for (at, value) in [
        (8, 0),
        (8, 45),
        (12, 0),
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
        (60, 1),
        (60, 0),
        (64, 0),
        (72, 0),
        (80, 0),
    ] {
        reject_word(&data, at, value);
    }
    let block = 12 + 76 + case.name.len() + case.source.len() + case.frame.len();
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
        (19, 625),
    ] {
        reject_word(&data, block + word * 4, value);
    }
    let mut at = block + 80;
    let mut query = false;
    for (e, s) in case.blocks[0]
        .events
        .iter()
        .zip(&case.blocks[0].search_changes)
    {
        let size = event_bytes(e, s);
        if !query && e.query.is_some() && !s.is_empty() {
            reject_word(
                &data,
                at + size - 8 * s.len() - 4,
                (case.search_cells() + 1) as u32,
            );
            reject_word(&data, at + size - 8 * s.len(), case.search_cells() as u32);
            query = true;
        }
        at += size;
    }
    assert!(query);
    assert_eq!(at, data.len());
    let mut bad = data.clone();
    bad.push(0);
    assert!(fixture::read(&bad).is_err());
}

#[test]
fn all_signed_alternative_operands_and_following_publication_are_checked() {
    let data = isolated("uf_p2_alternative_seed_0");
    let cases = fixture::read(&data).unwrap();
    let case = &cases[0];
    let mut at = 12 + 76 + case.name.len() + case.source.len() + case.frame.len() + 80;
    let mut considered = 0;
    let mut selected = 0;
    let mut rejected = 0;
    for (e, s) in case.blocks[0]
        .events
        .iter()
        .zip(&case.blocks[0].search_changes)
    {
        let size = event_bytes(e, s);
        if let Some(a) = e.alternative {
            considered += 1;
            if a[8] < a[9] && a[8] < a[10] {
                selected += 1;
            } else {
                rejected += 1;
            }
            if considered == 1 {
                let alternative = at + event_prefix(e) + 4;
                for (column, value) in a.into_iter().enumerate() {
                    let mut bad = data.clone();
                    let pos = alternative + column * 8;
                    bad[pos..pos + 8].copy_from_slice(&(value + 1).to_le_bytes());
                    assert!(fixture::read(&bad).is_err(), "alternative operand {column}");
                }
                reject_word(&data, at, 4);
                reject_word(&data, alternative - 4, 2);
                reject_word(&data, at + size + 20, 0);
            }
        }
        at += size;
    }
    assert_eq!(considered, 72);
    assert!(selected > 0 && rejected > 0);
}
