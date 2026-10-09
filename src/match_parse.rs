//! Bounded greedy and one-byte lazy selection from indexed match candidates.

use alloc::vec::Vec;

use crate::frame::MAX_BLOCK_SIZE;
use crate::matchchain::MatchChain;
use crate::sequence_context::LogicalSequence;
use crate::{Error, Result};

const MIN_MATCH: usize = 4;
const MAX_VISITS: usize = 16;

/// Plain search policy values; storage access remains in MatchChain.
struct ParsePolicy {
    visits: usize,
    look_ahead: bool,
}

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
    resolve: impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
    literals: &mut Vec<u8>,
    sequences: &mut Vec<LogicalSequence>,
) -> Result<()> {
    parse(
        input,
        table,
        virtual_base,
        ParsePolicy {
            visits: budget,
            look_ahead: false,
        },
        resolve,
        literals,
        sequences,
    )
}

/// Parse with at most sixteen visits at each of two adjacent source positions.
///
/// Corresponds to the one-byte lookahead role of `ZSTD_compressBlock_lazy_generic`,
/// using an independent local rule: delay only if the next match's length exceeds
/// the current length plus one. Score ties keep the current match. This accounts
/// for the additional literal but does not promise globally optimal coded size.
///
/// Current insertion precedes lookahead, permitting distance-one matches. The
/// next position is not inserted until a selection is made; both paths insert
/// each consumed start once. Full-tail matches skip lookahead. Resolver calls use
/// each probe's actual position and never change entropy or repeat state here.
/// Source verification, preflight validation and partial-error behavior follow
/// `parse_chain`. The caller remains responsible for actual window eligibility.
pub(crate) fn parse_lazy(
    input: &[u8],
    table: &mut MatchChain,
    virtual_base: u64,
    resolve: impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
    literals: &mut Vec<u8>,
    sequences: &mut Vec<LogicalSequence>,
) -> Result<()> {
    parse(
        input,
        table,
        virtual_base,
        ParsePolicy {
            visits: MAX_VISITS,
            look_ahead: true,
        },
        resolve,
        literals,
        sequences,
    )
}

fn parse(
    input: &[u8],
    table: &mut MatchChain,
    virtual_base: u64,
    policy: ParsePolicy,
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
    if policy.visits > MAX_VISITS {
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
        let mut best = find_best(
            input,
            table,
            virtual_base,
            position,
            policy.visits,
            &mut resolve,
        )?;
        // Lookup finishes before current insertion: neither the current source
        // coordinate nor a newly inserted node can consume this search budget.
        table.insert(word, current)?;
        let mut match_start = position;
        if policy.look_ahead
            && let Some((_, length)) = best
            && (length as usize) < input.len() - position
            && input.len() - position > MIN_MATCH
        {
            let next = find_best(
                input,
                table,
                virtual_base,
                position + 1,
                policy.visits,
                &mut resolve,
            )?;
            // Both lengths are bounded by the 128KiB block, so +1 fits u32.
            if let Some((_, next_length)) = next
                && next_length > length + 1
            {
                best = next;
                match_start += 1;
            }
        }
        if let Some((distance, length)) = best {
            literals.extend_from_slice(&input[literal_start..match_start]);
            sequences.push(LogicalSequence {
                literal_length: (match_start - literal_start) as u32,
                match_length: length,
                distance,
            });
            let match_end = match_start + length as usize;
            // Current was inserted before probing. Start at current+1 whether
            // it becomes the delayed match start or part of the current match.
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

fn find_best(
    input: &[u8],
    table: &MatchChain,
    virtual_base: u64,
    position: usize,
    budget: usize,
    resolve: &mut impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
) -> Result<Option<(u32, u32)>> {
    let current = virtual_base + position as u64;
    let mut best: Option<(u32, u32)> = None;
    for previous in table.candidates(word_at(input, position), current, budget)? {
        if previous >= current {
            return Err(Error::CorruptData);
        }
        let Some((distance, length)) = resolve(previous, position)? else {
            continue;
        };
        if distance == 0 || length < MIN_MATCH as u32 || length as usize > input.len() - position {
            return Err(Error::CorruptData);
        }
        if best.is_none_or(|(old_distance, old_length)| {
            length > old_length || (length == old_length && distance < old_distance)
        }) {
            best = Some((distance, length));
        }
    }
    Ok(best)
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

    fn lazy_bytes(prefix: &[u8], input: &[u8], nodes: usize) -> (Vec<u8>, Vec<LogicalSequence>) {
        let mut table = MatchChain::new(nodes).unwrap();
        seed_prefix(&mut table, prefix, input);
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        parse_lazy(
            input,
            &mut table,
            prefix.len() as u64,
            |previous, position| resolve_bytes(prefix, input, previous, position),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(reconstruct(prefix, &literals, &sequences), input);
        (literals, sequences)
    }

    #[test]
    fn lazy_one_byte_literal_replaces_two_greedy_sequences_with_one_long_match() {
        let prefix = b"Xabc!abcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let input = b"XabcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut table = MatchChain::new(128).unwrap();
        seed_prefix(&mut table, prefix, input);
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        parse_chain(
            input,
            &mut table,
            prefix.len() as u64,
            16,
            |previous, position| resolve_bytes(prefix, input, previous, position),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(sequences.len(), 2);
        assert_eq!(sequence_values(&sequences)[0], (0, 4, prefix.len() as u32));
        assert_eq!(reconstruct(prefix, &literals, &sequences), input);
        let (literals, sequences) = lazy_bytes(prefix, input, 128);
        assert_eq!(literals, b"X");
        assert_eq!(sequence_values(&sequences), [(1, 36, 37)]);
    }

    #[test]
    fn lazy_accounts_for_the_extra_literal_and_keeps_score_ties_current() {
        for length in [5, 6] {
            let pattern = &b"abcdEF"[..length];
            let mut prefix = Vec::from(&b"Xabc!"[..]);
            prefix.extend_from_slice(pattern);
            prefix.push(0);
            let mut input = vec![b'X'];
            input.extend_from_slice(pattern);
            let (literals, sequences) = lazy_bytes(&prefix, &input, 32);
            assert_eq!(sequences.len(), 1);
            if length == 5 {
                assert_eq!(sequence_values(&sequences), [(0, 4, prefix.len() as u32)]);
                assert_eq!(literals, &input[4..]);
            } else {
                assert_eq!(sequence_values(&sequences), [(1, 6, 8)]);
                assert_eq!(literals, b"X");
            }
        }
    }

    #[test]
    fn lazy_bounded_dictionary_witness_reduces_complete_serialized_payload_cost() {
        let mut prefix = Vec::new();
        let mut input = Vec::new();
        let mut state = 0x7ac1_53bd_u32;
        for _ in 0..128 {
            let pattern: Vec<_> = (0..64)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    (state >> 24) as u8
                })
                .collect();
            prefix.push(b'X');
            prefix.extend_from_slice(&pattern[..3]);
            // Neither separator may accidentally establish X + the long pattern.
            prefix.push(u8::from(pattern[3] == 0));
            prefix.extend_from_slice(&pattern);
            prefix.push(0);
            input.push(b'X');
            input.extend_from_slice(&pattern);
        }
        let mut costs = Vec::new();
        for lazy in [false, true] {
            let mut table = MatchChain::new(32_768).unwrap();
            seed_prefix(&mut table, &prefix, &input);
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            let resolver = |previous, position| resolve_bytes(&prefix, &input, previous, position);
            if lazy {
                parse_lazy(
                    &input,
                    &mut table,
                    prefix.len() as u64,
                    resolver,
                    &mut literals,
                    &mut sequences,
                )
                .unwrap();
            } else {
                parse_chain(
                    &input,
                    &mut table,
                    prefix.len() as u64,
                    16,
                    resolver,
                    &mut literals,
                    &mut sequences,
                )
                .unwrap();
            }
            assert_eq!(reconstruct(&prefix, &literals, &sequences), input);
            assert_eq!(sequences.len(), if lazy { 128 } else { 256 });
            if lazy {
                assert_eq!(literals, vec![b'X'; 128]);
            }
            let mut literal_encoder = crate::literal_encode::LiteralEncoder::new();
            literal_encoder.select(&literals, None).unwrap();
            let mut sequence_encoder = crate::sequence_context::SequenceEncoder::new();
            sequence_encoder.start_frame(None).unwrap();
            sequence_encoder.prepare(&sequences).unwrap();
            costs.push(literal_encoder.as_bytes().len() + sequence_encoder.as_bytes().len());
        }
        assert!(
            costs[1] * 4 < costs[0] * 3,
            "greedy/lazy payload sizes: {costs:?}"
        );
    }

    #[test]
    fn lazy_checks_dictionary_expiration_at_the_actual_lookahead_position() {
        let prefix = b"Xabc!abcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let input = b"XabcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        for frame_position in [1023_u64, 1024] {
            let mut table = MatchChain::new(128).unwrap();
            seed_prefix(&mut table, prefix, input);
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            let mut saw_next_dictionary = false;
            parse_lazy(
                input,
                &mut table,
                prefix.len() as u64,
                |previous, position| {
                    if previous < prefix.len() as u64 {
                        saw_next_dictionary |= position == 1;
                        if frame_position + position as u64 > 1024 {
                            return Ok(None);
                        }
                    }
                    resolve_bytes(prefix, input, previous, position)
                },
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            assert!(saw_next_dictionary);
            assert_eq!(reconstruct(prefix, &literals, &sequences), input);
            assert_eq!(
                sequences[0].literal_length,
                u32::from(frame_position == 1023)
            );
            assert_eq!(
                sequences[0].match_length,
                if frame_position == 1023 { 36 } else { 4 }
            );
        }
    }

    #[test]
    fn lazy_respects_output_window_rejection_for_an_older_longer_next_match() {
        let prefix = b"abcdEFGHIJKLMNOPQRSTUVWXYZ0123456789|Xabc!";
        let input = b"XabcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        for window in [5_u32, 64] {
            let mut table = MatchChain::new(128).unwrap();
            seed_prefix(&mut table, prefix, input);
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            parse_lazy(
                input,
                &mut table,
                prefix.len() as u64,
                |previous, position| {
                    Ok(resolve_bytes(prefix, input, previous, position)?
                        .filter(|&(distance, _)| distance <= window))
                },
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            assert_eq!(reconstruct(prefix, &literals, &sequences), input);
            assert!(sequences.iter().all(|sequence| sequence.distance <= window));
            assert_eq!(sequences[0].literal_length, u32::from(window == 64));
        }
    }

    #[test]
    fn lazy_one_slot_overwrite_keeps_the_current_choice_or_enables_distance_one() {
        let prefix = b"aaaaX";
        let input = b"aaaaaaaaaa";
        let mut table = MatchChain::new(1).unwrap();
        table.insert(*b"aaaa", 0).unwrap();
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        parse_lazy(
            input,
            &mut table,
            5,
            |previous, position| resolve_bytes(prefix, input, previous, position),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(literals, b"a");
        assert_eq!(sequence_values(&sequences), [(1, 9, 1)]);
        assert_eq!(reconstruct(prefix, &literals, &sequences), input);
        assert_eq!(
            table
                .candidates(*b"aaaa", 15, 16)
                .unwrap()
                .collect::<Vec<_>>(),
            [11]
        );
        table.reset();
        table.insert(*b"abcd", 0).unwrap();
        parse_lazy(
            b"abcdXYZ123",
            &mut table,
            5,
            |previous, position| resolve_bytes(b"abcd!", b"abcdXYZ123", previous, position),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(sequence_values(&sequences), [(0, 4, 5)]);
        assert_eq!(literals, b"XYZ123");
        assert_eq!(reconstruct(b"abcd!", &literals, &sequences), b"abcdXYZ123");
    }

    #[test]
    fn lazy_stale_next_probe_cannot_invalidate_an_already_resolved_current_match() {
        let prefix = b"Xabc!abcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let input = b"XabcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        for nodes in [2, 3] {
            let mut table = MatchChain::new(nodes).unwrap();
            table.insert(*b"abcd", 5).unwrap();
            table.insert(*b"Xabc", 0).unwrap();
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            parse_lazy(
                input,
                &mut table,
                prefix.len() as u64,
                |previous, position| resolve_bytes(prefix, input, previous, position),
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            assert_eq!(reconstruct(prefix, &literals, &sequences), input);
            assert_eq!(sequences[0].match_length, if nodes == 2 { 4 } else { 36 });
            assert_eq!(sequences[0].literal_length, u32::from(nodes == 3));
        }
    }

    #[test]
    fn lazy_delayed_overlap_inserts_every_start_once_at_full_width_coordinates() {
        let prefix = b"aaaaX";
        let input = b"aaaaaaaaaa";
        let origin = u64::MAX - (prefix.len() + input.len()) as u64;
        let base = origin + prefix.len() as u64;
        let mut table = MatchChain::new(32).unwrap();
        table.insert(*b"aaaa", origin).unwrap();
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        parse_lazy(
            input,
            &mut table,
            base,
            |previous, position| resolve_bytes(prefix, input, previous - origin, position),
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(sequence_values(&sequences), [(1, 9, 1)]);
        assert_eq!(reconstruct(prefix, &literals, &sequences), input);
        let mut expected: Vec<_> = (0..=6).rev().map(|position| base + position).collect();
        expected.push(origin);
        assert_eq!(
            table
                .candidates(*b"aaaa", u64::MAX, 16)
                .unwrap()
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn lazy_probes_sixteen_candidates_per_position_and_skips_full_tail_lookahead() {
        let mut table = MatchChain::new(64).unwrap();
        for previous in 0..16 {
            table.insert(*b"ABCD", previous).unwrap();
        }
        for previous in 16..32 {
            table.insert(*b"BCDE", previous).unwrap();
        }
        let (mut literals, mut sequences) = (Vec::new(), Vec::new());
        let mut calls = [0; 2];
        parse_lazy(
            b"ABCDEFGHIJKL",
            &mut table,
            100,
            |previous, position| {
                if position < 2 {
                    calls[position] += 1;
                }
                Ok(if position == 0 {
                    Some(((100 - previous) as u32, 4))
                } else {
                    None
                })
            },
            &mut literals,
            &mut sequences,
        )
        .unwrap();
        assert_eq!(calls, [16, 16]);
        for length in [0, 1, 3, 4, 5, 7, 16] {
            table.reset();
            table.insert(*b"aaaa", 0).unwrap();
            parse_lazy(
                &vec![b'a'; length],
                &mut table,
                1,
                |previous, position| {
                    assert_eq!((previous, position), (0, 0));
                    Ok(Some((1, length as u32)))
                },
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            assert_eq!(reconstruct(b"a", &literals, &sequences), vec![b'a'; length]);
        }
    }

    #[test]
    fn lazy_rejects_malformed_lookahead_results_and_propagates_resolver_errors() {
        let prefix = b"Xabc!abcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let input = b"XabcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        for answer in [
            Ok(Some((0, 6))),
            Ok(Some((1, input.len() as u32))),
            Err(Error::InvalidDictionary),
        ] {
            let mut table = MatchChain::new(128).unwrap();
            seed_prefix(&mut table, prefix, input);
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            assert_eq!(
                parse_lazy(
                    input,
                    &mut table,
                    prefix.len() as u64,
                    |previous, position| {
                        if position == 1 {
                            answer
                        } else {
                            resolve_bytes(prefix, input, previous, position)
                        }
                    },
                    &mut literals,
                    &mut sequences
                ),
                Err(answer.err().unwrap_or(Error::CorruptData))
            );
            assert!(sequences.is_empty());
            assert_eq!(
                table
                    .candidates(*b"Xabc", u64::MAX, 1)
                    .unwrap()
                    .collect::<Vec<_>>(),
                [prefix.len() as u64]
            );
        }
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
