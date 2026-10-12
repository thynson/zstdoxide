//! Checked scalar keys and row geometry for bounded reference Greedy.
//!
//! Values own no allocation or source borrow. Exact-width loads and wrapping
//! arithmetic stay safe in every backend; there is no pointer operation to
//! optimize behind a feature. Existing named/Fast/DoubleFast domains are separate.
//! Hash formulas derive from facebook/zstd v1.5.7, revision
//! f8745da6ff1ad1e7bab384bd1f9d742439278e99,
//! lib/compress/zstd_compress_internal.h:903-960 (LICENSE.zstd).
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "Greedy storage integration follows")
)]

use crate::{Error, Result};

/// Observed after initializing a fresh zeroed no-dictionary row context, in both
/// C builds: tests/fixtures/reference-greedy-tiny/capture.json. This says nothing
/// about reused contexts or prepared dictionaries, which have distinct contracts.
pub(crate) const FRESH_ROW_SALT: u64 = 0x8358_92b4_bb2c_ae74;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GreedyKey {
    word: u64,
    width: u8,
}

/// Greedy alone admits hash logs 6..19 and effective key widths 4..6.
/// Construction validates the upstream minimum-match domain before clamping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GreedyKeyLayout {
    hash_log: u8,
    width: u8,
    cells: usize,
}

impl GreedyKeyLayout {
    pub(crate) fn new(hash_log: u8, minimum_match: u8) -> Result<Self> {
        if !(6..=19).contains(&hash_log) || !(3..=7).contains(&minimum_match) {
            return Err(Error::InvalidParameter);
        }
        let cells = 1_usize
            .checked_shl(u32::from(hash_log))
            .ok_or(Error::ResourceLimit)?;
        Ok(Self {
            hash_log,
            width: minimum_match.clamp(4, 6),
            cells,
        })
    }

    pub(crate) const fn hash_log(self) -> u8 {
        self.hash_log
    }
    pub(crate) const fn key_bytes(self) -> u8 {
        self.width
    }
    pub(crate) const fn cells(self) -> usize {
        self.cells
    }

    /// Copy exactly the logical key bytes; even five/six-byte tails require no
    /// physical padding. Higher bytes remain initialized zero and are discarded
    /// by the hash's left shift. Corresponds to `ZSTD_hashPtr{,Salted}` loading.
    pub(crate) fn read_key(self, source: &[u8]) -> Option<GreedyKey> {
        let width = usize::from(self.width);
        let mut bytes = [0; 8];
        bytes[..width].copy_from_slice(source.get(..width)?);
        Some(GreedyKey {
            word: u64::from_le_bytes(bytes),
            width: self.width,
        })
    }

    pub(crate) fn bucket(self, key: GreedyKey) -> Result<usize> {
        // The hash selects at most hash_log bits, and construction proved that
        // 2^hash_log fits usize. No source position is encoded in this value.
        Ok(self.hash(key, self.hash_log, 0)? as usize)
    }

    /// Salt XOR follows multiplication and precedes selection of the high bits.
    /// Four-byte keys use only the low 32 salt bits; five/six-byte keys use all
    /// 64 bits. Corresponds to `ZSTD_hash4`, `ZSTD_hash5`, `ZSTD_hash6`.
    fn hash(self, key: GreedyKey, bits: u8, salt: u64) -> Result<u32> {
        if key.width != self.width || !(1..=32).contains(&bits) {
            return Err(Error::InvalidParameter);
        }
        let bits = u32::from(bits);
        let value = match key.width {
            4 => u64::from(
                ((key.word as u32).wrapping_mul(2_654_435_761) ^ salt as u32) >> (32 - bits),
            ),
            5 => ((key.word << 24).wrapping_mul(889_523_592_379) ^ salt) >> (64 - bits),
            6 => ((key.word << 16).wrapping_mul(227_718_039_650_203) ^ salt) >> (64 - bits),
            _ => return Err(Error::InvalidParameter),
        };
        Ok(value as u32) // At most 32 selected bits, including on 32-bit targets.
    }
}

/// Row cells include one reserved head and 15/31/63 usable positions. The full
/// salted hash has (hash_log - row_log + 8) bits: row selection plus an 8-bit tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RowLayout {
    key: GreedyKeyLayout,
    row_log: u8,
    attempts: usize,
}

impl RowLayout {
    pub(crate) fn new(key: GreedyKeyLayout, search_log: u8) -> Result<Self> {
        // The parameter layer handles target-dependent upstream limits (29 on
        // 32-bit, 30 on 64-bit). This value layer safely represents either; it
        // never shifts by an unbounded search_log or allocates a chain owner.
        if !(1..=30).contains(&search_log) {
            return Err(Error::InvalidParameter);
        }
        let row_log = search_log.clamp(4, 6);
        Ok(Self {
            key,
            row_log,
            attempts: 1 << search_log.min(row_log),
        })
    }

    pub(crate) const fn key_layout(self) -> GreedyKeyLayout {
        self.key
    }
    pub(crate) const fn row_log(self) -> u8 {
        self.row_log
    }
    pub(crate) const fn row_len(self) -> usize {
        1 << self.row_log
    }
    pub(crate) const fn attempts(self) -> usize {
        self.attempts
    }
    pub(crate) const fn cells(self) -> usize {
        self.key.cells()
    }

    pub(crate) fn key(self, key: GreedyKey, salt: u64) -> Result<RowKey> {
        // hash_log >= 6 >= row_log, so subtraction is defined. The maximum
        // width is 19 - 4 + 8 == 23, inside hash's checked 1..=32-bit domain.
        let hash = self
            .key
            .hash(key, self.key.hash_log - self.row_log + 8, salt)?;
        Ok(RowKey { layout: self, hash })
    }
}

/// An initialized salted row/tag result, retaining its layout identity to reject
/// use with incompatible storage. A collision is not proof of matching bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RowKey {
    layout: RowLayout,
    hash: u32,
}

impl RowKey {
    pub(crate) const fn layout(self) -> RowLayout {
        self.layout
    }
    pub(crate) const fn hash(self) -> u32 {
        self.hash
    }
    pub(crate) const fn tag(self) -> u8 {
        self.hash as u8
    }
    pub(crate) const fn row_start(self) -> usize {
        // Construction selected exactly hash_log - row_log upper bits, so
        // shifting back fits the checked 2^hash_log cell domain on every target.
        ((self.hash >> 8) as usize) << self.layout.row_log
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::match_search::SearchKeyLayout;

    #[test]
    fn greedy_dimensions_do_not_widen_existing_hash_domains() {
        for log in 0..=u8::MAX {
            for minimum in 0..=u8::MAX {
                let result = GreedyKeyLayout::new(log, minimum);
                assert_eq!(
                    result.is_ok(),
                    (6..=19).contains(&log) && (3..=7).contains(&minimum)
                );
                if let Ok(layout) = result {
                    assert_eq!(layout.key_bytes(), minimum.clamp(4, 6));
                    assert_eq!(layout.cells(), 1 << log);
                    assert_eq!(layout.hash_log(), log);
                }
            }
        }
        for width in 4..=6 {
            assert_eq!(
                SearchKeyLayout::new(19, width),
                Err(Error::InvalidParameter)
            );
            assert_eq!(
                SearchKeyLayout::for_fast(19, width),
                Err(Error::InvalidParameter)
            );
        }
        assert_eq!(
            SearchKeyLayout::for_double_fast_long(19),
            Err(Error::InvalidParameter)
        );
    }

    #[test]
    fn exact_width_short_tails_and_incompatible_keys_are_checked() {
        let bytes = [1, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        for width in 4..=6 {
            let layout = GreedyKeyLayout::new(19, width).unwrap();
            assert!(layout.read_key(&bytes[..usize::from(width) - 1]).is_none());
            let key = layout.read_key(&bytes[..usize::from(width)]).unwrap();
            assert_eq!(Some(key), layout.read_key(&bytes));
            assert_eq!(
                key.word,
                u64::from_le_bytes(bytes) & ((1 << (width * 8)) - 1)
            );
            for other in 4..=6 {
                if other != width {
                    let incompatible = GreedyKeyLayout::new(6, other).unwrap();
                    assert_eq!(incompatible.bucket(key), Err(Error::InvalidParameter));
                    assert_eq!(
                        RowLayout::new(incompatible, 4).unwrap().key(key, 0),
                        Err(Error::InvalidParameter)
                    );
                }
            }
            for bits in [0, 33, u8::MAX] {
                assert_eq!(layout.hash(key, bits, 0), Err(Error::InvalidParameter));
            }
        }
    }

    #[test]
    fn pinned_c_unsalted_vectors_determine_salted_high_bits() {
        // The Module 11 primitive C vectors precede this implementation. XOR
        // commutes with high-bit selection, giving an independent salted
        // expectation without rerunning a codec or generating expected hashes.
        let vectors = include_str!("../tests/upstream/match-search/vectors.tsv");
        let patterns = include_bytes!("../tests/upstream/match-search/patterns.bin");
        let mut count = 0;
        for line in vectors.lines().skip(1) {
            let fields: alloc::vec::Vec<_> = line.split('\t').collect();
            let width: u8 = fields[0].parse().unwrap();
            if width > 6 {
                continue;
            }
            let bits: u8 = fields[1].parse().unwrap();
            let id: usize = fields[2].parse().unwrap();
            let expected: u32 = fields[4].parse().unwrap();
            let layout = GreedyKeyLayout::new(bits, width).unwrap();
            let key = layout.read_key(&patterns[id * 8..id * 8 + 8]).unwrap();
            assert_eq!(layout.bucket(key), Ok(expected as usize));
            for salt in [0, FRESH_ROW_SALT, u64::MAX] {
                let high_salt = if width == 4 {
                    (salt as u32) >> (32 - bits)
                } else {
                    (salt >> (64 - bits)) as u32
                };
                assert_eq!(layout.hash(key, bits, salt), Ok(expected ^ high_salt));
                for search in [1, 4, 5, 6, 29, 30] {
                    let row_log = search.clamp(4, 6);
                    let hash_log = bits + row_log - 8;
                    if hash_log < 6 {
                        continue;
                    }
                    let row =
                        RowLayout::new(GreedyKeyLayout::new(hash_log, width).unwrap(), search)
                            .unwrap();
                    let got = row.key(key, salt).unwrap();
                    assert_eq!(got.hash(), expected ^ high_salt);
                    assert!(got.row_start() + row.row_len() <= row.cells());
                    assert_eq!(got.row_start() % row.row_len(), 0);
                    assert_eq!(got.tag(), got.hash() as u8);
                }
            }
            count += 1;
        }
        assert_eq!(count, 3 * 9 * 64);
    }

    #[test]
    fn all_row_shapes_bound_tags_and_attempts_without_chain_resources() {
        for log in 6..=19 {
            for width in 4..=6 {
                let layout = GreedyKeyLayout::new(log, width).unwrap();
                let zero = layout.read_key(&[0; 6]).unwrap();
                for search in 0..=u8::MAX {
                    let result = RowLayout::new(layout, search);
                    assert_eq!(result.is_ok(), (1..=30).contains(&search));
                    if let Ok(row) = result {
                        assert_eq!(row.row_log(), search.clamp(4, 6));
                        assert_eq!(row.attempts(), 1 << search.min(6));
                        assert_eq!(row.key_layout(), layout);
                        let first = row.key(zero, 0).unwrap();
                        let last = row.key(zero, u64::MAX).unwrap();
                        assert_eq!((first.row_start(), first.tag()), (0, 0));
                        assert_eq!(
                            (last.row_start(), last.tag()),
                            (row.cells() - row.row_len(), 255)
                        );
                        assert_eq!(last.layout(), row);
                        assert_eq!(last.hash(), (1 << (log - row.row_log() + 8)) - 1);
                    }
                }
            }
        }
        // All salts with the same low half are indistinguishable for width 4.
        let layout = GreedyKeyLayout::new(19, 4).unwrap();
        let row = RowLayout::new(layout, 4).unwrap();
        let key = layout.read_key(b"abcd").unwrap();
        assert_eq!(
            row.key(key, FRESH_ROW_SALT),
            row.key(key, FRESH_ROW_SALT as u32 as u64)
        );
    }
}
