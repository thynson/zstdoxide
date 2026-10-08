// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Derived from facebook/zstd v1.5.7 tests/fuzz/fuzz_data_producer.c.
// Used under the BSD-3-Clause option; see root LICENSE.zstd.
//! Safe tail-driven parameter production for the public fuzz target ports.

/// Borrowed fuzz input with checked slice indices replacing the C data pointer.
pub struct DataProducer<'a> {
    input: &'a [u8],
    start: usize,
    end: usize,
}

impl<'a> DataProducer<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        assert!(u32::try_from(input.len()).is_ok());
        Self {
            input,
            start: 0,
            end: input.len(),
        }
    }

    /// Corresponds to `FUZZ_dataProducer_uint32Range`.
    pub fn unsigned_range(&mut self, minimum: u32, maximum: u32) -> u32 {
        assert!(minimum <= maximum);
        let range = maximum - minimum;
        let mut rolling = range;
        let mut result = 0;
        while rolling > 0 && self.end > self.start {
            self.end -= 1;
            result = (result << 8) | u32::from(self.input[self.end]);
            rolling >>= 8;
        }
        if range == u32::MAX {
            result
        } else {
            minimum + result % (range + 1)
        }
    }

    pub fn unsigned(&mut self) -> u32 {
        self.unsigned_range(0, u32::MAX)
    }

    /// Corresponds to `FUZZ_dataProducer_int32Range`; the wider intermediate
    /// makes the complete signed range well-defined instead of overflowing C.
    pub fn signed_range(&mut self, minimum: i32, maximum: i32) -> i32 {
        assert!(minimum <= maximum);
        let range = (i64::from(maximum) - i64::from(minimum)) as u32;
        (i64::from(minimum) + i64::from(self.unsigned_range(0, range))) as i32
    }

    pub fn remaining(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Corresponds to `FUZZ_dataProducer_rollBack`. Checked access additionally
    /// requires that the restored byte count fits the borrowed allocation.
    pub fn rollback(&mut self, remaining: usize) {
        assert!(remaining >= self.remaining());
        assert!(remaining <= self.input.len() - self.start);
        self.end = self.start + remaining;
    }

    /// Corresponds to `FUZZ_dataProducer_contract`: keep the remaining tail,
    /// return the number of discarded prefix bytes, and never extend the view.
    pub fn contract(&mut self, new_size: usize) -> usize {
        let dropped = self.remaining() - new_size.min(self.remaining());
        self.start += dropped;
        dropped
    }

    /// Corresponds exactly to `FUZZ_dataProducer_reserveDataPrefix`: selecting
    /// the tail size first consumes bytes, so the prefix may be shorter than
    /// `original_size - selected_size`.
    pub fn reserve_prefix(&mut self) -> usize {
        let size = self.unsigned_range(0, self.remaining() as u32) as usize;
        self.contract(size)
    }
}

/// Append exact tail selectors that reserve the entire frame as the input
/// prefix and provide a chosen simple-decompress output limit. Searching the
/// at-most-four-byte reserve-width boundary keeps the producer semantics exact.
pub fn decompression_case(frame: &[u8], output_limit: u32) -> Vec<u8> {
    let maximum = u32::try_from(frame.len() * 10).unwrap();
    assert!(output_limit <= maximum);
    let output_bytes = width(maximum);
    for selector_bytes in 0..=4 {
        let total = frame.len() + output_bytes + selector_bytes;
        if width(total as u32) != selector_bytes {
            continue;
        }
        let mut input = frame.to_vec();
        input.extend_from_slice(&output_limit.to_le_bytes()[..output_bytes]);
        input.extend_from_slice(&(output_bytes as u32).to_le_bytes()[..selector_bytes]);
        let mut producer = DataProducer::new(&input);
        assert_eq!(producer.reserve_prefix(), frame.len());
        assert_eq!(producer.unsigned_range(0, maximum), output_limit);
        return input;
    }
    panic!("no consistent tail-width selector");
}

fn width(value: u32) -> usize {
    ((u32::BITS - value.leading_zeros()) as usize).div_ceil(8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_consume_tail_in_upstream_order() {
        let input = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut producer = DataProducer::new(&input);
        assert_eq!(producer.unsigned_range(7, 7), 7);
        assert_eq!(producer.remaining(), 8);
        assert_eq!(producer.unsigned_range(0, 255), 0x88);
        assert_eq!(producer.unsigned_range(1000, 66535), 1000 + 0x7766);
        assert_eq!(producer.unsigned(), 0x5544_3322);
        assert_eq!(producer.signed_range(-99, 100), -82);
        assert!(producer.is_empty());
        assert_eq!(producer.unsigned_range(123, 900), 123);
        assert_eq!(producer.signed_range(-5, 5), -5);
        assert_eq!(producer.unsigned(), 0);
    }

    #[test]
    fn prefix_reservation_matches_independent_upstream_vectors() {
        // Produced by compiling v1.5.7 fuzz_data_producer.c independently.
        for (length, prefix, before, value, after) in [
            (0, 0, 0, 0, 0),
            (1, 0, 0, 0, 0),
            (2, 0, 1, 0, 1),
            (255, 165, 89, 326, 87),
            (256, 14, 240, 72, 239),
            (65535, 42677, 22856, 200061, 22853),
            (65536, 42783, 22750, 191581, 22747),
        ] {
            let input: Vec<_> = (0..length).map(|index| (index * 17 + 123) as u8).collect();
            let mut producer = DataProducer::new(&input);
            assert_eq!(producer.reserve_prefix(), prefix);
            assert_eq!(producer.remaining(), before);
            assert_eq!(producer.unsigned_range(0, 10 * prefix as u32), value);
            assert_eq!(producer.remaining(), after);
        }
    }

    #[test]
    fn contraction_and_rollback_preserve_the_selected_tail() {
        let mut producer = DataProducer::new(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 5]);
        assert_eq!(producer.reserve_prefix(), 4);
        assert_eq!(producer.remaining(), 5);
        assert_eq!(producer.unsigned_range(0, 255), 9);
        producer.rollback(5);
        assert_eq!(producer.unsigned_range(0, 255), 9);
        assert_eq!(producer.contract(100), 0);
        assert_eq!(producer.contract(2), 2);
        assert_eq!(producer.unsigned_range(0, 255), 8);
        assert_eq!(producer.unsigned_range(0, 255), 7);
        assert!(producer.is_empty());
    }

    #[test]
    fn signed_full_width_is_well_defined() {
        let mut producer = DataProducer::new(&[255; 4]);
        assert_eq!(producer.signed_range(i32::MIN, i32::MAX), i32::MAX);
        assert_eq!(producer.signed_range(i32::MIN, i32::MAX), i32::MIN);
    }

    #[test]
    #[should_panic(expected = "remaining <= self.input.len() - self.start")]
    fn rollback_cannot_escape_its_borrowed_slice() {
        DataProducer::new(&[1, 2]).rollback(3);
    }
}
