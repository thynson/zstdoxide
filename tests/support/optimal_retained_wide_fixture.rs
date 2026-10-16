//! Independent bounded retained observation reader; never a candidate provider.
#![allow(dead_code)]
use crate::optimal_path_fixture::{
    Event, check_pairs, frequency_state, opt, path_value, publish, shape, source_match,
};
pub const PACK: &[u8] = include_bytes!("../fixtures/reference-optimal-retained-wide/cases.bin");
#[derive(Debug)]
pub struct Block {
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
    pub events: Vec<Event>,
    pub search_changes: Vec<Vec<(usize, u32)>>,
}
#[derive(Debug)]
pub struct Case<'a> {
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
    need(bytes.len() <= 16 << 20, "bounded retained pack")?;
    let mut r = Reader { bytes, offset: 0 };
    need(r.take(8)? == b"ZORFW02\0", "retained magic")?;
    let count = r.word()? as usize;
    need((1..=7).contains(&count), "bounded nonempty cases")?;
    r.ensure(count, 80)?;
    let mut cases = reserved(count, 7)?;
    for _ in 0..count {
        let h = r.words::<12>()?;
        let (name_len, source_len, block_count) = (h[0] as usize, h[1] as usize, h[9] as usize);
        need(
            (1..=128).contains(&name_len)
                && (128..=16384).contains(&source_len)
                && (10..=16).contains(&h[2])
                && (6..=12).contains(&h[3])
                && (6..=12).contains(&h[4])
                && (1..=6).contains(&h[5])
                && (3..=7).contains(&h[6])
                && h[7] <= 4095
                && h[8] <= 1
                && (2..=4).contains(&block_count)
                && (12..=32768).contains(&h[10])
                && h[11] == 4099,
            "retained case domain",
        )?;
        need(
            source_len <= 1_usize << h[2],
            "complete source inside window",
        )?;
        let allocations = [r.words::<3>()?, r.words::<3>()?];
        for [context, workspace, peak] in allocations {
            need(
                context > 0
                    && workspace > 0
                    && peak <= 8 << 20
                    && context.checked_add(workspace) == Some(peak),
                "exact two-owner high water",
            )?;
        }
        need(
            allocations[0][0] == allocations[1][0]
                && allocations[0][1].checked_add(3584) == Some(allocations[1][1]),
            "sanitizer redzones",
        )?;
        let name = core::str::from_utf8(r.take(name_len)?).map_err(|_| "case name")?;
        need(
            name.is_ascii() && cases.iter().all(|c: &Case<'_>| c.name != name),
            "unique ASCII name",
        )?;
        let source = r.take(source_len)?;
        let frame = r.take(h[10] as usize)?;
        let mut case = Case {
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
        let mut previous_owner = 0;
        let mut source_end = 0;
        for block_index in 0..block_count {
            let b = r.words::<20>()?;
            let (block_start, length, kind) = (b[0], b[1] as usize, b[2]);
            let (previous_before, previous_after, pre_frontier, event_count) =
                (b[3] as usize, b[4] as usize, b[5], b[6] as usize);
            let incoming = [b[7], b[8], b[9]];
            let candidate = [b[10], b[11], b[12]];
            let published = [[b[13], b[14], b[15]], [b[16], b[17], b[18]]];
            let tail = b[19];
            need(
                block_start as usize == source_end && (64..=8192).contains(&length),
                "contiguous bounded schedule",
            )?;
            let block_end = source_end.checked_add(length).ok_or("block end overflow")?;
            need(
                block_end <= source_len
                    && kind <= 2
                    && previous_before == previous_owner
                    && previous_after
                        == if kind == 2 {
                            1 - previous_owner
                        } else {
                            previous_owner
                        }
                    && incoming == owners[previous_owner]
                    && (2..=source_len as u32 + 2).contains(&pre_frontier)
                    && (2..=32768).contains(&event_count)
                    && tail <= length as u32,
                "block publication domain",
            )?;
            need(
                candidate.iter().all(|&v| v > 0 && v <= source_len as u32),
                "candidate repeats",
            )?;
            owners[1 - previous_owner] = candidate;
            need(published == owners, "pending/committed owner contents")?;
            let prefix = &source[..block_end];
            let mut wire_repeats = incoming;
            let mut wire_end = block_start;
            let mut live_sequences = 0;
            r.ensure(event_count, 64)?;
            let mut events = reserved(event_count, 32768)?;
            let mut search_changes = reserved(event_count, 32768)?;
            for index in 0..event_count {
                let a = r.words::<10>()?;
                need(
                    a[1] <= 1 && a[3] >= block_start && a[3] <= a[2] && a[2] <= block_end as u32,
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
                        .all(|&v| v > 0 && v <= source_len.max(8) as u32),
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
                            && q[0] <= block_end as u32 - 8
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
                            .all(|&v| v > 0 && v <= source_len.max(8) as u32),
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
                frequency_state(&frequencies, h[8], matches!(event.stage, 0 | 10))?;
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
                            && (value == 0 || (2..=block_end as u32 + 2).contains(&value)),
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
                        old_frontier == pre_frontier
                            || (block_index == 0 && old_frontier == 0 && pre_frontier == 2),
                        "pre-call frontier",
                    )?;
                    let current = block_start + 2;
                    let adjusted = if current > pre_frontier + 384 {
                        current - 192.min(current - pre_frontier - 384)
                    } else {
                        pre_frontier
                    };
                    need(
                        search[search_size - 2..] == [adjusted, adjusted],
                        "strict entry catch-up and local short reset",
                    )?;
                }
                search_changes.push(changes);
                events.push(event);
            }
            need(
                wire_repeats == candidate && tail == block_end as u32 - wire_end,
                "candidate publication and trailing literals",
            )?;
            case.blocks.push(Block {
                start: block_start,
                length,
                kind,
                previous_before,
                previous_after,
                pre_frontier,
                incoming,
                candidate,
                owners: published,
                tail,
                events,
                search_changes,
            });
            previous_owner = previous_after;
            source_end = block_end;
        }
        need(source_end == source_len, "complete source schedule")?;
        frame_extent(&case)?;
        cases.push(case);
    }
    need(r.offset == bytes.len(), "exact retained pack EOF")?;
    Ok(cases)
}
