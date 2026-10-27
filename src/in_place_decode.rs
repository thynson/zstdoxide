//! Suffix-input decoding within one exclusively borrowed allocation.

use crate::cursor::ByteCursor;
use crate::frame::{BlockHeader, BlockKind, MAX_BLOCK_SIZE};
use crate::history::SequenceOutput;
use crate::huffman::decode_literals_with_dictionary_view_in;
use crate::in_place_buffer::InPlaceBuffer;
use crate::sequences::SequenceState;
use crate::slice_history::SliceHistory;
use crate::xxhash::checksum;
use crate::{DecodeOptions, Error, FrameHeader, Result};
use core::alloc::Allocator;

/// The private coordinator corresponding to `ZSTD_decompressFrame`.
/// Metadata borrows end before any writes; compressed payload borrows are
/// disjoint from resumed history, and raw moves explicitly permit overlap.
pub(crate) fn decode_in_place<A: Allocator + Clone>(
    bytes: &mut [u8],
    input_start: usize,
    options: DecodeOptions,
    allocator: A,
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
            return Err(Error::DictionaryRequired);
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
                        let mut history =
                            SliceHistory::resume(output, position, frame_start, window, limit)?;
                        let literal_limit = max_block.min(limit - position);
                        let (literals, consumed) = decode_literals_with_dictionary_view_in(
                            encoded,
                            &mut huffman,
                            literal_limit,
                            None,
                            allocator.clone(),
                        )?;
                        sequences.decode_with_dictionary_view(
                            &encoded[consumed..],
                            &literals,
                            &mut history,
                            max_block,
                            None,
                        )?;
                        history.len()
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
