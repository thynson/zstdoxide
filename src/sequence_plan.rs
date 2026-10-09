//! Forward sequence planning without mutating emitted frame state.

use crate::constants::{LL_BASE, LL_BITS, ML_BASE, ML_BITS};
use crate::frame::MAX_BLOCK_SIZE;
use crate::{Error, Result};

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
        if distance == 0
            || repeat.contains(&0)
            || match_length < 3
            || literal_length
                .checked_add(match_length)
                .is_none_or(|length| length > MAX_BLOCK_SIZE as u32)
        {
            return Err(Error::CorruptData);
        }
        let mut chosen = None;
        for value in 1..=3 {
            if let Some((offset, next)) = repeated_offset(repeat, value, literal_length == 0)
                && offset == distance
            {
                chosen = Some((value, next));
                break;
            }
        }
        let (offset_value, next_repeat) = match chosen {
            Some(selected) => selected,
            None => (
                distance.checked_add(3).ok_or(Error::CorruptData)?,
                [distance, repeat[0], repeat[1]],
            ),
        };
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
