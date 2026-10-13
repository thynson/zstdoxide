//! Bounded borrowed access to frozen direct tree operations for backend replay.
//! The separate integration reader continues to validate observation semantics.
#![forbid(unsafe_code)]
use crate::{Error, Result};
use alloc::vec::Vec;
pub(crate) const PACK: &[u8] = include_bytes!("../fixtures/reference-tree/cases.bin");
pub(crate) struct Operation<'a> {
    pub(crate) query: usize,
    pub(crate) before: usize,
    pub(crate) after: usize,
    pub(crate) window_floor: usize,
    pub(crate) node_floor: usize,
    pub(crate) length: usize,
    pub(crate) offset: usize,
    pub(crate) heads: &'a [u8],
    pub(crate) edges: &'a [u8],
}
pub(crate) struct Case<'a> {
    pub(crate) name: &'a str,
    pub(crate) source: &'a [u8],
    pub(crate) window: u8,
    pub(crate) chain: u8,
    pub(crate) hash: u8,
    pub(crate) search: u8,
    pub(crate) minimum: u8,
    pub(crate) operations: Vec<Operation<'a>>,
}
struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.position.checked_add(n).ok_or(Error::CorruptData)?;
        let b = self
            .bytes
            .get(self.position..end)
            .ok_or(Error::CorruptData)?;
        self.position = end;
        Ok(b)
    }
    fn number(&mut self) -> Result<usize> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| Error::CorruptData)?) as usize)
    }
    fn bounded(&mut self, min: usize, max: usize) -> Result<usize> {
        let n = self.number()?;
        if !(min..=max).contains(&n) {
            return Err(Error::CorruptData);
        }
        Ok(n)
    }
}
pub(crate) fn cells(bytes: &[u8]) -> impl Iterator<Item = usize> + '_ {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b) as usize)
}
pub(crate) fn read(bytes: &[u8]) -> Result<Vec<Case<'_>>> {
    if bytes.len() > 32 << 20 {
        return Err(Error::ResourceLimit);
    }
    let mut c = Cursor { bytes, position: 0 };
    if c.take(8)? != b"ZTREEFX1" {
        return Err(Error::CorruptData);
    }
    let count = c.bounded(1, 103)?;
    let mut cases = Vec::new();
    cases
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    for _ in 0..count {
        let names = c.bounded(1, 63)?;
        let size = c.bounded(8, 131072)?;
        let count = c.bounded(1, 256)?;
        let window = c.bounded(10, 17)? as u8;
        let chain = c.bounded(6, 12)? as u8;
        let hash = c.bounded(6, 12)? as u8;
        let search = c.bounded(1, 8)? as u8;
        let minimum = c.bounded(3, 7)? as u8;
        c.bounded(0, 0)?;
        c.bounded(6, 6)?;
        let name = core::str::from_utf8(c.take(names)?).map_err(|_| Error::CorruptData)?;
        if !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || cases.iter().any(|case: &Case<'_>| case.name == name)
        {
            return Err(Error::CorruptData);
        }
        let source = c.take(size)?;
        let mut operations = Vec::new();
        operations
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        let mut last = None;
        let mut frontier = 0;
        for _ in 0..count {
            let query = c.bounded(0, size - 8)?;
            if last.is_some_and(|p| query <= p) {
                return Err(Error::CorruptData);
            }
            last = Some(query);
            let before = c.bounded(0, size)?;
            let after = c.bounded(0, size)?;
            if before != frontier {
                return Err(Error::CorruptData);
            }
            frontier = after;
            let window_floor = c.number()?;
            let node_floor = c.number()?;
            if window_floor != 2.max((query + 2).saturating_sub(1 << window))
                || node_floor != (query + 2).saturating_sub((1 << (chain - 1)) - 1)
            {
                return Err(Error::CorruptData);
            }
            let length = c.bounded(0, size - query)?;
            let offset = c.bounded(4, 999999999)?;
            let heads = c.take(4 * (1 << hash))?;
            let edges = c.take(4 * (1 << chain))?;
            if cells(heads).any(|v| v == 1 || v > query + 2)
                || cells(edges)
                    .enumerate()
                    .any(|(i, v)| v > query + 2 || (v == 1 && i % 2 == 0))
            {
                return Err(Error::CorruptData);
            }
            if length == 0 {
                if offset != 999999999 {
                    return Err(Error::CorruptData);
                }
            } else {
                let previous = query.checked_sub(offset - 3).ok_or(Error::CorruptData)?;
                if previous + 2 <= window_floor
                    || source.get(previous..previous + length) != source.get(query..query + length)
                {
                    return Err(Error::CorruptData);
                }
            }
            operations.push(Operation {
                query,
                before,
                after,
                window_floor,
                node_floor,
                length,
                offset,
                heads,
                edges,
            });
        }
        cases.push(Case {
            name,
            source,
            window,
            chain,
            hash,
            search,
            minimum,
            operations,
        });
    }
    if c.position != bytes.len() {
        return Err(Error::CorruptData);
    }
    Ok(cases)
}
