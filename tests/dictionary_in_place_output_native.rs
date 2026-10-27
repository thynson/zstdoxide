//! Supplemental pinned dictionary layouts using one actual source/output allocation.
#![forbid(unsafe_code)]
use std::{fs, path::PathBuf};
use zstdoxide::{
    DecodeOptions, Dictionary, Error, FrameHeader, decompress_in_place_with_dictionary,
};

#[test]
#[ignore = "requires ZSTDOXIDE_DICTIONARY_IN_PLACE_CASES from the pinned native campaign"]
fn pinned_dictionary_shared_allocation_layouts() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_DICTIONARY_IN_PLACE_CASES").unwrap());
    let manifest = fs::read_to_string(root.join("rust-cases.tsv")).unwrap();
    let mut count = 0;
    let mut differences = [0; 2];
    for line in manifest.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 8);
        let [
            name,
            dictionary_name,
            kind,
            frame_name,
            capacity,
            limit,
            expectation,
            guards,
        ] = fields.as_slice()
        else {
            unreachable!()
        };
        assert!(
            name.bytes()
                .chain(dictionary_name.bytes())
                .chain(frame_name.bytes())
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        );
        let dictionary_bytes =
            fs::read(root.join("inputs").join(format!("{dictionary_name}.dict"))).unwrap();
        let dictionary = match *kind {
            "raw" => Dictionary::raw(&dictionary_bytes),
            "auto" => Dictionary::from_bytes(&dictionary_bytes),
            _ => panic!("unknown dictionary kind"),
        }
        .unwrap()
        .into_shared();
        let dictionary_before = dictionary.content().to_vec();
        let frame = fs::read(root.join("inputs").join(format!("{frame_name}.zst"))).unwrap();
        let capacity: usize = capacity.parse().unwrap();
        let limit: usize = limit.parse().unwrap();
        assert!(frame.len() <= capacity && limit <= capacity && capacity <= 32 * 1024 * 1024);
        let start = capacity - frame.len();
        let mut bytes = vec![0xa5; capacity + 64];
        bytes[32 + start..32 + capacity].copy_from_slice(&frame);
        let before = bytes.clone();
        let result = decompress_in_place_with_dictionary(
            &mut bytes[32..32 + capacity],
            start,
            &dictionary,
            DecodeOptions::new(limit),
        );
        match *expectation {
            "ok" => {
                let expected =
                    fs::read(root.join("normal-replay").join(format!("{name}.raw"))).unwrap();
                assert_eq!(result, Ok(expected.len()), "{name}");
                assert_eq!(&bytes[32..32 + expected.len()], expected, "{name}");
                assert_eq!(
                    &bytes[32 + expected.len()..],
                    &before[32 + expected.len()..],
                    "{name}"
                );
            }
            "empty-difference" => {
                assert!(frame.is_empty());
                assert_eq!(result, Err(Error::UnexpectedEof), "{name}");
                differences[0] += 1;
            }
            "window-difference" => {
                assert_eq!(*dictionary_name, "boundary");
                assert_eq!(*frame_name, "p-window-plus-one");
                assert_eq!(FrameHeader::parse(&frame).unwrap().0.window_size, 1024);
                assert_eq!(result, Err(Error::CorruptData), "{name}");
                let native =
                    fs::read(root.join("normal-replay").join(format!("{name}.raw"))).unwrap();
                assert_eq!(
                    native,
                    include_bytes!("upstream/dictionaries/p-window-plus-one.expected")
                );
                differences[1] += 1;
            }
            "error" => assert!(result.is_err(), "{name}"),
            _ => panic!("unknown expectation"),
        }
        assert_eq!(&bytes[..32], &before[..32], "{name}");
        assert_eq!(&bytes[32 + limit..], &before[32 + limit..], "{name}");
        assert_eq!(dictionary.content(), dictionary_before, "{name}");
        assert_eq!(*guards, "guards-verified");
        count += 1;
    }
    assert_eq!(count, 1836);
    assert_eq!(differences, [66, 3]);
    eprintln!(
        "replayed {count} dictionary shared-allocation layouts; 66 empty and 3 window-policy differences"
    );
}
