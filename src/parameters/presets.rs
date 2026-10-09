//! Unadjusted, attributed facebook/zstd v1.5.7 compression presets.
//!
//! This module selects scalar reference records only. It does not configure
//! the Rust encoder or apply source-dependent window, chain, or hash adjustment.

use super::{RawCompressionParameters, SourceSize, normalized_level};
use crate::{Error, Result};

// Derived from ZSTD_defaultCParameters in lib/compress/clevels.h:25-130,
// facebook/zstd v1.5.7, commit f8745da6ff1ad1e7bab384bd1f9d742439278e99.
// Copyright (c) Meta Platforms, Inc. and affiliates. All rights reserved.
// Distributed here under the upstream BSD license; see LICENSE.zstd.
// Source SHA-256: d764daa89b7a636d26fe96ff3536c5075e9867fb5dddbf8c9d57023a8ff4a155.
// Fields: window, chain, hash, search, minimum match, target length, strategy.
// Tiers: above 256 KiB, at most 256 KiB, at most 128 KiB, at most 16 KiB.
// Row zero is the negative-level baseline; the default level is row three.
const PRESETS: [[[u32; 7]; 23]; 4] = [
    [
        [19, 12, 13, 1, 6, 1, 1],
        [19, 13, 14, 1, 7, 0, 1],
        [20, 15, 16, 1, 6, 0, 1],
        [21, 16, 17, 1, 5, 0, 2],
        [21, 18, 18, 1, 5, 0, 2],
        [21, 18, 19, 3, 5, 2, 3],
        [21, 18, 19, 3, 5, 4, 4],
        [21, 19, 20, 4, 5, 8, 4],
        [21, 19, 20, 4, 5, 16, 5],
        [22, 20, 21, 4, 5, 16, 5],
        [22, 21, 22, 5, 5, 16, 5],
        [22, 21, 22, 6, 5, 16, 5],
        [22, 22, 23, 6, 5, 32, 5],
        [22, 22, 22, 4, 5, 32, 6],
        [22, 22, 23, 5, 5, 32, 6],
        [22, 23, 23, 6, 5, 32, 6],
        [22, 22, 22, 5, 5, 48, 7],
        [23, 23, 22, 5, 4, 64, 7],
        [23, 23, 22, 6, 3, 64, 8],
        [23, 24, 22, 7, 3, 256, 9],
        [25, 25, 23, 7, 3, 256, 9],
        [26, 26, 24, 7, 3, 512, 9],
        [27, 27, 25, 9, 3, 999, 9],
    ],
    [
        [18, 12, 13, 1, 5, 1, 1],
        [18, 13, 14, 1, 6, 0, 1],
        [18, 14, 14, 1, 5, 0, 2],
        [18, 16, 16, 1, 4, 0, 2],
        [18, 16, 17, 3, 5, 2, 3],
        [18, 17, 18, 5, 5, 2, 3],
        [18, 18, 19, 3, 5, 4, 4],
        [18, 18, 19, 4, 4, 4, 4],
        [18, 18, 19, 4, 4, 8, 5],
        [18, 18, 19, 5, 4, 8, 5],
        [18, 18, 19, 6, 4, 8, 5],
        [18, 18, 19, 5, 4, 12, 6],
        [18, 19, 19, 7, 4, 12, 6],
        [18, 18, 19, 4, 4, 16, 7],
        [18, 18, 19, 4, 3, 32, 7],
        [18, 18, 19, 6, 3, 128, 7],
        [18, 19, 19, 6, 3, 128, 8],
        [18, 19, 19, 8, 3, 256, 8],
        [18, 19, 19, 6, 3, 128, 9],
        [18, 19, 19, 8, 3, 256, 9],
        [18, 19, 19, 10, 3, 512, 9],
        [18, 19, 19, 12, 3, 512, 9],
        [18, 19, 19, 13, 3, 999, 9],
    ],
    [
        [17, 12, 12, 1, 5, 1, 1],
        [17, 12, 13, 1, 6, 0, 1],
        [17, 13, 15, 1, 5, 0, 1],
        [17, 15, 16, 2, 5, 0, 2],
        [17, 17, 17, 2, 4, 0, 2],
        [17, 16, 17, 3, 4, 2, 3],
        [17, 16, 17, 3, 4, 4, 4],
        [17, 16, 17, 3, 4, 8, 5],
        [17, 16, 17, 4, 4, 8, 5],
        [17, 16, 17, 5, 4, 8, 5],
        [17, 16, 17, 6, 4, 8, 5],
        [17, 17, 17, 5, 4, 8, 6],
        [17, 18, 17, 7, 4, 12, 6],
        [17, 18, 17, 3, 4, 12, 7],
        [17, 18, 17, 4, 3, 32, 7],
        [17, 18, 17, 6, 3, 256, 7],
        [17, 18, 17, 6, 3, 128, 8],
        [17, 18, 17, 8, 3, 256, 8],
        [17, 18, 17, 10, 3, 512, 8],
        [17, 18, 17, 5, 3, 256, 9],
        [17, 18, 17, 7, 3, 512, 9],
        [17, 18, 17, 9, 3, 512, 9],
        [17, 18, 17, 11, 3, 999, 9],
    ],
    [
        [14, 12, 13, 1, 5, 1, 1],
        [14, 14, 15, 1, 5, 0, 1],
        [14, 14, 15, 1, 4, 0, 1],
        [14, 14, 15, 2, 4, 0, 2],
        [14, 14, 14, 4, 4, 2, 3],
        [14, 14, 14, 3, 4, 4, 4],
        [14, 14, 14, 4, 4, 8, 5],
        [14, 14, 14, 6, 4, 8, 5],
        [14, 14, 14, 8, 4, 8, 5],
        [14, 15, 14, 5, 4, 8, 6],
        [14, 15, 14, 9, 4, 8, 6],
        [14, 15, 14, 3, 4, 12, 7],
        [14, 15, 14, 4, 3, 24, 7],
        [14, 15, 14, 5, 3, 32, 8],
        [14, 15, 15, 6, 3, 64, 8],
        [14, 15, 15, 7, 3, 256, 8],
        [14, 15, 15, 5, 3, 48, 9],
        [14, 15, 15, 6, 3, 128, 9],
        [14, 15, 15, 7, 3, 256, 9],
        [14, 15, 15, 8, 3, 256, 9],
        [14, 15, 15, 8, 3, 512, 9],
        [14, 15, 15, 9, 3, 512, 9],
        [14, 15, 15, 10, 3, 999, 9],
    ],
];

/// Select an unadjusted preset for the no-attached-dictionary reference mode.
///
/// Corresponds to the table and acceleration steps of
/// `ZSTD_getCParams_internal`; its final adjustment is deliberately separate.
/// Level normalization precedes negation, including for `i32::MIN`.
/// No allocation, encoder state, or target-width address calculation occurs.
pub(super) fn select(
    level: i32,
    source_size: SourceSize,
    dictionary_size: usize,
) -> Result<RawCompressionParameters> {
    let tier = source_tier(source_size, dictionary_size)?;
    let level = normalized_level(level);
    let row = if level < 0 { 0 } else { level as usize };
    let fields = PRESETS
        .get(tier)
        .and_then(|rows| rows.get(row))
        .ok_or(Error::InvalidParameter)?;
    let mut parameters = raw(*fields);
    if level < 0 {
        // Normalization bounds the negative level to -131072..=-1, making
        // negation and the unsigned conversion exact on every target.
        parameters.target_length = (-level) as u32;
    }
    Ok(parameters)
}

const fn raw(fields: [u32; 7]) -> RawCompressionParameters {
    RawCompressionParameters {
        window_log: fields[0],
        chain_log: fields[1],
        hash_log: fields[2],
        search_log: fields[3],
        min_match: fields[4],
        target_length: fields[5],
        strategy: fields[6],
    }
}

/// Compute the reference preset tier with an explicit non-modular size policy.
///
/// Corresponds to `ZSTD_getCParamRowSize` in no-attached-dictionary mode. The
/// reference's unknown sentinel plus a nonempty dictionary plus 500 wraps to
/// dictionary size plus 499. We express that intended value directly and check
/// its addition. Known source plus dictionary is checked independently. These
/// checks belong to preset selection; adjustment has different expressions and
/// must check only the arithmetic it actually performs.
fn source_tier(source_size: SourceSize, dictionary_size: usize) -> Result<usize> {
    let dictionary_size = u64::try_from(dictionary_size).map_err(|_| Error::ResourceLimit)?;
    let size = match source_size {
        SourceSize::Known(u64::MAX) => return Err(Error::InvalidParameter),
        SourceSize::Known(source_size) => source_size
            .checked_add(dictionary_size)
            .ok_or(Error::ResourceLimit)?,
        SourceSize::Unknown if dictionary_size == 0 => return Ok(0),
        SourceSize::Unknown => dictionary_size
            .checked_add(499)
            .ok_or(Error::ResourceLimit)?,
    };
    Ok(match size {
        0..=16_384 => 3,
        16_385..=131_072 => 2,
        131_073..=262_144 => 1,
        _ => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parameters::{CompressionParameterRequest, fixture};

    const FROZEN: &str = include_str!("../../tests/upstream/parameters/presets.tsv");
    const TIER_SOURCES: [SourceSize; 4] = [
        SourceSize::Unknown,
        SourceSize::Known(262_144),
        SourceSize::Known(131_072),
        SourceSize::Known(16_384),
    ];

    #[test]
    fn every_attributed_entry_matches_the_92_independent_c_rows() {
        let rows = fixture::rows(FROZEN);
        assert_eq!(rows.len(), 92);
        let mut seen = [[false; 23]; 4];
        for row in rows {
            let tier = fixture::number::<usize>(row[0]);
            let index = fixture::number::<usize>(row[1]);
            assert!(!seen[tier][index], "duplicate frozen preset");
            seen[tier][index] = true;
            let expected: [u32; 7] = core::array::from_fn(|i| fixture::number(row[i + 2]));
            assert_eq!(PRESETS[tier][index], expected, "tier {tier}, row {index}");
            assert_eq!(raw(expected).validate().unwrap().to_raw(), raw(expected));
        }
        assert!(seen.into_iter().flatten().all(|present| present));
    }

    #[test]
    fn every_frozen_row_is_reachable_without_adjustment() {
        let mut checked = 0;
        for row in fixture::rows(FROZEN) {
            let tier = fixture::number::<usize>(row[0]);
            let index = fixture::number::<i32>(row[1]);
            let level = if index == 0 { -1 } else { index };
            let expected = core::array::from_fn(|i| fixture::number(row[i + 2]));
            assert_eq!(select(level, TIER_SOURCES[tier], 0), Ok(raw(expected)));
            checked += 1;
        }
        assert_eq!(checked, 92);
    }

    #[test]
    fn level_zero_and_extremes_share_request_normalization() {
        for source in TIER_SOURCES {
            for (input, normalized) in [
                (i32::MIN, -131_072),
                (-131_073, -131_072),
                (-131_072, -131_072),
                (-9, -9),
                (-1, -1),
                (0, 3),
                (1, 1),
                (22, 22),
                (23, 22),
                (i32::MAX, 22),
            ] {
                assert_eq!(
                    CompressionParameterRequest::with_level(input).level(),
                    normalized
                );
                assert_eq!(select(input, source, 0), select(normalized, source, 0));
            }
        }
    }

    #[test]
    fn negative_levels_change_only_acceleration_in_every_tier() {
        for (tier, source) in TIER_SOURCES.into_iter().enumerate() {
            for level in [-1, -2, -9, -131_072, i32::MIN] {
                let mut expected = raw(PRESETS[tier][0]);
                expected.target_length = if level == i32::MIN {
                    131_072
                } else {
                    (-level) as u32
                };
                let actual = select(level, source, 0).unwrap();
                assert_eq!(actual, expected);
                assert_eq!(actual.validate().unwrap().to_raw(), expected);
            }
        }
    }

    #[test]
    fn known_source_and_dictionary_sum_cross_every_inclusive_tier_boundary() {
        for (boundary, at_tier, above_tier) in [(16_384, 3, 2), (131_072, 2, 1), (262_144, 1, 0)] {
            for dictionary in [0, 1, 499, 16_000] {
                for (total, expected) in [
                    (boundary - 1, at_tier),
                    (boundary, at_tier),
                    (boundary + 1, above_tier),
                ] {
                    assert_eq!(
                        source_tier(SourceSize::Known(total - dictionary as u64), dictionary),
                        Ok(expected)
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_source_uses_the_nonempty_dictionary_plus_499_tier() {
        assert_eq!(source_tier(SourceSize::Unknown, 0), Ok(0));
        assert_eq!(source_tier(SourceSize::Unknown, 1), Ok(3));
        for (boundary, at_tier, above_tier) in [(16_384, 3, 2), (131_072, 2, 1), (262_144, 1, 0)] {
            for (dictionary, expected) in [
                (boundary - 500, at_tier),
                (boundary - 499, at_tier),
                (boundary - 498, above_tier),
            ] {
                assert_eq!(source_tier(SourceSize::Unknown, dictionary), Ok(expected));
            }
        }
    }

    #[test]
    fn known_empty_is_distinct_and_selection_does_not_perform_adjustment() {
        assert_eq!(
            select(3, SourceSize::Known(0), 0),
            Ok(raw([14, 14, 15, 2, 4, 0, 2]))
        );
        assert_eq!(
            select(3, SourceSize::Unknown, 0),
            Ok(raw([21, 16, 17, 1, 5, 0, 2]))
        );
        // Stage-three adjustment reduces these fields for an empty source.
        // Returning an unadjusted preset here is intentional and observable.
        assert_ne!(
            select(3, SourceSize::Known(0), 0),
            Ok(raw([10, 6, 7, 2, 4, 0, 2]))
        );
        assert_eq!(source_tier(SourceSize::Known(0), 262_145), Ok(0));
    }

    #[test]
    fn reserved_known_size_and_overflow_are_errors_without_modular_tiers() {
        assert_eq!(
            select(3, SourceSize::Known(u64::MAX), 0),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            select(3, SourceSize::Known(u64::MAX), 1),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            source_tier(SourceSize::Known(u64::MAX - 10), 20),
            Err(Error::ResourceLimit)
        );
        // A maximum arithmetic result is still a known sum, not an input
        // sentinel: the original source was valid and no addition overflowed.
        assert_eq!(source_tier(SourceSize::Known(u64::MAX - 1), 1), Ok(0));
        assert_eq!(
            source_tier(SourceSize::Known(u64::MAX - 1), 2),
            Err(Error::ResourceLimit)
        );
    }

    #[test]
    fn pointer_width_only_limits_dictionary_input_not_u64_tier_arithmetic() {
        if usize::BITS == 64 {
            assert_eq!(source_tier(SourceSize::Unknown, usize::MAX - 499), Ok(0));
            assert_eq!(
                source_tier(SourceSize::Unknown, usize::MAX - 498),
                Err(Error::ResourceLimit)
            );
            assert_eq!(
                source_tier(SourceSize::Unknown, usize::MAX),
                Err(Error::ResourceLimit)
            );
            // Known zero does not use the unknown-source +499 expression.
            assert_eq!(source_tier(SourceSize::Known(0), usize::MAX), Ok(0));
        } else {
            assert_eq!(source_tier(SourceSize::Unknown, usize::MAX), Ok(0));
            assert_eq!(source_tier(SourceSize::Known(0), usize::MAX), Ok(0));
        }
    }
}
