//! Checked direct Lazy-family evidence. Depth is a copied selector; observation
//! spans borrow the immutable pack and metadata uses bounded fallible vectors.
//! The common reader verifies no selection algorithm and executes no C code.
#![forbid(unsafe_code)]

use crate::greedy_oracle_fixture::OracleCase;
use alloc::vec::Vec;

const FIXTURE: &[u8] = include_bytes!("../fixtures/reference-lazy/cases.bin");
const CASES: usize = 645;
type Result<T> = core::result::Result<T, &'static str>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LazyCase<'a> {
    pub(crate) depth: u8,
    pub(crate) observation: OracleCase<'a>,
}

fn take<'a>(input: &mut &'a [u8], size: usize) -> Result<&'a [u8]> {
    let bytes = input.get(..size).ok_or("truncated Lazy fixture")?;
    *input = &input[size..];
    Ok(bytes)
}

fn size(input: &mut &[u8]) -> Result<usize> {
    let bytes = take(input, 4)?.try_into().map_err(|_| "size width")?;
    usize::try_from(u32::from_le_bytes(bytes)).map_err(|_| "target size")
}

impl<'a> LazyCase<'a> {
    fn read(mut bytes: &'a [u8]) -> Result<Self> {
        let selector = take(&mut bytes, 4)?;
        let depth = selector[0];
        if depth > 2 || selector[1..] != [0; 3] {
            return Err("Lazy depth/reserved selector");
        }
        let observation = OracleCase::read(bytes)?;
        if !observation
            .name
            .as_bytes()
            .starts_with(&[b'd', b'0' + depth, b'-'])
        {
            return Err("Lazy name/depth mismatch");
        }
        Ok(Self { depth, observation })
    }
}

/// Decode the bounded structure. Dedicated tests independently replay every
/// sequence and complete table before parser-equivalence tests use the evidence.
pub(crate) fn cases() -> Result<Vec<LazyCase<'static>>> {
    decode(FIXTURE)
}

fn decode(mut bytes: &[u8]) -> Result<Vec<LazyCase<'_>>> {
    if bytes.len() > 64 << 20 {
        return Err("Lazy fixture size cap");
    }
    if take(&mut bytes, 8)? != b"ZLZST01\0" || size(&mut bytes)? != CASES {
        return Err("Lazy fixture identity");
    }
    let mut cases = Vec::new();
    cases.try_reserve_exact(CASES).map_err(|_| "allocation")?;
    for _ in 0..CASES {
        let length = size(&mut bytes)?;
        if length > 32 << 20 {
            return Err("Lazy record cap");
        }
        let case = LazyCase::read(take(&mut bytes, length)?)?;
        if cases
            .iter()
            .any(|c: &LazyCase<'_>| c.observation.name == case.observation.name)
        {
            return Err("duplicate Lazy case");
        }
        cases.push(case);
    }
    if !bytes.is_empty() {
        return Err("trailing Lazy fixture bytes");
    }
    Ok(cases)
}

#[test]
fn every_depth_replays_sequences_and_retains_borrowed_source_bounds() {
    let all = cases().unwrap();
    assert_eq!(all.len(), CASES);
    assert_eq!(all.iter().filter(|c| c.observation.row_search).count(), 342);
    assert_eq!(
        all.iter()
            .map(|c| c.observation.source.len())
            .sum::<usize>(),
        18_900_771
    );
    for depth in 0..=2 {
        assert_eq!(all.iter().filter(|c| c.depth == depth).count(), 215);
    }
    for case in all {
        let c = &case.observation;
        c.verify_sequences()
            .unwrap_or_else(|e| panic!("{}: {e}", c.name));
        let start = FIXTURE.as_ptr() as usize;
        let source = c.source.as_ptr() as usize;
        assert!(source >= start && source - start <= FIXTURE.len() - c.source.len());
    }
}

#[test]
fn every_depth_zero_predecessor_preserves_the_complete_greedy_record() {
    let all = cases().unwrap();
    let previous = crate::greedy_oracle_fixture::cases().unwrap();
    assert_eq!(previous.len(), 173);
    for old in previous {
        let new = all
            .iter()
            .find(|c| c.depth == 0 && c.observation.name.strip_prefix("d0-") == Some(old.name))
            .unwrap();
        let mut observation = new.observation.clone();
        observation.name = old.name;
        assert_eq!(observation, old);
    }
}

#[test]
fn complete_maps_and_seeded_rust_owners_match_all_lazy_inputs() {
    use crate::{reference_chain::ReferenceChain, row_table::RowTable};
    use alloc::alloc::Global;
    fn raw(index: usize) -> u32 {
        if index == usize::MAX {
            0
        } else {
            u32::try_from(index).unwrap().checked_add(2).unwrap()
        }
    }
    for case in cases().unwrap() {
        let c = case.observation;
        c.verify_tables()
            .unwrap_or_else(|e| panic!("{}: {e}", c.name));
        let layout = c.key_layout().unwrap();
        let (first, second) = if c.row_search {
            let row = c.row_layout().unwrap();
            let mut tables = RowTable::new_in(row, Global, Global).unwrap();
            for &p in &c.seeds {
                tables
                    .insert(
                        row.key(
                            layout.read_key(&c.source[p as usize..]).unwrap(),
                            c.initial_salt,
                        )
                        .unwrap(),
                        p as usize,
                    )
                    .unwrap();
            }
            let (positions, tags) = tables.test_maps();
            (
                positions.iter().map(|&p| raw(p)).collect::<Vec<_>>(),
                tags.iter().copied().map(u32::from).collect::<Vec<_>>(),
            )
        } else {
            let mut tables = ReferenceChain::new_in(layout, c.chain_log, Global, Global).unwrap();
            for &p in &c.seeds {
                tables
                    .insert(
                        layout.read_key(&c.source[p as usize..]).unwrap(),
                        p as usize,
                    )
                    .unwrap();
            }
            let (heads, links) = tables.test_maps();
            (
                heads.iter().map(|&p| raw(p)).collect::<Vec<_>>(),
                links.iter().map(|&p| raw(p)).collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            first,
            c.first_table.values(false).unwrap(),
            "{} first owner",
            c.name
        );
        assert_eq!(
            second,
            c.second_table.values(false).unwrap(),
            "{} second owner",
            c.name
        );
    }
}

#[test]
fn truncated_extents_bad_selectors_and_duplicate_records_reject() {
    for end in 0..16 {
        assert!(decode(&FIXTURE[..end]).is_err());
    }
    let mut rest = &FIXTURE[12..];
    let length = size(&mut rest).unwrap();
    let record = take(&mut rest, length).unwrap();
    for end in 0..record.len() {
        assert!(LazyCase::read(&record[..end]).is_err());
    }
    for (index, value, error) in [
        (0, 3, "Lazy depth/reserved selector"),
        (0, 1, "Lazy name/depth mismatch"),
        (1, 1, "Lazy depth/reserved selector"),
        (2, 1, "Lazy depth/reserved selector"),
        (3, 1, "Lazy depth/reserved selector"),
    ] {
        let mut bad = record.to_vec();
        bad[index] = value;
        assert_eq!(LazyCase::read(&bad).unwrap_err(), error);
    }
    let mut extra = record.to_vec();
    extra.push(0);
    assert_eq!(
        LazyCase::read(&extra).unwrap_err(),
        "trailing fixture bytes"
    );
    let mut count = FIXTURE[..12].to_vec();
    count[8..12].copy_from_slice(&644_u32.to_le_bytes());
    assert_eq!(decode(&count).unwrap_err(), "Lazy fixture identity");
    count[8..12].copy_from_slice(&645_u32.to_le_bytes());
    count.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode(&count).unwrap_err(), "Lazy record cap");
    let mut duplicate = FIXTURE[..16 + length].to_vec();
    duplicate.extend_from_slice(&FIXTURE[12..16 + length]);
    assert_eq!(decode(&duplicate).unwrap_err(), "duplicate Lazy case");
    assert!(decode(&FIXTURE[..FIXTURE.len() - 1]).is_err());
    let mut extra = FIXTURE.to_vec();
    extra.push(0);
    assert_eq!(decode(&extra).unwrap_err(), "trailing Lazy fixture bytes");
}

#[test]
fn malformed_lazy_observations_reject_in_shared_semantic_validation() {
    let all = cases().unwrap();
    for depth in [1, 2] {
        let original = &all
            .iter()
            .find(|c| {
                c.depth == depth && c.observation.name.ends_with("chain-explicit-depth-1-above")
            })
            .unwrap()
            .observation;
        for field in 0..5 {
            let mut c = original.clone();
            match field {
                0 => c.sequences[0].distance = 0,
                1 => c.sequences[0].raw_match_base ^= 1,
                2 => c.wire_repeats[0] ^= 1,
                3 => c.final_salt ^= 1,
                _ => c.final_entropy = 1,
            }
            assert!(c.verify_sequences().is_err());
        }
        let mut source = original.source.to_vec();
        source[original.prefix] ^= 1;
        let mut c = original.clone();
        c.source = &source;
        assert_eq!(c.verify_sequences(), Err("source hash"));
        let mut c = original.clone();
        c.first_table.hashes[1] ^= 1;
        assert_eq!(c.verify_tables(), Err("final owner fingerprint"));
        let mut c = original.clone();
        c.second_table.element_bytes = 1;
        assert_eq!(c.verify_tables(), Err("owner role geometry"));
    }
}
