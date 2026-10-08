//! Checked access to Zstandard bitstreams read from their end marker backward.

use crate::{Error, Result};

/// An immutable stream whose cursor is a count of payload bits remaining.
///
/// Corresponds to the reading contract of upstream `BIT_DStream_t`. The most
/// significant set bit of the final byte is the end marker, not payload.
#[derive(Clone, Debug)]
pub(crate) struct ReverseBits<'a> {
    data: &'a [u8],
    remaining: usize,
}

impl<'a> ReverseBits<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Result<Self> {
        let &last = data.last().ok_or(Error::UnexpectedEof)?;
        if last == 0 {
            return Err(Error::CorruptData);
        }
        let tail_bits = 7 - last.leading_zeros() as usize;
        let remaining = (data.len() - 1)
            .checked_mul(8)
            .and_then(|bits| bits.checked_add(tail_bits))
            .ok_or(Error::ResourceLimit)?;
        Ok(Self { data, remaining })
    }

    pub(crate) fn remaining(&self) -> usize {
        self.remaining
    }

    /// Read an exact field, preserving its original little-endian significance.
    pub(crate) fn read_bits(&mut self, count: u8) -> Result<u32> {
        let value = self.peek_bits(count)?;
        self.remaining -= usize::from(count);
        Ok(value)
    }

    /// Inspect an exact field without consuming it. At most 32 bits are allowed.
    pub(crate) fn peek_bits(&self, count: u8) -> Result<u32> {
        if count > 32 {
            return Err(Error::CorruptData);
        }
        if usize::from(count) > self.remaining {
            return Err(Error::UnexpectedEof);
        }
        if count == 0 {
            return Ok(0);
        }
        let start = self.remaining - usize::from(count);
        let first_byte = start / 8;
        let shift = start % 8;
        let byte_count = (shift + usize::from(count)).div_ceil(8);
        let bytes = self
            .data
            .get(first_byte..first_byte + byte_count)
            .ok_or(Error::UnexpectedEof)?;
        let mut word = 0_u64;
        for index in 0..bytes.len() {
            #[cfg(not(feature = "unsafe-bitstream"))]
            let byte = bytes[index];
            #[cfg(feature = "unsafe-bitstream")]
            // SAFETY: `index` is produced by `0..bytes.len()`, so it addresses
            // an initialized u8 in the live shared slice. u8 has alignment one,
            // the read does not mutate or retain a reference, and shared reads
            // respect the slice's aliasing and lifetime guarantees.
            let byte = unsafe { *bytes.get_unchecked(index) };
            word |= u64::from(byte) << (index * 8);
        }
        let mask = (1_u64 << count) - 1;
        Ok(((word >> shift) & mask) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_is_not_payload() {
        let mut bits = ReverseBits::new(&[0b1101_0100]).unwrap();
        assert_eq!(bits.remaining(), 7);
        assert_eq!(bits.peek_bits(3), Ok(5));
        assert_eq!(bits.remaining(), 7);
        assert_eq!(bits.read_bits(3), Ok(5));
        assert_eq!(bits.read_bits(4), Ok(4));
        assert_eq!(bits.remaining(), 0);
        assert_eq!(bits.read_bits(0), Ok(0));
        assert_eq!(bits.read_bits(1), Err(Error::UnexpectedEof));
    }

    #[test]
    fn crosses_bytes_in_original_significance() {
        let mut bits = ReverseBits::new(&[0x12, 0x35, 0x01]).unwrap();
        assert_eq!(bits.read_bits(12), Ok(0x351));
        assert_eq!(bits.read_bits(4), Ok(2));
    }

    #[test]
    fn reads_unaligned_full_width() {
        let mut bits = ReverseBits::new(&[0x78, 0x56, 0x34, 0x12, 0x0f]).unwrap();
        assert_eq!(bits.read_bits(32), Ok(0xe246_8acf));
        assert_eq!(bits.read_bits(3), Ok(0));
    }

    #[test]
    fn rejects_missing_marker_and_oversized_reads() {
        assert!(matches!(ReverseBits::new(&[]), Err(Error::UnexpectedEof)));
        assert!(matches!(ReverseBits::new(&[0]), Err(Error::CorruptData)));
        let mut bits = ReverseBits::new(&[1]).unwrap();
        assert_eq!(bits.read_bits(33), Err(Error::CorruptData));
        assert_eq!(bits.remaining(), 0);
    }

    #[test]
    fn matches_bit_by_bit_reference_for_every_alignment() {
        let data = [0x97, 0x51, 0xc3, 0xa5, 0x6e, 0x81, 0xff];
        let original = ReverseBits::new(&data).unwrap();
        for skip in 0..=original.remaining() {
            let mut bits = original.clone();
            for _ in 0..skip {
                bits.read_bits(1).unwrap();
            }
            for count in 0..=32.min(bits.remaining()) {
                let start = bits.remaining() - count;
                let mut expected = 0_u32;
                for bit in 0..count {
                    expected |=
                        u32::from((data[(start + bit) / 8] >> ((start + bit) % 8)) & 1) << bit;
                }
                assert_eq!(bits.peek_bits(count as u8), Ok(expected));
            }
        }
    }
}
