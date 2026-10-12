//! Checked test-only access to frozen ordinary DoubleFast trajectories.
//!
//! Source/literal bytes borrow a checked projection of fixed C recipes. Complete
//! frames borrow the original collection. Two independent table maps, raw parser
//! repeats and full wire repeats retain their distinct meanings. Validation does
//! not use the production parser; full Rust frame decoding is a separate test.
#![forbid(unsafe_code)]

use alloc::vec::Vec;

const FIXTURE: &[u8] = include_bytes!("../fixtures/reference-double-fast-frames/cases.bin");
const MAX_BLOCK: usize = 128 << 10;
const MAX_SOURCE: usize = 4 << 20;
type Result<T> = core::result::Result<T, &'static str>;

type FrameFiles = (&'static str, &'static [u8], Option<&'static [u8]>);
include!("../fixtures/reference-double-fast-frames/frames.rs");

#[derive(Clone, Copy, Debug)]
pub(crate) struct OracleSequence {
    pub(crate) literal_length: u32,
    pub(crate) match_length: u32,
    pub(crate) distance: u32,
    pub(crate) offset_value: u32,
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
    /// Sorted bucket, old raw C index, new raw C index; zero is invalid, bias=2.
    pub(crate) long_table: OracleTable,
    pub(crate) short_table: OracleTable,
}

#[derive(Clone, Debug)]
pub(crate) struct OracleTable {
    pub(crate) changes: Vec<[u32; 3]>,
    pub(crate) hashes: [u64; 2],
}

#[derive(Clone, Debug)]
pub(crate) struct OracleFrameCase<'a> {
    pub(crate) name: &'a str,
    pub(crate) window_log: u8,
    pub(crate) long_log: u8,
    pub(crate) short_log: u8,
    pub(crate) search_log: u8,
    pub(crate) min_match: u8,
    pub(crate) target_length: u32,
    pub(crate) source: &'a [u8],
    pub(crate) source_hash: u64,
    pub(crate) frame: &'static [u8],
    pub(crate) whole_frame: Option<&'static [u8]>,
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
fn table_hash(table: &[u32]) -> u64 {
    fingerprint(table.iter().flat_map(|n| n.to_le_bytes()))
}

fn bucket(bytes: &[u8], log: u8) -> Result<usize> {
    let width = bytes.len();
    if !(4..=8).contains(&width) || !(6..=18).contains(&log) {
        return Err("hash shape");
    }
    let mut initialized = [0; 8];
    initialized[..width].copy_from_slice(bytes);
    let word = u64::from_le_bytes(initialized);
    if width == 4 {
        return Ok(((word as u32).wrapping_mul(2_654_435_761) >> (32 - log)) as usize);
    }
    let prime = [
        889_523_592_379,
        227_718_039_650_203,
        58_295_818_150_454_627,
        0xcf1b_bcdc_b7a5_6463,
    ][width - 5];
    Ok((word
        .wrapping_shl((8 * (8 - width)) as u32)
        .wrapping_mul(prime)
        >> (64 - log)) as usize)
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
    fn read(bytes: &'a [u8], long_buckets: usize, short_buckets: usize) -> Result<Self> {
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
        let mut table = |buckets: usize| -> Result<OracleTable> {
            let changes = r.records(buckets.min(length), 12, |r| {
                Ok([r.u32()?, r.u32()?, r.u32()?])
            })?;
            Ok(OracleTable {
                changes,
                hashes: [r.u64()?, r.u64()?],
            })
        };
        let long_table = table(long_buckets)?;
        let short_table = table(short_buckets)?;
        let candidate = if fresh == 1 {
            let parser_repeats = r.repeats()?;
            let wire_repeats = r.repeats()?;
            let trailing_literals = r.size()?;
            let literals = r.blob(length)?;
            let sequences = r.records(length / 4, 16, |r| {
                Ok(OracleSequence {
                    literal_length: r.u32()?,
                    match_length: r.u32()?,
                    distance: r.u32()?,
                    offset_value: r.u32()?,
                })
            })?;
            Some(OracleCandidate {
                parser_repeats,
                wire_repeats,
                trailing_literals,
                literals,
                sequences,
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
            long_table,
            short_table,
        })
    }
}

impl<'a> OracleFrameCase<'a> {
    fn read(bytes: &'a [u8], index: usize) -> Result<Self> {
        let mut r = Reader { bytes };
        let name_len = usize::from(u16::from_le_bytes(r.number()?));
        if !(1..=63).contains(&name_len) {
            return Err("case name length");
        }
        let name = core::str::from_utf8(r.take(name_len)?).map_err(|_| "case name UTF-8")?;
        let &(expected_name, frame, whole_frame) = FRAMES.get(index).ok_or("frame index")?;
        if name != expected_name {
            return Err("exact ordered case identity");
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
            || r.number::<3>()? != [0; 3]
        {
            return Err("case parameters");
        }
        let target_length = r.u32()?;
        let source = r.blob(MAX_SOURCE)?;
        let source_hash = r.u64()?;
        let frame_hash = r.u64()?;
        let whole_hash = r.u64()?;
        if fingerprint(frame.iter().copied()) != frame_hash
            || whole_frame.map_or(0, |f| fingerprint(f.iter().copied())) != whole_hash
        {
            return Err("complete C frame fingerprint");
        }
        let schedule = r.records(64, 4, Reader::size)?;
        let blocks = r.records(64, 4, |r| {
            OracleBlock::read(r.blob(4 << 20)?, 1usize << long_log, 1usize << short_log)
        })?;
        r.finish()?;
        let result = Self {
            name,
            window_log,
            long_log,
            short_log,
            search_log,
            min_match,
            target_length,
            source,
            source_hash,
            frame,
            whole_frame,
            schedule,
            blocks,
        };
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<()> {
        if !(10..=21).contains(&self.window_log)
            || !(6..=18).contains(&self.long_log)
            || !(6..=18).contains(&self.short_log)
            || !(1..=29).contains(&self.search_log)
            || !(3..=7).contains(&self.min_match)
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
        let mut long_table = empty_table(self.long_log)?;
        let mut short_table = empty_table(self.short_log)?;
        let mut cursor: usize = 0;
        let mut wire = [1, 4, 8];
        let mut raw = [1, 4, 8];
        let window = 1 << self.window_log;
        let width = usize::from(self.min_match.max(4));
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
            {
                return Err("block trajectory before state");
            }
            if let Some(candidate) = &block.candidate {
                let mut position = cursor;
                let mut literal_cursor: usize = 0;
                let mut trial = wire;
                for sequence in &candidate.sequences {
                    let ll =
                        usize::try_from(sequence.literal_length).map_err(|_| "literal size")?;
                    let ml = usize::try_from(sequence.match_length).map_err(|_| "match size")?;
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
                if candidate.trailing_literals != end - position
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
                || !block.long_table.changes.is_empty()
                || !block.short_table.changes.is_empty()
            {
                return Err("tiny bypass changed candidate/table");
            }
            if block.wire_after != wire || block.parser_after != raw || raw[..2] != wire[..2] {
                return Err("selected publication state");
            }
            block
                .long_table
                .apply(&mut long_table, self.source, cursor, end, self.long_log, 8)?;
            block.short_table.apply(
                &mut short_table,
                self.source,
                cursor,
                end,
                self.short_log,
                width,
            )?;
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
    if bytes.len() > 32 << 20 {
        return Err("trajectory file cap");
    }
    let mut r = Reader { bytes };
    if r.take(8)? != b"ZDFTR01\0" || r.size()? != 51 || r.size()? != 160 {
        return Err("trajectory identity");
    }
    let mut cases = Vec::new();
    cases.try_reserve_exact(51).map_err(|_| "allocation")?;
    let mut blocks = 0;
    for index in 0..51 {
        let case = OracleFrameCase::read(r.blob(16 << 20)?, index)?;
        blocks += case.blocks.len();
        cases.push(case);
    }
    r.finish()?;
    if blocks != 160 {
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
    fn apply(
        &self,
        table: &mut [u32],
        source: &[u8],
        start: usize,
        end: usize,
        log: u8,
        width: usize,
    ) -> Result<()> {
        if table_hash(table) != self.hashes[0] {
            return Err("table before fingerprint");
        }
        let mut last = None;
        for &[slot, old, new] in &self.changes {
            let slot = usize::try_from(slot).map_err(|_| "bucket size")?;
            let position = usize::try_from(new.checked_sub(2).ok_or("C index bias")?)
                .map_err(|_| "position size")?;
            let key_end = position
                .checked_add(width)
                .filter(|&n| n <= end)
                .ok_or("changed key extent")?;
            if slot >= table.len()
                || last.is_some_and(|prior| prior >= slot)
                || table[slot] != old
                || old == new
                || position < start
                || bucket(source.get(position..key_end).ok_or("key bounds")?, log)? != slot
            {
                return Err("ordered table delta/key");
            }
            table[slot] = new;
            last = Some(slot);
        }
        // Unchanged stale entries are intentionally retained. Their complete key
        // must still lie in consumed input, regardless of the current floor.
        for (slot, &raw) in table.iter().enumerate() {
            if raw == 0 {
                continue;
            }
            let position = usize::try_from(raw.checked_sub(2).ok_or("retained C index")?)
                .map_err(|_| "retained size")?;
            let key_end = position
                .checked_add(width)
                .filter(|&n| n <= end)
                .ok_or("retained extent")?;
            if bucket(
                source.get(position..key_end).ok_or("retained key bounds")?,
                log,
            )? != slot
            {
                return Err("complete table bucket");
            }
        }
        if table_hash(table) != self.hashes[1] {
            return Err("table after fingerprint");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ordinary_candidates_replay_both_complete_tables_and_publication() {
        let cases = cases().unwrap();
        assert_eq!(cases.len(), 51);
        assert_eq!(
            cases.iter().map(|c| c.source.len()).sum::<usize>(),
            6_348_944
        );
        let mut kinds = [0; 3];
        let mut tiny = 0;
        let mut discarded = [0; 2];
        for case in &cases {
            for block in &case.blocks {
                kinds[usize::from(block.kind)] += 1;
                if let Some(candidate) = &block.candidate {
                    if block.kind < 2 && candidate.wire_repeats[..2] != block.wire_before[..2] {
                        discarded[usize::from(block.kind)] += 1;
                    }
                } else {
                    tiny += 1;
                }
            }
        }
        assert_eq!(kinds, [88, 6, 66]);
        assert_eq!(tiny, 12);
        assert_eq!(discarded, [13, 3]);
    }

    #[test]
    fn all_complete_c_frames_decode_with_the_existing_rust_decoder() {
        let cases = cases().unwrap();
        let mut frames = 0;
        let mut bytes = 0;
        for case in cases {
            for frame in core::iter::once(case.frame).chain(case.whole_frame) {
                let decoded = crate::decompress(frame, case.source.len()).unwrap();
                assert_eq!(decoded, case.source, "{}", case.name);
                let (header, _) = crate::FrameHeader::parse(frame).unwrap();
                assert_eq!(header.content_size, Some(case.source.len() as u64));
                assert_eq!(
                    header.window_size,
                    case.source.len().min(1usize << case.window_log) as u64
                );
                assert_eq!(header.dictionary_id, 0);
                assert!(!header.has_checksum);
                frames += 1;
                bytes += decoded.len();
            }
        }
        assert_eq!(frames, 58);
        assert_eq!(bytes, 12_382_352);
    }

    #[test]
    fn empty_tiny_and_large_window_records_preserve_distinct_boundaries() {
        let cases = cases().unwrap();
        let zero = cases.iter().find(|c| c.name == "zero_uniform").unwrap();
        assert_eq!(zero.schedule, [0, 4096, 0, 0]);
        assert_eq!(zero.blocks[0].call, 1);
        let tiny = cases.iter().find(|c| c.name == "tiny_stale").unwrap();
        for block in &tiny.blocks[1..11] {
            assert_eq!(block.candidate.is_some(), block.length >= 7);
        }
        for (name, last_floor, last_kind) in [
            ("window_expiry20", 131_072, 0),
            ("window_retained21", 0, 2),
            ("window_expiry21", 131_072, 0),
        ] {
            let case = cases.iter().find(|c| c.name == name).unwrap();
            let block = case.blocks.last().unwrap();
            assert_eq!((block.prefix_floor, block.kind), (last_floor, last_kind));
            assert_eq!(
                block.prefix_floor,
                block.start.saturating_sub(1 << case.window_log)
            );
        }
        let unequal = cases.iter().find(|c| c.name == "table_l6_s18").unwrap();
        assert_eq!((unequal.long_log, unequal.short_log), (6, 18));
    }

    #[test]
    fn truncated_records_forged_counts_and_fingerprints_are_rejected() {
        for length in [0, 7, 8, 15, 16, FIXTURE.len() - 1] {
            assert!(decode(&FIXTURE[..length]).is_err());
        }
        let mut changed = FIXTURE.to_vec();
        changed.push(0);
        assert!(decode(&changed).is_err());
        for offset in [8, 12, 16] {
            let mut changed = FIXTURE.to_vec();
            changed[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(decode(&changed).is_err());
        }
        // First empty case: length prefix, u16/name, eight parameter bytes,
        // target and zero source length, then source/frame/whole fingerprints.
        for offset in [43, 51, 59] {
            let mut changed = FIXTURE.to_vec();
            changed[offset] ^= 1;
            assert!(decode(&changed).is_err());
        }
        let mut reader = Reader { bytes: &[255; 4] };
        assert!(reader.records::<u8>(64, 4, |_| Ok(0)).is_err());
        let mut reader = Reader { bytes: &[1] };
        assert!(reader.take(2).is_err());
        assert_eq!(reader.bytes, [1]);
    }

    #[test]
    fn either_table_repeat_intent_or_publication_forgery_is_rejected() {
        let cases = cases().unwrap();
        let original = cases.iter().find(|c| c.name == "prefix_length9").unwrap();
        let mut altered = original.clone();
        altered.blocks[1].wire_after = altered.blocks[1].candidate.as_ref().unwrap().wire_repeats;
        assert!(altered.validate().is_err());
        for long in [false, true] {
            let mut altered = original.clone();
            let table = if long {
                &mut altered.blocks[0].long_table
            } else {
                &mut altered.blocks[0].short_table
            };
            table.changes[0][2] = u32::MAX;
            assert!(altered.validate().is_err());
            let mut altered = original.clone();
            let table = if long {
                &mut altered.blocks[0].long_table
            } else {
                &mut altered.blocks[0].short_table
            };
            table.hashes[1] ^= 1;
            assert!(altered.validate().is_err());
        }
        let mut altered = original.clone();
        altered.blocks[0].candidate.as_mut().unwrap().sequences[0].offset_value = 2;
        assert!(altered.validate().is_err());
        let mut altered = original.clone();
        altered.blocks[0].candidate.as_mut().unwrap().parser_repeats[2] += 1;
        assert!(altered.validate().is_err());
        let mut tiny = cases
            .iter()
            .find(|c| c.name == "tiny_stale")
            .unwrap()
            .clone();
        tiny.blocks[1].candidate = original.blocks[0].candidate.clone();
        assert!(tiny.validate().is_err());
        let mut empty_candidate = cases
            .iter()
            .find(|c| c.name == "fresh_length7")
            .unwrap()
            .clone();
        empty_candidate.blocks[0].kind = 2;
        assert!(empty_candidate.validate().is_err());
    }
}
