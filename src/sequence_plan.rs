//! Forward sequence planning without mutating emitted frame state.

use crate::constants::{LL_BASE, LL_BITS, ML_BASE, ML_BITS};
use crate::frame::MAX_BLOCK_SIZE;
use crate::{Error, Result};

/// A requested wire offset value, not an index into the repeat array.
/// Zero literal length changes the meaning of each repeat value. These plain
/// initialized values have the same representation in every safety backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OffsetIntent {
    Explicit,
    Repeat1,
    Repeat2,
    Repeat3,
}

/// The complete wire transition, without publishing emitted frame state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedOffset {
    pub(crate) offset_value: u32,
    pub(crate) next_repeat: [u32; 3],
}

impl OffsetIntent {
    /// Checks the requested wire value against the byte-verified distance.
    /// Corresponds to the format's repeat-offset rules and `ZSTD_updateRep`.
    /// This checks no source range or block length: only whether literal length
    /// is zero affects the offset transition. The caller retains that context.
    pub(crate) fn resolve(
        self,
        literal_length: u32,
        distance: u32,
        repeat: [u32; 3],
    ) -> Result<ResolvedOffset> {
        if distance == 0 || repeat.contains(&0) {
            return Err(Error::CorruptData);
        }
        let offset_value = match self {
            Self::Explicit => {
                return Ok(ResolvedOffset {
                    offset_value: distance.checked_add(3).ok_or(Error::CorruptData)?,
                    next_repeat: [distance, repeat[0], repeat[1]],
                });
            }
            Self::Repeat1 => 1,
            Self::Repeat2 => 2,
            Self::Repeat3 => 3,
        };
        let (decoded_distance, next_repeat) =
            repeated_offset(repeat, offset_value, literal_length == 0).ok_or(Error::CorruptData)?;
        if decoded_distance != distance {
            return Err(Error::CorruptData);
        }
        Ok(ResolvedOffset {
            offset_value,
            next_repeat,
        })
    }
}

/// An initialized value record; no separate unsafe representation is useful.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlannedSequence {
    pub(crate) literal_length: u32,
    pub(crate) match_length: u32,
    pub(crate) distance: u32,
    pub(crate) literal_code: u8,
    pub(crate) match_code: u8,
    pub(crate) offset_code: u8,
    pub(crate) offset_value: u32,
    pub(crate) literal_extra: u32,
    pub(crate) match_extra: u32,
    pub(crate) offset_extra: u32,
    pub(crate) next_repeat: [u32; 3],
}

impl PlannedSequence {
    /// Selects the smallest valid wire offset value, then derives its exact
    /// repeat triple. Call in source order using the preceding candidate's
    /// `next_repeat`; only the accepted block may publish the final triple.
    /// Corresponds to offset-code planning in `ZSTD_storeSeq` and the format's
    /// repeat-offset update rules, without truncating long literal lengths.
    pub(crate) fn new(
        literal_length: u32,
        match_length: u32,
        distance: u32,
        repeat: [u32; 3],
    ) -> Result<Self> {
        let mut intent = OffsetIntent::Explicit;
        for (value, candidate) in [
            (1, OffsetIntent::Repeat1),
            (2, OffsetIntent::Repeat2),
            (3, OffsetIntent::Repeat3),
        ] {
            if let Some((offset, _)) = repeated_offset(repeat, value, literal_length == 0)
                && offset == distance
            {
                intent = candidate;
                break;
            }
        }
        Self::with_offset(literal_length, match_length, distance, intent, repeat)
    }

    /// Preserves the requested wire offset even when a smaller repeat value
    /// could encode the same distance. The complete repeat transition comes
    /// from that wire choice, not from canonicalizing the distance.
    pub(crate) fn with_offset(
        literal_length: u32,
        match_length: u32,
        distance: u32,
        intent: OffsetIntent,
        repeat: [u32; 3],
    ) -> Result<Self> {
        if match_length < 3
            || literal_length
                .checked_add(match_length)
                .is_none_or(|length| length > MAX_BLOCK_SIZE as u32)
        {
            return Err(Error::CorruptData);
        }
        let ResolvedOffset {
            offset_value,
            next_repeat,
        } = intent.resolve(literal_length, distance, repeat)?;
        let literal_code = LL_BASE.partition_point(|&base| base <= literal_length) - 1;
        let match_code = ML_BASE.partition_point(|&base| base <= match_length) - 1;
        let offset_code = (31 - offset_value.leading_zeros()) as u8;
        let literal_extra = literal_length - LL_BASE[literal_code];
        let match_extra = match_length - ML_BASE[match_code];
        // Explicit checks keep this value record safe to consume even if the
        // block-size policy or the wire tables change independently later.
        if u64::from(literal_extra) >= 1_u64 << LL_BITS[literal_code]
            || u64::from(match_extra) >= 1_u64 << ML_BITS[match_code]
        {
            return Err(Error::CorruptData);
        }
        Ok(Self {
            literal_length,
            match_length,
            distance,
            literal_code: literal_code as u8,
            match_code: match_code as u8,
            offset_code,
            offset_value,
            literal_extra,
            match_extra,
            offset_extra: offset_value - (1_u32 << offset_code),
            next_repeat,
        })
    }
}

/// Evaluates one repeat value without modifying the caller's state.
fn repeated_offset(repeat: [u32; 3], value: u32, no_literals: bool) -> Option<(u32, [u32; 3])> {
    match (no_literals, value) {
        (false, 1) => Some((repeat[0], repeat)),
        (false, 2) | (true, 1) => Some((repeat[1], [repeat[1], repeat[0], repeat[2]])),
        (false, 3) | (true, 2) => Some((repeat[2], [repeat[2], repeat[0], repeat[1]])),
        (true, 3) => {
            let distance = repeat[0].checked_sub(1).filter(|&distance| distance != 0)?;
            Some((distance, [distance, repeat[0], repeat[1]]))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_intent_checks_every_zero_and_nonzero_literal_repeat_branch() {
        let repeat = [10, 20, 30];
        for (intent, literals, distance, value, next) in [
            (OffsetIntent::Repeat1, 1, 10, 1, [10, 20, 30]),
            (OffsetIntent::Repeat2, 1, 20, 2, [20, 10, 30]),
            (OffsetIntent::Repeat3, 1, 30, 3, [30, 10, 20]),
            (OffsetIntent::Repeat1, 0, 20, 1, [20, 10, 30]),
            (OffsetIntent::Repeat2, 0, 30, 2, [30, 10, 20]),
            (OffsetIntent::Repeat3, 0, 9, 3, [9, 10, 20]),
        ] {
            let expected = ResolvedOffset {
                offset_value: value,
                next_repeat: next,
            };
            assert_eq!(intent.resolve(literals, distance, repeat), Ok(expected));
            let plan = PlannedSequence::with_offset(literals, 4, distance, intent, repeat).unwrap();
            assert_eq!((plan.offset_value, plan.next_repeat), (value, next));
            assert_eq!((1 << plan.offset_code) + plan.offset_extra, value);
            // A 16-bit storage truncation must not select the zero-LL rule.
            if literals != 0 {
                for long_literals in [65_536, u32::MAX] {
                    assert_eq!(
                        intent.resolve(long_literals, distance, repeat),
                        Ok(expected)
                    );
                }
                let long =
                    PlannedSequence::with_offset(65_536, 4, distance, intent, repeat).unwrap();
                assert_eq!(long.next_repeat, next);
                assert_eq!(long.literal_code, 35);
            }
        }
    }

    #[test]
    fn explicit_repeat_distance_preserves_intent_and_changes_the_following_transition() {
        let repeat = [10, 20, 30];
        for literals in [0, 1, 65_536] {
            for distance in repeat {
                let plan = PlannedSequence::with_offset(
                    literals,
                    4,
                    distance,
                    OffsetIntent::Explicit,
                    repeat,
                )
                .unwrap();
                assert_eq!(plan.offset_value, distance + 3);
                assert_eq!(plan.next_repeat, [distance, 10, 20]);
            }
        }
        let canonical = PlannedSequence::new(1, 4, 10, repeat).unwrap();
        let explicit =
            PlannedSequence::with_offset(1, 4, 10, OffsetIntent::Explicit, repeat).unwrap();
        assert_eq!((canonical.offset_value, canonical.next_repeat), (1, repeat));
        assert_eq!(
            (explicit.offset_value, explicit.next_repeat),
            (13, [10, 10, 20])
        );
        assert_eq!(
            OffsetIntent::Repeat1.resolve(0, 10, explicit.next_repeat),
            Ok(ResolvedOffset {
                offset_value: 1,
                next_repeat: [10, 10, 20],
            })
        );
        assert_eq!(
            OffsetIntent::Repeat1.resolve(0, 10, canonical.next_repeat),
            Err(Error::CorruptData)
        );
        assert_eq!(
            OffsetIntent::Repeat1.resolve(0, 20, canonical.next_repeat),
            Ok(ResolvedOffset {
                offset_value: 1,
                next_repeat: [20, 10, 30],
            })
        );
    }

    #[test]
    fn duplicate_offsets_retain_distinct_valid_wire_choices() {
        for (literals, repeat, distance, first_next, third_next) in [
            (1, [10, 20, 10], 10, [10, 20, 10], [10, 10, 20]),
            (0, [10, 9, 30], 9, [9, 10, 30], [9, 10, 9]),
        ] {
            let first = OffsetIntent::Repeat1
                .resolve(literals, distance, repeat)
                .unwrap();
            let third = OffsetIntent::Repeat3
                .resolve(literals, distance, repeat)
                .unwrap();
            assert_eq!((first.offset_value, first.next_repeat), (1, first_next));
            assert_eq!((third.offset_value, third.next_repeat), (3, third_next));
            let canonical = PlannedSequence::new(literals, 4, distance, repeat).unwrap();
            assert_eq!(
                (canonical.offset_value, canonical.next_repeat),
                (1, first_next)
            );
            let preserved =
                PlannedSequence::with_offset(literals, 4, distance, OffsetIntent::Repeat3, repeat)
                    .unwrap();
            assert_eq!(
                (preserved.offset_value, preserved.next_repeat),
                (3, third_next)
            );
        }
    }

    #[test]
    fn repeat_intent_rejects_another_valid_repeat_instead_of_reselecting() {
        for (intent, literals, wrong_distance) in [
            (OffsetIntent::Repeat1, 1, 20),
            (OffsetIntent::Repeat2, 1, 10),
            (OffsetIntent::Repeat3, 1, 20),
            (OffsetIntent::Repeat1, 0, 30),
            (OffsetIntent::Repeat2, 0, 9),
            (OffsetIntent::Repeat3, 0, 20),
        ] {
            assert!(PlannedSequence::new(literals, 4, wrong_distance, [10, 20, 30]).is_ok());
            assert_eq!(
                intent.resolve(literals, wrong_distance, [10, 20, 30]),
                Err(Error::CorruptData)
            );
            assert_eq!(
                PlannedSequence::with_offset(literals, 4, wrong_distance, intent, [10, 20, 30]),
                Err(Error::CorruptData)
            );
        }
        assert_eq!(
            OffsetIntent::Repeat3.resolve(0, 1, [1, 4, 8]),
            Err(Error::CorruptData)
        );
    }

    #[test]
    fn offset_intent_rejects_zeros_and_checks_only_explicit_addition() {
        for intent in [
            OffsetIntent::Explicit,
            OffsetIntent::Repeat1,
            OffsetIntent::Repeat2,
            OffsetIntent::Repeat3,
        ] {
            for literals in [0, 1] {
                assert_eq!(
                    intent.resolve(literals, 0, [1, 4, 8]),
                    Err(Error::CorruptData)
                );
                for repeat in [[0, 4, 8], [1, 0, 8], [1, 4, 0]] {
                    assert_eq!(intent.resolve(literals, 1, repeat), Err(Error::CorruptData));
                }
            }
        }
        for distance in u32::MAX - 2..=u32::MAX {
            assert_eq!(
                OffsetIntent::Explicit.resolve(1, distance, [distance, 4, 8]),
                Err(Error::CorruptData)
            );
            let repeated = PlannedSequence::with_offset(
                1,
                3,
                distance,
                OffsetIntent::Repeat1,
                [distance, 4, 8],
            )
            .unwrap();
            assert_eq!((repeated.offset_value, repeated.offset_code), (1, 0));
        }
        let largest =
            PlannedSequence::with_offset(0, 3, u32::MAX - 3, OffsetIntent::Explicit, [1, 4, 8])
                .unwrap();
        assert_eq!(
            (
                largest.offset_value,
                largest.offset_code,
                largest.offset_extra
            ),
            (u32::MAX, 31, (1 << 31) - 1)
        );
        assert_eq!(
            OffsetIntent::Repeat3.resolve(0, u32::MAX - 1, [u32::MAX, 4, 8]),
            Ok(ResolvedOffset {
                offset_value: 3,
                next_repeat: [u32::MAX - 1, u32::MAX, 4],
            })
        );
    }

    #[test]
    fn explicit_plans_enforce_lengths_after_pure_offset_resolution() {
        for (literals, matches) in [
            (0, 0),
            (1, 2),
            (u32::MAX, 4),
            (0, u32::MAX),
            (MAX_BLOCK_SIZE as u32, 3),
            (0, MAX_BLOCK_SIZE as u32 + 1),
        ] {
            for intent in [OffsetIntent::Explicit, OffsetIntent::Repeat1] {
                let distance = if literals == 0 { 4 } else { 1 };
                assert!(intent.resolve(literals, distance, [1, 4, 8]).is_ok());
                assert_eq!(
                    PlannedSequence::with_offset(literals, matches, distance, intent, [1, 4, 8]),
                    Err(Error::CorruptData)
                );
            }
        }
        for (literals, matches) in [(0, 131_072), (65_536, 65_536), (131_069, 3)] {
            let plan = PlannedSequence::with_offset(
                literals,
                matches,
                10,
                OffsetIntent::Explicit,
                [1, 4, 8],
            )
            .unwrap();
            assert_eq!(
                LL_BASE[usize::from(plan.literal_code)] + plan.literal_extra,
                literals
            );
            assert_eq!(
                ML_BASE[usize::from(plan.match_code)] + plan.match_extra,
                matches
            );
            assert_eq!((plan.offset_value, plan.next_repeat), (13, [10, 1, 4]));
        }
    }

    #[test]
    fn every_repeat_branch_preserves_its_exact_wire_update() {
        let repeat = [10, 20, 30];
        for (literals, distance, value, next) in [
            (1, 10, 1, [10, 20, 30]),
            (1, 20, 2, [20, 10, 30]),
            (1, 30, 3, [30, 10, 20]),
            (0, 20, 1, [20, 10, 30]),
            (0, 30, 2, [30, 10, 20]),
            (0, 9, 3, [9, 10, 20]),
            (0, 10, 13, [10, 10, 20]),
            (1, 40, 43, [40, 10, 20]),
        ] {
            let plan = PlannedSequence::new(literals, 4, distance, repeat).unwrap();
            assert_eq!((plan.offset_value, plan.next_repeat), (value, next));
            assert_eq!((1 << plan.offset_code) + plan.offset_extra, value);
        }
        assert_eq!(repeat, [10, 20, 30]);
    }

    #[test]
    fn duplicate_repeats_choose_the_smallest_value_without_deducing_state_from_distance() {
        for literals in [0, 1, 65_536] {
            let plan = PlannedSequence::new(literals, 4, 10, [10; 3]).unwrap();
            assert_eq!(plan.offset_value, 1);
            assert_eq!(plan.next_repeat, [10; 3]);
        }
        let plan = PlannedSequence::new(0, 4, 9, [10, 9, 30]).unwrap();
        assert_eq!(plan.offset_value, 1);
        assert_eq!(plan.next_repeat, [9, 10, 30]);
    }

    #[test]
    fn forward_planning_carries_candidate_repeats_across_every_sequence() {
        let mut pending = [1, 4, 8];
        for (literals, distance, expected_value, expected_repeat) in [
            (10, 7, 10, [7, 1, 4]),
            (0, 6, 3, [6, 7, 1]),
            (0, 7, 1, [7, 6, 1]),
            (1, 1, 3, [1, 7, 6]),
            (0, 6, 2, [6, 1, 7]),
        ] {
            let plan = PlannedSequence::new(literals, 7, distance, pending).unwrap();
            assert_eq!(plan.offset_value, expected_value);
            assert_eq!(plan.next_repeat, expected_repeat);
            pending = plan.next_repeat;
        }
    }

    #[test]
    fn all_block_bounded_lengths_preserve_full_width_codes_and_extras() {
        for literal_length in 0..=MAX_BLOCK_SIZE as u32 - 3 {
            let plan = PlannedSequence::new(literal_length, 3, 1, [1, 4, 8]).unwrap();
            assert_eq!(
                LL_BASE[usize::from(plan.literal_code)] + plan.literal_extra,
                literal_length
            );
            if literal_length >= 65_536 {
                assert_eq!(plan.literal_code, 35);
                assert_eq!(plan.offset_value, 1);
            }
        }
        for match_length in 3..=MAX_BLOCK_SIZE as u32 {
            let plan = PlannedSequence::new(0, match_length, 4, [1, 4, 8]).unwrap();
            assert_eq!(
                ML_BASE[usize::from(plan.match_code)] + plan.match_extra,
                match_length
            );
        }
    }

    #[test]
    fn zero_subtraction_overflow_and_malformed_lengths_are_checked() {
        for (literals, matches, distance, repeat) in [
            (0, 4, 0, [1, 4, 8]),
            (0, 2, 1, [1, 4, 8]),
            (0, 4, 1, [1, 0, 8]),
            (u32::MAX, 4, 1, [1, 4, 8]),
            (MAX_BLOCK_SIZE as u32, 3, 1, [1, 4, 8]),
            (0, 4, u32::MAX - 2, [1, 4, 8]),
        ] {
            assert_eq!(
                PlannedSequence::new(literals, matches, distance, repeat),
                Err(Error::CorruptData)
            );
        }
        assert_eq!(repeated_offset([1, 4, 8], 3, true), None);
        let largest = PlannedSequence::new(0, 3, u32::MAX - 3, [1, 4, 8]).unwrap();
        assert_eq!((largest.offset_value, largest.offset_code), (u32::MAX, 31));
        let repeated = PlannedSequence::new(1, 3, u32::MAX, [u32::MAX, 4, 8]).unwrap();
        assert_eq!(repeated.offset_value, 1);
    }
}
