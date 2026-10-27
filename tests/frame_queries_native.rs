//! External pinned structural queries, with explicit semantic differences.
#![forbid(unsafe_code)]
use std::{collections::BTreeMap, fs, path::PathBuf};
use zstdoxide::{
    decompress, decompressed_size, decompression_bound, frame_compressed_size, frame_info,
    is_skippable_frame, skippable_payload,
};
fn root() -> PathBuf {
    PathBuf::from(std::env::var_os("ZSTDOXIDE_FRAME_QUERY_CASES").unwrap())
}
#[test]
#[ignore = "requires ZSTDOXIDE_FRAME_QUERY_CASES from the pinned native campaign"]
fn pinned_structural_query_rows_and_documented_differences() {
    let root = root();
    let native = fs::read_to_string(root.join("normal-queries.tsv")).unwrap();
    let expected = fs::read_to_string(root.join("safe-rust.tsv")).unwrap();
    let labels = fs::read_to_string(root.join("difference-labels.tsv")).unwrap();
    let differences: BTreeMap<_, _> = labels
        .lines()
        .map(|line| line.split_once('\t').unwrap())
        .collect();
    let native: Vec<_> = native.lines().collect();
    let expected: Vec<_> = expected.lines().collect();
    assert_eq!(native.len(), 10818);
    assert_eq!(expected.len(), native.len());
    assert_eq!(differences.len(), 550);
    let mut counts = BTreeMap::new();
    for (native, expected) in native.iter().zip(&expected) {
        let native: Vec<_> = native.split('\t').collect();
        let expected: Vec<_> = expected.split('\t').collect();
        assert_eq!(native.len(), 11);
        assert_eq!(expected.len(), 11);
        assert_eq!(native[0], expected[0]);
        let name = native[0];
        assert!(
            name.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
        let input = fs::read(root.join("inputs").join(format!("{name}.bin"))).unwrap();
        assert!(input.len() <= 32 * 1024 * 1024);
        let (first_status, first) = match frame_compressed_size(&input) {
            Ok(n) => ("ok", n),
            Err(_) => ("error", 0),
        };
        let (exact_status, exact) = match decompressed_size(&input) {
            Ok(Some(n)) => ("known", n),
            Ok(None) => ("unknown", 0),
            Err(_) => ("error", 0),
        };
        let (bound_status, bound) = match decompression_bound(&input) {
            Ok(n) => ("ok", n),
            Err(_) => ("error", 0),
        };
        let marker = is_skippable_frame(&input);
        let (skip_status, skip, variant) = if marker {
            match skippable_payload(&input) {
                Ok((v, p)) => {
                    assert_eq!(p.as_ptr(), input[8..].as_ptr());
                    ("ok", p.len(), v)
                }
                Err(_) => ("error", 0, 0),
            }
        } else {
            ("not-skip", 0, 0)
        };
        let actual = format!(
            "{name}\t{first_status}\t{first}\t{exact_status}\t{exact}\t{bound_status}\t{bound}\t{}\t{skip_status}\t{skip}\t{variant}",
            u8::from(marker)
        );
        assert_eq!(actual, expected.join("\t"), "{name}");
        if let Some(category) = differences.get(name) {
            assert_ne!(native, expected, "{name}");
            *counts.entry(*category).or_insert(0) += 1;
            match *category {
                "full-validation-after-unknown" => {
                    assert_eq!(native[3], "unknown");
                    assert_eq!(exact_status, "error");
                    assert_eq!(&native[1..3], &expected[1..3]);
                    assert_eq!(&native[5..], &expected[5..]);
                }
                "structural-window-cap" => {
                    assert_eq!(input[5], 255);
                    assert_eq!(native[1], "error");
                    assert_eq!(native[3], "error");
                    assert_eq!(native[5], "error");
                    assert_eq!(first_status, "ok");
                }
                "numeric-sentinel-or-checked-overflow" => {
                    assert!(
                        name.starts_with("known-")
                            || name.starts_with("sum-")
                            || name.starts_with("unknown-before-")
                    );
                }
                _ => panic!("unclassified difference"),
            }
        } else {
            assert_eq!(native, expected, "{name}");
        }
        if let Ok(info) = frame_info(&input) {
            assert_eq!(first, info.encoded_size());
            assert!(first > 0 && first <= input.len());
        }
    }
    assert_eq!(counts.get("full-validation-after-unknown"), Some(&519));
    assert_eq!(counts.get("structural-window-cap"), Some(&17));
    assert_eq!(
        counts.get("numeric-sentinel-or-checked-overflow"),
        Some(&14)
    );
}
#[test]
#[ignore = "requires original v1.5.7 test_decompressBound fixtures in ZSTDOXIDE_FRAME_QUERY_CASES"]
fn original_known_empty_stream_and_large_block_product_assertions() {
    let root = root().join("normal-originals");
    for name in [
        "original-known",
        "original-empty-unknown",
        "original-4g-bound",
    ] {
        let frame = fs::read(root.join(format!("{name}.zst"))).unwrap();
        let expected = fs::read(root.join(format!("{name}.raw"))).unwrap();
        assert_eq!(decompress(&frame, expected.len()).unwrap(), expected);
        assert_eq!(frame_compressed_size(&frame), Ok(frame.len()));
        match name {
            "original-known" => {
                assert_eq!(expected, b"abcd\0");
                assert_eq!(decompressed_size(&frame), Ok(Some(5)));
                assert_eq!(decompression_bound(&frame), Ok(5));
            }
            "original-empty-unknown" => {
                assert!(expected.is_empty());
                assert_eq!(decompressed_size(&frame), Ok(None));
                assert_eq!(decompression_bound(&frame), Ok(131072));
            }
            "original-4g-bound" => {
                assert_eq!(expected.len(), ((1 << 15) + 1) * 2);
                let (pairs, remainder) = expected.as_chunks::<2>();
                assert!(remainder.is_empty());
                assert!(pairs.iter().all(|pair| pair == b"a\0"));
                assert_eq!(decompressed_size(&frame), Ok(None));
                assert!(decompression_bound(&frame).unwrap() > 0x1_0000_0000);
            }
            _ => unreachable!(),
        }
    }
}
