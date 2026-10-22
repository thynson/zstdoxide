//! Independent borrowed retained Ultra2 lifecycle reader; never a codec candidate provider.
#![allow(dead_code)]
use crate::ultra_path_fixture::{
    Event, check_pairs, frequency_state, opt, path_value, publish, shape, source_match,
};
pub const PACK: &[u8] = include_bytes!("../fixtures/reference-ultra2-retained/cases.bin");
#[derive(Debug)]
pub struct Reset {
    pub discarded_sequences: u32,
    pub discarded_literals: u32,
    pub discarded_tail: u32,
    pub caller_repeats: [u32; 3],
    pub discarded_repeats: [u32; 3],
    pub origin_before: u32,
    pub origin_after: u32,
    pub frequency: [u32; 386],
}
#[derive(Debug)]
pub struct Phase {
    pub header_offset: usize,
    pub footer_offset: usize,
    pub reset_offset: Option<usize>,
    pub seeding: bool,
    pub origin: u32,
    pub incoming: [u32; 3],
    pub candidate: [u32; 3],
    pub initial_frequency: [u32; 386],
    pub final_frequency: [u32; 386],
    pub tail: u32,
    pub events: Vec<Event>,
    pub search_changes: Vec<Vec<(usize, u32)>>,
    pub reset: Option<Reset>,
}
#[derive(Debug)]
pub struct Block {
    pub header_offset: usize,
    pub eligibility_offset: usize,
    pub start: u32,
    pub length: usize,
    pub kind: u32,
    pub previous_before: usize,
    pub previous_after: usize,
    pub pre_frontier: u32,
    pub incoming: [u32; 3],
    pub candidate: [u32; 3],
    pub owners: [[u32; 3]; 2],
    pub tail: u32,
    pub sequence_count: usize,
    pub eligibility: [u32; 15],
    pub phases: Vec<Phase>,
}
#[derive(Debug)]
pub struct Case<'a> {
    pub encoded: &'a [u8],
    pub name: &'a str,
    pub source: &'a [u8],
    pub frame: &'a [u8],
    pub window: u8,
    pub hash: u8,
    pub chain: u8,
    pub search: u8,
    pub minimum: u32,
    pub target: u32,
    pub literal_mode: u32,
    pub allocations: [[u32; 3]; 2],
    pub blocks: Vec<Block>,
}
impl Case<'_> {
    pub fn short_cells(&self) -> usize {
        if self.minimum == 3 {
            1 << self.window
        } else {
            0
        }
    }
    pub fn search_cells(&self) -> usize {
        (1 << self.hash) + (1 << self.chain) + self.short_cells() + 2
    }
}
const INFINITY: u32 = 1 << 30;
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
fn need(ok: bool, why: &'static str) -> Result<(), &'static str> {
    if ok { Ok(()) } else { Err(why) }
}
fn reserved<T>(count: usize, maximum: usize) -> Result<Vec<T>, &'static str> {
    need(count <= maximum, "bounded allocation count")?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| "allocation failed")?;
    Ok(result)
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], &'static str> {
        let end = self.offset.checked_add(n).ok_or("extent overflow")?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated fixture")?;
        self.offset = end;
        Ok(bytes)
    }
    fn ensure(&self, count: usize, unit: usize) -> Result<(), &'static str> {
        let n = count.checked_mul(unit).ok_or("array extent overflow")?;
        need(
            n <= self.bytes.len() - self.offset,
            "complete array before allocation",
        )
    }
    fn word(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| "word")?,
        ))
    }
    fn words<const N: usize>(&mut self) -> Result<[u32; N], &'static str> {
        self.ensure(N, 4)?;
        let mut words = [0; N];
        for word in &mut words {
            *word = self.word()?;
        }
        Ok(words)
    }
    fn optional<const N: usize>(&mut self) -> Result<Option<[u32; N]>, &'static str> {
        match self.word()? {
            0 => Ok(None),
            1 => self.words().map(Some),
            _ => Err("optional flag"),
        }
    }
    fn alternative(&mut self) -> Result<Option<[i64; 12]>, &'static str> {
        match self.word()? {
            0 => Ok(None),
            1 => {
                self.ensure(12, 8)?;
                let mut values = [0; 12];
                for value in &mut values {
                    *value =
                        i64::from_le_bytes(self.take(8)?.try_into().map_err(|_| "signed word")?);
                }
                Ok(Some(values))
            }
            _ => Err("alternative presence flag"),
        }
    }
    fn matches(&mut self, maximum: usize) -> Result<Vec<[u32; 2]>, &'static str> {
        let count = self.word()? as usize;
        self.ensure(count, 8)?;
        let mut result = reserved(count, maximum)?;
        for _ in 0..count {
            result.push(self.words()?);
        }
        Ok(result)
    }
}
const LL_START: [u32; 36] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24, 28, 32, 40, 48, 64,
    128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536,
];
const LL_BITS: [u32; 36] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11,
    12, 13, 14, 15, 16,
];
fn weight(raw: u32, policy: u32) -> Result<u32, &'static str> {
    need(
        raw <= if policy == 2 {
            8_388_606
        } else {
            (1 << 24) - 1
        },
        "bounded frequency weight",
    )?;
    let adjusted = u64::from(raw) + 1;
    let exponent = adjusted.ilog2();
    Ok(exponent * 256
        + if policy == 2 {
            (adjusted * 256 / (1u64 << exponent)) as u32
        } else {
            0
        })
}
fn ll_cost(length: u32, frequencies: &[u32; 386], policy: u32) -> Result<i64, &'static str> {
    need(length <= 131072, "literal price length")?;
    if length == 131072 {
        return Ok(256 + ll_cost(length - 1, frequencies, policy)?);
    }
    let code = LL_START
        .partition_point(|&base| base <= length)
        .checked_sub(1)
        .ok_or("literal length code")?;
    let cost = frequencies[382]
        .checked_sub(weight(frequencies[256 + code], policy)?)
        .ok_or("LL frequency exceeds cache")?;
    Ok(i64::from(LL_BITS[code] * 256 + cost))
}

// Independent v1.5.7 ZSTD_rescaleFreqs distributions and integer scaling.
// Pinned constants are attributed in NOTICE and LICENSE.zstd.
fn begun_frequency(
    mut values: [u32; 386],
    source: &[u8],
    mode: u32,
) -> Result<[u32; 386], &'static str> {
    values[385] = 0;
    if values[378] == 0 {
        values[385] = u32::from(source.len() <= 8);
        if mode == 0 {
            let mut histogram = [0u32; 256];
            for &byte in source {
                histogram[usize::from(byte)] += 1;
            }
            for (value, count) in values[..256].iter_mut().zip(histogram) {
                *value = u32::from(count > 0) + (count >> 8);
            }
        }
        values[256..377].fill(1);
        values[256] = 4;
        values[257] = 2;
        values[345..357].copy_from_slice(&[6, 2, 1, 1, 2, 3, 4, 4, 4, 3, 2, 1]);
    } else {
        let mut start = 0;
        for (table, count) in [256, 36, 53, 32].into_iter().enumerate() {
            if table != 0 || mode == 0 {
                let factor = values[377 + table] >> if table == 0 { 12 } else { 11 };
                if factor > 1 {
                    let shift = factor.ilog2();
                    need(shift < 30, "bounded retained frequency scaling")?;
                    for value in &mut values[start..start + count] {
                        *value = 1 + (*value >> shift);
                    }
                }
            }
            start += count;
        }
    }
    let mut start = 0;
    for (table, count) in [256, 36, 53, 32].into_iter().enumerate() {
        let sum = values[start..start + count]
            .iter()
            .try_fold(0u32, |sum, &v| sum.checked_add(v))
            .ok_or("frequency sum overflow")?;
        values[377 + table] = sum;
        values[381 + table] = if table == 0 && mode == 1 {
            0
        } else {
            weight(sum, 2)?
        };
        start += count;
    }
    Ok(values)
}

fn frame_extent(case: &Case<'_>) -> Result<(), &'static str> {
    let mut r = Reader {
        bytes: case.frame,
        offset: 0,
    };
    need(r.take(4)? == [0x28, 0xb5, 0x2f, 0xfd], "frame magic")?;
    let n = case.source.len();
    let flag = r.take(1)?[0];
    if n < 256 {
        need(
            flag == 0x20 && r.take(1)?[0] as usize == n,
            "small content size",
        )?;
    } else {
        let size =
            u16::from_le_bytes(r.take(2)?.try_into().map_err(|_| "content size")?) as usize + 256;
        need(flag == 0x60 && size == n, "large content size")?;
    }
    for block in &case.blocks {
        let h = r.take(3)?;
        let word = u32::from_le_bytes([h[0], h[1], h[2], 0]);
        let size = (word >> 3) as usize;
        need(
            word & 1 == 0 && (word >> 1) & 3 == block.kind,
            "scheduled wire publication",
        )?;
        let payload = r.take(if block.kind == 1 { 1 } else { size })?;
        let source = &case.source[block.start as usize..block.start as usize + block.length];
        match block.kind {
            0 => need(payload == source, "raw source")?,
            1 => need(
                size == block.length && source.iter().all(|&b| b == payload[0]),
                "RLE source",
            )?,
            2 => need(
                size > 0 && size < block.length,
                "beneficial compressed extent",
            )?,
            _ => return Err("block kind"),
        }
    }
    need(
        r.take(3)? == [1, 0, 0] && r.offset == case.frame.len(),
        "empty final raw block and EOF",
    )
}

pub fn read(bytes: &[u8]) -> Result<Vec<Case<'_>>, &'static str> {
    need(bytes.len() <= 128 << 20, "bounded retained lifecycle pack")?;
    let mut r = Reader { bytes, offset: 0 };
    need(r.take(8)? == b"ZU2RFX01", "retained lifecycle magic")?;
    let count = r.word()? as usize;
    need((1..=32).contains(&count), "bounded nonempty cases")?;
    r.ensure(count, 80)?;
    let mut cases = reserved(count, 32)?;
    let mut total_events = 0usize;
    for _ in 0..count {
        let case_start = r.offset;
        let h = r.words::<14>()?;
        let (name_len, source_len, block_count, frame_len) =
            (h[0] as usize, h[1] as usize, h[9] as usize, h[10] as usize);
        need(
            (1..=128).contains(&name_len)
                && (14..=32768).contains(&source_len)
                && (10..=16).contains(&h[2])
                && (6..=12).contains(&h[3])
                && (6..=12).contains(&h[4])
                && (1..=6).contains(&h[5])
                && (3..=7).contains(&h[6])
                && h[7] <= 4095
                && h[8] <= 1
                && (2..=4).contains(&block_count)
                && (10..=32832).contains(&frame_len)
                && h[11] == 4099
                && h[12] == 1
                && h[13] == 2,
            "case domain",
        )?;
        need(
            source_len <= 1usize << h[2],
            "source within admitted window",
        )?;
        let allocations = [r.words::<3>()?, r.words::<3>()?];
        for [context, workspace, peak] in allocations {
            need(
                context > 0
                    && workspace > 0
                    && peak <= 8 << 20
                    && context.checked_add(workspace) == Some(peak),
                "allocation high water",
            )?;
        }
        need(
            allocations[0][0] == allocations[1][0]
                && allocations[0][1].checked_add(3584) == Some(allocations[1][1]),
            "sanitizer redzones",
        )?;
        let name = core::str::from_utf8(r.take(name_len)?).map_err(|_| "ASCII name")?;
        need(
            name.is_ascii() && cases.iter().all(|c: &Case<'_>| c.name != name),
            "unique ASCII name",
        )?;
        let source = r.take(source_len)?;
        let frame = r.take(frame_len)?;
        let mut case = Case {
            encoded: &[],
            name,
            source,
            frame,
            window: h[2] as u8,
            hash: h[3] as u8,
            chain: h[4] as u8,
            search: h[5] as u8,
            minimum: h[6],
            target: h[7],
            literal_mode: h[8],
            allocations,
            blocks: reserved(block_count, 4)?,
        };
        let minimum = if h[6] == 3 { 3 } else { 4 };
        let path_cells = 4099;
        let mut path = reserved(path_cells, 4099)?;
        path.resize(path_cells, [0; 7]);
        let mut frequencies = [0; 386];
        let search_size = case.search_cells();
        let mut search = reserved(search_size, 73730)?;
        search.resize(search_size, 0);
        let mut owners = [[1, 4, 8], [0, 0, 0]];
        let mut previous = 0usize;
        let mut consumed = 0usize;
        let mut first_length = 0usize;
        let mut seeded = false;
        for block_index in 0..block_count {
            let block_header_offset = r.offset - case_start;
            let b = r.words::<21>()?;
            let block_start = b[0];
            let length = b[1] as usize;
            let block_end = consumed.checked_add(length).ok_or("block end overflow")?;
            let phase_count = b[6] as usize;
            need(
                block_start as usize == consumed
                    && (7..=8192).contains(&length)
                    && block_end <= source_len
                    && b[2] <= 2
                    && b[3] as usize == previous
                    && b[4] <= 1,
                "block domain/order",
            )?;
            if block_index == 0 {
                first_length = length;
                seeded = length > 8;
            }
            let initial_origin = if seeded && block_index > 0 {
                first_length as u32 + 2
            } else {
                2
            };
            let current = initial_origin + block_start;
            let pre_frontier = if block_index == 0 {
                2
            } else {
                search[search_size - 2]
            };
            need(b[5] == pre_frontier, "complete retained entry frontier")?;
            let entry_frontier = if current > pre_frontier + 384 {
                current - 192.min(current - pre_frontier - 384)
            } else {
                pre_frontier
            };
            let incoming = [b[7], b[8], b[9]];
            need(
                incoming == owners[previous]
                    && phase_count == if seeded && block_index == 0 { 2 } else { 1 },
                "phase count and caller owner",
            )?;
            let eligibility_offset = r.offset - case_start;
            let eligibility = r.words::<15>()?;
            let gate = block_index == 0 && seeded;
            need(
                eligibility
                    == [
                        u32::from(frequencies[378] == 0),
                        1,
                        1,
                        u32::from(current == initial_origin),
                        u32::from(length > 8),
                        u32::from(gate),
                        current,
                        initial_origin,
                        initial_origin,
                        entry_frontier,
                        length as u32,
                        incoming[0],
                        incoming[1],
                        incoming[2],
                        0,
                    ],
                "independent block eligibility",
            )?;
            let mut block = Block {
                header_offset: block_header_offset,
                eligibility_offset,
                start: block_start,
                length,
                kind: b[2],
                previous_before: previous,
                previous_after: b[4] as usize,
                pre_frontier,
                incoming,
                candidate: [b[10], b[11], b[12]],
                owners: [[b[13], b[14], b[15]], [b[16], b[17], b[18]]],
                tail: b[19],
                sequence_count: b[20] as usize,
                eligibility,
                phases: reserved(phase_count, 2)?,
            };
            let prefix = &source[..block_end];
            for phase_index in 0..phase_count {
                let header_offset = r.offset - case_start;
                let p = r.words::<6>()?;
                let seeding = gate && phase_index == 0;
                let origin = if seeded && (block_index > 0 || phase_index > 0) {
                    first_length as u32 + 2
                } else {
                    2
                };
                let phase_frontier = if phase_index > 0 {
                    origin
                } else {
                    entry_frontier
                };
                let incoming = [p[2], p[3], p[4]];
                let event_count = p[5] as usize;
                need(
                    p[0] == u32::from(!seeding)
                        && p[1] == origin
                        && incoming == block.incoming
                        && (2..=32768).contains(&event_count),
                    "phase origin/order/caller repeats",
                )?;
                let initial_frequency = r.words::<386>()?;
                need(
                    initial_frequency == frequencies,
                    "preserved pre-rescale frequencies",
                )?;
                let expected_begin =
                    begun_frequency(initial_frequency, &source[consumed..block_end], h[8])?;
                let mut wire_repeats = incoming;
                let mut wire_end = block_start;
                let mut live_sequences = 0;
                r.ensure(event_count, 64)?;
                total_events = total_events
                    .checked_add(event_count)
                    .ok_or("event count overflow")?;
                need(total_events <= 229657, "bounded full event inventory")?;
                let mut pending: Option<(usize, u32, [u32; 7], bool)> = None;
                let mut events = reserved(event_count, 32768)?;
                let mut search_changes = reserved(event_count, 32768)?;
                for index in 0..event_count {
                    let a = r.words::<10>()?;
                    need(
                        a[1] <= 1
                            && a[3] >= block_start
                            && a[3] <= a[2]
                            && a[2] <= block_end as u32,
                        "phase/source coordinates",
                    )?;
                    let stretch = r.optional::<7>()?;
                    let reverse = r.optional::<3>()?;
                    let query = r.optional::<6>()?;
                    let matches = if query.is_some() {
                        r.matches(68)?
                    } else {
                        Vec::new()
                    };
                    let alternative = r.alternative()?;
                    if let Some(values) = alternative {
                        need(a[0] == 12, "fractional alternative stage")?;
                        let cur = usize::try_from(a[4]).map_err(|_| "alternative cursor")?;
                        let mut saved = [0u32; 7];
                        for (destination, &value) in saved.iter_mut().zip(&values[..7]) {
                            *destination = u32::try_from(value).map_err(|_| "saved cell word")?;
                        }
                        need(
                            path.get(cur) == Some(&saved),
                            "complete saved match before overwrite",
                        )?;
                        path_value(saved, source_len)?;
                        need(
                            saved[0] < INFINITY && saved[2] >= minimum && saved[3] == 0,
                            "saved finite match",
                        )?;
                    }
                    let prior_bases = [
                        frequencies[381],
                        frequencies[382],
                        frequencies[383],
                        frequencies[384],
                    ];
                    let n = r.word()? as usize;
                    r.ensure(n, 32)?;
                    let mut changes = reserved(n, path_cells)?;
                    let mut previous = None;
                    for _ in 0..n {
                        let at = r.word()? as usize;
                        let value = r.words::<7>()?;
                        need(
                            at < path_cells && previous.is_none_or(|p| p < at),
                            "canonical path index",
                        )?;
                        path_value(value, source_len)?;
                        need(path[at] != value, "changed path cell only")?;
                        path[at] = value;
                        changes.push((at, value));
                        previous = Some(at);
                    }
                    let n = r.word()? as usize;
                    r.ensure(n, 8)?;
                    let mut freq_changes = reserved(n, 386)?;
                    let mut previous = None;
                    for _ in 0..n {
                        let at = r.word()? as usize;
                        let value = r.word()?;
                        need(
                            at < 386 && previous.is_none_or(|p| p < at),
                            "canonical frequency index",
                        )?;
                        need(frequencies[at] != value, "changed frequency cell only")?;
                        frequencies[at] = value;
                        freq_changes.push((at, value));
                        previous = Some(at);
                    }
                    if index == 0 {
                        need(
                            frequencies == expected_begin,
                            "exact independently rescaled begin frequencies",
                        )?;
                    }
                    let sequence_count = r.word()? as usize;
                    let n = r.word()? as usize;
                    r.ensure(n, 16)?;
                    need(
                        n == usize::from(a[0] == 9)
                            && sequence_count == live_sequences + n
                            && sequence_count <= length / 3 + 1,
                        "append-only sequence count",
                    )?;
                    let mut appended = reserved(n, 1)?;
                    for _ in 0..n {
                        let row = r.words::<4>()?;
                        need(
                            row[0] == wire_end && row[2] >= minimum,
                            "contiguous sequence/literal source",
                        )?;
                        let position = row[0]
                            .checked_add(row[1])
                            .ok_or("literal extent overflow")?;
                        source_match(prefix, position, row[3], row[2], wire_repeats, row[1] == 0)?;
                        wire_repeats = publish(row[3], wire_repeats, row[1] == 0)?;
                        wire_end = position
                            .checked_add(row[2])
                            .ok_or("sequence end overflow")?;
                        need(wire_end == a[3], "published anchor after sequence")?;
                        appended.push(row);
                    }
                    live_sequences = sequence_count;
                    let event = Event {
                        stage: a[0],
                        reverse_phase: a[1] == 1,
                        ip: a[2],
                        anchor: a[3],
                        cur: opt(a[4]),
                        last: opt(a[5]),
                        tail: opt(a[6]),
                        repeats: [a[7], a[8], a[9]],
                        stretch,
                        reverse,
                        query,
                        matches,
                        alternative,
                        path: changes,
                        frequencies: freq_changes,
                        sequence_count,
                        appended,
                    };
                    shape(&event, index, event_count)?;
                    need(
                        event
                            .repeats
                            .iter()
                            .all(|&v| v > 0 && v <= block_end.max(8) as u32),
                        "published repeat values",
                    )?;
                    need(
                        event.last.is_none_or(|v| v <= block_end as u32),
                        "bounded path end",
                    )?;
                    if !event.reverse_phase || event.stage == 10 {
                        need(
                            event.repeats == wire_repeats,
                            "committed repeat publication",
                        )?;
                    }
                    if let Some(q) = event.query {
                        need(
                            q[0] >= block_start.max(1)
                                && q[0]
                                    .checked_add(8)
                                    .is_some_and(|end| end <= block_end as u32)
                                && q[4] <= 1
                                && q[5] == minimum,
                            "query footprint/flags",
                        )?;
                        need(
                            event
                                .ip
                                .checked_add(event.cur.ok_or("query relative coordinate")?)
                                == Some(q[0]),
                            "query absolute coordinate",
                        )?;
                        need(
                            q[1..4]
                                .iter()
                                .all(|&v| v > 0 && v <= block_end.max(8) as u32),
                            "query repeat values",
                        )?;
                        check_pairs(
                            prefix,
                            q[0],
                            &event.matches,
                            minimum,
                            Some(([q[1], q[2], q[3]], q[4] == 1)),
                        )?;
                    }
                    if let Some(stretch) = event.stretch {
                        path_value(stretch, source_len)?;
                        if event.stage == 6 {
                            need(
                                event.last.and_then(|v| v.checked_sub(stretch[2])) == event.cur,
                                "selected predecessor coordinate",
                            )?;
                        }
                    }
                    if let Some([start, end, position]) = event.reverse {
                        need(
                            start <= end
                                && (end as usize) < path_cells
                                && (position as usize) < path_cells,
                            "reconstruction extent",
                        )?;
                        if event.stage == 7 {
                            need(
                                start == end
                                    && event.cur == Some(position)
                                    && position.checked_add(2) == Some(end),
                                "initial reconstruction destination",
                            )?;
                        }
                        if event.stage == 8 {
                            need(
                                start > position
                                    && path[start as usize][3] == path[position as usize][3],
                                "preserved reverse predecessor",
                            )?;
                            let old = path[position as usize];
                            if old[2] > 0 {
                                need(
                                    old[2].checked_add(old[3]).is_some_and(|v| v <= position),
                                    "strict preceding stretch",
                                )?;
                            }
                        }
                        if event.stage == 9 {
                            need(start <= position && position <= end, "emitted reverse slot")?;
                        }
                    }
                    need(
                        frequencies[385] == expected_begin[385],
                        "exact predefined threshold",
                    )?;
                    let mut checked_frequencies = frequencies;
                    checked_frequencies[385] = 0;
                    frequency_state(&checked_frequencies, h[8], 2, matches!(event.stage, 0 | 10))?;
                    if !matches!(event.stage, 0 | 10) {
                        need(
                            frequencies[381..385] == prior_bases,
                            "retained dirty caches",
                        )?;
                    }
                    if let Some((at, frontier, expected, accepted)) = pending.take() {
                        need(
                            event.stage == 4
                                && event.cur == Some((at - 1) as u32)
                                && event.last == Some(frontier),
                            "alternative publication phase/frontier",
                        )?;
                        if accepted {
                            need(
                                event.path.as_slice() == [(at, expected)] && path[at] == expected,
                                "exact accepted alternative publication",
                            )?;
                        } else {
                            need(event.path.is_empty(), "rejected alternative preserves path")?;
                        }
                    }
                    if let Some(values) = event.alternative {
                        let cur = event.cur.ok_or("alternative cursor")? as usize;
                        let last = event.last.ok_or("alternative frontier")?;
                        let current = *path.get(cur).ok_or("alternative current cell")?;
                        let next = *path.get(cur + 1).ok_or("alternative next cell")?;
                        let mut saved = [0u32; 7];
                        for (destination, &value) in saved.iter_mut().zip(&values[..7]) {
                            *destination = u32::try_from(value).map_err(|_| "saved cell")?;
                        }
                        let predecessor = cur
                            .checked_sub(saved[2] as usize)
                            .ok_or("alternative predecessor subtraction")?;
                        need(
                            values[11] == predecessor as i64 && current[3] > 0,
                            "alternative predecessor/literal state",
                        )?;
                        let before = *path
                            .get(predecessor)
                            .ok_or("alternative predecessor cell")?;
                        let at = event
                            .ip
                            .checked_add(cur as u32)
                            .ok_or("alternative source overflow")?;
                        need(
                            at < block_end as u32,
                            "alternative literal within consumed prefix",
                        )?;
                        let byte = *prefix
                            .get(at as usize)
                            .ok_or("alternative literal source")?;
                        let literal = if h[8] == 1 {
                            2048
                        } else {
                            frequencies[381]
                                .checked_sub(weight(frequencies[usize::from(byte)], 2)?)
                                .ok_or("literal frequency exceeds cache")?
                                .max(256)
                        };
                        let increment = ll_cost(1, &frequencies, 2)? - ll_cost(0, &frequencies, 2)?;
                        need(
                            increment < 0 && values[7] == increment,
                            "negative LL increment",
                        )?;
                        let one = i64::from(saved[0]) + i64::from(literal) + increment;
                        let more = i64::from(current[0])
                            + i64::from(literal)
                            + ll_cost(
                                current[3].checked_add(1).ok_or("literal increment")?,
                                &frequencies,
                                2,
                            )?
                            - ll_cost(current[3], &frequencies, 2)?;
                        need(
                            (0..i64::from(INFINITY)).contains(&one)
                                && (0..i64::from(INFINITY)).contains(&more)
                                && values[8] == one
                                && values[9] == more
                                && values[10] == i64::from(next[0]),
                            "complete alternative candidate prices",
                        )?;
                        let accepted = one < more && one < i64::from(next[0]);
                        let mut expected = saved;
                        if accepted {
                            expected[4..].copy_from_slice(&publish(
                                saved[1],
                                [before[4], before[5], before[6]],
                                before[3] == 0,
                            )?);
                            expected[0] = one as u32;
                            expected[3] = 1;
                        }
                        pending = Some((
                            cur + 1,
                            if accepted {
                                last.max(cur as u32 + 1)
                            } else {
                                last
                            },
                            expected,
                            accepted,
                        ));
                    }

                    if let Some(tail) = event.tail {
                        need(
                            tail == block_end as u32 - wire_end && event.anchor == wire_end,
                            "complete trailing literals",
                        )?;
                    }
                    if index == 0 {
                        need(
                            event.ip == block_start.max(1)
                                && event.anchor == block_start
                                && event.repeats == incoming
                                && event.path.is_empty(),
                            "block entry state",
                        )?;
                    }
                    let n = r.word()? as usize;
                    r.ensure(n, 8)?;
                    need(
                        index == 0 || event.query.is_some() || n == 0,
                        "only queries change search",
                    )?;
                    let mut changes = reserved(n, search_size)?;
                    let mut previous = None;
                    let old_frontier = search[search_size - 2];
                    for _ in 0..n {
                        let [at, value] = r.words()?;
                        let at = at as usize;
                        need(
                            at < search_size && previous.is_none_or(|p| p < at),
                            "canonical search index",
                        )?;
                        need(
                            search[at] != value
                                && (value == 0 || (2..=origin + block_end as u32).contains(&value)),
                            "changed biased search index",
                        )?;
                        need(
                            index != 0 || at >= search_size - 2,
                            "entry preserves search maps",
                        )?;
                        search[at] = value;
                        previous = Some(at);
                        changes.push((at, value));
                    }
                    need(
                        search[search_size - 2..].iter().all(|&v| v >= 2),
                        "initialized frontiers",
                    )?;
                    if index == 0 {
                        need(
                            search[search_size - 2..] == [phase_frontier, phase_frontier],
                            "exact reset frontiers",
                        )?;
                        need(
                            (block_index == 0 && phase_index == 0 && old_frontier == 0)
                                || phase_index == 1
                                || block_index > 0,
                            "initial or retained frontier",
                        )?;
                    }
                    search_changes.push(changes);
                    events.push(event);
                }
                let footer_offset = r.offset - case_start;
                let footer = r.words::<5>()?;
                let candidate = [footer[0], footer[1], footer[2]];
                let tail = footer[4];
                need(
                    candidate == wire_repeats
                        && footer[3] as usize == live_sequences
                        && wire_end <= block_end as u32
                        && tail == block_end as u32 - wire_end
                        && pending.is_none(),
                    "complete phase footer",
                )?;
                let final_frequency = r.words::<386>()?;
                need(
                    final_frequency == frequencies,
                    "complete final frequency state",
                )?;
                need(
                    r.word()? == u32::from(seeding),
                    "exact discarded-pass reset presence",
                )?;
                let reset_offset = seeding.then_some(r.offset - case_start);
                let reset = if seeding {
                    let value = r.words::<15>()?;
                    let discarded_literals = events
                        .iter()
                        .flat_map(|e| &e.appended)
                        .try_fold(0u32, |sum, row| sum.checked_add(row[1]))
                        .ok_or("literal count overflow")?;
                    let next_origin = origin
                        .checked_add(length as u32)
                        .ok_or("coordinate overflow")?;
                    need(
                        value
                            == [
                                live_sequences as u32,
                                discarded_literals,
                                tail,
                                1,
                                4,
                                8,
                                candidate[0],
                                candidate[1],
                                candidate[2],
                                origin,
                                next_origin,
                                next_origin,
                                next_origin,
                                next_origin,
                                1,
                            ],
                        "independent reset/count/repeat/coordinate state",
                    )?;
                    let frequency = r.words::<386>()?;
                    need(frequency == frequencies, "reset retains frequencies")?;
                    Some(Reset {
                        discarded_sequences: value[0],
                        discarded_literals,
                        discarded_tail: tail,
                        caller_repeats: incoming,
                        discarded_repeats: candidate,
                        origin_before: origin,
                        origin_after: next_origin,
                        frequency,
                    })
                } else {
                    None
                };
                block.phases.push(Phase {
                    header_offset,
                    footer_offset,
                    reset_offset,
                    seeding,
                    origin,
                    incoming,
                    candidate,
                    initial_frequency,
                    final_frequency,
                    tail,
                    events,
                    search_changes,
                    reset,
                });
            }

            let actual = block.phases.last().ok_or("encoding phase")?;
            need(
                !actual.seeding
                    && actual.candidate == block.candidate
                    && actual.tail == block.tail
                    && actual.events.last().ok_or("terminal event")?.sequence_count
                        == block.sequence_count,
                "actual phase alone reaches publication",
            )?;
            let pending_owner = 1 - previous;
            owners[pending_owner] = block.candidate;
            if block.kind == 2 {
                previous = pending_owner;
            }
            need(
                block.owners == owners && block.previous_after == previous,
                "compressed commit or raw/RLE repeat rollback",
            )?;
            consumed = block_end;
            case.blocks.push(block);
        }
        need(consumed == source_len, "complete source schedule")?;
        frame_extent(&case)?;
        case.encoded = &bytes[case_start..r.offset];
        cases.push(case);
    }
    need(r.offset == bytes.len(), "exact retained lifecycle EOF")?;
    Ok(cases)
}
