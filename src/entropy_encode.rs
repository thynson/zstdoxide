//! Fallible entropy metadata writers used by formatted dictionary construction.
//!
//! Wire rules and golden vectors follow facebook/zstd v1.5.7's
//! `lib/compress/fse_compress.c::FSE_writeNCount`. This module inverts the checked
//! Rust format reader rather than adopting C's word buffers or pointer layout.

use alloc::vec::Vec;

use crate::{Error, Result};

/// A setup-only little-endian bit buffer with initialized, owned byte storage.
///
/// `bytes.len() == position.div_ceil(8)` and all unused high bits are zero.
/// Reservation and length arithmetic finish before mutation. This plain owned
/// buffer has no unsafe alternative: performance-sensitive FSE table access
/// reuses the existing independent backend instead of inventing unchecked setup.
struct EntropyBits {
    bytes: Vec<u8>,
    position: usize,
}

impl EntropyBits {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            position: 0,
        }
    }

    fn push(&mut self, value: u32, width: u8) -> Result<()> {
        if width > 32 || u64::from(value) >= (1_u64 << width) {
            return Err(Error::InvalidParameter);
        }
        let end = self
            .position
            .checked_add(usize::from(width))
            .ok_or(Error::ResourceLimit)?;
        let bytes = end.div_ceil(8);
        self.bytes
            .try_reserve(bytes - self.bytes.len())
            .map_err(|_| Error::AllocationFailed)?;
        self.bytes.resize(bytes, 0);
        for offset in 0..usize::from(width) {
            let position = self.position + offset;
            self.bytes[position / 8] |= ((value >> offset) as u8 & 1) << (position % 8);
        }
        self.position = end;
        Ok(())
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Serialize a normalized-count header, including a single-symbol distribution.
///
/// Counts must have exact absolute mass `2^table_log`, use only `-1` or
/// nonnegative entries, and contain between one and 256 symbols. Table logs are
/// in `5..=12`. Trailing zero symbols are omitted by the wire format. Invalid
/// parameters fail before allocation. Corresponds to upstream `FSE_writeNCount`.
pub(crate) fn write_normalized_counts(counts: &[i16], table_log: u8) -> Result<Vec<u8>> {
    if !(5..=12).contains(&table_log) || counts.is_empty() || counts.len() > 256 {
        return Err(Error::InvalidParameter);
    }
    let table_size = 1_usize << table_log;
    let mut mass = 0;
    for &count in counts {
        if count < -1 {
            return Err(Error::InvalidParameter);
        }
        let contribution = usize::from(count.unsigned_abs());
        if contribution > table_size - mass {
            return Err(Error::InvalidParameter);
        }
        // Both the running sum and this addition are bounded by table_size.
        mass += contribution;
    }
    if mass != table_size {
        return Err(Error::InvalidParameter);
    }
    let mut bits = EntropyBits::new();
    bits.push(u32::from(table_log - 5), 4)?;
    let mut remaining = table_size + 1;
    let mut threshold = table_size;
    let mut width = table_log + 1;
    let mut symbol = 0;
    let mut previous_zero = false;
    while remaining > 1 {
        if previous_zero {
            let mut zeros = 0;
            while counts.get(symbol) == Some(&0) {
                zeros += 1;
                symbol += 1;
            }
            // Each continued group says that another three absent symbols
            // follow. A final group in 0..=2 terminates the zero run.
            while zeros >= 3 {
                bits.push(3, 2)?;
                zeros -= 3;
            }
            bits.push(zeros as u32, 2)?;
        }
        let count = *counts.get(symbol).ok_or(Error::InvalidParameter)?;
        let cutoff = 2 * threshold - 1 - remaining;
        let encoded = (i32::from(count) + 1) as usize;
        if encoded < cutoff {
            bits.push(encoded as u32, width - 1)?;
        } else {
            let value = if encoded >= threshold {
                encoded + cutoff
            } else {
                encoded
            };
            bits.push(value as u32, width)?;
        }
        remaining -= usize::from(count.unsigned_abs());
        previous_zero = count == 0;
        symbol += 1;
        while remaining > 1 && remaining < threshold {
            threshold >>= 1;
            width -= 1;
        }
    }
    Ok(bits.into_bytes())
}

/// Allocate exact FSE mass with positive units and integer largest remainders.
///
/// Every nonzero frequency receives at least one state. Remaining states are
/// proportional to the original frequencies; equal fractional remainders prefer
/// the lower symbol. Unit allocations use the format's `-1` marker. This
/// deterministic policy does not reproduce upstream `FSE_normalizeCount`.
/// Zero-only alphabets or more positive symbols than states are invalid.
pub(crate) fn normalize_frequencies(counts: &[u64], table_log: u8) -> Result<Vec<i16>> {
    if !(5..=12).contains(&table_log) || counts.is_empty() || counts.len() > 256 {
        return Err(Error::InvalidParameter);
    }
    let table_size = 1_usize << table_log;
    let present = counts.iter().filter(|&&count| count != 0).count();
    if present == 0 || present > table_size {
        return Err(Error::InvalidParameter);
    }
    // With at most 256 u64 inputs, their sum fits in 72 bits. Multiplying a
    // frequency by the at-most 4096 remaining states fits in 76 bits.
    let total: u128 = counts.iter().map(|&count| u128::from(count)).sum();
    let remaining = table_size - present;
    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(counts.len())
        .map_err(|_| Error::AllocationFailed)?;
    normalized.resize(counts.len(), 0_i16);
    let mut remainders = Vec::new();
    remainders
        .try_reserve_exact(present)
        .map_err(|_| Error::AllocationFailed)?;
    let mut allocated = 0;
    for (symbol, &count) in counts.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let product = u128::from(count) * remaining as u128;
        let share = 1 + (product / total) as usize;
        // A share is at most the table's 4096 states and therefore fits i16.
        normalized[symbol] = share as i16;
        allocated += share;
        remainders.push((product % total, symbol));
    }
    remainders.sort_unstable_by(|&(left, left_symbol), &(right, right_symbol)| {
        right
            .cmp(&left)
            .then_with(|| left_symbol.cmp(&right_symbol))
    });
    for &(_, symbol) in remainders.iter().take(table_size - allocated) {
        normalized[symbol] += 1;
    }
    for count in &mut normalized {
        if *count == 1 {
            *count = -1;
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fse::read_normalized_counts;

    /// Exact bytes generated independently with v1.5.7 FSE_writeNCount,
    /// revision f8745da6ff1ad1e7bab384bd1f9d742439278e99. Constants are used
    /// under the BSD-3-Clause option; see LICENSE.zstd.
    #[test]
    fn normalized_headers_match_pinned_c_cutoff_and_low_probability_vectors() {
        for (counts, log, expected) in [
            (&[16, 16][..], 5, &[0x10, 0x3f][..]),
            (&[31, -1][..], 5, &[0xe0, 0x03][..]),
            (&[32][..], 5, &[0xf0, 0x03][..]),
            (
                &[0, 0, 3, 0, -1, 0, 0, 0, 12, 16][..],
                5,
                &[0x10, 0x22, 0x01, 0x08, 0xdb, 0x07][..],
            ),
            (&[1, 0, -1, 14, 16][..], 5, &[0x20, 0x02, 0xe0, 0x7d][..]),
            (&[2, 3, 5, 7, 15][..], 5, &[0x30, 0x88, 0x41, 0x1f][..]),
            (&[4094, 1, -1][..], 12, &[0xf7, 0xff, 0x04][..]),
        ] {
            let bytes = write_normalized_counts(counts, log).unwrap();
            assert_eq!(bytes, expected);
            let read = read_normalized_counts(&bytes, 255, 12).unwrap();
            assert_eq!(read.table_log, log);
            assert_eq!(&read.counts[..read.symbols], counts);
            assert_eq!(read.bytes_read, bytes.len());
        }
    }

    #[test]
    fn normalized_headers_match_pinned_c_at_every_supported_log() {
        let expected: &[&[u8]] = &[
            &[0xe0, 3],
            &[0xe1, 7],
            &[0xe2, 15],
            &[0xe3, 31],
            &[0xe4, 63],
            &[0xe5, 127],
            &[0xe6, 255, 0],
            &[0xe7, 255, 1],
        ];
        for (log, &expected) in (5..=12).zip(expected) {
            let counts = [(1_i16 << log) - 1, -1];
            assert_eq!(write_normalized_counts(&counts, log).unwrap(), expected);
        }
    }

    #[test]
    fn normalized_header_long_zero_run_and_all_unit_distribution_match_c() {
        let mut counts = [0; 256];
        counts[255] = 32;
        let mut expected = alloc::vec![0x10, 0xfe];
        expected.extend_from_slice(&[0xff; 20]);
        expected.extend_from_slice(&[0xfd, 0x01]);
        assert_eq!(write_normalized_counts(&counts, 5).unwrap(), expected);
        let parsed = read_normalized_counts(&expected, 255, 12).unwrap();
        assert_eq!(parsed.counts, counts);
        assert_eq!(parsed.symbols, 256);
        assert_eq!(
            write_normalized_counts(&[1; 32], 5).unwrap(),
            [
                0x20, 0x84, 0x10, 0x42, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x44, 0x92, 0x24,
                0x49, 0x07,
            ]
        );
    }

    #[test]
    fn zero_run_lengths_and_trailing_zeros_preserve_the_distribution() {
        for zeros in 0..=253 {
            let mut counts = alloc::vec![0; zeros + 2];
            counts[0] = -1;
            counts[zeros + 1] = 31;
            let bytes = write_normalized_counts(&counts, 5).unwrap();
            let parsed = read_normalized_counts(&bytes, 255, 12).unwrap();
            assert_eq!(&parsed.counts[..parsed.symbols], counts);
            if counts.len() < 256 {
                counts.push(0);
                assert_eq!(write_normalized_counts(&counts, 5).unwrap(), bytes);
            }
        }
    }

    #[test]
    fn normalized_headers_reject_invalid_inputs_before_writing() {
        for log in [0, 4, 13, 255] {
            assert_eq!(
                write_normalized_counts(&[32], log),
                Err(Error::InvalidParameter)
            );
        }
        for counts in [
            &[][..],
            &[0][..],
            &[31][..],
            &[33][..],
            &[34, -2][..],
            &[i16::MIN][..],
            &[i16::MAX][..],
            &[0; 257][..],
        ] {
            assert_eq!(
                write_normalized_counts(counts, 5),
                Err(Error::InvalidParameter)
            );
        }
    }

    #[test]
    fn setup_bits_preserve_unaligned_values_and_reject_invalid_fields_atomically() {
        let mut bits = EntropyBits::new();
        bits.push(5, 3).unwrap();
        bits.push(0x1234_5678, 32).unwrap();
        let before = bits.bytes.clone();
        assert_eq!(bits.position, 35);
        assert_eq!(bits.push(8, 3), Err(Error::InvalidParameter));
        assert_eq!(bits.push(0, 33), Err(Error::InvalidParameter));
        assert_eq!(bits.bytes, before);
        assert_eq!(bits.position, 35);
        bits.push(1, 1).unwrap();
        assert_eq!(bits.into_bytes(), [0xc5, 0xb3, 0xa2, 0x91, 0x08]);
    }

    #[test]
    fn normalization_has_exact_mass_sparse_support_and_lower_symbol_ties() {
        assert_eq!(normalize_frequencies(&[1, 1, 1], 5).unwrap(), [11, 11, 10]);
        assert_eq!(normalize_frequencies(&[2, 1, 0], 5).unwrap(), [21, 11, 0]);
        assert_eq!(normalize_frequencies(&[0, 1, 0], 5).unwrap(), [0, 32, 0]);
        assert_eq!(normalize_frequencies(&[1; 32], 5).unwrap(), [-1; 32]);
        assert_eq!(normalize_frequencies(&[1; 256], 8).unwrap(), [-1; 256]);
    }

    #[test]
    fn normalization_uses_wide_arithmetic_for_maximum_frequency_inputs() {
        assert_eq!(normalize_frequencies(&[u64::MAX, 1], 5).unwrap(), [31, -1]);
        assert_eq!(
            normalize_frequencies(&[u64::MAX; 256], 8).unwrap(),
            [-1; 256]
        );
        assert_eq!(
            normalize_frequencies(&[u64::MAX; 2], 12).unwrap(),
            [2048, 2048]
        );
        let small = [0, 1, 2, 3, 4];
        let scaled = small.map(|value| value * 1_000_000_000_000);
        assert_eq!(
            normalize_frequencies(&small, 6).unwrap(),
            normalize_frequencies(&scaled, 6).unwrap()
        );
    }

    #[test]
    fn normalized_frequencies_preserve_support_and_serialize_at_every_table_log() {
        for log in 5..=12 {
            for symbols in [1, 2, 17, 32, 64, 128, 256] {
                if symbols > 1 << log {
                    continue;
                }
                let counts: Vec<_> = (0..symbols)
                    .map(|index| {
                        if index % 3 == 0 {
                            0
                        } else {
                            (index * index + 1) as u64
                        }
                    })
                    .collect();
                if counts.iter().all(|&count| count == 0) {
                    continue;
                }
                let normalized = normalize_frequencies(&counts, log).unwrap();
                assert_eq!(normalized.len(), counts.len());
                assert_eq!(
                    normalized
                        .iter()
                        .map(|count| u32::from(count.unsigned_abs()))
                        .sum::<u32>(),
                    1 << log
                );
                for (&frequency, &count) in counts.iter().zip(&normalized) {
                    assert_eq!(frequency == 0, count == 0);
                    assert!(count >= -1);
                }
                let header = write_normalized_counts(&normalized, log).unwrap();
                let parsed = read_normalized_counts(&header, 255, 12).unwrap();
                assert_eq!(&parsed.counts[..normalized.len()], normalized);
                assert_eq!(normalize_frequencies(&counts, log).unwrap(), normalized);
            }
        }
    }

    #[test]
    fn normalization_rejects_invalid_logs_empty_support_and_oversized_alphabets() {
        for log in [0, 4, 13, 255] {
            assert_eq!(
                normalize_frequencies(&[1], log),
                Err(Error::InvalidParameter)
            );
        }
        for counts in [
            &[][..],
            &[0][..],
            &[0; 256][..],
            &[1; 257][..],
            &[1; 33][..],
        ] {
            assert_eq!(
                normalize_frequencies(counts, 5),
                Err(Error::InvalidParameter)
            );
        }
    }
}
