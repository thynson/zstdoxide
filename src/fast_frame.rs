//! Allocation-free frame metadata for an admitted whole-buffer Fast profile.
//!
//! The named convenience encoder keeps its existing checksummed header policy.
//! These plain bytes and checked sizes share one safe representation.

use crate::fast_plan::FastExecutionPlan;
use crate::frame::MAGIC;
use crate::{Error, Result};

/// Initialized standard frame prefix with known size, no dictionary/checksum.
/// At most fourteen bytes: magic, descriptor, optional window and eight-byte size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "whole-buffer Fast emission follows")
)]
pub(crate) struct FastFrameHeader {
    bytes: [u8; 14],
    length: usize,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "whole-buffer Fast emission follows")
)]
impl FastFrameHeader {
    /// Build truthful window/content metadata from the admitted known-size plan.
    /// Corresponds to `ZSTD_writeFrameHeader` in pinned zstd_compress.c:4695–4748.
    /// The reference promotes 0xffffffff to an eight-byte field, although that
    /// value fits the format's four-byte representation used by the named API.
    pub(crate) fn new(plan: &FastExecutionPlan) -> Self {
        let size = plan.source_size();
        let single_segment = plan.source_len() <= plan.window_size();
        let (size_flag, size_value, size_bytes) = match size {
            0..=255 => (0, size, 1),
            256..=65_791 => (1, size - 256, 2),
            65_792..=0xffff_fffe => (2, size, 4),
            _ => (3, size, 8),
        };
        let mut bytes = [0; 14];
        bytes[..4].copy_from_slice(&MAGIC.to_le_bytes());
        bytes[4] = (size_flag << 6) | (u8::from(single_segment) << 5);
        let mut length = 5;
        if !single_segment {
            // Admitted logs are 10..=20, so the exponent fits the descriptor
            // and its zero mantissa denotes exactly the selected power of two.
            bytes[length] = (plan.parameters().window_log() - 10) << 3;
            length += 1;
        }
        // Size <256 is necessarily single-segment because the minimum window
        // is 1024. Thus its one-byte field is present; all other flags explicitly
        // carry the selected size. The maximum extent is 6+8=14 bytes.
        bytes[length..length + size_bytes].copy_from_slice(&size_value.to_le_bytes()[..size_bytes]);
        length += size_bytes;
        Self { bytes, length }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }

    /// Bound a complete frame by representing every input block as raw.
    /// A zero-size input still emits one final empty raw block. No checksum,
    /// dictionary field, allocator overhead or capacity over-allocation is added.
    /// This checks scalar arithmetic without creating an input or output buffer.
    pub(crate) fn raw_frame_bound(plan: &FastExecutionPlan) -> Result<usize> {
        let blocks = plan.source_len().div_ceil(plan.block_limit()).max(1);
        let headers = blocks.checked_mul(3).ok_or(Error::ResourceLimit)?;
        plan.source_len()
            .checked_add(Self::new(plan).length)
            .and_then(|size| size.checked_add(headers))
            .ok_or(Error::ResourceLimit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompressionParameterRequest, FrameHeader};

    fn plan(size: usize, window_log: u8) -> FastExecutionPlan {
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_window_log(window_log).unwrap();
        FastExecutionPlan::new(&request, size).unwrap()
    }

    #[test]
    fn known_size_boundaries_use_reference_field_widths_and_no_checksum() {
        // Source-derived wire expectations from the pinned writer, not a new
        // captured C oracle. No huge source allocation is made for these sizes.
        for (size, descriptor, encoded_size) in [
            (0, 0x20, &[0][..]),
            (1, 0x20, &[1][..]),
            (255, 0x20, &[255][..]),
            (256, 0x60, &[0, 0][..]),
            (65_791, 0x60, &[255, 255][..]),
            (65_792, 0xa0, &[0, 1, 1, 0][..]),
        ] {
            let selected = plan(size, 20);
            let header = FastFrameHeader::new(&selected);
            assert_eq!(&header.as_bytes()[..4], &[0x28, 0xb5, 0x2f, 0xfd]);
            assert_eq!(header.as_bytes()[4], descriptor);
            assert_eq!(&header.as_bytes()[5..], encoded_size);
            let (parsed, used) = FrameHeader::parse(header.as_bytes()).unwrap();
            assert_eq!(used, header.as_bytes().len());
            assert_eq!(parsed.content_size, Some(size as u64));
            assert_eq!(parsed.window_size, size as u64);
            assert_eq!(parsed.dictionary_id, 0);
            assert!(!parsed.has_checksum);
        }
    }

    #[test]
    fn source_above_selected_window_uses_an_explicit_honest_descriptor() {
        for log in 10..=20 {
            let window = 1_usize << log;
            let equal = FastFrameHeader::new(&plan(window, log));
            assert_ne!(equal.as_bytes()[4] & 0x20, 0);
            let selected = plan(window + 1, log);
            assert_eq!(selected.window_size(), window);
            let header = FastFrameHeader::new(&selected);
            assert_eq!(header.as_bytes()[4] & 0x20, 0);
            assert_eq!(header.as_bytes()[5], (log - 10) << 3);
            let (parsed, used) = FrameHeader::parse(header.as_bytes()).unwrap();
            assert_eq!(used, header.as_bytes().len());
            assert_eq!(parsed.content_size, Some((window + 1) as u64));
            assert_eq!(parsed.window_size, window as u64);
            assert_eq!(parsed.dictionary_id, 0);
            assert!(!parsed.has_checksum);
        }
    }

    #[test]
    fn maximum_four_byte_size_promotes_before_the_format_requires_it() {
        for (size, flag, width) in [
            (0xffff_fffe_u64, 2, 4),
            (0xffff_ffff, 3, 8),
            (0x1_0000_0000, 3, 8),
        ] {
            let Ok(source_len) = usize::try_from(size) else {
                continue;
            };
            let selected = plan(source_len, 20);
            let header = FastFrameHeader::new(&selected);
            assert_eq!(header.as_bytes()[4], flag << 6);
            assert_eq!(header.as_bytes()[5], 80);
            assert_eq!(header.as_bytes().len(), 6 + width);
            assert_eq!(&header.as_bytes()[6..], &size.to_le_bytes()[..width]);
            let (parsed, used) = FrameHeader::parse(header.as_bytes()).unwrap();
            assert_eq!(used, 6 + width);
            assert_eq!(parsed.content_size, Some(size));
            assert_eq!(parsed.window_size, 1 << 20);
        }
    }

    #[test]
    fn raw_frame_bound_counts_actual_window_blocks_and_the_empty_final_block() {
        for (size, window_log, expected) in [
            (0, 10, 9),
            (1, 10, 10),
            (1024, 10, 1034),
            (1025, 10, 1039),
            (2048, 10, 2062),
            (2049, 10, 2066),
            (131_072, 20, 131_084),
            (131_073, 20, 131_088),
        ] {
            assert_eq!(
                FastFrameHeader::raw_frame_bound(&plan(size, window_log)),
                Ok(expected)
            );
        }
        // The known-size sentinel is excluded by admission on 64-bit targets.
        // MAX-1 remains an inspectable scalar whose complete output overflows.
        let selected = plan(usize::MAX - 1, 10);
        assert_eq!(
            FastFrameHeader::raw_frame_bound(&selected),
            Err(Error::ResourceLimit)
        );
    }

    #[test]
    fn header_plus_real_raw_blocks_forms_a_complete_decodable_frame() {
        use alloc::vec::Vec;
        for (size, log) in [(0, 10), (1, 10), (1025, 10), (4097, 10), (131_073, 20)] {
            let source: Vec<_> = (0..size).map(|index| (index % 251) as u8).collect();
            let selected = plan(size, log);
            let header = FastFrameHeader::new(&selected);
            let mut frame = Vec::from(header.as_bytes());
            if source.is_empty() {
                frame.extend_from_slice(&[1, 0, 0]);
            } else {
                let blocks = source.chunks(selected.block_limit());
                let count = blocks.len();
                for (index, block) in blocks.enumerate() {
                    let value = ((block.len() as u32) << 3) | u32::from(index + 1 == count);
                    frame.extend_from_slice(&value.to_le_bytes()[..3]);
                    frame.extend_from_slice(block);
                }
            }
            assert_eq!(
                frame.len(),
                FastFrameHeader::raw_frame_bound(&selected).unwrap()
            );
            assert_eq!(crate::decompress(&frame, size).unwrap(), source);
        }
    }
}
