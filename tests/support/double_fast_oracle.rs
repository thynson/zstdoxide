//! Checked test-only projection of frozen direct DoubleFast observations.
//!
//! Source/name/literal slices borrow static fixture bytes. Metadata owns bounded
//! vectors. No JSON dependency, compiler, subprocess or codec wrapper is used.
#![forbid(unsafe_code)]

use crate::match_search::SearchKeyLayout;
use alloc::{vec, vec::Vec};

const FIXTURE: &[u8] = include_bytes!("../fixtures/reference-double-fast/cases.bin");
const CASES: usize = 184;
const MAX_PREFIX: usize = 2 << 20;
const MAX_BLOCK: usize = 128 << 10;
type Result<T> = core::result::Result<T, &'static str>;

#[derive(Clone, Debug)]
pub(crate) struct OracleSequence {
    pub(crate) raw_literal_length: u32,
    pub(crate) raw_match_base: u32,
    pub(crate) literal_length: u32,
    pub(crate) match_length: u32,
    pub(crate) distance: u32,
    pub(crate) offset_value: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct OracleTable {
    pub(crate) seeds: Vec<u32>,
    /// Nonzero bucket/raw-index pairs, with valid source indices biased by two.
    pub(crate) before: Vec<[u32; 2]>,
    /// Sorted bucket/old/new triples. No intermediate store chronology is implied.
    pub(crate) changes: Vec<[u32; 3]>,
    pub(crate) hashes: [u64; 2],
}

#[derive(Clone, Debug)]
pub(crate) struct OracleCase<'a> {
    pub(crate) name: &'a str,
    pub(crate) window_log: u8,
    pub(crate) long_log: u8,
    pub(crate) short_log: u8,
    pub(crate) search_log: u8,
    pub(crate) min_match: u8,
    pub(crate) target_length: u32,
    pub(crate) prefix: usize,
    pub(crate) prefix_floor: usize,
    pub(crate) initial_repeats: [u32; 3],
    pub(crate) parser_repeats: [u32; 3],
    pub(crate) wire_repeats: [u32; 3],
    pub(crate) trailing_literals: usize,
    pub(crate) initial_current: usize,
    pub(crate) ordinary_floor: usize,
    pub(crate) repeat_floor: usize,
    pub(crate) long_length_kind: u8,
    pub(crate) long_length_position: usize,
    pub(crate) source: &'a [u8],
    source_hash: u64,
    pub(crate) literals: &'a [u8],
    pub(crate) sequences: Vec<OracleSequence>,
    pub(crate) long_table: OracleTable,
    pub(crate) short_table: OracleTable,
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        let value = self.bytes.get(..size).ok_or("truncated fixture")?;
        self.bytes = &self.bytes[size..];
        Ok(value)
    }

    fn number<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| "scalar width")
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.number::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.number()?))
    }

    fn size(&mut self) -> Result<usize> {
        usize::try_from(self.u32()?).map_err(|_| "target size")
    }

    fn repeats(&mut self) -> Result<[u32; 3]> {
        let values = [self.u32()?, self.u32()?, self.u32()?];
        if values.contains(&0) {
            return Err("zero repeat");
        }
        Ok(values)
    }

    fn blob(&mut self, cap: usize) -> Result<&'a [u8]> {
        let size = self.size()?;
        if size > cap {
            return Err("blob cap");
        }
        self.take(size)
    }

    fn records<T>(
        &mut self,
        cap: usize,
        stride: usize,
        mut read: impl FnMut(&mut Self) -> Result<T>,
    ) -> Result<Vec<T>> {
        let count = self.size()?;
        if count > cap
            || count
                .checked_mul(stride)
                .is_none_or(|n| n > self.bytes.len())
        {
            return Err("record count");
        }
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| "allocation")?;
        for _ in 0..count {
            values.push(read(self)?);
        }
        Ok(values)
    }

    fn finish(self) -> Result<()> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err("trailing fixture bytes")
        }
    }
}

impl<'a> OracleCase<'a> {
    fn read(bytes: &'a [u8]) -> Result<Self> {
        let mut r = Reader { bytes };
        let name_len = usize::from(u16::from_le_bytes(r.number()?));
        if !(1..=63).contains(&name_len) {
            return Err("case name length");
        }
        let name = core::str::from_utf8(r.take(name_len)?).map_err(|_| "case name UTF-8")?;
        if !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            return Err("case name alphabet");
        }
        let window_log = r.byte()?;
        let long_log = r.byte()?;
        let short_log = r.byte()?;
        let search_log = r.byte()?;
        let min_match = r.byte()?;
        if !(10..=21).contains(&window_log)
            || !(6..=18).contains(&long_log)
            || !(6..=18).contains(&short_log)
            || !(1..=29).contains(&search_log)
            || !(3..=7).contains(&min_match)
            || r.take(3)? != [0; 3]
        {
            return Err("parameter domain");
        }
        let target_length = r.u32()?;
        let prefix = r.size()?;
        let prefix_floor = r.size()?;
        let initial_repeats = r.repeats()?;
        let parser_repeats = r.repeats()?;
        let wire_repeats = r.repeats()?;
        let trailing_literals = r.size()?;
        let initial_current = r.size()?;
        let ordinary_floor = r.size()?;
        let repeat_floor = r.size()?;
        let long_length_kind = r.byte()?;
        let long_length_position = r.size()?;
        let source = r.blob(MAX_PREFIX + MAX_BLOCK)?;
        let source_hash = u64::from_le_bytes(r.number()?);
        let block_size = source.len().checked_sub(prefix).ok_or("prefix extent")?;
        if !(7..=MAX_BLOCK).contains(&block_size)
            || block_size > 1 << window_log
            || prefix > MAX_PREFIX
            || prefix_floor > prefix
            || target_length > 131_072
            || trailing_literals > block_size
            || long_length_kind > 2
        {
            return Err("case extent");
        }
        let long_seeds = r.records(1024, 4, Reader::u32)?;
        let short_seeds = r.records(1024, 4, Reader::u32)?;
        let literals = r.blob(block_size)?;
        let sequences = r.records(block_size / 3 + 1, 24, |r| {
            Ok(OracleSequence {
                raw_literal_length: r.u32()?,
                raw_match_base: r.u32()?,
                literal_length: r.u32()?,
                match_length: r.u32()?,
                distance: r.u32()?,
                offset_value: r.u32()?,
            })
        })?;
        let mut table = |log, seeds| -> Result<OracleTable> {
            Ok(OracleTable {
                seeds,
                before: r.records(1usize << log, 8, |r| Ok([r.u32()?, r.u32()?]))?,
                changes: r.records(1usize << log, 12, |r| Ok([r.u32()?, r.u32()?, r.u32()?]))?,
                hashes: [
                    u64::from_le_bytes(r.number()?),
                    u64::from_le_bytes(r.number()?),
                ],
            })
        };
        let long_table = table(long_log, long_seeds)?;
        let short_table = table(short_log, short_seeds)?;
        r.finish()?;
        Ok(Self {
            name,
            window_log,
            long_log,
            short_log,
            search_log,
            min_match,
            target_length,
            prefix,
            prefix_floor,
            initial_repeats,
            parser_repeats,
            wire_repeats,
            trailing_literals,
            initial_current,
            ordinary_floor,
            repeat_floor,
            long_length_kind,
            long_length_position,
            source,
            source_hash,
            literals,
            sequences,
            long_table,
            short_table,
        })
    }

    /// Replay every logical byte and the full wire repeat triple independently
    /// of the production parser and entropy emitter. No C raw-field truncation
    /// is used to infer expanded lengths: both representations must agree.
    fn verify_sequences(&self) -> Result<()> {
        if fnv(self.source.iter().copied()) != self.source_hash {
            return Err("source hash");
        }
        let window = 1 << self.window_log;
        let floor = self
            .prefix_floor
            .max(self.source.len().saturating_sub(window));
        let current = self.prefix + usize::from(self.prefix == floor);
        let repeat_floor = self.prefix_floor.max(current.saturating_sub(window));
        if (self.initial_current, self.ordinary_floor, self.repeat_floor)
            != (current, floor, repeat_floor)
        {
            return Err("entry bounds");
        }
        let mut decoded = Vec::new();
        decoded
            .try_reserve_exact(self.source.len())
            .map_err(|_| "allocation")?;
        decoded.extend_from_slice(&self.source[..self.prefix]);
        let mut literal_cursor = 0usize;
        let mut repeats = self.initial_repeats;
        let mut marker = None;
        for (index, seq) in self.sequences.iter().enumerate() {
            let ll = usize::try_from(seq.literal_length).map_err(|_| "literal length")?;
            let ml = usize::try_from(seq.match_length).map_err(|_| "match length")?;
            if ml < 4
                || ll > MAX_BLOCK
                || ml > MAX_BLOCK
                || seq.raw_literal_length > 65535
                || seq.raw_match_base > 65535
                || seq.raw_literal_length != seq.literal_length & 65535
                || seq.raw_match_base != (seq.match_length - 3) & 65535
            {
                return Err("sequence lengths");
            }
            for (kind, length) in [(1, ll), (2, ml - 3)] {
                if length >= 65536 && marker.replace((kind, index)).is_some() {
                    return Err("multiple long lengths");
                }
            }
            let literal_end = literal_cursor.checked_add(ll).ok_or("literal extent")?;
            let literals = self
                .literals
                .get(literal_cursor..literal_end)
                .ok_or("literal extent")?;
            if decoded
                .len()
                .checked_add(ll)
                .and_then(|n| n.checked_add(ml))
                .is_none_or(|n| n > self.source.len())
            {
                return Err("sequence extent");
            }
            decoded.extend_from_slice(literals);
            literal_cursor = literal_end;
            let [a, b, c] = repeats;
            let distance = match seq.offset_value {
                1 if ll != 0 => a,
                1 => {
                    repeats = [b, a, c];
                    b
                }
                value if value >= 4 => {
                    let d = value - 3;
                    repeats = [d, a, b];
                    d
                }
                _ => return Err("DoubleFast offset intent"),
            };
            if distance != seq.distance || distance == 0 {
                return Err("resolved distance");
            }
            let distance = usize::try_from(distance).map_err(|_| "distance width")?;
            if distance > window
                || decoded
                    .len()
                    .checked_sub(distance)
                    .is_none_or(|p| p < self.prefix_floor)
            {
                return Err("distance extent");
            }
            for _ in 0..ml {
                decoded.push(decoded[decoded.len() - distance]);
            }
        }
        if literal_cursor != self.literals.len()
            || self.source.len() - decoded.len() != self.trailing_literals
        {
            return Err("literal exhaustion");
        }
        decoded.extend_from_slice(&self.source[decoded.len()..]);
        if decoded != self.source {
            return Err("source replay");
        }
        if repeats != self.wire_repeats
            || self.parser_repeats[..2] != repeats[..2]
            || self.parser_repeats[2] != self.initial_repeats[2]
        {
            return Err("repeat state");
        }
        if marker.unwrap_or((0, 0)) != (self.long_length_kind, self.long_length_position) {
            return Err("long marker");
        }
        Ok(())
    }
}

fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

impl OracleTable {
    /// Expand and check every initialized cell, preserving ordered seed writes
    /// and validating final fingerprints independently of sparse-delta ordering.
    pub(crate) fn after(&self, case: &OracleCase<'_>, layout: SearchKeyLayout) -> Result<Vec<u32>> {
        let mut values = vec![0u32; layout.bucket_count()];
        let bucket = |position: usize| -> Result<usize> {
            let key = case
                .source
                .get(position..)
                .and_then(|s| layout.read_key(s))
                .ok_or("table key extent")?;
            layout.bucket(key).map_err(|_| "table key")
        };
        for &position in &self.seeds {
            let position = usize::try_from(position).map_err(|_| "seed width")?;
            if position
                .checked_add(usize::from(layout.key_bytes()))
                .is_none_or(|p| p > case.prefix)
            {
                return Err("seed extent");
            }
            values[bucket(position)?] = u32::try_from(position).map_err(|_| "seed width")? + 2;
        }
        let before: Vec<_> = values
            .iter()
            .enumerate()
            .filter(|&(_, &v)| v != 0)
            .map(|(b, &v)| [b as u32, v])
            .collect();
        if before != self.before
            || fnv(values.iter().flat_map(|v| v.to_le_bytes())) != self.hashes[0]
        {
            return Err("seed table");
        }
        let mut previous = None;
        for &[slot, old, new] in &self.changes {
            if previous.is_some_and(|p| p >= slot) || old == new || new < 2 {
                return Err("table change order/value");
            }
            let slot_usize = usize::try_from(slot).map_err(|_| "bucket width")?;
            let cell = values.get_mut(slot_usize).ok_or("table bucket")?;
            if *cell != old {
                return Err("table old value");
            }
            *cell = new;
            previous = Some(slot);
        }
        for (slot, &value) in values.iter().enumerate() {
            if value == 0 {
                continue;
            }
            let position = usize::try_from(value.checked_sub(2).ok_or("table raw index")?)
                .map_err(|_| "table index width")?;
            if bucket(position)? != slot {
                return Err("table final key");
            }
        }
        if fnv(values.iter().flat_map(|v| v.to_le_bytes())) != self.hashes[1] {
            return Err("table final hash");
        }
        Ok(values)
    }
}

pub(crate) fn cases() -> Result<Vec<OracleCase<'static>>> {
    decode(FIXTURE)
}

fn decode(bytes: &[u8]) -> Result<Vec<OracleCase<'_>>> {
    if bytes.len() > 16 << 20 {
        return Err("fixture size cap");
    }
    let mut r = Reader { bytes };
    if r.take(8)? != b"ZDFST02\0" || r.size()? != CASES {
        return Err("fixture identity");
    }
    let mut cases = Vec::new();
    cases.try_reserve_exact(CASES).map_err(|_| "allocation")?;
    for _ in 0..CASES {
        let case = OracleCase::read(r.blob(8 << 20)?)?;
        if cases.iter().any(|c: &OracleCase<'_>| c.name == case.name) {
            return Err("duplicate case");
        }
        cases.push(case);
    }
    r.finish()?;
    Ok(cases)
}

#[test]
fn all_frozen_sequences_replay_and_entry_bounds_match_checked_blocks() {
    use crate::double_fast_block::DoubleFastBlock;
    let cases = cases().unwrap();
    assert_eq!(cases.len(), CASES);
    for case in cases {
        case.verify_sequences()
            .unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let block = DoubleFastBlock::new(
            case.source,
            case.prefix..case.source.len(),
            case.prefix_floor,
            SearchKeyLayout::for_double_fast_long(case.long_log).unwrap(),
            SearchKeyLayout::for_fast(case.short_log, case.min_match.max(4)).unwrap(),
            1 << case.window_log,
        )
        .unwrap();
        assert_eq!(
            (
                block.initial_position(),
                block.ordinary_floor(),
                block.repeat_floor()
            ),
            (case.initial_current, case.ordinary_floor, case.repeat_floor),
            "{}",
            case.name
        );
        assert!((1..=29).contains(&case.search_log));
        assert!(case.target_length <= 131_072);
    }
}

#[test]
fn both_complete_tables_preserve_seed_order_and_canonical_fingerprints() {
    for case in cases().unwrap() {
        for (table, layout) in [
            (
                &case.long_table,
                SearchKeyLayout::for_double_fast_long(case.long_log).unwrap(),
            ),
            (
                &case.short_table,
                SearchKeyLayout::for_fast(case.short_log, case.min_match.max(4)).unwrap(),
            ),
        ] {
            let values = table
                .after(&case, layout)
                .unwrap_or_else(|e| panic!("{}: {e}", case.name));
            assert_eq!(values.len(), layout.bucket_count());
        }
    }
}

#[test]
fn truncated_scalars_counts_extents_and_nonexhausted_records_are_rejected() {
    for length in 0..16 {
        assert!(decode(&FIXTURE[..length]).is_err());
    }
    assert!(decode(&FIXTURE[..FIXTURE.len() - 1]).is_err());
    let mut extra = FIXTURE.to_vec();
    extra.push(0);
    assert_eq!(decode(&extra).unwrap_err(), "trailing fixture bytes");
    let mut count = FIXTURE.to_vec();
    count[8..12].copy_from_slice(&183u32.to_le_bytes());
    assert_eq!(decode(&count).unwrap_err(), "fixture identity");
    let mut extent = FIXTURE.to_vec();
    extent[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode(&extent).unwrap_err(), "blob cap");
    for bytes in [&[255u8; 4][..], &[2, 0, 0, 0, 0, 0, 0, 0]] {
        assert_eq!(
            Reader { bytes }.records(1024, 4, Reader::u32).unwrap_err(),
            "record count"
        );
    }
    let length = u32::from_le_bytes(FIXTURE[12..16].try_into().unwrap()) as usize;
    let record = &FIXTURE[16..16 + length];
    for length in 0..record.len() {
        assert!(OracleCase::read(&record[..length]).is_err());
    }
    let mut extra = record.to_vec();
    extra.push(0);
    assert_eq!(
        OracleCase::read(&extra).unwrap_err(),
        "trailing fixture bytes"
    );
    let mut invalid = record.to_vec();
    let name_len = u16::from_le_bytes(invalid[..2].try_into().unwrap()) as usize;
    invalid[2 + name_len + 2] = 19;
    assert_eq!(OracleCase::read(&invalid).unwrap_err(), "parameter domain");
}

#[test]
fn semantic_forgery_controls_reject_lengths_repeats_intents_and_both_tables() {
    let all = cases().unwrap();
    let original = all
        .iter()
        .find(|c| c.name == "long-literal-extended-tail")
        .unwrap();
    for field in 0..8 {
        let mut case = original.clone();
        match field {
            0 => case.sequences[0].raw_literal_length ^= 1,
            1 => case.sequences[0].raw_match_base ^= 1,
            2 => case.sequences[0].offset_value = 2,
            3 => case.sequences[0].distance = 0,
            4 => case.parser_repeats[2] ^= 1,
            5 => case.wire_repeats[2] ^= 1,
            6 => case.long_length_kind = 0,
            _ => case.ordinary_floor += 1,
        }
        assert!(case.verify_sequences().is_err(), "forgery {field}");
    }
    for (table, layout) in [
        (
            &original.long_table,
            SearchKeyLayout::for_double_fast_long(original.long_log).unwrap(),
        ),
        (
            &original.short_table,
            SearchKeyLayout::for_fast(original.short_log, original.min_match.max(4)).unwrap(),
        ),
    ] {
        for field in 0..6 {
            let mut table = table.clone();
            match field {
                0 => table.hashes[0] ^= 1,
                1 => table.hashes[1] ^= 1,
                2 => table.changes[0][0] = u32::MAX,
                3 => table.changes[0][1] ^= 1,
                4 => table.changes[0][2] = 1,
                _ => table.seeds[0] = original.prefix as u32,
            }
            assert!(
                table.after(original, layout).is_err(),
                "table forgery {field}"
            );
        }
    }
}
