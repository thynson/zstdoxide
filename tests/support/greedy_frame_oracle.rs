//! Checked test-only access to frozen ordinary Greedy trajectories.
//!
//! Source/literal bytes borrow a checked projection of fixed C recipes. Complete
//! frames borrow byte-identical C frames in the projection. Both table maps, raw parser
//! repeats and full wire repeats retain their distinct meanings. Validation does
//! not use the production parser; full Rust frame decoding is a separate test.
#![forbid(unsafe_code)]
#![allow(dead_code, reason = "Greedy retained-state integration follows")]

use alloc::vec::Vec;

const FIXTURE: &[u8] = include_bytes!("../fixtures/reference-greedy-frames/cases.bin");
const MAX_BLOCK: usize = 128 << 10;
const MAX_SOURCE: usize = 4 << 20;
type Result<T> = core::result::Result<T, &'static str>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct OracleSequence {
    pub(crate) literal_length: u32,
    pub(crate) match_length: u32,
    pub(crate) distance: u32,
    pub(crate) offset_value: u32,
    pub(crate) raw_literal_length: u32,
    pub(crate) raw_match_base: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct OracleCandidate<'a> {
    /// The C third raw cell is preserved, not equated to the semantic third.
    pub(crate) parser_repeats: [u32; 3],
    pub(crate) wire_repeats: [u32; 3],
    pub(crate) trailing_literals: usize,
    /// Includes the trailing literal suffix, unlike the direct oracle's store.
    pub(crate) literals: &'a [u8],
    pub(crate) sequences: Vec<OracleSequence>,
    pub(crate) long_length_kind: u8,
    pub(crate) long_length_position: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct OracleBlock<'a> {
    pub(crate) call: usize,
    pub(crate) start: usize,
    pub(crate) length: usize,
    pub(crate) prefix_floor: usize,
    /// Actual C ordinary-frame selection: raw=0, RLE=1, compressed=2.
    pub(crate) kind: u8,
    pub(crate) wire_before: [u32; 3],
    pub(crate) wire_after: [u32; 3],
    pub(crate) parser_before: [u32; 3],
    pub(crate) parser_after: [u32; 3],
    /// None means the tiny-call bypass preserved stale scratch, not zero sequences.
    pub(crate) candidate: Option<OracleCandidate<'a>>,
    pub(crate) sampling_before: OracleSampling,
    pub(crate) sampling_after: OracleSampling,
    pub(crate) first_table: OracleTable,
    pub(crate) second_table: OracleTable,
}

#[derive(Clone, Debug)]
pub(crate) struct OracleTable {
    pub(crate) log: u8,
    pub(crate) element_bytes: u8,
    /// Sorted slot, old and new raw cell. Source indices use bias two; tags do not.
    pub(crate) changes: Vec<[u32; 3]>,
    pub(crate) hashes: [u64; 2],
}

#[derive(Clone, Debug)]
pub(crate) struct OracleFrameCase<'a> {
    pub(crate) name: &'a str,
    pub(crate) window_log: u8,
    pub(crate) hash_log: u8,
    pub(crate) chain_log: u8,
    pub(crate) search_log: u8,
    pub(crate) min_match: u8,
    pub(crate) row_search: bool,
    pub(crate) target_length: u32,
    pub(crate) source: &'a [u8],
    pub(crate) source_hash: u64,
    pub(crate) frame: &'a [u8],
    pub(crate) whole_frame: Option<&'a [u8]>,
    /// Includes zero-size Continue calls; blocks contains only nonempty calls.
    pub(crate) schedule: Vec<usize>,
    pub(crate) blocks: Vec<OracleBlock<'a>>,
}

struct Reader<'a> {
    bytes: &'a [u8],
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let value = self.bytes.get(..count).ok_or("truncated trajectory")?;
        self.bytes = &self.bytes[count..];
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
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.number()?))
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
            Err("trailing trajectory bytes")
        }
    }
}

fn fingerprint(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}
fn table_hash(table: &[u32], bytes: u8) -> u64 {
    fingerprint(
        table
            .iter()
            .flat_map(|n| n.to_le_bytes().into_iter().take(usize::from(bytes))),
    )
}

/// Independent width-specific primitive, checked against frozen C hash vectors.
fn key_hash(source: &[u8], position: usize, width: usize, bits: u8, salt: u64) -> Result<usize> {
    if !(4..=6).contains(&width) || !(1..=23).contains(&bits) {
        return Err("key shape");
    }
    let end = position.checked_add(width).ok_or("key overflow")?;
    let bytes = source.get(position..end).ok_or("key extent")?;
    let mut word = [0; 8];
    word[..width].copy_from_slice(bytes);
    let word = u64::from_le_bytes(word);
    if width == 4 {
        return Ok(
            (((word as u32).wrapping_mul(2_654_435_761) ^ salt as u32) >> (32 - bits)) as usize,
        );
    }
    let prime = if width == 5 {
        889_523_592_379
    } else {
        227_718_039_650_203
    };
    Ok(((word
        .wrapping_shl((8 * (8 - width)) as u32)
        .wrapping_mul(prime)
        ^ salt)
        >> (64 - bits)) as usize)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OracleSampling {
    pub(crate) raw_next: u32,
    pub(crate) skipping: bool,
    pub(crate) row_hash_log: u32,
    pub(crate) salt_entropy: u32,
    pub(crate) salt: u64,
    pub(crate) cache: [u32; 8],
}
impl OracleSampling {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let raw_next = r.u32()?;
        let skipping = r.u32()?;
        let row_hash_log = r.u32()?;
        let salt_entropy = r.u32()?;
        let salt = r.u64()?;
        let mut cache = [0; 8];
        for value in &mut cache {
            *value = r.u32()?;
        }
        if !(2..=MAX_SOURCE as u32 + 2).contains(&raw_next) || skipping > 1 || row_hash_log > 15 {
            return Err("sampling scalar domain");
        }
        Ok(Self {
            raw_next,
            skipping: skipping != 0,
            row_hash_log,
            salt_entropy,
            salt,
            cache,
        })
    }
    pub(crate) fn next(self) -> Result<usize> {
        usize::try_from(self.raw_next.checked_sub(2).ok_or("raw frontier bias")?)
            .map_err(|_| "frontier size")
    }
}

fn resolve(repeat: [u32; 3], offset: u32, literals: u32) -> Result<(u32, [u32; 3])> {
    let [a, b, c] = repeat;
    if offset > 3 {
        return Ok((offset - 3, [offset - 3, a, b]));
    }
    match (offset, literals == 0) {
        (1, false) => Ok((a, repeat)),
        (1, true) => Ok((b, [b, a, c])),
        _ => Err("invalid repeat selector"),
    }
}

impl<'a> OracleBlock<'a> {
    fn read(bytes: &'a [u8]) -> Result<Self> {
        let mut r = Reader { bytes };
        let call = r.size()?;
        let start = r.size()?;
        let length = r.size()?;
        let prefix_floor = r.size()?;
        let kind = r.byte()?;
        let fresh = r.byte()?;
        if kind > 2 || fresh > 1 || r.number::<2>()? != [0, 0] || !(1..=MAX_BLOCK).contains(&length)
        {
            return Err("block flags/length");
        }
        let wire_before = r.repeats()?;
        let wire_after = r.repeats()?;
        let parser_before = r.repeats()?;
        let parser_after = r.repeats()?;
        let sampling_before = OracleSampling::read(&mut r)?;
        let sampling_after = OracleSampling::read(&mut r)?;
        let mut table = || -> Result<OracleTable> {
            let log = r.byte()?;
            let element_bytes = r.byte()?;
            if !(6..=19).contains(&log)
                || ![1, 4].contains(&element_bytes)
                || r.number::<2>()? != [0; 2]
            {
                return Err("table dimensions");
            }
            let changes = r.records(1usize << log, 12, |r| Ok([r.u32()?, r.u32()?, r.u32()?]))?;
            Ok(OracleTable {
                log,
                element_bytes,
                changes,
                hashes: [r.u64()?, r.u64()?],
            })
        };
        let first_table = table()?;
        let second_table = table()?;
        let candidate = if fresh == 1 {
            let parser_repeats = r.repeats()?;
            let wire_repeats = r.repeats()?;
            let trailing_literals = r.size()?;
            let long_length_kind = r.byte()?;
            if long_length_kind > 2 || r.number::<3>()? != [0; 3] {
                return Err("long marker shape");
            }
            let long_length_position = r.size()?;
            let literals = r.blob(length)?;
            let sequences = r.records(length / 4, 24, |r| {
                Ok(OracleSequence {
                    literal_length: r.u32()?,
                    match_length: r.u32()?,
                    distance: r.u32()?,
                    offset_value: r.u32()?,
                    raw_literal_length: r.u32()?,
                    raw_match_base: r.u32()?,
                })
            })?;
            Some(OracleCandidate {
                parser_repeats,
                wire_repeats,
                trailing_literals,
                literals,
                sequences,
                long_length_kind,
                long_length_position,
            })
        } else {
            None
        };
        if candidate.is_some() != (length >= 7) {
            return Err("fresh versus stale tiny candidate");
        }
        r.finish()?;
        Ok(Self {
            call,
            start,
            length,
            prefix_floor,
            kind,
            wire_before,
            wire_after,
            parser_before,
            parser_after,
            candidate,
            first_table,
            second_table,
            sampling_before,
            sampling_after,
        })
    }
}

impl<'a> OracleFrameCase<'a> {
    fn read(bytes: &'a [u8]) -> Result<Self> {
        let mut r = Reader { bytes };
        let name_len = usize::from(u16::from_le_bytes(r.number()?));
        if !(1..=63).contains(&name_len) {
            return Err("case name length");
        }
        let name = core::str::from_utf8(r.take(name_len)?).map_err(|_| "case name UTF-8")?;
        if !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err("case name domain");
        }
        let window_log = r.byte()?;
        let hash_log = r.byte()?;
        let chain_log = r.byte()?;
        let search_log = r.byte()?;
        let min_match = r.byte()?;
        let row = r.byte()?;
        if !(10..=21).contains(&window_log)
            || !(6..=19).contains(&hash_log)
            || !(6..=29).contains(&chain_log)
            || !(1..=29).contains(&search_log)
            || !(3..=7).contains(&min_match)
            || row > 1
            || (row != 0) != (window_log > 14)
            || (row == 0 && (chain_log > 18 || search_log > 8))
            || r.number::<2>()? != [0; 2]
        {
            return Err("case parameters");
        }
        let target_length = r.u32()?;
        if target_length > 131_072 {
            return Err("target bound");
        }
        let source = r.blob(MAX_SOURCE)?;
        let source_hash = r.u64()?;
        if fingerprint(source.iter().copied()) != source_hash {
            return Err("source fingerprint");
        }
        let frame = r.blob(MAX_SOURCE + 256)?;
        let frame_hash = r.u64()?;
        let whole = r.byte()?;
        if whole > 1 {
            return Err("whole frame flag");
        }
        let whole_frame = if whole != 0 {
            let bytes = r.blob(MAX_SOURCE + 256)?;
            if fingerprint(bytes.iter().copied()) != r.u64()? {
                return Err("whole frame fingerprint");
            }
            Some(bytes)
        } else {
            None
        };
        if fingerprint(frame.iter().copied()) != frame_hash {
            return Err("complete frame fingerprint");
        }
        let schedule = r.records(64, 4, Reader::size)?;
        let blocks = r.records(64, 4, |r| OracleBlock::read(r.blob(16 << 20)?))?;
        r.finish()?;
        let result = Self {
            name,
            window_log,
            hash_log,
            chain_log,
            search_log,
            min_match,
            row_search: row != 0,
            target_length,
            source,
            source_hash,
            frame,
            whole_frame,
            schedule,
            blocks,
        };
        Ok(result)
    }

    fn validate(&self) -> Result<()> {
        if !(10..=21).contains(&self.window_log)
            || !(6..=19).contains(&self.hash_log)
            || !(6..=29).contains(&self.chain_log)
            || !(1..=29).contains(&self.search_log)
            || !(3..=7).contains(&self.min_match)
            || self.row_search != (self.window_log > 14)
            || (!self.row_search && (self.chain_log > 18 || self.search_log > 8))
            || self.source.len() > MAX_SOURCE
            || fingerprint(self.source.iter().copied()) != self.source_hash
            || self.target_length > 131_072
            || self
                .schedule
                .iter()
                .any(|&n| n > MAX_BLOCK.min(1 << self.window_log))
            || self.blocks.len() != self.schedule.iter().filter(|&&n| n > 0).count()
        {
            return Err("source schedule shape");
        }
        let mut first_table = empty_table(self.hash_log)?;
        let mut second_table = empty_table(if self.row_search {
            self.hash_log
        } else {
            self.chain_log
        })?;
        let mut cursor: usize = 0;
        let mut wire = [1, 4, 8];
        let mut raw = [1, 4, 8];
        let window = 1 << self.window_log;
        let width = usize::from(self.min_match.clamp(4, 6));
        let row_log = self.search_log.clamp(4, 6);
        let mut sampling = OracleSampling {
            raw_next: 2,
            skipping: false,
            row_hash_log: if self.row_search {
                u32::from(self.hash_log - row_log)
            } else {
                0
            },
            salt_entropy: 0,
            salt: if self.row_search {
                0x8358_92b4_bb2c_ae74
            } else {
                0
            },
            cache: [0; 8],
        };
        for (block, (call, &length)) in self
            .blocks
            .iter()
            .zip(self.schedule.iter().enumerate().filter(|(_, n)| **n > 0))
        {
            let end = cursor
                .checked_add(length)
                .filter(|&n| n <= self.source.len())
                .ok_or("source block extent")?;
            if block.call != call
                || block.start != cursor
                || block.length != length
                || block.candidate.is_some() != (length >= 7)
                || block.prefix_floor != cursor.saturating_sub(window)
                || block.wire_before != wire
                || block.parser_before != raw
                || raw[..2] != wire[..2]
                || block.kind > 2
                || block.sampling_before != sampling
            {
                return Err("block trajectory before state");
            }
            let state = block.sampling_after;
            let frontier = state.next()?;
            if frontier > end
                || state.row_hash_log != sampling.row_hash_log
                || state.salt != sampling.salt
                || (!self.row_search && (state.salt_entropy != 0 || state.cache != [0; 8]))
                || (self.row_search
                    && state
                        .cache
                        .iter()
                        .any(|&h| h >= 1 << (state.row_hash_log + 8)))
            {
                return Err("sampling geometry/frontier");
            }
            let mut entering = sampling.next()?.max(block.prefix_floor);
            if length >= 7 {
                let gap = cursor
                    .checked_sub(entering)
                    .ok_or("frontier beyond block start")?;
                if gap > 384 {
                    entering = cursor - 192.min(gap - 384);
                }
                if frontier < entering {
                    return Err("ordered fresh frontier adjustments");
                }
            } else if state
                != (OracleSampling {
                    raw_next: (entering + 2) as u32,
                    ..sampling
                })
            {
                return Err("tiny floor clamp only");
            }
            if let Some(candidate) = &block.candidate {
                let mut position = cursor;
                let mut literal_cursor: usize = 0;
                let mut trial = wire;
                let mut marker = None;
                for (sequence_index, sequence) in candidate.sequences.iter().enumerate() {
                    let ll =
                        usize::try_from(sequence.literal_length).map_err(|_| "literal size")?;
                    let ml = usize::try_from(sequence.match_length).map_err(|_| "match size")?;
                    if sequence.raw_literal_length != (sequence.literal_length & 65535)
                        || sequence.match_length < 4
                        || sequence.raw_match_base != ((sequence.match_length - 3) & 65535)
                    {
                        return Err("raw sequence lengths");
                    }
                    for (kind, value) in
                        [(1, sequence.literal_length), (2, sequence.match_length - 3)]
                    {
                        if value >= 65536 {
                            if marker.is_some() {
                                return Err("multiple long lengths");
                            }
                            marker = Some((kind, sequence_index));
                        }
                    }
                    let next_literal = literal_cursor
                        .checked_add(ll)
                        .filter(|&n| n <= candidate.literals.len())
                        .ok_or("literal extent")?;
                    let next_position = position
                        .checked_add(ll)
                        .filter(|&n| n <= end)
                        .ok_or("literal output extent")?;
                    if self.source[position..next_position]
                        != candidate.literals[literal_cursor..next_literal]
                    {
                        return Err("literal bytes");
                    }
                    position = next_position;
                    literal_cursor = next_literal;
                    let (distance, next) =
                        resolve(trial, sequence.offset_value, sequence.literal_length)?;
                    let distance_usize = usize::try_from(distance).map_err(|_| "distance size")?;
                    let previous = position
                        .checked_sub(distance_usize)
                        .filter(|&n| n >= block.prefix_floor)
                        .ok_or("match prefix")?;
                    let match_end = position
                        .checked_add(ml)
                        .filter(|&n| n <= end)
                        .ok_or("match extent")?;
                    if ml < 4
                        || distance != sequence.distance
                        || distance_usize > window
                        || self.source[position..match_end] != self.source[previous..previous + ml]
                    {
                        return Err("independent match replay");
                    }
                    position = match_end;
                    trial = next;
                }
                if marker.unwrap_or((0, 0))
                    != (candidate.long_length_kind, candidate.long_length_position)
                    || candidate.trailing_literals != end - position
                    || candidate.trailing_literals != candidate.literals.len() - literal_cursor
                    || self.source[position..end] != candidate.literals[literal_cursor..]
                    || candidate.wire_repeats != trial
                    || candidate.parser_repeats[..2] != trial[..2]
                    || candidate.parser_repeats[2] != raw[2]
                {
                    return Err("candidate trailing/repeats");
                }
                if block.kind == 2 {
                    wire = trial;
                    raw = candidate.parser_repeats;
                }
            } else if block.kind != 0
                || !block.first_table.changes.is_empty()
                || !block.second_table.changes.is_empty()
            {
                return Err("tiny bypass changed candidate/table");
            }
            if block.wire_after != wire || block.parser_after != raw || raw[..2] != wire[..2] {
                return Err("selected publication state");
            }
            block
                .first_table
                .apply(&mut first_table, self.hash_log, 4)?;
            block.second_table.apply(
                &mut second_table,
                if self.row_search {
                    self.hash_log
                } else {
                    self.chain_log
                },
                if self.row_search { 1 } else { 4 },
            )?;
            self.verify_tables(
                &first_table,
                &second_table,
                end,
                frontier,
                width,
                state.salt,
            )?;
            sampling = state;
            cursor = end;
        }
        if cursor != self.source.len() {
            return Err("complete source coverage");
        }
        self.validate_frame(self.frame, false)?;
        if let Some(whole) = self.whole_frame {
            self.validate_frame(whole, true)?;
        }
        Ok(())
    }

    fn validate_frame(&self, frame: &[u8], whole: bool) -> Result<()> {
        // Header decoding is shared with the existing decoder, never with the
        // matching parser. Each block boundary and raw/RLE payload is checked
        // here independently of the separate full compressed-frame test.
        let (header, consumed) = crate::FrameHeader::parse(frame).map_err(|_| "frame header")?;
        if header.content_size != Some(self.source.len() as u64)
            || header.window_size != self.source.len().min(1usize << self.window_log) as u64
            || header.dictionary_id != 0
            || header.has_checksum
        {
            return Err("plain frame metadata");
        }
        let mut r = Reader {
            bytes: frame.get(consumed..).ok_or("header extent")?,
        };
        for (index, block) in self.blocks.iter().enumerate() {
            let [a, b, c] = r.number::<3>()?;
            let value = u32::from_le_bytes([a, b, c, 0]);
            let field = (value >> 3) as usize;
            let last = value & 1 != 0;
            if ((value >> 1) & 3) as u8 != block.kind
                || last != (whole && index + 1 == self.blocks.len())
                || field > MAX_BLOCK
                || (block.kind < 2 && field != block.length)
            {
                return Err("frame block selection");
            }
            let payload = r.take(if block.kind == 1 { 1 } else { field })?;
            let source = &self.source[block.start..block.start + block.length];
            if (block.kind == 0 && payload != source)
                || (block.kind == 1 && source.iter().any(|&byte| byte != payload[0]))
            {
                return Err("raw/RLE frame payload");
            }
        }
        if !whole && r.number::<3>()? != [1, 0, 0] {
            return Err("terminal empty raw block");
        }
        r.finish()
    }
}

pub(crate) fn cases() -> Result<Vec<OracleFrameCase<'static>>> {
    decode(FIXTURE)
}

fn decode(bytes: &[u8]) -> Result<Vec<OracleFrameCase<'_>>> {
    if bytes.len() > 64 << 20 {
        return Err("trajectory file cap");
    }
    let mut r = Reader { bytes };
    if r.take(8)? != b"ZGRFR01\0" || r.size()? != 138 || r.size()? != 395 {
        return Err("trajectory identity");
    }
    let mut cases = Vec::new();
    cases.try_reserve_exact(138).map_err(|_| "allocation")?;
    let mut blocks = 0;
    for _ in 0..138 {
        let case = OracleFrameCase::read(r.blob(32 << 20)?)?;
        blocks += case.blocks.len();
        cases.push(case);
    }
    r.finish()?;
    if blocks != 395 {
        return Err("complete block count");
    }
    Ok(cases)
}

fn empty_table(log: u8) -> Result<Vec<u32>> {
    let count = 1usize << log;
    let mut table = Vec::new();
    table.try_reserve_exact(count).map_err(|_| "allocation")?;
    table.resize(count, 0);
    Ok(table)
}

impl OracleTable {
    pub(crate) fn apply(&self, table: &mut [u32], log: u8, bytes: u8) -> Result<()> {
        if self.log != log
            || self.element_bytes != bytes
            || !(6..=19).contains(&log)
            || ![1, 4].contains(&bytes)
            || table.len() != 1usize << log
            || table_hash(table, bytes) != self.hashes[0]
        {
            return Err("table dimensions/before hash");
        }
        let mut last = None;
        for &[slot, old, new] in &self.changes {
            let slot = usize::try_from(slot).map_err(|_| "slot size")?;
            if last.is_some_and(|n| n >= slot)
                || old == new
                || (bytes == 1 && (old > 255 || new > 255))
            {
                return Err("delta order/value");
            }
            let cell = table.get_mut(slot).ok_or("table slot")?;
            if *cell != old {
                return Err("old table cell");
            }
            *cell = new;
            last = Some(slot);
        }
        if table_hash(table, bytes) != self.hashes[1] {
            return Err("table after hash");
        }
        Ok(())
    }
}

impl OracleFrameCase<'_> {
    fn verify_tables(
        &self,
        first: &[u32],
        second: &[u32],
        end: usize,
        frontier: usize,
        width: usize,
        salt: u64,
    ) -> Result<()> {
        let position = |raw: u32| -> Result<usize> {
            let pos = usize::try_from(raw.checked_sub(2).ok_or("raw position bias")?)
                .map_err(|_| "position size")?;
            if pos >= frontier || pos.checked_add(width).is_none_or(|n| n > end) {
                return Err("stored position extent/frontier");
            }
            Ok(pos)
        };
        if self.row_search {
            let row_log = self.search_log.clamp(4, 6);
            let row_width = 1usize << row_log;
            for (row, values) in first.chunks_exact(row_width).enumerate() {
                let base = row * row_width;
                let head = second[base] as usize;
                if values[0] != 0
                    || head >= row_width
                    || (head == 0) != values.iter().all(|&v| v == 0)
                    || (head != 0 && values[head] == 0)
                {
                    return Err("reserved row/head occupancy");
                }
                for (slot, &raw) in values.iter().enumerate().skip(1) {
                    let tag = second[base + slot];
                    if raw == 0 {
                        if tag != 0 {
                            return Err("empty row tag");
                        }
                        continue;
                    }
                    let hash = key_hash(
                        self.source,
                        position(raw)?,
                        width,
                        self.hash_log - row_log + 8,
                        salt,
                    )?;
                    if (hash >> 8) << row_log != base || (hash & 255) as u32 != tag {
                        return Err("row hash/tag membership");
                    }
                }
            }
        } else {
            for (slot, &raw) in first.iter().enumerate() {
                if raw != 0
                    && key_hash(self.source, position(raw)?, width, self.hash_log, 0)? != slot
                {
                    return Err("head membership");
                }
            }
            for &raw in second {
                if raw != 0 {
                    position(raw)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ordinary_records_replay_both_complete_tables_and_publication() {
        let cases = cases().unwrap();
        assert_eq!(cases.len(), 138);
        assert_eq!(
            cases.iter().map(|c| c.source.len()).sum::<usize>(),
            7_365_784
        );
        let mut kinds = [0; 3];
        let mut discarded = [0; 2];
        let mut fresh = 0;
        for case in &cases {
            case.validate()
                .unwrap_or_else(|e| panic!("{}: {e}", case.name));
            for block in &case.blocks {
                kinds[usize::from(block.kind)] += 1;
                if let Some(candidate) = &block.candidate {
                    fresh += 1;
                    if block.kind < 2 && candidate.wire_repeats[..2] != block.wire_before[..2] {
                        discarded[usize::from(block.kind)] += 1;
                    }
                }
            }
        }
        assert_eq!(kinds, [181, 38, 176]);
        assert_eq!(discarded, [25, 12]);
        assert_eq!(fresh, 367);
    }

    #[test]
    fn every_complete_c_frame_decodes_with_existing_rust_code() {
        let mut count = 0;
        let mut bytes = 0;
        for case in cases().unwrap() {
            for frame in core::iter::once(case.frame).chain(case.whole_frame) {
                let decoded = crate::decompress(frame, case.source.len())
                    .unwrap_or_else(|e| panic!("{}: {e:?}", case.name));
                assert_eq!(decoded, case.source, "{}", case.name);
                bytes += decoded.len();
                count += 1;
            }
        }
        assert_eq!((count, bytes), (151, 13_587_614));
    }

    #[test]
    fn tiny_floor_clamps_and_long_markers_preserve_distinct_boundaries() {
        let cases = cases().unwrap();
        for branch in ["chain", "row"] {
            for tiny in 1..=6 {
                let name = alloc::format!("{branch}_window_floor_tiny{tiny}");
                let case = cases.iter().find(|c| c.name == name).unwrap();
                let block = &case.blocks[2];
                assert!(block.candidate.is_none());
                assert!(block.sampling_before.next().unwrap() < block.prefix_floor);
                assert_eq!(block.sampling_after.next().unwrap(), block.prefix_floor);
                assert!(
                    block.first_table.changes.is_empty() && block.second_table.changes.is_empty()
                );
                assert_eq!(
                    block.sampling_after,
                    OracleSampling {
                        raw_next: (block.prefix_floor + 2) as u32,
                        ..block.sampling_before
                    }
                );
            }
        }
        for (name, kind, length, raw) in [
            ("row_long_match65543", 2, 65_539, 0),
            ("row_long_literals", 1, 65_796, 260),
        ] {
            let c = cases.iter().find(|c| c.name == name).unwrap().blocks[0]
                .candidate
                .as_ref()
                .unwrap();
            assert_eq!((c.long_length_kind, c.long_length_position), (kind, 0));
            let s = c.sequences[0];
            assert_eq!(
                if kind == 1 {
                    (s.literal_length, s.raw_literal_length)
                } else {
                    (s.match_length, s.raw_match_base)
                },
                (length, raw)
            );
        }
        let unused = cases
            .iter()
            .find(|c| c.row_search && c.chain_log == 29)
            .unwrap();
        assert_eq!(unused.blocks[0].second_table.log, unused.hash_log);
        assert_eq!(unused.blocks[0].second_table.element_bytes, 1);
    }

    #[test]
    fn truncated_extents_dimensions_counts_and_fingerprints_reject() {
        for length in [0, 7, 8, 15, 16] {
            assert!(decode(&FIXTURE[..length]).is_err());
        }
        for offset in [8, 12, 16] {
            let mut data = FIXTURE.to_vec();
            data[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(decode(&data).is_err());
        }
        let size = u32::from_le_bytes(FIXTURE[16..20].try_into().unwrap()) as usize;
        let first = &FIXTURE[20..20 + size];
        for end in 0..first.len() {
            assert!(OracleFrameCase::read(&first[..end]).is_err());
        }
        let mut extra = first.to_vec();
        extra.push(0);
        assert!(OracleFrameCase::read(&extra).is_err());
        let name = u16::from_le_bytes(first[..2].try_into().unwrap()) as usize;
        for (offset, value) in [
            (2 + name, 9),
            (3 + name, 20),
            (4 + name, 30),
            (5 + name, 0),
            (6 + name, 2),
            (7 + name, 1),
            (8 + name, 1),
        ] {
            let mut data = first.to_vec();
            data[offset] = value;
            assert!(OracleFrameCase::read(&data).is_err());
        }
        // Empty first source: header/name, parameters, target, source size, hash.
        let mut data = first.to_vec();
        data[2 + name + 8 + 4 + 4] ^= 1;
        assert!(OracleFrameCase::read(&data).is_err());
        let mut reader = Reader { bytes: &[1] };
        assert!(reader.take(2).is_err());
        assert_eq!(reader.bytes, [1]);
        let mut reader = Reader { bytes: &[255; 4] };
        assert!(reader.records::<u8>(64, 4, |_| Ok(0)).is_err());
    }

    #[test]
    fn actual_repeat_table_marker_and_sampling_forgeries_reject() {
        let cases = cases().unwrap();
        for branch in ["chain", "row"] {
            let name = alloc::format!("{branch}_raw_candidate_k4_d128");
            let original = cases.iter().find(|c| c.name == name).unwrap();
            original.validate().unwrap();
            let mut altered = original.clone();
            altered.blocks[1].wire_after =
                altered.blocks[1].candidate.as_ref().unwrap().wire_repeats;
            assert!(altered.validate().is_err());
            let mut altered = original.clone();
            altered.blocks[1].candidate.as_mut().unwrap().sequences[0].offset_value = 2;
            assert!(altered.validate().is_err());
            let mut altered = original.clone();
            altered.blocks[1].candidate.as_mut().unwrap().sequences[0].raw_literal_length ^= 1;
            assert!(altered.validate().is_err());
            let mut altered = original.clone();
            altered.blocks[1]
                .candidate
                .as_mut()
                .unwrap()
                .long_length_kind = 1;
            assert!(altered.validate().is_err());
            for first in [true, false] {
                let mut altered = original.clone();
                let table = if first {
                    &mut altered.blocks[0].first_table
                } else {
                    &mut altered.blocks[0].second_table
                };
                table.hashes[1] ^= 1;
                assert!(altered.validate().is_err());
            }
            // Recompute the complete hash after a forged raw head/position. A
            // coherent digest must not bypass source-index and membership checks.
            let mut altered = original.clone();
            let table = &mut altered.blocks[0].first_table;
            assert!(!table.changes.is_empty());
            table.changes[0][2] = 1;
            let mut values = empty_table(table.log).unwrap();
            for &[slot, _, value] in &table.changes {
                values[slot as usize] = value;
            }
            table.hashes[1] = table_hash(&values, 4);
            assert!(altered.validate().is_err());
            let name = alloc::format!("{branch}_window_floor_tiny1");
            let mut altered = cases.iter().find(|c| c.name == name).unwrap().clone();
            altered.blocks[2].sampling_after = altered.blocks[2].sampling_before;
            assert!(altered.validate().is_err());
        }
        let mut row = cases
            .iter()
            .find(|c| c.name == "row_first_uniform")
            .unwrap()
            .clone();
        row.blocks[0].sampling_after.salt ^= 1;
        assert!(row.validate().is_err());
        let mut row = cases
            .iter()
            .find(|c| c.name == "row_first_uniform")
            .unwrap()
            .clone();
        row.blocks[0].sampling_after.cache[0] = u32::MAX;
        assert!(row.validate().is_err());
    }
}
