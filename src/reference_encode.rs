//! Bounded execution of pinned no-dictionary Fast, DoubleFast and Greedy profiles.

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
/// Higher levels need unsupported strategies unless explicitly overridden.
/// Admission uses the fully resolved strategy and its resource limits, rather
/// than a level allowlist.
/// Unsupported strategies return [`crate::Error::UnsupportedFeature`].
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
/// [`crate::ReferenceStrategy::DoubleFast`] and [`crate::ReferenceStrategy::Greedy`]
/// are implemented; another valid strategy returns
/// [`crate::Error::UnsupportedFeature`] before resource admission.
///
/// Fast supports effective window logs 10..=20 and hash logs 6..=18. DoubleFast
/// supports window logs 10..=21 and independent hash/chain logs 6..=18. Greedy
/// supports window logs 10..=21 and hash logs 6..=19. Its resolved window selects
/// chain storage at logs 10..=14 (chain log <=18, search log <=8), and row storage
/// at logs 15..=21 (unused chain log is not resource-capped, at most 64 visits).
/// All support minimum-match settings 3..=7 and target lengths 0..=131072. An otherwise
/// valid implemented profile outside these caps returns [`crate::Error::ResourceLimit`]. Caps
/// apply after known-size adjustment, not to the unadjusted requested values.
/// Fast owns one table of `2^hash_log` usize indices into the borrowed source.
/// DoubleFast owns separate long/short hash tables of `2^hash_log` and
/// `2^chain_log` indices; its chain log sizes a hash table, not a chain. No source
/// copy or inactive algorithm storage is allocated. Search log is validated but
/// creates no visit budget for either strategy; Fast also ignores chain log.
///
/// Greedy owns two independent tables: chain heads/links of `2^hash_log` and
/// `2^chain_log` usize positions, or `2^hash_log` usize positions and byte tags
/// for rows. Rows use a fresh context salt and scalar hashing on every target.
/// Chain search visits at most `2^search_log` candidates; rows cap that at 64.
/// Sampled history survives raw/RLE fallback; only complete compressed blocks
/// publish tentative entropy and wire repeats.
///
/// Minimum-match setting 3 selects four-byte search keys; Fast/DoubleFast settings
/// 4..=7 choose their respective key widths. Greedy clamps key widths to 4..=6.
/// DoubleFast additionally samples eight-byte long keys. Emitted matches have a
/// four-byte minimum. Positive target length controls Fast acceleration and
/// disables literal Huffman/RLE selection. DoubleFast and Greedy ignore this
/// control and retain normal literal selection. Complete block RLE remains
/// independently available for all three strategies.
/// Block size is the smaller of the selected window and 128 KiB.
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
