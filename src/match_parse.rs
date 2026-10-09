//! Bounded greedy selection from indexed match-chain candidates.

use alloc::vec::Vec;

use crate::frame::MAX_BLOCK_SIZE;
use crate::matchchain::MatchChain;
use crate::sequence_context::LogicalSequence;
use crate::{Error, Result};

const MIN_MATCH: usize = 4;
const MAX_VISITS: usize = 16;

/// Parse one block with longest-match selection and deterministic distance ties.
///
/// Fulfills the bounded chain-search role of `ZSTD_HcFindBestMatch`, with this
/// implementation's own policy. The named chain strategy supplies eight visits;
/// this foundation accepts zero through sixteen. The caller seeds dictionary
/// and retained-output positions before calling; this function never resets the
/// chain or owns any source bytes beyond its literal output.
///
/// `resolve(previous, local_position)` validates bytes, dictionary eligibility,
/// retained history, window bounds and overlaps. It returns `(distance, length)`
/// for a valid match. Virtual coordinates may omit evicted frame regions, so the
/// distance is not inferred by subtracting these coordinates. This parser still
/// requires a strictly earlier candidate, a positive distance, and a match of at
/// least four bytes within the remaining block. Wire representability is checked
/// later by sequence planning.
///
/// Invalid block size, coordinate overflow or visit budget is rejected before
/// changing outputs or table state. Output reservations are fallible and precede
/// clearing logical contents. A resolver or insertion failure may leave partial
/// scratch output and table state; the caller must discard that failed parse.
pub(crate) fn parse_chain(
    input: &[u8],
    table: &mut MatchChain,
    virtual_base: u64,
    budget: usize,
    mut resolve: impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
    literals: &mut Vec<u8>,
    sequences: &mut Vec<LogicalSequence>,
) -> Result<()> {
    if input.len() > MAX_BLOCK_SIZE {
        return Err(Error::ResourceLimit);
    }
    virtual_base
        .checked_add(input.len() as u64)
        .ok_or(Error::ResourceLimit)?;
    if budget > MAX_VISITS {
        return Err(Error::InvalidParameter);
    }
    literals
        .try_reserve(input.len().saturating_sub(literals.len()))
        .map_err(|_| Error::AllocationFailed)?;
    sequences
        .try_reserve((input.len() / MIN_MATCH).saturating_sub(sequences.len()))
        .map_err(|_| Error::AllocationFailed)?;
    literals.clear();
    sequences.clear();

    let mut position = 0;
    let mut literal_start = 0;
    while input.len() - position >= MIN_MATCH {
        // The complete block-end check above proves every local addition fits.
        let current = virtual_base + position as u64;
        let word = word_at(input, position);
        let mut best: Option<(u32, u32)> = None;
        for previous in table.candidates(word, current, budget)? {
            if previous >= current {
                return Err(Error::CorruptData);
            }
            let Some((distance, length)) = resolve(previous, position)? else {
                continue;
            };
            if distance == 0
                || length < MIN_MATCH as u32
                || length as usize > input.len() - position
            {
                return Err(Error::CorruptData);
            }
            if best.is_none_or(|(old_distance, old_length)| {
                length > old_length || (length == old_length && distance < old_distance)
            }) {
                best = Some((distance, length));
            }
        }
        // Lookup finishes before current insertion: neither the current source
        // coordinate nor a newly inserted node can consume this search budget.
        table.insert(word, current)?;
        if let Some((distance, length)) = best {
            literals.extend_from_slice(&input[literal_start..position]);
            sequences.push(LogicalSequence {
                literal_length: (position - literal_start) as u32,
                match_length: length,
                distance,
            });
            let match_end = position + length as usize;
            position += 1;
            while position < match_end && input.len() - position >= MIN_MATCH {
                table.insert(word_at(input, position), virtual_base + position as u64)?;
                position += 1;
            }
            position = match_end;
            literal_start = match_end;
        } else {
            position += 1;
        }
    }
    literals.extend_from_slice(&input[literal_start..]);
    Ok(())
}

fn word_at(input: &[u8], position: usize) -> [u8; 4] {
    [
        input[position],
        input[position + 1],
        input[position + 2],
        input[position + 3],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn byte_at(prefix: &[u8], input: &[u8], position: usize) -> u8 {
        if position < prefix.len() {
            prefix[position]
        } else {
            input[position - prefix.len()]
        }
    }

    fn resolve_bytes(
        prefix: &[u8],
        input: &[u8],
        previous: u64,
        position: usize,
    ) -> Result<Option<(u32, u32)>> {
        let previous = usize::try_from(previous).map_err(|_| Error::CorruptData)?;
        let current = prefix.len() + position;
        if previous >= current {
            return Err(Error::CorruptData);
        }
        let mut length = 0;
        while length < input.len() - position
            && byte_at(prefix, input, previous + length) == input[position + length]
        {
            length += 1;
        }
        Ok((length >= MIN_MATCH).then_some(((current - previous) as u32, length as u32)))
    }

    fn seed_prefix(table: &mut MatchChain, prefix: &[u8], input: &[u8]) {
        for position in 0..prefix.len() {
            if prefix.len() + input.len() - position < MIN_MATCH {
                break;
            }
            let word = core::array::from_fn(|offset| byte_at(prefix, input, position + offset));
            table.insert(word, position as u64).unwrap();
        }
    }

    fn reconstruct(prefix: &[u8], literals: &[u8], sequences: &[LogicalSequence]) -> Vec<u8> {
        let mut output = Vec::from(prefix);
        let mut literal_position = 0;
        for sequence in sequences {
            let literal_end = literal_position + sequence.literal_length as usize;
            output.extend_from_slice(&literals[literal_position..literal_end]);
            literal_position = literal_end;
            for _ in 0..sequence.match_length {
                let source = output
                    .len()
                    .checked_sub(sequence.distance as usize)
                    .unwrap();
                output.push(output[source]);
            }
        }
        output.extend_from_slice(&literals[literal_position..]);
        output[prefix.len()..].to_vec()
    }

    fn sequence_values(sequences: &[LogicalSequence]) -> Vec<(u32, u32, u32)> {
        sequences
            .iter()
            .map(|sequence| {
                (
                    sequence.literal_length,
                    sequence.match_length,
                    sequence.distance,
                )
            })
            .collect()
    }

    #[test]
    fn older_longer_match_beats_the_single_newest_candidate() {
        let prefix = b"abcdEFGHIJKLMN|abcdX";
        let input = b"abcdEFGHIJKLMN";
        let mut results = Vec::new();
        for budget in [1, 8] {
            let mut table = MatchChain::new(64).unwrap();
            table.insert(*b"abcd", 0).unwrap();
            table.insert(*b"abcd", 15).unwrap();
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            parse_chain(
                input,
                &mut table,
                prefix.len() as u64,
                budget,
                |previous, position| resolve_bytes(prefix, input, previous, position),
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            assert_eq!(reconstruct(prefix, &literals, &sequences), input);
            results.push(sequence_values(&sequences));
        }
        assert_eq!(results[0][0], (0, 4, 5));
        assert_eq!(results[1], [(0, input.len() as u32, prefix.len() as u32)]);
    }

    #[test]
    fn a_hash_collision_does_not_hide_an_older_byte_verified_match() {
        let mut prefix = vec![0; 4];
        prefix.extend_from_slice(b"ABCDE");
        prefix.extend_from_slice(&0x0e8b_2f51_u32.to_le_bytes());
        let input = &prefix[..9];
        let mut table = MatchChain::new(32).unwrap();
        table.insert([0; 4], 0).unwrap();
        table.insert(0x0e8b_2f51_u32.to_le_bytes(), 9).unwrap();
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        let mut visited = Vec::new();
        parse_chain(
            input,
            &mut table,
            prefix.len() as u64,
            8,
            |previous, position| {
                visited.push(previous);
                resolve_bytes(&prefix, input, previous, position)
            },
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(visited, [9, 0]);
        assert_eq!(sequence_values(&sequences), [(0, 9, 13)]);
        assert_eq!(reconstruct(&prefix, &literals, &sequences), input);
    }

    #[test]
    fn equal_lengths_prefer_nearer_distance_even_when_inserted_earlier() {
        let prefix = b"abcd?abcd!";
        let input = b"abcdZZZ";
        let mut table = MatchChain::new(16).unwrap();
        table.insert(*b"abcd", 5).unwrap();
        table.insert(*b"abcd", 0).unwrap();
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        parse_chain(
            input,
            &mut table,
            prefix.len() as u64,
            8,
            |previous, position| resolve_bytes(prefix, input, previous, position),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(sequence_values(&sequences), [(0, 4, 5)]);
        assert_eq!(literals, b"ZZZ");
        assert_eq!(reconstruct(prefix, &literals, &sequences), input);
    }

    #[test]
    fn empty_short_and_zero_budget_inputs_preserve_every_literal() {
        for input in [&b""[..], b"a", b"ab", b"abc", b"abcdefg"] {
            let mut table = MatchChain::new(16).unwrap();
            let (mut literals, mut sequences) = (vec![99], Vec::new());
            parse_chain(
                input,
                &mut table,
                0,
                0,
                |_, _| panic!("zero-budget resolver"),
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            assert_eq!(literals, input);
            assert!(sequences.is_empty());
        }
    }

    #[test]
    fn visit_budget_includes_future_coordinates_and_never_resolves_the_current_position() {
        for budget in [0, 1, 2, 8, 16] {
            let mut table = MatchChain::new(32).unwrap();
            table.insert(*b"same", 0).unwrap();
            table.insert(*b"same", 100).unwrap();
            table.insert(*b"same", 101).unwrap();
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            let mut calls = Vec::new();
            parse_chain(
                b"same",
                &mut table,
                100,
                budget,
                |previous, position| {
                    assert!(previous < 100 + position as u64);
                    calls.push((previous, position));
                    Ok(Some((100, 4)))
                },
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            if budget <= 2 {
                assert!(calls.is_empty());
                assert_eq!(literals, b"same");
            } else {
                assert_eq!(calls, [(0, 0)]);
                assert_eq!(sequence_values(&sequences), [(0, 4, 100)]);
            }
        }
    }

    #[test]
    fn overlapping_match_inserts_each_consumed_four_byte_start_exactly_once() {
        let prefix = b"a";
        let input = b"aaaaaaa";
        let mut table = MatchChain::new(32).unwrap();
        seed_prefix(&mut table, prefix, input);
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        parse_chain(
            input,
            &mut table,
            1,
            8,
            |previous, position| resolve_bytes(prefix, input, previous, position),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(sequence_values(&sequences), [(0, 7, 1)]);
        assert_eq!(reconstruct(prefix, &literals, &sequences), input);
        assert_eq!(
            table
                .candidates(*b"aaaa", 8, 16)
                .unwrap()
                .collect::<Vec<_>>(),
            [4, 3, 2, 1, 0]
        );
    }

    #[test]
    fn lookup_precedes_insertion_even_when_the_only_node_will_be_overwritten() {
        let prefix = b"abcd";
        let input = b"abcd";
        let mut table = MatchChain::new(1).unwrap();
        table.insert(*b"abcd", 0).unwrap();
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        parse_chain(
            input,
            &mut table,
            4,
            8,
            |previous, position| resolve_bytes(prefix, input, previous, position),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(sequence_values(&sequences), [(0, 4, 4)]);
        assert_eq!(reconstruct(prefix, &literals, &sequences), input);
        assert_eq!(
            table
                .candidates(*b"abcd", 5, 16)
                .unwrap()
                .collect::<Vec<_>>(),
            [4]
        );
    }

    #[test]
    fn literal_and_match_runs_reconstruct_complete_inputs_across_source_boundaries() {
        for (prefix, input) in [
            (
                &b""[..],
                &b"abcabcabc!abcdefghijklmnopabcdefghijklmnop?tail"[..],
            ),
            (&b"history-abc"[..], &b"abcabcabc#history-abc!tail"[..]),
            (&b"ab"[..], &b"abababababxabababababyz"[..]),
        ] {
            let mut table = MatchChain::new(256).unwrap();
            seed_prefix(&mut table, prefix, input);
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            parse_chain(
                input,
                &mut table,
                prefix.len() as u64,
                8,
                |previous, position| resolve_bytes(prefix, input, previous, position),
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            assert!(!sequences.is_empty());
            assert_eq!(reconstruct(prefix, &literals, &sequences), input);
            let consumed = literals.len()
                + sequences
                    .iter()
                    .map(|sequence| sequence.match_length as usize)
                    .sum::<usize>();
            assert_eq!(consumed, input.len());
        }
    }

    #[test]
    fn malformed_resolver_matches_and_errors_are_rejected_without_current_insertion() {
        for answer in [
            Ok(Some((0, 4))),
            Ok(Some((1, 0))),
            Ok(Some((1, 3))),
            Ok(Some((1, 5))),
            Ok(Some((1, u32::MAX))),
            Err(Error::InvalidDictionary),
        ] {
            let mut table = MatchChain::new(8).unwrap();
            table.insert(*b"same", 0).unwrap();
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            let expected = answer.err().unwrap_or(Error::CorruptData);
            assert_eq!(
                parse_chain(
                    b"same",
                    &mut table,
                    1,
                    8,
                    |_, _| answer,
                    &mut literals,
                    &mut sequences
                ),
                Err(expected)
            );
            assert_eq!(
                table
                    .candidates(*b"same", 2, 16)
                    .unwrap()
                    .collect::<Vec<_>>(),
                [0]
            );
        }
        let mut table = MatchChain::new(8).unwrap();
        table.insert(*b"same", 0).unwrap();
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        parse_chain(
            b"same",
            &mut table,
            1,
            8,
            |_, _| Ok(Some((u32::MAX, 4))),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        // Distance wire representability belongs to forward sequence planning.
        assert_eq!(sequences[0].distance, u32::MAX);
    }

    #[test]
    fn invalid_shape_preserves_outputs_and_seeded_table_before_resolving() {
        let oversized = vec![0; MAX_BLOCK_SIZE + 1];
        for (input, base, budget, expected) in [
            (&oversized[..], 0, 8, Error::ResourceLimit),
            (&b"x"[..], u64::MAX, 8, Error::ResourceLimit),
            (&b"same"[..], 1, 17, Error::InvalidParameter),
            (&b"same"[..], 1, usize::MAX, Error::InvalidParameter),
        ] {
            let mut table = MatchChain::new(8).unwrap();
            table.insert(*b"same", 0).unwrap();
            let mut literals = vec![77];
            let mut sequences = vec![LogicalSequence {
                literal_length: 1,
                match_length: 4,
                distance: 9,
            }];
            assert_eq!(
                parse_chain(
                    input,
                    &mut table,
                    base,
                    budget,
                    |_, _| panic!("invalid shape"),
                    &mut literals,
                    &mut sequences
                ),
                Err(expected)
            );
            assert_eq!(literals, [77]);
            assert_eq!(sequence_values(&sequences), [(1, 4, 9)]);
            assert_eq!(
                table
                    .candidates(*b"same", 2, 16)
                    .unwrap()
                    .collect::<Vec<_>>(),
                [0]
            );
        }
    }

    #[test]
    fn maximum_block_overlap_and_full_width_virtual_coordinates_remain_bounded() {
        let input = vec![b'a'; MAX_BLOCK_SIZE];
        let base = u64::MAX - input.len() as u64;
        let mut table = MatchChain::new(64).unwrap();
        table.insert(*b"aaaa", base - 1).unwrap();
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        parse_chain(
            &input,
            &mut table,
            base,
            8,
            |previous, position| {
                assert_eq!((previous, position), (base - 1, 0));
                Ok(Some((1, input.len() as u32)))
            },
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(sequence_values(&sequences), [(0, MAX_BLOCK_SIZE as u32, 1)]);
        assert_eq!(reconstruct(b"a", &literals, &sequences), input);
        let newest: Vec<_> = table.candidates(*b"aaaa", u64::MAX, 16).unwrap().collect();
        let expected: Vec<_> = (0..16)
            .map(|index| base + (MAX_BLOCK_SIZE - 4 - index) as u64)
            .collect();
        assert_eq!(newest, expected);
        let capacities = (
            literals.capacity(),
            sequences.capacity(),
            table.retained_capacity(),
        );
        table.reset();
        parse_chain(
            b"end",
            &mut table,
            u64::MAX - 3,
            8,
            |_, _| panic!("short tail"),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(literals, b"end");
        assert_eq!(
            (
                literals.capacity(),
                sequences.capacity(),
                table.retained_capacity()
            ),
            capacities
        );
    }
}
