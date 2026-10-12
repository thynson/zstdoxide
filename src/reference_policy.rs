//! Shared scalar entropy policy for bounded reference search encoding.
//!
//! These values select existing owners; they allocate no storage and need no
//! alternate unsafe representation. Algorithm admission remains the caller's
//! responsibility and is not widened by applying an entropy policy.
#![forbid(unsafe_code)]

use crate::{ReferenceStrategy, ResolvedCompressionParameters};

/// Automatic literal compression is disabled only for accelerated Fast.
/// A positive DoubleFast or Greedy target does not disable Huffman or literal RLE.
/// Corresponds to `ZSTD_literalsCompressionIsDisabled` for automatic literal
/// mode in pinned zstd_compress_internal.h:685-698. Explicit caller overrides
/// from the upstream context API are outside this bounded reference API.
pub(crate) const fn raw_literals(parameters: ResolvedCompressionParameters) -> bool {
    matches!(parameters.strategy(), ReferenceStrategy::Fast) && parameters.target_length() > 0
}

/// Strict payload gain shared by Fast, DoubleFast and Greedy execution.
/// Excludes the relevant section or outer block header, according to the caller.
/// Corresponds to the strategy-below-BtUltra branch of `ZSTD_minGain` in pinned
/// zstd_compress_internal.h:673-683. This is not a policy for future BtUltra paths.
/// Shifting by six leaves room for adding two for every usize value.
pub(crate) const fn minimum_gain(source_size: usize) -> usize {
    (source_size >> 6) + 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RawCompressionParameters;

    #[test]
    fn non_fast_positive_targets_keep_huffman_and_literal_rle_candidates() {
        use crate::literal_encode::LiteralEncoder;

        let mut patterned = [0; 4096];
        for (index, byte) in patterned.iter_mut().enumerate() {
            *byte = (index % 8) as u8;
        }
        let uniform = [b'x'; 4096];
        for (input, normal_kind) in [(&patterned[..], 2), (&uniform[..], 1)] {
            let mut normal = LiteralEncoder::new();
            normal.select_reference(input, None, false).unwrap();
            assert_eq!(normal.as_bytes()[0] & 3, normal_kind);
            for (strategy, target_length, expected_kind) in [
                (1, 0, normal_kind),
                (1, 1, 0),
                (1, 131_072, 0),
                (2, 0, normal_kind),
                (2, 4, normal_kind),
                (2, 131_072, normal_kind),
                (3, 0, normal_kind),
                (3, 4, normal_kind),
                (3, 131_072, normal_kind),
            ] {
                let parameters = RawCompressionParameters {
                    window_log: 21,
                    chain_log: 16,
                    hash_log: 14,
                    search_log: 1,
                    min_match: 4,
                    target_length,
                    strategy,
                }
                .validate()
                .unwrap();
                let mut encoder = LiteralEncoder::new();
                encoder
                    .select_reference(input, None, raw_literals(parameters))
                    .unwrap();
                assert_eq!(encoder.as_bytes()[0] & 3, expected_kind);
                if expected_kind == normal_kind {
                    assert_eq!(encoder.as_bytes(), normal.as_bytes());
                }
            }
        }
    }
}
