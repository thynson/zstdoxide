//! Initialized search keys and the pinned unsalted four-through-eight-byte hash.
//!
//! These copied values own no allocation or source reference. Exact-width byte
//! extraction and wrapping scalar arithmetic stay safe under every backend;
//! there is no pointer access to justify a separate unsafe implementation.
//!
//! Hash constants and formulas derive from facebook/zstd v1.5.7, revision
//! f8745da6ff1ad1e7bab384bd1f9d742439278e99, in
//! lib/compress/zstd_compress_internal.h:903-961. See LICENSE.zstd for the
//! selected BSD-3-Clause license. The independent C fixtures were frozen before
//! this implementation in tests/upstream/match-search.
#![forbid(unsafe_code)]

use crate::match_search::SearchKeyLayout;
use crate::{Error, Result};

/// An initialized little-endian key with explicit width identity.
///
/// Exact-width extraction and four-byte array conversion construct these keys.
/// Bytes above `width` are zero, and a key never retains the source's address
/// or lifetime.
/// The hash log belongs to the layout, so a key can be reused across bucket
/// counts as long as its width remains compatible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "match storage consumers migrate in the next checkpoint"
    )
)]
pub(crate) struct MatchKey {
    word: u64,
    width: u8,
}

/// Preserve the existing four-byte private caller contract without accepting
/// an untyped slice or an unchecked width.
impl From<[u8; 4]> for MatchKey {
    fn from(bytes: [u8; 4]) -> Self {
        Self {
            word: u64::from(u32::from_le_bytes(bytes)),
            width: 4,
        }
    }
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "match storage consumers migrate in the next checkpoint"
    )
)]
impl SearchKeyLayout {
    /// Copy exactly the selected initialized bytes, returning None for a short
    /// tail. Additional source bytes do not participate in the key.
    ///
    /// Corresponds to the byte-loading role of `ZSTD_hash4Ptr` through
    /// `ZSTD_hash8Ptr`, without the C five-through-seven-byte physical overread.
    pub(crate) fn read_key(&self, source: &[u8]) -> Option<MatchKey> {
        let width = usize::from(self.key_bytes());
        let source = source.get(..width)?;
        let mut bytes = [0; 8];
        // SearchKeyLayout admits only widths 4..=8. The selected source range
        // has exactly this length, and all remaining local bytes stay zero.
        bytes[..width].copy_from_slice(source);
        Some(MatchKey {
            word: u64::from_le_bytes(bytes),
            width: self.key_bytes(),
        })
    }

    /// Return the unsalted hash bucket, rejecting incompatible key widths.
    ///
    /// Corresponds to `ZSTD_hash4/5/6/7/8` with zero salt. SearchKeyLayout checks
    /// hash logs 6..=18 (10..=18 for named settings) and bucket counts at construction;
    /// these shifts therefore select a value below that count on every target.
    /// Leaf owners still check their own live initialized storage length.
    pub(crate) fn bucket(&self, key: MatchKey) -> Result<usize> {
        if key.width != self.key_bytes() {
            return Err(Error::InvalidParameter);
        }
        let log = u32::from(self.hash_log());
        let value = match key.width {
            4 => u64::from((key.word as u32).wrapping_mul(2_654_435_761) >> (32 - log)),
            5 => (key.word << 24).wrapping_mul(889_523_592_379) >> (64 - log),
            6 => (key.word << 16).wrapping_mul(227_718_039_650_203) >> (64 - log),
            7 => (key.word << 8).wrapping_mul(58_295_818_150_454_627) >> (64 - log),
            8 => key.word.wrapping_mul(0xcf1b_bcdc_b7a5_6463) >> (64 - log),
            _ => return Err(Error::InvalidParameter),
        };
        // Every arm returns at most 2^log - 1, and the checked layout's
        // bucket_count == 2^log fits usize, so this conversion cannot truncate.
        Ok(value as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATTERNS: &[u8; 512] = include_bytes!("../tests/upstream/match-search/patterns.bin");
    const VECTORS: &str = include_str!("../tests/upstream/match-search/vectors.tsv");
    const PAIRS: &str = include_str!("../tests/upstream/match-search/invariance.tsv");
    const COMPATIBILITY: &str = include_str!("../tests/upstream/match-search/compatibility.tsv");

    fn fields<const N: usize>(line: &str) -> [&str; N] {
        let mut values = line.split('\t');
        let result = core::array::from_fn(|_| values.next().expect("missing fixture field"));
        assert!(values.next().is_none(), "extra fixture field");
        result
    }

    fn hex8(text: &str) -> [u8; 8] {
        assert_eq!(text.len(), 16);
        assert!(text.is_ascii());
        core::array::from_fn(|index| {
            u8::from_str_radix(&text[2 * index..2 * index + 2], 16).unwrap()
        })
    }

    fn pattern(index: usize) -> &'static [u8] {
        assert!(index < 64);
        &PATTERNS[index * 8..index * 8 + 8]
    }

    #[test]
    fn every_pinned_primitive_vector_matches_exact_width_and_padded_inputs() {
        let mut lines = VECTORS.lines();
        assert_eq!(lines.next(), Some("width\tlog\tpattern\tbytes\tbucket"));
        let mut count = 0;
        for line in lines {
            let [width, log, id, bytes, expected] = fields(line);
            let width: u8 = width.parse().unwrap();
            let log: u8 = log.parse().unwrap();
            let id: usize = id.parse().unwrap();
            let expected: usize = expected.parse().unwrap();
            // Exact order/count also rejects missing, duplicate, reordered or
            // extra tuples rather than succeeding on a narrowed fixture set.
            assert_eq!(
                (usize::from(width), usize::from(log), id),
                (4 + count / (9 * 64), 10 + (count / 64) % 9, count % 64)
            );
            let bytes = hex8(bytes);
            assert_eq!(bytes, pattern(id));
            let layout = SearchKeyLayout::new(log, width).unwrap();
            let exact = layout.read_key(&bytes[..usize::from(width)]).unwrap();
            assert_eq!(Some(exact), layout.read_key(&bytes));
            assert_eq!(layout.bucket(exact), Ok(expected));
            assert!(expected < layout.bucket_count());
            count += 1;
        }
        assert_eq!(count, 2880);
    }

    #[test]
    fn fast_small_hashes_select_high_bits_of_independent_pinned_vectors() {
        // The pinned log-ten observations supply independent expected high
        // bits. Smaller logs discard low bucket bits without changing the hash.
        let mut count = 0;
        for line in VECTORS.lines().skip(1) {
            let [width, log, _, bytes, expected] = fields(line);
            let width: u8 = width.parse().unwrap();
            if log != "10" || width > 7 {
                continue;
            }
            let bytes = hex8(bytes);
            let expected: usize = expected.parse().unwrap();
            for small_log in 6..=9 {
                let layout = SearchKeyLayout::for_fast(small_log, width).unwrap();
                let key = layout.read_key(&bytes[..usize::from(width)]).unwrap();
                assert_eq!(layout.bucket(key), Ok(expected >> (10 - small_log)));
                count += 1;
            }
        }
        assert_eq!(count, 4 * 64 * 4);
    }

    #[test]
    fn double_fast_long_layouts_preserve_pinned_eight_byte_hash_bits() {
        let mut count = 0;
        for line in VECTORS.lines().skip(1) {
            let [width, log, _, bytes, expected] = fields(line);
            if width != "8" {
                continue;
            }
            let log: u8 = log.parse().unwrap();
            let bytes = hex8(bytes);
            let expected: usize = expected.parse().unwrap();
            let layout = SearchKeyLayout::for_double_fast_long(log).unwrap();
            assert_eq!(
                layout.bucket(layout.read_key(&bytes).unwrap()),
                Ok(expected)
            );
            count += 1;
            if log == 10 {
                // Derived high-bit expectations from existing C log-ten
                // observations, not a new direct low-log C capture. Dedicated
                // DoubleFast direct-oracle observations follow separately.
                for small_log in 6..=9 {
                    let layout = SearchKeyLayout::for_double_fast_long(small_log).unwrap();
                    let key = layout.read_key(&bytes).unwrap();
                    assert_eq!(layout.bucket(key), Ok(expected >> (10 - small_log)));
                    let short = SearchKeyLayout::for_fast(small_log, 7)
                        .unwrap()
                        .read_key(&bytes)
                        .unwrap();
                    assert_eq!(layout.bucket(short), Err(Error::InvalidParameter));
                    count += 1;
                }
            }
        }
        assert_eq!(count, 13 * 64);
    }

    #[test]
    fn every_pinned_unused_byte_pair_preserves_keys_and_width_eight_controls() {
        let mut lines = PAIRS.lines();
        assert_eq!(
            lines.next(),
            Some(
                "width\tlog\tpattern\tbase\tchanged\tbase_bucket\tchanged_bucket\tfull8_base\tfull8_changed"
            )
        );
        let mut count = 0;
        for line in lines {
            let [
                width,
                log,
                id,
                base,
                changed,
                expected,
                expected_changed,
                full,
                full_changed,
            ] = fields(line);
            let width: u8 = width.parse().unwrap();
            let log: u8 = log.parse().unwrap();
            let id: usize = id.parse().unwrap();
            assert_eq!(
                (usize::from(width), usize::from(log), id),
                (4 + count / (9 * 64), 10 + (count / 64) % 9, count % 64)
            );
            let base = hex8(base);
            let changed = hex8(changed);
            assert_eq!(base, pattern(id));
            let start = usize::from(width);
            assert_eq!(base[..start], changed[..start]);
            for (&first, &second) in base[start..].iter().zip(&changed[start..]) {
                assert_eq!(first ^ second, 0xff);
            }
            let layout = SearchKeyLayout::new(log, width).unwrap();
            let first = layout.read_key(&base).unwrap();
            let second = layout.read_key(&changed).unwrap();
            assert_eq!(first, second);
            assert_eq!(expected.parse::<usize>(), expected_changed.parse());
            assert_eq!(layout.bucket(first), Ok(expected.parse().unwrap()));
            assert_eq!(layout.bucket(second), Ok(expected_changed.parse().unwrap()));
            let full_width = SearchKeyLayout::new(log, 8).unwrap();
            let first_full = full_width.bucket(full_width.read_key(&base).unwrap());
            let second_full = full_width.bucket(full_width.read_key(&changed).unwrap());
            assert_eq!(first_full, Ok(full.parse().unwrap()));
            assert_eq!(second_full, Ok(full_changed.parse().unwrap()));
            assert_ne!(first_full, second_full);
            count += 1;
        }
        assert_eq!(count, 2304);
    }

    #[test]
    fn every_historical_four_byte_guard_preserves_existing_hash_buckets() {
        let layout = SearchKeyLayout::new(16, 4).unwrap();
        let mut lines = COMPATIBILITY.lines();
        assert_eq!(
            lines.next(),
            Some("pattern\tfirst4_le\tcurrent_rust_expression_bucket\tobserved_c_bucket")
        );
        let mut count = 0;
        for line in lines {
            let [id, word, historical, observed] = fields(line);
            let id: usize = id.parse().unwrap();
            assert_eq!(id, count);
            let bytes = pattern(id);
            assert_eq!(
                u32::from_str_radix(word, 16).unwrap(),
                u32::from_le_bytes(bytes[..4].try_into().unwrap())
            );
            let historical: usize = historical.parse().unwrap();
            let observed: usize = observed.parse().unwrap();
            assert_eq!(historical, observed);
            let key = layout.read_key(bytes).unwrap();
            assert_eq!(
                MatchKey::from(<[u8; 4]>::try_from(&bytes[..4]).unwrap()),
                key
            );
            assert_eq!(layout.bucket(key), Ok(observed));
            count += 1;
        }
        assert_eq!(count, 64);
        for (word, expected) in [
            (0_u32, 0),
            (0x0e8b_2f51, 0),
            (1, 0x9e37),
            (u32::MAX, 0x61c8),
        ] {
            let key = layout.read_key(&word.to_le_bytes()).unwrap();
            assert_eq!(layout.bucket(key), Ok(expected));
        }
    }

    #[test]
    fn portability_exact_allocations_short_tails_unaligned_and_extra_bytes() {
        #[repr(align(8))]
        struct Aligned([u8; 16]);
        let storage = Aligned([
            0xcc, 1, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xff, 0xfe, 0xfd, 0xfc, 0xfb, 0xfa,
            0xf9,
        ]);
        let unaligned = &storage.0[1..];
        assert_eq!(unaligned.as_ptr().align_offset(8), 7);
        for width in 4..=8 {
            let length = usize::from(width);
            for log in [10, 16, 18] {
                let layout = SearchKeyLayout::new(log, width).unwrap();
                // Boxed slice allocation lengths are exact, including 5/6/7:
                // Miri catches any attempt to load a physical eight-byte word.
                let exact = unaligned[..length].to_vec().into_boxed_slice();
                let key = layout.read_key(&exact).unwrap();
                assert_eq!(key, layout.read_key(unaligned).unwrap());
                assert_eq!(key.width, width);
                assert!(
                    key.word.to_le_bytes()[length..]
                        .iter()
                        .all(|&byte| byte == 0)
                );
                for short in 0..length {
                    let tail = unaligned[..short].to_vec().into_boxed_slice();
                    assert_eq!(layout.read_key(&tail), None);
                }
                let mut extra = unaligned.to_vec();
                extra[length..].fill(0x5a);
                assert_eq!(layout.read_key(&extra), Some(key));
                assert!(layout.bucket(key).unwrap() < layout.bucket_count());
            }
        }
    }

    #[test]
    fn portability_width_identity_rejects_mismatch_and_allows_different_hash_logs() {
        let bytes = [1, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        for width in 4..=8 {
            let narrow = SearchKeyLayout::new(10, width).unwrap();
            let wide = SearchKeyLayout::new(18, width).unwrap();
            let key = narrow.read_key(&bytes).unwrap();
            if width == 4 {
                assert_eq!(key, MatchKey::from([1, 0x23, 0x45, 0x67]));
            }
            assert_eq!(wide.read_key(&bytes), Some(key));
            assert_eq!(narrow.bucket(key).unwrap(), wide.bucket(key).unwrap() >> 8);
            for other_width in 4..=8 {
                let other = SearchKeyLayout::new(16, other_width).unwrap();
                if width != other_width {
                    assert_eq!(other.bucket(key), Err(Error::InvalidParameter));
                    // Width identity matters even when the numeric word is zero.
                    assert_ne!(narrow.read_key(&[0; 8]), other.read_key(&[0; 8]));
                }
            }
        }
    }

    #[test]
    fn portability_pinned_little_endian_wrapping_witnesses_at_log_extremes() {
        // Frozen C rows: patterns 1/4/5, widths 4..8, logs 10/18. Keeping this
        // small subset literal avoids parsing the whole TSV under target Miri.
        let expected = [
            [391, 100_130, 222, 56_894, 145, 37_345],
            [195, 50_065, 398, 102_053, 86, 22_167],
            [195, 50_065, 405, 103_903, 741, 189_940],
            [195, 50_065, 648, 166_064, 77, 19_837],
            [195, 50_065, 900, 230_555, 427, 109_549],
        ];
        for (index, row) in expected.into_iter().enumerate() {
            let width = u8::try_from(index + 4).unwrap();
            for (source_index, id) in [1, 4, 5].into_iter().enumerate() {
                for (log_index, log) in [10, 18].into_iter().enumerate() {
                    let layout = SearchKeyLayout::new(log, width).unwrap();
                    let key = layout.read_key(&pattern(id)[..usize::from(width)]).unwrap();
                    assert_eq!(layout.bucket(key), Ok(row[source_index * 2 + log_index]));
                }
            }
        }
        let full = SearchKeyLayout::new(18, 8).unwrap();
        assert_eq!(
            full.read_key(pattern(4)).unwrap().word,
            0xefcd_ab89_6745_2301
        );
        assert_eq!(
            full.read_key(pattern(5)).unwrap().word,
            0x0123_4567_89ab_cdef
        );
    }
}
