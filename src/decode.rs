//! One-shot frame orchestration, resource limits, and checksum verification.

use crate::cursor::ByteCursor;
use crate::frame::{BlockHeader, BlockKind, MAX_BLOCK_SIZE};
use crate::history::History;
use crate::huffman::decode_literals;
use crate::sequences::SequenceState;
use crate::xxhash::checksum;
use crate::{Error, FrameHeader, Result};
use alloc::vec::Vec;

/// Resource limits shared by one-shot and incremental decompression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeOptions {
    /// Maximum combined output of all concatenated frames.
    pub max_output_size: usize,
    /// Maximum window advertised by any frame. One-shot decoding retains whole
    /// output. Streaming retains at most the validated window, bounded further
    /// by the remaining output allowance.
    pub max_window_size: u64,
}

impl DecodeOptions {
    /// Set the output limit and use a 128 MiB advertised-window limit.
    pub const fn new(max_output_size: usize) -> Self {
        Self {
            max_output_size,
            max_window_size: 128 * 1024 * 1024,
        }
    }
}

/// Decompress standard and skippable Zstandard frames with an explicit limit.
///
/// Concatenated frames are supported. Nonzero dictionary IDs are rejected;
/// prepared/raw dictionaries and legacy formats are not implemented. Optional
/// content checksums are always verified. Empty input is an incomplete stream.
/// This is a whole-buffer API, not a streaming API.
///
/// Corresponds to the one-shot behavior of upstream `ZSTD_decompress`.
pub fn decompress(input: &[u8], max_output_size: usize) -> Result<Vec<u8>> {
    decompress_with_options(input, DecodeOptions::new(max_output_size))
}

/// Decompress frames with explicit output and advertised-window limits.
pub fn decompress_with_options(input: &[u8], options: DecodeOptions) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Err(Error::UnexpectedEof);
    }
    let mut cursor = ByteCursor::new(input);
    let mut history = History::new(options.max_output_size);
    while !cursor.remaining().is_empty() {
        let mut prefix = ByteCursor::new(cursor.remaining());
        let magic = prefix.little_endian(4)? as u32;
        if magic & 0xffff_fff0 == 0x184d_2a50 {
            let size = prefix.little_endian(4)?;
            cursor.take(8)?;
            cursor.take(usize::try_from(size).map_err(|_| Error::ResourceLimit)?)?;
            continue;
        }
        let (header, header_size) = FrameHeader::parse(cursor.remaining())?;
        cursor.take(header_size)?;
        if header.dictionary_id != 0 {
            return Err(Error::DictionaryRequired);
        }
        if header.window_size > options.max_window_size {
            return Err(Error::ResourceLimit);
        }
        let remaining_limit = options.max_output_size - history.len();
        if header
            .content_size
            .is_some_and(|size| size > remaining_limit as u64)
        {
            return Err(Error::ResourceLimit);
        }
        let window = usize::try_from(header.window_size).map_err(|_| Error::ResourceLimit)?;
        history.start_frame(window);
        let max_block = window.min(MAX_BLOCK_SIZE);
        let mut huffman = None;
        let mut sequences = SequenceState::new();
        loop {
            let block = BlockHeader::read(&mut cursor, max_block)?;
            match block.kind {
                BlockKind::Raw => history.append(cursor.take(block.size)?)?,
                BlockKind::Rle => history.repeat(cursor.byte()?, block.size)?,
                BlockKind::Compressed => {
                    let encoded = cursor.take(block.size)?;
                    let literal_limit = max_block.min(options.max_output_size - history.len());
                    let (literals, consumed) =
                        decode_literals(encoded, &mut huffman, literal_limit)?;
                    sequences.decode(&encoded[consumed..], &literals, &mut history, max_block)?;
                }
            }
            if block.last {
                break;
            }
        }
        let decoded = history.frame_bytes();
        if header
            .content_size
            .is_some_and(|size| size != decoded.len() as u64)
        {
            return Err(Error::ContentSizeMismatch);
        }
        if header.has_checksum && cursor.little_endian(4)? as u32 != checksum(decoded) as u32 {
            return Err(Error::ChecksumMismatch);
        }
    }
    Ok(history.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn raw(input: &[u8], checksum_flag: bool) -> Vec<u8> {
        let mut encoded = vec![
            0x28,
            0xb5,
            0x2f,
            0xfd,
            0x20 | (u8::from(checksum_flag) * 4),
            input.len() as u8,
        ];
        let block_header = ((input.len() as u32) << 3) | 1;
        encoded.extend_from_slice(&block_header.to_le_bytes()[..3]);
        encoded.extend_from_slice(input);
        if checksum_flag {
            encoded.extend_from_slice(&(checksum(input) as u32).to_le_bytes());
        }
        encoded
    }

    #[test]
    fn raw_rle_and_checksum_frames() {
        assert_eq!(decompress(&raw(b"abc", true), 3).unwrap(), b"abc");
        let rle = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 5, 43, 0, 0, b'x'];
        assert_eq!(decompress(&rle, 5).unwrap(), b"xxxxx");
        assert_eq!(decompress(&raw(b"", false), 0).unwrap(), b"");
        let mut bad_checksum = raw(b"abc", true);
        *bad_checksum.last_mut().unwrap() ^= 1;
        assert_eq!(decompress(&bad_checksum, 3), Err(Error::ChecksumMismatch));
    }

    #[test]
    fn concatenation_and_all_skippable_magic_values() {
        let mut stream = raw(b"first", false);
        for low in 0..16 {
            stream.extend_from_slice(&(0x184d_2a50_u32 + low).to_le_bytes());
            stream.extend_from_slice(&3_u32.to_le_bytes());
            stream.extend_from_slice(b"tag");
        }
        stream.extend_from_slice(&raw(b"second", true));
        assert_eq!(decompress(&stream, 11).unwrap(), b"firstsecond");
        assert_eq!(decompress(&stream, 10), Err(Error::ResourceLimit));
    }

    #[test]
    fn rejects_truncation_invalid_sizes_and_limits() {
        let frame = raw(b"abc", true);
        for end in 0..frame.len() {
            assert!(decompress(&frame[..end], 10).is_err());
        }
        assert_eq!(decompress(&frame, 2), Err(Error::ResourceLimit));
        assert_eq!(
            decompress_with_options(
                &frame,
                DecodeOptions {
                    max_output_size: 3,
                    max_window_size: 2
                }
            ),
            Err(Error::ResourceLimit)
        );
        let mut wrong_size = raw(b"abc", false);
        wrong_size[5] = 4;
        assert_eq!(decompress(&wrong_size, 10), Err(Error::ContentSizeMismatch));
        let mut reserved = raw(b"abc", false);
        reserved[6] |= 6;
        assert_eq!(decompress(&reserved, 10), Err(Error::CorruptData));
        let dictionary = [0x28, 0xb5, 0x2f, 0xfd, 0x21, 1, 0];
        assert_eq!(decompress(&dictionary, 10), Err(Error::DictionaryRequired));
    }

    #[test]
    fn compressed_frame_executes_sequence() {
        // One raw-literals section "abc", then one RLE-coded sequence.
        let block = [24, b'a', b'b', b'c', 1, 0x54, 3, 2, 0, 6];
        // A non-single-segment frame keeps window >= compressed block size.
        let mut encoded = vec![0x28, 0xb5, 0x2f, 0xfd, 0, 0];
        let header = ((block.len() as u32) << 3) | 5;
        encoded.extend_from_slice(&header.to_le_bytes()[..3]);
        encoded.extend_from_slice(&block);
        assert_eq!(decompress(&encoded, 6).unwrap(), b"abcabc");
    }
}
