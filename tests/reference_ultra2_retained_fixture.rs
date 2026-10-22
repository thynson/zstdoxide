//! Independent complete-state reader and native-frame checks before parser admission.
#![forbid(unsafe_code)]
#[path = "support/ultra2_retained_fixture.rs"]
mod fixture;
#[path = "support/ultra_path_fixture.rs"]
mod ultra_path_fixture;
use std::sync::OnceLock;
static INPUT: &[u8] = fixture::PACK;
fn cases() -> &'static [fixture::Case<'static>] {
    static CASES: OnceLock<Vec<fixture::Case<'static>>> = OnceLock::new();
    CASES.get_or_init(|| fixture::read(INPUT).expect("qualified retained Ultra2 fixture"))
}
fn case(name: &str) -> &'static fixture::Case<'static> {
    cases().iter().find(|c| c.name == name).unwrap()
}
fn single(case: &fixture::Case<'_>) -> Vec<u8> {
    let mut bytes = b"ZU2RFX01".to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(case.encoded);
    bytes
}
fn reject_word(bytes: &[u8], offset: usize, value: u32) {
    let mut bad = bytes.to_vec();
    bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    assert!(
        fixture::read(&bad).is_err(),
        "offset {offset}, value {value}"
    );
}
#[test]
fn complete_native_lifecycle_inventory_and_borrowed_ranges() {
    let cases = cases();
    assert_eq!(cases.len(), 32);
    assert_eq!(cases.iter().map(|c| c.blocks.len()).sum::<usize>(), 93);
    let phases: Vec<_> = cases
        .iter()
        .flat_map(|c| &c.blocks)
        .flat_map(|b| &b.phases)
        .collect();
    assert_eq!(phases.len(), 123);
    assert_eq!(phases.iter().filter(|p| p.seeding).count(), 30);
    assert_eq!(phases.iter().map(|p| p.events.len()).sum::<usize>(), 229657);
    let base = INPUT.as_ptr() as usize;
    let end = base + INPUT.len();
    for case in cases {
        for bytes in [case.encoded, case.name.as_bytes(), case.source, case.frame] {
            let start = bytes.as_ptr() as usize;
            assert!(base <= start && start + bytes.len() <= end);
        }
        let first = &case.blocks[0];
        let bias = if first.length > 8 {
            first.length as u32
        } else {
            0
        };
        for (index, block) in case.blocks.iter().enumerate() {
            assert_eq!(
                block.phases.len(),
                if index == 0 && bias > 0 { 2 } else { 1 }
            );
            for phase in &block.phases {
                assert_eq!(phase.origin, if phase.seeding { 2 } else { bias + 2 });
                assert_eq!(phase.incoming, block.incoming);
                assert_eq!(phase.reset.is_some(), phase.seeding);
                assert_eq!(phase.events.len(), phase.search_changes.len());
                if let Some(reset) = &phase.reset {
                    assert_eq!(reset.origin_after, first.length as u32 + 2);
                    assert_eq!(reset.frequency, phase.final_frequency);
                    assert_eq!(reset.caller_repeats, block.incoming);
                    assert_eq!(reset.discarded_repeats, phase.candidate);
                }
            }
        }
    }
    assert_eq!(
        cases
            .iter()
            .flat_map(|c| c.blocks.iter().skip(1))
            .flat_map(|b| &b.phases)
            .flat_map(|p| &p.events)
            .filter(|e| e.alternative.is_some())
            .count(),
        572
    );
}
#[test]
fn every_native_complete_frame_decodes_with_rust_codec() {
    for case in cases() {
        assert_eq!(
            zstdoxide::decompress(case.frame, 32768).unwrap(),
            case.source,
            "{}",
            case.name
        );
    }
}
#[test]
fn first_and_later_thresholds_have_distinct_bootstrap_and_wire_boundaries() {
    for length in [7, 8, 9] {
        let first = case(&format!("u2r_first_threshold_{length}"));
        assert_eq!(first.blocks[0].phases.len(), if length > 8 { 2 } else { 1 });
        assert_eq!(first.blocks[0].kind, if length == 7 { 0 } else { 2 });
        let later = case(&format!("u2r_later_threshold_{length}"));
        assert_eq!(later.blocks[0].phases.len(), 2);
        assert_eq!(later.blocks[1].phases.len(), 1);
        assert_eq!(later.blocks[1].kind, 1);
    }
    for first in [65, 95, 97] {
        let case = case(&format!("u2r_ring_{first}"));
        assert_eq!(case.blocks[1].phases[0].origin, first + 2);
        assert_ne!(
            case.blocks[1].phases[0].origin,
            case.source.len() as u32 + 2
        );
    }
}
#[test]
fn future_source_changes_preserve_every_first_block_phase_state() {
    let original = case("u2r_alternatives_0202");
    for mask in [128, 255] {
        let changed = case(&format!("u2r_future_{mask}"));
        assert_ne!(changed.source, original.source);
        for (a, b) in original.blocks[0]
            .phases
            .iter()
            .zip(&changed.blocks[0].phases)
        {
            assert_eq!(format!("{:?}", a.events), format!("{:?}", b.events));
            assert_eq!(a.search_changes, b.search_changes);
            assert_eq!(a.initial_frequency, b.initial_frequency);
            assert_eq!(a.final_frequency, b.final_frequency);
            assert_eq!(
                (a.incoming, a.candidate, a.tail),
                (b.incoming, b.candidate, b.tail)
            );
        }
    }
}
#[test]
fn truncated_and_oversized_fixture_domains_reject_before_access() {
    let c = case("u2r_first_threshold_7");
    let data = single(c);
    for end in [0, 7, 8, 11, 12, 79, data.len() / 2, data.len() - 1] {
        assert!(fixture::read(&data[..end]).is_err());
    }
    let mut extra = data.clone();
    extra.push(0);
    assert!(fixture::read(&extra).is_err());
    for (offset, value) in [
        (8, 0),
        (8, 33),
        (12, u32::MAX),
        (16, 13),
        (16, 32769),
        (20, 9),
        (24, 13),
        (28, 13),
        (32, 7),
        (36, 8),
        (40, 4096),
        (44, 2),
        (48, 1),
        (48, 5),
        (52, 32833),
        (56, 4100),
        (60, 0),
        (64, 0),
    ] {
        reject_word(&data, offset, value);
    }
}
#[test]
fn eligibility_discarded_reset_and_later_owner_forgeries_reject() {
    let c = case("u2r_alternatives_0202");
    let bytes = single(c);
    for block in &c.blocks {
        let base = 12 + block.header_offset;
        for (index, value) in [
            (0, block.start + 1),
            (1, 8193),
            (2, 3),
            (3, 1 - block.previous_before as u32),
            (4, 1 - block.previous_after as u32),
            (5, block.pre_frontier + 1),
            (6, 3),
            (7, block.incoming[0] + 1),
            (10, block.candidate[0] + 1),
            (19, block.tail + 1),
            (20, u32::MAX),
        ] {
            reject_word(&bytes, base + 4 * index, value);
        }
        for (index, value) in block.eligibility.iter().enumerate() {
            reject_word(&bytes, 12 + block.eligibility_offset + 4 * index, value + 1);
        }
        for phase in &block.phases {
            reject_word(&bytes, 12 + phase.header_offset + 4, phase.origin + 1);
            reject_word(&bytes, 12 + phase.header_offset + 20, 32769);
            reject_word(&bytes, 12 + phase.footer_offset, phase.candidate[0] + 1);
            if let Some(reset) = phase.reset_offset {
                for index in 0..15 {
                    let at = 12 + reset + 4 * index;
                    let value = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
                    reject_word(&bytes, at, value + 1);
                }
            }
        }
    }
}
fn event_prefix(e: &ultra_path_fixture::Event) -> usize {
    40 + 4
        + e.stretch.map_or(0, |_| 28)
        + 4
        + e.reverse.map_or(0, |_| 12)
        + 4
        + e.query.map_or(0, |_| 24 + 4 + 8 * e.matches.len())
}
fn event_bytes(e: &ultra_path_fixture::Event, search: &[(usize, u32)]) -> usize {
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
        + 8 * search.len()
}
#[test]
fn later_query_path_frequency_and_search_deltas_are_independently_checked() {
    let c = case("u2r_alternatives_0202");
    let bytes = single(c);
    let mut checked = [false; 4];
    for phase in &c.blocks[1].phases {
        let mut at = 12 + phase.header_offset + (6 + 386) * 4;
        for (event, search) in phase.events.iter().zip(&phase.search_changes) {
            if !checked[0] && event.query.is_some() {
                let query = at
                    + 40
                    + 4
                    + event.stretch.map_or(0, |_| 28)
                    + 4
                    + event.reverse.map_or(0, |_| 12)
                    + 4;
                reject_word(&bytes, query, c.source.len() as u32);
                checked[0] = true;
            }
            let path = at + event_prefix(event) + 4 + event.alternative.map_or(0, |_| 96);
            if !checked[1] && !event.path.is_empty() {
                reject_word(&bytes, path + 4, 4099);
                checked[1] = true;
            }
            let frequency = path + 4 + 32 * event.path.len();
            if !checked[2] && !event.frequencies.is_empty() {
                reject_word(&bytes, frequency + 4, 386);
                checked[2] = true;
            }
            let search_at =
                frequency + 4 + 8 * event.frequencies.len() + 8 + 16 * event.appended.len();
            if !checked[3] && !search.is_empty() {
                reject_word(&bytes, search_at + 8, u32::MAX);
                checked[3] = true;
            }
            at += event_bytes(event, search);
        }
        assert_eq!(at, 12 + phase.footer_offset);
    }
    assert_eq!(checked, [true; 4]);
    // Change a table cell, its sum and its fresh fractional cache together.
    // Independent source-derived initialization must still reject this state.
    let phase = &c.blocks[0].phases[0];
    let event = &phase.events[0];
    let at = 12 + phase.header_offset + (6 + 386) * 4;
    let path = at + event_prefix(event) + 4 + event.alternative.map_or(0, |_| 96);
    let frequency = path + 4 + 32 * event.path.len();
    let value = |index| {
        event
            .frequencies
            .iter()
            .find(|(at, _)| *at == index)
            .unwrap()
            .1
    };
    let sum = value(378) + 1;
    let adjusted = u64::from(sum) + 1;
    let exponent = adjusted.ilog2();
    let cache = exponent * 256 + (adjusted * 256 / (1u64 << exponent)) as u32;
    let mut bad = bytes.clone();
    for (index, value) in [(256, value(256) + 1), (378, sum), (382, cache)] {
        let slot = event
            .frequencies
            .iter()
            .position(|(at, _)| *at == index)
            .unwrap();
        let offset = frequency + 8 + slot * 8;
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(
        fixture::read(&bad).unwrap_err(),
        "exact independently rescaled begin frequencies"
    );
}
