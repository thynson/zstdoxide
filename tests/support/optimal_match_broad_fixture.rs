//! Independent sparse reader and source replay for broader observed C maps.
//! Hash constants: facebook/zstd v1.5.7 zstd_compress_internal.h (LICENSE.zstd).
#![allow(dead_code)]
pub const PACK: &[u8] = include_bytes!("../fixtures/reference-optimal-matches-broad/cases.bin");

/// Canonical sparse encoding uniquely represents every cell, including zeros.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SparseCells {
    pub length: usize,
    pub entries: Vec<(usize, u32)>,
}
impl SparseCells {
    pub fn get(&self, index: usize) -> Option<u32> {
        if index >= self.length {
            return None;
        }
        Some(
            self.entries
                .binary_search_by_key(&index, |&(i, _)| i)
                .map_or(0, |found| self.entries[found].1),
        )
    }
    pub fn full(&self) -> impl Iterator<Item = u32> + '_ {
        let mut entries = self.entries.iter().peekable();
        (0..self.length).map(move |index| {
            if entries.peek().is_some_and(|entry| entry.0 == index) {
                entries.next().map_or(0, |entry| entry.1)
            } else {
                0
            }
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub frontier: u32,
    pub short_frontier: u32,
    pub heads: SparseCells,
    pub nodes: SparseCells,
    pub short_heads: SparseCells,
}
#[derive(Clone, Debug)]
pub struct Observation {
    pub query: [u32; 6],
    pub before: Snapshot,
    pub after: Snapshot,
    pub matches: Vec<[u32; 2]>,
}
#[derive(Debug)]
pub struct Case<'a> {
    pub name: &'a str,
    pub source: &'a [u8],
    /// Window, primary hash, chain, search, minimum, applied target, short hash.
    pub geometry: [u32; 7],
    pub observations: Vec<Observation>,
}
struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], &'static str> {
        let end = self.offset.checked_add(count).ok_or("extent overflow")?;
        let data = self.data.get(self.offset..end).ok_or("truncated fixture")?;
        self.offset = end;
        Ok(data)
    }
    fn word(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| "word")?,
        ))
    }
    fn words<const N: usize>(&mut self) -> Result<[u32; N], &'static str> {
        let mut result = [0; N];
        for value in &mut result {
            *value = self.word()?;
        }
        Ok(result)
    }
    fn cells(&mut self, count: usize) -> Result<SparseCells, &'static str> {
        let length = usize::try_from(self.word()?).map_err(|_| "cell count narrowing")?;
        let nonzero = usize::try_from(self.word()?).map_err(|_| "entry count narrowing")?;
        need(length == count && nonzero <= length, "sparse map extent")?;
        let bytes = self.take(nonzero.checked_mul(8).ok_or("entry extent")?)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(nonzero)
            .map_err(|_| "entry allocation")?;
        let mut previous = None;
        for chunk in bytes.as_chunks::<8>().0 {
            let index = usize::try_from(u32::from_le_bytes(
                chunk[..4].try_into().map_err(|_| "index")?,
            ))
            .map_err(|_| "index narrowing")?;
            let value = u32::from_le_bytes(chunk[4..].try_into().map_err(|_| "value")?);
            need(
                index < length && previous.is_none_or(|p| index > p) && value != 0,
                "canonical sparse entry",
            )?;
            previous = Some(index);
            entries.push((index, value));
        }
        Ok(SparseCells { length, entries })
    }
    fn snapshot(&mut self, sizes: [usize; 3]) -> Result<Snapshot, &'static str> {
        Ok(Snapshot {
            frontier: self.word()?,
            short_frontier: self.word()?,
            heads: self.cells(sizes[0])?,
            nodes: self.cells(sizes[1])?,
            short_heads: self.cells(sizes[2])?,
        })
    }
}
fn need(value: bool, error: &'static str) -> Result<(), &'static str> {
    if value { Ok(()) } else { Err(error) }
}
fn bucket(source: &[u8], native: u32, bits: u32, width: u32) -> Result<usize, &'static str> {
    let position = native.checked_sub(2).ok_or("position bias")? as usize;
    let width = width.min(6) as usize;
    let bytes = source
        .get(position..position.checked_add(width.max(4)).ok_or("hash extent")?)
        .ok_or("hash footprint")?;
    let mut word = 0u64;
    for (i, byte) in bytes[..width].iter().enumerate() {
        word |= u64::from(*byte) << (i * 8);
    }
    let hash = match width {
        3 => ((word as u32).wrapping_shl(8).wrapping_mul(506_832_829) >> (32 - bits)) as u64,
        4 => ((word as u32).wrapping_mul(2_654_435_761) >> (32 - bits)) as u64,
        5 => word.wrapping_shl(24).wrapping_mul(889_523_592_379) >> (64 - bits),
        6 => word.wrapping_shl(16).wrapping_mul(227_718_039_650_203) >> (64 - bits),
        _ => return Err("hash width"),
    };
    Ok(hash as usize)
}
fn state_valid(
    state: &Snapshot,
    source: &[u8],
    geometry: [u32; 7],
    current: u32,
) -> Result<(), &'static str> {
    need(
        (2..=source.len() as u32 + 2).contains(&state.frontier),
        "main frontier domain",
    )?;
    need(
        (2..=current).contains(&state.short_frontier),
        "short frontier domain",
    )?;
    for &(key, value) in &state.heads.entries {
        need(
            value == 0 || (2..=current).contains(&value),
            "head position",
        )?;
        if value != 0 {
            need(value as usize - 2 <= source.len() - 8, "primary footprint")?;
            need(
                bucket(source, value, geometry[1], geometry[4].clamp(4, 6))? == key,
                "primary bucket",
            )?;
        }
    }
    for &(_, value) in &state.nodes.entries {
        need(
            value == 0 || (2..=current).contains(&value),
            "sorted edge domain",
        )?;
    }
    for &(key, value) in &state.short_heads.entries {
        need(
            value == 0 || (2..current).contains(&value),
            "short head position",
        )?;
        if value != 0 {
            need(
                bucket(source, value, geometry[6], 3)? == key,
                "short bucket",
            )?;
        }
    }
    Ok(())
}

pub fn read(data: &[u8]) -> Result<Vec<Case<'_>>, &'static str> {
    need(data.len() <= 16 << 20, "fixture bound")?;
    let mut reader = Reader { data, offset: 0 };
    need(reader.take(8)? == b"ZOMBFX2\0", "fixture magic")?;
    let count = reader.word()? as usize;
    need((1..=37).contains(&count), "case count")?;
    let mut cases = Vec::new();
    cases
        .try_reserve_exact(count)
        .map_err(|_| "case allocation")?;
    let mut total_queries = 0usize;
    for _ in 0..count {
        let [name_len, source_len, queries] = reader.words()?;
        let geometry: [u32; 7] = reader.words()?;
        let [w, h, c, s, minimum, target, h3] = geometry;
        need(
            (4..=63).contains(&name_len)
                && (8..=524_288).contains(&source_len)
                && (1..=16).contains(&queries),
            "case bounds",
        )?;
        need(
            (10..=21).contains(&w)
                && (6..=12).contains(&h)
                && (6..=12).contains(&c)
                && (1..=8).contains(&s),
            "geometry",
        )?;
        need(
            (3..=7).contains(&minimum) && target <= 131_072,
            "minimum/target",
        )?;
        need(
            if minimum == 3 {
                h3 == w.min(17)
            } else {
                h3 == 0
            },
            "short owner geometry",
        )?;
        let name =
            core::str::from_utf8(reader.take(name_len as usize)?).map_err(|_| "name UTF8")?;
        need(
            name.starts_with("om_")
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            "name domain",
        )?;
        need(
            !cases.iter().any(|c: &Case<'_>| c.name == name),
            "duplicate name",
        )?;
        let source = reader.take(source_len as usize)?;
        let sizes = [
            1usize << h,
            1usize << c,
            if h3 == 0 { 0 } else { 1usize << h3 },
        ];
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(queries as usize)
            .map_err(|_| "observation allocation")?;
        let mut previous = None;
        for _ in 0..queries {
            let query: [u32; 6] = reader.words()?;
            let [q, rep0, rep1, rep2, ll0, beat] = query;
            need(
                q <= source_len - 8 && previous.is_none_or(|p| q > p),
                "ordered query footprint",
            )?;
            previous = Some(q);
            need(
                [rep0, rep1, rep2].iter().all(|&v| v <= 4096 && v <= q + 2),
                "repeat pointer domain",
            )?;
            need(
                ll0 <= 1
                    && beat >= if minimum == 3 { 3 } else { 4 }
                    && beat <= 4096
                    && beat <= source_len - q,
                "query flags/length",
            )?;
            let before = reader.snapshot(sizes)?;
            let after = reader.snapshot(sizes)?;
            let current = q + 2;
            state_valid(&before, source, geometry, current)?;
            state_valid(&after, source, geometry, current)?;
            if let Some(prior) = observations.last() {
                let prior: &Observation = prior;
                need(before == prior.after, "complete state continuity")?;
            } else {
                need(
                    before.frontier == 2
                        && before.short_frontier == 2
                        && before.heads.entries.is_empty()
                        && before.nodes.entries.is_empty()
                        && before.short_heads.entries.is_empty(),
                    "zero initial state",
                )?;
            }
            let match_count = reader.word()? as usize;
            need(match_count <= (1usize << s) + 4, "candidate capacity")?;
            let mut matches = Vec::new();
            matches
                .try_reserve_exact(match_count)
                .map_err(|_| "match allocation")?;
            let mut best = beat - 1;
            for _ in 0..match_count {
                let [intent, length] = reader.words()?;
                need(
                    intent != 0 && length > best && length <= source_len - q,
                    "ordered match lengths",
                )?;
                best = length;
                let distance = match (intent, ll0) {
                    (1, 0) => rep0,
                    (2, 0) | (1, 1) => rep1,
                    (3, 0) | (2, 1) => rep2,
                    (3, 1) => rep0.checked_sub(1).ok_or("repeat underflow")?,
                    _ => intent.checked_sub(3).ok_or("offset intent")?,
                };
                need(
                    distance != 0 && distance <= q && distance <= 1u32 << w,
                    "match source/window",
                )?;
                let begin = (q - distance) as usize;
                let end = q as usize;
                let length = length as usize;
                need(
                    source[begin..begin + length] == source[end..end + length],
                    "complete source match replay",
                )?;
                matches.push([intent, length as u32]);
            }
            if current < before.frontier {
                need(
                    before == after && matches.is_empty(),
                    "skipped state unchanged",
                )?;
            } else {
                need(
                    after.frontier >= current && after.short_frontier >= before.short_frontier,
                    "frontier progress",
                )?;
            }
            observations.push(Observation {
                query,
                before,
                after,
                matches,
            });
        }
        total_queries = total_queries
            .checked_add(queries as usize)
            .ok_or("query total")?;
        need(total_queries <= 82, "aggregate query bound")?;
        cases.push(Case {
            name,
            source,
            geometry,
            observations,
        });
    }
    need(reader.offset == data.len(), "exact fixture EOF")?;
    Ok(cases)
}
