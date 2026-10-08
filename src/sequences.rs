//! FSE sequence decoding and checked repeat-offset execution.

use crate::bitstream::ReverseBits;
use crate::constants::{LL_BASE, LL_BITS, LL_DEFAULT, ML_BASE, ML_BITS, ML_DEFAULT, OF_DEFAULT};
use crate::cursor::ByteCursor;
use crate::dictionary::DictionaryEntropy;
use crate::fse::FseTable;
use crate::history::SequenceOutput;
use crate::{Error, Result};

pub(crate) struct SequenceState {
    literal_lengths: Option<FseTable>,
    offsets: Option<FseTable>,
    match_lengths: Option<FseTable>,
    repeat: [usize; 3],
}

impl SequenceState {
    pub(crate) fn retained_capacity(&self) -> usize {
        [&self.literal_lengths, &self.offsets, &self.match_lengths]
            .into_iter()
            .filter_map(Option::as_ref)
            .map(FseTable::retained_capacity)
            .sum()
    }

    pub(crate) fn new() -> Self {
        Self::with_repeat([1, 4, 8])
    }

    pub(crate) fn with_repeat(repeat: [usize; 3]) -> Self {
        Self {
            literal_lengths: None,
            offsets: None,
            match_lengths: None,
            repeat,
        }
    }

    /// Decode and execute one sequence section, including trailing literals.
    ///
    /// Corresponds to `ZSTD_decodeSeqHeaders`, `ZSTD_decodeSequence`, and
    /// `ZSTD_execSequence`, with indices replacing C pointer state.
    pub(crate) fn decode<O: SequenceOutput>(
        &mut self,
        input: &[u8],
        literals: &[u8],
        history: &mut O,
        max_block: usize,
    ) -> Result<()> {
        self.decode_with_dictionary(input, literals, history, max_block, None)
    }

    /// Use immutable prepared tables until a block overrides each distribution.
    pub(crate) fn decode_with_dictionary<O: SequenceOutput>(
        &mut self,
        input: &[u8],
        literals: &[u8],
        history: &mut O,
        max_block: usize,
        initial: Option<&DictionaryEntropy>,
    ) -> Result<()> {
        let mut cursor = ByteCursor::new(input);
        let count = match cursor.byte()? {
            byte @ 0..=127 => usize::from(byte),
            byte @ 128..=254 => (usize::from(byte - 128) << 8) + usize::from(cursor.byte()?),
            _ => cursor.little_endian(2)? as usize + 0x7f00,
        };
        if literals.len() > max_block || count > max_block / 3 {
            return Err(Error::CorruptData);
        }
        if count == 0 {
            if !cursor.remaining().is_empty() {
                return Err(Error::CorruptData);
            }
            return history.append(literals);
        }
        let modes = cursor.byte()?;
        if modes & 3 != 0 {
            return Err(Error::CorruptData);
        }
        read_table(
            &mut cursor,
            modes >> 6,
            &mut self.literal_lengths,
            initial.map(|tables| &tables.literal_lengths),
            &LL_DEFAULT,
            6,
            (35, 9),
        )?;
        read_table(
            &mut cursor,
            (modes >> 4) & 3,
            &mut self.offsets,
            initial.map(|tables| &tables.offsets),
            &OF_DEFAULT,
            5,
            (31, 8),
        )?;
        read_table(
            &mut cursor,
            (modes >> 2) & 3,
            &mut self.match_lengths,
            initial.map(|tables| &tables.match_lengths),
            &ML_DEFAULT,
            6,
            (52, 9),
        )?;
        let ll = self
            .literal_lengths
            .as_ref()
            .or(initial.map(|tables| &tables.literal_lengths))
            .ok_or(Error::CorruptData)?;
        let of = self
            .offsets
            .as_ref()
            .or(initial.map(|tables| &tables.offsets))
            .ok_or(Error::CorruptData)?;
        let ml = self
            .match_lengths
            .as_ref()
            .or(initial.map(|tables| &tables.match_lengths))
            .ok_or(Error::CorruptData)?;
        let mut bits = ReverseBits::new(cursor.remaining())?;
        let mut literal_state = bits.read_bits(ll.table_log())? as usize;
        let mut offset_state = bits.read_bits(of.table_log())? as usize;
        let mut match_state = bits.read_bits(ml.table_log())? as usize;
        let mut literal_position = 0;
        let block_start = history.len();
        for index in 0..count {
            let literal_entry = ll.entry(literal_state)?;
            let offset_entry = of.entry(offset_state)?;
            let match_entry = ml.entry(match_state)?;
            let literal_code = usize::from(literal_entry.symbol);
            let match_code = usize::from(match_entry.symbol);
            if literal_code >= LL_BASE.len()
                || match_code >= ML_BASE.len()
                || offset_entry.symbol > 31
            {
                return Err(Error::CorruptData);
            }
            let offset_value =
                (1_u32 << offset_entry.symbol) + bits.read_bits(offset_entry.symbol)?;
            let match_length =
                (ML_BASE[match_code] + bits.read_bits(ML_BITS[match_code])?) as usize;
            let literal_length =
                (LL_BASE[literal_code] + bits.read_bits(LL_BITS[literal_code])?) as usize;
            let end = literal_position + literal_length;
            let pending = literals
                .get(literal_position..end)
                .ok_or(Error::CorruptData)?;
            let generated = history.len() - block_start;
            if literal_length + match_length > max_block - generated {
                return Err(Error::CorruptData);
            }
            let offset = resolve_offset(&mut self.repeat, offset_value as usize, literal_length)?;
            history.append(pending)?;
            history.copy_match(offset, match_length)?;
            literal_position = end;
            if index + 1 < count {
                literal_state =
                    usize::from(literal_entry.base) + bits.read_bits(literal_entry.bits)? as usize;
                match_state =
                    usize::from(match_entry.base) + bits.read_bits(match_entry.bits)? as usize;
                offset_state =
                    usize::from(offset_entry.base) + bits.read_bits(offset_entry.bits)? as usize;
            }
        }
        if bits.remaining() != 0 {
            return Err(Error::CorruptData);
        }
        let trailing = &literals[literal_position..];
        if trailing.len() > max_block - (history.len() - block_start) {
            return Err(Error::CorruptData);
        }
        history.append(trailing)
    }
}

fn read_table(
    cursor: &mut ByteCursor<'_>,
    mode: u8,
    previous: &mut Option<FseTable>,
    initial: Option<&FseTable>,
    default: &[i16],
    default_log: u8,
    limits: (usize, u8),
) -> Result<()> {
    let (max_symbol, max_log) = limits;
    let table = match mode {
        0 => FseTable::from_normalized(default, default_log)?,
        1 => {
            let symbol = cursor.byte()?;
            if usize::from(symbol) > max_symbol {
                return Err(Error::CorruptData);
            }
            FseTable::rle(symbol)
        }
        2 => {
            let (table, consumed) = FseTable::read(cursor.remaining(), max_symbol, max_log)?;
            cursor.take(consumed)?;
            table
        }
        3 => {
            return previous
                .as_ref()
                .or(initial)
                .map(|_| ())
                .ok_or(Error::CorruptData);
        }
        _ => return Err(Error::CorruptData),
    };
    *previous = Some(table);
    Ok(())
}

fn resolve_offset(repeat: &mut [usize; 3], value: usize, literal_length: usize) -> Result<usize> {
    if value > 3 {
        let offset = value - 3;
        *repeat = [offset, repeat[0], repeat[1]];
        return Ok(offset);
    }
    let index = value - 1 + usize::from(literal_length == 0);
    let offset = match index {
        0..=2 => repeat[index],
        3 => repeat[0]
            .checked_sub(1)
            .filter(|&n| n != 0)
            .ok_or(Error::CorruptData)?,
        _ => return Err(Error::CorruptData),
    };
    match index {
        0 => {}
        1 => repeat.swap(0, 1),
        _ => *repeat = [offset, repeat[0], repeat[1]],
    }
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::History;

    #[test]
    fn prepared_tables_and_repcodes_are_borrowed_until_each_table_is_replaced() {
        let entropy = DictionaryEntropy {
            huffman: crate::huffman::HuffmanTable::read(&[128, 0x10]).unwrap().0,
            offsets: FseTable::rle(0),
            literal_lengths: FseTable::rle(0),
            match_lengths: FseTable::rle(0),
            repeat: [8, 4, 1],
        };
        let mut state = SequenceState::with_repeat(entropy.repeat);
        let mut output = History::new(32);
        output.start_frame(32);
        output.append(b"abcdefgh").unwrap();
        state
            .decode_with_dictionary(&[1, 0xfc, 1], b"", &mut output, 32, Some(&entropy))
            .unwrap();
        assert!(state.offsets.is_none());
        assert_eq!(state.repeat, [4, 8, 1]);
        // Replace only OF with an explicit distance-one code, retaining LL/ML.
        state
            .decode_with_dictionary(&[1, 0xdc, 2, 4], b"", &mut output, 32, Some(&entropy))
            .unwrap();
        state
            .decode_with_dictionary(&[1, 0xfc, 4], b"", &mut output, 32, Some(&entropy))
            .unwrap();
        assert!(state.offsets.is_some());
        assert!(state.literal_lengths.is_none());
        assert!(state.match_lengths.is_none());
        assert_eq!(entropy.repeat, [8, 4, 1]);
        assert_eq!(output.into_bytes(), b"abcdefghefggggggg");
    }

    #[test]
    fn executes_rle_symbols_and_overlapping_matches() {
        let mut output = History::new(32);
        output.start_frame(32);
        SequenceState::new()
            .decode(&[1, 0x54, 3, 2, 0, 6], b"abc!", &mut output, 32)
            .unwrap();
        assert_eq!(output.into_bytes(), b"abcabc!");
        let mut output = History::new(32);
        output.start_frame(32);
        SequenceState::new()
            .decode(&[2, 0x54, 1, 2, 0, 16], b"ab", &mut output, 32)
            .unwrap();
        assert_eq!(output.into_bytes(), b"aaaabbbb");
    }

    #[test]
    fn repeat_offsets_follow_specification_examples() {
        let mut repeat = [1, 4, 8];
        for (value, literals, expected) in [
            (1114, 11, [1111, 1, 4]),
            (1, 22, [1111, 1, 4]),
            (2225, 22, [2222, 1111, 1]),
            (1114, 111, [1111, 2222, 1111]),
            (3336, 33, [3333, 1111, 2222]),
            (2, 22, [1111, 3333, 2222]),
            (3, 33, [2222, 1111, 3333]),
            (3, 0, [2221, 2222, 1111]),
            (1, 0, [2222, 2221, 1111]),
        ] {
            resolve_offset(&mut repeat, value, literals).unwrap();
            assert_eq!(repeat, expected);
        }
        assert_eq!(
            resolve_offset(&mut [1, 4, 8], 3, 0),
            Err(Error::CorruptData)
        );
    }

    #[test]
    fn rejects_invalid_modes_extra_bits_and_missing_history() {
        for input in [
            &[1, 0xfc, 1][..],
            &[0, 0][..],
            &[1, 0x57][..],
            &[1, 0x54, 0, 2, 0, 6][..],
            &[1, 0x54, 3, 2, 0, 14][..],
        ] {
            let mut output = History::new(32);
            output.start_frame(32);
            assert!(
                SequenceState::new()
                    .decode(input, b"abc", &mut output, 32)
                    .is_err()
            );
        }
    }
}
