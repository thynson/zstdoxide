//! Replay independently captured pinned native destination cases.
#![forbid(unsafe_code)]

use std::{fs, path::PathBuf};
use zstdoxide::{Error, decompress_into};

#[test]
#[ignore = "requires ZSTDOXIDE_DESTINATION_CASES from the pinned native campaign"]
fn pinned_native_destinations() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_DESTINATION_CASES").unwrap());
    let manifest = fs::read_to_string(root.join("manifest.tsv")).unwrap();
    let mut count = 0;
    for row in manifest.lines() {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 3);
        let name = fields[0];
        assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'));
        let capacity: usize = fields[1].parse().unwrap();
        assert!(capacity <= 16 * 1024 * 1024);
        let frame = fs::read(root.join(format!("{name}.zst"))).unwrap();
        let expected = fs::read(root.join(format!("{name}.raw"))).unwrap();
        let mut output = vec![0xa5; capacity + 64];
        let result = decompress_into(&frame, &mut output[32..32 + capacity]);
        match fields[2] {
            "ok" => {
                assert_eq!(result, Ok(expected.len()), "{name}, capacity {capacity}");
                assert_eq!(&output[32..32 + expected.len()], expected, "{name}");
                assert!(output[32 + expected.len()..].iter().all(|b| *b == 0xa5));
            }
            "small" => assert_eq!(result, Err(Error::ResourceLimit), "{name}"),
            "error" => assert!(result.is_err(), "{name}"),
            other => panic!("unrecognized expectation {other}"),
        }
        assert!(output[..32].iter().all(|b| *b == 0xa5), "{name}");
        assert!(output[32 + capacity..].iter().all(|b| *b == 0xa5), "{name}");
        count += 1;
    }
    assert!(count > 0);
    eprintln!("checked {count} pinned native destination cases");
}
