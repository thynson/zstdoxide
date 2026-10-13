//! Independent bounded reader for immutable v1.5.7 direct tree observations.
//! This replays record invariants and selected bytes, not tree match selection.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

const PACK: &[u8] = include_bytes!("fixtures/reference-tree/cases.bin");
const SEED: usize = 999_999_999;
type Result<T> = std::result::Result<T, &'static str>;

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        let end = self.position.checked_add(size).ok_or("extent overflow")?;
        let data = self.bytes.get(self.position..end).ok_or("truncated")?;
        self.position = end;
        Ok(data)
    }
    fn number(&mut self) -> Result<usize> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| "integer")?) as usize)
    }
    fn bounded(&mut self, minimum: usize, maximum: usize) -> Result<usize> {
        let value = self.number()?;
        ensure((minimum..=maximum).contains(&value), "scalar domain")?;
        Ok(value)
    }
}
fn ensure(value: bool, reason: &'static str) -> Result<()> {
    if value { Ok(()) } else { Err(reason) }
}
fn cells(bytes: &[u8]) -> impl Iterator<Item = usize> + '_ {
    bytes.as_chunks::<4>().0.iter().map(|b| {
        // The iterator supplies exactly four initialized bytes.
        u32::from_le_bytes(*b) as usize
    })
}

// v1.5.7 ZSTD_hash4/5/6 in lib/common/mem.h and lib/compress/zstd_compress_internal.h.
// Constants are attributed to facebook/zstd; see LICENSE.zstd.
fn bucket(source: &[u8], position: usize, bits: usize, minimum: usize) -> Result<usize> {
    let bytes = source
        .get(position..position.checked_add(8).ok_or("hash overflow")?)
        .ok_or("hash footprint")?;
    let width = minimum.clamp(4, 6);
    let word = bytes[..width]
        .iter()
        .enumerate()
        .fold(0_u64, |n, (i, b)| n | (u64::from(*b) << (8 * i)));
    Ok(if width == 4 {
        (word as u32).wrapping_mul(2_654_435_761) as u64 >> (32 - bits)
    } else {
        let (shift, prime) = if width == 5 {
            (24, 889_523_592_379)
        } else {
            (16, 227_718_039_650_203)
        };
        (word << shift).wrapping_mul(prime) >> (64 - bits)
    } as usize)
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Summary {
    cases: usize,
    operations: usize,
    selected: usize,
    skipped: usize,
    source_bytes: usize,
}

fn read(bytes: &[u8]) -> Result<Summary> {
    ensure(bytes.len() <= 32 << 20, "fixture cap")?;
    let mut cursor = Cursor { bytes, position: 0 };
    ensure(cursor.take(8)? == b"ZTREEFX1", "magic")?;
    let count = cursor.bounded(1, 103)?;
    let mut summary = Summary::default();
    let mut names = BTreeSet::new();
    for _ in 0..count {
        let name_size = cursor.bounded(1, 63)?;
        let source_size = cursor.bounded(8, 131_072)?;
        let operations = cursor.bounded(1, 256)?;
        let window = cursor.bounded(10, 17)?;
        let chain = cursor.bounded(6, 12)?;
        let hash = cursor.bounded(6, 12)?;
        cursor.bounded(1, 8)?;
        let minimum = cursor.bounded(3, 7)?;
        cursor.bounded(0, 0)?;
        cursor.bounded(6, 6)?;
        let name = cursor.take(name_size)?;
        ensure(
            name.iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-'),
            "name alphabet",
        )?;
        ensure(names.insert(name), "duplicate case")?;
        let source = cursor.take(source_size)?;
        let head_size = (1_usize << hash).checked_mul(4).ok_or("heads extent")?;
        let node_size = (1_usize << chain).checked_mul(4).ok_or("nodes extent")?;
        let mut previous_query = None;
        let mut frontier = 0;
        let mut previous_maps = None;
        for _ in 0..operations {
            let query = cursor.bounded(0, source_size - 8)?;
            ensure(
                previous_query.is_none_or(|q| query > q),
                "increasing queries",
            )?;
            previous_query = Some(query);
            let before = cursor.bounded(0, source_size)?;
            let after = cursor.bounded(0, source_size)?;
            ensure(before == frontier, "frontier continuity")?;
            let current = query + 2;
            let floor = 2.max(current.saturating_sub(1 << window));
            ensure(cursor.number()? == floor, "window floor")?;
            ensure(
                cursor.number()? == current.saturating_sub((1 << (chain - 1)) - 1),
                "node floor",
            )?;
            let length = cursor.bounded(0, source_size - query)?;
            let offset = cursor.bounded(4, SEED)?;
            let heads = cursor.take(head_size)?;
            let nodes = cursor.take(node_size)?;
            for (index, value) in cells(heads).enumerate() {
                ensure(value != 1 && value <= current, "head domain")?;
                if value != 0 {
                    ensure(
                        bucket(source, value - 2, hash, minimum)? == index,
                        "head hash",
                    )?;
                }
            }
            for (index, value) in cells(nodes).enumerate() {
                ensure(
                    value <= current && (value != 1 || index % 2 == 1),
                    "edge domain",
                )?;
            }
            if length == 0 {
                ensure(offset == SEED, "zero offset seed")?;
            } else {
                let distance = offset - 3;
                let previous = query.checked_sub(distance).ok_or("match before source")?;
                ensure(previous + 2 > floor, "match outside window")?;
                ensure(
                    source.get(previous..previous + length) == source.get(query..query + length),
                    "selected bytes",
                )?;
                summary.selected += 1;
            }
            if query < before {
                ensure(length == 0 && before == after, "skipped result")?;
                ensure(previous_maps == Some((heads, nodes)), "skipped maps")?;
                summary.skipped += 1;
            } else {
                ensure(after > query, "frontier progress")?;
                let key = bucket(source, query, hash, minimum)?;
                ensure(cells(heads).nth(key) == Some(current), "current hash head")?;
            }
            frontier = after;
            previous_maps = Some((heads, nodes));
            summary.operations += 1;
        }
        summary.cases += 1;
        summary.source_bytes += source_size;
    }
    ensure(cursor.position == bytes.len(), "trailing data")?;
    Ok(summary)
}

fn number_at(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
}
fn replace(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn first_case() -> Vec<u8> {
    let name = number_at(PACK, 12);
    let source = number_at(PACK, 16);
    let operations = number_at(PACK, 20);
    let chain = number_at(PACK, 28);
    let hash = number_at(PACK, 32);
    let end = 52 + name + source + operations * (28 + (4 << hash) + (4 << chain));
    let mut result = PACK[..end].to_vec();
    replace(&mut result, 8, 1);
    result
}

#[test]
fn every_frozen_operation_has_checked_maps_and_selected_bytes() {
    let actual = read(PACK).unwrap();
    assert_eq!(actual.cases, 103);
    assert_eq!(actual.operations, 2334);
    assert_eq!(actual.source_bytes, 270216);
    assert!(actual.selected > 238 && actual.skipped >= 54);
}

#[test]
fn every_truncated_prefix_of_the_smallest_case_is_rejected() {
    let sample = first_case();
    assert!(read(&sample).is_ok());
    for end in 0..sample.len() {
        assert!(read(&sample[..end]).is_err(), "prefix {end}");
    }
}

#[test]
fn malformed_header_dimensions_and_parameters_are_rejected() {
    let sample = first_case();
    for (offset, value) in [
        (8, 0),
        (8, 104),
        (12, 0),
        (12, 64),
        (16, 7),
        (16, 131073),
        (20, 0),
        (20, 257),
        (24, 9),
        (24, 18),
        (28, 5),
        (28, 13),
        (32, 5),
        (32, 13),
        (36, 0),
        (36, 9),
        (40, 2),
        (40, 8),
        (44, 1),
        (48, 5),
    ] {
        let mut bad = sample.clone();
        replace(&mut bad, offset, value);
        assert!(read(&bad).is_err(), "field {offset} value {value}");
    }
    let mut bad = sample.clone();
    bad[0] ^= 1;
    assert!(read(&bad).is_err());
    let mut bad = sample.clone();
    bad[52] = 0xff;
    assert!(read(&bad).is_err());
    let mut bad = sample;
    bad.push(0);
    assert!(read(&bad).is_err());
}

#[test]
fn malformed_frontiers_floors_and_offsets_are_rejected() {
    let sample = first_case();
    let op = 52 + number_at(&sample, 12) + number_at(&sample, 16);
    for (field, value) in [
        (0, 1),
        (1, 1),
        (2, 0),
        (2, 9),
        (3, 0),
        (4, 1),
        (5, 9),
        (6, 3),
        (6, 4),
        (6, 1_000_000_000),
    ] {
        let mut bad = sample.clone();
        replace(&mut bad, op + field * 4, value);
        assert!(read(&bad).is_err(), "operation field {field} value {value}");
    }
}

#[test]
fn malformed_hash_heads_and_edge_markers_are_rejected() {
    let sample = first_case();
    let heads = 52 + number_at(&sample, 12) + number_at(&sample, 16) + 28;
    let nodes = heads + (4 << number_at(&sample, 32));
    for (offset, value) in [
        (heads, 1),
        (heads, u32::MAX),
        (nodes, 1),
        (nodes, 3),
        (nodes + 4, 3),
    ] {
        let mut bad = sample.clone();
        replace(&mut bad, offset, value);
        assert!(read(&bad).is_err());
    }
    let mut bad = sample;
    bad[heads..nodes].fill(0);
    assert!(read(&bad).is_err());
}

#[test]
fn duplicate_case_names_and_hash_footprints_are_rejected() {
    let sample = first_case();
    let mut duplicate = sample.clone();
    duplicate.extend_from_slice(&sample[12..]);
    replace(&mut duplicate, 8, 2);
    assert!(read(&duplicate).is_err());
    assert!(bucket(&[0; 8], 1, 6, 4).is_err());
    assert!(bucket(&[0; 8], usize::MAX, 6, 4).is_err());
}

fn operation_locations() -> Vec<(usize, usize, usize)> {
    let mut cursor = Cursor {
        bytes: PACK,
        position: 12,
    };
    let mut result = Vec::new();
    for _ in 0..number_at(PACK, 8) {
        let name = cursor.number().unwrap();
        let source = cursor.number().unwrap();
        let operations = cursor.number().unwrap();
        cursor.number().unwrap();
        let chain = cursor.number().unwrap();
        let hash = cursor.number().unwrap();
        cursor.take(16).unwrap();
        cursor.take(name + source).unwrap();
        for _ in 0..operations {
            let offset = cursor.position;
            cursor.take(28 + (4 << hash) + (4 << chain)).unwrap();
            result.push((offset, hash, chain));
        }
    }
    assert_eq!(cursor.position, PACK.len());
    result
}

#[test]
fn selected_match_cannot_point_before_source_or_exceed_its_extent() {
    let (offset, _, _) = operation_locations()
        .into_iter()
        .find(|(o, _, _)| number_at(PACK, o + 20) != 0)
        .unwrap();
    for (field, value) in [(20, u32::MAX), (24, SEED as u32)] {
        let mut bad = PACK.to_vec();
        replace(&mut bad, offset + field, value);
        assert!(read(&bad).is_err());
    }
}

#[test]
fn skipped_operation_cannot_change_an_otherwise_valid_edge() {
    let (offset, hash, _) = operation_locations()
        .into_iter()
        .find(|(o, _, _)| number_at(PACK, *o) < number_at(PACK, o + 4))
        .unwrap();
    let odd_edge = offset + 28 + (4 << hash) + 4;
    let value = if number_at(PACK, odd_edge) == 0 { 1 } else { 0 };
    let mut bad = PACK.to_vec();
    replace(&mut bad, odd_edge, value);
    assert_eq!(read(&bad), Err("skipped maps"));
}
