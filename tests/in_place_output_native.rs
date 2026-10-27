//! Replay pinned v1.5.7 shared-allocation layouts, including original fuzzer cases.
#![forbid(unsafe_code)]
use std::{fs, path::PathBuf};
use zstdoxide::{DecodeOptions, Error, decompress_in_place_with_options, decompression_margin};

#[test]
#[ignore = "requires ZSTDOXIDE_IN_PLACE_CASES from the pinned native campaign"]
fn pinned_native_shared_allocation_layouts() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_IN_PLACE_CASES").unwrap());
    let manifest = fs::read_to_string(root.join("rust-cases.tsv")).unwrap();
    let mut count = 0;
    for line in manifest.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 6);
        let name = fields[0];
        let frame_name = fields[1];
        assert!(
            name.bytes()
                .chain(frame_name.bytes())
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        );
        let frame = fs::read(root.join("inputs").join(format!("{frame_name}.zst"))).unwrap();
        let capacity: usize = fields[2].parse().unwrap();
        let limit: usize = fields[3].parse().unwrap();
        assert!(frame.len() <= capacity && limit <= capacity && capacity <= 32 * 1024 * 1024);
        let start = capacity - frame.len();
        let mut buffer = vec![0xa5; capacity + 64];
        buffer[32 + start..32 + capacity].copy_from_slice(&frame);
        let before = buffer.clone();
        let result = decompress_in_place_with_options(
            &mut buffer[32..32 + capacity],
            start,
            DecodeOptions::new(limit),
        );
        match fields[4] {
            "ok" => {
                let expected =
                    fs::read(root.join("normal-replay").join(format!("{name}.raw"))).unwrap();
                assert_eq!(result, Ok(expected.len()), "{name}");
                assert_eq!(&buffer[32..32 + expected.len()], expected, "{name}");
                assert_eq!(
                    &buffer[32 + expected.len()..],
                    &before[32 + expected.len()..],
                    "{name}"
                );
            }
            "empty-difference" => {
                assert!(frame.is_empty());
                assert_eq!(result, Err(Error::UnexpectedEof), "{name}");
            }
            "error" => assert!(result.is_err(), "{name}"),
            other => panic!("unknown expectation {other}"),
        }
        assert_eq!(&buffer[..32], &before[..32], "{name}");
        assert_eq!(&buffer[32 + limit..], &before[32 + limit..], "{name}");
        assert_eq!(fields[5], "guards-verified");
        count += 1;
    }
    assert_eq!(count, 1300);
    eprintln!("replayed {count} native shared-allocation layouts");
}

#[test]
#[ignore = "requires ZSTDOXIDE_IN_PLACE_CASES original 5 MiB native-generated fixtures"]
fn original_expanded_single_and_two_frame_assertions() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_IN_PLACE_CASES").unwrap());
    let original_size: usize = 5 * 1024 * 1024;
    for frames in [1, 2] {
        let encoded = fs::read(root.join("normal").join(format!("original-{frames}.zst"))).unwrap();
        let source = fs::read(root.join("normal").join(format!("original-{frames}.raw"))).unwrap();
        let size = if frames == 1 {
            original_size
        } else {
            (original_size / 3) * 2
        };
        assert_eq!(source.len(), size);
        let margin = decompression_margin(&encoded).unwrap();
        let capacity = original_size + margin;
        assert!(encoded.len() < capacity);
        if frames == 1 {
            assert!(original_size < encoded.len());
            // ZSTD_DECOMPRESSION_MARGIN(size, ZSTD_BLOCKSIZE_MAX):
            // max header + checksum + 3 bytes per maximum-sized block + block.
            assert!(margin <= 18 + 4 + 3 * original_size.div_ceil(131072) + 131072);
        }
        let mut buffer = vec![0xa5; capacity + 64];
        let start = capacity - encoded.len();
        buffer[32 + start..32 + capacity].copy_from_slice(&encoded);
        let before = buffer.clone();
        assert_eq!(
            decompress_in_place_with_options(
                &mut buffer[32..32 + capacity],
                start,
                DecodeOptions::new(capacity)
            ),
            Ok(size),
            "original-{frames}"
        );
        assert_eq!(&buffer[32..32 + size], source);
        assert_eq!(&buffer[..32], &before[..32]);
        assert_eq!(&buffer[32 + size..], &before[32 + size..]);
    }
}
