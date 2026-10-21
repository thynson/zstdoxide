//! Bounded execution of pinned no-dictionary Fast, DoubleFast, lazy-family binary-tree and bounded Optimal/Ultra/Ultra2 profiles.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::reference_frame_encode::compress_reference_in;
use crate::reference_search::ReferenceSearchPlan;
use crate::{CompressionParameterRequest, Result};

/// Compress one known-size frame using an admitted reference level.
///
/// Level 1 and negative levels select Fast for every source-size tier. Level 2
/// selects DoubleFast for inputs larger than 128 KiB and no larger than 256 KiB,
/// and Fast otherwise. Level 0 means level 3; both select DoubleFast. Level 4
/// selects DoubleFast for inputs larger than 16 KiB and no larger than 128 KiB,
/// and for inputs larger than 256 KiB; its remaining source tiers select Greedy.
/// Levels 5 and 6 are also supported across source tiers. Levels 7 through 9
/// are supported through 256 KiB; levels 10 through 12 are supported for inputs
/// larger than 16 KiB and no larger than 256 KiB. Level 10 at smaller sizes and
/// levels 7 through 17 above 256 KiB exceed local resource caps. Resolved
/// strategy-7/8 requests admit 64..=32768 bytes; strategy-9 requests admit
/// 64..=16384 bytes, each fitting its window within the stated resource caps; default levels 11/12 (Optimal), 13/14 (Ultra) and 16/17 (Ultra2) also work for
/// sizes 64..=2048. Explicit overrides can change this outcome.
/// Admission uses the fully resolved strategy and its resource limits, rather
/// than a level allowlist.
///
/// Frames include the actual content size, omit a checksum and dictionary ID,
/// and advertise a window no larger than the resolved window. The parser
/// follows pinned zstd v1.5.7 sampling and repeat-offset behavior; entropy choices
/// are independent Rust implementations, so C-identical bytes or compression
/// ratios are not promised. This API does not change [`crate::compress`] or the
/// named streaming policies. Dictionary, worker and long-distance modes are not
/// exposed here. See [`compress_with_parameters`] for operational resource caps.
///
/// Corresponds to the no-dictionary one-shot role of `ZSTD_compress`.
///
/// ```
/// use zstdoxide::{compress_at_level, decompress};
/// let input = b"reference DoubleFast, reference DoubleFast, reference DoubleFast";
/// let frame = compress_at_level(input, 0)?;
/// assert_eq!(decompress(&frame, input.len())?, input);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
pub fn compress_at_level(input: &[u8], level: i32) -> Result<Vec<u8>> {
    compress_at_level_in(input, level, Global)
}

/// Allocate a level-selected reference frame and scratch through this allocator.
///
/// Behavior and admission match [`compress_at_level`]. The returned vector
/// retains the original allocator value; every temporary owns a fresh clone and
/// is released before return. References to non-Clone allocators are supported.
/// Admission and checked output-bound errors occur before allocation or cloning.
/// Collection requests are fallible; errors release partial output and scratch.
/// Ordinary allocator cloning may allocate or panic according to its own
/// contract. No source or parameter borrow escapes in the result.
pub fn compress_at_level_in<A: Allocator + Clone>(
    input: &[u8],
    level: i32,
    allocator: A,
) -> Result<Vec<u8, A>> {
    compress_with_parameters_in(
        input,
        &CompressionParameterRequest::with_level(level),
        allocator,
    )
}

/// Compress one known-size frame using explicit pinned-reference requests.
///
/// Resolve `request` using the actual source length and dictionary size zero.
/// A source-size hint never overrides that known length, including known empty
/// input. The request is unchanged. Effective [`crate::ReferenceStrategy::Fast`],
/// [`crate::ReferenceStrategy::DoubleFast`], [`crate::ReferenceStrategy::Greedy`],
/// [`crate::ReferenceStrategy::Lazy`], [`crate::ReferenceStrategy::Lazy2`] and
/// [`crate::ReferenceStrategy::BinaryTreeLazy2`], [`crate::ReferenceStrategy::Optimal`]
/// [`crate::ReferenceStrategy::Ultra`] and [`crate::ReferenceStrategy::Ultra2`]
/// are implemented within their separately bounded scopes.
///
/// Fast supports effective window logs 10..=20 and hash logs 6..=18. DoubleFast
/// supports window logs 10..=21 and independent hash/chain logs 6..=18.
/// Greedy, Lazy and Lazy2 support window logs 10..=21 and hash logs 6..=19.
/// Their resolved window selects
/// chain storage at logs 10..=14 (chain log <=18, search log <=8), and row storage
/// at logs 15..=21 (unused chain log is not resource-capped, at most 64 visits).
/// BinaryTreeLazy2 supports window logs 10..=21, independent hash/chain logs
/// 6..=19 and search logs 1..=8, always using tree storage.
/// The preceding six strategies support minimum-match settings 3..=7 and target
/// lengths 0..=131072. Optimal and Ultra support 64..=32768 source bytes; Ultra2
/// supports 64..=16384 bytes. Each source must fit its window, with window logs
/// 10..=16, hash/chain logs 6..=12, search logs 1..=6, minimum-match settings
/// 3..=7 and targets 0..=4095. An otherwise
/// valid implemented profile outside these caps returns [`crate::Error::ResourceLimit`]. Caps
/// apply after known-size adjustment, not to the unadjusted requested values.
/// Fast owns one table of `2^hash_log` usize indices into the borrowed source.
/// DoubleFast owns separate long/short hash tables of `2^hash_log` and
/// `2^chain_log` indices; its chain log sizes a hash table, not a chain. No source
/// copy or inactive algorithm storage is allocated. Search log is validated but
/// creates no visit budget for either strategy; Fast also ignores chain log.
///
/// Greedy, Lazy and Lazy2 own two independent tables: chain heads/links of `2^hash_log` and
/// `2^chain_log` usize positions, or `2^hash_log` usize positions and byte tags
/// for rows. Rows use a fresh context salt and scalar hashing on every target.
/// Chain search visits at most `2^search_log` candidates; rows cap that at 64.
/// BinaryTreeLazy2 owns `2^hash_log` heads and `2^chain_log` paired edge cells
/// representing `2^(chain_log-1)` nodes. It performs bounded deferred sorting
/// and at most `2^search_log` best-match visits with checked offset-cost gains.
/// Sampled history survives raw/RLE fallback; only complete compressed blocks
/// publish tentative entropy and wire repeats.
///
/// Minimum-match setting 3 selects four-byte search keys; Fast/DoubleFast settings
/// 4..=7 choose their respective key widths. The lazy family and binary tree clamp key widths to 4..=6.
/// DoubleFast additionally samples eight-byte long keys. Emitted matches have a
/// four-byte minimum. Positive target length controls Fast acceleration and
/// disables literal Huffman/RLE selection. DoubleFast, the lazy family and binary tree ignore this
/// control and retain normal literal selection. Complete block RLE remains
/// independently available to the preceding strategies. Optimal, Ultra and Ultra2 preserve three-byte
/// keys and three-byte matches for minimum 3, otherwise using four-byte minimum
/// matches. Optimal uses pinned optLevel0 path/frequency decisions; Ultra uses
/// coupled optLevel2 fractional prices, match-plus-literal alternatives and thorough
/// path relaxation. Ultra2 first runs a discarded Ultra pass to seed statistics,
/// invalidates history by rebasing checked indices, restores original repeat
/// offsets and encodes a second pass with the retained statistics. All three
/// use the sufficient-match target and automatic literal compression. Each
/// uses a single complete block through 16384 bytes. Optimal and Ultra sources of
/// 16385..=32768 bytes use three or four balanced blocks of at most 8192 bytes.
/// Search and price history survive raw/RLE blocks; only compressed blocks
/// publish candidate repeat offsets and entropy. Retained Ultra2, window
/// expiry and dictionary paths remain excluded.
/// Lazy uses checked depth-one speculative gains; Lazy2 and BinaryTreeLazy2
/// use depth-two gains and retain rejected
/// sampling probes; Greedy stops at its first eligible baseline candidate.
/// The preceding six non-Optimal strategies cap blocks at the smaller of the
/// selected window and 128 KiB. Retained Optimal and Ultra use their balanced schedules.
///
/// Frame flags and independent entropy qualifications match [`compress_at_level`].
/// Corresponds to applying compression parameters before `ZSTD_compress2` in a
/// fresh no-dictionary context, within the stated algorithm and resource scope.
///
/// ```
/// use zstdoxide::{CompressionParameterRequest, compress_with_parameters, decompress};
/// let input = b"custom Fast source, custom Fast source";
/// let mut request = CompressionParameterRequest::with_level(1);
/// request.set_window_log(17)?;
/// request.set_hash_log(14)?;
/// request.set_min_match(5)?;
/// let frame = compress_with_parameters(input, &request)?;
/// assert_eq!(decompress(&frame, input.len())?, input);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
pub fn compress_with_parameters(
    input: &[u8],
    request: &CompressionParameterRequest,
) -> Result<Vec<u8>> {
    compress_with_parameters_in(input, request, Global)
}

/// Allocate a requested reference frame and temporary codec storage in `A`.
///
/// Admission and frame behavior match [`compress_with_parameters`]. Ownership,
/// failure ordering and allocator-clone semantics match [`compress_at_level_in`].
/// The result owns only its output allocation and borrows neither source nor
/// request; each independent scratch owner is released through its exact
/// allocating value before return. No allocator equivalence between clones is
/// assumed. This API also accepts a borrowed non-Clone allocator as `&allocator`.
pub fn compress_with_parameters_in<A: Allocator + Clone>(
    input: &[u8],
    request: &CompressionParameterRequest,
    allocator: A,
) -> Result<Vec<u8, A>> {
    let plan = ReferenceSearchPlan::new(request, input.len())?;
    compress_reference_in(input, plan, allocator)
}
