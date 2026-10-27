//! Allocation-free borrowed serialization on foreign width and endianness.
#![forbid(unsafe_code)]
use zstdoxide::{
    Error, compression_bound, frame_compressed_size, skippable_payload, write_skippable_frame,
};
#[test]
fn real_borrows_encode_little_endian_and_preserve_guards() {
    for variant in 0..16 {
        let payload = [3_u8, 17, 251, 0, 44];
        let mut storage = [0xa5; 45];
        let written = write_skippable_frame(&mut storage[16..29], &payload, variant).unwrap();
        assert_eq!(written, 13);
        assert_eq!(&storage[16..20], &[0x50 + variant, 0x2a, 0x4d, 0x18]);
        assert_eq!(&storage[20..24], &[5, 0, 0, 0]);
        assert_eq!(&storage[24..29], &payload);
        assert_eq!(
            skippable_payload(&storage[16..29]),
            Ok((variant, payload.as_slice()))
        );
        assert_eq!(frame_compressed_size(&storage[16..29]), Ok(13));
        assert!(
            storage[..16]
                .iter()
                .chain(&storage[29..])
                .all(|&b| b == 0xa5)
        );
    }
}
#[test]
fn invalid_arguments_preserve_output_and_scalar_bound_checks_width() {
    let mut output = [0xa5; 16];
    assert_eq!(
        write_skippable_frame(&mut output, b"ninebytes", 0),
        Err(Error::ResourceLimit)
    );
    assert_eq!(output, [0xa5; 16]);
    assert_eq!(
        write_skippable_frame(&mut output, b"", 16),
        Err(Error::InvalidParameter)
    );
    assert_eq!(output, [0xa5; 16]);
    assert_eq!(write_skippable_frame(&mut output, b"", 15), Ok(8));
    assert_eq!(&output[..8], &[0x5f, 0x2a, 0x4d, 0x18, 0, 0, 0, 0]);
    assert_eq!(&output[8..], &[0xa5; 8]);
    let maximum = (usize::MAX / 257) * 256;
    assert_eq!(compression_bound(maximum), Err(Error::ResourceLimit));
    assert_eq!(compression_bound(maximum + 1), Err(Error::ResourceLimit));
    let n = maximum - 1;
    assert_eq!(compression_bound(n), Ok(n + n / 256));
    assert_eq!(compression_bound(0), Ok(64));
}
