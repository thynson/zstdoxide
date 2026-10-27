//! Allocation-free framing traversal and a sufficient in-place output margin.
//!
//! Corresponds to v1.5.7 `ZSTD_findFrameSizeInfo` and
//! `ZSTD_decompressionMargin`. Framing traversal does not validate block content.

use crate::cursor::ByteCursor;
use crate::frame::{FrameHeader, MAX_BLOCK_SIZE};
use crate::{Error, Result};

/// All positions are relative to the beginning of the traversed frame.
/// This value owns no bytes and is shared by safe and unsafe storage backends.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameLayout {
    encoded_size: usize,
    overhead: usize,
    block_count: usize,
    header: Option<FrameHeader>,
    skippable_variant: u8,
}

impl FrameLayout {
    fn read(input: &[u8]) -> Result<Self> {
        let mut cursor = ByteCursor::new(input);
        let magic = cursor.little_endian(4)?;
        if magic & 0xffff_fff0 == 0x184d_2a50 {
            let size =
                usize::try_from(cursor.little_endian(4)?).map_err(|_| Error::ResourceLimit)?;
            cursor.take(size)?;
            return Ok(Self {
                encoded_size: cursor.position(),
                overhead: cursor.position(),
                block_count: 0,
                header: None,
                skippable_variant: (magic & 15) as u8,
            });
        }
        let (header, header_size) = FrameHeader::parse(input)?;
        cursor = ByteCursor::new(input);
        cursor.take(header_size)?;
        let mut block_count = 0_usize;
        loop {
            let block = cursor.little_endian(3)?;
            let payload_size = match (block >> 1) & 3 {
                0 | 2 => usize::try_from(block >> 3).map_err(|_| Error::ResourceLimit)?,
                1 => 1,
                _ => return Err(Error::CorruptData),
            };
            // This is only encoded-length traversal. Semantic block/window,
            // literal, sequence and content-size checks belong to decoding.
            cursor.take(payload_size)?;
            block_count = block_count.checked_add(1).ok_or(Error::ResourceLimit)?;
            if block & 1 != 0 {
                break;
            }
        }
        let checksum_size = if header.has_checksum { 4 } else { 0 };
        cursor.take(checksum_size)?;
        Ok(Self {
            encoded_size: cursor.position(),
            overhead: header_size
                .checked_add(checksum_size)
                .ok_or(Error::ResourceLimit)?,
            block_count,
            header: Some(header),
            skippable_variant: 0,
        })
    }

    fn maximum_block_size(self) -> usize {
        self.header.map_or(0, |header| {
            header.window_size.min(MAX_BLOCK_SIZE as u64) as usize
        })
    }

    fn add_overhead(self, previous: usize) -> Result<usize> {
        let blocks = self
            .block_count
            .checked_mul(3)
            .ok_or(Error::ResourceLimit)?;
        previous
            .checked_add(self.overhead)
            .and_then(|value| value.checked_add(blocks))
            .ok_or(Error::ResourceLimit)
    }
}

/// Copied structural metadata for a standard or skippable frame.
///
/// This plain value owns no bytes. A standard header describes advertised
/// properties, not validated decoded content. Skippable variants are 0 through 15.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameKind {
    /// An ordinary frame and its parsed header.
    Standard(FrameHeader),
    /// A skippable frame's magic variant and declared payload length.
    Skippable {
        /// Low four bits of the skippable magic number.
        variant: u8,
        /// Payload length, excluding the eight-byte header.
        payload_size: u32,
    },
}

/// Metadata from scanning one complete encoded frame without decompression.
///
/// Values are constructed by [`frame_info`]. Private fields preserve the checked
/// encoded extent and block count. Copies own no input or allocator and share the
/// same representation in every storage configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameInfo {
    kind: FrameKind,
    encoded_size: usize,
    block_count: usize,
}

impl FrameInfo {
    /// Return copied standard-header or skippable metadata.
    pub fn kind(self) -> FrameKind {
        self.kind
    }

    /// Complete encoded extent of the first frame, including any checksum.
    pub fn encoded_size(self) -> usize {
        self.encoded_size
    }

    /// Number of standard block headers; zero for a skippable frame.
    pub fn block_count(self) -> usize {
        self.block_count
    }

    /// Advertised decoded length; skippable frames contribute `Some(0)`.
    pub fn content_size(self) -> Option<u64> {
        match self.kind {
            FrameKind::Standard(header) => header.content_size,
            FrameKind::Skippable { .. } => Some(0),
        }
    }

    /// Structural upper bound for a semantically valid supported frame.
    ///
    /// Known advertised sizes are returned directly. Otherwise, multiply block
    /// count by the maximum permitted block size in u64 arithmetic, even on
    /// 32-bit hosts. This does not validate compressed contents or size truth.
    pub fn decompression_bound(self) -> Result<u64> {
        match self.kind {
            FrameKind::Standard(header) => match header.content_size {
                Some(size) => Ok(size),
                None => u64::try_from(self.block_count)
                    .map_err(|_| Error::ResourceLimit)?
                    .checked_mul(header.window_size.min(MAX_BLOCK_SIZE as u64))
                    .ok_or(Error::ResourceLimit),
            },
            FrameKind::Skippable { .. } => Ok(0),
        }
    }
}

/// Scan the first complete standard or skippable frame without allocating.
///
/// Trailing bytes are ignored. The returned encoded extent is nonzero and within
/// `input`. Block headers, payload ranges and checksum presence are checked;
/// literals, sequences, checksum values and advertised size truth are not.
/// Structural success does not establish decodability. Legacy and magicless
/// input are unsupported. Corresponds to upstream `ZSTD_findFrameSizeInfo`.
///
/// Header parsing retains this crate's structural window and u64 size contract,
/// including values native APIs reserve as sentinels. No pointer or input borrow
/// is retained in the returned metadata.
///
/// ```
/// use zstdoxide::{frame_info, frame_compressed_size, decompressed_size,
///                 decompression_bound, FrameKind};
/// let frame = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 25, 0, 0, b'a', b'b', b'c'];
/// let info = frame_info(&frame)?;
/// assert!(matches!(info.kind(), FrameKind::Standard(_)));
/// assert_eq!(info.encoded_size(), 12);
/// assert_eq!(info.block_count(), 1);
/// assert_eq!(info.content_size(), Some(3));
/// assert_eq!(frame_compressed_size(&frame)?, 12);
/// assert_eq!(decompressed_size(&frame)?, Some(3));
/// assert_eq!(decompression_bound(&frame)?, 3);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
pub fn frame_info(input: &[u8]) -> Result<FrameInfo> {
    let layout = FrameLayout::read(input)?;
    let kind = match layout.header {
        Some(header) => FrameKind::Standard(header),
        None => FrameKind::Skippable {
            variant: layout.skippable_variant,
            // A skipped layout was built from a checked u32 payload length.
            payload_size: u32::try_from(layout.encoded_size - 8)
                .map_err(|_| Error::ResourceLimit)?,
        },
    };
    Ok(FrameInfo {
        kind,
        encoded_size: layout.encoded_size,
        block_count: layout.block_count,
    })
}

/// Return the complete encoded extent of the first frame.
///
/// Uses [`frame_info`]'s structural contract, ignoring trailing input. Empty input
/// is incomplete. Corresponds to upstream `ZSTD_findFrameCompressedSize`.
pub fn frame_compressed_size(input: &[u8]) -> Result<usize> {
    Ok(FrameLayout::read(input)?.encoded_size)
}

/// Sum advertised sizes across an exact concatenation of complete frames.
///
/// Returns `Some(total)` when all standard sizes are known, or `None` if any is
/// unknown. Skippable frames contribute zero; empty input returns `Some(0)`.
/// The entire input is structurally scanned, even after an unknown size, so
/// trailing garbage or truncation is rejected. This is intentionally stricter
/// than the native `ZSTD_findDecompressedSize` early unknown-size return.
///
/// Known-size addition is checked even when another size is unknown; overflow is
/// `ResourceLimit`. All u64 header sizes remain ordinary values rather than
/// reserved error/unknown sentinels. No payloads or checksum values are validated.
pub fn decompressed_size(mut input: &[u8]) -> Result<Option<u64>> {
    let mut total = 0_u64;
    let mut known = true;
    while !input.is_empty() {
        let info = frame_info(input)?;
        if let Some(size) = info.content_size() {
            total = total.checked_add(size).ok_or(Error::ResourceLimit)?;
        } else {
            known = false;
        }
        input = &input[info.encoded_size..];
    }
    Ok(known.then_some(total))
}

/// Compute a checked decompressed-size bound for an exact frame concatenation.
///
/// Sums [`FrameInfo::decompression_bound`] with checked u64 arithmetic. Skippable
/// frames and empty input contribute zero. Overflow is `ResourceLimit`, including
/// native unsigned-wrap cases. The full input must be structurally complete.
/// This bound applies only to semantically valid supported frames; it neither
/// validates compressed payloads nor authorizes allocating an untrusted size.
/// Corresponds to upstream `ZSTD_decompressBound`, without numeric sentinels.
pub fn decompression_bound(mut input: &[u8]) -> Result<u64> {
    let mut total = 0_u64;
    while !input.is_empty() {
        let info = frame_info(input)?;
        total = total
            .checked_add(info.decompression_bound()?)
            .ok_or(Error::ResourceLimit)?;
        input = &input[info.encoded_size..];
    }
    Ok(total)
}

/// Recognize a four-byte skippable marker without validating its header/payload.
///
/// Returns false for fewer than four bytes. A true result does not guarantee
/// [`skippable_payload`] succeeds. Corresponds to upstream `ZSTD_isSkippableFrame`.
pub fn is_skippable_frame(input: &[u8]) -> bool {
    ByteCursor::new(input)
        .little_endian(4)
        .is_ok_and(|magic| magic & 0xffff_fff0 == 0x184d_2a50)
}

/// Borrow the first skippable payload and return its 0..15 magic variant.
///
/// Requires the complete eight-byte header and payload. Trailing input is ignored.
/// A non-skippable magic returns `InvalidMagic`; truncation is `UnexpectedEof`.
/// No payload bytes are copied or allocated, and the borrow cannot outlive input.
/// Corresponds to the payload/variant result of upstream `ZSTD_readSkippableFrame`.
///
/// ```
/// use zstdoxide::{is_skippable_frame, skippable_payload};
/// let frame = [0x5f, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, b'a', b'b', b'c'];
/// assert!(is_skippable_frame(&frame[..4]));
/// assert!(skippable_payload(&frame[..4]).is_err());
/// assert_eq!(skippable_payload(&frame)?, (15, b"abc".as_slice()));
/// # Ok::<(), zstdoxide::Error>(())
/// ```
///
/// ```compile_fail,E0597
/// use zstdoxide::skippable_payload;
/// let payload = {
///     let input = [0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0];
///     skippable_payload(&input).unwrap().1
/// };
/// assert!(payload.is_empty());
/// ```
pub fn skippable_payload(input: &[u8]) -> Result<(u8, &[u8])> {
    let mut cursor = ByteCursor::new(input);
    let magic = cursor.little_endian(4)?;
    if magic & 0xffff_fff0 != 0x184d_2a50 {
        return Err(Error::InvalidMagic);
    }
    let size = usize::try_from(cursor.little_endian(4)?).map_err(|_| Error::ResourceLimit)?;
    let payload = cursor.take(size)?;
    Ok(((magic & 15) as u8, payload))
}

/// Compute a sufficient extra capacity for suffix-input in-place decompression.
///
/// For supported valid frames, place the encoded input at the end of a buffer
/// whose length is at least the decoded length plus this margin. The encoded
/// input must also fit inside that buffer. The result supports concatenated
/// standard and skippable frames; empty input returns zero.
///
/// This allocation-free function checks framing and encoded ranges, not block
/// contents, content-size truth or checksums. Success does not establish that
/// the input can be decoded. Margin is sufficient, not a minimum; some smaller
/// placements may work. Overflow is `ResourceLimit`. Legacy and magicless frames
/// are not supported. Corresponds to upstream `ZSTD_decompressionMargin`.
///
/// Header parsing follows this crate's structural `FrameHeader` contract, not
/// the native implementation's platform window-log cap or its reserved numeric
/// content-size sentinels. Those extreme headers can therefore produce a margin
/// here while native scanning rejects them. Actual decoding must still enforce
/// output/window limits and validate the frame contents.
///
/// ```
/// use zstdoxide::decompression_margin;
/// // One empty standard frame: six header bytes and one three-byte block header.
/// let frame = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0];
/// assert_eq!(decompression_margin(&frame)?, 9);
/// assert_eq!(decompression_margin(&[])?, 0);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
pub fn decompression_margin(mut input: &[u8]) -> Result<usize> {
    let mut overhead = 0_usize;
    let mut maximum_block = 0;
    while !input.is_empty() {
        let frame = FrameLayout::read(input)?;
        overhead = frame.add_overhead(overhead)?;
        maximum_block = maximum_block.max(frame.maximum_block_size());
        // read() consumed at least a complete frame header and proved its end
        // is within this input. Progress is therefore nonzero and bounded.
        input = &input[frame.encoded_size..];
    }
    overhead
        .checked_add(maximum_block)
        .ok_or(Error::ResourceLimit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    fn frame(descriptor: u8, header_tail: &[u8], blocks: &[(u8, usize, &[u8])]) -> Vec<u8> {
        let mut bytes = vec![0x28, 0xb5, 0x2f, 0xfd, descriptor];
        bytes.extend_from_slice(header_tail);
        for (index, &(kind, size, payload)) in blocks.iter().enumerate() {
            let last = usize::from(index + 1 == blocks.len());
            let word = ((size << 3) | (usize::from(kind) << 1) | last) as u32;
            bytes.extend_from_slice(&word.to_le_bytes()[..3]);
            bytes.extend_from_slice(payload);
        }
        if descriptor & 4 != 0 {
            bytes.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        }
        bytes
    }

    #[test]
    fn raw_rle_compressed_and_checksum_overheads_follow_encoded_framing() {
        for (kind, payload) in [(0, &b"test"[..]), (1, &b"x"[..]), (2, &b"bad!"[..])] {
            for checksum in [false, true] {
                let bytes = frame(
                    0x20 | if checksum { 4 } else { 0 },
                    &[4],
                    &[(kind, 4, payload)],
                );
                assert_eq!(
                    decompression_margin(&bytes),
                    Ok(13 + usize::from(checksum) * 4)
                );
                let layout = FrameLayout::read(&bytes).unwrap();
                assert_eq!(layout.encoded_size, bytes.len());
                assert_eq!(layout.block_count, 1);
                assert_eq!(layout.maximum_block_size(), 4);
            }
        }
    }

    #[test]
    fn empty_unknown_size_and_invalid_payloads_are_not_decode_claims() {
        assert_eq!(decompression_margin(&[]), Ok(0));
        for (kind, payload) in [(0, &b""[..]), (1, &b"x"[..]), (2, &b""[..])] {
            assert_eq!(
                decompression_margin(&frame(0x20, &[0], &[(kind, 0, payload)])),
                Ok(9)
            );
        }
        assert_eq!(
            decompression_margin(&frame(0, &[0], &[(2, 0, b"")])),
            Ok(1033)
        );
        // Encoded range traversal admits a raw body larger than a zero window;
        // the actual decoder rejects that semantic violation.
        assert_eq!(
            decompression_margin(&frame(0x20, &[0], &[(0, 1, b"x")])),
            Ok(9)
        );
    }

    #[test]
    fn concatenation_sums_overhead_but_uses_one_largest_block() {
        let first = frame(0x20, &[3], &[(0, 3, b"abc")]);
        let second = frame(0, &[0], &[(0, 1, b"x"), (1, 5, b"y")]);
        let mut bytes = first;
        bytes.extend_from_slice(&second);
        assert_eq!(decompression_margin(&bytes), Ok(6 + 3 + 6 + 6 + 1024));
        let huge_window = frame(0, &[255], &[(0, 0, b"")]);
        assert_eq!(decompression_margin(&huge_window), Ok(6 + 3 + 128 * 1024));
    }

    #[test]
    fn every_skippable_magic_counts_its_complete_frame() {
        for magic in 0x184d_2a50_u32..=0x184d_2a5f {
            for length in [0_u32, 1, 127] {
                let mut skip = magic.to_le_bytes().to_vec();
                skip.extend_from_slice(&length.to_le_bytes());
                skip.resize(8 + length as usize, 0xa5);
                assert_eq!(decompression_margin(&skip), Ok(skip.len()));
                let standard = frame(0x20, &[0], &[(0, 0, b"")]);
                let mut both = skip.clone();
                both.extend_from_slice(&standard);
                both.extend_from_slice(&skip);
                assert_eq!(decompression_margin(&both), Ok(skip.len() * 2 + 9));
                for end in 1..skip.len() {
                    assert!(decompression_margin(&skip[..end]).is_err());
                }
            }
        }
    }

    #[test]
    fn all_header_field_widths_use_actual_consumed_bytes() {
        for single in [false, true] {
            for size_flag in 0_u8..4 {
                for dictionary_flag in 0_u8..4 {
                    let descriptor =
                        (size_flag << 6) | if single { 0x20 } else { 0 } | dictionary_flag;
                    let mut tail = Vec::new();
                    if !single {
                        tail.push(0);
                    }
                    tail.resize(tail.len() + [0, 1, 2, 4][usize::from(dictionary_flag)], 0);
                    let size_bytes = [usize::from(single), 2, 4, 8][usize::from(size_flag)];
                    tail.resize(tail.len() + size_bytes, 0);
                    let bytes = frame(descriptor, &tail, &[(0, 0, b"")]);
                    let maximum = if single {
                        if size_flag == 1 { 256 } else { 0 }
                    } else {
                        1024
                    };
                    assert_eq!(
                        decompression_margin(&bytes),
                        Ok(5 + tail.len() + 3 + maximum)
                    );
                    for end in 1..bytes.len() {
                        assert!(decompression_margin(&bytes[..end]).is_err());
                    }
                }
            }
        }
    }

    #[test]
    fn malformed_magic_reserved_fields_and_lengths_reject() {
        let mut bytes = frame(0x20, &[0], &[(0, 0, b"")]);
        bytes[0] = 0;
        assert_eq!(decompression_margin(&bytes), Err(Error::InvalidMagic));
        bytes[0] = 0x28;
        bytes[4] |= 8;
        assert_eq!(decompression_margin(&bytes), Err(Error::CorruptData));
        assert_eq!(
            decompression_margin(&frame(0x20, &[0], &[(3, 0, b"")])),
            Err(Error::CorruptData)
        );
        assert_eq!(
            decompression_margin(&frame(0x20, &[0], &[(0, 0x1f_ffff, b"")])),
            Err(Error::UnexpectedEof)
        );
        assert_eq!(
            decompression_margin(&[0x50, 0x2a, 0x4d, 0x18, 255, 255, 255, 255]),
            Err(Error::UnexpectedEof)
        );
        let checksum = frame(0x24, &[1], &[(0, 1, b"x")]);
        for end in 1..checksum.len() {
            assert!(decompression_margin(&checksum[..end]).is_err());
        }
    }

    #[test]
    fn full_width_content_sizes_remain_structural_metadata() {
        for size in [
            0_u64,
            1,
            u32::MAX as u64,
            1_u64 << 63,
            u64::MAX - 1,
            u64::MAX,
        ] {
            for single in [false, true] {
                let mut tail = Vec::new();
                if !single {
                    tail.push(0);
                }
                tail.extend_from_slice(&size.to_le_bytes());
                let bytes = frame(0xc0 | if single { 0x20 } else { 0 }, &tail, &[(0, 0, b"")]);
                let maximum = if single {
                    size.min(MAX_BLOCK_SIZE as u64) as usize
                } else {
                    1024
                };
                assert_eq!(
                    decompression_margin(&bytes),
                    Ok(5 + tail.len() + 3 + maximum)
                );
            }
        }
    }

    #[test]
    fn arithmetic_overflow_is_checked_without_large_allocations() {
        let normal = FrameLayout {
            encoded_size: 9,
            overhead: 6,
            block_count: 1,
            header: None,
            skippable_variant: 0,
        };
        assert_eq!(normal.add_overhead(usize::MAX - 9), Ok(usize::MAX));
        assert_eq!(
            normal.add_overhead(usize::MAX - 8),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            FrameLayout {
                block_count: usize::MAX,
                ..normal
            }
            .add_overhead(0),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            FrameLayout {
                overhead: usize::MAX,
                ..normal
            }
            .add_overhead(0),
            Err(Error::ResourceLimit)
        );
    }

    #[test]
    fn complete_layout_retains_standard_header_and_distinguishes_skips() {
        for descriptor in [0x20, 0x24] {
            let bytes = frame(descriptor, &[3], &[(0, 3, b"abc")]);
            let (header, _) = FrameHeader::parse(&bytes).unwrap();
            let layout = FrameLayout::read(&bytes).unwrap();
            assert_eq!(layout.header, Some(header));
            assert_eq!(layout.maximum_block_size(), 3);
            assert_eq!(layout.encoded_size, bytes.len());
        }
        for variant in 0..16 {
            let bytes = [0x50 + variant, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 7, 8, 9];
            let layout = FrameLayout::read(&bytes).unwrap();
            assert_eq!(layout.header, None);
            assert_eq!(layout.block_count, 0);
            assert_eq!(layout.maximum_block_size(), 0);
            assert_eq!(layout.encoded_size, bytes.len());
        }
    }
}
