//! Checked literal-section size fields, independent of table selection.

use crate::frame::MAX_BLOCK_SIZE;
use crate::{Error, Result};

/// A small initialized wire record. Its fixed array and bounded prefix require
/// no allocation or separate unsafe representation.
pub(crate) struct LiteralHeader {
    bytes: [u8; 5],
    length: usize,
}

impl LiteralHeader {
    /// Serialize raw or repeated literals. Corresponds to the uncompressed and
    /// RLE header branches of `ZSTD_compressLiterals` in facebook/zstd v1.5.7.
    pub(crate) fn raw(regenerated: usize, repeated: bool) -> Result<Self> {
        if regenerated > MAX_BLOCK_SIZE {
            return Err(Error::CorruptData);
        }
        let kind = u64::from(repeated);
        let (value, length) = if regenerated < 32 {
            (((regenerated as u64) << 3) | kind, 1)
        } else if regenerated < 4096 {
            (((regenerated as u64) << 4) | 4 | kind, 2)
        } else {
            (((regenerated as u64) << 4) | 12 | kind, 3)
        };
        Ok(Self::from_value(value, length))
    }

    /// Serialize a new or reused Huffman section's complete payload size.
    /// `compressed` includes its optional table, jump table and all markers,
    /// but excludes this header. Both sizes select the four-stream field width.
    /// Corresponds to the compressed header branch of `ZSTD_compressLiterals`.
    pub(crate) fn huffman(
        regenerated: usize,
        compressed: usize,
        four_streams: bool,
        reused: bool,
    ) -> Result<Self> {
        if regenerated > MAX_BLOCK_SIZE || compressed == 0 || compressed >= 1 << 18 {
            return Err(Error::CorruptData);
        }
        let (format, size_bits, length) = if four_streams {
            if regenerated < 6 || compressed < 10 {
                return Err(Error::CorruptData);
            }
            if regenerated < 1 << 10 && compressed < 1 << 10 {
                (1_u64, 10, 3)
            } else if regenerated < 1 << 14 && compressed < 1 << 14 {
                (2, 14, 4)
            } else {
                (3, 18, 5)
            }
        } else {
            if regenerated >= 1 << 10 || compressed >= 1 << 10 {
                return Err(Error::CorruptData);
            }
            (0, 10, 3)
        };
        let kind = if reused { 3 } else { 2 };
        let value = kind
            | (format << 2)
            | ((regenerated as u64) << 4)
            | ((compressed as u64) << (4 + size_bits));
        Ok(Self::from_value(value, length))
    }

    fn from_value(value: u64, length: usize) -> Self {
        let mut bytes = [0; 5];
        bytes.copy_from_slice(&value.to_le_bytes()[..5]);
        Self { bytes, length }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn raw_and_repeated_headers_decode_at_every_size_field_boundary() {
        for length in [0, 1, 31, 32, 4095, 4096, MAX_BLOCK_SIZE] {
            for repeated in [false, true] {
                let header = LiteralHeader::raw(length, repeated).unwrap();
                assert_eq!(
                    header.as_bytes().len(),
                    if length < 32 {
                        1
                    } else if length < 4096 {
                        2
                    } else {
                        3
                    }
                );
                let mut section = Vec::from(header.as_bytes());
                section.resize(section.len() + if repeated { 1 } else { length }, 42);
                let (decoded, consumed) =
                    crate::huffman::decode_literals(&section, &mut None, MAX_BLOCK_SIZE).unwrap();
                assert_eq!(decoded, alloc::vec![42; length]);
                assert_eq!(consumed, section.len());
            }
        }
        assert_eq!(LiteralHeader::raw(31, false).unwrap().as_bytes(), &[0xf8]);
        assert_eq!(
            LiteralHeader::raw(32, true).unwrap().as_bytes(),
            &[0x05, 0x02]
        );
        assert_eq!(
            LiteralHeader::raw(4095, false).unwrap().as_bytes(),
            &[0xf4, 0xff]
        );
        assert_eq!(
            LiteralHeader::raw(4096, true).unwrap().as_bytes(),
            &[0x0d, 0, 1]
        );
        assert_eq!(
            LiteralHeader::raw(MAX_BLOCK_SIZE, false)
                .unwrap()
                .as_bytes(),
            &[0x0c, 0, 0x20]
        );
    }

    #[test]
    fn huffman_fields_select_width_from_both_sizes_and_preserve_kind() {
        for (regenerated, compressed, expected_format, expected_length) in [
            (6, 10, 1, 3),
            (1023, 1023, 1, 3),
            (1024, 10, 2, 4),
            (6, 1024, 2, 4),
            (16383, 16383, 2, 4),
            (16384, 10, 3, 5),
            (6, 16384, 3, 5),
            (MAX_BLOCK_SIZE, (1 << 18) - 1, 3, 5),
        ] {
            for reused in [false, true] {
                let header = LiteralHeader::huffman(regenerated, compressed, true, reused).unwrap();
                let bytes = header.as_bytes();
                assert_eq!(bytes.len(), expected_length);
                assert_eq!((bytes[0] >> 2) & 3, expected_format);
                assert_eq!(bytes[0] & 3, if reused { 3 } else { 2 });
                let mut padded = [0; 8];
                padded[..bytes.len()].copy_from_slice(bytes);
                let value = u64::from_le_bytes(padded);
                let bits = match expected_format {
                    1 => 10,
                    2 => 14,
                    _ => 18,
                };
                assert_eq!((value >> 4) & ((1 << bits) - 1), regenerated as u64);
                assert_eq!(value >> (4 + bits), compressed as u64);
            }
        }
        assert_eq!(
            LiteralHeader::huffman(1023, 1023, false, false)
                .unwrap()
                .as_bytes(),
            &[0xf2, 0xff, 0xff]
        );
        assert_eq!(
            LiteralHeader::huffman(1023, 1023, true, true)
                .unwrap()
                .as_bytes(),
            &[0xf7, 0xff, 0xff]
        );
        assert_eq!(
            LiteralHeader::huffman(1024, 10, true, false)
                .unwrap()
                .as_bytes(),
            &[0x0a, 0x40, 0x28, 0]
        );
    }

    #[test]
    fn invalid_lengths_and_impossible_stream_partitions_are_rejected() {
        for length in [MAX_BLOCK_SIZE + 1, usize::MAX] {
            assert!(LiteralHeader::raw(length, false).is_err());
            assert!(LiteralHeader::huffman(length, 10, true, false).is_err());
        }
        for (regenerated, compressed, four) in [
            (6, 0, true),
            (5, 10, true),
            (6, 9, true),
            (1024, 1, false),
            (0, 1024, false),
            (6, 1 << 18, true),
            (6, usize::MAX, true),
        ] {
            assert!(LiteralHeader::huffman(regenerated, compressed, four, false).is_err());
        }
    }
}
