//! Independent lifecycle decoding, predecessor identity and reset-boundary forgeries.
#![forbid(unsafe_code)]
#[path = "support/ultra2_fixture.rs"]
mod fixture;
#[path = "support/ultra_fresh_fixture.rs"]
mod ordinary;
#[path = "support/ultra_path_fixture.rs"]
mod ultra_path_fixture;
use std::sync::OnceLock;
static INPUT: &[u8] = fixture::PACK;
fn cases() -> &'static [fixture::Case<'static>] {
    static CASES: OnceLock<Vec<fixture::Case<'static>>> = OnceLock::new();
    CASES.get_or_init(|| fixture::read(INPUT).expect("qualified native lifecycle fixture"))
}
fn single(case: &fixture::Case<'_>) -> Vec<u8> {
    let mut bytes = b"ZU2FX01\0".to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(case.encoded);
    bytes
}
fn word(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
#[test]
fn complete_lifecycle_inventory_and_borrowed_source_ranges() {
    let cases = cases();
    assert_eq!(cases.len(), 53);
    assert_eq!(
        cases
            .iter()
            .flat_map(|c| &c.phases)
            .map(|p| p.events.len())
            .sum::<usize>(),
        394656
    );
    let mut changed = Vec::new();
    for case in cases {
        let base = INPUT.as_ptr() as usize;
        let end = base + INPUT.len();
        assert!((base..end).contains(&(case.name.as_ptr() as usize)));
        assert!((base..end).contains(&(case.source.as_ptr() as usize)));
        assert!((base..end).contains(&(case.encoded.as_ptr() as usize)));
        assert_eq!(
            case.phases.len(),
            if case.requested_seed && case.source.len() > 8 {
                2
            } else {
                1
            }
        );
        for phase in &case.phases {
            assert_eq!(phase.incoming, [1, 4, 8]);
            assert_eq!(phase.reset.is_some(), phase.seeding);
            assert_eq!(phase.events.len(), phase.search_changes.len());
        }
        if case.phases.len() == 2 {
            let first = &case.phases[0];
            let second = &case.phases[1];
            let reset = first.reset.as_ref().unwrap();
            assert_eq!(reset.origin_before, 2);
            assert_eq!(reset.origin_after, case.source.len() as u32 + 2);
            assert_eq!(first.final_frequency, second.initial_frequency);
            assert_eq!(reset.frequency, first.final_frequency);
            assert_eq!(reset.caller_repeats, [1, 4, 8]);
            assert_eq!(reset.discarded_repeats, first.candidate);
            let a: Vec<_> = first.events.iter().flat_map(|e| &e.appended).collect();
            let b: Vec<_> = second.events.iter().flat_map(|e| &e.appended).collect();
            if a != b {
                changed.push(case.name);
            }
        }
    }
    assert_eq!(
        changed,
        ["u2_seed_alternative_seed_0", "u2_seed_alternative_seed_2"]
    );
    for length in [7, 8, 9] {
        let case = cases
            .iter()
            .find(|c| c.name == format!("u2_threshold_{length}"))
            .unwrap();
        assert_eq!(case.phases.len(), if length > 8 { 2 } else { 1 });
        assert_eq!(case.phases[0].final_frequency[385], u32::from(length <= 8));
    }
}
fn same(a: &ultra_path_fixture::Event, b: &ultra_path_fixture::Event) {
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
        (
            a.stretch,
            a.reverse,
            a.query,
            a.alternative,
            a.sequence_count
        ),
        (
            b.stretch,
            b.reverse,
            b.query,
            b.alternative,
            b.sequence_count
        )
    );
    assert_eq!(a.matches, b.matches);
    assert_eq!(a.path, b.path);
    assert_eq!(a.frequencies, b.frequencies);
    assert_eq!(a.appended, b.appended);
}
#[test]
fn all_44_predecessor_first_passes_preserve_every_ordinary_state() {
    let old = ordinary::read(ordinary::PACK).unwrap();
    let mut checked = 0;
    for case in cases() {
        let suffix = case
            .name
            .strip_prefix("u2_skip_")
            .or_else(|| case.name.strip_prefix("u2_seed_"));
        let Some(suffix) = suffix else { continue };
        let prior = old
            .iter()
            .find(|c| c.name == format!("uf_p2_{suffix}"))
            .unwrap();
        assert_eq!(case.source, prior.source);
        assert_eq!(
            (
                case.window,
                case.hash,
                case.chain,
                case.search,
                case.minimum,
                case.target,
                case.literal_mode
            ),
            (
                prior.window,
                prior.hash,
                prior.chain,
                prior.search,
                prior.minimum,
                prior.target,
                prior.literal_mode
            )
        );
        let phase = &case.phases[0];
        let block = &prior.blocks[0];
        assert_eq!(phase.events.len(), block.events.len());
        for (a, b) in phase.events.iter().zip(&block.events) {
            same(a, b);
        }
        assert_eq!(phase.search_changes, block.search_changes);
        assert_eq!(phase.candidate, block.candidate);
        assert_eq!(phase.tail, block.tail);
        checked += 1;
    }
    assert_eq!(checked, 44);
}
#[test]
fn malformed_extents_and_types_fail_before_unbounded_allocation() {
    for n in 0..12 {
        assert!(fixture::read(&fixture::PACK[..n]).is_err());
    }
    let case = &cases()[1];
    let valid = single(case);
    for (at, value) in [
        (8, 0),
        (8, 54),
        (12, 0),
        (12, 129),
        (16, 6),
        (16, 16385),
        (20, 9),
        (20, 17),
        (24, 5),
        (28, 13),
        (32, 0),
        (36, 8),
        (40, 4096),
        (44, 2),
        (48, 2),
        (52, 3),
        (56, 4098),
    ] {
        let mut bytes = valid.clone();
        word(&mut bytes, at, value);
        assert!(fixture::read(&bytes).is_err(), "field {at}={value}");
    }
    for n in [12, 20, valid.len() - 1] {
        assert!(fixture::read(&valid[..n]).is_err());
    }
    let mut bytes = valid.clone();
    bytes.push(0);
    assert!(fixture::read(&bytes).is_err());
    let mut bytes = valid;
    bytes[0] ^= 1;
    assert!(fixture::read(&bytes).is_err());
}
#[test]
fn discarded_state_and_rebased_history_cannot_be_coherently_republished() {
    let case = cases()
        .iter()
        .find(|c| c.name == "u2_seed_fresh_startup_uniform")
        .unwrap();
    let valid = single(case);
    let first = &case.phases[0];
    let second = &case.phases[1];
    let entry = 12 + 72 + case.name.len() + case.source.len();
    let mut offsets: Vec<_> = (0..15).map(|i| entry + 4 * i).collect();
    offsets.extend((0..15).map(|i| 12 + first.reset_offset.unwrap() + 4 * i));
    offsets.extend([
        12 + first.header_offset,
        12 + second.header_offset,
        16 + second.header_offset,
        20 + second.header_offset,
        32 + second.header_offset,
        36 + second.header_offset,
        12 + first.reset_offset.unwrap() + 60,
        12 + second.footer_offset,
        28 + second.footer_offset,
    ]);
    for at in offsets {
        let mut bytes = valid.clone();
        let old = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        word(&mut bytes, at, old ^ 1);
        assert!(fixture::read(&bytes).is_err(), "lifecycle word {at}");
    }
    let mut bytes = valid.clone();
    word(&mut bytes, 16 + second.header_offset, 2);
    assert!(fixture::read(&bytes).is_err());
    let mut bytes = valid;
    word(&mut bytes, 12 + first.footer_offset + 20 + 386 * 4, 0);
    assert!(fixture::read(&bytes).is_err());
}
