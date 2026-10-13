//! Bounded reader for audited C price observations, independent of codec code.
#![allow(dead_code)]

pub const PACK: &[u8] = include_bytes!("../fixtures/reference-optimal-prices/cases.bin");
const SOURCE_CAP: usize = 131_072;
const WIDTHS: [usize; 4] = [256, 36, 53, 32];

#[derive(Clone, Debug)]
pub struct Observation {
    pub operation: [u32; 6],
    pub predefined: bool,
    pub frequencies: [u32; 377],
    pub sums: [u32; 4],
    pub base_prices: [u32; 4],
    pub query: Option<[u32; 3]>,
}

#[derive(Debug)]
pub struct Case<'a> {
    pub name: &'a str,
    pub source: &'a [u8],
    pub disabled_literals: bool,
    pub observations: Vec<Observation>,
}

struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], &'static str> {
        let end = self.offset.checked_add(count).ok_or("extent overflow")?;
        let bytes = self.data.get(self.offset..end).ok_or("truncated fixture")?;
        self.offset = end;
        Ok(bytes)
    }
    fn word(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| "word")?,
        ))
    }
    fn words<const N: usize>(&mut self) -> Result<[u32; N], &'static str> {
        let mut words = [0; N];
        for word in &mut words {
            *word = self.word()?;
        }
        Ok(words)
    }
}
fn need(value: bool, error: &'static str) -> Result<(), &'static str> {
    if value { Ok(()) } else { Err(error) }
}
fn flag(value: u32) -> Result<bool, &'static str> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err("boolean tag"),
    }
}
fn span(source: &[u8], offset: u32, length: u32) -> Result<(), &'static str> {
    let offset = usize::try_from(offset).map_err(|_| "offset narrowing")?;
    let length = usize::try_from(length).map_err(|_| "length narrowing")?;
    need(
        offset <= source.len() && length <= source.len() - offset,
        "source range",
    )
}

pub fn read(data: &[u8]) -> Result<Vec<Case<'_>>, &'static str> {
    need(data.len() <= 16 << 20, "fixture limit")?;
    let mut reader = Reader { data, offset: 0 };
    need(reader.take(8)? == b"ZOPFX01\0", "fixture magic")?;
    let count = usize::try_from(reader.word()?).map_err(|_| "case count narrowing")?;
    need((1..=147).contains(&count), "case count")?;
    let mut cases = Vec::new();
    cases
        .try_reserve_exact(count)
        .map_err(|_| "case allocation")?;
    let mut total_operations = 0usize;
    for _ in 0..count {
        let [name_length, source_length, operations, mode] = reader.words()?;
        need((4..=63).contains(&name_length), "name length")?;
        need(source_length <= SOURCE_CAP as u32, "source length")?;
        need((1..=128).contains(&operations), "operation count")?;
        let disabled_literals = flag(mode)?;
        let name =
            core::str::from_utf8(reader.take(name_length as usize)?).map_err(|_| "name UTF8")?;
        need(
            name.starts_with("op_")
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            "name domain",
        )?;
        need(
            !cases.iter().any(|c: &Case<'_>| c.name == name),
            "duplicate name",
        )?;
        let source = reader.take(source_length as usize)?;
        let mut observations = Vec::new();
        observations
            .try_reserve_exact(operations as usize)
            .map_err(|_| "observation allocation")?;
        let mut updates = 0u32;
        let mut updated_literals = 0u64;
        let mut dirty = false;
        let mut literal_mass = false;
        let mut predefined = false;
        let mut cache = [0; 4];
        for index in 0..operations {
            let operation = reader.words()?;
            let [tag, a, b, c, d, e] = operation;
            need(tag <= 5 && (index == 0) == (tag == 0), "initialization/tag")?;
            match tag {
                0 | 3 => {
                    span(source, a, b)?;
                    need(c == 0 && d == 0 && e == 0, "rescale reserved")?;
                    predefined = tag == 0 && b <= 8;
                    if tag == 0 {
                        literal_mass = b > 0 && !disabled_literals;
                    }
                    dirty = false;
                }
                1 | 5 => {
                    span(source, a, b)?;
                    need(
                        b < SOURCE_CAP as u32 && c != 0 && (3..=SOURCE_CAP as u32).contains(&d),
                        "update domain",
                    )?;
                    let repeat = if tag == 1 {
                        need(e == 0, "update reserved")?;
                        1
                    } else {
                        e
                    };
                    need((1..=8192).contains(&repeat), "batch count")?;
                    updates = updates.checked_add(repeat).ok_or("update overflow")?;
                    updated_literals = updated_literals
                        .checked_add(u64::from(b) * u64::from(repeat))
                        .ok_or("literal update overflow")?;
                    need(
                        updates <= 8192 && updated_literals <= 4_194_304,
                        "update budget",
                    )?;
                    literal_mass |= b > 0 && !disabled_literals;
                    dirty = true;
                }
                2 => {
                    need(
                        a == 0 && b == 0 && c == 0 && d == 0 && e == 0,
                        "refresh reserved",
                    )?;
                    dirty = false;
                }
                4 => {
                    span(source, a, b)?;
                    need(
                        c <= SOURCE_CAP as u32 && d != 0 && (3..=SOURCE_CAP as u32).contains(&e),
                        "query domain",
                    )?;
                    need(
                        !dirty && (b == 0 || disabled_literals || predefined || literal_mass),
                        "query lifecycle",
                    )?;
                }
                _ => return Err("tag"),
            }
            let actual_predefined = flag(reader.word()?)?;
            need(actual_predefined == predefined, "price mode")?;
            let frequencies: [u32; 377] = reader.words()?;
            let sums: [u32; 4] = reader.words()?;
            let base_prices: [u32; 4] = reader.words()?;
            let has_query = flag(reader.word()?)?;
            let prices: [u32; 3] = reader.words()?;
            need(has_query == (tag == 4), "query presence")?;
            need(
                prices.iter().all(|&p| p < 1 << 30) && (has_query || prices == [0; 3]),
                "query price domain",
            )?;
            let mut offset = 0;
            for (table, width) in WIDTHS.into_iter().enumerate() {
                let actual = frequencies[offset..offset + width]
                    .iter()
                    .try_fold(0u32, |sum, &n| sum.checked_add(n))
                    .ok_or("frequency sum overflow")?;
                need(actual == sums[table] && actual <= 1 << 24, "frequency sum")?;
                offset += width;
                if matches!(tag, 0 | 2 | 3) && (table != 0 || !disabled_literals) {
                    let incremented = actual.checked_add(1).ok_or("weight overflow")?;
                    cache[table] = incremented.ilog2() * 256;
                }
            }
            need(cache == base_prices, "cached base prices")?;
            if disabled_literals {
                need(
                    sums[0] == 0 && frequencies[..256].iter().all(|&n| n == 0),
                    "disabled literal statistics",
                )?;
            }
            observations.push(Observation {
                operation,
                predefined,
                frequencies,
                sums,
                base_prices,
                query: has_query.then_some(prices),
            });
        }
        total_operations = total_operations
            .checked_add(operations as usize)
            .ok_or("operation total")?;
        need(total_operations <= 2018, "operation total limit")?;
        cases.push(Case {
            name,
            source,
            disabled_literals,
            observations,
        });
    }
    need(reader.offset == data.len(), "trailing fixture bytes")?;
    Ok(cases)
}
