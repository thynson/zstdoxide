//! Bounded greedy and one-byte lazy selection from indexed match candidates.

use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::frame::MAX_BLOCK_SIZE;
use crate::match_key::MatchKey;
use crate::matchchain::MatchChain;
use crate::sequence_context::LogicalSequence;
use crate::{Error, Result};

const MIN_MATCH: usize = 4;
const MAX_VISITS: usize = 64;
const DEFAULT_LAZY_VISITS: usize = 16;

/// Plain search policy values; storage access remains in MatchChain.
struct ParsePolicy {
    visits: usize,
    look_ahead: bool,
}

/// Parse one block with longest-match selection and deterministic distance ties.
///
/// Fulfills the bounded chain-search role of `ZSTD_HcFindBestMatch`, with this
/// implementation's own policy. The named chain strategy supplies eight visits;
/// this foundation accepts zero through sixty-four. The caller seeds dictionary
/// and retained-output positions before calling; this function never resets the
/// chain or owns any source bytes beyond its literal output.
///
/// `resolve(previous, local_position)` validates bytes, dictionary eligibility,
/// retained history, window bounds and overlaps. It returns `(distance, length)`
/// for a valid match. Virtual coordinates may omit evicted frame regions, so the
/// distance is not inferred by subtracting these coordinates. This parser still
/// requires a strictly earlier candidate, a positive distance, and a match of at
/// least four bytes within the remaining block. Wire representability is checked
/// later by sequence planning. The selected key width only controls probing and
/// insertion: a shorter tail becomes literals, while byte-verified wider-key
/// collisions may still emit a four-byte match.
///
/// Invalid block size, coordinate overflow or visit budget is rejected before
/// changing outputs or table state. Output reservations are fallible and precede
/// clearing logical contents. A resolver or insertion failure may leave partial
/// scratch output and table state; the caller must discard that failed parse.
/// Chain, literal, and sequence storage may use independent allocators. No
/// allocator clone or ownership transfer is needed for these borrowed owners.
pub(crate) fn parse_chain<A: Allocator, L: Allocator, S: Allocator>(
    input: &[u8],
    table: &mut MatchChain<A>,
    virtual_base: u64,
    budget: usize,
    resolve: impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
    literals: &mut Vec<u8, L>,
    sequences: &mut Vec<LogicalSequence, S>,
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
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "default-effort wrapper retained for focused callers"
    )
)]
pub(crate) fn parse_lazy<A: Allocator, L: Allocator, S: Allocator>(
    input: &[u8],
    table: &mut MatchChain<A>,
    virtual_base: u64,
    resolve: impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
    literals: &mut Vec<u8, L>,
    sequences: &mut Vec<LogicalSequence, S>,
) -> Result<()> {
    parse_lazy_with_budget(
        input,
        table,
        virtual_base,
        DEFAULT_LAZY_VISITS,
        resolve,
        literals,
        sequences,
    )
}

/// Parse lazily with an explicit zero-through-sixty-four examined-link limit.
///
/// The same budget applies independently to current and lookahead probes.
/// Zero preserves every byte as a literal while still inserting complete keys.
/// All ordering, source-verification and failure rules follow `parse_lazy`.
pub(crate) fn parse_lazy_with_budget<A: Allocator, L: Allocator, S: Allocator>(
    input: &[u8],
    table: &mut MatchChain<A>,
    virtual_base: u64,
    budget: usize,
    resolve: impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
    literals: &mut Vec<u8, L>,
    sequences: &mut Vec<LogicalSequence, S>,
) -> Result<()> {
    parse(
        input,
        table,
        virtual_base,
        ParsePolicy {
            visits: budget,
            look_ahead: true,
        },
        resolve,
        literals,
        sequences,
    )
}

fn parse<A: Allocator, L: Allocator, S: Allocator>(
    input: &[u8],
    table: &mut MatchChain<A>,
    virtual_base: u64,
    policy: ParsePolicy,
    mut resolve: impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
    literals: &mut Vec<u8, L>,
    sequences: &mut Vec<LogicalSequence, S>,
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

    // The exclusive table borrow prevents outside reconfiguration during this
    // parse; insertion changes links but preserves this checked key layout.
    let layout = table.layout();
    let mut position = 0;
    let mut literal_start = 0;
    while let Some(key) = layout.read_key(&input[position..]) {
        // The complete block-end check above proves every local addition fits.
        let current = virtual_base + position as u64;
        let mut best = find_best(
            input,
            table,
            key,
            virtual_base,
            position,
            policy.visits,
            &mut resolve,
        )?;
        // Lookup finishes before current insertion: neither the current source
        // coordinate nor a newly inserted node can consume this search budget.
        table.insert(key, current)?;
        let mut match_start = position;
        if policy.look_ahead
            && let Some((_, length)) = best
            && (length as usize) < input.len() - position
            && let Some(next_key) = layout.read_key(&input[position + 1..])
        {
            let next = find_best(
                input,
                table,
                next_key,
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
            // Complete input capacity was reserved before clearing either
            // output; initialized literal pushes cannot request another block.
            for &byte in &input[literal_start..match_start] {
                literals.push(byte);
            }
            sequences.push(LogicalSequence {
                literal_length: (match_start - literal_start) as u32,
                match_length: length,
                distance,
            });
            let match_end = match_start + length as usize;
            // Current was inserted before probing. Start at current+1 whether
            // it becomes the delayed match start or part of the current match.
            position += 1;
            while position < match_end
                && let Some(consumed_key) = layout.read_key(&input[position..])
            {
                table.insert(consumed_key, virtual_base + position as u64)?;
                position += 1;
            }
            position = match_end;
            literal_start = match_end;
        } else {
            position += 1;
        }
    }
    for &byte in &input[literal_start..] {
        literals.push(byte);
    }
    Ok(())
}

fn find_best<A: Allocator>(
    input: &[u8],
    table: &MatchChain<A>,
    key: MatchKey,
    virtual_base: u64,
    position: usize,
    budget: usize,
    resolve: &mut impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
) -> Result<Option<(u32, u32)>> {
    let current = virtual_base + position as u64;
    let mut best: Option<(u32, u32)> = None;
    for previous in table.candidates(key, current, budget)? {
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

#[cfg(test)]
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
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 8,
            fail_at: None,
        })
        .unwrap()
    }

    // The bounds intentionally omit Clone for every borrowed owner. Resolver
    // traces and fixture bytes are test infrastructure outside those domains.
    fn parse_traced<A: Allocator, L: Allocator, S: Allocator>(
        prefix: &[u8],
        input: &[u8],
        lazy: bool,
        table: &mut MatchChain<A>,
        outputs: (&mut Vec<u8, L>, &mut Vec<LogicalSequence, S>),
        trace: &mut Vec<(u64, usize)>,
        fail_at: Option<usize>,
    ) -> Result<()> {
        let resolve = |previous, position| {
            trace.push((previous, position));
            if fail_at == Some(position) {
                return Err(Error::InvalidDictionary);
            }
            resolve_bytes(prefix, input, previous, position)
        };
        if lazy {
            parse_lazy(
                input,
                table,
                prefix.len() as u64,
                resolve,
                outputs.0,
                outputs.1,
            )
        } else {
            parse_chain(
                input,
                table,
                prefix.len() as u64,
                8,
                resolve,
                outputs.0,
                outputs.1,
            )
        }
    }

    #[test]
    fn allocator_independent_nonclone_outputs_preserve_candidates_traces_and_bytes() {
        let prefix = b"Xabc!abcdEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let input = b"XabcdEFGHIJKLMNOPQRSTUVWXYZ0123456789!tail";
        for lazy in [false, true] {
            let chain_owner = allocator().into_non_clone();
            let chain_monitor = chain_owner.monitor();
            let mut chain = MatchChain::new_in(128, &chain_owner).unwrap();
            seed_prefix(&mut chain, prefix, input);
            let chain_before = chain_monitor.snapshot();
            let literal_owner = allocator();
            let literal_monitor = literal_owner.monitor();
            let sequence_owner = allocator().into_non_clone();
            let sequence_monitor = sequence_owner.monitor();
            let mut literals = Vec::new_in(literal_owner);
            let mut sequences = Vec::new_in(sequence_owner);
            let mut custom_trace = Vec::new();
            parse_traced(
                prefix,
                input,
                lazy,
                &mut chain,
                (&mut literals, &mut sequences),
                &mut custom_trace,
                None,
            )
            .unwrap();
            assert_eq!(chain_monitor.snapshot(), chain_before);
            assert_eq!(literal_monitor.snapshot().attempts, 1);
            assert_eq!(sequence_monitor.snapshot().attempts, 1);
            assert_eq!(literal_monitor.snapshot().identities_created, 1);
            assert_eq!(sequence_monitor.snapshot().identities_created, 1);
            let mut global_chain = MatchChain::new(128).unwrap();
            seed_prefix(&mut global_chain, prefix, input);
            let (mut expected_literals, mut expected_sequences) = (Vec::new(), Vec::new());
            let mut expected_trace = Vec::new();
            parse_traced(
                prefix,
                input,
                lazy,
                &mut global_chain,
                (&mut expected_literals, &mut expected_sequences),
                &mut expected_trace,
                None,
            )
            .unwrap();
            assert_eq!(literals, expected_literals);
            assert_eq!(sequences, expected_sequences);
            assert_eq!(custom_trace, expected_trace);
            assert_eq!(reconstruct(prefix, &literals, &sequences), input);
            for position in 0..=input.len() - MIN_MATCH {
                let word = word_at(input, position);
                assert_eq!(
                    chain
                        .candidates(word, u64::MAX, 16)
                        .unwrap()
                        .collect::<Vec<_>>(),
                    global_chain
                        .candidates(word, u64::MAX, 16)
                        .unwrap()
                        .collect::<Vec<_>>()
                );
            }
            // Output owners move independently and retain no chain borrow.
            drop(chain);
            chain_monitor.assert_released();
            let moved = (literals, sequences);
            assert_eq!(reconstruct(prefix, &moved.0, &moved.1), input);
            drop(moved);
            literal_monitor.assert_released();
            sequence_monitor.assert_released();
        }
    }

    #[test]
    fn allocator_each_output_reservation_failure_preserves_logical_inputs_and_chain() {
        let prefix = b"abcdefgh";
        let input = b"abcdefgh!abcdefgh?abcdefgh!tail";
        let sentinel = LogicalSequence {
            literal_length: 1,
            match_length: 4,
            distance: 9,
        };
        for lazy in [false, true] {
            for failing_literals in [true, false] {
                let chain_owner = allocator().into_non_clone();
                let chain_monitor = chain_owner.monitor();
                let mut chain = MatchChain::new_in(128, &chain_owner).unwrap();
                seed_prefix(&mut chain, prefix, input);
                let chain_before = chain_monitor.snapshot();
                let candidates = chain
                    .candidates(*b"abcd", u64::MAX, 16)
                    .unwrap()
                    .collect::<Vec<_>>();
                let literal_owner = allocator().into_non_clone();
                let literal_monitor = literal_owner.monitor();
                let sequence_owner = allocator().into_non_clone();
                let sequence_monitor = sequence_owner.monitor();
                let mut literals = Vec::new_in(literal_owner);
                literals.try_reserve_exact(1).unwrap();
                literals.push(77);
                let mut sequences = Vec::new_in(sequence_owner);
                sequences.try_reserve_exact(1).unwrap();
                sequences.push(sentinel);
                let literal_before = literal_monitor.snapshot();
                let sequence_before = sequence_monitor.snapshot();
                let literal_pointer = literals.as_ptr();
                let sequence_pointer = sequences.as_ptr();
                if failing_literals {
                    assert!(literal_monitor.fail_next());
                } else {
                    assert!(sequence_monitor.fail_next());
                }
                let mut trace = Vec::new();
                assert_eq!(
                    parse_traced(
                        prefix,
                        input,
                        lazy,
                        &mut chain,
                        (&mut literals, &mut sequences),
                        &mut trace,
                        None
                    ),
                    Err(Error::AllocationFailed)
                );
                assert!(trace.is_empty());
                assert_eq!(literals, [77]);
                assert_eq!(sequences, [sentinel]);
                assert_eq!(sequence_pointer, sequences.as_ptr());
                assert_eq!(chain_monitor.snapshot(), chain_before);
                assert_eq!(
                    chain
                        .candidates(*b"abcd", u64::MAX, 16)
                        .unwrap()
                        .collect::<Vec<_>>(),
                    candidates
                );
                assert_eq!(
                    literal_monitor.snapshot().attempts,
                    literal_before.attempts + 1
                );
                assert_eq!(
                    sequence_monitor.snapshot().attempts,
                    sequence_before.attempts + u64::from(!failing_literals)
                );
                if failing_literals {
                    assert_eq!(literal_pointer, literals.as_ptr());
                    assert_eq!(
                        literal_monitor.snapshot().live_bytes,
                        literal_before.live_bytes
                    );
                    assert_eq!(sequence_monitor.snapshot(), sequence_before);
                } else {
                    // The first reserve may retain larger capacity even though
                    // the second fails; existing initialized contents survive.
                    assert!(literals.capacity() >= input.len());
                    assert_eq!(
                        sequence_monitor.snapshot().live_bytes,
                        sequence_before.live_bytes
                    );
                }
                literal_monitor.clear_failure();
                sequence_monitor.clear_failure();
                parse_traced(
                    prefix,
                    input,
                    lazy,
                    &mut chain,
                    (&mut literals, &mut sequences),
                    &mut trace,
                    None,
                )
                .unwrap();
                assert_eq!(reconstruct(prefix, &literals, &sequences), input);
                drop(literals);
                drop(sequences);
                drop(chain);
                literal_monitor.assert_released();
                sequence_monitor.assert_released();
                chain_monitor.assert_released();
            }
        }
    }

    #[test]
    fn allocator_shape_validation_precedes_allocations_or_resolver_calls() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let mut chain = MatchChain::new_in(8, &owner).unwrap();
        chain.insert(*b"same", 0).unwrap();
        let mut literals = Vec::new_in(&owner);
        let mut sequences = Vec::new_in(&owner);
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        assert_eq!(
            parse_chain(
                b"same",
                &mut chain,
                1,
                65,
                |_, _| panic!("invalid budget"),
                &mut literals,
                &mut sequences
            ),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            parse_lazy(
                b"same",
                &mut chain,
                u64::MAX,
                |_, _| panic!("overflowing position"),
                &mut literals,
                &mut sequences
            ),
            Err(Error::ResourceLimit)
        );
        assert_eq!(monitor.snapshot(), before);
        assert!(literals.is_empty() && sequences.is_empty());
        assert_eq!(
            chain
                .candidates(*b"same", u64::MAX, 16)
                .unwrap()
                .collect::<Vec<_>>(),
            [0]
        );
        drop(literals);
        drop(sequences);
        drop(chain);
        monitor.assert_released();
    }

    #[test]
    fn allocator_resolver_failure_matches_global_partial_scratch_and_insertions() {
        let prefix = b"abcd!efgh?";
        let input = b"abcdZefgh";
        for lazy in [false, true] {
            let owner = allocator().into_non_clone();
            let monitor = owner.monitor();
            let mut chain = MatchChain::new_in(32, &owner).unwrap();
            seed_prefix(&mut chain, prefix, input);
            let mut literals = Vec::new_in(&owner);
            let mut sequences = Vec::new_in(&owner);
            let mut trace = Vec::new();
            assert_eq!(
                parse_traced(
                    prefix,
                    input,
                    lazy,
                    &mut chain,
                    (&mut literals, &mut sequences),
                    &mut trace,
                    Some(5)
                ),
                Err(Error::InvalidDictionary)
            );
            let mut expected_chain = MatchChain::new(32).unwrap();
            seed_prefix(&mut expected_chain, prefix, input);
            let (mut expected_literals, mut expected_sequences) = (Vec::new(), Vec::new());
            let mut expected_trace = Vec::new();
            assert_eq!(
                parse_traced(
                    prefix,
                    input,
                    lazy,
                    &mut expected_chain,
                    (&mut expected_literals, &mut expected_sequences),
                    &mut expected_trace,
                    Some(5)
                ),
                Err(Error::InvalidDictionary)
            );
            assert_eq!(sequences.len(), 1);
            assert_eq!(sequences[0].match_length, 4);
            assert_eq!(literals, expected_literals);
            assert_eq!(sequences, expected_sequences);
            assert_eq!(trace, expected_trace);
            for position in 0..=input.len() - MIN_MATCH {
                let word = word_at(input, position);
                assert_eq!(
                    chain
                        .candidates(word, u64::MAX, 16)
                        .unwrap()
                        .collect::<Vec<_>>(),
                    expected_chain
                        .candidates(word, u64::MAX, 16)
                        .unwrap()
                        .collect::<Vec<_>>()
                );
            }
            drop(literals);
            drop(sequences);
            drop(chain);
            monitor.assert_released();
        }
    }

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

    fn seed_prefix<A: Allocator>(table: &mut MatchChain<A>, prefix: &[u8], input: &[u8]) {
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
        for budget in [0, 1, 2, 8, 16, 64] {
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
            (&b"same"[..], 1, 65, Error::InvalidParameter),
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
    fn dynamic_chain(width: u8, nodes: usize) -> MatchChain {
        MatchChain::with_layout_in(
            crate::match_search::SearchKeyLayout::new(10, width).unwrap(),
            nodes,
            alloc::alloc::Global,
        )
        .unwrap()
    }

    fn parse_at_budget<A: Allocator, L: Allocator, S: Allocator>(
        lazy: bool,
        input: &[u8],
        table: &mut MatchChain<A>,
        base_and_budget: (u64, usize),
        resolve: impl FnMut(u64, usize) -> Result<Option<(u32, u32)>>,
        literals: &mut Vec<u8, L>,
        sequences: &mut Vec<LogicalSequence, S>,
    ) -> Result<()> {
        let (base, budget) = base_and_budget;
        if lazy {
            parse_lazy_with_budget(input, table, base, budget, resolve, literals, sequences)
        } else {
            parse_chain(input, table, base, budget, resolve, literals, sequences)
        }
    }

    #[test]
    fn dynamic_exact_and_short_tails_skip_probes_and_full_tail_skips_lookahead() {
        for width in [4_u8, 5, 8] {
            for length in [
                0,
                usize::from(width) - 1,
                usize::from(width),
                usize::from(width) + 1,
            ] {
                for lazy in [false, true] {
                    // Exact allocation makes an eight-byte physical load from a
                    // five-byte input observable under Miri, unlike padded data.
                    let input = vec![b'a'; length].into_boxed_slice();
                    let mut table = dynamic_chain(width, 16);
                    let key = table.layout().read_key(&[b'a'; 8]).unwrap();
                    table.insert(key, 0).unwrap();
                    let (mut literals, mut sequences) = (Vec::new(), Vec::new());
                    let mut trace = Vec::new();
                    parse_at_budget(
                        lazy,
                        &input,
                        &mut table,
                        (1, 64),
                        |previous, position| {
                            trace.push((previous, position));
                            resolve_bytes(b"a", &input, previous, position)
                        },
                        &mut literals,
                        &mut sequences,
                    )
                    .unwrap();
                    let complete = length >= usize::from(width);
                    assert_eq!(trace, if complete { vec![(0, 0)] } else { vec![] });
                    assert_eq!(reconstruct(b"a", &literals, &sequences), &*input);
                    if complete {
                        assert!(literals.is_empty());
                        assert_eq!(sequence_values(&sequences), [(0, length as u32, 1)]);
                    } else {
                        assert_eq!(literals, &*input);
                        assert!(sequences.is_empty());
                    }
                    let mut expected = Vec::new();
                    if complete {
                        expected.extend(
                            (0..=length - usize::from(width))
                                .rev()
                                .map(|p| 1 + p as u64),
                        );
                    }
                    expected.push(0);
                    assert_eq!(
                        table
                            .candidates(key, u64::MAX, 64)
                            .unwrap()
                            .collect::<Vec<_>>(),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn dynamic_wider_hash_collision_can_emit_a_verified_four_byte_match() {
        // These distinct eight-byte values share bucket66 at hash log10. Only
        // their first four bytes agree, so key width must not become minMatch.
        let prefix = [b'a', b'b', b'c', b'd', 1, 0, 0, 0];
        let input = [b'a', b'b', b'c', b'd', 0x72, 3, 0, 0];
        for lazy in [false, true] {
            let mut table = dynamic_chain(8, 16);
            let layout = table.layout();
            let original = layout.read_key(&prefix).unwrap();
            let probe = layout.read_key(&input).unwrap();
            assert_ne!(original, probe);
            assert_eq!(layout.bucket(original), Ok(66));
            assert_eq!(layout.bucket(probe), Ok(66));
            table.insert(original, 0).unwrap();
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            let mut trace = Vec::new();
            parse_at_budget(
                lazy,
                &input,
                &mut table,
                (8, 8),
                |previous, position| {
                    trace.push((previous, position));
                    resolve_bytes(&prefix, &input, previous, position)
                },
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            assert_eq!(trace, [(0, 0)]); // Seven-byte next probe is too short.
            assert_eq!(sequence_values(&sequences), [(0, 4, 8)]);
            assert_eq!(literals, input[4..]);
            assert_eq!(reconstruct(&prefix, &literals, &sequences), input);
            assert_eq!(
                table
                    .candidates(probe, u64::MAX, 64)
                    .unwrap()
                    .collect::<Vec<_>>(),
                [8, 0]
            );
        }
    }

    #[test]
    fn dynamic_zero_effort_inserts_each_complete_key_without_resolving() {
        for width in [4_u8, 5, 8] {
            for lazy in [false, true] {
                let input = [b'a'; 12];
                let mut table = dynamic_chain(width, 32);
                let key = table.layout().read_key(&input).unwrap();
                table.insert(key, 0).unwrap();
                let (mut literals, mut sequences) = (Vec::new(), Vec::new());
                parse_at_budget(
                    lazy,
                    &input,
                    &mut table,
                    (1, 0),
                    |_, _| panic!("zero effort must never resolve"),
                    &mut literals,
                    &mut sequences,
                )
                .unwrap();
                assert_eq!(literals, input);
                assert!(sequences.is_empty());
                let expected: Vec<_> = (0..=input.len() - usize::from(width))
                    .rev()
                    .map(|p| 1 + p as u64)
                    .chain([0])
                    .collect();
                assert_eq!(
                    table
                        .candidates(key, u64::MAX, 64)
                        .unwrap()
                        .collect::<Vec<_>>(),
                    expected
                );
            }
        }
    }

    #[test]
    fn dynamic_effort_one_eight_sixteen_sixty_four_changes_verified_selection() {
        let input: Vec<_> = (0..80).collect();
        for width in [4_u8, 5, 8] {
            for budget in [1, 8, 16, 64] {
                for lazy in [false, true] {
                    let mut table = dynamic_chain(width, 256);
                    let layout = table.layout();
                    let key = layout.read_key(&input).unwrap();
                    assert_ne!(
                        layout.bucket(key),
                        layout.bucket(layout.read_key(&input[1..]).unwrap())
                    );
                    let mut prefix = Vec::new();
                    let mut starts = Vec::new();
                    // Oldest is longest. Each additional examined predecessor
                    // exposes one more byte of an independently verified match.
                    for rank in (0..64).rev() {
                        let start = prefix.len() as u64;
                        starts.push(start);
                        prefix.extend_from_slice(&input[..usize::from(width) + rank]);
                        prefix.push(0xff);
                        table.insert(key, start).unwrap();
                    }
                    let (mut literals, mut sequences) = (Vec::new(), Vec::new());
                    let mut first_trace = Vec::new();
                    parse_at_budget(
                        lazy,
                        &input,
                        &mut table,
                        (prefix.len() as u64, budget),
                        |previous, position| {
                            if position == 0 {
                                first_trace.push(previous);
                            }
                            resolve_bytes(&prefix, &input, previous, position)
                        },
                        &mut literals,
                        &mut sequences,
                    )
                    .unwrap();
                    let expected: Vec<_> = starts.iter().rev().copied().take(budget).collect();
                    assert_eq!(first_trace, expected);
                    assert_eq!(sequences[0].literal_length, 0);
                    assert_eq!(
                        sequences[0].match_length as usize,
                        usize::from(width) + budget - 1
                    );
                    assert_eq!(
                        sequences[0].distance as usize,
                        prefix.len() - expected[budget - 1] as usize
                    );
                    assert_eq!(reconstruct(&prefix, &literals, &sequences), input);
                }
            }
        }
    }

    #[test]
    fn dynamic_lazy_budget_reaches_both_current_and_next_verified_probes() {
        let input = b"ABCDEFGHIJKLMNOPQRST";
        for width in [4_u8, 5, 8] {
            for budget in [1, 8, 16, 64] {
                let mut table = dynamic_chain(width, 256);
                let layout = table.layout();
                let current_key = layout.read_key(input).unwrap();
                let next_key = layout.read_key(&input[1..]).unwrap();
                assert_ne!(layout.bucket(current_key), layout.bucket(next_key));
                let mut prefix = Vec::new();
                let mut starts = [Vec::new(), Vec::new()];
                for (position, key) in [current_key, next_key].into_iter().enumerate() {
                    for _ in 0..64 {
                        let start = prefix.len() as u64;
                        starts[position].push(start);
                        let length = usize::from(width) + 2 * position;
                        prefix.extend_from_slice(&input[position..position + length]);
                        prefix.push(0xff);
                        table.insert(key, start).unwrap();
                    }
                }
                let (mut literals, mut sequences) = (Vec::new(), Vec::new());
                let mut trace = [Vec::new(), Vec::new()];
                parse_lazy_with_budget(
                    input,
                    &mut table,
                    prefix.len() as u64,
                    budget,
                    |previous, position| {
                        if position < 2 {
                            trace[position].push(previous);
                        }
                        resolve_bytes(&prefix, input, previous, position)
                    },
                    &mut literals,
                    &mut sequences,
                )
                .unwrap();
                for position in 0..2 {
                    assert_eq!(
                        trace[position],
                        starts[position]
                            .iter()
                            .rev()
                            .copied()
                            .take(budget)
                            .collect::<Vec<_>>()
                    );
                }
                assert_eq!(sequences[0].literal_length, 1);
                assert_eq!(sequences[0].match_length, u32::from(width) + 2);
                assert_eq!(reconstruct(&prefix, &literals, &sequences), input);
            }
        }
    }

    #[test]
    fn dynamic_lazy_distance_one_inserts_consumed_starts_once_at_full_coordinates() {
        for width in [4_u8, 5, 8] {
            let mut prefix = vec![b'a'; usize::from(width)];
            prefix.push(b'X');
            let input = vec![b'a'; usize::from(width) + 4];
            let origin = u64::MAX - (prefix.len() + input.len()) as u64;
            let base = origin + prefix.len() as u64;
            let mut table = dynamic_chain(width, 32);
            let key = table.layout().read_key(&input).unwrap();
            table.insert(key, origin).unwrap();
            let (mut literals, mut sequences) = (Vec::new(), Vec::new());
            let mut trace = Vec::new();
            parse_lazy_with_budget(
                &input,
                &mut table,
                base,
                64,
                |previous, position| {
                    trace.push((previous, position));
                    resolve_bytes(&prefix, &input, previous - origin, position)
                },
                &mut literals,
                &mut sequences,
            )
            .unwrap();
            assert_eq!(trace, [(origin, 0), (base, 1), (origin, 1)]);
            assert_eq!(
                sequence_values(&sequences),
                [(1, input.len() as u32 - 1, 1)]
            );
            assert_eq!(literals, b"a");
            assert_eq!(reconstruct(&prefix, &literals, &sequences), input);
            let expected: Vec<_> = (0..=4)
                .rev()
                .map(|position| base + position)
                .chain([origin])
                .collect();
            assert_eq!(
                table
                    .candidates(key, u64::MAX, 64)
                    .unwrap()
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[test]
    fn dynamic_invalid_effort_rejects_before_reserve_or_table_output_mutation() {
        for width in [4_u8, 5, 8] {
            for budget in [65, usize::MAX] {
                for lazy in [false, true] {
                    let owner = allocator().into_non_clone();
                    let monitor = owner.monitor();
                    let layout = crate::match_search::SearchKeyLayout::new(10, width).unwrap();
                    let mut table = MatchChain::with_layout_in(layout, 8, &owner).unwrap();
                    let input = [b'a'; 12];
                    let key = layout.read_key(&input).unwrap();
                    table.insert(key, 0).unwrap();
                    let mut literals = Vec::new_in(&owner);
                    literals.try_reserve_exact(1).unwrap();
                    literals.push(77);
                    let mut sequences = Vec::new_in(&owner);
                    sequences.try_reserve_exact(1).unwrap();
                    sequences.push(LogicalSequence {
                        literal_length: 1,
                        match_length: 4,
                        distance: 9,
                    });
                    let before = monitor.snapshot();
                    let literal_pointer = literals.as_ptr();
                    let sequence_pointer = sequences.as_ptr();
                    assert!(monitor.fail_next());
                    assert_eq!(
                        parse_at_budget(
                            lazy,
                            &input,
                            &mut table,
                            (1, budget),
                            |_, _| panic!("invalid effort must never resolve"),
                            &mut literals,
                            &mut sequences,
                        ),
                        Err(Error::InvalidParameter)
                    );
                    assert_eq!(monitor.snapshot(), before);
                    assert_eq!(literals, [77]);
                    assert_eq!(sequence_values(&sequences), [(1, 4, 9)]);
                    assert_eq!(literal_pointer, literals.as_ptr());
                    assert_eq!(sequence_pointer, sequences.as_ptr());
                    assert_eq!(
                        table
                            .candidates(key, u64::MAX, 64)
                            .unwrap()
                            .collect::<Vec<_>>(),
                        [0]
                    );
                    drop((table, literals, sequences));
                    monitor.assert_released();
                }
            }
        }
    }
}
