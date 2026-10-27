//! External pinned utility observations; supplemental layouts use real guards.
#![forbid(unsafe_code)]
use std::{fs, path::PathBuf};
use zstdoxide::{Error, compression_bound, write_skippable_frame};
fn root() -> PathBuf {
    PathBuf::from(std::env::var_os("ZSTDOXIDE_FRAME_UTILITY_CASES").unwrap())
}
#[test]
#[ignore = "requires pinned normal/sanitized scalar observations in ZSTDOXIDE_FRAME_UTILITY_CASES"]
fn pinned_compression_bound_scalar_rows_match_typed_results() {
    let text = fs::read_to_string(root().join("normal-bounds.tsv")).unwrap();
    let mut count = 0;
    for line in text.lines() {
        let columns: Vec<_> = line.split('\t').collect();
        assert_eq!(columns.len(), 5);
        let size = columns[0].parse::<usize>().unwrap();
        let value = columns[2].parse::<usize>().unwrap();
        let macro_value = columns[3].parse::<usize>().unwrap();
        let raw_result = columns[4].parse::<usize>().unwrap();
        match columns[1] {
            "ok" => {
                assert_eq!(value, macro_value);
                assert_eq!(compression_bound(size), Ok(value));
            }
            "error" => {
                if macro_value == 0 {
                    assert_eq!(compression_bound(size), Err(Error::ResourceLimit));
                } else {
                    // A huge valid formula result overlaps C's encoded errors.
                    // The typed Rust result preserves the numeric bound.
                    assert_eq!(raw_result, macro_value);
                    assert_eq!(compression_bound(size), Ok(macro_value));
                    assert_eq!(size, (usize::MAX / 257) * 256 - 1);
                }
            }
            _ => panic!("invalid native verdict"),
        }
        count += 1;
    }
    assert_eq!(count, 280);
}
#[test]
#[ignore = "requires real pinned native writer layouts in ZSTDOXIDE_FRAME_UTILITY_CASES"]
fn pinned_real_writer_layouts_match_all_bytes_and_error_guards() {
    let root = root();
    let cases = fs::read_to_string(root.join("cases.tsv")).unwrap();
    let observed = fs::read_to_string(root.join("normal-write.tsv")).unwrap();
    assert_eq!(cases.lines().count(), observed.lines().count());
    let mut count = 0;
    for (case, row) in cases.lines().zip(observed.lines()) {
        let c: Vec<_> = case.split('\t').collect();
        let native: Vec<_> = row.split('\t').collect();
        assert_eq!(c.len(), 5);
        assert_eq!(native.len(), 4);
        assert_eq!(c[0], native[0]);
        let length = c[1].parse::<usize>().unwrap();
        let capacity = c[2].parse::<usize>().unwrap();
        let variant = c[3].parse::<u8>().unwrap();
        let seed = c[4].parse::<usize>().unwrap();
        let payload: Vec<_> = (0..length).map(|i| ((seed + i) % 256) as u8).collect();
        let mut storage = vec![0xa5; capacity + 32];
        let result = write_skippable_frame(&mut storage[16..16 + capacity], &payload, variant);
        let expected = if variant > 15 {
            Err(Error::InvalidParameter)
        } else if capacity < length + 8 {
            Err(Error::ResourceLimit)
        } else {
            Ok(length + 8)
        };
        assert_eq!(result, expected);
        assert_eq!(result.is_ok(), native[1] == "ok");
        if let Ok(written) = result {
            assert_eq!(written, native[2].parse::<usize>().unwrap());
            assert_eq!(&storage[24..16 + written], payload);
            assert!(storage[16 + written..].iter().all(|&b| b == 0xa5));
        } else {
            assert!(storage.iter().all(|&b| b == 0xa5));
        }
        assert!(storage[..16].iter().all(|&b| b == 0xa5));
        assert!(storage[16 + capacity..].iter().all(|&b| b == 0xa5));
        assert_eq!(
            storage,
            fs::read(root.join("normal-outputs").join(format!("{}.bin", c[0]))).unwrap()
        );
        count += 1;
    }
    assert_eq!(count, 343);
}
