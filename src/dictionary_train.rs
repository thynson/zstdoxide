//! Bounded formatted dictionary construction from samples and explicit content.
//!
//! Literal frequencies are learned from proportional sample prefixes. Sequence
//! distributions initially use the attributed predefined priors. This is an
//! independent deterministic builder, not a reproduction of ZDICT's entropy
//! optimization, COVER, fastCover, or their compression-level parameters.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::constants::{LL_DEFAULT, ML_DEFAULT, OF_DEFAULT};
use crate::entropy_encode::write_normalized_counts_in;
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

/// Select content and construct a bounded formatted dictionary from samples.
///
/// The complete serialized dictionary, including entropy metadata, fits the
/// requested selection dictionary_size. Selected samples and content are never
/// borrowed by the returned object. Empty or insufficient samples return
/// [`Error::TrainingFailed`]; invalid options return [`Error::InvalidParameter`].
///
/// This fulfills the dictionary-building role of upstream `ZDICT_trainFromBuffer`
/// using independent greedy content selection, learned literal frequencies, and
/// predefined sequence priors. COVER/fastCover and their parameter matrices are
/// not implemented by this API.
pub fn train_dictionary(
    samples: &[&[u8]],
    options: DictionaryTrainingOptions,
) -> Result<Dictionary> {
    train_dictionary_in(samples, options, Global)
}

/// Train with explicit ownership for all raw-selection, entropy, serialization,
/// and validation storage. The final dictionary retains the original allocator
/// for its bytes; fresh clones own all scratch and prepared tables.
/// A borrowed non-Clone allocator is supported. Ordinary Clone may itself
/// allocate or panic; codec collection requests remain fallible.
pub fn train_dictionary_in<A: Allocator + Clone>(
    samples: &[&[u8]],
    options: DictionaryTrainingOptions,
    allocator: A,
) -> Result<Dictionary<A>> {
    let options = options.validate()?;
    let content =
        crate::training::train_raw_dictionary_in(samples, options.selection, allocator.clone())?;
    finalize_dictionary_in(content.content(), samples, options, allocator)
}

/// Build a formatted dictionary from explicit content and bounded samples.
///
/// Literal statistics include every byte in the proportional sample prefixes,
/// even from samples shorter than eight bytes. Empty samples are valid and
/// produce a full-alphabet pseudocount tree. Sequence tables use predefined
/// priors; this does not reproduce upstream compressed-sequence profiling.
///
/// Metadata consumes part of selection.dictionary_size. Content is trimmed from
/// its front to preserve the suffix; fewer than eight content bytes receive
/// leading zero padding for initial repeat offsets `[1, 4, 8]`. Automatic IDs
/// hash the original untrimmed, unpadded content. Allocation is fallible, and the
/// result is revalidated by the normal formatted dictionary parser.
/// Corresponds to the construction contract of `ZDICT_finalizeDictionary`.
pub fn finalize_dictionary(
    content: &[u8],
    samples: &[&[u8]],
    options: DictionaryTrainingOptions,
) -> Result<Dictionary> {
    finalize_dictionary_in(content, samples, options, Global)
}

/// Finalize through the chosen allocator family, including the validation copy.
/// The returned dictionary owns its bytes independently of content and samples.
/// Failure releases all partial owners; no partially validated object escapes.
/// See [`train_dictionary_in`] for allocator clone semantics.
pub fn finalize_dictionary_in<A: Allocator + Clone>(
    content: &[u8],
    samples: &[&[u8]],
    options: DictionaryTrainingOptions,
    allocator: A,
) -> Result<Dictionary<A>> {
    let options = options.validate()?;
    let observed = literal_histogram(samples, options.selection.max_sample_bytes)?;
    let literal_table =
        crate::huffman_encode::serialize_literal_table_in(&observed, allocator.clone())?;
    assemble_dictionary_in(content, &literal_table, options, allocator)
}

/// Assemble metadata and a preserved content suffix into a validated object.
/// The literal description is computed by the tree serializer at the call site;
/// the attributed wire-format sequence priors come from constants.rs.
#[cfg(test)]
fn assemble_dictionary(
    content: &[u8],
    literal_table: &[u8],
    options: DictionaryTrainingOptions,
) -> Result<Dictionary> {
    assemble_dictionary_in(content, literal_table, options, Global)
}

fn assemble_dictionary_in<A: Allocator + Clone>(
    content: &[u8],
    literal_table: &[u8],
    options: DictionaryTrainingOptions,
    allocator: A,
) -> Result<Dictionary<A>> {
    let options = options.validate()?;
    let offsets = write_normalized_counts_in(&OF_DEFAULT, 5, allocator.clone())?;
    let matches = write_normalized_counts_in(&ML_DEFAULT, 6, allocator.clone())?;
    let literals = write_normalized_counts_in(&LL_DEFAULT, 6, allocator.clone())?;
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
    let mut bytes = Vec::new_in(allocator.clone());
    bytes
        .try_reserve_exact(size)
        .map_err(|_| Error::AllocationFailed)?;
    for byte in 0xec30_a437_u32
        .to_le_bytes()
        .into_iter()
        .chain(id.to_le_bytes())
    {
        bytes.push(byte);
    }
    for table in descriptions {
        for &byte in table {
            bytes.push(byte);
        }
    }
    for distance in [1_u32, 4, 8] {
        for byte in distance.to_le_bytes() {
            bytes.push(byte);
        }
    }
    while bytes.len() < metadata_size + padded - retained {
        bytes.push(0);
    }
    for &byte in &content[content.len() - retained..] {
        bytes.push(byte);
    }
    Dictionary::from_bytes_with_mode_in(&bytes, DictionaryMode::Formatted, allocator)
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
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 32,
            fail_at: None,
        })
        .unwrap()
    }

    fn options() -> DictionaryTrainingOptions {
        DictionaryTrainingOptions {
            selection: RawTrainingOptions {
                dictionary_size: 256,
                max_sample_bytes: 1024,
                segment_size: 16,
            },
            dictionary_id: None,
        }
    }

    #[test]
    fn allocator_formatted_bytes_preserve_the_pre_migration_baseline() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let content: Vec<_> = (0..1024).map(|index| (index * 37) as u8).collect();
        let samples = [
            &b"short"[..],
            &b"several repeated symbols aaaabbbbccccdddd"[..],
            &b"aaaabbbbccccddddeeee repeated suffix"[..],
        ];
        let mut fingerprint = 0xcbf29ce484222325_u64;
        let mut cases = 0;
        let mut add = |bytes: &[u8]| {
            for &byte in (bytes.len() as u32).to_le_bytes().iter().chain(bytes) {
                fingerprint = (fingerprint ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
            cases += 1;
        };
        for capacity in [256, 512] {
            for id in [None, Some(u32::MAX)] {
                let options = DictionaryTrainingOptions {
                    selection: RawTrainingOptions {
                        dictionary_size: capacity,
                        ..options().selection
                    },
                    dictionary_id: id,
                };
                for length in [0, 1, 7, 8, 9, 1024] {
                    for samples in [&[][..], &samples[..]] {
                        let actual =
                            finalize_dictionary_in(&content[..length], samples, options, &owner)
                                .unwrap();
                        let expected =
                            finalize_dictionary(&content[..length], samples, options).unwrap();
                        assert_eq!(actual.as_bytes(), expected.as_bytes());
                        assert_eq!(actual.id(), expected.id());
                        assert_eq!(actual.content(), expected.content());
                        add(actual.as_bytes());
                    }
                }
                for budget in [512, 1024] {
                    let options = DictionaryTrainingOptions {
                        selection: RawTrainingOptions {
                            max_sample_bytes: budget,
                            ..options.selection
                        },
                        ..options
                    };
                    let actual = train_dictionary_in(&samples, options, &owner).unwrap();
                    let expected = train_dictionary(&samples, options).unwrap();
                    assert_eq!(actual.as_bytes(), expected.as_bytes());
                    add(actual.as_bytes());
                }
            }
        }
        assert_eq!(cases, 56);
        // Captured from the dedicated queue-checkpoint library before formatted
        // allocator migration: complete descriptions, IDs, padding and suffixes.
        assert_eq!(fingerprint, 14_787_186_922_959_212_481);
        assert_eq!(monitor.snapshot().identities_created, 1);
        monitor.assert_released();
    }

    #[test]
    fn allocator_formatted_result_moves_with_its_five_independent_owners() {
        let content = b"separate explicit content".to_vec();
        let samples = [
            b"first observed sample".to_vec(),
            b"second observed sample".to_vec(),
        ];
        let views: Vec<_> = samples.iter().map(Vec::as_slice).collect();
        let expected = finalize_dictionary(&content, &views, options()).unwrap();
        let owner = allocator();
        let monitor = owner.monitor();
        let actual = finalize_dictionary_in(&content, &views, options(), owner).unwrap();
        assert_eq!(actual.as_bytes(), expected.as_bytes());
        // Only serialized bytes and four prepared decoding tables survive.
        // Every construction/validation temporary has released its own domain.
        assert_eq!(monitor.snapshot().live_blocks, 5);
        assert_eq!(
            monitor.snapshot().identities_created - monitor.snapshot().owners_dropped,
            5
        );
        assert!(monitor.snapshot().identities_created > 5);
        drop(content);
        drop(views);
        drop(samples);
        let pointer = actual.as_bytes().as_ptr();
        let moved = actual;
        assert_eq!(moved.as_bytes().as_ptr(), pointer);
        assert_eq!(moved.as_bytes(), expected.as_bytes());
        let entropy = moved.entropy().unwrap();
        assert_eq!(entropy.repeat, [1, 4, 8]);
        assert_eq!(entropy.offsets.table_log(), 5);
        assert_eq!(entropy.match_lengths.table_log(), 6);
        assert_eq!(entropy.literal_lengths.table_log(), 6);
        drop(moved);
        assert_eq!(
            monitor.snapshot().identities_created,
            monitor.snapshot().owners_dropped
        );
        monitor.assert_released();
    }

    #[test]
    fn allocator_trained_result_releases_raw_content_before_returning() {
        let samples = [&b"abcdefghabcdefgh"[..]];
        let owner = allocator();
        let monitor = owner.monitor();
        let dictionary = train_dictionary_in(&samples, options(), owner).unwrap();
        assert_eq!(dictionary.content(), samples[0]);
        let expected = finalize_dictionary(samples[0], &samples, options()).unwrap();
        assert_eq!(dictionary.as_bytes(), expected.as_bytes());
        // The raw content owner and its four scratch domains do not remain
        // reachable after finalization; only the final dictionary's five do.
        assert_eq!(monitor.snapshot().live_blocks, 5);
        assert_eq!(
            monitor.snapshot().identities_created - monitor.snapshot().owners_dropped,
            5
        );
        drop(dictionary);
        monitor.assert_released();
    }

    #[test]
    fn allocator_invalid_options_and_empty_training_fail_before_storage_requests() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        for options in [
            DictionaryTrainingOptions {
                dictionary_id: Some(0),
                ..options()
            },
            DictionaryTrainingOptions {
                selection: RawTrainingOptions {
                    dictionary_size: 255,
                    ..options().selection
                },
                ..options()
            },
        ] {
            assert!(matches!(
                finalize_dictionary_in(b"content", &[b"sample"], options, &owner),
                Err(Error::InvalidParameter)
            ));
            assert!(matches!(
                train_dictionary_in(&[b"long enough sample"], options, &owner),
                Err(Error::InvalidParameter)
            ));
            assert!(matches!(
                assemble_dictionary_in(&[], &[128, 0x10], options, &owner),
                Err(Error::InvalidParameter)
            ));
        }
        assert!(matches!(
            train_dictionary_in(&[b"short"], options(), &owner),
            Err(Error::TrainingFailed)
        ));
        assert_eq!(monitor.snapshot().attempts, 0);
        monitor.assert_released();
    }

    #[test]
    fn allocator_malformed_assembly_and_metadata_limit_release_temporary_domains() {
        for literal_table in [&[0][..], &[128, 0][..], &[255, 0][..]] {
            let owner = allocator();
            let monitor = owner.monitor();
            assert!(matches!(
                assemble_dictionary_in(b"content bytes", literal_table, options(), owner),
                Err(Error::InvalidDictionary)
            ));
            assert!(monitor.snapshot().attempts >= 4);
            assert_eq!(
                monitor.snapshot().identities_created,
                monitor.snapshot().owners_dropped
            );
            monitor.assert_released();
        }
        let owner = allocator();
        let monitor = owner.monitor();
        assert!(matches!(
            assemble_dictionary_in(&[], &[0; 256], options(), owner),
            Err(Error::ResourceLimit)
        ));
        monitor.assert_released();
    }

    fn check_every_failure(train: bool) {
        let content: Vec<_> = (0..1024).map(|index| (index * 37) as u8).collect();
        let samples = [
            &b"aaaabbbbccccdddd first sample"[..],
            &b"aaaabbbbcccceeee second sample"[..],
        ];
        let expected = if train {
            train_dictionary(&samples, options())
        } else {
            finalize_dictionary(&content, &samples, options())
        }
        .unwrap();
        let run = |owner| {
            if train {
                train_dictionary_in(&samples, options(), owner)
            } else {
                finalize_dictionary_in(&content, &samples, options(), owner)
            }
        };
        let owner = allocator();
        let monitor = owner.monitor();
        let baseline = run(owner).unwrap();
        let attempts = monitor.snapshot().attempts;
        assert!(attempts > 10);
        assert_eq!(baseline.as_bytes(), expected.as_bytes());
        drop(baseline);
        monitor.assert_released();
        for ordinal in 1..=attempts {
            let owner = allocator();
            let monitor = owner.monitor();
            monitor.set_fail_at(NonZeroU64::new(ordinal));
            assert!(
                matches!(run(owner), Err(Error::AllocationFailed)),
                "allocation {ordinal}, train {train}"
            );
            assert_eq!(monitor.snapshot().failures, 1);
            assert_eq!(
                monitor.snapshot().identities_created,
                monitor.snapshot().owners_dropped
            );
            monitor.assert_released();
        }
        // Reusing unchanged inputs after the complete failure campaign produces
        // the same validated object; no temporary publication affects selection.
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let recovered = if train {
            train_dictionary_in(&samples, options(), &owner)
        } else {
            finalize_dictionary_in(&content, &samples, options(), &owner)
        }
        .unwrap();
        assert_eq!(recovered.as_bytes(), expected.as_bytes());
        drop(recovered);
        monitor.assert_released();
    }

    #[test]
    fn allocator_every_finalization_failure_releases_partial_validation_and_output() {
        check_every_failure(false);
    }

    #[test]
    fn allocator_every_train_then_finalize_failure_releases_raw_and_formatted_owners() {
        check_every_failure(true);
    }

    #[test]
    fn allocator_final_validation_copy_failure_releases_all_prepared_tables() {
        let owner = allocator();
        let monitor = owner.monitor();
        let result =
            assemble_dictionary_in(b"content bytes", &[128, 0x10], options(), owner).unwrap();
        let final_request = monitor.snapshot().attempts;
        assert_eq!(monitor.snapshot().live_blocks, 5);
        drop(result);
        monitor.assert_released();
        let owner = allocator();
        let monitor = owner.monitor();
        monitor.set_fail_at(NonZeroU64::new(final_request));
        assert!(matches!(
            assemble_dictionary_in(b"content bytes", &[128, 0x10], options(), owner),
            Err(Error::AllocationFailed)
        ));
        assert_eq!(monitor.snapshot().failures, 1);
        assert_eq!(
            monitor.snapshot().identities_created,
            monitor.snapshot().owners_dropped
        );
        monitor.assert_released();
    }

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
