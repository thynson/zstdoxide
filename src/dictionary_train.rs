//! Bounded formatted dictionary construction from samples and explicit content.
//!
//! Literal frequencies are learned from proportional sample prefixes. Sequence
//! distributions initially use the attributed predefined priors. This is an
//! independent deterministic builder, not a reproduction of ZDICT's entropy
//! optimization, COVER, fastCover, or their compression-level parameters.

use crate::training::visit_sample_prefixes;
use crate::{Error, RawTrainingOptions, Result};

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
