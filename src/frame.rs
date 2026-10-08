//! Zstandard frame and block metadata.

use crate::{Error, Result, cursor::ByteCursor};

pub(crate) const MAGIC: u32 = 0xfd2f_b528;
pub(crate) const MAX_BLOCK_SIZE: usize = 128 * 1024;

/// Validated metadata from the header of a standard Zstandard frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    /// Maximum allowed backward match distance, in bytes.
    pub window_size: u64,
    /// Decoded frame length, when the encoder supplied it.
    pub content_size: Option<u64>,
    /// Dictionary identifier, or zero when unspecified.
    pub dictionary_id: u32,
    /// Whether a four-byte XXH64 content checksum follows the final block.
    pub has_checksum: bool,
}

impl FrameHeader {
    /// Parse a frame header, returning the header and bytes consumed.
    ///
    /// Corresponds to `ZSTD_getFrameHeader_advanced`. Skippable frames are
    /// handled by the frame decoder, rather than represented as codec metadata.
    pub fn parse(input: &[u8]) -> Result<(Self, usize)> {
        let mut cursor = ByteCursor::new(input);
        if cursor.little_endian(4)? != u64::from(MAGIC) {
            return Err(Error::InvalidMagic);
        }
        let descriptor = cursor.byte()?;
        if descriptor & 8 != 0 {
            return Err(Error::CorruptData);
        }
        let single_segment = descriptor & 32 != 0;
        let mut window_size = 0;
        if !single_segment {
            let descriptor = cursor.byte()?;
            let base = 1_u64 << (10 + (descriptor >> 3));
            window_size = base + (base / 8) * u64::from(descriptor & 7);
        }
        let dictionary_bytes = [0, 1, 2, 4][usize::from(descriptor & 3)];
        let dictionary_id = cursor.little_endian(dictionary_bytes)? as u32;
        let size_bytes = match descriptor >> 6 {
            0 => usize::from(single_segment),
            1 => 2,
            2 => 4,
            _ => 8,
        };
        let content_size = if size_bytes == 0 {
            None
        } else {
            let value = cursor.little_endian(size_bytes)?;
            Some(if size_bytes == 2 { value + 256 } else { value })
        };
        if single_segment {
            window_size = content_size.ok_or(Error::CorruptData)?;
        }
        Ok((
            Self {
                window_size,
                content_size,
                dictionary_id,
                has_checksum: descriptor & 4 != 0,
            },
            cursor.position(),
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BlockKind {
    Raw,
    Rle,
    Compressed,
}

pub(crate) struct BlockHeader {
    pub(crate) last: bool,
    pub(crate) kind: BlockKind,
    pub(crate) size: usize,
}

impl BlockHeader {
    pub(crate) fn read(cursor: &mut ByteCursor<'_>, maximum: usize) -> Result<Self> {
        let value = cursor.little_endian(3)? as u32;
        let kind = match (value >> 1) & 3 {
            0 => BlockKind::Raw,
            1 => BlockKind::Rle,
            2 => BlockKind::Compressed,
            _ => return Err(Error::CorruptData),
        };
        let size = (value >> 3) as usize;
        if size > maximum {
            return Err(Error::CorruptData);
        }
        Ok(Self {
            last: value & 1 != 0,
            kind,
            size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_segment_and_two_byte_size_bias() {
        let (header, size) = FrameHeader::parse(&[0x28, 0xb5, 0x2f, 0xfd, 0x64, 0, 0]).unwrap();
        assert_eq!(size, 7);
        assert_eq!(header.content_size, Some(256));
        assert_eq!(header.window_size, 256);
        assert!(header.has_checksum);
    }

    #[test]
    fn unknown_size_and_largest_window_descriptor() {
        let (header, _) = FrameHeader::parse(&[0x28, 0xb5, 0x2f, 0xfd, 0, 255]).unwrap();
        assert_eq!(header.content_size, None);
        assert_eq!(header.window_size, (1_u64 << 41) + 7 * (1_u64 << 38));
    }

    #[test]
    fn reserved_bit_rejected_but_unused_bit_ignored() {
        let mut frame = [0x28, 0xb5, 0x2f, 0xfd, 0x38, 0];
        assert_eq!(FrameHeader::parse(&frame), Err(Error::CorruptData));
        frame[4] = 0x30;
        assert_eq!(FrameHeader::parse(&frame).unwrap().0.content_size, Some(0));
        for end in 0..frame.len() {
            assert!(FrameHeader::parse(&frame[..end]).is_err());
        }
    }
}
