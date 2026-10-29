extern crate alloc;
use alloc::alloc::Global;
use zstdoxide::{
    DecodeOptions, Dictionary, Error, FrameHeader, OneShotEncoder, compress,
    compress_with_dictionary_in, decompress_with_dictionary_in,
};
#[test]
fn borrowed_compression_selection_consumes_attempts_and_preserves_wire_guards() {
    let mut prepared = include_bytes!("upstream/dictionaries/formatted.dict").to_vec();
    prepared[4..8].copy_from_slice(&0x0102_0304_u32.to_le_bytes());
    for dictionary in [
        Dictionary::raw(b"pattern ").unwrap(),
        Dictionary::from_bytes(&prepared).unwrap(),
    ] {
        let before = dictionary.as_bytes().to_vec();
        for input in [&b""[..], &b"a"[..]] {
            let expected = compress_with_dictionary_in(input, &dictionary.view(), Global).unwrap();
            let ordinary = compress(input).unwrap();
            let header = FrameHeader::parse(&expected).unwrap().0;
            assert_eq!(header.dictionary_id, dictionary.id());
            assert_eq!(header.content_size, Some(input.len() as u64));
            let mut encoder = OneShotEncoder::with_prefix(dictionary.view());
            let mut output = vec![0xa5; expected.len() + 10];
            assert_eq!(
                encoder.compress_into(input, &mut output[2..2 + expected.len() + 5]),
                Ok(expected.len())
            );
            assert_eq!(&output[2..2 + expected.len()], expected);
            assert!(
                output[..2]
                    .iter()
                    .chain(&output[2 + expected.len()..])
                    .all(|&b| b == 0xa5)
            );
            assert!(!encoder.prefix_pending());
            assert!(encoder.dictionary().is_none());
            assert_eq!(
                decompress_with_dictionary_in(
                    &expected,
                    &dictionary.view(),
                    DecodeOptions::new(input.len()),
                    Global
                )
                .unwrap(),
                input
            );
            output.fill(0xa5);
            assert_eq!(
                encoder.compress_into(input, &mut output[2..2 + ordinary.len()]),
                Ok(ordinary.len())
            );
            assert_eq!(&output[2..2 + ordinary.len()], ordinary);
            assert!(
                output[..2]
                    .iter()
                    .chain(&output[2 + ordinary.len()..])
                    .all(|&b| b == 0xa5)
            );
            encoder.set_prefix(Some(dictionary.view()));
            output.fill(0xa5);
            assert_eq!(
                encoder.compress_into(input, &mut output[2..1 + expected.len()]),
                Err(Error::ResourceLimit)
            );
            assert!(!encoder.prefix_pending());
            assert!(encoder.dictionary().is_none());
            assert!(
                output[..2]
                    .iter()
                    .chain(&output[1 + expected.len()..])
                    .all(|&b| b == 0xa5)
            );
            encoder.set_dictionary(Some(dictionary.view()));
            assert_eq!(
                encoder.compress_into(input, &mut []),
                Err(Error::ResourceLimit)
            );
            assert!(encoder.dictionary().is_some());
            assert!(!encoder.prefix_pending());
            output.fill(0xa5);
            assert_eq!(
                encoder.compress_into(input, &mut output[2..2 + expected.len()]),
                Ok(expected.len())
            );
            assert_eq!(&output[2..2 + expected.len()], expected);
            assert!(
                output[..2]
                    .iter()
                    .chain(&output[2 + expected.len()..])
                    .all(|&b| b == 0xa5)
            );
            assert_eq!(dictionary.as_bytes(), before);
        }
    }
}
