//! Bounded execution of the pinned reference's no-dictionary Fast profiles.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::fast_encode::compress_fast_in;
use crate::{CompressionParameterRequest, Result};

/// Compress one known-size frame using an admitted reference Fast level.
///
/// Level 1 and negative levels select Fast for every source-size tier. Level 2
/// is supported except when the source is larger than 128 KiB and no larger
/// than 256 KiB, where the pinned profile selects the unimplemented DoubleFast
/// algorithm. Level 0 means level 3, which is also currently unsupported.
/// Other levels are admitted only if their fully resolved strategy is Fast.
/// Unsupported strategies return [`crate::Error::UnsupportedFeature`].
///
/// Frames include the actual content size, omit a checksum and dictionary ID,
/// and advertise a window no larger than the resolved Fast window. The parser
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
/// let input = b"reference Fast, reference Fast, reference Fast";
/// let frame = compress_at_level(input, -3)?;
/// assert_eq!(decompress(&frame, input.len())?, input);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
pub fn compress_at_level(input: &[u8], level: i32) -> Result<Vec<u8>> {
    compress_at_level_in(input, level, Global)
}

/// Allocate a level-selected Fast frame and all scratch through this allocator.
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
/// input. The request is unchanged. Only effective [`crate::ReferenceStrategy::Fast`]
/// is implemented here; another valid strategy returns
/// [`crate::Error::UnsupportedFeature`] before resource admission.
///
/// Effective window logs 10..=20, hash logs 6..=18, minimum-match settings 3..=7,
/// and target lengths 0..=131072 are supported. An otherwise valid Fast profile
/// outside the operational caps returns [`crate::Error::ResourceLimit`]. Caps
/// apply after known-size adjustment, not to the unadjusted requested values.
/// The search table contains `2^hash_log` usize indices into the borrowed source;
/// no source copy or unused chain table is allocated. Chain/search fields remain
/// validated reference parameters but create no additional Fast search storage.
///
/// Minimum-match setting 3 selects four-byte search keys; settings 4..=7 choose
/// their respective key widths. Emitted matches have a four-byte minimum.
/// Positive target length controls Fast acceleration and disables literal
/// Huffman/RLE selection; complete block RLE remains independently available.
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

/// Allocate a requested Fast frame and all temporary codec storage in `A`.
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
    compress_fast_in(input, request, allocator)
}
