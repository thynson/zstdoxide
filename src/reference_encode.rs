//! Bounded execution of pinned no-dictionary Fast, DoubleFast, lazy-family binary-tree and bounded Optimal/Ultra/Ultra2 profiles.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::reference_frame_encode::{
    compress_reference_in, compress_reference_into_in, compress_reference_with_dictionary_in,
    compress_reference_with_dictionary_into_in,
};
use crate::reference_search::ReferenceSearchPlan;
use crate::{CompressionParameterRequest, DictionaryHandle, Result};

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
/// strategy-7/8/9 requests admit 64..=32768 bytes, each fitting its window
/// within the stated resource caps; default levels 11/12 (Optimal), 13/14 (Ultra)
/// and 16/17 (Ultra2) also work for
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
/// lengths 0..=131072. Optimal, Ultra and Ultra2 support 64..=32768 source
/// bytes. Each source must fit its window, with window logs
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
/// path relaxation. Ultra2 first runs a discarded Ultra pass over the first block
/// to seed statistics, invalidates history by rebasing checked indices, restores
/// original repeat offsets and encodes that block again with retained statistics.
/// Later Ultra2 blocks use one retained Ultra pass. All three
/// use the sufficient-match target and automatic literal compression. Each
/// uses a single complete block through 16384 bytes. Sources of
/// 16385..=32768 bytes use three or four balanced blocks of at most 8192 bytes.
/// Search and price history survive raw/RLE blocks; only compressed blocks
/// publish candidate repeat offsets and entropy. Window expiry and dictionary
/// paths remain excluded.
/// Lazy uses checked depth-one speculative gains; Lazy2 and BinaryTreeLazy2
/// use depth-two gains and retain rejected
/// sampling probes; Greedy stops at its first eligible baseline candidate.
/// The preceding six non-Optimal strategies cap blocks at the smaller of the
/// selected window and 128 KiB. Retained Optimal, Ultra and Ultra2 use their balanced schedules.
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

/// Compress one known-size parameterized frame into caller-owned storage.
///
/// The frame bytes equal [`compress_with_parameters`] for the same input and
/// request. Complete header and block appends are atomic. A source that does
/// not fit the selected profile, an unsupported request or a checked size
/// failure is reported before the first output byte. A destination shorter
/// than the emitted frame returns [`crate::Error::ResourceLimit`] after any
/// earlier whole appends; no byte outside the supplied slice is changed.
/// Scratch uses independent clones of the supplied allocator and is released
/// before return. Success returns the exact written length; the output borrow
/// does not survive the call.
///
/// ```compile_fail
/// let mut storage = [0_u8; 64];
/// let request = zstdoxide::CompressionParameterRequest::with_level(1);
/// let _ = zstdoxide::compress_into_with_parameters(&storage[..16], &mut storage, &request);
/// ```
pub fn compress_into_with_parameters(
    input: &[u8],
    output: &mut [u8],
    request: &CompressionParameterRequest,
) -> Result<usize> {
    compress_into_with_parameters_in(input, output, request, Global)
}

/// Write a parameterized frame into caller-owned storage using `allocator`.
///
/// Admission, byte identity and output-guard behavior match
/// [`compress_into_with_parameters`]. The caller destination is neither
/// allocated nor freed here. All scratch owners retain fresh clones of the
/// supplied allocator, including a borrowed non-Clone allocator, and release
/// before return. Ordinary allocator cloning retains the caller's own
/// allocation/panic behavior.
pub fn compress_into_with_parameters_in<A: Allocator + Clone>(
    input: &[u8],
    output: &mut [u8],
    request: &CompressionParameterRequest,
    allocator: A,
) -> Result<usize> {
    let plan = ReferenceSearchPlan::new(request, input.len())?;
    compress_reference_into_in(input, output, plan, allocator)
}

/// Compress one parameterized frame with an immutable dictionary attachment.
///
/// Resolves `request` against the actual source length and the dictionary's
/// matchable content length, matching the pinned CDict content-size tier, then
/// runs the same admitted reference profile as [`compress_with_parameters`]
/// over a per-call combined borrow: only the dictionary suffix that fits the
/// resolved window can provide matches, so dictionary content beyond that
/// window expires exactly as `ZSTD_checkDictValidity` requires. A formatted
/// dictionary's validated entropy seeds the first block; its nonzero identifier
/// is written in the frame header. Raw dictionaries supply content without
/// prepared entropy.
///
/// The supported dictionary subset is the Fast, DoubleFast, Greedy, Lazy,
/// Lazy2 and BinaryTreeLazy2 families. Optimal, Ultra and Ultra2 requests - and
/// their retained source tiers - return [`crate::Error::UnsupportedFeature`]
/// for nonzero dictionary content instead of silently dropping the dictionary.
/// Profile/resource caps and source-size tiers are otherwise identical to
/// [`compress_with_parameters`].
///
/// The result owns only its output allocation. It borrows neither input,
/// request nor dictionary, and a borrowed [`crate::DictionaryView`] can be the
/// attachment. Resolution and prepared-table errors surface before output
/// admission; each scratch owner is released through its exact allocator.
/// Corresponds to the supported one-shot subset of combining resolved
/// `ZSTD_compress2` parameters with an attached CDict, without claiming native
/// workspace reuse or complete parameter/context parity.
///
/// ```
/// use zstdoxide::{
///     CompressionParameterRequest, DecodeOptions, Dictionary,
///     compress_with_parameters_and_dictionary, decompress_with_dictionary,
/// };
/// let dictionary = Dictionary::raw(b"shared reference dictionary content")?.into_shared();
/// let input = b"shared reference dictionary content";
/// let frame = compress_with_parameters_and_dictionary(
///     input,
///     &CompressionParameterRequest::with_level(4),
///     &dictionary,
/// )?;
/// assert_eq!(
///     decompress_with_dictionary(&frame, &dictionary, DecodeOptions::new(input.len()))?,
///     input
/// );
/// # Ok::<(), zstdoxide::Error>(())
/// ```
///
/// A borrowed view cannot outlive its dictionary owner:
///
/// ```compile_fail
/// use zstdoxide::{CompressionParameterRequest, Dictionary, compress_with_parameters_and_dictionary};
/// let view = {
///     let dictionary = Dictionary::raw(b"prefix").unwrap();
///     dictionary.view()
/// };
/// let _ = compress_with_parameters_and_dictionary(
///     b"input",
///     &CompressionParameterRequest::with_level(1),
///     &view,
/// );
/// ```
pub fn compress_with_parameters_and_dictionary<D: DictionaryHandle>(
    input: &[u8],
    request: &CompressionParameterRequest,
    dictionary: &D,
) -> Result<Vec<u8>> {
    compress_with_parameters_and_dictionary_in(input, request, dictionary, Global)
}

/// Allocate a parameterized dictionary frame in `A`.
///
/// Behavior, admission and supported strategies match
/// [`compress_with_parameters_and_dictionary`]. The result retains the
/// supplied allocator value; every scratch owner uses a fresh clone and is
/// released before return. Input, request and dictionary borrows never escape.
/// Failure releases partial output and scratch, and leaves the dictionary
/// owner unchanged.
pub fn compress_with_parameters_and_dictionary_in<A: Allocator + Clone, D: DictionaryHandle>(
    input: &[u8],
    request: &CompressionParameterRequest,
    dictionary: &D,
    allocator: A,
) -> Result<Vec<u8, A>> {
    let view = dictionary.view();
    let plan =
        ReferenceSearchPlan::new_with_dictionary(request, input.len(), view.content().len())?;
    compress_reference_with_dictionary_in(input, view, plan, allocator)
}

/// Compress one parameterized dictionary frame into caller-owned storage.
///
/// Uses the exact parameters, dictionary staging, entropy seed and frame flags
/// of [`compress_with_parameters_and_dictionary`]. Request resolution,
/// dictionary-profile rejection and prepared-table failures occur before the
/// first output byte. Complete header and block appends are atomic; a short
/// destination returns [`crate::Error::ResourceLimit`] after earlier whole
/// appends and never changes bytes outside the supplied slice. Success returns
/// the exact written length.
pub fn compress_into_with_parameters_and_dictionary<D: DictionaryHandle>(
    input: &[u8],
    output: &mut [u8],
    request: &CompressionParameterRequest,
    dictionary: &D,
) -> Result<usize> {
    compress_into_with_parameters_and_dictionary_in(input, output, request, dictionary, Global)
}

/// Write a parameterized dictionary frame into caller storage using `A`.
///
/// Byte identity and output-guard behavior match
/// [`compress_into_with_parameters_and_dictionary`]. The caller destination is
/// never allocated or freed here. Dictionary ownership remains with the
/// attachment, and every scratch owner receives an independent allocator clone
/// that is released before return, including for borrowed non-Clone allocators.
pub fn compress_into_with_parameters_and_dictionary_in<
    A: Allocator + Clone,
    D: DictionaryHandle,
>(
    input: &[u8],
    output: &mut [u8],
    request: &CompressionParameterRequest,
    dictionary: &D,
    allocator: A,
) -> Result<usize> {
    let view = dictionary.view();
    let plan =
        ReferenceSearchPlan::new_with_dictionary(request, input.len(), view.content().len())?;
    compress_reference_with_dictionary_into_in(input, output, view, plan, allocator)
}
