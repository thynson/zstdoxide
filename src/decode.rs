//! One-shot frame orchestration, resource limits, and checksum verification.

use crate::cursor::ByteCursor;
use crate::frame::{BlockHeader, BlockKind, MAX_BLOCK_SIZE};
use crate::history::History;
use crate::huffman::decode_literals_in;
use crate::sequences::SequenceState;
use crate::xxhash::checksum;
use crate::{Decoder, Error, FrameHeader, Result, SharedDictionary, StreamStatus};
use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

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
/// Concatenated frames are supported. Use [`decompress_with_dictionary`] for
/// frames requiring raw or prepared dictionaries. Legacy formats are unsupported.
/// Content checksums are always verified. Empty input is an incomplete stream.
/// This is a whole-buffer API, not a streaming API.
///
/// Corresponds to the one-shot behavior of upstream `ZSTD_decompress`.
pub fn decompress(input: &[u8], max_output_size: usize) -> Result<Vec<u8>> {
    decompress_with_options(input, DecodeOptions::new(max_output_size))
}

/// Decompress whole frames into the supplied allocator, including every
/// temporary table and literal buffer. The result retains the original
/// allocator value; fresh clones own scratch and never adopt its allocations.
/// A borrowed non-Clone allocator can be supplied as `&allocator`.
///
/// Ordinary allocator cloning may allocate or panic outside the codec's
/// fallible collection requests. Frame, checksum, dictionary, and resource
/// contracts are the same as [`decompress`].
pub fn decompress_in<A: Allocator + Clone>(
    input: &[u8],
    max_output_size: usize,
    allocator: A,
) -> Result<Vec<u8, A>> {
    decompress_with_options_in(input, DecodeOptions::new(max_output_size), allocator)
}

/// Decompress concatenated frames using one persistent prepared or raw dictionary.
///
/// Uses the incremental decoder's dictionary, checksum, and resource contracts.
/// The returned allocation contains only frame output; dictionary content never
/// counts toward its size or output limit. The supplied shared handle remains
/// usable after success or failure. Corresponds to `ZSTD_decompress_usingDDict`.
pub fn decompress_with_dictionary(
    input: &[u8],
    dictionary: &SharedDictionary,
    options: DecodeOptions,
) -> Result<Vec<u8>> {
    let mut decoder = Decoder::with_dictionary(options, dictionary.clone());
    let mut output = Vec::new();
    let mut consumed = 0;
    let mut scratch = [0_u8; 4096];
    loop {
        let progress = decoder.process(&input[consumed..], &mut scratch)?;
        consumed += progress.consumed;
        output
            .try_reserve(progress.produced)
            .map_err(|_| Error::AllocationFailed)?;
        output.extend_from_slice(&scratch[..progress.produced]);
        match progress.status {
            StreamStatus::FrameFinished | StreamStatus::NeedInput if consumed == input.len() => {
                decoder.finish()?;
                return Ok(output);
            }
            StreamStatus::Flushed => return Err(Error::InvalidState),
            _ => {}
        }
    }
}

/// Decompress frames with explicit output and advertised-window limits.
pub fn decompress_with_options(input: &[u8], options: DecodeOptions) -> Result<Vec<u8>> {
    decompress_with_options_in(input, options, Global)
}

/// Decompress with explicit resource limits and allocator ownership.
///
/// The complete output is retained during decoding. All collection requests
/// are fallible; an error drops the partial result and frame-local tables.
/// The selected allocator is retained by the returned vector, without copying
/// bytes or converting allocation ownership. See [`decompress_in`] for the
/// allocator-cloning boundary.
pub fn decompress_with_options_in<A: Allocator + Clone>(
    input: &[u8],
    options: DecodeOptions,
    allocator: A,
) -> Result<Vec<u8, A>> {
    if input.is_empty() {
        return Err(Error::UnexpectedEof);
    }
    let scratch_allocator = allocator.clone();
    let mut cursor = ByteCursor::new(input);
    let mut history = History::new_in(options.max_output_size, allocator);
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
        let mut sequences = SequenceState::new_in(scratch_allocator.clone());
        loop {
            let block = BlockHeader::read(&mut cursor, max_block)?;
            match block.kind {
                BlockKind::Raw => history.append(cursor.take(block.size)?)?,
                BlockKind::Rle => history.repeat(cursor.byte()?, block.size)?,
                BlockKind::Compressed => {
                    let encoded = cursor.take(block.size)?;
                    let literal_limit = max_block.min(options.max_output_size - history.len());
                    let (literals, consumed) = decode_literals_in(
                        encoded,
                        &mut huffman,
                        literal_limit,
                        scratch_allocator.clone(),
                    )?;
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
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    const HTTP_FRAME: &[u8] = include_bytes!("../tests/upstream/data/generated/http.zst");
    const HTTP_SOURCE: &[u8] = include_bytes!("../tests/upstream/data/golden-compression/http");
    const LARGER_FRAME: &[u8] =
        include_bytes!("../tests/upstream/data/generated/huffman-compressed-larger.zst");
    const LARGER_SOURCE: &[u8] =
        include_bytes!("../tests/upstream/data/golden-compression/huffman-compressed-larger");

    fn tracking() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig::default()).unwrap()
    }

    #[test]
    fn custom_complete_output_owns_the_requested_allocator_after_scratch_release() {
        // Independently generated pinned C frames; original fixture attribution
        // and source bytes remain under tests/upstream/data and its manifest.
        for (frame, source) in [(HTTP_FRAME, HTTP_SOURCE), (LARGER_FRAME, LARGER_SOURCE)] {
            let owner = tracking();
            let identity = owner.identity();
            let monitor = owner.monitor();
            let output = decompress_in(frame, source.len(), owner).unwrap();
            assert_eq!(output.as_slice(), source);
            assert_eq!(output.allocator().identity(), identity);
            assert_eq!(monitor.snapshot().live_blocks, 1);
            assert_eq!(monitor.snapshot().live_bytes, output.capacity() as u128);
            let pointer = output.as_ptr();
            let moved = output;
            assert_eq!(moved.as_ptr(), pointer);
            drop(moved);
            monitor.assert_released();
        }
    }

    #[test]
    fn borrowed_non_clone_allocator_handles_concatenation_and_skippable_frames() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let mut stream = HTTP_FRAME.to_vec();
        for low in 0..16 {
            stream.extend_from_slice(&(0x184d_2a50_u32 + low).to_le_bytes());
            stream.extend_from_slice(&3_u32.to_le_bytes());
            stream.extend_from_slice(b"tag");
        }
        stream.extend_from_slice(LARGER_FRAME);
        let size = HTTP_SOURCE.len() + LARGER_SOURCE.len();
        let output = decompress_in(&stream, size, &owner).unwrap();
        assert_eq!(&output[..HTTP_SOURCE.len()], HTTP_SOURCE);
        assert_eq!(&output[HTTP_SOURCE.len()..], LARGER_SOURCE);
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(output);
        monitor.assert_released();

        let skipped = [0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0];
        let before = monitor.snapshot().attempts;
        assert!(monitor.fail_next());
        let output = decompress_in(&skipped, 0, &owner).unwrap();
        assert!(output.is_empty());
        assert_eq!(monitor.snapshot().attempts, before);
        assert_eq!(output.allocator().identity(), owner.identity());
        drop(output);
        assert!(matches!(
            decompress_in(&raw(b"x", false), 1, &owner),
            Err(Error::AllocationFailed)
        ));
        monitor.assert_released();
    }

    #[test]
    fn each_complete_decoder_allocation_failure_releases_output_and_entropy() {
        let owner = tracking();
        let monitor = owner.monitor();
        let result = decompress_in(HTTP_FRAME, HTTP_SOURCE.len(), owner).unwrap();
        let requests = monitor.snapshot().attempts;
        assert!(requests > 5);
        drop(result);
        monitor.assert_released();
        for ordinal in 1..=requests {
            let owner = TrackingAllocator::new(AllocatorConfig {
                fail_at: NonZeroU64::new(ordinal),
                ..AllocatorConfig::default()
            })
            .unwrap();
            let monitor = owner.monitor();
            let result = decompress_in(HTTP_FRAME, HTTP_SOURCE.len(), owner);
            assert!(
                matches!(result, Err(Error::AllocationFailed)),
                "request {ordinal}"
            );
            assert_eq!(monitor.snapshot().attempts, ordinal);
            assert_eq!(monitor.snapshot().failures, 1);
            drop(result);
            monitor.assert_released();
        }
    }

    #[test]
    fn custom_decoder_preserves_limits_errors_and_releases_partial_results() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        for (frame, options, error) in [
            (&[][..], DecodeOptions::new(0), Error::UnexpectedEof),
            (
                &[0, 0, 0, 0][..],
                DecodeOptions::new(1),
                Error::InvalidMagic,
            ),
            (
                &raw(b"abc", true)[..],
                DecodeOptions::new(2),
                Error::ResourceLimit,
            ),
            (
                &raw(b"abc", true)[..],
                DecodeOptions {
                    max_output_size: 3,
                    max_window_size: 2,
                },
                Error::ResourceLimit,
            ),
        ] {
            let before = monitor.snapshot().attempts;
            assert_eq!(
                decompress_with_options_in(frame, options, &owner).err(),
                Some(error)
            );
            assert_eq!(monitor.snapshot().attempts, before);
            monitor.assert_released();
        }
        let mut bad_checksum = raw(b"abc", true);
        *bad_checksum.last_mut().unwrap() ^= 1;
        assert_eq!(
            decompress_in(&bad_checksum, 3, &owner).err(),
            Some(Error::ChecksumMismatch)
        );
        monitor.assert_released();
        for end in 0..HTTP_FRAME.len() {
            assert!(decompress_in(&HTTP_FRAME[..end], HTTP_SOURCE.len(), &owner).is_err());
            monitor.assert_released();
        }
        assert_eq!(
            decompress_in(HTTP_FRAME, HTTP_SOURCE.len(), &owner)
                .unwrap()
                .as_slice(),
            HTTP_SOURCE
        );
        monitor.assert_released();
    }

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
