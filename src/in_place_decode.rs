//! Suffix-input decoding within one exclusively borrowed allocation.

use crate::cursor::ByteCursor;
use crate::frame::{BlockHeader, BlockKind, MAX_BLOCK_SIZE};
use crate::history::SequenceOutput;
use crate::huffman::decode_literals_with_dictionary_view_in;
use crate::in_place_buffer::InPlaceBuffer;
use crate::sequences::SequenceState;
use crate::slice_history::{DictionarySliceOutput, SliceHistory};
use crate::xxhash::checksum;
use crate::{DecodeOptions, DictionaryView, Error, FrameHeader, Result};
use core::alloc::Allocator;

/// Decompress suffix-encoded frames within one exclusively borrowed buffer.
///
/// Input occupies `buffer[input_start..]`; decoded output starts at zero.
/// Standard, skippable and concatenated frames are supported. Dictionaries,
/// prefixes and legacy frames are unsupported. Checksums are always verified.
/// Empty input is incomplete; valid empty or skippable frames can return zero.
/// The buffer length is the combined output limit and the default advertised
/// window limit is 128 MiB.
///
/// Success returns the written prefix length and leaves every byte after it
/// unchanged. On error, bytes inside the output allowance can be provisional;
/// unread encoded bytes and bytes beyond the allowance are preserved. Invalid
/// `input_start` is rejected before any writes. No rollback or partial progress
/// is promised. Even when the decoded size fits, insufficient space between
/// output and an unread compressed/RLE block returns [`Error::ResourceLimit`].
///
/// For supported valid frames, placing input at the end of a buffer at least
/// `decoded_size + decompression_margin(input)?` bytes long is sufficient when
/// the input also fits. The margin scanner validates framing structure only.
/// Smaller layouts can succeed, including actual raw-block overlap; decoding
/// always checks live input boundaries. Unknown content sizes are supported.
///
/// No complete input clone, output allocation, or history copy is made.
/// Compressed blocks can allocate fallible literal and entropy scratch.
/// Corresponds to the overlapping-buffer use of upstream `ZSTD_decompress`.
///
/// ```
/// use zstdoxide::{compress, decompression_margin, decompress_in_place};
/// let source = b"in-place output, in-place output";
/// let frame = compress(source)?;
/// let capacity = (source.len() + decompression_margin(&frame)?).max(frame.len());
/// let mut buffer = vec![0xa5; capacity];
/// let input_start = capacity - frame.len();
/// buffer[input_start..].copy_from_slice(&frame);
/// let written = decompress_in_place(&mut buffer, input_start)?;
/// assert_eq!(&buffer[..written], source);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
pub fn decompress_in_place(buffer: &mut [u8], input_start: usize) -> Result<usize> {
    let options = DecodeOptions::new(buffer.len());
    decompress_in_place_with_options(buffer, input_start, options)
}

/// Decode in place with caller-selected scratch allocation.
///
/// The supplied slice is never allocated, resized, adopted or freed. Temporary
/// collections use clones of `allocator` and release their storage on success
/// and failure. A borrowed non-Clone allocator is accepted as `&allocator`.
/// Ordinary allocator cloning retains its own allocation and panic semantics.
/// Raw/RLE/skippable paths require no codec payload allocation. All other
/// contracts follow [`decompress_in_place`].
pub fn decompress_in_place_in<A: Allocator + Clone>(
    buffer: &mut [u8],
    input_start: usize,
    allocator: A,
) -> Result<usize> {
    let options = DecodeOptions::new(buffer.len());
    decompress_in_place_with_options_in(buffer, input_start, options, allocator)
}

/// Decode in place with explicit output and advertised-window limits.
///
/// The effective output allowance is `min(buffer.len(), options.max_output_size)`.
/// Bytes beyond it remain unchanged even on error. The window limit is
/// independent. Output capacity does not replace the unread-input fences.
/// The ownership, layout and format contracts follow [`decompress_in_place`].
pub fn decompress_in_place_with_options(
    buffer: &mut [u8],
    input_start: usize,
    options: DecodeOptions,
) -> Result<usize> {
    decompress_in_place_with_options_in(buffer, input_start, options, alloc::alloc::Global)
}

/// Decode in place with explicit limits and a scratch-only allocator.
///
/// Combines [`decompress_in_place_with_options`] bounds with
/// [`decompress_in_place_in`] allocation ownership. One exclusive mutable slice
/// expresses the shared allocation without independently aliasing Rust borrows.
/// No borrow of that slice or its input survives this call.
pub fn decompress_in_place_with_options_in<A: Allocator + Clone>(
    buffer: &mut [u8],
    input_start: usize,
    options: DecodeOptions,
    allocator: A,
) -> Result<usize> {
    decode_in_place(buffer, input_start, options, allocator)
}

/// Decompress suffix-encoded frames in place with one persistent dictionary.
///
/// The buffer and its input placement follow [`decompress_in_place`]. The selected
/// raw/prepared dictionary applies to every standard frame; skippable frames keep
/// the selection. Nonzero dictionary IDs must match, and an omitted ID still uses
/// the selection. Every frame resets its match history, repeat offsets and entropy
/// overrides independently. Dictionary bytes do not count against output capacity.
///
/// Success preserves the entire unused suffix. Errors can leave provisional
/// writes inside `min(buffer.len(), options.max_output_size)` but preserve unread
/// input and bytes beyond that allowance. Window limits remain independent.
/// Checksums and content sizes are always validated. Dictionary mismatch precedes
/// frame resource checks; native numeric errors and precedence are not promised.
/// The call borrows the selected handle without cloning or changing it.
///
/// Corresponds to the shared-buffer use of upstream `ZSTD_decompress_usingDDict`.
/// Single-use prefixes and incremental overlapping sessions remain outside this API.
///
/// ```
/// use zstdoxide::{Dictionary, DecodeOptions, compress_with_dictionary,
///     decompression_margin, decompress_in_place_with_dictionary};
/// let dictionary = Dictionary::raw(b"hello dictionary")?.into_shared();
/// let source = b"hello dictionary hello";
/// let encoded = compress_with_dictionary(source, &dictionary)?;
/// let capacity = (source.len() + decompression_margin(&encoded)?).max(encoded.len());
/// let mut buffer = vec![0xa5; capacity];
/// let start = capacity - encoded.len();
/// buffer[start..].copy_from_slice(&encoded);
/// let size = decompress_in_place_with_dictionary(&mut buffer, start, &dictionary,
///                                               DecodeOptions::new(source.len()))?;
/// assert_eq!(&buffer[..size], source);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
pub fn decompress_in_place_with_dictionary(
    buffer: &mut [u8],
    input_start: usize,
    dictionary: &crate::SharedDictionary,
    options: DecodeOptions,
) -> Result<usize> {
    decompress_in_place_with_dictionary_in(
        buffer,
        input_start,
        dictionary,
        options,
        alloc::alloc::Global,
    )
}

/// Decode in place with independent dictionary and scratch allocator owners.
///
/// Accepts a shared dictionary owner or a borrowed [`DictionaryView`] through the
/// sealed [`crate::DictionaryHandle`] contract. The dictionary and its allocator
/// are not cloned; all temporary collections use clones of `allocator` and release
/// their storage on success or failure. Its type can differ from the dictionary
/// allocator. A reference to a non-Clone scratch allocator is accepted. Cloning
/// retains the allocator's own allocation/panic semantics.
///
/// There is no copied dictionary/output history, complete input clone, or output
/// allocation. Raw, RLE and skippable paths make no codec payload allocation merely
/// because a dictionary is selected. The destination is never adopted or freed.
/// Other contracts follow [`decompress_in_place_with_dictionary`].
///
/// A borrowed selection must remain within its original owner's lifetime:
///
/// ```compile_fail,E0597
/// use zstdoxide::{Dictionary, DecodeOptions, decompress_in_place_with_dictionary_in};
/// let view = {
///     let owner = Dictionary::raw(b"dictionary").unwrap();
///     owner.view()
/// };
/// let mut buffer = [0_u8; 32];
/// let _ = decompress_in_place_with_dictionary_in(&mut buffer, 0, &view,
///     DecodeOptions::new(32), std::alloc::Global);
/// ```
///
/// A view exposes immutable dictionary content, which cannot be supplied as the
/// exclusive mutable in-place destination:
///
/// ```compile_fail,E0308
/// use zstdoxide::{Dictionary, DecodeOptions, decompress_in_place_with_dictionary_in};
/// let owner = Dictionary::raw(b"dictionary").unwrap();
/// let view = owner.view();
/// let _ = decompress_in_place_with_dictionary_in(view.content(), 0, &view,
///     DecodeOptions::new(10), std::alloc::Global);
/// ```
pub fn decompress_in_place_with_dictionary_in<A: Allocator + Clone, D: crate::DictionaryHandle>(
    buffer: &mut [u8],
    input_start: usize,
    dictionary: &D,
    options: DecodeOptions,
    allocator: A,
) -> Result<usize> {
    decode_in_place_with_dictionary_view(
        buffer,
        input_start,
        options,
        allocator,
        Some(dictionary.view()),
    )
}

/// The private coordinator corresponding to `ZSTD_decompressFrame`.
/// Metadata borrows end before any writes; compressed payload borrows are
/// disjoint from resumed history, and raw moves explicitly permit overlap.
pub(crate) fn decode_in_place<A: Allocator + Clone>(
    bytes: &mut [u8],
    input_start: usize,
    options: DecodeOptions,
    allocator: A,
) -> Result<usize> {
    decode_in_place_with_dictionary_view(bytes, input_start, options, allocator, None)
}

/// A persistent immutable selection supplies fallback state freshly per frame.
/// Dictionary bytes cannot alias the exclusive source/output borrow in safe Rust.
pub(crate) fn decode_in_place_with_dictionary_view<A: Allocator + Clone>(
    bytes: &mut [u8],
    input_start: usize,
    options: DecodeOptions,
    allocator: A,
    dictionary: Option<DictionaryView<'_>>,
) -> Result<usize> {
    let mut buffer = InPlaceBuffer::new(bytes, input_start, options.max_output_size)?;
    if buffer.remaining().is_empty() {
        return Err(Error::UnexpectedEof);
    }
    while !buffer.remaining().is_empty() {
        let mut prefix = ByteCursor::new(buffer.remaining());
        let magic = prefix.little_endian(4)? as u32;
        if magic & 0xffff_fff0 == 0x184d_2a50 {
            let size =
                usize::try_from(prefix.little_endian(4)?).map_err(|_| Error::ResourceLimit)?;
            buffer.consume(8)?;
            buffer.consume(size)?;
            continue;
        }
        let (header, header_size) = FrameHeader::parse(buffer.remaining())?;
        if header.dictionary_id != 0 {
            let selected = dictionary.ok_or(Error::DictionaryRequired)?;
            if selected.id() != header.dictionary_id {
                return Err(Error::DictionaryMismatch);
            }
        }
        if header.window_size > options.max_window_size {
            return Err(Error::ResourceLimit);
        }
        let (_, position, limit) = buffer.positions();
        if header
            .content_size
            .is_some_and(|size| size > (limit - position) as u64)
        {
            return Err(Error::ResourceLimit);
        }
        let window = usize::try_from(header.window_size).map_err(|_| Error::ResourceLimit)?;
        buffer.consume(header_size)?;
        buffer.start_frame(window);
        let max_block = window.min(MAX_BLOCK_SIZE);
        let mut huffman = None;
        let mut sequences = SequenceState::new_in(allocator.clone());
        let initial = dictionary.and_then(|view| view.entropy());
        if let Some(entropy) = initial {
            sequences.reset(entropy.repeat);
        }
        loop {
            let block = BlockHeader::read(&mut ByteCursor::new(buffer.remaining()), max_block)?;
            buffer.consume(3)?;
            match block.kind {
                BlockKind::Raw => buffer.copy_raw(block.size)?,
                BlockKind::Rle => buffer.repeat_rle(block.size)?,
                BlockKind::Compressed => {
                    let (_, position, _) = buffer.positions();
                    let (frame_start, window) = buffer.frame_coordinates();
                    let end = {
                        let (output, encoded) = buffer.compressed_parts(block.size)?;
                        let limit = output.len();
                        let literal_limit = max_block.min(limit - position);
                        let (literals, consumed) = decode_literals_with_dictionary_view_in(
                            encoded,
                            &mut huffman,
                            literal_limit,
                            initial.map(|entropy| entropy.huffman),
                            allocator.clone(),
                        )?;
                        if let Some(dictionary) = dictionary {
                            let mut history = DictionarySliceOutput::resume(
                                output,
                                position,
                                frame_start,
                                window,
                                limit,
                                dictionary,
                            )?;
                            sequences.decode_with_dictionary_view(
                                &encoded[consumed..],
                                &literals,
                                &mut history,
                                max_block,
                                initial,
                            )?;
                            history.len()
                        } else {
                            let mut history =
                                SliceHistory::resume(output, position, frame_start, window, limit)?;
                            sequences.decode_with_dictionary_view(
                                &encoded[consumed..],
                                &literals,
                                &mut history,
                                max_block,
                                None,
                            )?;
                            history.len()
                        }
                    };
                    buffer.commit_compressed(block.size, end)?;
                }
            }
            if block.last {
                break;
            }
        }
        let decoded = buffer.frame_bytes();
        if header
            .content_size
            .is_some_and(|size| size != decoded.len() as u64)
        {
            return Err(Error::ContentSizeMismatch);
        }
        if header.has_checksum {
            let expected = checksum(decoded) as u32;
            let actual = ByteCursor::new(buffer.remaining()).little_endian(4)? as u32;
            if actual != expected {
                return Err(Error::ChecksumMismatch);
            }
            buffer.consume(4)?;
        }
    }
    Ok(buffer.written().len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{alloc::Global, vec, vec::Vec};

    fn raw(bytes: &[u8], checksum_flag: bool) -> Vec<u8> {
        let mut encoded = vec![
            0x28,
            0xb5,
            0x2f,
            0xfd,
            0x20 | (u8::from(checksum_flag) * 4),
            bytes.len() as u8,
        ];
        encoded.extend_from_slice(&(((bytes.len() as u32) << 3) | 1).to_le_bytes()[..3]);
        encoded.extend_from_slice(bytes);
        if checksum_flag {
            encoded.extend_from_slice(&(checksum(bytes) as u32).to_le_bytes());
        }
        encoded
    }

    fn check(frame: &[u8], expected: &[u8], capacity: usize) {
        let start = capacity - frame.len();
        let mut bytes = vec![0xa5; capacity + 2];
        bytes[1 + start..1 + capacity].copy_from_slice(frame);
        let before = bytes.clone();
        assert_eq!(
            decode_in_place(
                &mut bytes[1..capacity + 1],
                start,
                DecodeOptions::new(expected.len()),
                Global
            ),
            Ok(expected.len())
        );
        assert_eq!(&bytes[1..1 + expected.len()], expected);
        assert_eq!(bytes[0], 0xa5);
        assert_eq!(&bytes[1 + expected.len()..], &before[1 + expected.len()..]);
    }

    #[test]
    fn actual_raw_overlap_and_empty_blocks_preserve_tail_and_outer_guards() {
        for length in [0, 1, 8, 9, 10, 31, 127, 255] {
            let expected: Vec<_> = (0..length).map(|i| (i * 7) as u8).collect();
            for flag in [false, true] {
                let frame = raw(&expected, flag);
                for gap in [0, 1, 9, 100] {
                    check(&frame, &expected, frame.len() + gap);
                }
            }
        }
    }

    #[test]
    fn compressed_sequence_reuses_disjoint_history_across_blocks() {
        let block = [24, b'a', b'b', b'c', 1, 0x54, 3, 2, 0, 6];
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0, 0];
        // Two compressed blocks share sequence repeat offsets and output history.
        for last in [false, true] {
            frame.extend_from_slice(
                &(((block.len() as u32) << 3) | 4 | u32::from(last)).to_le_bytes()[..3],
            );
            frame.extend_from_slice(&block);
        }
        let expected = crate::decompress(&frame, 12).unwrap();
        assert_eq!(expected, b"abcabcabcabc");
        for gap in [0, 1, 9] {
            check(&frame, &expected, frame.len() + gap);
        }
    }

    #[test]
    fn native_generated_entropy_fixtures_decode_at_sufficient_margin() {
        let cases: &[(&[u8], &[u8])] = &[
            (
                include_bytes!("../tests/upstream/data/generated/http.zst"),
                include_bytes!("../tests/upstream/data/golden-compression/http"),
            ),
            (
                include_bytes!("../tests/upstream/data/generated/huffman-compressed-larger.zst"),
                include_bytes!(
                    "../tests/upstream/data/golden-compression/huffman-compressed-larger"
                ),
            ),
        ];
        for &(frame, expected) in cases {
            let margin = crate::decompression_margin(frame).unwrap();
            for extra in [0, 17] {
                check(
                    frame,
                    expected,
                    (expected.len() + margin).max(frame.len()) + extra,
                );
            }
        }
    }

    #[test]
    fn concatenated_frames_skips_and_rle_keep_frame_boundaries() {
        let mut frame = Vec::new();
        let mut expected = Vec::new();
        for low in 0..16 {
            frame.extend_from_slice(&(0x184d_2a50_u32 + low).to_le_bytes());
            frame.extend_from_slice(&3_u32.to_le_bytes());
            frame.extend_from_slice(b"tag");
            frame.extend_from_slice(&raw(b"abc", true));
            expected.extend_from_slice(b"abc");
        }
        frame.extend_from_slice(&[0x28, 0xb5, 0x2f, 0xfd, 0x20, 5, 43, 0, 0, b'x']);
        expected.extend_from_slice(b"xxxxx");
        check(&frame, &expected, frame.len());
        let skipped = [0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0];
        check(&skipped, &[], skipped.len());
    }

    #[test]
    fn malformed_ranges_limits_and_checksums_preserve_protected_bytes() {
        let frame = raw(b"abcdefghi", true);
        for end in 0..frame.len() {
            let mut bytes = vec![0xa5; end + 12];
            bytes[12..].copy_from_slice(&frame[..end]);
            let before = bytes.clone();
            assert!(decode_in_place(&mut bytes, 12, DecodeOptions::new(9), Global).is_err());
            assert_eq!(&bytes[9..], &before[9..]);
        }
        let mut bytes = frame.clone();
        let length = bytes.len();
        assert_eq!(
            decode_in_place(&mut bytes, length + 1, DecodeOptions::new(9), Global),
            Err(Error::ResourceLimit)
        );
        assert_eq!(bytes, frame);
        assert_eq!(
            decode_in_place(&mut bytes, 0, DecodeOptions::new(8), Global),
            Err(Error::ResourceLimit)
        );
        assert_eq!(bytes, frame);
        *bytes.last_mut().unwrap() ^= 1;
        assert_eq!(
            decode_in_place(&mut bytes, 0, DecodeOptions::new(9), Global),
            Err(Error::ChecksumMismatch)
        );
    }

    #[test]
    fn insufficient_compressed_and_rle_fences_never_overwrite_encoded_payloads() {
        // Output cannot reach into the RLE byte even though it fits the buffer.
        let rle = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 10, 83, 0, 0, b'x'];
        let mut bytes = rle;
        assert_eq!(
            decode_in_place(&mut bytes, 0, DecodeOptions::new(10), Global),
            Err(Error::ResourceLimit)
        );
        assert_eq!(bytes, rle);
        // The same conservative fence applies to compressed output at byte 9.
        let block = [24, b'a', b'b', b'c', 1, 0x54, 3, 2, 6, 6];
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0, 0];
        frame.extend_from_slice(&(((block.len() as u32) << 3) | 5).to_le_bytes()[..3]);
        frame.extend_from_slice(&block);
        let mut bytes = frame.clone();
        assert_eq!(
            decode_in_place(&mut bytes, 0, DecodeOptions::new(19), Global),
            Err(Error::ResourceLimit)
        );
        assert_eq!(&bytes[9..], &frame[9..]);
    }
}

#[cfg(test)]
mod dictionary_tests {
    use super::*;
    use crate::Dictionary;
    use alloc::{alloc::Global, vec};
    macro_rules! fixture {
        ($name:literal) => {
            include_bytes!(concat!("../tests/upstream/dictionaries/", $name))
        };
    }

    #[test]
    fn persistent_borrowed_dictionary_decodes_fallback_crossings_and_repeated_frames() {
        let cases: &[(&[u8], &[u8], &[u8])] = &[
            (
                fixture!("boundary-dictionary.raw"),
                fixture!("p0-far-dictionary.zst"),
                fixture!("p0-far-dictionary.expected"),
            ),
            (
                fixture!("boundary-dictionary.raw"),
                fixture!("p-window-minus-one.zst"),
                fixture!("p-window-minus-one.expected"),
            ),
            (
                fixture!("boundary-dictionary.raw"),
                fixture!("p-window-equal.zst"),
                fixture!("p-window-equal.expected"),
            ),
            (
                fixture!("boundary-dictionary.raw"),
                fixture!("dictionary-to-frame-eviction.zst"),
                fixture!("dictionary-to-frame-eviction.expected"),
            ),
            (
                fixture!("boundary-dictionary.raw"),
                fixture!("output-distance-equals-window.zst"),
                fixture!("output-distance-equals-window.expected"),
            ),
            (
                fixture!("boundary-dictionary.raw"),
                fixture!("raw-small-window.zst"),
                fixture!("raw-small-window.expected"),
            ),
            (
                fixture!("formatted.dict"),
                fixture!("formatted-id.zst"),
                fixture!("formatted-id.expected"),
            ),
            (
                fixture!("formatted.dict"),
                fixture!("formatted-omitted-id.zst"),
                fixture!("formatted-omitted-id.expected"),
            ),
            (
                fixture!("formatted.dict"),
                fixture!("persistent-concatenation.zst"),
                fixture!("persistent-concatenation.expected"),
            ),
            (
                fixture!("formatted.dict"),
                fixture!("dictionary-treeless-literals.zst"),
                fixture!("dictionary-treeless-literals.expected"),
            ),
            (
                fixture!("formatted.dict"),
                fixture!("dictionary-treeless-four-streams.zst"),
                fixture!("dictionary-treeless-four-streams.expected"),
            ),
            (
                fixture!("trained.dict"),
                fixture!("dictionary-repeat-sequences.zst"),
                fixture!("dictionary-repeat-sequences.expected"),
            ),
            (
                fixture!("duplicate-reps.dict"),
                fixture!("duplicate-reps-initial.zst"),
                fixture!("duplicate-reps-initial.expected"),
            ),
        ];
        for &(dictionary, frame, expected) in cases {
            let dictionary = Dictionary::from_bytes(dictionary).unwrap();
            for copies in [1, 2] {
                let encoded = frame.repeat(copies);
                let expected = expected.repeat(copies);
                let capacity = (expected.len() + crate::decompression_margin(&encoded).unwrap())
                    .max(encoded.len());
                let start = capacity - encoded.len();
                let mut bytes = vec![0xa5; capacity + 16];
                bytes[8 + start..8 + capacity].copy_from_slice(&encoded);
                let before = bytes.clone();
                assert_eq!(
                    decode_in_place_with_dictionary_view(
                        &mut bytes[8..8 + capacity],
                        start,
                        DecodeOptions::new(expected.len()),
                        Global,
                        Some(dictionary.view())
                    ),
                    Ok(expected.len())
                );
                assert_eq!(&bytes[8..8 + expected.len()], expected);
                assert_eq!(&bytes[..8], &before[..8]);
                assert_eq!(&bytes[8 + expected.len()..], &before[8 + expected.len()..]);
            }
        }
    }

    #[test]
    fn invalid_dictionary_distances_and_ids_preserve_unread_bytes_and_allowance() {
        let dictionary = Dictionary::raw(fixture!("boundary-dictionary.raw")).unwrap();
        for frame in [
            fixture!("p-window-plus-one.zst").as_slice(),
            fixture!("dictionary-out-of-bounds.zst").as_slice(),
        ] {
            let capacity = 8192 + frame.len();
            let start = capacity - frame.len();
            let mut bytes = vec![0xa5; capacity + 16];
            bytes[8 + start..8 + capacity].copy_from_slice(frame);
            let before = bytes.clone();
            assert_eq!(
                decode_in_place_with_dictionary_view(
                    &mut bytes[8..8 + capacity],
                    start,
                    DecodeOptions::new(8192),
                    Global,
                    Some(dictionary.view())
                ),
                Err(Error::CorruptData)
            );
            assert_eq!(&bytes[..8], &before[..8]);
            assert_eq!(&bytes[8 + 8192..], &before[8 + 8192..]);
        }
        let frame = fixture!("formatted-id.zst");
        let mut bytes = frame.to_vec();
        assert_eq!(
            decode_in_place_with_dictionary_view(
                &mut bytes,
                0,
                DecodeOptions::new(0),
                Global,
                Some(dictionary.view())
            ),
            Err(Error::DictionaryMismatch)
        );
        assert_eq!(&bytes, frame);
    }
}
