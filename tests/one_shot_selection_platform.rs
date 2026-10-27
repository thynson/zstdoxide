//! Foreign width/endian ownership, dictionary dependence and real overlap.
#![forbid(unsafe_code)]
use zstdoxide::{DecodeOptions, Dictionary, Error, OneShotDecoder, decompression_margin};
#[test]
fn call_scoped_selection_crossings_failures_and_overlap() {
    type Case<'a> = (&'a [u8], bool, &'a [u8], &'a [u8]);
    let cases: &[Case<'_>] = &[
        (
            include_bytes!("upstream/dictionaries/boundary-dictionary.raw"),
            true,
            include_bytes!("upstream/dictionaries/p0-far-dictionary.zst"),
            include_bytes!("upstream/dictionaries/p0-far-dictionary.expected"),
        ),
        (
            include_bytes!("upstream/dictionaries/formatted.dict"),
            false,
            include_bytes!("upstream/dictionaries/dictionary-treeless-literals.zst"),
            include_bytes!("upstream/dictionaries/dictionary-treeless-literals.expected"),
        ),
    ];
    for &(bytes, raw, frame, expected) in cases {
        let dictionary = if raw {
            Dictionary::raw(bytes)
        } else {
            Dictionary::from_bytes(bytes)
        }
        .unwrap();
        let mut joined = frame.to_vec();
        joined.extend_from_slice(&[0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0]);
        joined.extend_from_slice(frame);
        let capacity =
            (expected.len() * 2 + decompression_margin(&joined).unwrap()).max(joined.len());
        let start = capacity - joined.len();
        for in_place in [false, true] {
            let mut decoder = OneShotDecoder::with_prefix(
                DecodeOptions::new(expected.len() * 2),
                dictionary.view(),
            );
            let mut output = vec![0xa5; capacity + 16];
            if in_place {
                output[8 + start..8 + capacity].copy_from_slice(&joined);
            }
            let before = output.clone();
            let result = if in_place {
                decoder.decompress_in_place(&mut output[8..8 + capacity], start)
            } else {
                decoder.decompress_into(&joined, &mut output[8..8 + capacity])
            };
            assert_eq!(result, Ok(expected.len() * 2));
            assert_eq!(&output[8..8 + expected.len()], expected);
            assert_eq!(
                &output[8 + expected.len()..8 + expected.len() * 2],
                expected
            );
            assert_eq!(&output[..8], &before[..8]);
            assert_eq!(
                &output[8 + expected.len() * 2..],
                &before[8 + expected.len() * 2..]
            );
            assert!(decoder.dictionary().is_none());
            assert!(!decoder.prefix_pending());
            assert!(
                decoder
                    .decompress_into(frame, &mut output[8..8 + capacity])
                    .is_err()
            );
            decoder.set_dictionary(Some(dictionary.view()));
            decoder.set_options(DecodeOptions::new(0));
            assert_eq!(
                decoder.decompress_into(frame, &mut []),
                Err(Error::ResourceLimit)
            );
            assert!(decoder.dictionary().is_some());
            decoder.set_options(DecodeOptions::new(expected.len()));
            assert_eq!(
                decoder.decompress_into(frame, &mut output[8..8 + capacity]),
                Ok(expected.len())
            );
            assert_eq!(&output[8..8 + expected.len()], expected);
            decoder.set_prefix(Some(dictionary.view()));
            assert_eq!(
                decoder.decompress_into(b"", &mut []),
                Err(Error::UnexpectedEof)
            );
            assert!(decoder.dictionary().is_none());
        }
        let raw_frame: Vec<_> = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 24, 193, 0, 0]
            .into_iter()
            .chain(0..24)
            .collect();
        let mut output = raw_frame.clone();
        let mut decoder = OneShotDecoder::with_prefix(DecodeOptions::new(24), dictionary.view());
        assert_eq!(decoder.decompress_in_place(&mut output, 0), Ok(24));
        assert_eq!(&output[..24], &(0..24).collect::<Vec<_>>());
        assert_eq!(&output[24..], &raw_frame[24..]);
        assert!(decoder.dictionary().is_none());
    }
}
