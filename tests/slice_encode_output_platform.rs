use zstdoxide::{Error, compress, compress_into, decompress, frame_info};

#[test]
fn borrowed_compression_wire_bytes_capacity_and_guards() {
    for input in [
        &b""[..],
        &b"raw!?"[..],
        &b"rrrrrrrrrrrrrrrr"[..],
        &b"repeat phrase repeat phrase repeat phrase"[..],
    ] {
        let expected = compress(input).unwrap();
        let mut output = vec![0xa5; expected.len() + 10];
        assert_eq!(
            compress_into(input, &mut output[2..2 + expected.len() + 5]),
            Ok(expected.len())
        );
        assert_eq!(&output[2..2 + expected.len()], expected);
        assert_eq!(
            frame_info(&expected).unwrap().content_size(),
            Some(input.len() as u64)
        );
        assert_eq!(
            decompress(&output[2..2 + expected.len()], input.len()).unwrap(),
            input
        );
        assert!(
            output[..2]
                .iter()
                .chain(&output[2 + expected.len()..])
                .all(|&b| b == 0xa5)
        );
        output.fill(0xa5);
        assert_eq!(
            compress_into(input, &mut output[2..2 + expected.len() - 1]),
            Err(Error::ResourceLimit)
        );
        assert!(
            output[..2]
                .iter()
                .chain(&output[1 + expected.len()..])
                .all(|&b| b == 0xa5)
        );
    }
}
