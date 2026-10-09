//! Bounded literal-tree construction for independently trained dictionaries.
//!
//! Frequency and parent records are plain values, with all links represented by
//! indices. Dynamic heap storage is reserved fallibly before insertion. This
//! module initially builds metadata; hot-path literal encoding follows later.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;
use core::cmp::Reverse;

#[cfg(test)]
use crate::entropy_encode::encode_weights;
use crate::entropy_encode::encode_weights_in;
use crate::heap::MaxHeap;
use crate::{Error, Result};

const SYMBOLS: usize = 256;
const MAX_DEPTH: u8 = 11;

/// Serialize a complete dictionary literal alphabet learned from sample counts.
///
/// Corresponds to the metadata role of upstream `HUF_writeCTable_wksp`. The
/// full alphabet needs 255 explicit weights, exceeding the direct nibble form's
/// 128-symbol limit, so its description always uses a two-state FSE stream.
/// An oversized description falls back to a deterministic near-flat complete
/// tree; no partial or truncated table is ever published.
pub(crate) fn serialize_literal_table(observed: &[u64; SYMBOLS]) -> Result<Vec<u8>> {
    serialize_literal_table_in(observed, Global)
}

/// Keep returned description bytes in the original allocator while every tree
/// and FSE temporary allocates through its own fresh clone. Allocator values
/// need not be equivalent; whole owners retain their allocating instance.
pub(crate) fn serialize_literal_table_in<A: Allocator + Clone>(
    observed: &[u64; SYMBOLS],
    allocator: A,
) -> Result<Vec<u8, A>> {
    let lengths = literal_lengths_in(observed, allocator.clone())?;
    let mut payload = encode_weights_in(&explicit_weights(&lengths)?, allocator.clone())?;
    if payload.len() >= 128 {
        payload = encode_weights_in(
            &explicit_weights(&near_flat_lengths(observed))?,
            allocator.clone(),
        )?;
    }
    if payload.is_empty() || payload.len() >= 128 {
        return Err(Error::InvalidState);
    }
    let mut description = Vec::new_in(allocator);
    description
        .try_reserve_exact(payload.len() + 1)
        .map_err(|_| Error::AllocationFailed)?;
    description.push(payload.len() as u8);
    for &byte in &payload {
        description.push(byte);
    }
    Ok(description)
}

/// Pair the actual published table description with its checked canonical codes.
/// Parsing the selected bytes also preserves any near-flat serialization fallback.
#[cfg(test)]
pub(crate) fn build_literal_table(
    observed: &[u64; SYMBOLS],
) -> Result<crate::huffman_codebook::BuiltLiteralTable> {
    build_literal_table_in(observed, Global)
}

#[cfg(test)]
pub(crate) fn build_literal_table_in<A: Allocator + Clone>(
    observed: &[u64; SYMBOLS],
    allocator: A,
) -> Result<crate::huffman_codebook::BuiltLiteralTable<A>> {
    crate::huffman_codebook::BuiltLiteralTable::from_description(serialize_literal_table_in(
        observed, allocator,
    )?)
}

/// Build a sparse inline literal table, preserving the exact selected codes.
/// Zero- and one-symbol inputs belong to raw/RLE selection and are rejected here.
/// Corresponds to the bounded tree/description role of `HUF_compress_internal`.
pub(crate) fn build_sparse_literal_table(
    observed: &[u64; SYMBOLS],
) -> Result<crate::huffman_codebook::BuiltLiteralTable> {
    build_sparse_literal_table_in(observed, Global)
}

/// Build and validate through the selected allocator without replacing the
/// description's allocator during fallback. Borrowing a non-Clone allocator is
/// supported; scratch clones then clone only that external shared reference.
pub(crate) fn build_sparse_literal_table_in<A: Allocator + Clone>(
    observed: &[u64; SYMBOLS],
    allocator: A,
) -> Result<crate::huffman_codebook::BuiltLiteralTable<A>> {
    let total = observed.iter().try_fold(0_u64, |sum, &count| {
        sum.checked_add(count).ok_or(Error::ResourceLimit)
    })?;
    let lengths = tree_lengths_in(observed, total, MAX_DEPTH, allocator.clone())?;
    let mut description = Vec::new_in(allocator);
    if !write_sparse_description(&lengths, &mut description)? {
        // Unavailable descriptions leave the destination empty. Inline selection
        // retains the same complete-alphabet fallback and the original owner.
        if !write_sparse_description(&near_flat_lengths(observed), &mut description)? {
            return Err(Error::InvalidState);
        }
    }
    crate::huffman_codebook::BuiltLiteralTable::from_description(description)
}

/// Select a standalone tree under a caller-supplied hard depth cap.
/// Plain policy values are shared by all codebook accessor backends.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StandaloneDepthPolicy {
    /// Construct once at the requested maximum; the actual tree may be shallower.
    Requested,
    /// Minimize serialized description bytes plus floor(weighted code bits / 8).
    SmallestEstimatedSize,
}

/// Build a standalone Huffman table with a hard maximum depth in `1..=12`.
///
/// Corresponds to the construction and size-estimation roles of
/// `HUF_buildCTable_wksp`, `HUF_optimalTableLog`, and `HUF_writeCTable_wksp`.
/// This independent tree/serialization policy does not promise C-identical depth
/// choices. The fuzz driver's adjustment of its original requested depth belongs
/// outside this API. The cost search evaluates every feasible limit in ascending
/// order, retaining the lower limit on a tie; stream markers and jump tables are
/// excluded from its estimate. Actual codes are reconstructed from the selected
/// description, including sparse holes. `None` means no candidate description
/// fits the format; a fallback may never silently exceed the caller's depth cap.
#[cfg(test)]
pub(crate) fn build_standalone_table(
    observed: &[u64; SYMBOLS],
    maximum_depth: u8,
    policy: StandaloneDepthPolicy,
) -> Result<Option<crate::huffman_codebook::BuiltStandaloneTable>> {
    build_standalone_table_in(observed, maximum_depth, policy, Global)
}

/// Keep each cost candidate with its own allocator clone, then move the entire
/// winning owner. No allocation is detached from or freed through another clone.
#[cfg(test)]
pub(crate) fn build_standalone_table_in<A: Allocator + Clone>(
    observed: &[u64; SYMBOLS],
    maximum_depth: u8,
    policy: StandaloneDepthPolicy,
    allocator: A,
) -> Result<Option<crate::huffman_codebook::BuiltStandaloneTable<A>>> {
    use crate::huffman_codebook::BuiltStandaloneTable;

    if !(1..=12).contains(&maximum_depth) {
        return Err(Error::InvalidParameter);
    }
    let total = observed.iter().try_fold(0_u64, |sum, &count| {
        sum.checked_add(count).ok_or(Error::ResourceLimit)
    })?;
    let present = observed.iter().filter(|&&count| count != 0).count();
    if present < 2 || present > 1 << maximum_depth {
        return Err(Error::InvalidParameter);
    }
    let first_depth = match policy {
        StandaloneDepthPolicy::Requested => maximum_depth,
        StandaloneDepthPolicy::SmallestEstimatedSize => {
            (usize::BITS - (present - 1).leading_zeros()) as u8
        }
    };
    let mut best = None;
    let mut best_size = u128::MAX;
    for depth in first_depth..=maximum_depth {
        let lengths = tree_lengths_in(observed, total, depth, allocator.clone())?;
        let mut description = Vec::new_in(allocator.clone());
        if !write_sparse_description(&lengths, &mut description)? {
            continue;
        }
        let table = BuiltStandaloneTable::from_description(description)?;
        let weighted_bits = observed.iter().enumerate().try_fold(
            0_u128,
            |bits, (symbol, &count)| -> Result<u128> {
                if count == 0 {
                    return Ok(bits);
                }
                let code = table.codebook().code(symbol as u8)?;
                // The checked u64 total and at-most-twelve-bit codes bound this
                // sum below 12 * u64::MAX, well within u128 on every platform.
                Ok(bits + u128::from(count) * u128::from(code.bits))
            },
        )?;
        let size = weighted_bits / 8 + table.description().len() as u128;
        if size < best_size {
            best_size = size;
            best = Some(table);
        }
    }
    Ok(best)
}

/// Serialize validated complete lengths without changing their support or depth.
/// Constant or oversized FSE weights return false without changing the empty
/// destination. All successful writes follow a complete fallible reservation.
fn write_sparse_description<A: Allocator + Clone>(
    lengths: &[u8; SYMBOLS],
    description: &mut Vec<u8, A>,
) -> Result<bool> {
    let maximum = *lengths.iter().max().ok_or(Error::InvalidState)?;
    let last = lengths
        .iter()
        .rposition(|&length| length != 0)
        .ok_or(Error::InvalidState)?;
    let mut weights = [0; SYMBOLS];
    for (weight, &length) in weights.iter_mut().zip(lengths) {
        *weight = if length == 0 { 0 } else { maximum + 1 - length };
    }
    if last <= 128 {
        description
            .try_reserve_exact(1 + last.div_ceil(2))
            .map_err(|_| Error::AllocationFailed)?;
        description.push((127 + last) as u8);
        for pair in weights[..last].chunks(2) {
            description.push((pair[0] << 4) | pair.get(1).copied().unwrap_or(0));
        }
    } else {
        if weights[..last].iter().all(|&weight| weight == weights[0]) {
            return Ok(false);
        }
        let payload = encode_weights_in(&weights[..last], description.allocator().clone())?;
        if payload.len() >= 128 {
            return Ok(false);
        }
        if payload.is_empty() {
            return Err(Error::InvalidState);
        }
        description
            .try_reserve_exact(payload.len() + 1)
            .map_err(|_| Error::AllocationFailed)?;
        description.push(payload.len() as u8);
        for &byte in &payload {
            description.push(byte);
        }
    }
    Ok(true)
}

/// Construct a complete full-alphabet tree with one pseudocount per symbol.
///
/// Corresponds to the tree-construction role of `HUF_buildCTable_wksp`, using
/// indexed parents and deterministic heap ties. A weighted partition bounds
/// depth for distributions whose unconstrained Huffman tree is too deep.
#[cfg(test)]
pub(crate) fn literal_lengths(observed: &[u64; SYMBOLS]) -> Result<[u8; SYMBOLS]> {
    literal_lengths_in(observed, Global)
}

/// Tree setup has one allocation owner and requires only Allocator, not Clone.
pub(crate) fn literal_lengths_in<A: Allocator>(
    observed: &[u64; SYMBOLS],
    allocator: A,
) -> Result<[u8; SYMBOLS]> {
    let mut frequencies = [0_u64; SYMBOLS];
    let mut total = 0_u64;
    for (frequency, &count) in frequencies.iter_mut().zip(observed) {
        *frequency = count.checked_add(1).ok_or(Error::ResourceLimit)?;
        total = total.checked_add(*frequency).ok_or(Error::ResourceLimit)?;
    }
    let mut lengths = tree_lengths_in(&frequencies, total, MAX_DEPTH, allocator)?;
    if lengths.iter().all(|&length| length == 8) {
        lengths = near_flat_lengths(&frequencies);
    }
    Ok(lengths)
}

/// Build a complete tree over exactly the nonzero frequencies. Parent links
/// remain indices even when absent byte values leave holes in the leaf range.
fn tree_lengths_in<A: Allocator>(
    frequencies: &[u64; SYMBOLS],
    total: u64,
    maximum_depth: u8,
    allocator: A,
) -> Result<[u8; SYMBOLS]> {
    if !(1..=12).contains(&maximum_depth) {
        return Err(Error::InvalidParameter);
    }
    let present = frequencies.iter().filter(|&&count| count != 0).count();
    if present < 2 || present > 1 << maximum_depth {
        return Err(Error::InvalidParameter);
    }
    let mut heap = MaxHeap::new_in(allocator);
    heap.try_reserve(present)?;
    for (symbol, &weight) in frequencies.iter().enumerate() {
        if weight != 0 {
            heap.try_push(Reverse((weight, symbol, symbol)))?;
        }
    }
    let mut parents = [usize::MAX; SYMBOLS * 2 - 1];
    let mut next = SYMBOLS;
    while heap.len() > 1 {
        let Reverse((left_weight, left_symbol, left)) = heap.pop().ok_or(Error::InvalidState)?;
        let Reverse((right_weight, right_symbol, right)) = heap.pop().ok_or(Error::InvalidState)?;
        let weight = left_weight
            .checked_add(right_weight)
            .ok_or(Error::ResourceLimit)?;
        parents[left] = next;
        parents[right] = next;
        heap.try_push(Reverse((weight, left_symbol.min(right_symbol), next)))?;
        next += 1;
    }
    let mut lengths = [0_u8; SYMBOLS];
    for (symbol, length) in lengths.iter_mut().enumerate() {
        if frequencies[symbol] == 0 {
            continue;
        }
        let mut node = symbol;
        while parents[node] != usize::MAX {
            *length += 1; // A binary tree with 256 leaves has depth at most 255.
            node = parents[node];
        }
    }
    if lengths.iter().any(|&length| length > maximum_depth) {
        let mut symbols: [usize; SYMBOLS] = core::array::from_fn(|index| index);
        symbols.sort_unstable_by_key(|&symbol| (Reverse(frequencies[symbol]), symbol));
        partition_lengths(
            &symbols[..present],
            frequencies,
            total,
            0,
            maximum_depth,
            &mut lengths,
        );
    }
    Ok(lengths)
}

/// Partition into complete subtrees while reserving enough leaves per level.
/// The validated cap bounds recursion to at most twelve levels. Every child has
/// enough leaf capacity to terminate before that cap, so each shift is bounded.
fn partition_lengths(
    symbols: &[usize],
    frequencies: &[u64; SYMBOLS],
    total: u64,
    depth: u8,
    maximum_depth: u8,
    lengths: &mut [u8; SYMBOLS],
) {
    if symbols.len() == 1 {
        lengths[symbols[0]] = depth;
        return;
    }
    let child_capacity = 1_usize << (maximum_depth - depth - 1);
    let minimum = symbols.len().saturating_sub(child_capacity).max(1);
    let maximum = (symbols.len() - 1).min(child_capacity);
    let mut prefix = 0_u64;
    let mut best = (u128::MAX, minimum, 0_u64);
    for (position, &symbol) in symbols.iter().take(maximum).enumerate() {
        prefix += frequencies[symbol]; // Disjoint partial sum is <= checked total.
        let split = position + 1;
        if split >= minimum {
            let difference = (u128::from(prefix) * 2).abs_diff(u128::from(total));
            if difference < best.0 {
                best = (difference, split, prefix);
            }
        }
    }
    let (_, split, left_total) = best;
    partition_lengths(
        &symbols[..split],
        frequencies,
        left_total,
        depth + 1,
        maximum_depth,
        lengths,
    );
    partition_lengths(
        &symbols[split..],
        frequencies,
        total - left_total,
        depth + 1,
        maximum_depth,
        lengths,
    );
}

/// Avoid a single repeated explicit weight, which is not an FSE weight stream.
/// The Kraft mass is 1/128 + 253/256 + 2/512 = 1.
pub(crate) fn near_flat_lengths(frequencies: &[u64; SYMBOLS]) -> [u8; SYMBOLS] {
    let mut symbols: [usize; SYMBOLS] = core::array::from_fn(|index| index);
    symbols.sort_unstable_by_key(|&symbol| (Reverse(frequencies[symbol]), symbol));
    let mut lengths = [8; SYMBOLS];
    lengths[symbols[0]] = 7;
    lengths[symbols[SYMBOLS - 2]] = 9;
    lengths[symbols[SYMBOLS - 1]] = 9;
    lengths
}

/// Convert a complete tree to explicit weights, leaving the last one implicit.
pub(crate) fn explicit_weights(lengths: &[u8; SYMBOLS]) -> Result<[u8; SYMBOLS - 1]> {
    let maximum = lengths
        .iter()
        .copied()
        .max()
        .ok_or(Error::InvalidParameter)?;
    if maximum > MAX_DEPTH || lengths.contains(&0) {
        return Err(Error::InvalidParameter);
    }
    let mass: usize = lengths
        .iter()
        .map(|&length| 1_usize << (maximum - length))
        .sum();
    if mass != 1 << maximum {
        return Err(Error::InvalidParameter);
    }
    Ok(core::array::from_fn(|symbol| maximum + 1 - lengths[symbol]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, NonCloneAllocator, TrackingAllocator};

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 32,
            fail_at: None,
        })
        .unwrap()
    }

    fn assert_same_codes(
        actual: &crate::huffman_codebook::HuffmanCodebook,
        expected: &crate::huffman_codebook::HuffmanCodebook,
    ) {
        for symbol in 0..=255 {
            assert_eq!(actual.code(symbol), expected.code(symbol));
        }
    }

    fn check_each_failure(operation: impl Fn(&NonCloneAllocator) -> Result<()>) {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        operation(&owner).unwrap();
        let attempts = monitor.snapshot().attempts;
        assert!(attempts > 0);
        monitor.assert_released();
        for ordinal in 1..=attempts {
            let owner = allocator().into_non_clone();
            let monitor = owner.monitor();
            monitor.set_fail_at(NonZeroU64::new(ordinal));
            assert_eq!(operation(&owner), Err(Error::AllocationFailed), "{ordinal}");
            assert_eq!(monitor.snapshot().failures, 1);
            monitor.assert_released();
        }
    }

    #[test]
    fn allocator_tree_leaf_accepts_a_non_clone_owner_by_value() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let counts = core::array::from_fn(|symbol| (symbol % 17) as u64);
        assert_eq!(
            literal_lengths_in(&counts, owner).unwrap(),
            literal_lengths(&counts).unwrap()
        );
        assert_eq!(monitor.snapshot().attempts, 1);
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().owners_dropped, 1);
        monitor.assert_released();
    }

    #[test]
    fn allocator_borrowed_builders_preserve_direct_fse_and_fallback_bytes() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        for last in [1, 128, 129, 255] {
            let mut counts = [0; SYMBOLS];
            counts[0] = 7;
            counts[last] = 11;
            let expected = build_sparse_literal_table(&counts).unwrap();
            let actual = build_sparse_literal_table_in(&counts, &owner).unwrap();
            assert_eq!(actual.description(), expected.description());
            assert_same_codes(actual.codebook(), expected.codebook());
        }
        let expected = build_sparse_literal_table(&[1; SYMBOLS]).unwrap();
        let actual = build_sparse_literal_table_in(&[1; SYMBOLS], &owner).unwrap();
        assert_eq!(actual.codebook().table_log(), 9);
        assert_eq!(actual.description(), expected.description());
        assert_same_codes(actual.codebook(), expected.codebook());
        drop(actual);
        let expected = build_literal_table(&[17; SYMBOLS]).unwrap();
        let actual = build_literal_table_in(&[17; SYMBOLS], &owner).unwrap();
        assert_eq!(actual.description(), expected.description());
        assert_same_codes(actual.codebook(), expected.codebook());
        drop(actual);
        assert_eq!(monitor.snapshot().identities_created, 1);
        monitor.assert_released();
    }

    #[test]
    fn allocator_standalone_depths_and_cost_candidates_preserve_exact_descriptions() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        for depth in 1..=12 {
            let mut counts = [0; SYMBOLS];
            for (symbol, count) in counts.iter_mut().take(usize::from(depth)).enumerate() {
                *count = 1 << (usize::from(depth) - symbol);
            }
            counts[255] = 1;
            for policy in [
                StandaloneDepthPolicy::Requested,
                StandaloneDepthPolicy::SmallestEstimatedSize,
            ] {
                let actual = build_standalone_table_in(&counts, depth, policy, &owner)
                    .unwrap()
                    .unwrap();
                let expected = build_standalone_table(&counts, depth, policy)
                    .unwrap()
                    .unwrap();
                assert_eq!(actual.description(), expected.description());
                assert_same_codes(actual.codebook(), expected.codebook());
            }
        }
        assert_eq!(monitor.snapshot().identities_created, 1);
        monitor.assert_released();
    }

    #[test]
    fn allocator_fresh_clone_domains_keep_outputs_and_temporaries_with_their_owners() {
        let owner = allocator();
        let monitor = owner.monitor();
        let original_id = owner.identity();
        let bytes = serialize_literal_table_in(&[0; SYMBOLS], owner).unwrap();
        assert_eq!(bytes.allocator().identity(), original_id);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert!(monitor.snapshot().identities_created > 2);
        drop(bytes);
        monitor.assert_released();

        let owner = allocator();
        let monitor = owner.monitor();
        let table = build_sparse_literal_table_in(&[1; SYMBOLS], owner).unwrap();
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert!(monitor.snapshot().identities_created > 2);
        drop(table);
        monitor.assert_released();

        let owner = allocator();
        let monitor = owner.monitor();
        let mut counts = [0; SYMBOLS];
        counts[0] = 100;
        counts[1] = 10;
        counts[2] = 1;
        let table = build_standalone_table_in(
            &counts,
            4,
            StandaloneDepthPolicy::SmallestEstimatedSize,
            owner,
        )
        .unwrap()
        .unwrap();
        assert_eq!(monitor.snapshot().live_blocks, 1);
        assert!(monitor.snapshot().identities_created >= 7);
        drop(table);
        monitor.assert_released();
    }

    #[test]
    fn allocator_invalid_counts_and_unavailable_weights_do_not_allocate() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        assert_eq!(
            literal_lengths_in(&[u64::MAX; SYMBOLS], &owner),
            Err(Error::ResourceLimit)
        );
        assert!(matches!(
            build_sparse_literal_table_in(&[0; SYMBOLS], &owner),
            Err(Error::InvalidParameter)
        ));
        assert!(matches!(
            build_standalone_table_in(&[1; SYMBOLS], 7, StandaloneDepthPolicy::Requested, &owner),
            Err(Error::InvalidParameter)
        ));
        let mut destination = Vec::new_in(&owner);
        assert!(!write_sparse_description(&[8; SYMBOLS], &mut destination).unwrap());
        assert!(destination.is_empty());
        assert_eq!(monitor.snapshot().attempts, 0);
        drop(destination);
        monitor.assert_released();
    }

    #[test]
    fn allocator_direct_builder_releases_every_failed_temporary() {
        let mut counts = [0; SYMBOLS];
        counts[0] = 100;
        counts[1] = 1;
        check_each_failure(|owner| {
            let table = build_sparse_literal_table_in(&counts, owner)?;
            assert_eq!(table.description(), &[128, 0x10]);
            Ok(())
        });
    }

    #[test]
    fn allocator_dictionary_fse_and_fallback_builders_release_every_failed_temporary() {
        check_each_failure(|owner| {
            let table = build_literal_table_in(&[0; SYMBOLS], owner)?;
            assert_eq!(table.codebook().table_log(), 9);
            Ok(())
        });
        let mut sparse = [0; SYMBOLS];
        sparse[0] = 10;
        sparse[255] = 1;
        for counts in [sparse, [1; SYMBOLS]] {
            check_each_failure(|owner| {
                let table = build_sparse_literal_table_in(&counts, owner)?;
                assert!(table.description()[0] < 128);
                Ok(())
            });
        }
    }

    #[test]
    fn allocator_standalone_search_releases_failed_and_discarded_candidates() {
        let mut counts = [0; SYMBOLS];
        counts[0] = 100;
        counts[1] = 10;
        counts[255] = 1;
        for policy in [
            StandaloneDepthPolicy::Requested,
            StandaloneDepthPolicy::SmallestEstimatedSize,
        ] {
            check_each_failure(|owner| {
                let table = build_standalone_table_in(&counts, 4, policy, owner)?.unwrap();
                assert!(table.description()[0] < 128);
                Ok(())
            });
        }
    }

    fn standalone_cost(
        counts: &[u64; SYMBOLS],
        table: &crate::huffman_codebook::BuiltStandaloneTable,
    ) -> u128 {
        let bits: u128 = counts
            .iter()
            .enumerate()
            .filter(|&(_, &count)| count != 0)
            .map(|(symbol, &count)| {
                u128::from(count) * u128::from(table.codebook().code(symbol as u8).unwrap().bits)
            })
            .sum();
        bits / 8 + table.description().len() as u128
    }

    #[test]
    fn standalone_trees_obey_every_hard_depth_and_preserve_sparse_support() {
        for depth in 1..=12 {
            for high_symbol in [false, true] {
                let mut counts = [0; SYMBOLS];
                for (symbol, count) in counts.iter_mut().take(usize::from(depth)).enumerate() {
                    *count = 1 << (usize::from(depth) - symbol);
                }
                counts[if high_symbol { 255 } else { usize::from(depth) }] = 1;
                let built =
                    build_standalone_table(&counts, depth, StandaloneDepthPolicy::Requested)
                        .unwrap()
                        .unwrap();
                assert_eq!(built.codebook().table_log(), depth);
                assert_eq!(built.description()[0] < 128, high_symbol);
                let (parsed, used) =
                    crate::huffman::HuffmanTable::read_dictionary(built.description()).unwrap();
                assert_eq!(used, built.description().len());
                let parsed_lengths = parsed.code_lengths();
                for symbol in 0..=255_u8 {
                    let actual = built.codebook().code(symbol);
                    assert_eq!(actual.is_ok(), counts[usize::from(symbol)] != 0);
                    assert_eq!(
                        actual.map_or(0, |code| code.bits),
                        parsed_lengths[usize::from(symbol)]
                    );
                }
                assert_eq!(
                    crate::huffman::HuffmanTable::read(built.description()).is_ok(),
                    depth <= 11
                );
            }
        }
    }

    #[test]
    fn standalone_caps_reject_impossible_support_and_checked_count_overflow() {
        let mut counts = [0; SYMBOLS];
        counts[..3].fill(1);
        for policy in [
            StandaloneDepthPolicy::Requested,
            StandaloneDepthPolicy::SmallestEstimatedSize,
        ] {
            for depth in [0, 1, 13, 255] {
                assert!(matches!(
                    build_standalone_table(&counts, depth, policy),
                    Err(Error::InvalidParameter)
                ));
            }
            assert!(matches!(
                build_standalone_table(&[0; SYMBOLS], 12, policy),
                Err(Error::InvalidParameter)
            ));
            let mut single = [0; SYMBOLS];
            single[255] = 1;
            assert!(matches!(
                build_standalone_table(&single, 12, policy),
                Err(Error::InvalidParameter)
            ));
            let mut overflow = [0; SYMBOLS];
            overflow[0] = u64::MAX;
            overflow[255] = 1;
            assert!(matches!(
                build_standalone_table(&overflow, 12, policy),
                Err(Error::ResourceLimit)
            ));
        }
    }

    #[test]
    fn standalone_unrepresentable_trees_do_not_silently_exceed_the_depth_cap() {
        // Uniform full-alphabet codes have 255 constant explicit weights, which
        // FSE cannot represent. The inline builder's depth-nine fallback remains
        // available only to inline selection, never hidden behind a hard cap.
        for depth in 8..=12 {
            for policy in [
                StandaloneDepthPolicy::Requested,
                StandaloneDepthPolicy::SmallestEstimatedSize,
            ] {
                assert!(
                    build_standalone_table(&[1; SYMBOLS], depth, policy)
                        .unwrap()
                        .is_none()
                );
            }
        }
        assert_eq!(
            build_sparse_literal_table(&[1; SYMBOLS])
                .unwrap()
                .codebook()
                .table_log(),
            9
        );
        let mut counts = [0; SYMBOLS];
        counts[0] = u64::MAX - 1;
        counts[255] = 1;
        assert_eq!(
            build_standalone_table(&counts, 12, StandaloneDepthPolicy::SmallestEstimatedSize)
                .unwrap()
                .unwrap()
                .codebook()
                .table_log(),
            1
        );
    }

    #[test]
    fn standalone_cost_search_selects_the_smallest_serialized_estimate() {
        let mut improved = 0;
        let mut random = 0x632b_e59b_u64;
        for trial in 0..4 {
            let counts: [u64; SYMBOLS] = core::array::from_fn(|symbol| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                if symbol > 128 && symbol < 255 {
                    0
                } else {
                    1 + ((random & 15) << ((trial + symbol) % 4))
                }
            });
            let optimized =
                build_standalone_table(&counts, 12, StandaloneDepthPolicy::SmallestEstimatedSize)
                    .unwrap()
                    .unwrap();
            let mut minimum = u128::MAX;
            let mut first_description = None;
            for depth in 8..=12 {
                if let Some(candidate) =
                    build_standalone_table(&counts, depth, StandaloneDepthPolicy::Requested)
                        .unwrap()
                {
                    let cost = standalone_cost(&counts, &candidate);
                    if cost < minimum {
                        minimum = cost;
                        first_description = Some(Vec::from(candidate.description()));
                    }
                }
            }
            assert_eq!(standalone_cost(&counts, &optimized), minimum);
            assert_eq!(optimized.description(), first_description.unwrap());
            let requested = build_standalone_table(&counts, 12, StandaloneDepthPolicy::Requested)
                .unwrap()
                .unwrap();
            if trial == 0 {
                assert_eq!(minimum, 3307);
                assert_eq!(standalone_cost(&counts, &requested), 3309);
            }
            if minimum < standalone_cost(&counts, &requested) {
                improved += 1;
            }
        }
        assert!(
            improved > 0,
            "cost search needs an actual serialized-size improvement"
        );
    }

    #[test]
    fn sparse_trees_preserve_holes_and_direct_description_boundaries() {
        for last in [1, 2, 15, 127, 128, 129, 254, 255] {
            let mut counts = [0; SYMBOLS];
            counts[0] = 7;
            counts[last] = 11;
            let built = build_sparse_literal_table(&counts).unwrap();
            assert_eq!(built.description()[0] >= 128, last <= 128);
            assert_eq!(built.codebook().code(0).unwrap().bits, 1);
            assert_eq!(built.codebook().code(last as u8).unwrap().bits, 1);
            for symbol in 1..last {
                assert!(built.codebook().code(symbol as u8).is_err());
            }
            if last <= 128 {
                assert_eq!(built.description().len(), 1 + last.div_ceil(2));
                if last % 2 != 0 {
                    assert_eq!(built.description().last().unwrap() & 15, 0);
                }
            }
        }
        let mut two = [0; SYMBOLS];
        two[0] = 1;
        two[1] = 1;
        assert_eq!(
            build_sparse_literal_table(&two).unwrap().description(),
            &[128, 0x10]
        );
    }

    #[test]
    fn sparse_tree_depth_and_constant_weight_fallback_are_paired_correctly() {
        let mut counts = [0; SYMBOLS];
        for (symbol, count) in counts.iter_mut().enumerate().take(60) {
            *count = 1_u64 << symbol;
        }
        let built = build_sparse_literal_table(&counts).unwrap();
        assert!(built.codebook().table_log() <= MAX_DEPTH);
        for symbol in 0..60 {
            assert!(built.codebook().code(symbol).is_ok());
        }
        assert!(built.codebook().code(60).is_err());

        // A flat 256-symbol tree would have 255 identical explicit weights,
        // which FSE cannot encode. The selected fallback is a different tree.
        let flat = build_sparse_literal_table(&[1; SYMBOLS]).unwrap();
        assert_eq!(flat.codebook().code(0).unwrap().bits, 7);
        assert_eq!(flat.codebook().code(254).unwrap().bits, 9);
        assert_eq!(flat.codebook().code(255).unwrap().bits, 9);
        let source: Vec<_> = (0..=255).collect();
        let mut writer = crate::bitwriter::BitWriter::new();
        flat.codebook().encode_stream(&source, &mut writer).unwrap();
        let (table, _) = crate::huffman::HuffmanTable::read(flat.description()).unwrap();
        assert_eq!(
            table
                .decode_stream(writer.as_bytes(), source.len())
                .unwrap(),
            source
        );
    }

    #[test]
    fn sparse_tables_reject_degenerate_and_overflowing_histograms() {
        let mut counts = [0; SYMBOLS];
        assert!(matches!(
            build_sparse_literal_table(&counts),
            Err(Error::InvalidParameter)
        ));
        counts[255] = 1;
        assert!(matches!(
            build_sparse_literal_table(&counts),
            Err(Error::InvalidParameter)
        ));
        counts[0] = u64::MAX;
        assert!(matches!(
            build_sparse_literal_table(&counts),
            Err(Error::ResourceLimit)
        ));
    }

    #[test]
    fn dictionary_table_bytes_preserve_the_module_seven_baseline() {
        let mut random = 0x5eed_f00d_u64;
        let mut fingerprint = 0xcbf29ce484222325_u64;
        for trial in 0..128 {
            let counts = core::array::from_fn(|symbol| {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                if trial == 0 {
                    0
                } else {
                    (random & 255) << ((symbol + trial) % 32)
                }
            });
            let bytes = serialize_literal_table(&counts).unwrap();
            for byte in (bytes.len() as u32).to_le_bytes().into_iter().chain(bytes) {
                fingerprint = (fingerprint ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
        }
        // Captured before sparse-tree refactoring from Module 7 serialization.
        assert_eq!(fingerprint, 17_806_659_356_530_344_110);
    }

    fn check_serialized_tree(observed: &[u64; SYMBOLS]) {
        let encoded = serialize_literal_table(observed).unwrap();
        assert!((1..128).contains(&encoded[0]));
        assert_eq!(encoded.len(), usize::from(encoded[0]) + 1);
        let (_, consumed) = crate::huffman::HuffmanTable::read_dictionary(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        let (_, consumed) = crate::huffman::HuffmanTable::read(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        let learned = explicit_weights(&literal_lengths(observed).unwrap()).unwrap();
        let expected = if encode_weights(&learned).unwrap().len() < 128 {
            learned
        } else {
            explicit_weights(&near_flat_lengths(observed)).unwrap()
        };
        assert_eq!(crate::fse::decode_weights(&encoded[1..]).unwrap(), expected);
        assert_eq!(serialize_literal_table(observed).unwrap(), encoded);
        for length in 0..encoded.len() {
            assert!(crate::huffman::HuffmanTable::read_dictionary(&encoded[..length]).is_err());
        }
    }

    #[test]
    fn serialized_tables_preserve_flat_extreme_and_randomized_literal_trees() {
        check_serialized_tree(&[0; SYMBOLS]);
        check_serialized_tree(&[u32::MAX as u64; SYMBOLS]);
        let mut extreme = [0; SYMBOLS];
        extreme[255] = 1 << 50;
        check_serialized_tree(&extreme);
        for (index, frequency) in extreme.iter_mut().enumerate().take(60) {
            *frequency = 1 << index;
        }
        check_serialized_tree(&extreme);
        let mut state = 0x9e37_79b9_u64;
        for trial in 0..64 {
            let frequencies = core::array::from_fn(|symbol| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state & 1023) << ((symbol + trial) % 32)
            });
            check_serialized_tree(&frequencies);
        }
    }

    #[test]
    fn literal_table_serialization_rejects_overflow_before_building_metadata() {
        assert_eq!(
            serialize_literal_table(&[u64::MAX; SYMBOLS]),
            Err(Error::ResourceLimit)
        );
    }

    fn check_tree(lengths: &[u8; SYMBOLS]) {
        assert!(
            lengths
                .iter()
                .all(|&length| (1..=MAX_DEPTH).contains(&length))
        );
        let mass: u32 = lengths
            .iter()
            .map(|&length| 1_u32 << (MAX_DEPTH - length))
            .sum();
        assert_eq!(mass, 1 << MAX_DEPTH);
        let weights = explicit_weights(lengths).unwrap();
        let maximum = *lengths.iter().max().unwrap();
        let known: u32 = weights.iter().map(|&weight| 1_u32 << (weight - 1)).sum();
        assert_eq!((1 << maximum) - known, 1_u32 << (maximum - lengths[255]));
    }

    #[test]
    fn flat_alphabets_form_a_complete_nonconstant_weight_tree() {
        for count in [0, 1, 1000, u32::MAX as u64] {
            let lengths = literal_lengths(&[count; SYMBOLS]).unwrap();
            check_tree(&lengths);
            assert_eq!(lengths[0], 7);
            assert_eq!(lengths[254..], [9, 9]);
            let weights = explicit_weights(&lengths).unwrap();
            assert!(weights.iter().any(|&weight| weight != weights[0]));
        }
    }

    #[test]
    fn extreme_distributions_keep_every_symbol_and_bound_tree_depth() {
        let mut frequencies = [0; SYMBOLS];
        frequencies[137] = 1 << 50;
        let lengths = literal_lengths(&frequencies).unwrap();
        check_tree(&lengths);
        assert_eq!(lengths[137], 1);
        for (index, frequency) in frequencies.iter_mut().enumerate().take(60) {
            *frequency = 1 << index;
        }
        check_tree(&literal_lengths(&frequencies).unwrap());
    }

    #[test]
    fn randomized_distributions_are_complete_and_deterministic() {
        let mut state = 0x12345678_u64;
        for trial in 0..128 {
            let frequencies = core::array::from_fn(|symbol| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                if trial % 2 == 0 {
                    state & 255
                } else {
                    (state & 1023) << (symbol % 32)
                }
            });
            let first = literal_lengths(&frequencies).unwrap();
            check_tree(&first);
            assert_eq!(first, literal_lengths(&frequencies).unwrap());
        }
    }

    #[test]
    fn counter_overflow_and_invalid_kraft_mass_are_rejected() {
        let mut frequencies = [0; SYMBOLS];
        frequencies[0] = u64::MAX;
        assert_eq!(literal_lengths(&frequencies), Err(Error::ResourceLimit));
        assert_eq!(
            literal_lengths(&[u64::MAX / 128; SYMBOLS]),
            Err(Error::ResourceLimit)
        );
        for lengths in [[0; SYMBOLS], [7; SYMBOLS], [9; SYMBOLS], [12; SYMBOLS]] {
            assert_eq!(explicit_weights(&lengths), Err(Error::InvalidParameter));
        }
    }
}
