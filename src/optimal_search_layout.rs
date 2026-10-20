//! Checked scalar records for the private contiguous Optimal match primitive.
//! No allocation or pointer arithmetic: all backends share these value records.
#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "Private sorted-search integration follows these records"
)]

use crate::greedy_key::{GreedyKey, GreedyKeyLayout};
use crate::tree_layout::TreeLayout;
use crate::ultra2_plan::Ultra2Coordinates;
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OptimalSearchLayout {
    tree: TreeLayout,
    window: usize,
    attempts: usize,
    minimum: u8,
    target: usize,
    short_log: Option<u8>,
}
impl OptimalSearchLayout {
    pub(crate) fn new(
        window: u8,
        hash: u8,
        chain: u8,
        search: u8,
        minimum: u8,
        target: usize,
        short_log: Option<u8>,
    ) -> Result<Self> {
        if !(10..=21).contains(&window)
            || !(1..=8).contains(&search)
            || target > 131072
            || (minimum == 3) != short_log.is_some()
            || short_log.is_some_and(|v| !(6..=17).contains(&v))
        {
            return Err(Error::InvalidParameter);
        }
        let tree = TreeLayout::new(GreedyKeyLayout::new(hash, minimum)?, chain)?;
        Ok(Self {
            tree,
            window: 1 << window,
            attempts: 1 << search,
            minimum,
            target,
            short_log,
        })
    }
    /// Captured complete fresh CCtx domain; direct observation geometry is separate.
    pub(crate) fn validate_fresh(self, source_len: usize) -> Result<()> {
        if !(64..=16384).contains(&source_len) || source_len > self.window {
            return Err(Error::InvalidParameter);
        }
        self.validate_ordinary_geometry()
    }
    pub(crate) fn validate_retained(self, source_len: usize) -> Result<()> {
        if !(128..=16384).contains(&source_len) || source_len > self.window {
            return Err(Error::InvalidParameter);
        }
        self.validate_ordinary_geometry()
    }
    /// Private observed threshold cases do not widen ordinary fresh admission.
    pub(crate) fn validate_ultra2(self, source_len: usize) -> Result<()> {
        if !(7..=16384).contains(&source_len) || source_len > self.window {
            return Err(Error::InvalidParameter);
        }
        self.validate_ordinary_geometry()
    }
    fn validate_ordinary_geometry(self) -> Result<()> {
        if !(1024..=65536).contains(&self.window)
            || !(64..=4096).contains(&self.tree.key().cells())
            || !(32..=2048).contains(&self.tree.nodes())
            || !(2..=64).contains(&self.attempts)
            || self.target > 4095
            || self
                .short_log
                .is_some_and(|log| 1_usize << log != self.window)
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
    pub(crate) const fn tree(self) -> TreeLayout {
        self.tree
    }
    pub(crate) const fn attempts(self) -> usize {
        self.attempts
    }
    pub(crate) const fn minimum(self) -> u8 {
        self.minimum
    }
    pub(crate) const fn sufficient(self) -> usize {
        if self.target < 4095 {
            self.target
        } else {
            4095
        }
    }
    pub(crate) const fn short_log(self) -> Option<u8> {
        self.short_log
    }
    pub(crate) fn floor(self, current: usize) -> usize {
        current.saturating_sub(self.window)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OptimalMatch {
    off_base: u32,
    length: u32,
}
impl OptimalMatch {
    /// Only initialized unused cells may contain this unpublished placeholder.
    pub(crate) const EMPTY: Self = Self {
        off_base: 0,
        length: 0,
    };
    pub(crate) fn new(off_base: u32, length: usize) -> Result<Self> {
        if off_base == 0 || length < 3 {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            off_base,
            length: u32::try_from(length).map_err(|_| Error::ResourceLimit)?,
        })
    }
    pub(crate) const fn off_base(self) -> u32 {
        self.off_base
    }
    pub(crate) const fn length(self) -> usize {
        self.length as usize
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OptimalQuery {
    pub(crate) position: usize,
    pub(crate) repeats: [u32; 3],
    pub(crate) zero_literals: bool,
    pub(crate) length_to_beat: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct OptimalSource<'a> {
    bytes: &'a [u8],
    end: usize,
    origin: usize,
}
impl<'a> OptimalSource<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < 8 || bytes.len() > (u32::MAX - 2) as usize {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            bytes,
            end: bytes.len(),
            origin: 0,
        })
    }
    pub(crate) fn for_ultra2(bytes: &'a [u8]) -> Result<Self> {
        if !(7..=16384).contains(&bytes.len()) {
            return Err(Error::InvalidParameter);
        }
        let coordinates = Ultra2Coordinates::new(0, bytes.len())?;
        Ok(Self {
            bytes,
            end: coordinates.end(),
            origin: 0,
        })
    }
    pub(crate) const fn origin(self) -> usize {
        self.origin
    }
    /// Retain the original borrow. No old integer position becomes a pointer.
    pub(crate) fn rebase_ultra2(&mut self) -> Result<()> {
        if self.origin != 0
            || self.end != self.bytes.len()
            || !(7..=16384).contains(&self.bytes.len())
        {
            return Err(Error::InvalidState);
        }
        let next = Ultra2Coordinates::new(self.origin, self.bytes.len())?.rebase()?;
        self.origin = next.origin();
        self.end = next.end();
        Ok(())
    }
    fn offset(self, index: usize, width: usize) -> Result<usize> {
        let length = self
            .end
            .checked_sub(self.origin)
            .ok_or(Error::CorruptData)?;
        let offset = index
            .checked_sub(self.origin)
            .ok_or(Error::InvalidParameter)?;
        if width > length || offset > length - width {
            return Err(Error::InvalidParameter);
        }
        Ok(offset)
    }
    pub(crate) fn is_same_source(self, source: &[u8]) -> bool {
        core::ptr::eq(self.bytes, source)
    }
    pub(crate) const fn len(self) -> usize {
        self.end
    }
    pub(crate) const fn backing_len(self) -> usize {
        self.bytes.len()
    }
    pub(crate) fn restore_end(&mut self) {
        self.origin = 0;
        self.end = self.bytes.len();
    }
    pub(crate) fn set_end(&mut self, end: usize) -> Result<()> {
        if self.origin != 0 || !(8..=self.bytes.len()).contains(&end) {
            return Err(Error::InvalidParameter);
        }
        self.end = end;
        Ok(())
    }
    pub(crate) fn validate(self, query: OptimalQuery, layout: OptimalSearchLayout) -> Result<()> {
        if self.origin != 0
            || self.len() < 8
            || query.position > self.len().saturating_sub(8)
            || query.length_to_beat < if layout.minimum() == 3 { 3 } else { 4 }
            || query.length_to_beat > 4096
            || query.length_to_beat > self.len() - query.position
            || query
                .repeats
                .iter()
                .any(|&v| v > 4096 || v as usize > query.position + 2)
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
    /// Fresh repeats may exceed available prefix history. They remain integers;
    /// the search checks subtraction and the window floor before source access.
    /// This does not relax the original direct-oracle validate contract.
    pub(crate) fn validate_fresh(
        self,
        query: OptimalQuery,
        layout: OptimalSearchLayout,
    ) -> Result<()> {
        layout.validate_fresh(self.len())?;
        if self.origin != 0
            || query.position == 0
            || query.position > self.len() - 8
            || query.length_to_beat != if layout.minimum() == 3 { 3 } else { 4 }
            || query
                .repeats
                .iter()
                .any(|&v| v == 0 || v as usize > self.len())
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
    pub(crate) fn key(self, position: usize, layout: GreedyKeyLayout) -> Result<GreedyKey> {
        let offset = self.offset(position, 0)?;
        let end = self
            .end
            .checked_sub(self.origin)
            .ok_or(Error::CorruptData)?;
        layout
            .read_key(self.bytes.get(offset..end).ok_or(Error::InvalidParameter)?)
            .ok_or(Error::InvalidParameter)
    }
    /// Hash constants from facebook/zstd v1.5.7 ZSTD_hash3 (LICENSE.zstd).
    pub(crate) fn short_bucket(self, position: usize, log: u8) -> Result<usize> {
        if !(6..=17).contains(&log) {
            return Err(Error::InvalidParameter);
        }
        let position = self.offset(position, 3)?;
        let tail = self
            .bytes
            .get(position..)
            .and_then(|v| v.get(..3))
            .ok_or(Error::InvalidParameter)?;
        let word = u32::from_le_bytes([tail[0], tail[1], tail[2], 0]);
        Ok(((word << 8).wrapping_mul(506_832_829) >> (32 - log)) as usize)
    }
    /// Compare immutable overlapping suffixes without reading past logical end.
    pub(crate) fn match_length(self, current: usize, previous: usize) -> Result<usize> {
        if previous >= current || current >= self.len() {
            return Err(Error::CorruptData);
        }
        let current = self.offset(current, 1)?;
        let previous = self.offset(previous, 1)?;
        let end = self
            .end
            .checked_sub(self.origin)
            .ok_or(Error::CorruptData)?;
        let a = self.bytes.get(current..end).ok_or(Error::CorruptData)?;
        let previous_end = previous.checked_add(a.len()).ok_or(Error::CorruptData)?;
        let b = self
            .bytes
            .get(previous..previous_end)
            .ok_or(Error::CorruptData)?;
        let mut matched = 0;
        // Safe slice equality handles long repetitive observations efficiently.
        for (x, y) in a.chunks(64).zip(b.chunks(64)) {
            if x == y {
                matched += x.len();
            } else {
                matched += x.iter().zip(y).take_while(|(x, y)| x == y).count();
                break;
            }
        }
        Ok(matched)
    }
    pub(crate) fn byte(self, position: usize) -> Result<u8> {
        let position = self.offset(position, 1)?;
        self.bytes.get(position).copied().ok_or(Error::CorruptData)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometry_domains_and_optional_short_keys() {
        for minimum in 3..=7 {
            for window in [10, 17, 21] {
                let l = OptimalSearchLayout::new(
                    window,
                    19,
                    19,
                    8,
                    minimum,
                    131072,
                    (minimum == 3).then_some(window.min(17)),
                )
                .unwrap();
                assert_eq!(l.tree().key().key_bytes(), minimum.clamp(4, 6));
                assert_eq!(l.sufficient(), 4095);
                assert_eq!(l.attempts(), 256);
                assert_eq!(l.floor((1 << window) + 7), 7);
            }
        }
        for args in [
            (9, 6, 6, 1, 3, 0, Some(6)),
            (22, 6, 6, 1, 3, 0, Some(6)),
            (10, 5, 6, 1, 3, 0, Some(6)),
            (10, 6, 20, 1, 3, 0, Some(6)),
            (10, 6, 6, 0, 3, 0, Some(6)),
            (10, 6, 6, 9, 3, 0, Some(6)),
            (10, 6, 6, 1, 2, 0, None),
            (10, 6, 6, 1, 3, 0, None),
            (10, 6, 6, 1, 4, 0, Some(6)),
            (10, 6, 6, 1, 3, 0, Some(18)),
            (10, 6, 6, 1, 3, 131073, Some(6)),
        ] {
            assert!(
                OptimalSearchLayout::new(args.0, args.1, args.2, args.3, args.4, args.5, args.6)
                    .is_err()
            );
        }
    }
    #[test]
    fn request_bounds_precede_access() {
        let s = OptimalSource::new(b"abcdefghabcdefgh").unwrap();
        let q = OptimalQuery {
            position: 8,
            repeats: [1, 4, 8],
            zero_literals: false,
            length_to_beat: 8,
        };
        s.validate(
            q,
            OptimalSearchLayout::new(10, 6, 6, 1, 3, 0, Some(6)).unwrap(),
        )
        .unwrap();
        for q in [
            OptimalQuery {
                position: usize::MAX,
                ..q
            },
            OptimalQuery { position: 9, ..q },
            OptimalQuery {
                length_to_beat: 9,
                ..q
            },
            OptimalQuery {
                length_to_beat: 2,
                ..q
            },
            OptimalQuery {
                repeats: [4097, 4, 8],
                ..q
            },
            OptimalQuery {
                repeats: [11, 4, 8],
                ..q
            },
        ] {
            assert!(
                s.validate(
                    q,
                    OptimalSearchLayout::new(10, 6, 6, 1, 3, 0, Some(6)).unwrap()
                )
                .is_err()
            );
        }
        assert!(OptimalSource::new(b"1234567").is_err());
    }
    #[test]
    fn overlap_tail_and_hash_width() {
        let bytes = [b'a'; 257];
        let s = OptimalSource::new(&bytes).unwrap();
        assert_eq!(s.match_length(1, 0), Ok(256));
        assert_eq!(s.match_length(256, 0), Ok(1));
        assert!(s.match_length(0, 0).is_err());
        assert!(s.match_length(258, 0).is_err());
        assert_eq!(s.short_bucket(254, 17), s.short_bucket(0, 17));
        assert!(s.short_bucket(255, 17).is_err());
        assert!(s.short_bucket(0, 18).is_err());
        let s = OptimalSource::new(b"abcdXabcY").unwrap();
        assert_eq!(s.match_length(5, 0), Ok(3));
    }
    #[test]
    fn candidate_values_include_long_direct_matches() {
        assert!(OptimalMatch::new(0, 3).is_err());
        assert!(OptimalMatch::new(1, 2).is_err());
        let m = OptimalMatch::new(262148, 524280).unwrap();
        assert_eq!(m.off_base(), 262148);
        assert_eq!(m.length(), 524280);
        assert_eq!(OptimalMatch::EMPTY.length(), 0);
    }
}
