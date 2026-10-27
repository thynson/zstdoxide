//! Structural size arithmetic and borrowed payloads across pointer width/endian.
#![forbid(unsafe_code)]
use zstdoxide::{
    Error, decompressed_size, decompression_bound, frame_info, is_skippable_frame,
    skippable_payload,
};
#[test]
fn borrowed_payloads_unknown_tails_and_checked_u64_totals() {
    for variant in 0..16 {
        let frame = [0x50 + variant, 0x2a, 0x4d, 0x18, 3, 0, 0, 0, 7, 8, 9];
        let (v, payload) = skippable_payload(&frame).unwrap();
        assert_eq!(v, variant);
        assert_eq!(payload, &[7, 8, 9]);
        assert_eq!(payload.as_ptr(), frame[8..].as_ptr());
        assert_eq!(frame_info(&frame).unwrap().encoded_size(), 11);
        for end in 0..frame.len() {
            assert_eq!(is_skippable_frame(&frame[..end]), end >= 4);
            assert!(skippable_payload(&frame[..end]).is_err());
        }
    }
    let mut huge = vec![0x28, 0xb5, 0x2f, 0xfd, 0xe0];
    huge.extend_from_slice(&u64::MAX.to_le_bytes());
    huge.extend_from_slice(&[1, 0, 0]);
    assert_eq!(decompressed_size(&huge), Ok(Some(u64::MAX)));
    assert_eq!(decompression_bound(&huge), Ok(u64::MAX));
    huge.extend_from_slice(&[0x28, 0xb5, 0x2f, 0xfd, 0x20, 1, 1, 0, 0]);
    assert_eq!(decompressed_size(&huge), Err(Error::ResourceLimit));
    assert_eq!(decompression_bound(&huge), Err(Error::ResourceLimit));
    let unknown = [0x28, 0xb5, 0x2f, 0xfd, 0, 56, 1, 0, 0];
    assert_eq!(decompressed_size(&unknown), Ok(None));
    assert_eq!(decompression_bound(&unknown), Ok(131072));
    let mut malformed = unknown.to_vec();
    malformed.push(0);
    assert_eq!(decompressed_size(&malformed), Err(Error::UnexpectedEof));
}
#[test]
fn block_product_exceeds_four_gib_on_small_actual_input() {
    let count = (1_usize << 15) + 1;
    let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0, 56];
    for _ in 1..count {
        frame.extend_from_slice(&[0, 0, 0]);
    }
    frame.extend_from_slice(&[1, 0, 0]);
    assert_eq!(frame_info(&frame).unwrap().block_count(), count);
    assert_eq!(decompression_bound(&frame), Ok((count as u64) * 131072));
    assert!(decompression_bound(&frame).unwrap() > 0x1_0000_0000);
}
