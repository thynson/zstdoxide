//! Shared-buffer dictionary history under foreign pointer widths and byte order.
#![forbid(unsafe_code)]
use zstdoxide::{
    DecodeOptions, Dictionary, Error, decompress_in_place_with_dictionary, decompression_margin,
};

#[test]
fn dictionary_crossings_frame_resets_overlap_and_guards() {
    type Case<'a> = (&'a [u8], bool, &'a [u8], &'a [u8]);
    let cases: &[Case<'_>] = &[
        (
            include_bytes!("upstream/dictionaries/boundary-dictionary.raw"),
            true,
            include_bytes!("upstream/dictionaries/p0-far-dictionary.zst"),
            include_bytes!("upstream/dictionaries/p0-far-dictionary.expected"),
        ),
        (
            include_bytes!("upstream/dictionaries/boundary-dictionary.raw"),
            true,
            include_bytes!("upstream/dictionaries/dictionary-to-frame-eviction.zst"),
            include_bytes!("upstream/dictionaries/dictionary-to-frame-eviction.expected"),
        ),
        (
            include_bytes!("upstream/dictionaries/formatted.dict"),
            false,
            include_bytes!("upstream/dictionaries/dictionary-treeless-literals.zst"),
            include_bytes!("upstream/dictionaries/dictionary-treeless-literals.expected"),
        ),
        (
            include_bytes!("upstream/dictionaries/duplicate-reps.dict"),
            false,
            include_bytes!("upstream/dictionaries/duplicate-reps-initial.zst"),
            include_bytes!("upstream/dictionaries/duplicate-reps-initial.expected"),
        ),
    ];
    for &(dictionary_bytes, raw, frame, expected) in cases {
        let dictionary = if raw {
            Dictionary::raw(dictionary_bytes)
        } else {
            Dictionary::from_bytes(dictionary_bytes)
        }
        .unwrap()
        .into_shared();
        let dictionary_before = dictionary.content().to_vec();
        let mut combined = frame.to_vec();
        combined.extend_from_slice(&[0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0]);
        combined.extend_from_slice(frame);
        let capacity =
            (2 * expected.len() + decompression_margin(&combined).unwrap()).max(combined.len());
        let start = capacity - combined.len();
        let mut bytes = vec![0xa5; capacity + 16];
        bytes[8 + start..8 + capacity].copy_from_slice(&combined);
        let before = bytes.clone();
        assert_eq!(
            decompress_in_place_with_dictionary(
                &mut bytes[8..8 + capacity],
                start,
                &dictionary,
                DecodeOptions::new(expected.len() * 2)
            ),
            Ok(expected.len() * 2)
        );
        assert_eq!(&bytes[8..8 + expected.len()], expected);
        assert_eq!(&bytes[8 + expected.len()..8 + expected.len() * 2], expected);
        assert_eq!(&bytes[..8], &before[..8]);
        assert_eq!(
            &bytes[8 + expected.len() * 2..],
            &before[8 + expected.len() * 2..]
        );
        bytes.copy_from_slice(&before);
        let limit = expected.len() - 1;
        assert_eq!(
            decompress_in_place_with_dictionary(
                &mut bytes[8..8 + capacity],
                start,
                &dictionary,
                DecodeOptions::new(limit)
            ),
            Err(Error::ResourceLimit)
        );
        assert_eq!(&bytes[8 + limit..], &before[8 + limit..]);
        assert_eq!(dictionary.content(), dictionary_before);
        let raw: Vec<_> = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 24, 193, 0, 0]
            .into_iter()
            .chain(0..24)
            .collect();
        let mut bytes = raw.clone();
        assert_eq!(
            decompress_in_place_with_dictionary(&mut bytes, 0, &dictionary, DecodeOptions::new(24)),
            Ok(24)
        );
        assert_eq!(&bytes[..24], &(0..24).collect::<Vec<_>>());
        assert_eq!(&bytes[24..], &raw[24..]);
    }
}
