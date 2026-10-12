//! Allocation-free plain framing shared by admitted reference search profiles.
//!
//! The named convenience encoder keeps its existing checksummed header policy.
//! These plain bytes and checked sizes share one safe representation.

#![forbid(unsafe_code)]

use crate::frame::{MAGIC, MAX_BLOCK_SIZE};
use crate::{Error, Result};

/// Algorithm-independent, checked known-source metadata. This plain record owns
/// no storage and does not select or broaden any search algorithm's admission.
/// Corresponds to the no-dictionary/no-checksum fields consumed by the pinned
/// `ZSTD_writeFrameHeader` and ordinary maximum-block calculations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceFrameSettings {
    source_len: usize,
    source_size: u64,
    window_log: u8,
    window_size: usize,
    block_limit: usize,
}

impl ReferenceFrameSettings {
    /// Check scalar metadata before output bounds and any allocation. The
    /// caller must separately admit the selected algorithm; window21 here does
    /// not extend Fast's window20 limit or admit an unsupported strategy.
    pub(crate) fn new(source_len: usize, window_log: u8) -> Result<Self> {
        let source_size = u64::try_from(source_len).map_err(|_| Error::ResourceLimit)?;
        if source_size == u64::MAX || !(10..=21).contains(&window_log) {
            return Err(Error::InvalidParameter);
        }
        let window_size = 1usize
            .checked_shl(u32::from(window_log))
            .ok_or(Error::ResourceLimit)?;
        Ok(Self {
            source_len,
            source_size,
            window_log,
            window_size,
            block_limit: window_size.min(MAX_BLOCK_SIZE),
        })
    }
    pub(crate) const fn source_len(&self) -> usize {
        self.source_len
    }
    pub(crate) const fn source_size(&self) -> u64 {
        self.source_size
    }
    pub(crate) const fn window_log(&self) -> u8 {
        self.window_log
    }
    pub(crate) const fn window_size(&self) -> usize {
        self.window_size
    }
    pub(crate) const fn block_limit(&self) -> usize {
        self.block_limit
    }
}

/// Initialized standard frame prefix with known size, no dictionary/checksum.
/// At most fourteen bytes: magic, descriptor, optional window and eight-byte size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReferenceFrameHeader {
    bytes: [u8; 14],
    length: usize,
}

impl ReferenceFrameHeader {
    /// Build truthful window/content metadata from checked plain settings.
    /// Corresponds to `ZSTD_writeFrameHeader` in pinned zstd_compress.c:4695–4748.
    /// The reference promotes 0xffffffff to an eight-byte field, although that
    /// value fits the format's four-byte representation used by the named API.
    pub(crate) fn new(settings: &ReferenceFrameSettings) -> Self {
        let size = settings.source_size();
        let single_segment = settings.source_len() <= settings.window_size();
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
            // Checked logs are 10..=21, so the exponent fits the descriptor
            // and its zero mantissa denotes exactly the selected power of two.
            bytes[length] = (settings.window_log() - 10) << 3;
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
    pub(crate) fn raw_frame_bound(settings: &ReferenceFrameSettings) -> Result<usize> {
        let blocks = settings
            .source_len()
            .div_ceil(settings.block_limit())
            .max(1);
        let headers = blocks.checked_mul(3).ok_or(Error::ResourceLimit)?;
        settings
            .source_len()
            .checked_add(Self::new(settings).length)
            .and_then(|size| size.checked_add(headers))
            .ok_or(Error::ResourceLimit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fast_plan::FastExecutionPlan;
    use crate::{CompressionParameterRequest, FrameHeader};

    fn plan(size: usize, window_log: u8) -> ReferenceFrameSettings {
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_window_log(window_log).unwrap();
        let admitted = FastExecutionPlan::new(&request, size).unwrap();
        let settings =
            ReferenceFrameSettings::new(admitted.source_len(), admitted.parameters().window_log())
                .unwrap();
        assert_eq!(settings.block_limit(), admitted.block_limit());
        assert_eq!(settings.source_size(), admitted.source_size());
        settings
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
            let header = ReferenceFrameHeader::new(&selected);
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
            let equal = ReferenceFrameHeader::new(&plan(window, log));
            assert_ne!(equal.as_bytes()[4] & 0x20, 0);
            let selected = plan(window + 1, log);
            assert_eq!(selected.window_size(), window);
            let header = ReferenceFrameHeader::new(&selected);
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
            let header = ReferenceFrameHeader::new(&selected);
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
                ReferenceFrameHeader::raw_frame_bound(&plan(size, window_log)),
                Ok(expected)
            );
        }
        // The known-size sentinel is excluded by admission on 64-bit targets.
        // MAX-1 remains an inspectable scalar whose complete output overflows.
        let selected = plan(usize::MAX - 1, 10);
        assert_eq!(
            ReferenceFrameHeader::raw_frame_bound(&selected),
            Err(Error::ResourceLimit)
        );
    }

    #[test]
    fn header_plus_real_raw_blocks_forms_a_complete_decodable_frame() {
        use alloc::vec::Vec;
        for (size, log) in [(0, 10), (1, 10), (1025, 10), (4097, 10), (131_073, 20)] {
            let source: Vec<_> = (0..size).map(|index| (index % 251) as u8).collect();
            let selected = plan(size, log);
            let header = ReferenceFrameHeader::new(&selected);
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
                ReferenceFrameHeader::raw_frame_bound(&selected).unwrap()
            );
            assert_eq!(crate::decompress(&frame, size).unwrap(), source);
        }
    }

    #[test]
    fn plain_settings_reject_invalid_metadata_without_broadening_fast_admission() {
        for log in (0..10).chain(22..=255) {
            assert_eq!(
                ReferenceFrameSettings::new(0, log),
                Err(Error::InvalidParameter)
            );
        }
        if let Ok(size) = usize::try_from(u64::MAX) {
            assert_eq!(
                ReferenceFrameSettings::new(size, 21),
                Err(Error::InvalidParameter)
            );
        }
        let settings = ReferenceFrameSettings::new(2_097_153, 21).unwrap();
        assert_eq!(settings.window_size(), 2_097_152);
        assert_eq!(settings.block_limit(), 131_072);
        let mut request = CompressionParameterRequest::with_level(1);
        request.set_window_log(21).unwrap();
        assert_eq!(
            FastExecutionPlan::new(&request, settings.source_len()),
            Err(Error::ResourceLimit)
        );
    }

    #[test]
    fn original_c_partial_headers_match_including_window21_and_size_promotion() {
        // These are the original incomplete C header probes, not complete frame
        // decodes. Their exact NDJSON grammar/provenance is checked offline.
        let probes = [
            (
                255_u64,
                21,
                include_str!("../tests/upstream/double-fast-frames/header-0.stdout"),
            ),
            (
                256_u64,
                21,
                include_str!("../tests/upstream/double-fast-frames/header-1.stdout"),
            ),
            (
                65791_u64,
                21,
                include_str!("../tests/upstream/double-fast-frames/header-2.stdout"),
            ),
            (
                65792_u64,
                21,
                include_str!("../tests/upstream/double-fast-frames/header-3.stdout"),
            ),
            (
                4294967294_u64,
                21,
                include_str!("../tests/upstream/double-fast-frames/header-4.stdout"),
            ),
            (
                4294967295_u64,
                21,
                include_str!("../tests/upstream/double-fast-frames/header-5.stdout"),
            ),
            (
                4294967296_u64,
                21,
                include_str!("../tests/upstream/double-fast-frames/header-6.stdout"),
            ),
            (
                1024_u64,
                10,
                include_str!("../tests/upstream/double-fast-frames/header-7.stdout"),
            ),
            (
                1025_u64,
                10,
                include_str!("../tests/upstream/double-fast-frames/header-8.stdout"),
            ),
            (
                1048576_u64,
                20,
                include_str!("../tests/upstream/double-fast-frames/header-9.stdout"),
            ),
            (
                1048577_u64,
                20,
                include_str!("../tests/upstream/double-fast-frames/header-10.stdout"),
            ),
            (
                2097152_u64,
                21,
                include_str!("../tests/upstream/double-fast-frames/header-11.stdout"),
            ),
            (
                2097153_u64,
                21,
                include_str!("../tests/upstream/double-fast-frames/header-12.stdout"),
            ),
        ];
        let mut compared = 0;
        for (size, log, observation) in probes {
            let Ok(length) = usize::try_from(size) else {
                continue;
            };
            let encoded = observation
                .split("\"bytes\":\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            assert_eq!(encoded.len() % 2, 0);
            let expected: alloc::vec::Vec<_> = encoded
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| {
                    let text = core::str::from_utf8(pair).unwrap();
                    u8::from_str_radix(text, 16).unwrap()
                })
                .collect();
            let settings = ReferenceFrameSettings::new(length, log).unwrap();
            let header = ReferenceFrameHeader::new(&settings);
            assert_eq!(header.as_bytes(), expected, "size={size}, window={log}");
            compared += 1;
        }
        assert_eq!(compared, if usize::BITS == 32 { 12 } else { 13 });
    }
}
