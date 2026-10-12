//! Checked, dependency-free projection of frozen direct Greedy observations.
//! Source/name/literal bytes borrow the immutable pack; bounded metadata vectors
//! own initialized values. No codec wrapper, JSON dependency or process is used.
#![forbid(unsafe_code)]
#![allow(dead_code, reason = "Greedy parser observation integration follows")]

use crate::greedy_key::{GreedyKeyLayout, RowLayout};
use alloc::{vec, vec::Vec};

const FIXTURE: &[u8] = include_bytes!("../fixtures/reference-greedy/cases.bin");
const CASES: usize = 173;
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
    pub(crate) element_bytes: u8,
    pub(crate) log: u8,
    pub(crate) before: Vec<[u32; 2]>,
    pub(crate) changes: Vec<[u32; 3]>,
    pub(crate) hashes: [u64; 2],
}

#[derive(Clone, Debug)]
pub(crate) struct OracleCase<'a> {
    pub(crate) name: &'a str,
    pub(crate) row_search: bool,
    pub(crate) window_log: u8,
    pub(crate) hash_log: u8,
    pub(crate) chain_log: u8,
    pub(crate) search_log: u8,
    pub(crate) min_match: u8,
    pub(crate) key_width: u8,
    pub(crate) row_log: u8,
    pub(crate) target_length: u32,
    pub(crate) prefix: usize,
    pub(crate) prefix_floor: usize,
    pub(crate) initial_next: usize,
    pub(crate) initial_repeats: [u32; 3],
    pub(crate) parser_repeats: [u32; 3],
    pub(crate) wire_repeats: [u32; 3],
    pub(crate) trailing_literals: usize,
    pub(crate) initial_current: usize,
    pub(crate) repeat_floor: usize,
    /// Logical Rust index; the packed raw C value is checked before subtracting two.
    pub(crate) final_next: usize,
    pub(crate) final_skipping: bool,
    pub(crate) initial_entropy: u32,
    pub(crate) final_entropy: u32,
    pub(crate) initial_salt: u64,
    pub(crate) final_salt: u64,
    pub(crate) hash_cache: [u32; 8],
    pub(crate) long_length_kind: u8,
    pub(crate) long_length_position: usize,
    pub(crate) source: &'a [u8],
    source_hash: u64,
    pub(crate) seeds: Vec<u32>,
    pub(crate) literals: &'a [u8],
    pub(crate) sequences: Vec<OracleSequence>,
    pub(crate) first_table: OracleTable,
    pub(crate) second_table: OracleTable,
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
        let branch = r.byte()?;
        let window_log = r.byte()?;
        let hash_log = r.byte()?;
        let chain_log = r.byte()?;
        let search_log = r.byte()?;
        let min_match = r.byte()?;
        let key_width = r.byte()?;
        let row_log = r.byte()?;
        if branch > 1
            || !(10..=21).contains(&window_log)
            || !(6..=19).contains(&hash_log)
            || !(6..=18).contains(&chain_log)
            || !(1..=if branch == 0 { 8 } else { 29 }).contains(&search_log)
            || !(3..=7).contains(&min_match)
            || key_width != min_match.clamp(4, 6)
            || row_log
                != if branch == 0 {
                    0
                } else {
                    search_log.clamp(4, 6)
                }
        {
            return Err("parameter domain");
        }
        let row_search = branch == 1;
        let target_length = r.u32()?;
        let prefix = r.size()?;
        let prefix_floor = r.size()?;
        let initial_next = r.size()?;
        let initial_repeats = r.repeats()?;
        let parser_repeats = r.repeats()?;
        let wire_repeats = r.repeats()?;
        let trailing_literals = r.size()?;
        let initial_current = r.size()?;
        let repeat_floor = r.size()?;
        let final_next = r.size()?.checked_sub(2).ok_or("raw frontier bias")?;
        let skipping = r.u32()?;
        if skipping > 1 {
            return Err("skipping domain");
        }
        let final_skipping = skipping == 1;
        let initial_entropy = r.u32()?;
        let final_entropy = r.u32()?;
        let initial_salt = u64::from_le_bytes(r.number()?);
        let final_salt = u64::from_le_bytes(r.number()?);
        let mut hash_cache = [0; 8];
        for value in &mut hash_cache {
            *value = r.u32()?;
        }
        let long_length_kind = r.byte()?;
        let long_length_position = r.size()?;
        let source = r.blob(MAX_PREFIX + MAX_BLOCK)?;
        let source_hash = u64::from_le_bytes(r.number()?);
        let block = source.len().checked_sub(prefix).ok_or("prefix extent")?;
        if prefix > MAX_PREFIX
            || !(7..=MAX_BLOCK.min(1 << window_log)).contains(&block)
            || prefix_floor > initial_next
            || initial_next > prefix
            || final_next < initial_next
            || final_next > source.len()
            || target_length > 131_072
            || trailing_literals > block
            || long_length_kind > 2
        {
            return Err("case extent");
        }
        let seeds = r.records(524_288, 4, Reader::u32)?;
        let mut previous = None;
        for &position in &seeds {
            let p = usize::try_from(position).map_err(|_| "seed width")?;
            if previous.is_some_and(|old| old >= p)
                || p >= initial_next
                || p.checked_add(usize::from(key_width))
                    .is_none_or(|end| end > prefix)
            {
                return Err("seed extent/order");
            }
            previous = Some(p);
        }
        let literals = r.blob(block)?;
        let sequences = r.records(block / 3 + 1, 24, |r| {
            Ok(OracleSequence {
                raw_literal_length: r.u32()?,
                raw_match_base: r.u32()?,
                literal_length: r.u32()?,
                match_length: r.u32()?,
                distance: r.u32()?,
                offset_value: r.u32()?,
            })
        })?;
        let mut table = |log, width| -> Result<OracleTable> {
            if r.byte()? != width || r.byte()? != log || r.take(2)? != [0, 0] {
                return Err("owner geometry");
            }
            Ok(OracleTable {
                element_bytes: width,
                log,
                before: r.records(1usize << log, 8, |r| Ok([r.u32()?, r.u32()?]))?,
                changes: r.records(1usize << log, 12, |r| Ok([r.u32()?, r.u32()?, r.u32()?]))?,
                hashes: [
                    u64::from_le_bytes(r.number()?),
                    u64::from_le_bytes(r.number()?),
                ],
            })
        };
        let first_table = table(hash_log, 4)?;
        let second_table = table(
            if row_search { hash_log } else { chain_log },
            if row_search { 1 } else { 4 },
        )?;
        r.finish()?;
        Ok(Self {
            name,
            row_search,
            window_log,
            hash_log,
            chain_log,
            search_log,
            min_match,
            key_width,
            row_log,
            target_length,
            prefix,
            prefix_floor,
            initial_next,
            initial_repeats,
            parser_repeats,
            wire_repeats,
            trailing_literals,
            initial_current,
            repeat_floor,
            final_next,
            final_skipping,
            initial_entropy,
            final_entropy,
            initial_salt,
            final_salt,
            hash_cache,
            long_length_kind,
            long_length_position,
            source,
            source_hash,
            seeds,
            literals,
            sequences,
            first_table,
            second_table,
        })
    }

    pub(crate) fn key_layout(&self) -> Result<GreedyKeyLayout> {
        GreedyKeyLayout::new(self.hash_log, self.min_match).map_err(|_| "key layout")
    }
    pub(crate) fn row_layout(&self) -> Result<RowLayout> {
        if !self.row_search {
            return Err("not a row case");
        }
        RowLayout::new(self.key_layout()?, self.search_log).map_err(|_| "row layout")
    }

    /// Reconstruct source bytes and wire repeats without calling a production
    /// parser or entropy emitter. Raw/expanded lengths must agree independently.
    fn verify_sequences(&self) -> Result<()> {
        if fnv(self.source.iter().copied()) != self.source_hash {
            return Err("source hash");
        }
        let window = 1usize << self.window_log;
        let current = self.prefix + usize::from(self.prefix == self.prefix_floor);
        if self.initial_current != current
            || self.repeat_floor != self.prefix_floor.max(current.saturating_sub(window))
        {
            return Err("entry bounds");
        }
        if self.initial_salt != self.final_salt {
            return Err("changed direct salt");
        }
        if !self.row_search
            && (self.initial_salt != 0
                || self.initial_entropy != 0
                || self.final_entropy != 0
                || self.hash_cache != [0; 8])
        {
            return Err("inactive row state");
        }
        if self.row_search
            && self
                .hash_cache
                .iter()
                .any(|&h| h >= 1 << (self.hash_log - self.row_log + 8))
        {
            return Err("cache hash width");
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
            let ll = usize::try_from(seq.literal_length).map_err(|_| "literal width")?;
            let ml = usize::try_from(seq.match_length).map_err(|_| "match width")?;
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
            for (kind, len) in [(1, ll), (2, ml - 3)] {
                if len >= 65536 && marker.replace((kind, index)).is_some() {
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
                _ => return Err("Greedy offset intent"),
            };
            if distance == 0 || distance != seq.distance {
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

    /// Verify complete sparse owners, source extents and hash/tag membership.
    /// This checks frozen observations, not an expected-output generator.
    fn verify_tables(&self) -> Result<()> {
        if (self.first_table.log, self.first_table.element_bytes) != (self.hash_log, 4)
            || (self.second_table.log, self.second_table.element_bytes)
                != (
                    if self.row_search {
                        self.hash_log
                    } else {
                        self.chain_log
                    },
                    if self.row_search { 1 } else { 4 },
                )
        {
            return Err("owner role geometry");
        }
        let key_layout = self.key_layout()?;
        for after in [false, true] {
            let first = self.first_table.values(after)?;
            let second = self.second_table.values(after)?;
            let frontier = if after {
                self.final_next
            } else {
                self.initial_next
            };
            for values in if self.row_search {
                [&first[..], &[][..]]
            } else {
                [&first[..], &second[..]]
            } {
                for &raw in values {
                    if raw == 0 {
                        continue;
                    }
                    let pos = usize::try_from(raw.checked_sub(2).ok_or("raw position")?)
                        .map_err(|_| "position width")?;
                    if pos >= frontier
                        || pos
                            .checked_add(usize::from(self.key_width))
                            .is_none_or(|end| end > self.source.len())
                    {
                        return Err("owner position extent");
                    }
                }
            }
            if self.row_search {
                let row = self.row_layout()?;
                let width = row.row_len();
                for (base, positions) in first.chunks_exact(width).enumerate() {
                    let base = base * width;
                    let head = usize::try_from(second[base]).map_err(|_| "head width")?;
                    if positions[0] != 0
                        || head >= width
                        || (head == 0) != (positions.iter().all(|&p| p == 0))
                        || (head != 0 && positions[head] == 0)
                    {
                        return Err("reserved head");
                    }
                    for slot in 1..width {
                        let raw = positions[slot];
                        let tag = second[base + slot];
                        if raw == 0 {
                            if tag != 0 {
                                return Err("empty row tag");
                            }
                            continue;
                        }
                        let position = (raw - 2) as usize;
                        let key = key_layout
                            .read_key(&self.source[position..])
                            .ok_or("row key extent")?;
                        let key = row.key(key, self.initial_salt).map_err(|_| "row hash")?;
                        if key.row_start() != base || u32::from(key.tag()) != tag {
                            return Err("row membership");
                        }
                    }
                }
            } else {
                for (bucket, &raw) in first.iter().enumerate() {
                    if raw == 0 {
                        continue;
                    }
                    let key = key_layout
                        .read_key(&self.source[(raw - 2) as usize..])
                        .ok_or("chain key extent")?;
                    if key_layout.bucket(key).map_err(|_| "chain hash")? != bucket {
                        return Err("chain membership");
                    }
                }
            }
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
    pub(crate) fn values(&self, after: bool) -> Result<Vec<u32>> {
        if !(6..=19).contains(&self.log) || ![1, 4].contains(&self.element_bytes) {
            return Err("owner geometry");
        }
        let count = 1usize << self.log;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| "allocation")?;
        values.resize(count, 0);
        let valid = |v: u32| self.element_bytes == 4 || v <= 255;
        let hash = |values: &[u32]| {
            fnv(values.iter().flat_map(|v| {
                v.to_le_bytes()
                    .into_iter()
                    .take(usize::from(self.element_bytes))
            }))
        };
        let mut previous = None;
        for &[slot, value] in &self.before {
            if previous.is_some_and(|p| p >= slot) || value == 0 || !valid(value) {
                return Err("initial map order/value");
            }
            *values
                .get_mut(usize::try_from(slot).map_err(|_| "slot width")?)
                .ok_or("owner slot")? = value;
            previous = Some(slot);
        }
        if hash(&values) != self.hashes[0] {
            return Err("initial owner fingerprint");
        }
        if !after {
            return Ok(values);
        }
        previous = None;
        for &[slot, old, new] in &self.changes {
            if previous.is_some_and(|p| p >= slot) || old == new || !valid(old) || !valid(new) {
                return Err("delta order/value");
            }
            let cell = values
                .get_mut(usize::try_from(slot).map_err(|_| "slot width")?)
                .ok_or("owner slot")?;
            if *cell != old {
                return Err("delta old value");
            }
            *cell = new;
            previous = Some(slot);
        }
        if hash(&values) != self.hashes[1] {
            return Err("final owner fingerprint");
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
    if r.take(8)? != b"ZGRST01\0" || r.size()? != CASES {
        return Err("fixture identity");
    }
    let mut cases = Vec::new();
    cases.try_reserve_exact(CASES).map_err(|_| "allocation")?;
    for _ in 0..CASES {
        let case = OracleCase::read(r.blob(32 << 20)?)?;
        if cases.iter().any(|c: &OracleCase<'_>| c.name == case.name) {
            return Err("duplicate case");
        }
        cases.push(case);
    }
    r.finish()?;
    Ok(cases)
}

#[test]
fn every_frozen_sequence_replays_and_matches_borrowed_entry_bounds() {
    use crate::greedy_block::GreedyBlock;
    let all = cases().unwrap();
    assert_eq!(all.len(), 173);
    assert_eq!(all.iter().filter(|c| c.row_search).count(), 93);
    for case in all {
        case.verify_sequences()
            .unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let block = GreedyBlock::new(
            case.source,
            case.prefix..case.source.len(),
            case.prefix_floor,
            case.key_layout().unwrap(),
            1 << case.window_log,
            case.row_search,
        )
        .unwrap();
        assert_eq!(
            (block.initial_position(), block.repeat_floor()),
            (case.initial_current, case.repeat_floor),
            "{}",
            case.name
        );
        assert!(case.target_length <= 131_072);
    }
}

#[test]
fn complete_owner_maps_and_rust_seeded_backends_match_all_c_initial_states() {
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
        case.verify_tables()
            .unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let first = case.first_table.values(false).unwrap();
        let second = case.second_table.values(false).unwrap();
        let layout = case.key_layout().unwrap();
        let (observed_first, observed_second) = if case.row_search {
            let row = case.row_layout().unwrap();
            let mut table = RowTable::new_in(row, Global, Global).unwrap();
            for &p in &case.seeds {
                table
                    .insert(
                        row.key(
                            layout.read_key(&case.source[p as usize..]).unwrap(),
                            case.initial_salt,
                        )
                        .unwrap(),
                        p as usize,
                    )
                    .unwrap();
            }
            let (positions, tags) = table.test_maps();
            (
                positions.iter().map(|&p| raw(p)).collect::<Vec<_>>(),
                tags.iter().copied().map(u32::from).collect::<Vec<_>>(),
            )
        } else {
            let mut table = ReferenceChain::new_in(layout, case.chain_log, Global, Global).unwrap();
            for &p in &case.seeds {
                table
                    .insert(
                        layout.read_key(&case.source[p as usize..]).unwrap(),
                        p as usize,
                    )
                    .unwrap();
            }
            let (heads, links) = table.test_maps();
            (
                heads.iter().map(|&p| raw(p)).collect::<Vec<_>>(),
                links.iter().map(|&p| raw(p)).collect::<Vec<_>>(),
            )
        };
        assert_eq!(observed_first, first, "{} first owner", case.name);
        assert_eq!(observed_second, second, "{} second owner", case.name);
    }
}

#[test]
fn truncated_counts_bad_domains_and_nonexhausted_records_reject() {
    for length in 0..16 {
        assert!(decode(&FIXTURE[..length]).is_err());
    }
    assert!(decode(&FIXTURE[..FIXTURE.len() - 1]).is_err());
    let mut extra = FIXTURE.to_vec();
    extra.push(0);
    assert_eq!(decode(&extra).unwrap_err(), "trailing fixture bytes");
    let mut count = FIXTURE.to_vec();
    count[8..12].copy_from_slice(&172_u32.to_le_bytes());
    assert_eq!(decode(&count).unwrap_err(), "fixture identity");
    let mut extent = FIXTURE.to_vec();
    extent[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode(&extent).unwrap_err(), "blob cap");
    for bytes in [&[255_u8; 4][..], &[2, 0, 0, 0, 0, 0, 0, 0]] {
        assert_eq!(
            Reader { bytes }
                .records(524_288, 4, Reader::u32)
                .unwrap_err(),
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
    let name_len = u16::from_le_bytes(record[..2].try_into().unwrap()) as usize;
    let params = 2 + name_len;
    for (offset, value) in [
        (0, 2),
        (1, 22),
        (2, 20),
        (3, 19),
        (4, 9),
        (5, 8),
        (6, 5),
        (7, 4),
    ] {
        let mut bad = record.to_vec();
        bad[params + offset] = value;
        assert_eq!(OracleCase::read(&bad).unwrap_err(), "parameter domain");
    }
    for (field, value, error) in [
        (16, 1, "raw frontier bias"),
        (17, 2, "skipping domain"),
        (0, 131_073, "case extent"),
        (3, 1, "case extent"),
    ] {
        let mut bad = record.to_vec();
        let index = params + 8 + field * 4;
        bad[index..index + 4].copy_from_slice(&u32::to_le_bytes(value));
        assert_eq!(OracleCase::read(&bad).unwrap_err(), error);
    }
    assert_eq!(
        decode(&vec![0; (16 << 20) + 1]).unwrap_err(),
        "fixture size cap"
    );
}

#[test]
fn forged_sequence_semantics_reject_raw_fields_repeats_salts_and_entry_bounds() {
    let all = cases().unwrap();
    let original = all
        .iter()
        .find(|c| c.name == "chain-long-literals-65535")
        .unwrap();
    for field in 0..13 {
        let mut c = original.clone();
        match field {
            0 => c.sequences[0].raw_literal_length ^= 1,
            1 => c.sequences[0].raw_match_base ^= 1,
            2 => c.sequences[0].offset_value = 2,
            3 => c.sequences[0].distance = 0,
            4 => c.parser_repeats[2] ^= 1,
            5 => c.wire_repeats[2] ^= 1,
            6 => c.long_length_kind = 0,
            7 => c.initial_current += 1,
            8 => c.repeat_floor += 1,
            9 => c.final_salt = 1,
            10 => c.final_entropy = 1,
            11 => c.hash_cache[0] = 1,
            _ => c.source_hash ^= 1,
        }
        assert!(c.verify_sequences().is_err(), "forgery {field}");
    }
    let mut row = all
        .iter()
        .find(|c| c.name == "row-entropy-wrap")
        .unwrap()
        .clone();
    row.hash_cache[0] = u32::MAX;
    assert_eq!(row.verify_sequences(), Err("cache hash width"));
}

#[test]
fn forged_full_maps_reject_invalid_roles_order_values_and_coherent_bad_heads() {
    let all = cases().unwrap();
    let original = all
        .iter()
        .find(|c| c.name == "row-width-64-insertions-65")
        .unwrap();
    for table in [&original.first_table, &original.second_table] {
        for field in 0..8 {
            let mut t = table.clone();
            match field {
                0 => t.hashes[0] ^= 1,
                1 => t.hashes[1] ^= 1,
                2 => t.changes[0][0] = u32::MAX,
                3 => t.changes[0][1] ^= 1,
                4 => t.changes[0][2] = t.changes[0][1],
                5 => t.before[0][1] = 0,
                6 => t.before.insert(1, t.before[0]),
                _ => t.element_bytes = 2,
            }
            assert!(t.values(true).is_err(), "map forgery {field}");
        }
    }
    let mut wrong_role = original.clone();
    wrong_role.second_table.log -= 1;
    assert_eq!(wrong_role.verify_tables(), Err("owner role geometry"));
    let mut wrong_position = original.clone();
    let mut values = wrong_position.first_table.values(false).unwrap();
    let slot = wrong_position.first_table.before[0][0] as usize;
    wrong_position.first_table.before[0][1] = 1;
    values[slot] = 1;
    wrong_position.first_table.hashes[0] = fnv(values.iter().flat_map(|v| v.to_le_bytes()));
    assert_eq!(wrong_position.verify_tables(), Err("raw position"));
    let mut wrong_head = original.clone();
    let width = 1usize << wrong_head.row_log;
    let mut values = wrong_head.second_table.values(false).unwrap();
    let head = wrong_head
        .second_table
        .before
        .iter_mut()
        .find(|r| (r[0] as usize).is_multiple_of(width))
        .unwrap();
    head[1] = width as u32;
    values[head[0] as usize] = width as u32;
    wrong_head.second_table.hashes[0] = fnv(values.iter().map(|&v| v as u8));
    assert_eq!(wrong_head.verify_tables(), Err("reserved head"));
}
