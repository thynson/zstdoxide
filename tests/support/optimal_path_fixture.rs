//! Independent bounded reader for complete captured Optimal path/frequency deltas.
#![allow(dead_code)]
pub const PACK: &[u8] = include_bytes!("../fixtures/reference-optimal-paths/cases.bin");
const NONE: u32 = u32::MAX;
const INFINITY: u32 = 1 << 30;
#[derive(Debug)]
pub struct Script {
    pub position: u32,
    pub matches: Vec<[u32; 2]>,
}
#[derive(Debug)]
pub struct Event {
    pub stage: u32,
    pub reverse_phase: bool,
    pub ip: u32,
    pub anchor: u32,
    pub cur: Option<u32>,
    pub last: Option<u32>,
    pub tail: Option<u32>,
    pub repeats: [u32; 3],
    pub stretch: Option<[u32; 7]>,
    pub reverse: Option<[u32; 3]>,
    pub query: Option<[u32; 6]>,
    pub matches: Vec<[u32; 2]>,
    pub path: Vec<(usize, [u32; 7])>,
    pub frequencies: Vec<(usize, u32)>,
    pub sequence_count: usize,
    pub appended: Vec<[u32; 4]>,
}
#[derive(Debug)]
pub struct Case<'a> {
    pub name: &'a str,
    pub source: &'a [u8],
    pub minimum: u32,
    pub target: u32,
    pub literal_mode: u32,
    pub path_cells: usize,
    pub kind: u32,
    pub scripts: Vec<Script>,
    pub events: Vec<Event>,
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
pub(crate) fn opt(value: u32) -> Option<u32> {
    (value != NONE).then_some(value)
}
fn distance(off: u32, repeats: [u32; 3], zero: bool) -> Result<u32, &'static str> {
    need(
        off > 0 && repeats.iter().all(|&v| v > 0),
        "positive intent/history",
    )?;
    if off > 3 {
        return Ok(off - 3);
    }
    if !zero {
        return Ok(repeats[(off - 1) as usize]);
    }
    match off {
        1 => Ok(repeats[1]),
        2 => Ok(repeats[2]),
        3 => repeats[0].checked_sub(1).ok_or("repeat subtraction"),
        _ => Err("repeat intent"),
    }
}
pub(crate) fn publish(off: u32, repeats: [u32; 3], zero: bool) -> Result<[u32; 3], &'static str> {
    let d = distance(off, repeats, zero)?;
    need(d > 0, "nonzero resolved repeat")?;
    if off > 3 {
        return Ok([d, repeats[0], repeats[1]]);
    }
    match off - 1 + u32::from(zero) {
        0 => Ok(repeats),
        1 => Ok([d, repeats[0], repeats[2]]),
        _ => Ok([d, repeats[0], repeats[1]]),
    }
}
pub(crate) fn source_match(
    source: &[u8],
    position: u32,
    off: u32,
    length: u32,
    repeats: [u32; 3],
    zero: bool,
) -> Result<(), &'static str> {
    let d = distance(off, repeats, zero)?;
    need(d > 0 && d <= position, "available match history")?;
    let end = position.checked_add(length).ok_or("match end overflow")? as usize;
    let current = source
        .get(position as usize..end)
        .ok_or("match source extent")?;
    let previous = source
        .get((position - d) as usize..end - d as usize)
        .ok_or("prior match extent")?;
    need(current == previous, "exact source match")
}
pub(crate) fn check_pairs(
    source: &[u8],
    position: u32,
    pairs: &[[u32; 2]],
    minimum: u32,
    repeats: Option<([u32; 3], bool)>,
) -> Result<(), &'static str> {
    let mut previous = minimum - 1;
    for &[off, len] in pairs {
        need(
            off > 0
                && off <= position + 3
                && len > previous
                && len <= source.len() as u32 - position,
            "ordered bounded candidates",
        )?;
        if let Some((history, zero)) = repeats {
            source_match(source, position, off, len, history, zero)?;
        } else if off > 3 {
            source_match(source, position, off, len, [1, 4, 8], false)?;
        }
        previous = len;
    }
    Ok(())
}
pub(crate) fn path_value(value: [u32; 7], source_len: usize) -> Result<(), &'static str> {
    need(
        value[0] <= INFINITY
            && value[1] <= source_len as u32 + 3
            && value[2] <= source_len as u32
            && value[3] <= source_len as u32,
        "bounded cell fields",
    )?;
    need(value[2] == 0 || value[1] > 0, "matched cell has intent")?;
    need(
        value[4..].iter().all(|&v| v <= source_len.max(8) as u32),
        "bounded cell history",
    )
}
pub(crate) fn frequency_state(
    values: &[u32; 386],
    mode: u32,
    refreshed: bool,
) -> Result<(), &'static str> {
    let mut start = 0;
    for (i, count) in [256, 36, 53, 32].into_iter().enumerate() {
        let sum: u64 = values[start..start + count]
            .iter()
            .map(|&v| u64::from(v))
            .sum();
        need(sum == u64::from(values[377 + i]), "frequency sum")?;
        if refreshed {
            let base = if i == 0 && mode == 1 {
                0
            } else {
                let adjusted = values[377 + i]
                    .checked_add(1)
                    .ok_or("frequency weight overflow")?;
                adjusted.ilog2() * 256
            };
            need(values[381 + i] == base, "refreshed frequency price")?;
        }
        start += count;
    }
    need(values[385] == 0, "source exceeds predefined threshold")?;
    if mode == 1 {
        need(
            values[..256].iter().all(|&v| v == 0) && values[377] == 0 && values[381] == 0,
            "disabled literal frequencies",
        )?;
    }
    Ok(())
}
pub(crate) fn shape(event: &Event, index: usize, count: usize) -> Result<(), &'static str> {
    need(event.stage <= 11, "known event stage")?;
    need(
        event.reverse_phase == matches!(event.stage, 7..=10),
        "stage phase",
    )?;
    need(
        (event.stage == 0) == (index == 0) && (event.stage == 11) == (index + 1 == count),
        "begin/complete placement",
    )?;
    let positioned = !matches!(event.stage, 0 | 11);
    need(
        event.cur.is_some() == positioned && event.last.is_some() == positioned,
        "relative coordinate presence",
    )?;
    need(
        event.stretch.is_some() == matches!(event.stage, 6..=10),
        "stretch presence",
    )?;
    need(
        event.reverse.is_some() == matches!(event.stage, 7..=9),
        "reverse coordinate presence",
    )?;
    need(
        event.query.is_some() == (event.stage == 1) && event.tail.is_some() == (event.stage == 11),
        "query/tail presence",
    )?;
    need(
        event.cur.is_none_or(|v| v <= 4096),
        "bounded relative position",
    )
}

pub fn read(bytes: &[u8]) -> Result<Vec<Case<'_>>, &'static str> {
    need(bytes.len() <= 16 << 20, "bounded fixture bytes")?;
    let mut r = Reader { bytes, offset: 0 };
    need(r.take(8)? == b"ZOPFX01\0", "fixture magic")?;
    let count = r.word()? as usize;
    r.ensure(count, 36)?;
    let mut cases = reserved(count, 64)?;
    need(count > 0, "nonempty cases")?;
    for _ in 0..count {
        let h = r.words::<9>()?;
        let (name_len, source_len, path_cells, event_count, script_count) = (
            h[0] as usize,
            h[1] as usize,
            h[5] as usize,
            h[6] as usize,
            h[7] as usize,
        );
        need(
            h[8] <= 1 && (3..=7).contains(&h[2]) && h[3] <= 4095 && h[4] <= 1,
            "case parameters",
        )?;
        need(
            (9..=if h[8] == 0 { 512 } else { 8192 }).contains(&source_len)
                && path_cells == (source_len + 3).min(4099),
            "source/path extent",
        )?;
        need(
            (1..=128).contains(&name_len)
                && (2..=if h[8] == 0 { 1024 } else { 16384 }).contains(&event_count)
                && script_count <= 64,
            "case counts",
        )?;
        let name = core::str::from_utf8(r.take(name_len)?).map_err(|_| "case name")?;
        need(name.is_ascii(), "ASCII name")?;
        let source = r.take(source_len)?;
        let minimum = if h[2] == 3 { 3 } else { 4 };
        r.ensure(script_count, 16)?;
        let mut scripts = reserved(script_count, 64)?;
        let mut prior = 0;
        for _ in 0..script_count {
            let position = r.word()?;
            need(
                position > prior && position <= source_len as u32 - 8,
                "script position",
            )?;
            let matches = r.matches(8)?;
            need(!matches.is_empty(), "nonempty script candidates")?;
            check_pairs(source, position, &matches, minimum, None)?;
            scripts.push(Script { position, matches });
            prior = position;
        }
        r.ensure(event_count, 64)?;
        let mut events = reserved(event_count, 16384)?;
        let mut path = reserved(path_cells, 4099)?;
        path.resize(path_cells, [0; 7]);
        let mut frequencies = [0; 386];
        let mut wire_repeats = [1, 4, 8];
        let mut wire_end = 0_u32;
        let mut live_sequences = 0_usize;
        for index in 0..event_count {
            let a = r.words::<10>()?;
            need(
                a[1] <= 1 && a[3] <= a[2] && a[2] <= source_len as u32,
                "phase/source coordinates",
            )?;
            let stretch = r.optional::<7>()?;
            let reverse = r.optional::<3>()?;
            let query = r.optional::<6>()?;
            let matches = if query.is_some() {
                r.matches(8)?
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
                    && sequence_count <= source_len / 3 + 1,
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
                source_match(source, position, row[3], row[2], wire_repeats, row[1] == 0)?;
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
                event.last.is_none_or(|v| v <= source_len as u32),
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
                    q[0] >= 1 && q[0] <= source_len as u32 - 8 && q[4] <= 1 && q[5] == minimum,
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
                    source,
                    q[0],
                    &event.matches,
                    minimum,
                    Some(([q[1], q[2], q[3]], q[4] == 1)),
                )?;
                let expected = scripts
                    .iter()
                    .find(|s| s.position == q[0])
                    .map_or(&[][..], |s| s.matches.as_slice());
                need(event.matches == expected, "exact scripted callback")?;
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
                    start <= end && (end as usize) < path_cells && (position as usize) < path_cells,
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
                        start > position && path[start as usize][3] == path[position as usize][3],
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
            frequency_state(&frequencies, h[4], matches!(event.stage, 0 | 10))?;
            if let Some(tail) = event.tail {
                need(
                    tail == source_len as u32 - wire_end && event.anchor == wire_end,
                    "complete trailing literals",
                )?;
            }
            events.push(event);
        }
        cases.push(Case {
            name,
            source,
            minimum: h[2],
            target: h[3],
            literal_mode: h[4],
            path_cells,
            kind: h[8],
            scripts,
            events,
        });
    }
    need(r.offset == bytes.len(), "no trailing fixture bytes")?;
    Ok(cases)
}
