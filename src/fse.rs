//! Finite State Entropy tables and normalized-frequency parsing.
//!
//! Wire constants and table construction follow facebook/zstd v1.5.7,
//! `lib/common/entropy_common.c` and `lib/common/fse_decompress.c`.

use alloc::vec::Vec;

use crate::bitstream::ReverseBits;
use crate::{Error, Result};

/// One initialized FSE transition; a plain value shared by both feature modes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FseEntry {
    pub(crate) symbol: u8,
    pub(crate) bits: u8,
    pub(crate) base: u16,
}

/// An owned transition table, or one inline RLE transition without allocation.
#[derive(Debug)]
pub(crate) struct FseTable {
    entries: Vec<FseEntry>,
    repeated: Option<FseEntry>,
    table_log: u8,
}

impl FseTable {
    pub(crate) fn retained_capacity(&self) -> usize {
        self.entries.capacity() * core::mem::size_of::<FseEntry>()
    }

    /// Build a decoding table from a validated normalized distribution.
    ///
    /// Corresponds to upstream `FSE_buildDTable_internal`.
    pub(crate) fn from_normalized(counts: &[i16], table_log: u8) -> Result<Self> {
        if !(5..=12).contains(&table_log) || counts.is_empty() || counts.len() > 256 {
            return Err(Error::CorruptData);
        }
        let table_size = 1_usize << table_log;
        let mut total = 0_usize;
        let mut present = 0;
        for &count in counts {
            if count < -1 {
                return Err(Error::CorruptData);
            }
            total += usize::from(count.unsigned_abs());
            present += usize::from(count != 0);
        }
        if total != table_size || present < 2 {
            return Err(Error::CorruptData);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(table_size)
            .map_err(|_| Error::AllocationFailed)?;
        entries.resize(table_size, FseEntry::default());
        let mut next_states = [0_u16; 256];
        let mut low_probability_start = table_size;
        for (symbol, &count) in counts.iter().enumerate() {
            if count == -1 {
                low_probability_start -= 1;
                entries[low_probability_start].symbol = symbol as u8;
                next_states[symbol] = 1;
            } else {
                next_states[symbol] = count as u16;
            }
        }
        let step = (table_size >> 1) + (table_size >> 3) + 3;
        let mask = table_size - 1;
        let mut position = 0;
        for (symbol, &count) in counts.iter().enumerate() {
            for _ in 0..count.max(0) {
                entries[position].symbol = symbol as u8;
                position = (position + step) & mask;
                while position >= low_probability_start {
                    position = (position + step) & mask;
                }
            }
        }
        if position != 0 {
            return Err(Error::CorruptData);
        }
        for entry in &mut entries {
            let next = next_states[usize::from(entry.symbol)];
            if next == 0 {
                return Err(Error::CorruptData);
            }
            next_states[usize::from(entry.symbol)] += 1;
            entry.bits = table_log - (15 - next.leading_zeros() as u8);
            entry.base = ((usize::from(next) << entry.bits) - table_size) as u16;
            if usize::from(entry.base) + (1_usize << entry.bits) > table_size {
                return Err(Error::CorruptData);
            }
        }
        Ok(Self {
            entries,
            repeated: None,
            table_log,
        })
    }

    /// Read a normalized-count header and return its rounded-up byte length.
    ///
    /// `max_symbol` is inclusive. Corresponds to upstream `FSE_readNCount`.
    pub(crate) fn read(data: &[u8], max_symbol: usize, max_log: u8) -> Result<(Self, usize)> {
        let parsed = read_normalized_counts(data, max_symbol, max_log)?;
        let table = Self::from_normalized(&parsed.counts[..parsed.symbols], parsed.table_log)?;
        Ok((table, parsed.bytes_read))
    }

    pub(crate) fn rle(symbol: u8) -> Self {
        Self {
            entries: Vec::new(),
            repeated: Some(FseEntry {
                symbol,
                bits: 0,
                base: 0,
            }),
            table_log: 0,
        }
    }

    pub(crate) fn table_log(&self) -> u8 {
        self.table_log
    }

    pub(crate) fn entry(&self, state: usize) -> Result<FseEntry> {
        if let Some(entry) = self.repeated {
            return if state == 0 {
                Ok(entry)
            } else {
                Err(Error::CorruptData)
            };
        }
        if state >= self.entries.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-fse"))]
        let entry = self.entries[state];
        #[cfg(feature = "unsafe-fse")]
        // SAFETY: the runtime check above proves `state < entries.len()`.
        // Every entry was initialized during construction, the Vec owns its
        // properly aligned allocation for this borrow, and copying an entry
        // through a shared reference neither mutates nor extends its lifetime.
        let entry = unsafe { *self.entries.get_unchecked(state) };
        Ok(entry)
    }
}

/// Validated normalized-count metadata, independent of a decoding table.
pub(crate) struct NormalizedCounts {
    pub(crate) counts: [i16; 256],
    pub(crate) symbols: usize,
    pub(crate) table_log: u8,
    pub(crate) bytes_read: usize,
}

/// Parse the exact outputs of upstream `FSE_readNCount`.
///
/// Single-symbol distributions are valid metadata; compressed decoding tables
/// still reject them through `FseTable::from_normalized`.
pub(crate) fn read_normalized_counts(
    data: &[u8],
    max_symbol: usize,
    max_log: u8,
) -> Result<NormalizedCounts> {
    if max_symbol > 255 || !(5..=12).contains(&max_log) {
        return Err(Error::CorruptData);
    }
    let mut bits = ForwardBits { data, position: 0 };
    let table_log = bits.read(4)? as u8 + 5;
    if table_log > max_log {
        return Err(Error::CorruptData);
    }
    let mut counts = [0_i16; 256];
    let mut remaining = (1_i32 << table_log) + 1;
    let mut threshold = 1_i32 << table_log;
    let mut width = table_log + 1;
    let mut symbol = 0;
    let mut previous_zero = false;
    while remaining > 1 {
        if previous_zero {
            loop {
                let repeat = bits.read(2)? as usize;
                symbol += repeat;
                if symbol > max_symbol {
                    return Err(Error::CorruptData);
                }
                if repeat != 3 {
                    break;
                }
            }
        }
        if symbol > max_symbol {
            return Err(Error::CorruptData);
        }
        let cutoff = 2 * threshold - 1 - remaining;
        let low = bits.read(width - 1)? as i32;
        let encoded = if low < cutoff {
            low
        } else {
            let full = low + ((bits.read(1)? as i32) << (width - 1));
            if full >= threshold {
                full - cutoff
            } else {
                full
            }
        };
        let count = encoded - 1;
        if count < -1 || count.abs() >= remaining {
            return Err(Error::CorruptData);
        }
        counts[symbol] = count as i16;
        symbol += 1;
        remaining -= count.abs();
        previous_zero = count == 0;
        while remaining > 1 && remaining < threshold {
            threshold >>= 1;
            width -= 1;
        }
    }
    Ok(NormalizedCounts {
        counts,
        symbols: symbol,
        table_log,
        bytes_read: bits.position.div_ceil(8),
    })
}

/// A setup-only forward bit cursor for normalized-frequency descriptions.
struct ForwardBits<'a> {
    data: &'a [u8],
    position: usize,
}

impl ForwardBits<'_> {
    fn read(&mut self, count: u8) -> Result<u32> {
        let end = self
            .position
            .checked_add(usize::from(count))
            .ok_or(Error::ResourceLimit)?;
        if end.div_ceil(8) > self.data.len() {
            return Err(Error::UnexpectedEof);
        }
        let mut value = 0;
        for offset in 0..usize::from(count) {
            let position = self.position + offset;
            #[cfg(not(feature = "unsafe-bitstream"))]
            let byte = self.data[position / 8];
            #[cfg(feature = "unsafe-bitstream")]
            // SAFETY: position is less than end, and ceil(end / 8) was checked
            // against data.len(), so position / 8 is inside this initialized
            // slice. The shared slice supplies validity and lifetime; u8 has
            // alignment one, and this copied read cannot violate aliasing.
            let byte = unsafe { *self.data.get_unchecked(position / 8) };
            value |= u32::from((byte >> (position % 8)) & 1) << offset;
        }
        self.position = end;
        Ok(value)
    }
}

/// Decode explicit Huffman weights from a two-state FSE stream.
///
/// Corresponds to upstream `FSE_decompress_usingDTable_generic`. The weight
/// stream has its own termination rule: after emitting a symbol whose state
/// update would exceed the bitstream, emit the other state's final symbol.
pub(crate) fn decode_weights(data: &[u8]) -> Result<Vec<u8>> {
    let (table, header_size) = FseTable::read(data, 255, 6)?;
    let mut bits = ReverseBits::new(data.get(header_size..).ok_or(Error::UnexpectedEof)?)?;
    // Both initial states must be completely present, even for a terminal pair.
    let mut states = [
        bits.read_bits(table.table_log())? as usize,
        bits.read_bits(table.table_log())? as usize,
    ];
    let mut weights = Vec::new();
    weights
        .try_reserve_exact(255)
        .map_err(|_| Error::AllocationFailed)?;
    let mut current = 0;
    loop {
        let entry = table.entry(states[current])?;
        if weights.len() == 255 {
            return Err(Error::CorruptData);
        }
        weights.push(entry.symbol);
        if usize::from(entry.bits) > bits.remaining() {
            if weights.len() == 255 {
                return Err(Error::CorruptData);
            }
            weights.push(table.entry(states[current ^ 1])?.symbol);
            return Ok(weights);
        }
        states[current] = usize::from(entry.base) + bits.read_bits(entry.bits)? as usize;
        current ^= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_metadata_preserves_single_symbol_and_consumed_size() {
        for data in [&[0xf0, 0x03][..], &[0xf0, 0x03, 0xff, 0xff][..]] {
            let parsed = read_normalized_counts(data, 255, 12).unwrap();
            assert_eq!(parsed.table_log, 5);
            assert_eq!(parsed.symbols, 1);
            assert_eq!(parsed.counts[0], 32);
            assert!(parsed.counts[1..].iter().all(|&count| count == 0));
            assert_eq!(parsed.bytes_read, 2);
        }
        assert!(read_normalized_counts(&[0xf0], 255, 12).is_err());
        assert!(FseTable::read(&[0xf0, 0x03], 255, 12).is_err());
    }

    #[test]
    fn reads_balanced_distribution_header() {
        let (table, consumed) = FseTable::read(&[0x10, 0x3f], 1, 5).unwrap();
        assert_eq!(consumed, 2);
        assert_eq!(table.table_log(), 5);
        let reference = FseTable::from_normalized(&[16, 16], 5).unwrap();
        for state in 0..32 {
            assert_eq!(table.entry(state), reference.entry(state));
            assert_eq!(table.entry(state).unwrap().bits, 1);
        }
    }

    #[test]
    fn reads_zero_run_with_continuation() {
        // 16, 0, three repeated zeroes, then 16. Both two-bit run codes
        // (continuation 3 and terminator 0) are required by the wire format.
        let (table, consumed) = FseTable::read(&[0x10, 0x63, 0x3e], 5, 5).unwrap();
        assert_eq!(consumed, 3);
        let reference = FseTable::from_normalized(&[16, 0, 0, 0, 0, 16], 5).unwrap();
        for state in 0..32 {
            assert_eq!(table.entry(state), reference.entry(state));
        }
        assert!(FseTable::read(&[0x10, 0x63, 0x3e], 4, 5).is_err());
    }

    #[test]
    fn low_probability_symbols_fill_end_of_table() {
        let table = FseTable::from_normalized(&[30, -1, -1], 5).unwrap();
        assert_eq!(
            table.entry(31).unwrap(),
            FseEntry {
                symbol: 1,
                bits: 5,
                base: 0
            }
        );
        assert_eq!(
            table.entry(30).unwrap(),
            FseEntry {
                symbol: 2,
                bits: 5,
                base: 0
            }
        );
        for state in 0..30 {
            assert_eq!(table.entry(state).unwrap().symbol, 0);
        }
    }

    #[test]
    fn all_low_probability_symbols_are_supported() {
        let table = FseTable::from_normalized(&[-1; 32], 5).unwrap();
        for state in 0..32 {
            assert_eq!(
                table.entry(state).unwrap(),
                FseEntry {
                    symbol: 31 - state as u8,
                    bits: 5,
                    base: 0
                }
            );
        }
    }

    #[test]
    fn every_symbols_transitions_partition_the_state_space() {
        let counts = [0, 5, -1, 17, 9];
        let table = FseTable::from_normalized(&counts, 5).unwrap();
        for (symbol, &count) in counts.iter().enumerate() {
            let mut covered = [false; 32];
            let mut occurrences = 0;
            for state in 0..32 {
                let entry = table.entry(state).unwrap();
                if usize::from(entry.symbol) == symbol {
                    occurrences += 1;
                    let start = usize::from(entry.base);
                    let end = start + (1 << entry.bits);
                    for next in &mut covered[start..end] {
                        assert!(!*next);
                        *next = true;
                    }
                }
            }
            assert_eq!(occurrences, count.unsigned_abs() as usize);
            assert!(covered.iter().all(|&covered| covered == (count != 0)));
        }
    }

    #[test]
    fn weight_stream_emits_terminal_pair() {
        // The header describes counts [16, 16]. Initial states 0 and 3
        // select symbols 0 and 1, with no transition bits before the marker.
        assert_eq!(decode_weights(&[0x10, 0x3f, 3, 4]).unwrap(), [0, 1]);
        // Two zero transition bits precede the initial states. Both states
        // become zero before the final pair is emitted.
        assert_eq!(decode_weights(&[0x10, 0x3f, 12, 16]).unwrap(), [0, 1, 0, 0]);
    }

    #[test]
    fn weight_stream_rejects_missing_initial_states_and_marker() {
        for input in [
            &[0x10, 0x3f][..],
            &[0x10, 0x3f, 0][..],
            &[0x10, 0x3f, 1][..],
            &[0x10, 0x3f, 0, 1][..],
        ] {
            assert!(decode_weights(input).is_err(), "input: {input:?}");
        }
    }

    #[test]
    fn weight_output_is_bounded_at_255_symbols() {
        // Each transition consumes one bit. The two terminal symbols make
        // this 254-bit transition payload exceed the weight alphabet limit.
        let mut data = alloc::vec![0x10, 0x3f];
        data.extend_from_slice(&[0; 33]);
        data.push(1);
        assert_eq!(decode_weights(&data), Err(Error::CorruptData));
    }

    #[test]
    fn rejects_invalid_tables_and_states() {
        assert!(FseTable::from_normalized(&[32], 5).is_err());
        assert!(FseTable::from_normalized(&[17, 16], 5).is_err());
        assert!(FseTable::from_normalized(&[-2, 30], 5).is_err());
        assert!(FseTable::from_normalized(&[16, 16], 4).is_err());
        let table = FseTable::rle(7);
        assert_eq!(table.table_log(), 0);
        assert_eq!(
            table.entry(0),
            Ok(FseEntry {
                symbol: 7,
                bits: 0,
                base: 0
            })
        );
        assert_eq!(table.entry(1), Err(Error::CorruptData));
        assert!(FseTable::read(&[], 255, 6).is_err());
        assert!(FseTable::read(&[0x10], 1, 5).is_err());
        assert!(FseTable::read(&[0x10, 0x3f], 0, 5).is_err());
        assert!(FseTable::read(&[0x0f], 255, 6).is_err());
    }
}
