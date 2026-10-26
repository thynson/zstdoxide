//! Foreign-width/endian checked dictionary destinations and real compressed history.
#![forbid(unsafe_code)]
use std::alloc::Global;
use zstdoxide::{DecodeOptions, Dictionary, Error, decompress_into_with_dictionary_in};
macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!("upstream/dictionaries/", $name))
    };
}
#[test]
fn borrowed_dictionary_matches_cross_to_frame_and_preserve_guards() {
    let cases: &[(&[u8], &[u8], &[u8])] = &[
        (
            fixture!("boundary-dictionary.raw"),
            fixture!("p0-far-dictionary.zst"),
            fixture!("p0-far-dictionary.expected"),
        ),
        (
            fixture!("boundary-dictionary.raw"),
            fixture!("dictionary-to-frame-eviction.zst"),
            fixture!("dictionary-to-frame-eviction.expected"),
        ),
        (
            fixture!("formatted.dict"),
            fixture!("dictionary-treeless-four-streams.zst"),
            fixture!("dictionary-treeless-four-streams.expected"),
        ),
    ];
    for &(dictionary, frame, expected) in cases {
        let dictionary = Dictionary::from_bytes(dictionary).unwrap();
        let mut output = vec![0xa5; expected.len() + 18];
        assert_eq!(
            decompress_into_with_dictionary_in(
                frame,
                &mut output[1..expected.len() + 17],
                &dictionary.view(),
                DecodeOptions::new(expected.len()),
                Global
            ),
            Ok(expected.len())
        );
        assert_eq!(&output[1..expected.len() + 1], expected);
        assert_eq!(output[0], 0xa5);
        assert!(output[expected.len() + 1..].iter().all(|&b| b == 0xa5));
        output.fill(0xa5);
        assert_eq!(
            decompress_into_with_dictionary_in(
                frame,
                &mut output[1..expected.len()],
                &dictionary.view(),
                DecodeOptions::new(usize::MAX),
                Global
            ),
            Err(Error::ResourceLimit)
        );
        assert_eq!(output[0], 0xa5);
        assert!(output[expected.len()..].iter().all(|&b| b == 0xa5));
        let combined = [frame, frame].concat();
        let originals = [expected, expected].concat();
        let mut output = vec![0xa5; originals.len() + 2];
        assert_eq!(
            decompress_into_with_dictionary_in(
                &combined,
                &mut output[1..originals.len() + 1],
                &dictionary.view(),
                DecodeOptions::new(originals.len()),
                Global
            ),
            Ok(originals.len())
        );
        assert_eq!(&output[1..originals.len() + 1], originals);
        assert_eq!(output[0], 0xa5);
        assert_eq!(output[originals.len() + 1], 0xa5);
    }
    let dictionary = Dictionary::raw(fixture!("boundary-dictionary.raw")).unwrap();
    for frame in [
        fixture!("p-window-plus-one.zst").as_slice(),
        fixture!("dictionary-out-of-bounds.zst").as_slice(),
    ] {
        let mut output = [0xa5; 4098];
        assert_eq!(
            decompress_into_with_dictionary_in(
                frame,
                &mut output[1..4097],
                &dictionary.view(),
                DecodeOptions::new(4096),
                Global
            ),
            Err(Error::CorruptData)
        );
        assert_eq!(output[0], 0xa5);
        assert_eq!(output[4097], 0xa5);
    }
}
