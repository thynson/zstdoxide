//! Safe test-only access to frozen ordinary Fast candidate trajectories.
//!
//! Source bytes regenerate from independently frozen recipes; literal metadata
//! borrows the packed observations. Raw C repeat cells and semantic wire repeats
//! are intentionally separate. This module does not decode compressed frames.
#![forbid(unsafe_code)]

use alloc::vec::Vec;

const FIXTURE: &[u8] = include_bytes!("../fixtures/reference-fast-frames/cases.bin");
const MAX_BLOCK: usize = 128 << 10;
const MAX_SOURCE: usize = 2 << 20;
type Result<T> = core::result::Result<T, &'static str>;

const NAMES: [&str; 23] = [
    "empty",
    "first_uniform",
    "empty_then_first_uniform",
    "compressed_uniform_repeat",
    "raw_then_copy",
    "mixed_fallback",
    "tiny_stale",
    "literal_tree",
    "negative_literal_policy",
    "window_expiry_17",
    "window_retained_19",
    "raw_candidate_k4_d128",
    "raw_candidate_k4_d192",
    "raw_candidate_k4_d256",
    "raw_candidate_k4_d320",
    "raw_candidate_k8_d128",
    "raw_candidate_k8_d192",
    "raw_candidate_k8_d256",
    "raw_candidate_k8_d320",
    "raw_candidate_k12_d128",
    "raw_candidate_k12_d192",
    "raw_candidate_k12_d256",
    "raw_candidate_k12_d320",
];

#[derive(Clone, Copy, Debug)]
pub(crate) struct OracleSequence {
    pub(crate) literal_length: u32,
    pub(crate) match_length: u32,
    pub(crate) distance: u32,
    pub(crate) offset_value: u32,
}

#[derive(Debug)]
pub(crate) struct OracleCandidate<'a> {
    /// The C third raw cell is preserved, not equated to the semantic third.
    pub(crate) parser_repeats: [u32; 3],
    pub(crate) wire_repeats: [u32; 3],
    pub(crate) trailing_literals: usize,
    /// Includes the trailing literal suffix, unlike the direct oracle's store.
    pub(crate) literals: &'a [u8],
    pub(crate) sequences: Vec<OracleSequence>,
}

#[derive(Debug)]
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
    pub(crate) table_changes: Vec<[u32; 3]>,
    pub(crate) table_hashes: [u64; 2],
}

#[derive(Debug)]
pub(crate) struct OracleFrameCase<'a> {
    pub(crate) name: &'a str,
    pub(crate) window_log: u8,
    pub(crate) hash_log: u8,
    pub(crate) min_match: u8,
    pub(crate) target_length: u32,
    pub(crate) source: Vec<u8>,
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

fn next(seed: &mut u32) -> u8 {
    *seed ^= seed.wrapping_shl(13);
    *seed ^= *seed >> 17;
    *seed ^= seed.wrapping_shl(5);
    *seed as u8
}

/// Fixed recipe IDs are the ordered frozen C cases. Reserve the complete known
/// source once; all pushes below fit it. Temporary periodic storage is inline.
fn source(index: usize) -> Result<Vec<u8>> {
    let size = match index {
        0 => 0,
        1 | 2 => 4096,
        3 => 12_288,
        4 => 262_144,
        5 => 16_384,
        6 => 8210,
        7 | 8 => 9216,
        9 | 10 => 393_216,
        11..=22 => 9216,
        _ => return Err("recipe index"),
    };
    let mut out = Vec::new();
    out.try_reserve_exact(size).map_err(|_| "allocation")?;
    let noise = |out: &mut Vec<u8>, mut seed, size, mask| {
        for _ in 0..size {
            out.push(next(&mut seed) & mask);
        }
    };
    let periodic = |out: &mut Vec<u8>| {
        let mut seed = 0x1357_9bdf;
        let mut pattern = [0; 257];
        for byte in &mut pattern {
            *byte = next(&mut seed);
        }
        for i in 0..4096 {
            out.push(pattern[i % pattern.len()]);
        }
    };
    let uniform = |out: &mut Vec<u8>, byte, size| {
        let end = out.len() + size;
        while out.len() < end {
            out.push(byte);
        }
    };
    match index {
        0 => (),
        1 | 2 => uniform(&mut out, 0x5a, 4096),
        3 => {
            periodic(&mut out);
            uniform(&mut out, 0xa6, 4096);
            periodic(&mut out);
        }
        4 => {
            noise(&mut out, 0x2468_ace1, 131_072, 255);
            noise(&mut out, 0x2468_ace1, 131_072, 255);
        }
        5 => {
            periodic(&mut out);
            noise(&mut out, 0x7654_321f, 4096, 255);
            uniform(&mut out, 0x37, 4096);
            periodic(&mut out);
        }
        6 => {
            periodic(&mut out);
            uniform(&mut out, 0x99, 6);
            noise(&mut out, 0x1020_3041, 7, 255);
            uniform(&mut out, 0xee, 5);
            periodic(&mut out);
        }
        7 | 8 => {
            noise(&mut out, 0x5566_7789, 4096, 7);
            noise(&mut out, 0x1928_3745, 1024, 255);
            noise(&mut out, 0x89ab_cdef, 4096, 7);
        }
        9 | 10 => {
            noise(&mut out, 0x2468_ace1, 131_072, 255);
            noise(&mut out, 0xfedc_ba91, 131_072, 255);
            noise(&mut out, 0x2468_ace1, 131_072, 255);
        }
        11..=22 => {
            periodic(&mut out);
            noise(&mut out, 0xabcd_ef01, 1024, 255);
            let length = [4, 8, 12][(index - 11) / 4];
            let destination = [128, 192, 256, 320][(index - 11) % 4];
            out.copy_within(4096 + 32..4096 + 32 + length, 4096 + destination);
            periodic(&mut out);
        }
        _ => return Err("recipe index"),
    }
    if out.len() != size {
        return Err("source recipe length");
    }
    Ok(out)
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
    if !(4..=7).contains(&width) || !(6..=18).contains(&log) {
        return Err("hash shape");
    }
    let mut initialized = [0; 8];
    initialized[..width].copy_from_slice(bytes);
    let word = u64::from_le_bytes(initialized);
    if width == 4 {
        return Ok(((word as u32).wrapping_mul(2_654_435_761) >> (32 - log)) as usize);
    }
    let prime = [889_523_592_379, 227_718_039_650_203, 58_295_818_150_454_627][width - 5];
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
        (2, false) | (1, true) => Ok((b, [b, a, c])),
        (3, false) | (2, true) => Ok((c, [c, a, b])),
        (3, true) if a > 1 => Ok((a - 1, [a - 1, a, b])),
        _ => Err("invalid repeat selector"),
    }
}

impl<'a> OracleBlock<'a> {
    fn read(bytes: &'a [u8], buckets: usize) -> Result<Self> {
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
        let table_changes = r.records(buckets.min(length), 12, |r| {
            Ok([r.u32()?, r.u32()?, r.u32()?])
        })?;
        let table_hashes = [r.u64()?, r.u64()?];
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
            table_changes,
            table_hashes,
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
        if Some(&name) != NAMES.get(index) {
            return Err("exact ordered case identity");
        }
        let window_log = r.byte()?;
        let hash_log = r.byte()?;
        let min_match = r.byte()?;
        if !(10..=20).contains(&window_log)
            || !(6..=18).contains(&hash_log)
            || !(3..=7).contains(&min_match)
            || r.byte()? != 0
        {
            return Err("case parameters");
        }
        let target_length = r.u32()?;
        let length = r.size()?;
        let expected_hash = r.u64()?;
        if target_length > 131_072 || length > MAX_SOURCE {
            return Err("source/target cap");
        }
        let source = source(index)?;
        if source.len() != length || fingerprint(source.iter().copied()) != expected_hash {
            return Err("source recipe C fingerprint");
        }
        let schedule = r.records(70, 4, Reader::size)?;
        let blocks = r.records(64, 4, |r| {
            OracleBlock::read(r.blob(4 << 20)?, 1 << hash_log)
        })?;
        r.finish()?;
        let result = Self {
            name,
            window_log,
            hash_log,
            min_match,
            target_length,
            source,
            schedule,
            blocks,
        };
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<()> {
        if self.target_length > 131_072
            || self
                .schedule
                .iter()
                .any(|&n| n > MAX_BLOCK.min(1 << self.window_log))
            || self.blocks.len() != self.schedule.iter().filter(|&&n| n > 0).count()
        {
            return Err("source schedule shape");
        }
        let mut table = Vec::new();
        let buckets = 1 << self.hash_log;
        table.try_reserve_exact(buckets).map_err(|_| "allocation")?;
        while table.len() < buckets {
            table.push(0_u32);
        }
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
                || table_hash(&table) != block.table_hashes[0]
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
            } else if block.kind != 0 || !block.table_changes.is_empty() {
                return Err("tiny bypass changed candidate/table");
            }
            if block.wire_after != wire || block.parser_after != raw || raw[..2] != wire[..2] {
                return Err("selected publication state");
            }
            let mut last = None;
            for &[slot, old, new] in &block.table_changes {
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
                    || position < cursor
                    || bucket(&self.source[position..key_end], self.hash_log)? != slot
                {
                    return Err("ordered table delta/key");
                }
                table[slot] = new;
                last = Some(slot);
            }
            if table_hash(&table) != block.table_hashes[1] {
                return Err("table after fingerprint");
            }
            cursor = end;
        }
        if cursor != self.source.len() {
            return Err("complete source coverage");
        }
        Ok(())
    }
}

pub(crate) fn cases() -> Result<Vec<OracleFrameCase<'static>>> {
    decode(FIXTURE)
}

fn decode(bytes: &[u8]) -> Result<Vec<OracleFrameCase<'_>>> {
    if bytes.len() > 8 << 20 {
        return Err("trajectory file cap");
    }
    let mut r = Reader { bytes };
    if r.take(8)? != b"ZFTRJ01\0" || r.size()? != 23 || r.size()? != 64 {
        return Err("trajectory identity");
    }
    let mut cases = Vec::new();
    cases.try_reserve_exact(23).map_err(|_| "allocation")?;
    let mut blocks = 0;
    for index in 0..23 {
        let case = OracleFrameCase::read(r.blob(4 << 20)?, index)?;
        blocks += case.blocks.len();
        cases.push(case);
    }
    r.finish()?;
    if blocks != 64 {
        return Err("complete block count");
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_capture_replays_sources_candidates_and_selected_state() {
        let cases = cases().unwrap();
        assert_eq!(cases.len(), 23);
        assert_eq!(
            cases.iter().map(|c| c.source.len()).sum::<usize>(),
            1_222_674
        );
        let mut kinds = [0; 3];
        let mut tiny = 0;
        let mut changed_raw = 0;
        for case in &cases {
            for block in &case.blocks {
                kinds[usize::from(block.kind)] += 1;
                if let Some(candidate) = &block.candidate {
                    if block.kind == 0 && candidate.wire_repeats[..2] != block.wire_before[..2] {
                        changed_raw += 1;
                    }
                } else {
                    tiny += 1;
                }
            }
        }
        assert_eq!(kinds, [26, 2, 36]);
        assert_eq!(tiny, 2);
        assert_eq!(changed_raw, 10);
        let mixed = cases.iter().find(|c| c.name == "mixed_fallback").unwrap();
        assert_eq!(
            mixed.blocks.iter().map(|b| b.kind).collect::<Vec<_>>(),
            [2, 0, 1, 2]
        );
        assert_ne!(
            mixed.blocks[0].parser_after[2],
            mixed.blocks[0].wire_after[2]
        );
    }

    #[test]
    fn normalized_retained_floors_are_from_block_starts_not_ends() {
        let cases = cases().unwrap();
        for (name, floors) in [
            ("window_expiry_17", [0, 0, 131_072]),
            ("window_retained_19", [0, 0, 0]),
        ] {
            let case = cases.iter().find(|c| c.name == name).unwrap();
            assert_eq!(
                case.blocks
                    .iter()
                    .map(|b| b.prefix_floor)
                    .collect::<Vec<_>>(),
                floors
            );
        }
        let empty = cases
            .iter()
            .find(|c| c.name == "empty_then_first_uniform")
            .unwrap();
        assert_eq!(empty.schedule, [0, 4096]);
        assert_eq!(empty.blocks[0].call, 1);
        assert_eq!(empty.blocks[0].wire_before, [1, 4, 8]);
    }

    #[test]
    fn invalid_envelopes_counts_extents_and_recipe_fingerprints_are_rejected() {
        for size in [0, 7, 8, 15, 16, FIXTURE.len() - 1] {
            assert!(decode(&FIXTURE[..size]).is_err());
        }
        let mut altered = FIXTURE.to_vec();
        altered.push(0);
        assert!(decode(&altered).is_err());
        for (offset, bytes) in [
            (8, 22_u32.to_le_bytes()),
            (12, 63_u32.to_le_bytes()),
            (16, u32::MAX.to_le_bytes()),
        ] {
            let mut altered = FIXTURE.to_vec();
            altered[offset..offset + 4].copy_from_slice(&bytes);
            assert!(decode(&altered).is_err());
        }
        // First body: u16 name + five-byte "empty", four parameter bytes,
        // target/source u32 then the independent C source fingerprint.
        let hash = 20 + 2 + 5 + 4 + 8;
        let mut altered = FIXTURE.to_vec();
        altered[hash] ^= 1;
        assert!(decode(&altered).is_err());
        let mut tiny = Reader { bytes: &[255; 4] };
        assert!(tiny.records::<u8>(70, 4, |_| Ok(0)).is_err());
        let mut trailing = Reader { bytes: &[1] };
        assert!(trailing.take(2).is_err());
        assert_eq!(trailing.bytes, [1]);
    }

    #[test]
    fn stale_candidate_and_publication_and_table_mutations_are_rejected() {
        let mut cases = cases().unwrap();
        let candidate = cases[1].blocks[0].candidate.take().unwrap();
        let tiny = cases.iter_mut().find(|c| c.name == "tiny_stale").unwrap();
        assert!(tiny.blocks[1].candidate.is_none());
        tiny.blocks[1].kind = 1;
        assert!(tiny.validate().is_err());
        tiny.blocks[1].kind = 0;
        tiny.blocks[1].candidate = Some(candidate);
        assert!(tiny.validate().is_err());
        let mut cases = super::cases().unwrap();
        let mixed = cases
            .iter_mut()
            .find(|c| c.name == "mixed_fallback")
            .unwrap();
        mixed.blocks[1].wire_after[0] += 1;
        assert!(mixed.validate().is_err());
        mixed.blocks[1].wire_after[0] -= 1;
        mixed.blocks[0].table_changes[0][2] = u32::MAX;
        assert!(mixed.validate().is_err());
    }
}
