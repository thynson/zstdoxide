//! Checked scalar bounds and borrowed skippable-frame serialization.
//!
//! Bound formula/constants derive from facebook/zstd v1.5.7 `lib/zstd.h`
//! (`ZSTD_COMPRESSBOUND`, `ZSTD_MAX_INPUT_SIZE`), under LICENSE.zstd.
//! Skippable serialization corresponds to `ZSTD_writeSkippableFrame`.

#![forbid(unsafe_code)]

use crate::frame::MAX_BLOCK_SIZE;
use crate::{Error, Result};

// Repeating 0xff00 bytes on supported 32/64-bit targets, without a truncating cast.
const MAX_INPUT_SIZE: usize = (usize::MAX / 257) * 256;

/// Returns a checked worst-case encoded size for one-pass compression.
///
/// Uses the pinned v1.5.7 `ZSTD_compressBound` formula. It covers this crate's
/// ordinary one-shot compression, dictionary variants and currently admitted
/// reference one-shot profiles. It excludes arbitrarily flushed streaming
/// output and concatenations of separately compressed frames. This is neither
/// a workspace size nor a guarantee that allocation or compression succeeds.
///
/// Source sizes at or above the native pointer-width limit return
/// [`Error::ResourceLimit`]; no zero/error sentinels are reserved. The operation
/// allocates nothing and checks all arithmetic.
///
/// ```
/// use zstdoxide::{compress, compression_bound};
/// let input = b"a one-pass compressed frame";
/// assert!(compress(input)?.len() <= compression_bound(input.len())?);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
pub fn compression_bound(source_size: usize) -> Result<usize> {
    if source_size >= MAX_INPUT_SIZE {
        return Err(Error::ResourceLimit);
    }
    let margin = if source_size < MAX_BLOCK_SIZE {
        (MAX_BLOCK_SIZE - source_size) >> 11
    } else {
        0
    };
    source_size
        .checked_add(source_size >> 8)
        .and_then(|bound| bound.checked_add(margin))
        .ok_or(Error::ResourceLimit)
}

fn skippable_extent(payload_size: usize) -> Result<(u32, usize)> {
    let field = u32::try_from(payload_size).map_err(|_| Error::ResourceLimit)?;
    let extent = payload_size.checked_add(8).ok_or(Error::ResourceLimit)?;
    Ok((field, extent))
}

/// Writes a skippable frame into caller-owned storage and returns its extent.
///
/// `variant` selects one of sixteen magic numbers. The eight-byte header and
/// exact payload are written; all unused output bytes remain unchanged. The
/// payload and exclusive output borrow are independent, and neither is retained.
/// No allocation occurs. Every error leaves the entire output unchanged.
///
/// Invalid variants return [`Error::InvalidParameter`] first. Unrepresentable
/// u32 payload length, extent overflow or insufficient output capacity return
/// [`Error::ResourceLimit`]. This explicit precedence differs from native
/// destination-first checks when multiple arguments are invalid.
///
/// ```
/// use zstdoxide::{skippable_payload, write_skippable_frame};
/// let mut storage = [0xa5; 32];
/// let written = write_skippable_frame(&mut storage, b"metadata", 2)?;
/// assert_eq!(skippable_payload(&storage[..written])?, (2, b"metadata".as_slice()));
/// assert!(storage[written..].iter().all(|&byte| byte == 0xa5));
/// # Ok::<(), zstdoxide::Error>(())
/// ```
///
/// Safe borrows prevent overlapping payload and destination storage:
///
/// ```compile_fail
/// use zstdoxide::write_skippable_frame;
/// let mut bytes = [0_u8; 32];
/// write_skippable_frame(&mut bytes, &bytes[..8], 0).unwrap();
/// ```
pub fn write_skippable_frame(output: &mut [u8], payload: &[u8], variant: u8) -> Result<usize> {
    if variant > 15 {
        return Err(Error::InvalidParameter);
    }
    let (length, extent) = skippable_extent(payload.len())?;
    let frame = output.get_mut(..extent).ok_or(Error::ResourceLimit)?;
    // All fallible validation precedes mutation. The checked extent is at least
    // eight and leaves exactly payload.len() bytes for the independent input.
    frame[..4].copy_from_slice(&(0x184d_2a50 + u32::from(variant)).to_le_bytes());
    frame[4..8].copy_from_slice(&length.to_le_bytes());
    frame[8..].copy_from_slice(payload);
    Ok(extent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn original_compress_bound_powers_and_too_large_input() {
        // Pinned tests/fuzzer.c:test_compressBound, powers 0..29 and MAX+1.
        for shift in 0..30 {
            let size = 1_usize << shift;
            let n = size as u128;
            let expected = n + n / 256 + if n < 131072 { (131072 - n) / 2048 } else { 0 };
            assert_eq!(compression_bound(size), Ok(expected as usize));
        }
        assert_eq!(
            compression_bound(MAX_INPUT_SIZE + 1),
            Err(Error::ResourceLimit)
        );
    }

    #[test]
    fn scalar_bound_transitions_and_pointer_width_limits() {
        assert_eq!(compression_bound(0), Ok(64));
        for size in [
            1,
            2047,
            2048,
            2049,
            131071,
            131072,
            131073,
            MAX_INPUT_SIZE - 1,
        ] {
            let n = size as u128;
            let expected = n + n / 256 + if n < 131072 { (131072 - n) / 2048 } else { 0 };
            assert!(expected <= usize::MAX as u128);
            assert_eq!(compression_bound(size), Ok(expected as usize));
        }
        assert_eq!(compression_bound(MAX_INPUT_SIZE), Err(Error::ResourceLimit));
        assert_eq!(compression_bound(usize::MAX), Err(Error::ResourceLimit));
        #[cfg(target_pointer_width = "32")]
        assert_eq!(MAX_INPUT_SIZE, 0xff00_ff00);
        #[cfg(target_pointer_width = "64")]
        assert_eq!(MAX_INPUT_SIZE, 0xff00_ff00_ff00_ff00);
    }

    #[test]
    fn bound_covers_worst_default_and_one_kib_reference_framing() {
        // Independent wide raw-fallback bounds: default 128 KiB blocks include
        // the maximal dictionary header/checksum, reference blocks >=1 KiB
        // include their maximal known-source header and no checksum/dictionary.
        let check = |size: usize| {
            let n = size as u128;
            let default = n + 3 * n.div_ceil(131072).max(1) + 21;
            let reference = n + 3 * n.div_ceil(1024).max(1) + 14;
            let bound = compression_bound(size).unwrap() as u128;
            assert!(default <= bound, "default {size}");
            assert!(reference <= bound, "reference {size}");
        };
        for size in 0..=131073 {
            check(size);
        }
        for size in [262143, 262144, 262145, 1 << 20, MAX_INPUT_SIZE - 1] {
            check(size);
        }
    }

    #[test]
    fn skippable_numeric_limits_without_fabricated_slices() {
        assert_eq!(skippable_extent(0), Ok((0, 8)));
        let max = u32::MAX as usize;
        if let Some(extent) = max.checked_add(8) {
            assert_eq!(skippable_extent(max), Ok((u32::MAX, extent)));
            assert_eq!(skippable_extent(max + 1), Err(Error::ResourceLimit));
        } else {
            assert_eq!(skippable_extent(max), Err(Error::ResourceLimit));
            assert_eq!(skippable_extent(max - 7), Err(Error::ResourceLimit));
            assert_eq!(skippable_extent(max - 8), Ok((u32::MAX - 8, max)));
        }
        assert_eq!(skippable_extent(usize::MAX), Err(Error::ResourceLimit));
    }

    #[test]
    fn all_variants_exact_payload_and_preserved_suffix() {
        for variant in 0..16 {
            for size in [0, 1, 7, 8, 9, 255, 256, 4095, 4096, 131072] {
                let payload: alloc::vec::Vec<_> = (0..size).map(|i| (i * 37 + 5) as u8).collect();
                let mut output = vec![0xa5; size + 8 + 19];
                let written = write_skippable_frame(&mut output, &payload, variant).unwrap();
                assert_eq!(written, size + 8);
                assert_eq!(
                    &output[..4],
                    &(0x184d_2a50 + u32::from(variant)).to_le_bytes()
                );
                assert_eq!(&output[4..8], &(size as u32).to_le_bytes());
                assert_eq!(&output[8..written], payload);
                assert!(output[written..].iter().all(|&b| b == 0xa5));
            }
        }
    }

    #[test]
    fn rejected_capacity_or_variant_never_writes() {
        let payload = b"nonempty";
        for capacity in 0..payload.len() + 8 {
            let mut output = vec![0xa5; capacity];
            assert_eq!(
                write_skippable_frame(&mut output, payload, 0),
                Err(Error::ResourceLimit)
            );
            assert!(output.iter().all(|&b| b == 0xa5));
        }
        for variant in 16..=255 {
            let mut output = [0xa5; 32];
            assert_eq!(
                write_skippable_frame(&mut output, payload, variant),
                Err(Error::InvalidParameter)
            );
            assert_eq!(output, [0xa5; 32]);
            assert_eq!(
                write_skippable_frame(&mut [], payload, variant),
                Err(Error::InvalidParameter)
            );
        }
    }
}
