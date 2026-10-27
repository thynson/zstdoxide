//! Structural queries deliberately remain distinct from content validation.
#![forbid(unsafe_code)]
use zstdoxide::{
    Error, FrameHeader, FrameKind, decompress, decompressed_size, decompression_bound,
    frame_compressed_size, frame_info, is_skippable_frame, skippable_payload,
};
fn raw(size: Option<u64>, checksum: bool) -> Vec<u8> {
    let mut bytes = vec![0x28, 0xb5, 0x2f, 0xfd];
    if let Some(size) = size {
        bytes.push(0xe0 | (u8::from(checksum) * 4));
        bytes.extend_from_slice(&size.to_le_bytes());
    } else {
        bytes.extend_from_slice(&[u8::from(checksum) * 4, 56]);
    }
    bytes.extend_from_slice(&[25, 0, 0, b'a', b'b', b'c']);
    if checksum {
        bytes.extend_from_slice(&[0, 0, 0, 0]);
    }
    bytes
}
fn skip(variant: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x50 + variant, 0x2a, 0x4d, 0x18];
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}
#[test]
fn copied_first_frame_metadata_ignores_trailing_input() {
    for size in [0, 3, 255, 256, 65535, 1_u64 << 32, u64::MAX] {
        let bytes = raw(Some(size), false);
        let header = FrameHeader::parse(&bytes).unwrap().0;
        let info = frame_info(&bytes).unwrap();
        assert_eq!(info.kind(), FrameKind::Standard(header));
        assert_eq!(info.content_size(), Some(size));
        assert_eq!(info.block_count(), 1);
        assert_eq!(info.encoded_size(), bytes.len());
        assert_eq!(info.decompression_bound(), Ok(size));
        let mut trailing = bytes.clone();
        trailing.extend_from_slice(b"garbage");
        assert_eq!(frame_info(&trailing), Ok(info));
        assert_eq!(frame_compressed_size(&trailing), Ok(bytes.len()));
        assert!(decompressed_size(&trailing).is_err());
        assert!(decompression_bound(&trailing).is_err());
        drop(bytes);
        assert_eq!(info.content_size(), Some(size));
    }
}
#[test]
fn all_skip_variants_borrow_exact_payload_and_distinguish_marker_from_validation() {
    for variant in 0..16 {
        for payload in [b"".as_slice(), b"payload", &[0xa5; 257]] {
            let frame = skip(variant, payload);
            let mut bytes = frame.clone();
            bytes.extend_from_slice(b"trailing");
            let before = bytes.clone();
            let info = frame_info(&bytes).unwrap();
            assert_eq!(
                info.kind(),
                FrameKind::Skippable {
                    variant,
                    payload_size: payload.len() as u32
                }
            );
            assert_eq!(info.encoded_size(), frame.len());
            assert_eq!(info.block_count(), 0);
            assert_eq!(info.content_size(), Some(0));
            assert_eq!(info.decompression_bound(), Ok(0));
            let (found_variant, found) = skippable_payload(&bytes).unwrap();
            assert_eq!(found_variant, variant);
            assert_eq!(found, payload);
            assert_eq!(found.as_ptr(), bytes[8..].as_ptr());
            assert_eq!(bytes, before);
            for end in 0..frame.len() {
                assert_eq!(is_skippable_frame(&frame[..end]), end >= 4);
                assert!(skippable_payload(&frame[..end]).is_err());
                assert!(frame_info(&frame[..end]).is_err());
            }
        }
    }
}
#[test]
fn exact_concatenation_checks_every_tail_after_known_or_unknown_sizes() {
    assert_eq!(decompressed_size(&[]), Ok(Some(0)));
    assert_eq!(decompression_bound(&[]), Ok(0));
    assert_eq!(frame_info(&[]), Err(Error::UnexpectedEof));
    assert_eq!(frame_compressed_size(&[]), Err(Error::UnexpectedEof));
    for unknown in [false, true] {
        let first = raw(if unknown { None } else { Some(3) }, false);
        let mut bytes = skip(15, b"skip");
        bytes.extend_from_slice(&first);
        bytes.extend_from_slice(&skip(0, b""));
        bytes.extend_from_slice(&raw(Some(3), false));
        assert_eq!(
            decompressed_size(&bytes),
            Ok(if unknown { None } else { Some(6) })
        );
        assert_eq!(
            decompression_bound(&bytes),
            Ok(if unknown { 131072 + 3 } else { 6 })
        );
        for tail in [
            b"x".as_slice(),
            &[0x28, 0xb5, 0x2f, 0xfd],
            &[0x50, 0x2a, 0x4d, 0x18, 1, 0, 0, 0],
        ] {
            let mut changed = bytes.clone();
            changed.extend_from_slice(tail);
            assert!(decompressed_size(&changed).is_err());
            assert!(decompression_bound(&changed).is_err());
        }
    }
}
#[test]
fn small_encoded_input_has_bound_above_four_gib_without_pointer_width_overflow() {
    let count = (1_usize << 15) + 1;
    let mut bytes = vec![0x28, 0xb5, 0x2f, 0xfd, 0, 56];
    for _ in 1..count {
        bytes.extend_from_slice(&[0, 0, 0]);
    }
    bytes.extend_from_slice(&[1, 0, 0]);
    let info = frame_info(&bytes).unwrap();
    assert_eq!(info.block_count(), count);
    assert_eq!(info.encoded_size(), 6 + count * 3);
    assert_eq!(decompressed_size(&bytes), Ok(None));
    assert_eq!(decompression_bound(&bytes), Ok((count as u64) * 131072));
    assert!(decompression_bound(&bytes).unwrap() > 0x1_0000_0000);
}
#[test]
fn full_u64_sizes_and_aggregate_overflow_have_no_reserved_sentinels() {
    for size in [u64::MAX - 1, u64::MAX] {
        let frame = raw(Some(size), false);
        assert_eq!(decompressed_size(&frame), Ok(Some(size)));
        assert_eq!(decompression_bound(&frame), Ok(size));
        let mut joined = frame;
        joined.extend_from_slice(&raw(Some(3), false));
        assert_eq!(decompressed_size(&joined), Err(Error::ResourceLimit));
        assert_eq!(decompression_bound(&joined), Err(Error::ResourceLimit));
    }
    let mut unknown = raw(None, false);
    unknown.extend_from_slice(&raw(Some(u64::MAX), false));
    unknown.extend_from_slice(&raw(Some(1), false));
    assert_eq!(decompressed_size(&unknown), Err(Error::ResourceLimit));
    assert_eq!(decompression_bound(&unknown), Err(Error::ResourceLimit));
}
#[test]
fn structural_success_does_not_validate_payload_checksum_or_size_truth() {
    let frame = raw(Some(3), true);
    assert_eq!(frame_info(&frame).unwrap().content_size(), Some(3));
    assert_eq!(decompression_bound(&frame), Ok(3));
    assert_eq!(decompress(&frame, 16), Err(Error::ChecksumMismatch));
    let mismatched = raw(Some(4), false);
    assert_eq!(decompressed_size(&mismatched), Ok(Some(4)));
    assert_eq!(decompress(&mismatched, 16), Err(Error::ContentSizeMismatch));
    let mut invalid = vec![0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 13, 0, 0, 0xff];
    assert_eq!(frame_compressed_size(&invalid), Ok(invalid.len()));
    assert!(decompress(&invalid, 16).is_err());
    invalid[6] = 15;
    assert_eq!(frame_info(&invalid), Err(Error::CorruptData));
}
#[test]
fn every_fixture_truncation_and_impossible_skip_length_is_rejected() {
    for frame in [
        include_bytes!("upstream/decodecorpus/compressed-05.zst").as_slice(),
        include_bytes!("upstream/decodecorpus/mixed-04.zst").as_slice(),
    ] {
        assert_eq!(frame_compressed_size(frame), Ok(frame.len()));
        for end in 0..frame.len() {
            assert!(frame_info(&frame[..end]).is_err(), "end {end}");
            if end != 0 {
                assert!(decompressed_size(&frame[..end]).is_err(), "end {end}");
                assert!(decompression_bound(&frame[..end]).is_err(), "end {end}");
            }
        }
    }
    let impossible = [0x50, 0x2a, 0x4d, 0x18, 255, 255, 255, 255];
    assert!(is_skippable_frame(&impossible));
    assert_eq!(skippable_payload(&impossible), Err(Error::UnexpectedEof));
    assert_eq!(frame_info(&impossible), Err(Error::UnexpectedEof));
    assert_eq!(
        skippable_payload(&raw(Some(3), false)),
        Err(Error::InvalidMagic)
    );
    for magic in [0xfd2f_b524_u32, 0xfd2f_b527, 0x1234_5678] {
        let mut bytes = magic.to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0; 16]);
        assert!(!is_skippable_frame(&bytes));
        assert_eq!(frame_info(&bytes), Err(Error::InvalidMagic));
    }
}
