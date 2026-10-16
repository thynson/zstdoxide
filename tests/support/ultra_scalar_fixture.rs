//! Borrowed reader for native scalar observations, independent of codec pricing.
#![allow(dead_code)]

pub const PACK: &[u8] = include_bytes!("../fixtures/reference-ultra-scalars/cases.bin");
const BLOCK: u32 = 131_072;
const ROWS: usize = 2470;
const ROW_BYTES: usize = 48;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    pub words: [u32; 10],
    pub value: i64,
}

pub struct Fixture<'a> {
    rows: &'a [u8],
}

fn need(condition: bool, reason: &'static str) -> Result<(), &'static str> {
    if condition { Ok(()) } else { Err(reason) }
}

// Format interval widths from pinned zstd_internal.h; NOTICE/LICENSE.zstd.
fn literal_code(length: u32) -> usize {
    const WIDTHS: [u8; 36] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10,
        11, 12, 13, 14, 15, 16,
    ];
    let length = length.min(BLOCK - 1);
    let mut end = 0;
    for (code, width) in WIDTHS.into_iter().enumerate() {
        end += 1u32 << width;
        if length < end {
            return code;
        }
    }
    WIDTHS.len() - 1
}

fn row(bytes: &[u8]) -> Result<Observation, &'static str> {
    need(bytes.len() == ROW_BYTES, "row extent")?;
    let mut words = [0; 10];
    for (value, bytes) in words.iter_mut().zip(bytes[..40].as_chunks::<4>().0) {
        *value = u32::from_le_bytes(*bytes);
    }
    let value = i64::from_le_bytes(bytes[40..].try_into().map_err(|_| "result extent")?);
    let [mode, tag, a, b, c, d, e, f, g, h] = words;
    need(matches!(mode, 0 | 2) && tag <= 4, "mode/tag")?;
    let maximum = if mode == 0 { u32::MAX - 1 } else { 8_388_606 };
    match tag {
        0 => need(a <= maximum && b | c | d | e | f | g | h == 0, "weight")?,
        1 => {
            need(a <= BLOCK && d <= 1 && e | f | g | h == 0, "LL shape")?;
            need(
                b <= maximum && c <= b && (d == 0 || b | c == 0),
                "LL statistics",
            )?;
        }
        2 => {
            need(
                a != 0 && (3..=BLOCK).contains(&b) && g <= 1 && h == 0,
                "match shape",
            )?;
            need(
                c <= maximum && d <= c && e <= maximum && f <= e,
                "match statistics",
            )?;
            need(g == 0 || c | d | e | f == 0, "unused match statistics")?;
        }
        3 => {
            need(
                a <= BLOCK && e <= 1 && f <= 1 && g | h == 0,
                "literal shape",
            )?;
            need(b <= maximum && c <= b && d <= b - c, "literal statistics")?;
            need(e | f == 0 || b | c | d == 0, "unused literal statistics")?;
            need(
                a == 0 || e != 0 || f != 0 || mode == 2 || b >= 1,
                "positive literal base",
            )?;
        }
        4 => {
            need(
                (1..=BLOCK).contains(&a) && e <= 1 && f | g | h == 0,
                "increment shape",
            )?;
            need(
                b <= maximum && c <= b && d <= b && (e == 0 || b | c | d == 0),
                "increment statistics",
            )?;
            if literal_code(a - 1) == literal_code(a) {
                need(c == d, "same LL symbol")?;
            } else {
                need(d <= b - c, "LL symbols fit sum")?;
            }
        }
        _ => return Err("tag"),
    }
    need(
        value < 1 << 30 && value > -(1 << 30) && (tag == 4 || value >= 0),
        "finite signed result",
    )?;
    Ok(Observation { words, value })
}

impl<'a> Fixture<'a> {
    pub fn read(bytes: &'a [u8]) -> Result<Self, &'static str> {
        need(bytes.len() == 12 + ROWS * ROW_BYTES, "exact fixture extent")?;
        need(&bytes[..8] == b"ZUSFIX01", "fixture magic")?;
        let count = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| "count extent")?);
        need(count == ROWS as u32, "frozen row count")?;
        let fixture = Self { rows: &bytes[12..] };
        for observation in fixture.rows() {
            observation?;
        }
        Ok(fixture)
    }

    pub fn rows(&self) -> impl Iterator<Item = Result<Observation, &'static str>> + '_ {
        self.rows
            .as_chunks::<ROW_BYTES>()
            .0
            .iter()
            .map(|bytes| row(bytes))
    }
}
