//! Owned forward bit output for reverse-consumed entropy streams.

use alloc::vec::Vec;

use crate::{Error, Result};

/// A forward writer for fields consumed by `ReverseBits` in reverse order.
/// Storage is owned; unsafe-bitstream optionally initializes spare capacity
/// directly before increasing the length. Reservoir arithmetic stays shared.
pub(crate) struct BitWriter {
    bytes: Vec<u8>,
    pending: u64,
    used: u8,
    closed: bool,
}

impl BitWriter {
    pub(crate) const fn new() -> Self {
        Self {
            bytes: Vec::new(),
            pending: 0,
            used: 0,
            closed: false,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.bytes.clear();
        self.pending = 0;
        self.used = 0;
        self.closed = false;
    }

    pub(crate) fn write(&mut self, value: u32, count: u8) -> Result<()> {
        if self.closed || count > 32 || u64::from(value) >= (1_u64 << count) {
            return Err(Error::CorruptData);
        }
        // used < 8 and count <= 32, so at most four bytes will be emitted.
        self.bytes
            .try_reserve(4)
            .map_err(|_| Error::AllocationFailed)?;
        self.pending |= u64::from(value) << self.used;
        self.used += count;
        let emitted = usize::from(self.used / 8);
        #[cfg(not(feature = "unsafe-bitstream"))]
        for _ in 0..emitted {
            self.bytes.push(self.pending as u8);
            self.pending >>= 8;
        }
        #[cfg(feature = "unsafe-bitstream")]
        {
            let previous_length = self.bytes.len();
            for slot in self.bytes.spare_capacity_mut().iter_mut().take(emitted) {
                slot.write(self.pending as u8);
                self.pending >>= 8;
            }
            // SAFETY: used was below 8 before adding count <= 32, so emitted
            // is at most four. try_reserve(4) succeeded above, proving enough
            // spare capacity and no usize overflow in previous_length + 4.
            // The loop writes exactly emitted initialized u8 values, and its
            // exclusive spare-capacity borrow has ended. The original prefix
            // remains initialized; the new length is at most the capacity.
            unsafe { self.bytes.set_len(previous_length + emitted) };
        }
        self.used %= 8;
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<()> {
        self.write(1, 1)?;
        if self.used != 0 {
            self.bytes.push(self.pending as u8);
        }
        self.closed = true;
        Ok(())
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn retained_capacity(&self) -> usize {
        self.bytes.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_fields_preserve_zero_and_full_width_values() {
        let fields = [(0, 0), (5, 3), (u32::MAX, 32), (0x1234_5678, 29), (1, 1)];
        let mut writer = BitWriter::new();
        assert!(writer.write(1, 0).is_err());
        assert!(writer.write(8, 3).is_err());
        assert!(writer.write(0, 33).is_err());
        for &(value, count) in &fields {
            writer.write(value, count).unwrap();
        }
        writer.finish().unwrap();
        let mut reader = crate::bitstream::ReverseBits::new(writer.as_bytes()).unwrap();
        for &(value, count) in fields.iter().rev() {
            assert_eq!(reader.read_bits(count).unwrap(), value);
        }
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn closing_is_single_use_and_reset_reuses_storage() {
        let mut writer = BitWriter::new();
        writer.finish().unwrap();
        assert_eq!(writer.as_bytes(), &[1]);
        let capacity = writer.retained_capacity();
        assert_eq!(writer.finish(), Err(Error::CorruptData));
        assert_eq!(writer.write(0, 0), Err(Error::CorruptData));
        assert_eq!(writer.as_bytes(), &[1]);
        writer.reset();
        assert!(writer.as_bytes().is_empty());
        assert_eq!(writer.retained_capacity(), capacity);
        writer.write(0xff, 8).unwrap();
        writer.finish().unwrap();
        assert_eq!(writer.as_bytes(), &[0xff, 1]);
    }

    #[test]
    fn every_reservoir_alignment_matches_an_independent_bit_vector() {
        for alignment in 0..8 {
            for width in 0..=32 {
                let mut writer = BitWriter::new();
                let value = if width == 32 {
                    u32::MAX
                } else {
                    (1_u32 << width) - 1
                };
                writer.write(0, alignment).unwrap();
                writer.write(value, width).unwrap();
                writer.finish().unwrap();
                let mut expected =
                    alloc::vec![0_u8; (usize::from(alignment + width) + 1).div_ceil(8)];
                for index in usize::from(alignment)..=usize::from(alignment + width) {
                    expected[index / 8] |= 1 << (index % 8);
                }
                assert_eq!(writer.as_bytes(), expected);
            }
        }
    }
}
