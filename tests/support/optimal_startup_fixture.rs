//! Checked projection of ordinary fresh Optimal observations from pinned zstd 1.5.7.
//! Candidate lists in the nested path format are captured outputs, never C inputs.
#![allow(dead_code)]
#[path = "optimal_path_fixture.rs"]
pub mod path;

pub const PACK: &[u8] = include_bytes!("../fixtures/reference-optimal-startup/cases.bin");
#[derive(Debug)]
pub struct Case<'a> {
    pub window: u8,
    pub hash: u8,
    pub chain: u8,
    pub search: u8,
    pub frame: &'a [u8],
    pub allocations: [[u32; 3]; 2],
    pub path: path::Case<'a>,
    pub search_changes: Vec<Vec<(usize, u32)>>,
}
impl Case<'_> {
    pub fn search_cells(&self) -> usize {
        (1 << self.hash) + (1 << self.chain) + self.short_cells() + 2
    }
    pub fn short_cells(&self) -> usize {
        if self.path.minimum == 3 {
            1 << self.window
        } else {
            0
        }
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
fn need(ok: bool, why: &'static str) -> Result<(), &'static str> {
    if ok { Ok(()) } else { Err(why) }
}
fn reserved<T>(count: usize, cap: usize) -> Result<Vec<T>, &'static str> {
    need(count <= cap, "allocation cap")?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| "allocation failed")?;
    Ok(values)
}
impl<'a> Reader<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], &'static str> {
        let end = self.offset.checked_add(size).ok_or("extent overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated startup fixture")?;
        self.offset = end;
        Ok(value)
    }
    fn word(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| "word")?,
        ))
    }
    fn words<const N: usize>(&mut self) -> Result<[u32; N], &'static str> {
        let mut values = [0; N];
        for v in &mut values {
            *v = self.word()?;
        }
        Ok(values)
    }
}
fn frame_extent(frame: &[u8], source: &[u8]) -> Result<(), &'static str> {
    let mut r = Reader {
        bytes: frame,
        offset: 0,
    };
    need(r.take(4)? == [0x28, 0xb5, 0x2f, 0xfd], "frame magic")?;
    let n = source.len();
    let flag = r.take(1)?[0];
    if n < 256 {
        need(
            flag == 0x20 && r.take(1)?[0] as usize == n,
            "small single-segment content size",
        )?;
    } else {
        let size =
            u16::from_le_bytes(r.take(2)?.try_into().map_err(|_| "content size")?) as usize + 256;
        need(
            flag == 0x60 && size == n,
            "large single-segment content size",
        )?;
    }
    let b = r.take(3)?;
    let word = u32::from_le_bytes([b[0], b[1], b[2], 0]);
    let kind = (word >> 1) & 3;
    let size = (word >> 3) as usize;
    need(
        word & 1 == 1 && kind < 3 && size <= n,
        "one bounded final block",
    )?;
    let payload = r.take(if kind == 1 { 1 } else { size })?;
    match kind {
        0 => need(payload == source, "raw source bytes")?,
        1 => need(
            size == n && source.iter().all(|&v| v == payload[0]),
            "RLE source bytes",
        )?,
        _ => need(size > 0 && size < n, "beneficial compressed payload")?,
    }
    need(r.offset == frame.len(), "complete single frame EOF")
}

pub fn read(bytes: &[u8]) -> Result<Vec<Case<'_>>, &'static str> {
    need(bytes.len() <= 16 << 20, "bounded startup fixture")?;
    let mut r = Reader { bytes, offset: 0 };
    need(r.take(8)? == b"ZOSFX01\0", "startup magic")?;
    let count = r.word()? as usize;
    need((1..=12).contains(&count), "bounded nonempty cases")?;
    let mut cases = reserved(count, 12)?;
    for _ in 0..count {
        let h = r.words::<7>()?;
        need(
            (10..=12).contains(&h[0])
                && (6..=9).contains(&h[1])
                && (6..=9).contains(&h[2])
                && (1..=4).contains(&h[3]),
            "startup geometry",
        )?;
        need(
            (10..=1024).contains(&h[4])
                && (64..=1 << 20).contains(&h[5])
                && (2..=1024).contains(&h[6]),
            "case extents",
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
            "upstream sanitizer redzones",
        )?;
        let frame = r.take(h[4] as usize)?;
        let nested = r.take(h[5] as usize)?;
        let mut paths = path::read(nested)?;
        need(paths.len() == 1, "one embedded path case")?;
        let path = paths.pop().ok_or("missing embedded case")?;
        need(
            path.kind == 0
                && (64..=512).contains(&path.source.len())
                && path.events.len() == h[6] as usize,
            "fresh path scope",
        )?;
        need(
            cases.iter().all(|c: &Case<'_>| c.path.name != path.name),
            "unique case names",
        )?;
        frame_extent(frame, path.source)?;
        let mut case = Case {
            window: h[0] as u8,
            hash: h[1] as u8,
            chain: h[2] as u8,
            search: h[3] as u8,
            frame,
            allocations,
            path,
            search_changes: reserved(h[6] as usize, 1024)?,
        };
        let size = case.search_cells();
        let mut state = reserved(size, 5122)?;
        state.resize(size, 0_u32);
        for (index, event) in case.path.events.iter().enumerate() {
            let count = r.word()? as usize;
            need(
                count <= size && count <= bytes.len().saturating_sub(r.offset) / 8,
                "complete bounded search delta",
            )?;
            need(
                index == 0 || event.query.is_some() || count == 0,
                "search changes only during queries",
            )?;
            let mut changes = reserved(count, size)?;
            let mut previous = None;
            for _ in 0..count {
                let [at, value] = r.words()?;
                let at = at as usize;
                need(
                    at < size && previous.is_none_or(|p| p < at),
                    "canonical search indices",
                )?;
                need(
                    state[at] != value
                        && (value == 0 || (2..=case.path.source.len() as u32 + 2).contains(&value)),
                    "changed raw search index domain",
                )?;
                state[at] = value;
                previous = Some(at);
                changes.push((at, value));
            }
            need(
                state[size - 2] >= 2 && state[size - 1] >= 2,
                "initialized biased frontiers",
            )?;
            if index == 0 {
                need(
                    state[..size - 2].iter().all(|&v| v == 0) && state[size - 2..] == [2, 2],
                    "fresh zero maps and biased frontiers",
                )?;
            }
            case.search_changes.push(changes);
        }
        cases.push(case);
    }
    need(r.offset == bytes.len(), "exact startup fixture EOF")?;
    Ok(cases)
}
