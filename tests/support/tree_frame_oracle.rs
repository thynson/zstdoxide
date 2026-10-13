//! Checked borrowed ordinary binary-tree trajectories and complete C frames.
#![forbid(unsafe_code)]
use crate::greedy_frame_oracle_fixture::OracleFrameCase;
use alloc::vec::Vec;
const FIXTURE: &[u8] = include_bytes!("../fixtures/reference-tree-frames/cases.bin");
const CASES: usize = 245;
const BLOCKS: usize = 531;
type Result<T> = core::result::Result<T, &'static str>;
fn take<'a>(input: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    let b = input.get(..n).ok_or("truncated tree trajectory")?;
    *input = &input[n..];
    Ok(b)
}
fn size(input: &mut &[u8]) -> Result<usize> {
    usize::try_from(u32::from_le_bytes(
        take(input, 4)?.try_into().map_err(|_| "size width")?,
    ))
    .map_err(|_| "target size")
}
pub(crate) fn cases() -> Result<Vec<OracleFrameCase<'static>>> {
    decode(FIXTURE)
}
fn decode(mut bytes: &[u8]) -> Result<Vec<OracleFrameCase<'_>>> {
    if bytes.len() > 256 << 20 {
        return Err("tree trajectory collection cap");
    }
    if take(&mut bytes, 8)? != b"ZBTFR01\0"
        || size(&mut bytes)? != CASES
        || size(&mut bytes)? != BLOCKS
    {
        return Err("tree collection identity");
    }
    let mut result = Vec::new();
    result.try_reserve_exact(CASES).map_err(|_| "allocation")?;
    let mut blocks = 0usize;
    for _ in 0..CASES {
        let n = size(&mut bytes)?;
        if n > 64 << 20 {
            return Err("tree case extent");
        }
        let mut record = take(&mut bytes, n)?;
        if take(&mut record, 4)? != [6, 0, 0, 0] {
            return Err("tree selector/reserved");
        }
        let case = OracleFrameCase::read_tree(record)?;
        if result
            .iter()
            .any(|c: &OracleFrameCase<'_>| c.name == case.name)
        {
            return Err("duplicate tree name");
        }
        blocks = blocks
            .checked_add(case.blocks.len())
            .ok_or("tree block count overflow")?;
        result.push(case);
    }
    if !bytes.is_empty() || blocks != BLOCKS {
        return Err("tree collection exhaustion/count");
    }
    Ok(result)
}
#[test]
fn all_tree_records_replay_complete_maps_sequences_and_publication() {
    let all = cases().unwrap();
    assert_eq!(all.iter().map(|c| c.source.len()).sum::<usize>(), 7640096);
    assert_eq!(all.iter().filter(|c| c.whole_frame.is_some()).count(), 120);
    for c in all {
        assert!(c.tree_search && !c.row_search);
        c.validate().unwrap_or_else(|e| panic!("{}: {e}", c.name));
        for b in core::iter::once(c.source)
            .chain(core::iter::once(c.frame))
            .chain(c.whole_frame)
        {
            let base = FIXTURE.as_ptr() as usize;
            let start = b.as_ptr() as usize;
            assert!(start >= base && start - base <= FIXTURE.len() - b.len());
        }
    }
}
#[test]
fn bounded_tree_reader_rejects_identity_extent_selector_and_trailing_bytes() {
    for n in [0, 7, 15, 16, 19, FIXTURE.len() - 1] {
        assert!(decode(&FIXTURE[..n]).is_err());
    }
    let mut b = FIXTURE.to_vec();
    b.push(0);
    assert!(decode(&b).is_err());
    for (offset, value) in [(8, 244_u32), (12, 532), (16, u32::MAX), (20, 5), (20, 262)] {
        let mut b = FIXTURE.to_vec();
        b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(decode(&b).is_err());
    }
}
#[test]
fn actual_tree_marker_and_geometry_forgeries_reject() {
    let all = cases().unwrap();
    let original = all
        .iter()
        .find(|c| {
            c.blocks
                .iter()
                .any(|b| b.second_table.changes.iter().any(|x| x[2] == 1))
        })
        .unwrap();
    let mut c = original.clone();
    c.row_search = true;
    assert!(c.validate().is_err());
    let mut c = original.clone();
    c.chain_log = 20;
    assert!(c.validate().is_err());
    let mut c = original.clone();
    c.tree_search = false;
    assert!(c.validate().is_err());
    let mut c = original.clone();
    let b = c
        .blocks
        .iter_mut()
        .find(|b| b.second_table.changes.iter().any(|x| x[2] == 1))
        .unwrap();
    let cell = b
        .second_table
        .changes
        .iter_mut()
        .find(|x| x[2] == 1)
        .unwrap();
    cell[0] &= !1;
    assert!(c.validate().is_err());
}

#[test]
fn every_complete_c_tree_frame_decodes_to_its_original_source() {
    let mut frames = 0;
    let mut bytes = 0;
    for case in cases().unwrap() {
        for frame in core::iter::once(case.frame).chain(case.whole_frame) {
            assert_eq!(
                crate::decompress(frame, case.source.len()).unwrap(),
                case.source,
                "{}",
                case.name
            );
            frames += 1;
            bytes += case.source.len();
        }
    }
    assert_eq!((frames, bytes), (365, 14_136_238));
}
