//! Fallible entropy metadata writers used by formatted dictionary construction.
//!
//! Wire rules and golden vectors follow facebook/zstd v1.5.7's
//! `lib/compress/fse_compress.c::FSE_writeNCount`. This module inverts the checked
//! Rust format reader rather than adopting C's word buffers or pointer layout.

use alloc::vec::Vec;

use crate::fse::FseTable;
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

/// Encode explicit Huffman weights as a normalized header and two-state FSE
/// payload, excluding the leading Huffman payload-length byte.
///
/// Accepts between 2 and 255 weights in 0..=12, with at least two distinct values.
/// Weight twelve permits a one-bit code in a twelve-bit standalone or dictionary
/// tree; callers still enforce the lower depth limit for new inline frame trees.
/// The table log is six, or five when weight twelve is present. Setup scans
/// invert checked decoding transitions;
/// FseTable supplies its existing safe or unsafe backend under the same API.
/// Payloads may exceed 127 bytes; the caller selects a bounded tree fallback when
/// necessary. Corresponds to the weight-stream role of upstream FSE_compress_wksp.
pub(crate) fn encode_weights(weights: &[u8]) -> Result<Vec<u8>> {
    if !(2..=255).contains(&weights.len()) {
        return Err(Error::InvalidParameter);
    }
    let mut frequencies = [0_u64; 13];
    let mut maximum = 0;
    for &weight in weights {
        if weight > 12 {
            return Err(Error::InvalidParameter);
        }
        frequencies[usize::from(weight)] += 1;
        maximum = maximum.max(usize::from(weight));
    }
    if frequencies.iter().filter(|&&count| count != 0).count() < 2 {
        return Err(Error::InvalidParameter);
    }
    // v1.5.7 HUF_readStats reserves FSE workspace for maximum weight eleven
    // at log six. Weight twelve at log six exceeds that public reader's buffer;
    // log five fits all thirteen weight values and its smaller table fits the
    // same workspace. Existing zero-through-eleven streams retain exact bytes.
    let table_log = if maximum == 12 { 5 } else { 6 };
    let normalized = normalize_frequencies(&frequencies[..=maximum], table_log)?;
    let table = FseTable::from_normalized(&normalized, table_log)?;
    let mut states = [0; 2];
    // No transition bits follow the terminal pair. Requiring a positive width
    // in both states makes the decoder stop exactly when it reaches that pair,
    // even when earlier states consume zero bits or the sequence length is odd.
    for position in weights.len() - 2..weights.len() {
        states[position & 1] = terminal_state(&table, weights[position])?;
    }
    let mut bits = EntropyBits::new();
    for position in (0..weights.len() - 2).rev() {
        let current = position & 1;
        let (state, value, width) = inverse_transition(&table, weights[position], states[current])?;
        bits.push(value, width)?;
        states[current] = state;
    }
    // The reverse reader consumes state 0 first, then state 1. A final high bit
    // marks the exact payload extent, so byte padding cannot add FSE symbols.
    bits.push(states[1] as u32, table_log)?;
    bits.push(states[0] as u32, table_log)?;
    bits.push(1, 1)?;
    let payload = bits.into_bytes();
    let mut output = write_normalized_counts(&normalized, table_log)?;
    output
        .try_reserve_exact(payload.len())
        .map_err(|_| Error::AllocationFailed)?;
    output.extend_from_slice(&payload);
    Ok(output)
}

fn terminal_state(table: &FseTable, symbol: u8) -> Result<usize> {
    for state in 0..1_usize << table.table_log() {
        let entry = table.entry(state)?;
        if entry.symbol == symbol && entry.bits != 0 {
            return Ok(state);
        }
    }
    // Every present symbol in a nondegenerate normalized table has at least
    // one positive-width transition. Failure would violate that invariant.
    Err(Error::CorruptData)
}

/// Find the predecessor whose decoded symbol and update reach the given state.
/// Table construction partitions the full next-state space for every symbol.
fn inverse_transition(table: &FseTable, symbol: u8, next: usize) -> Result<(usize, u32, u8)> {
    for state in 0..1_usize << table.table_log() {
        let entry = table.entry(state)?;
        if entry.symbol != symbol {
            continue;
        }
        if let Some(value) = next.checked_sub(usize::from(entry.base))
            && value < 1_usize << entry.bits
        {
            return Ok((state, value as u32, entry.bits));
        }
    }
    Err(Error::CorruptData)
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

    fn weight_cases() -> Vec<Vec<u8>> {
        let mut cases = Vec::new();
        for length in 2..=255 {
            cases.push((0..length).map(|index| (index % 12) as u8).collect());
        }
        for first in 0..=11 {
            for second in 0..=11 {
                let mut weights: Vec<_> = (0..=11).collect();
                weights.extend_from_slice(&[first, second]);
                cases.push(weights);
            }
        }
        for length in [3, 5, 31, 32, 33, 63, 64, 65, 127, 128, 129, 254, 255] {
            for rare in [0, length / 2, length - 1] {
                let mut weights = alloc::vec![11; length];
                weights[rare] = 0;
                cases.push(weights);
            }
        }
        cases
    }

    fn twelve_bit_weight_cases() -> Vec<Vec<u8>> {
        let mut cases = Vec::new();
        for other in 0..12 {
            // Exercise weight twelve in both terminal states, including the
            // shortest even stream and the two possible odd terminal positions.
            cases.extend([
                alloc::vec![other, 12],
                alloc::vec![12, other],
                alloc::vec![other, 12, other],
                alloc::vec![12, other, 12],
            ]);
        }
        for length in [13, 31, 32, 33, 127, 128, 129, 254, 255] {
            cases.push((0..length).map(|index| (index % 13) as u8).collect());
            for rare in [0, length / 2, length - 1] {
                let mut weights = alloc::vec![12; length];
                weights[rare] = 0;
                cases.push(weights);
            }
        }
        cases
    }

    fn twelve_bit_tree_weights() -> Vec<u8> {
        // Explicit Kraft mass is 2048 + 1024 + ... + 2 + 1 = 4095;
        // the implicit final weight is one, completing a twelve-bit tree.
        let mut weights = alloc::vec![0; 255];
        for (index, weight) in weights.iter_mut().take(11).enumerate() {
            *weight = 12 - index as u8;
        }
        weights[254] = 1;
        weights
    }

    #[test]
    fn weight_streams_preserve_odd_even_lengths_terminal_pairs_and_zero_bit_updates() {
        let cases = weight_cases();
        assert_eq!(cases.len(), 437);
        for (case, weights) in cases.iter().enumerate() {
            let encoded = encode_weights(weights).unwrap();
            assert_eq!(
                crate::fse::decode_weights(&encoded).unwrap(),
                *weights,
                "case {case}"
            );
            assert_eq!(encode_weights(weights).unwrap(), encoded, "case {case}");
            let header_size = read_normalized_counts(&encoded, 255, 6).unwrap().bytes_read;
            // A single payload byte cannot contain both six-bit initial
            // states plus the stop marker, regardless of its bit pattern.
            for end in 0..header_size + 2 {
                assert!(
                    crate::fse::decode_weights(&encoded[..end]).is_err(),
                    "case {case}, prefix {end}"
                );
            }
        }
    }

    #[test]
    fn existing_eleven_bit_weight_streams_preserve_their_encoded_bytes() {
        let mut fingerprint = 0xcbf2_9ce4_8422_2325_u64;
        for weights in weight_cases() {
            let encoded = encode_weights(&weights).unwrap();
            for byte in (encoded.len() as u64)
                .to_le_bytes()
                .into_iter()
                .chain(encoded)
            {
                fingerprint ^= u64::from(byte);
                fingerprint = fingerprint.wrapping_mul(0x100_0000_01b3);
            }
        }
        // Captured from all 437 existing cases before adding weight twelve.
        assert_eq!(fingerprint, 5_470_459_091_833_037_505);
    }

    #[test]
    fn twelve_bit_weight_streams_preserve_terminal_states_and_sparse_alphabets() {
        let cases = twelve_bit_weight_cases();
        assert_eq!(cases.len(), 84);
        for (case, weights) in cases.iter().enumerate() {
            let encoded = encode_weights(weights).unwrap();
            assert_eq!(
                read_normalized_counts(&encoded, 12, 6).unwrap().table_log,
                5
            );
            assert_eq!(
                crate::fse::decode_weights(&encoded).unwrap(),
                *weights,
                "twelve-bit case {case}"
            );
            assert_eq!(encode_weights(weights).unwrap(), encoded);
        }
    }

    #[test]
    fn weight_twelve_builds_a_valid_description_without_broadening_inline_limits() {
        let weights = twelve_bit_tree_weights();
        let payload = encode_weights(&weights).unwrap();
        assert!((1..128).contains(&payload.len()));
        let mut description = alloc::vec![payload.len() as u8];
        description.extend_from_slice(&payload);
        let (table, consumed) =
            crate::huffman::HuffmanTable::read_dictionary(&description).unwrap();
        assert_eq!(consumed, description.len());
        let lengths = table.code_lengths();
        assert_eq!(lengths[0], 1);
        assert_eq!(lengths[254], 12);
        assert_eq!(lengths[255], 12);
        assert!(lengths[11..254].iter().all(|&length| length == 0));
        assert!(crate::huffman::HuffmanTable::read(&description).is_err());
    }

    #[test]
    fn near_flat_literal_tree_weights_fit_the_huffman_payload_limit() {
        let mut weights = alloc::vec![2; 255];
        weights[0] = 3;
        weights[254] = 1;
        let encoded = encode_weights(&weights).unwrap();
        assert!(encoded.len() < 128);
        assert_eq!(crate::fse::decode_weights(&encoded).unwrap(), weights);
    }

    #[test]
    fn weight_streams_reject_unrepresentable_alphabets_and_lengths() {
        for weights in [
            &[][..],
            &[1][..],
            &[1, 1][..],
            &[0; 255][..],
            &[0, 13][..],
            &[255, 1][..],
            &[1; 256][..],
        ] {
            assert_eq!(encode_weights(weights), Err(Error::InvalidParameter));
        }
    }

    /// Compile only the independent test oracle. The crate never links to C.
    #[cfg(feature = "std")]
    #[test]
    #[ignore = "requires ZSTD_UPSTREAM pointing to pinned v1.5.7 with built lib/libzstd.a and a C compiler"]
    fn official_fse_and_huffman_readers_accept_generated_headers() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let upstream = std::path::PathBuf::from(
            std::env::var_os("ZSTD_UPSTREAM").expect("ZSTD_UPSTREAM is required"),
        );
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        struct Directory(std::path::PathBuf);
        impl Drop for Directory {
            fn drop(&mut self) {
                // This test removes only a directory it created exclusively.
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let directory = std::env::temp_dir().join(alloc::format!(
            "zstdoxide-entropy-oracle-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let directory = Directory(directory);
        let source = directory.0.join("decode.c");
        let executable = directory.0.join("decode");
        std::fs::write(&source, r#"
#define FSE_STATIC_LINKING_ONLY
#define HUF_STATIC_LINKING_ONLY
#include "fse.h"
#include "huf.h"
#include "zstd.h"
#include <stdio.h>
#include <string.h>
int main(int argc, char** argv) {
    unsigned char input[512], output[256];
    unsigned workspace[FSE_DECOMPRESS_WKSP_SIZE_U32(6, 255)];
    size_t size = fread(input, 1, sizeof(input), stdin), decoded;
    if (ZSTD_versionNumber() != 10507 || ferror(stdin) || fgetc(stdin) != EOF) return 2;
    if (argc > 1 && !strcmp(argv[1], "huffman")) {
        unsigned ranks[HUF_TABLELOG_MAX + 1] = {0}, symbols = 0, log = 0;
        decoded = HUF_readStats(output, sizeof(output), ranks, &symbols, &log, input, size);
        if (HUF_isError(decoded)) { fprintf(stderr, "%s\n", HUF_getErrorName(decoded)); return 3; }
        if (decoded != size || symbols > 256) return 3;
        if (fputc((int)log, stdout) == EOF || fputc((int)(symbols & 255), stdout) == EOF ||
            fputc((int)(symbols >> 8), stdout) == EOF) return 4;
        return fwrite(output, 1, symbols, stdout) != symbols;
    }
    decoded = FSE_decompress_wksp_bmi2(output, 255, input, size, 6, workspace, sizeof(workspace), 0);
    if (FSE_isError(decoded)) { fprintf(stderr, "%s\n", FSE_getErrorName(decoded)); return 5; }
    return fwrite(output, 1, decoded, stdout) != decoded;
}
"#).unwrap();
        let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
        let compiled = Command::new(compiler)
            .arg("-I")
            .arg(upstream.join("lib/common"))
            .arg("-I")
            .arg(upstream.join("lib"))
            .arg(&source)
            .arg(upstream.join("lib/libzstd.a"))
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&compiled.stderr)
        );
        let mut cases = weight_cases();
        cases.extend(twelve_bit_weight_cases());
        for (case, weights) in cases.iter().enumerate() {
            let encoded = encode_weights(weights).unwrap();
            let mut child = Command::new(&executable)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(&encoded).unwrap();
            let result = child.wait_with_output().unwrap();
            assert!(
                result.status.success(),
                "case {case}: {}",
                std::string::String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, *weights, "case {case}");
        }
        std::eprintln!(
            "Official v1.5.7 FSE decoder verified {} weight streams",
            cases.len()
        );
        let mut frequencies = alloc::vec![[0_u64; 256], [u64::from(u32::MAX); 256]];
        let mut extreme = [0; 256];
        extreme[255] = 1 << 50;
        frequencies.push(extreme);
        for (index, value) in extreme.iter_mut().enumerate().take(60) {
            *value = 1 << index;
        }
        frequencies.push(extreme);
        let mut random = 0x9e37_79b9_u64;
        for trial in 0..64 {
            frequencies.push(core::array::from_fn(|symbol| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                (random & 1023) << ((symbol + trial) % 32)
            }));
        }
        for (case, observed) in frequencies.iter().enumerate() {
            let header = crate::huffman_encode::serialize_literal_table(observed).unwrap();
            let mut lengths = crate::huffman_encode::literal_lengths(observed).unwrap();
            let weights = crate::huffman_encode::explicit_weights(&lengths).unwrap();
            if encode_weights(&weights).unwrap().len() >= 128 {
                lengths = crate::huffman_encode::near_flat_lengths(observed);
            }
            let table_log = *lengths.iter().max().unwrap();
            let expected: Vec<_> = lengths
                .iter()
                .map(|&length| table_log + 1 - length)
                .collect();
            let mut child = Command::new(&executable)
                .arg("huffman")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(&header).unwrap();
            let result = child.wait_with_output().unwrap();
            assert!(
                result.status.success(),
                "HUF case {case}: {}",
                std::string::String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout.len(), 259, "HUF case {case}");
            assert_eq!(result.stdout[0], table_log, "HUF case {case}");
            assert_eq!(&result.stdout[1..3], &[0, 1], "HUF case {case}");
            assert_eq!(&result.stdout[3..], expected, "HUF case {case}");
        }
        std::eprintln!(
            "Official v1.5.7 HUF_readStats verified {} complete literal headers",
            frequencies.len()
        );
        let mut expected = twelve_bit_tree_weights();
        let payload = encode_weights(&expected).unwrap();
        assert!((1..128).contains(&payload.len()));
        let mut header = alloc::vec![payload.len() as u8];
        header.extend_from_slice(&payload);
        expected.push(1); // HUF_readStats also reconstructs the implicit weight.
        let mut child = Command::new(&executable)
            .arg("huffman")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&header).unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "twelve-bit HUF description: {}",
            std::string::String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(&result.stdout[..3], &[12, 0, 1]);
        assert_eq!(&result.stdout[3..], expected);
        std::eprintln!("Official v1.5.7 HUF_readStats verified a sparse weight-twelve description");
    }
}
