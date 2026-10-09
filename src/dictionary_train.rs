//! Bounded formatted dictionary construction from samples and explicit content.
//!
//! Literal frequencies are learned from proportional sample prefixes. Sequence
//! distributions initially use the attributed predefined priors. This is an
//! independent deterministic builder, not a reproduction of ZDICT's entropy
//! optimization, COVER, fastCover, or their compression-level parameters.

use alloc::vec::Vec;

use crate::constants::{LL_DEFAULT, ML_DEFAULT, OF_DEFAULT};
use crate::entropy_encode::write_normalized_counts;
use crate::training::visit_sample_prefixes;
use crate::{Dictionary, DictionaryMode, Error, RawTrainingOptions, Result, xxhash};

/// Resource limits and optional identifier for a formatted trained dictionary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DictionaryTrainingOptions {
    /// Selection limits; dictionary_size caps the complete serialized object
    /// including metadata and must be at least 256 bytes.
    pub selection: RawTrainingOptions,
    /// A caller-selected nonzero identifier. None derives a deterministic value
    /// in 32768..2^31 from the selected or explicit content before trimming.
    pub dictionary_id: Option<u32>,
}

impl DictionaryTrainingOptions {
    fn validate(self) -> Result<Self> {
        self.selection.validate()?;
        if self.selection.dictionary_size < 256 || self.dictionary_id == Some(0) {
            return Err(Error::InvalidParameter);
        }
        Ok(self)
    }
}

/// Assemble metadata and a preserved content suffix into a validated object.
/// The literal description is computed by the tree serializer at the call site;
/// the attributed wire-format sequence priors come from constants.rs.
fn assemble_dictionary(
    content: &[u8],
    literal_table: &[u8],
    options: DictionaryTrainingOptions,
) -> Result<Dictionary> {
    let options = options.validate()?;
    let offsets = write_normalized_counts(&OF_DEFAULT, 5)?;
    let matches = write_normalized_counts(&ML_DEFAULT, 6)?;
    let literals = write_normalized_counts(&LL_DEFAULT, 6)?;
    let descriptions = [literal_table, &offsets, &matches, &literals];
    let metadata_size = descriptions.iter().try_fold(20_usize, |size, table| {
        // Eight bytes of magic/ID and twelve bytes of initial repeat offsets.
        size.checked_add(table.len()).ok_or(Error::ResourceLimit)
    })?;
    let available = options
        .selection
        .dictionary_size
        .checked_sub(metadata_size)
        .filter(|&remaining| remaining >= 8)
        .ok_or(Error::ResourceLimit)?;
    let retained = content.len().min(available);
    let padded = retained.max(8);
    let size = metadata_size
        .checked_add(padded)
        .ok_or(Error::ResourceLimit)?;
    let id = options.dictionary_id.unwrap_or_else(|| {
        // The public distribution range excludes both reserved end regions.
        // Hash the original content before trimming or padding, as upstream
        // ZDICT_finalizeDictionary does when deriving an identifier.
        (xxhash::checksum(content) % ((1_u64 << 31) - 32768) + 32768) as u32
    });
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| Error::AllocationFailed)?;
    bytes.extend_from_slice(&0xec30_a437_u32.to_le_bytes());
    bytes.extend_from_slice(&id.to_le_bytes());
    for table in descriptions {
        bytes.extend_from_slice(table);
    }
    for distance in [1_u32, 4, 8] {
        bytes.extend_from_slice(&distance.to_le_bytes());
    }
    bytes.resize(metadata_size + padded - retained, 0);
    bytes.extend_from_slice(&content[content.len() - retained..]);
    Dictionary::from_bytes_with_mode(&bytes, DictionaryMode::Formatted)
}

/// Count all selected bytes, including short samples that cannot seed matches.
///
/// The fixed array is a plain initialized value record shared by all backends;
/// there is no independent allocation or artificial unchecked-access variant.
/// Corresponds to literal statistics input to upstream `ZDICT_analyzeEntropy`,
/// using bounded raw byte observations rather than compressed-literal profiling.
fn literal_histogram(samples: &[&[u8]], budget: usize) -> Result<[u64; 256]> {
    let mut observed = [0_u64; 256];
    visit_sample_prefixes(samples, budget, |_, bytes| {
        for &byte in bytes {
            let count = &mut observed[usize::from(byte)];
            *count = count.checked_add(1).ok_or(Error::ResourceLimit)?;
        }
        Ok(())
    })?;
    Ok(observed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembly_preserves_suffix_padding_ids_and_predefined_sequence_tables() {
        // This independent fixture supplies only the assembly test's HUF
        // description. Production finalization constructs its own literal tree.
        let fixture = include_bytes!("../tests/upstream/dictionaries/formatted.dict");
        let (_, length) = crate::huffman::HuffmanTable::read_dictionary(&fixture[8..]).unwrap();
        let literal_table = &fixture[8..8 + length];
        let options = DictionaryTrainingOptions {
            selection: RawTrainingOptions {
                dictionary_size: 256,
                ..RawTrainingOptions::default()
            },
            dictionary_id: Some(1234),
        };
        for length in 0..=8 {
            let bytes = [42; 8];
            let dictionary = assemble_dictionary(&bytes[..length], literal_table, options).unwrap();
            assert_eq!(dictionary.id(), 1234);
            assert!(dictionary.is_formatted());
            assert_eq!(&dictionary.content()[..8 - length], &[0; 8][..8 - length]);
            assert_eq!(&dictionary.content()[8 - length..], &bytes[..length]);
            let entropy = dictionary.entropy().unwrap();
            assert_eq!(entropy.repeat, [1, 4, 8]);
            assert_eq!(entropy.offsets.table_log(), 5);
            assert_eq!(entropy.match_lengths.table_log(), 6);
            assert_eq!(entropy.literal_lengths.table_log(), 6);
        }
        let content: Vec<_> = (0..1024).map(|index| index as u8).collect();
        let dictionary = assemble_dictionary(&content, literal_table, options).unwrap();
        assert_eq!(dictionary.as_bytes().len(), 256);
        assert_eq!(
            dictionary.content(),
            &content[content.len() - dictionary.content().len()..]
        );
        let automatic = DictionaryTrainingOptions {
            dictionary_id: None,
            ..options
        };
        let empty = assemble_dictionary(&[], literal_table, automatic).unwrap();
        // Pinned independent XXH64 empty-input vector, before the eight-byte pad.
        assert_eq!(
            empty.id(),
            (0xef46_db37_51d8_e999_u64 % ((1_u64 << 31) - 32768) + 32768) as u32
        );
        assert_eq!(
            assemble_dictionary(&[], &[0; 256], options).unwrap_err(),
            Error::ResourceLimit
        );
    }

    #[test]
    fn formatted_options_validate_complete_capacity_and_nonzero_explicit_ids() {
        let defaults = DictionaryTrainingOptions::default();
        assert_eq!(defaults.selection, RawTrainingOptions::default());
        assert_eq!(defaults.dictionary_id, None);
        assert_eq!(defaults.validate(), Ok(defaults));
        for options in [
            DictionaryTrainingOptions {
                dictionary_id: Some(0),
                ..defaults
            },
            DictionaryTrainingOptions {
                selection: RawTrainingOptions {
                    dictionary_size: 255,
                    ..defaults.selection
                },
                ..defaults
            },
            DictionaryTrainingOptions {
                selection: RawTrainingOptions {
                    dictionary_size: 512,
                    max_sample_bytes: 256,
                    ..defaults.selection
                },
                ..defaults
            },
            DictionaryTrainingOptions {
                selection: RawTrainingOptions {
                    segment_size: 7,
                    ..defaults.selection
                },
                ..defaults
            },
            DictionaryTrainingOptions {
                selection: RawTrainingOptions {
                    max_sample_bytes: usize::MAX,
                    ..defaults.selection
                },
                ..defaults
            },
        ] {
            // On a 32-bit target, usize::MAX is the valid u32::MAX budget.
            if options.selection.max_sample_bytes as u128 <= u128::from(u32::MAX)
                && options.selection.max_sample_bytes == usize::MAX
            {
                assert_eq!(options.validate(), Ok(options));
            } else {
                assert_eq!(options.validate(), Err(Error::InvalidParameter));
            }
        }
        for dictionary_id in [Some(1), Some(u32::MAX)] {
            let options = DictionaryTrainingOptions {
                dictionary_id,
                ..defaults
            };
            assert_eq!(options.validate(), Ok(options));
        }
    }

    #[test]
    fn literal_histogram_counts_short_samples_and_preserves_exact_prefix_quotas() {
        let samples = [&b"aa"[..], &b"bbb"[..], &b"cccc"[..], &b""[..]];
        let counts = literal_histogram(&samples, 6).unwrap();
        assert_eq!(counts[usize::from(b'a')], 1);
        assert_eq!(counts[usize::from(b'b')], 2);
        assert_eq!(counts[usize::from(b'c')], 2);
        assert_eq!(counts.iter().sum::<u64>(), 5);
        assert_eq!(literal_histogram(&samples, 0).unwrap(), [0; 256]);
        assert_eq!(literal_histogram(&[], 123).unwrap(), [0; 256]);

        let samples = [&b"0123456789abcdef"[..], &b"abcdefghijklmnopqrstuvwx"[..]];
        assert_eq!(
            literal_histogram(&samples, 23).unwrap(),
            literal_histogram(&[&samples[0][..9], &samples[1][..13]], 100).unwrap(),
        );
        let binary: [u8; 256] = core::array::from_fn(|index| index as u8);
        assert_eq!(literal_histogram(&[&binary], 256).unwrap(), [1; 256]);
    }
}
