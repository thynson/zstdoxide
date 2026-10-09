//! Checked canonical Huffman codes paired with exact serialized descriptions.
//!
//! Codewords are plain initialized values. The codebook owns its fixed alphabet
//! inline; it stores neither source pointers nor references into decoding tables.

use alloc::vec::Vec;

use crate::huffman::HuffmanTable;
use crate::{Error, Result};

const SYMBOLS: usize = 256;
const DICTIONARY_MAX_LOG: u8 = 12;

/// An ordinary numeric codeword, shared by every future accessor backend.
/// A zero bit count is the internal representation of an absent symbol.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Codeword {
    pub(crate) value: u16,
    pub(crate) bits: u8,
}

/// A complete canonical prefix code with explicit absent-symbol entries.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HuffmanCodebook {
    entries: [Codeword; SYMBOLS],
    table_log: u8,
}

impl HuffmanCodebook {
    /// Assign codes by ascending weight and then ascending symbol.
    ///
    /// Corresponds to the canonical assignment in `HUF_readCTable` and
    /// `HUF_buildCTableFromTree`. Zero lengths represent absent symbols; the
    /// nonzero lengths must have complete Kraft mass and at least two symbols.
    /// The explicit limit distinguishes new inline trees (eleven) from reused
    /// prepared dictionary trees (twelve). Construction allocates no heap storage.
    pub(crate) fn from_lengths(lengths: &[u8; SYMBOLS], maximum_log: u8) -> Result<Self> {
        if !(1..=DICTIONARY_MAX_LOG).contains(&maximum_log) {
            return Err(Error::InvalidParameter);
        }
        let table_log = *lengths.iter().max().ok_or(Error::InvalidParameter)?;
        if table_log == 0 || table_log > maximum_log {
            return Err(Error::InvalidParameter);
        }
        let table_size = 1_u32 << table_log;
        let mut mass = 0_u32;
        let mut present = 0;
        for &bits in lengths {
            if bits != 0 {
                // table_log is the validated maximum, so the shift is 0..12.
                mass = mass
                    .checked_add(1 << (table_log - bits))
                    .filter(|&value| value <= table_size)
                    .ok_or(Error::InvalidParameter)?;
                present += 1;
            }
        }
        if present < 2 || mass != table_size {
            return Err(Error::InvalidParameter);
        }

        let mut entries = [Codeword::default(); SYMBOLS];
        let mut rank_start = 0_u32;
        for bits in (1..=table_log).rev() {
            let copies = 1_u32 << (table_log - bits);
            for (symbol, &length) in lengths.iter().enumerate() {
                if length != bits {
                    continue;
                }
                if rank_start % copies != 0 {
                    return Err(Error::InvalidParameter);
                }
                let value = rank_start / copies;
                if value >= 1 << bits {
                    return Err(Error::InvalidParameter);
                }
                entries[symbol] = Codeword {
                    // value is below 2^bits, and bits is at most twelve.
                    value: value as u16,
                    bits,
                };
                rank_start = rank_start
                    .checked_add(copies)
                    .filter(|&value| value <= table_size)
                    .ok_or(Error::InvalidParameter)?;
            }
        }
        if rank_start != table_size {
            return Err(Error::InvalidParameter);
        }
        Ok(Self { entries, table_log })
    }

    /// Copy the canonical relation from a validated immutable decoding table.
    /// No borrow survives the call; dictionary tables may retain twelve-bit codes.
    pub(crate) fn from_decoding_table(table: &HuffmanTable) -> Result<Self> {
        Self::from_lengths(&table.code_lengths(), DICTIONARY_MAX_LOG)
    }

    pub(crate) fn table_log(&self) -> u8 {
        self.table_log
    }

    /// Return only a present symbol's code. Missing symbols cannot emit zero bits.
    pub(crate) fn code(&self, symbol: u8) -> Result<Codeword> {
        self.entries
            .get(usize::from(symbol))
            .copied()
            .filter(|entry| entry.bits != 0)
            .ok_or(Error::CorruptData)
    }

    /// Test table reuse eligibility without modifying the codebook or input.
    pub(crate) fn covers(&self, input: &[u8]) -> bool {
        input.iter().all(|&symbol| self.code(symbol).is_ok())
    }
}

/// A new inline table and the exact bytes that establish its codes for decoding.
/// Private fields prevent replacement of one half without revalidating the pair.
#[derive(Debug)]
pub(crate) struct BuiltLiteralTable {
    codebook: HuffmanCodebook,
    description: Vec<u8>,
}

impl BuiltLiteralTable {
    /// Validate and own one complete inline description, with no trailing bytes.
    ///
    /// Reconstructing from the selected description keeps fallback trees paired
    /// with their actual codes. `HuffmanTable::read` enforces the inline limit of
    /// eleven bits and performs fallible setup allocation. Dictionary reuse uses
    /// `HuffmanCodebook::from_decoding_table` instead of publishing a new header.
    pub(crate) fn from_description(description: Vec<u8>) -> Result<Self> {
        let (table, consumed) = HuffmanTable::read(&description)?;
        if consumed != description.len() {
            return Err(Error::CorruptData);
        }
        let codebook = HuffmanCodebook::from_decoding_table(&table)?;
        Ok(Self {
            codebook,
            description,
        })
    }

    pub(crate) fn codebook(&self) -> &HuffmanCodebook {
        &self.codebook
    }

    pub(crate) fn description(&self) -> &[u8] {
        &self.description
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn canonical_codes_match_the_format_example_and_symbol_order() {
        let mut lengths = [0; SYMBOLS];
        lengths[..6].copy_from_slice(&[1, 2, 3, 0, 4, 4]);
        let codes = HuffmanCodebook::from_lengths(&lengths, 11).unwrap();
        assert_eq!(codes.table_log(), 4);
        for (symbol, value, bits) in [(0, 1, 1), (1, 1, 2), (2, 1, 3), (4, 0, 4), (5, 1, 4)] {
            assert_eq!(codes.code(symbol), Ok(Codeword { value, bits }));
        }
        assert_eq!(codes.code(3), Err(Error::CorruptData));
        assert_eq!(codes.code(255), Err(Error::CorruptData));

        let flat = HuffmanCodebook::from_lengths(&[8; SYMBOLS], 11).unwrap();
        for symbol in 0..=255 {
            assert_eq!(
                flat.code(symbol),
                Ok(Codeword {
                    value: u16::from(symbol),
                    bits: 8
                })
            );
        }
    }

    #[test]
    fn sparse_codebooks_reject_absent_symbols_and_cover_the_extreme_indices() {
        let mut lengths = [0; SYMBOLS];
        lengths[0] = 1;
        lengths[255] = 1;
        let codes = HuffmanCodebook::from_lengths(&lengths, 1).unwrap();
        assert_eq!(codes.code(0), Ok(Codeword { value: 0, bits: 1 }));
        assert_eq!(codes.code(255), Ok(Codeword { value: 1, bits: 1 }));
        assert!(codes.covers(&[]));
        assert!(codes.covers(&[0, 255, 255, 0]));
        for symbol in 1..255 {
            assert_eq!(codes.code(symbol), Err(Error::CorruptData));
            assert!(!codes.covers(&[0, symbol, 255]));
        }
    }

    #[test]
    fn invalid_lengths_limits_and_incomplete_or_excess_mass_are_rejected() {
        for limit in [0, 13, u8::MAX] {
            assert_eq!(
                HuffmanCodebook::from_lengths(&[8; SYMBOLS], limit).unwrap_err(),
                Error::InvalidParameter
            );
        }
        for prefix in [
            &[][..],
            &[1][..],
            &[2, 2][..],
            &[1, 1, 1][..],
            &[13, 13][..],
            &[255, 1][..],
        ] {
            let mut lengths = [0; SYMBOLS];
            lengths[..prefix.len()].copy_from_slice(prefix);
            assert_eq!(
                HuffmanCodebook::from_lengths(&lengths, 12).unwrap_err(),
                Error::InvalidParameter
            );
        }
        let mut deepest = [0; SYMBOLS];
        deepest[..11].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        deepest[11..13].fill(12);
        assert_eq!(
            HuffmanCodebook::from_lengths(&deepest, 11).unwrap_err(),
            Error::InvalidParameter
        );
        assert_eq!(
            HuffmanCodebook::from_lengths(&deepest, 12)
                .unwrap()
                .table_log(),
            12
        );
    }

    #[test]
    fn direct_and_fse_descriptions_reconstruct_exact_sparse_codes() {
        // The format's weights [4,3,2,0,1] leave symbol five's weight implicit.
        let paired = BuiltLiteralTable::from_description(vec![132, 0x43, 0x20, 0x10]).unwrap();
        let mut lengths = [0; SYMBOLS];
        lengths[..6].copy_from_slice(&[1, 2, 3, 0, 4, 4]);
        assert_eq!(
            paired.codebook(),
            &HuffmanCodebook::from_lengths(&lengths, 11).unwrap()
        );
        assert_eq!(paired.description(), &[132, 0x43, 0x20, 0x10]);

        // Independent existing decoder vector: FSE weights [0,1], inferred1.
        let paired = BuiltLiteralTable::from_description(vec![4, 0x10, 0x3f, 3, 4]).unwrap();
        assert_eq!(paired.codebook().code(0), Err(Error::CorruptData));
        assert_eq!(
            paired.codebook().code(1),
            Ok(Codeword { value: 0, bits: 1 })
        );
        assert_eq!(
            paired.codebook().code(2),
            Ok(Codeword { value: 1, bits: 1 })
        );
    }

    #[test]
    fn paired_descriptions_reject_truncation_trailing_bytes_and_invalid_weights() {
        let valid = [132, 0x43, 0x20, 0x10];
        for end in 0..valid.len() {
            assert!(BuiltLiteralTable::from_description(valid[..end].to_vec()).is_err());
        }
        let mut trailing = valid.to_vec();
        trailing.push(0);
        assert_eq!(
            BuiltLiteralTable::from_description(trailing).unwrap_err(),
            Error::CorruptData
        );
        for bytes in [vec![0], vec![128, 0], vec![128, 0xf0]] {
            assert!(BuiltLiteralTable::from_description(bytes).is_err());
        }
    }

    #[test]
    fn official_dictionary_codes_preserve_twelve_bits_and_missing_symbols() {
        let dictionary =
            include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
        let (table, consumed) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        // Independently verified with v1.5.7 HUF_readStats/HUF_readCTable.
        assert_eq!(consumed, 72);
        let codes = HuffmanCodebook::from_decoding_table(&table).unwrap();
        assert_eq!(codes.table_log(), 12);
        assert_eq!(codes.code(0), Ok(Codeword { value: 0, bits: 12 }));
        let missing = [
            192, 193, 242, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254,
        ];
        for symbol in 0..=255 {
            assert_eq!(codes.code(symbol).is_err(), missing.contains(&symbol));
        }
        assert_eq!(
            BuiltLiteralTable::from_description(dictionary[8..80].to_vec()).unwrap_err(),
            Error::CorruptData
        );
    }

    #[test]
    fn learned_table_pairing_uses_the_exact_published_description() {
        let mut observed = [0; SYMBOLS];
        for trial in 0..16 {
            observed[(trial * 17) % SYMBOLS] = 1 << (trial * 3);
            let paired = crate::huffman_encode::build_literal_table(&observed).unwrap();
            let serialized = crate::huffman_encode::serialize_literal_table(&observed).unwrap();
            assert_eq!(paired.description(), serialized);
            let (table, consumed) = HuffmanTable::read(paired.description()).unwrap();
            assert_eq!(consumed, paired.description().len());
            assert_eq!(
                paired.codebook(),
                &HuffmanCodebook::from_decoding_table(&table).unwrap()
            );
            assert!(paired.codebook().covers(&(0..=255).collect::<Vec<_>>()));
        }
    }

    #[test]
    fn paired_fallback_uses_selected_codes_instead_of_discarded_learned_lengths() {
        let mut observed = [0; SYMBOLS];
        observed[0] = 1 << 40;
        let learned = crate::huffman_encode::literal_lengths(&observed).unwrap();
        let selected = crate::huffman_encode::near_flat_lengths(&observed);
        assert_ne!(selected, learned);
        // Explicitly supply the serializer's fallback selection. This checks
        // pairing without assuming a particular histogram reaches its 128-byte
        // cutoff or changing that production cutoff to force the branch.
        let weights = crate::huffman_encode::explicit_weights(&selected).unwrap();
        let payload = crate::entropy_encode::encode_weights(&weights).unwrap();
        assert!((1..128).contains(&payload.len()));
        let mut description = vec![payload.len() as u8];
        description.extend_from_slice(&payload);
        let paired = BuiltLiteralTable::from_description(description).unwrap();
        assert_eq!(
            paired.codebook().code(0),
            Ok(Codeword {
                value: 127,
                bits: 7
            })
        );
        assert_eq!(
            paired.codebook().code(254),
            Ok(Codeword { value: 0, bits: 9 })
        );
        assert_eq!(
            paired.codebook().code(255),
            Ok(Codeword { value: 1, bits: 9 })
        );
        assert_eq!(
            paired.codebook(),
            &HuffmanCodebook::from_lengths(&selected, 11).unwrap()
        );
        assert_ne!(
            paired.codebook(),
            &HuffmanCodebook::from_lengths(&learned, 11).unwrap()
        );
    }
}
