//! Checked borrowed ordinary lazy-family trajectories and complete C frames.
//! Depth is copied metadata; shared semantic replay does not call the parser.
#![forbid(unsafe_code)]

use crate::greedy_frame_oracle_fixture::OracleFrameCase;
use alloc::vec::Vec;

const FIXTURE: &[u8] = include_bytes!("../fixtures/reference-lazy-frames/cases.bin");
const CASES: usize = 510;
const BLOCKS: usize = 1377;
type Result<T> = core::result::Result<T, &'static str>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LazyFrameCase<'a> {
    pub(crate) depth: u8,
    pub(crate) observation: OracleFrameCase<'a>,
}

fn take<'a>(input: &mut &'a [u8], length: usize) -> Result<&'a [u8]> {
    let bytes = input.get(..length).ok_or("truncated Lazy trajectory")?;
    *input = &input[length..];
    Ok(bytes)
}

fn size(input: &mut &[u8]) -> Result<usize> {
    let bytes = take(input, 4)?.try_into().map_err(|_| "size width")?;
    usize::try_from(u32::from_le_bytes(bytes)).map_err(|_| "target size")
}

impl<'a> LazyFrameCase<'a> {
    fn read(mut bytes: &'a [u8]) -> Result<Self> {
        let selector = take(&mut bytes, 4)?;
        let depth = selector[0];
        if depth > 2 || selector[1..] != [0; 3] {
            return Err("Lazy trajectory depth/reserved");
        }
        let observation = OracleFrameCase::read(bytes)?;
        if !observation
            .name
            .as_bytes()
            .starts_with(&[b'd', b'0' + depth, b'_'])
        {
            return Err("Lazy trajectory name/depth");
        }
        Ok(Self { depth, observation })
    }
}

/// Structural reading is bounded separately from the independent semantic
/// replay tests, which check every complete map, frame and publication record.
pub(crate) fn cases() -> Result<Vec<LazyFrameCase<'static>>> {
    decode(FIXTURE)
}

fn decode(mut bytes: &[u8]) -> Result<Vec<LazyFrameCase<'_>>> {
    if bytes.len() > 256 << 20 {
        return Err("Lazy trajectory collection cap");
    }
    if take(&mut bytes, 8)? != b"ZLZFR01\0"
        || size(&mut bytes)? != CASES
        || size(&mut bytes)? != BLOCKS
    {
        return Err("Lazy trajectory identity");
    }
    let mut result = Vec::new();
    result.try_reserve_exact(CASES).map_err(|_| "allocation")?;
    let mut blocks = 0usize;
    for _ in 0..CASES {
        let length = size(&mut bytes)?;
        if length > 32 << 20 {
            return Err("Lazy trajectory record cap");
        }
        let case = LazyFrameCase::read(take(&mut bytes, length)?)?;
        if result
            .iter()
            .any(|c: &LazyFrameCase<'_>| c.observation.name == case.observation.name)
        {
            return Err("duplicate Lazy trajectory");
        }
        blocks = blocks
            .checked_add(case.observation.blocks.len())
            .ok_or("block count overflow")?;
        result.push(case);
    }
    if !bytes.is_empty() || blocks != BLOCKS {
        return Err("Lazy trajectory exhaustion/count");
    }
    Ok(result)
}

#[test]
fn every_depth_replays_complete_maps_sequences_and_publication() {
    let all = cases().unwrap();
    assert_eq!(
        all.iter()
            .map(|c| c.observation.source.len())
            .sum::<usize>(),
        22_499_784
    );
    assert_eq!(all.iter().filter(|c| c.observation.row_search).count(), 279);
    for depth in 0..3 {
        assert_eq!(all.iter().filter(|c| c.depth == depth).count(), 170);
    }
    for case in all {
        let c = &case.observation;
        c.validate().unwrap_or_else(|e| panic!("{}: {e}", c.name));
        for bytes in core::iter::once(c.source)
            .chain(core::iter::once(c.frame))
            .chain(c.whole_frame)
        {
            let base = FIXTURE.as_ptr() as usize;
            let start = bytes.as_ptr() as usize;
            assert!(start >= base && start - base <= FIXTURE.len() - bytes.len());
        }
    }
}

#[test]
fn all_138_predecessors_preserve_complete_ordinary_records() {
    let all = cases().unwrap();
    let previous = crate::greedy_frame_oracle_fixture::cases().unwrap();
    assert_eq!(previous.len(), 138);
    for old in previous {
        let new = all
            .iter()
            .find(|c| c.depth == 0 && c.observation.name.strip_prefix("d0_") == Some(old.name))
            .unwrap();
        let mut observation = new.observation.clone();
        observation.name = old.name;
        assert_eq!(observation, old);
    }
}

#[test]
fn all_complete_c_frames_decode_with_existing_rust_decoder() {
    let mut frames = 0;
    let mut bytes = 0;
    for case in cases().unwrap() {
        let c = case.observation;
        for frame in core::iter::once(c.frame).chain(c.whole_frame) {
            let decoded = crate::decompress(frame, c.source.len())
                .unwrap_or_else(|e| panic!("{}: {e:?}", c.name));
            assert_eq!(decoded, c.source, "{}", c.name);
            frames += 1;
            bytes += decoded.len();
        }
    }
    assert_eq!((frames, bytes), (549, 41_165_274));
}

#[test]
fn selectors_counts_duplicates_and_truncated_extents_reject() {
    for end in 0..20 {
        assert!(decode(&FIXTURE[..end]).is_err());
    }
    let mut rest = &FIXTURE[16..];
    let length = size(&mut rest).unwrap();
    let record = take(&mut rest, length).unwrap();
    for end in 0..record.len() {
        assert!(LazyFrameCase::read(&record[..end]).is_err());
    }
    for (index, value) in [(0, 3), (0, 1), (1, 1), (2, 1), (3, 1)] {
        let mut bad = record.to_vec();
        bad[index] = value;
        assert!(LazyFrameCase::read(&bad).is_err());
    }
    let mut extra = record.to_vec();
    extra.push(0);
    assert!(LazyFrameCase::read(&extra).is_err());
    for (offset, value) in [(8, 509_u32), (12, 1376), (16, u32::MAX)] {
        let mut bad = FIXTURE[..20].to_vec();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(decode(&bad).is_err());
    }
    let mut duplicate = FIXTURE[..20 + length].to_vec();
    duplicate.extend_from_slice(&FIXTURE[16..20 + length]);
    assert_eq!(decode(&duplicate).unwrap_err(), "duplicate Lazy trajectory");
    assert!(decode(&FIXTURE[..FIXTURE.len() - 1]).is_err());
    let mut extra = FIXTURE.to_vec();
    extra.push(0);
    assert_eq!(
        decode(&extra).unwrap_err(),
        "Lazy trajectory exhaustion/count"
    );
}

#[test]
fn mutated_actual_lazy_trajectories_reject_in_shared_semantic_replay() {
    let all = cases().unwrap();
    for depth in [1, 2] {
        for row in [false, true] {
            let original = &all
                .iter()
                .find(|c| {
                    c.depth == depth
                        && c.observation.row_search == row
                        && c.observation.name.ends_with("_first_uniform")
                })
                .unwrap()
                .observation;
            for field in 0..6 {
                let mut c = original.clone();
                let block = &mut c.blocks[0];
                match field {
                    0 => block.candidate.as_mut().unwrap().sequences[0].distance = 0,
                    1 => block.candidate.as_mut().unwrap().sequences[0].raw_match_base ^= 1,
                    2 => block.wire_after[0] ^= 1,
                    3 => block.prefix_floor = usize::MAX,
                    4 => block.sampling_after.raw_next = 1,
                    _ => block.first_table.hashes[1] ^= 1,
                }
                assert!(c.validate().is_err(), "{} field {field}", c.name);
            }
            let mut source = original.source.to_vec();
            source[0] ^= 1;
            let mut c = original.clone();
            c.source = &source;
            assert!(c.validate().is_err());
        }
    }
}
