//! Checked Zstandard Huffman table construction and literals decoding.
//!
//! The format and ordering follow facebook/zstd v1.5.7. This implementation
//! stores positions as indices and uses fallible owned allocations.

use alloc::vec::Vec;

use crate::bitstream::ReverseBits;
use crate::fse::decode_weights;
use crate::{Error, Result};

const MAX_TABLE_LOG: u8 = 11;
const MAX_SYMBOLS: usize = 256;

/// A plain initialized value; both table backends share its representation.
#[derive(Clone, Copy, Debug)]
struct DecodeEntry {
    symbol: u8,
    bits: u8,
}

/// An immutable, complete prefix lookup table owned by a frame decoder.
#[derive(Debug)]
pub(crate) struct HuffmanTable {
    entries: Vec<DecodeEntry>,
    table_log: u8,
}

impl HuffmanTable {
    /// Read weights and build a complete decoding table.
    ///
    /// Corresponds to upstream `HUF_readStats` and `HUF_readDTableX1_wksp`.
    /// The returned count includes the one-byte weights header.
    pub(crate) fn read(input: &[u8]) -> Result<(Self, usize)> {
        let header = usize::from(*input.first().ok_or(Error::UnexpectedEof)?);
        let (mut weights, consumed) = if header >= 128 {
            let count = header - 127;
            let encoded_len = count.div_ceil(2);
            let packed = input.get(1..1 + encoded_len).ok_or(Error::UnexpectedEof)?;
            let mut weights = Vec::new();
            weights
                .try_reserve_exact(count + 1)
                .map_err(|_| Error::AllocationFailed)?;
            for index in 0..count {
                let packed_byte = packed[index / 2];
                weights.push(if index % 2 == 0 {
                    packed_byte >> 4
                } else {
                    packed_byte & 15
                });
            }
            (weights, encoded_len + 1)
        } else {
            let encoded = input.get(1..1 + header).ok_or(Error::UnexpectedEof)?;
            (decode_weights(encoded)?, header + 1)
        };
        if weights.is_empty() || weights.len() >= MAX_SYMBOLS {
            return Err(Error::CorruptData);
        }

        let mut weight_total = 0usize;
        let mut rank_counts = [0usize; MAX_TABLE_LOG as usize + 1];
        for &weight in &weights {
            if weight > MAX_TABLE_LOG {
                return Err(Error::CorruptData);
            }
            rank_counts[usize::from(weight)] += 1;
            if weight != 0 {
                weight_total += 1usize << (weight - 1);
            }
        }
        if weight_total == 0 {
            return Err(Error::CorruptData);
        }
        let table_log = (usize::BITS - weight_total.leading_zeros()) as u8;
        if table_log > MAX_TABLE_LOG {
            return Err(Error::CorruptData);
        }
        let table_size = 1usize << table_log;
        let remainder = table_size - weight_total;
        if !remainder.is_power_of_two() {
            return Err(Error::CorruptData);
        }
        let final_weight = (remainder.trailing_zeros() + 1) as u8;
        rank_counts[usize::from(final_weight)] += 1;
        if rank_counts[1] < 2 || rank_counts[1] % 2 != 0 {
            return Err(Error::CorruptData);
        }
        weights
            .try_reserve(1)
            .map_err(|_| Error::AllocationFailed)?;
        weights.push(final_weight);

        let mut entries = Vec::new();
        entries
            .try_reserve_exact(table_size)
            .map_err(|_| Error::AllocationFailed)?;
        for weight in 1..=table_log {
            let copies = 1usize << (weight - 1);
            let bits = table_log + 1 - weight;
            for (symbol, &symbol_weight) in weights.iter().enumerate() {
                if symbol_weight == weight {
                    let entry = DecodeEntry {
                        symbol: symbol as u8,
                        bits,
                    };
                    for _ in 0..copies {
                        entries.push(entry);
                    }
                }
            }
        }
        if entries.len() != table_size {
            return Err(Error::CorruptData);
        }
        Ok((Self { entries, table_log }, consumed))
    }

    /// Checked access has the same contract in both table implementations.
    #[inline]
    fn lookup(&self, index: usize) -> Result<DecodeEntry> {
        #[cfg(not(feature = "unsafe-huffman"))]
        {
            self.entries.get(index).copied().ok_or(Error::CorruptData)
        }
        #[cfg(feature = "unsafe-huffman")]
        {
            if index >= self.entries.len() {
                return Err(Error::CorruptData);
            }
            // SAFETY: The runtime check proves index is inside the initialized
            // Vec allocation. Its pointer is non-null and aligned for DecodeEntry,
            // and add(index) stays within that allocation. The shared self borrow
            // keeps storage alive and prevents mutation for the load. DecodeEntry
            // is Copy, so reading it neither moves ownership nor creates aliases
            // permitting mutation. No pointer or reference escapes this block.
            Ok(unsafe { *self.entries.as_ptr().add(index) })
        }
    }

    /// Decode exactly `output_len` symbols from one terminated reverse stream.
    ///
    /// Corresponds to upstream `HUF_decompress1X1_usingDTable_internal_body`.
    pub(crate) fn decode_stream(&self, input: &[u8], output_len: usize) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        output
            .try_reserve_exact(output_len)
            .map_err(|_| Error::AllocationFailed)?;
        self.append_stream(input, output_len, &mut output)?;
        Ok(output)
    }

    /// Decode into an already reserved output allocation.
    fn append_stream(&self, input: &[u8], output_len: usize, output: &mut Vec<u8>) -> Result<()> {
        let mut bits = ReverseBits::new(input)?;
        for _ in 0..output_len {
            let available = bits.remaining().min(usize::from(self.table_log)) as u8;
            // A short final code is repeated across all possible suffixes in the
            // table. Zero-extension chooses one equivalent entry without reading
            // past the beginning of the encoded stream.
            let prefix = bits.peek_bits(available)?;
            let index = (prefix << (self.table_log - available)) as usize;
            let entry = self.lookup(index)?;
            if usize::from(entry.bits) > bits.remaining() {
                return Err(Error::CorruptData);
            }
            bits.read_bits(entry.bits)?;
            output.push(entry.symbol);
        }
        if bits.remaining() != 0 {
            return Err(Error::CorruptData);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_weights_and_two_symbol_stream() {
        let (table, consumed) = HuffmanTable::read(&[128, 0x10, 99]).unwrap();
        assert_eq!(consumed, 2);
        assert_eq!(table.decode_stream(&[0x16], 4).unwrap(), [0, 1, 1, 0]);
    }

    #[test]
    fn mixed_lengths_decode_short_final_code() {
        // Weights [1, 1, 2, 3] yield codes 000, 001, 01, 1.
        let (table, _) = HuffmanTable::read(&[130, 0x11, 0x20]).unwrap();
        assert_eq!(table.decode_stream(&[0x0b, 0x02], 4).unwrap(), [0, 1, 2, 3]);
        assert_eq!(table.decode_stream(&[0x03], 1).unwrap(), [3]);
    }

    #[test]
    fn invalid_weights_and_truncation_are_rejected() {
        for input in [
            &[][..],
            &[128][..],
            &[128, 0][..],
            &[128, 0xc0][..],
            &[130, 0x22, 0x10][..],
        ] {
            assert!(HuffmanTable::read(input).is_err(), "input: {input:?}");
        }
    }

    #[test]
    fn stream_must_end_at_declared_symbol_count() {
        let (table, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        assert!(table.decode_stream(&[0x16], 3).is_err());
        assert!(table.decode_stream(&[0x16], 5).is_err());
        assert!(table.decode_stream(&[0], 1).is_err());
        assert!(table.decode_stream(&[], 0).is_err());
        assert_eq!(table.decode_stream(&[1], 0).unwrap(), []);
        assert!(table.lookup(2).is_err());
    }
}
