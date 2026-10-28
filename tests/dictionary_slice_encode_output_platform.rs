extern crate alloc;
use alloc::alloc::Global;
use zstdoxide::{
    DecodeOptions, Dictionary, Error, FrameHeader, compress_into_with_dictionary_in,
    compress_with_dictionary_in, decompress_with_dictionary_in,
};

#[test]
fn borrowed_dictionary_compression_wire_bytes_capacity_and_guards() {
    let mut prepared = include_bytes!("upstream/dictionaries/formatted.dict").to_vec();
    prepared[4..8].copy_from_slice(&0x0102_0304u32.to_le_bytes());
    for dictionary in [
        Dictionary::raw(b"pattern ").unwrap(),
        Dictionary::from_bytes(&prepared).unwrap(),
    ] {
        let before = dictionary.as_bytes().to_vec();
        for input in [&b""[..], &b"pattern pattern pattern pattern"[..]] {
            let view = dictionary.view();
            let expected = compress_with_dictionary_in(input, &view, Global).unwrap();
            let mut output = vec![0xa5; expected.len() + 10];
            assert_eq!(
                compress_into_with_dictionary_in(
                    input,
                    &mut output[2..2 + expected.len() + 5],
                    &view,
                    Global
                ),
                Ok(expected.len())
            );
            assert_eq!(&output[2..2 + expected.len()], expected);
            let header = FrameHeader::parse(&expected).unwrap().0;
            assert_eq!(header.dictionary_id, dictionary.id());
            assert_eq!(header.content_size, Some(input.len() as u64));
            assert_eq!(
                decompress_with_dictionary_in(
                    &output[2..2 + expected.len()],
                    &view,
                    DecodeOptions::new(input.len()),
                    Global
                )
                .unwrap(),
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
                compress_into_with_dictionary_in(
                    input,
                    &mut output[2..2 + expected.len() - 1],
                    &view,
                    Global
                ),
                Err(Error::ResourceLimit)
            );
            assert!(
                output[..2]
                    .iter()
                    .chain(&output[1 + expected.len()..])
                    .all(|&b| b == 0xa5)
            );
            assert_eq!(dictionary.as_bytes(), before);
        }
    }
}
