//! Checked, test-only access to frozen direct Fast observations.
//!
//! This reader owns no codec state. Source bytes and names borrow the static
//! fixture; vectors contain plain metadata. No C or Python process is needed.

#![forbid(unsafe_code)]

use alloc::{vec, vec::Vec};

const FIXTURE: &[u8] = include_bytes!("../fixtures/reference-fast/cases.bin");
const MAX_SOURCE: usize = 2 << 20;
const MAX_BLOCK: usize = 128 << 10;
type Result<T> = core::result::Result<T, &'static str>;

#[derive(Debug)]
pub(crate) struct OracleSequence {
    pub(crate) literal_length: u32,
    pub(crate) match_length: u32,
    pub(crate) distance: u32,
    pub(crate) offset_value: u32,
}

#[derive(Debug)]
pub(crate) struct OracleCase<'a> {
    pub(crate) name: &'a str,
    pub(crate) window_log: u8,
    pub(crate) hash_log: u8,
    pub(crate) min_match: u8,
    pub(crate) target_length: u32,
    pub(crate) prefix: usize,
    pub(crate) prefix_floor: usize,
    pub(crate) initial_repeats: [u32; 3],
    pub(crate) parser_repeats: [u32; 3],
    pub(crate) wire_repeats: [u32; 3],
    pub(crate) trailing_literals: usize,
    pub(crate) long_length_kind: u8,
    pub(crate) long_length_position: usize,
    pub(crate) source: &'a [u8],
    pub(crate) seeds: Vec<u32>,
    pub(crate) literals: &'a [u8],
    pub(crate) sequences: Vec<OracleSequence>,
    /// Raw C indices: zero is invalid, valid source position is value minus two.
    pub(crate) table_before: Vec<[u32; 2]>,
    /// Sorted bucket, old C index, new C index.
    pub(crate) table_changes: Vec<[u32; 3]>,
    pub(crate) table_hashes: [u64; 2],
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
        let mut reader = Reader { bytes };
        let name_length = usize::from(u16::from_le_bytes(reader.number()?));
        if !(1..=63).contains(&name_length) {
            return Err("case name length");
        }
        let name =
            core::str::from_utf8(reader.take(name_length)?).map_err(|_| "case name UTF-8")?;
        if !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        {
            return Err("case name alphabet");
        }
        let window_log = reader.byte()?;
        let hash_log = reader.byte()?;
        let min_match = reader.byte()?;
        if !(10..=20).contains(&window_log)
            || !(6..=18).contains(&hash_log)
            || !(3..=7).contains(&min_match)
            || reader.byte()? != 0
        {
            return Err("parameter domain");
        }
        let target_length = reader.u32()?;
        let prefix = reader.size()?;
        let prefix_floor = reader.size()?;
        let initial_repeats = reader.repeats()?;
        let parser_repeats = reader.repeats()?;
        let wire_repeats = reader.repeats()?;
        let trailing_literals = reader.size()?;
        let long_length_kind = reader.byte()?;
        let long_length_position = reader.size()?;
        let source = reader.blob(MAX_SOURCE)?;
        let block_size = source.len().checked_sub(prefix).ok_or("prefix extent")?;
        if !(7..=MAX_BLOCK).contains(&block_size)
            || block_size > 1 << window_log
            || target_length > 131_072
            || prefix_floor > prefix
            || trailing_literals > block_size
            || long_length_kind > 2
        {
            return Err("case extent");
        }
        let seeds = reader.records(1024, 4, Reader::u32)?;
        let literals = reader.blob(block_size)?;
        let sequences = reader.records(block_size / 3 + 1, 16, |r| {
            Ok(OracleSequence {
                literal_length: r.u32()?,
                match_length: r.u32()?,
                distance: r.u32()?,
                offset_value: r.u32()?,
            })
        })?;
        let table_before = reader.records(1 << hash_log, 8, |r| Ok([r.u32()?, r.u32()?]))?;
        let table_changes =
            reader.records(1 << hash_log, 12, |r| Ok([r.u32()?, r.u32()?, r.u32()?]))?;
        let table_hashes = [
            u64::from_le_bytes(reader.number()?),
            u64::from_le_bytes(reader.number()?),
        ];
        reader.finish()?;
        Ok(Self {
            name,
            window_log,
            hash_log,
            min_match,
            target_length,
            prefix,
            prefix_floor,
            initial_repeats,
            parser_repeats,
            wire_repeats,
            trailing_literals,
            long_length_kind,
            long_length_position,
            source,
            seeds,
            literals,
            sequences,
            table_before,
            table_changes,
            table_hashes,
        })
    }
}

pub(crate) fn cases() -> Result<Vec<OracleCase<'static>>> {
    decode(FIXTURE)
}

fn decode(bytes: &[u8]) -> Result<Vec<OracleCase<'_>>> {
    if bytes.len() > 16 << 20 {
        return Err("fixture size cap");
    }
    let mut reader = Reader { bytes };
    if reader.take(8)? != b"ZFAST03\0" || reader.size()? != 142 {
        return Err("fixture identity");
    }
    let mut cases = Vec::new();
    cases.try_reserve_exact(142).map_err(|_| "allocation")?;
    for _ in 0..142 {
        let case = OracleCase::read(reader.blob(8 << 20)?)?;
        if cases
            .iter()
            .any(|prior: &OracleCase<'_>| prior.name == case.name)
        {
            return Err("duplicate case");
        }
        cases.push(case);
    }
    reader.finish()?;
    Ok(cases)
}

fn table_hash(table: &[u32]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for value in table {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

#[test]
fn complete_frozen_records_replay_source_and_separate_repeat_state() {
    let cases = cases().unwrap();
    assert_eq!(cases.len(), 142);
    for case in cases {
        let mut decoded = Vec::from(&case.source[..case.prefix]);
        let mut literal_cursor = 0;
        let mut repeats = case.initial_repeats;
        let mut long_length = None;
        for (index, sequence) in case.sequences.iter().enumerate() {
            let ll = sequence.literal_length as usize;
            let ml = sequence.match_length as usize;
            assert!(ml >= 4, "{}", case.name);
            decoded.extend_from_slice(&case.literals[literal_cursor..literal_cursor + ll]);
            literal_cursor += ll;
            let [a, b, c] = repeats;
            let off = sequence.offset_value;
            let distance = match (off, ll == 0) {
                (1, false) => a,
                (1, true) | (2, false) => {
                    repeats = [b, a, c];
                    b
                }
                (2, true) | (3, false) => {
                    repeats = [c, a, b];
                    c
                }
                (3, true) => {
                    let d = a.checked_sub(1).unwrap();
                    repeats = [d, a, b];
                    d
                }
                (value, _) => {
                    let d = value.checked_sub(3).unwrap();
                    repeats = [d, a, b];
                    d
                }
            };
            assert_eq!(distance, sequence.distance, "{}", case.name);
            let distance = distance as usize;
            assert!(distance > 0 && distance <= decoded.len() && distance <= 1 << case.window_log);
            assert!(decoded.len() - distance >= case.prefix_floor);
            assert!(decoded.len() + ml <= case.source.len());
            for _ in 0..ml {
                decoded.push(decoded[decoded.len() - distance]);
            }
            for (kind, length) in [(1, ll), (2, ml - 3)] {
                if length >= 65536 {
                    assert!(long_length.replace((kind, index)).is_none());
                }
            }
        }
        assert_eq!(literal_cursor, case.literals.len());
        assert_eq!(case.trailing_literals, case.source.len() - decoded.len());
        decoded.extend_from_slice(&case.source[decoded.len()..]);
        assert_eq!(decoded, case.source, "{}", case.name);
        assert_eq!(repeats, case.wire_repeats);
        assert_eq!(case.parser_repeats[..2], repeats[..2]);
        assert_eq!(case.parser_repeats[2], case.initial_repeats[2]);
        assert_eq!(
            long_length.unwrap_or((0, 0)),
            (case.long_length_kind, case.long_length_position)
        );
    }
}

#[test]
fn complete_tables_and_seed_buckets_match_canonical_fingerprints() {
    for case in cases().unwrap() {
        let layout =
            crate::match_search::SearchKeyLayout::for_fast(case.hash_log, case.min_match.max(4))
                .unwrap();
        let mut table = vec![0; 1 << case.hash_log];
        for &position in &case.seeds {
            let position = position as usize;
            assert!(position + usize::from(case.min_match.max(4)) <= case.prefix);
            let key = layout
                .read_key(case.source.get(position..).unwrap())
                .unwrap();
            table[layout.bucket(key).unwrap()] = position as u32 + 2;
        }
        let before: Vec<_> = table
            .iter()
            .enumerate()
            .filter(|&(_, &p)| p != 0)
            .map(|(i, &p)| [i as u32, p])
            .collect();
        assert_eq!(before, case.table_before, "{}", case.name);
        assert_eq!(table_hash(&table), case.table_hashes[0]);
        let mut previous = None;
        for &[slot, old, new] in &case.table_changes {
            assert!(previous.is_none_or(|p| slot > p));
            previous = Some(slot);
            assert_eq!(table[slot as usize], old);
            assert_ne!(old, new);
            table[slot as usize] = new;
        }
        for (slot, &value) in table.iter().enumerate() {
            if value == 0 {
                continue;
            }
            let position = value.checked_sub(2).unwrap() as usize;
            let key = layout
                .read_key(case.source.get(position..).unwrap())
                .unwrap();
            assert_eq!(layout.bucket(key).unwrap(), slot, "{}", case.name);
        }
        assert_eq!(table_hash(&table), case.table_hashes[1], "{}", case.name);
        assert!(case.target_length <= 131_072);
    }
}

#[test]
fn truncated_scalar_record_and_trailing_bytes_are_rejected() {
    for length in 0..16 {
        assert!(decode(&FIXTURE[..length]).is_err());
    }
    assert!(decode(&FIXTURE[..FIXTURE.len() - 1]).is_err());
    let mut extra = FIXTURE.to_vec();
    extra.push(0);
    assert_eq!(decode(&extra).unwrap_err(), "trailing fixture bytes");
    let mut count = FIXTURE.to_vec();
    count[8..12].copy_from_slice(&141_u32.to_le_bytes());
    assert_eq!(decode(&count).unwrap_err(), "fixture identity");
    let mut extent = FIXTURE.to_vec();
    extent[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode(&extent).unwrap_err(), "blob cap");
}

#[test]
fn counts_are_checked_before_reservation_and_records_require_exact_ends() {
    let mut reader = Reader { bytes: &[255; 4] };
    assert_eq!(
        reader.records(1024, 4, Reader::u32).unwrap_err(),
        "record count"
    );
    let mut reader = Reader {
        bytes: &[2, 0, 0, 0, 0, 0, 0, 0],
    };
    assert_eq!(
        reader.records(1024, 4, Reader::u32).unwrap_err(),
        "record count"
    );
    let length = u32::from_le_bytes(FIXTURE[12..16].try_into().unwrap()) as usize;
    let mut record = FIXTURE[16..16 + length].to_vec();
    record.push(0);
    assert_eq!(
        OracleCase::read(&record).unwrap_err(),
        "trailing fixture bytes"
    );
}
