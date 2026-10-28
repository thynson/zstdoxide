#![cfg(feature = "std")]
use std::{fs, path::PathBuf};
use zstdoxide::{Error, compress, compress_into, compression_bound, decompress};

fn cases() -> Vec<(String, Vec<u8>)> {
    let mut cases = Vec::new();
    for length in [
        0, 1, 5, 6, 255, 256, 65_791, 65_792, 131_071, 131_072, 131_073, 393_233,
    ] {
        for kind in 0..3 {
            let mut seed = 0x6a09e667u32;
            let input = (0..length)
                .map(|i| match kind {
                    0 => 0x5a,
                    1 => b"repeated meaningful phrase with a deterministic distant suffix"[i % 62],
                    _ => {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        seed as u8
                    }
                })
                .collect();
            cases.push((format!("{length}-{kind}"), input));
        }
    }
    cases
}

#[test]
#[ignore = "exports deterministic caller-owned frames for an external pinned native oracle"]
fn export_slice_frames_for_native_oracle() {
    let root =
        PathBuf::from(std::env::var_os("ZSTDOXIDE_SLICE_ENCODE_EXPORT").expect("export directory"));
    fs::create_dir_all(&root).unwrap();
    let mut names = String::new();
    for (name, input) in cases() {
        let expected = compress(&input).unwrap();
        let mut output = vec![0xa5; expected.len() + 32];
        assert_eq!(
            compress_into(&input, &mut output[16..16 + expected.len()]),
            Ok(expected.len())
        );
        assert_eq!(&output[16..16 + expected.len()], expected);
        assert!(
            output[..16]
                .iter()
                .chain(&output[16 + expected.len()..])
                .all(|&b| b == 0xa5)
        );
        fs::write(root.join(format!("{name}.input")), input).unwrap();
        fs::write(root.join(format!("{name}.rust")), expected).unwrap();
        names.push_str(&name);
        names.push('\n');
    }
    fs::write(root.join("cases.txt"), names).unwrap();
}

#[test]
#[ignore = "requires normal and sanitized v1.5.7 bounded-compression capture"]
fn replay_slice_frames_and_native_capacity_observations() {
    let root =
        PathBuf::from(std::env::var_os("ZSTDOXIDE_SLICE_ENCODE_CASES").expect("capture directory"));
    for (name, input) in cases() {
        assert_eq!(
            fs::read(root.join("inputs").join(format!("{name}.input"))).unwrap(),
            input
        );
        let expected = fs::read(root.join("inputs").join(format!("{name}.rust"))).unwrap();
        let native = fs::read(root.join("normal-outputs").join(format!("{name}.zst"))).unwrap();
        assert_eq!(decompress(&native, input.len()).unwrap(), input);
        let mut capacities = vec![
            0,
            1,
            5,
            6,
            9,
            expected.len() - 1,
            expected.len(),
            expected.len() + 7,
            compression_bound(input.len()).unwrap(),
        ];
        capacities.sort_unstable();
        capacities.dedup();
        for capacity in capacities {
            let mut output = vec![0xa5; capacity + 32];
            let result = compress_into(&input, &mut output[16..16 + capacity]);
            if capacity < expected.len() {
                assert_eq!(result, Err(Error::ResourceLimit));
            } else {
                assert_eq!(result, Ok(expected.len()));
                assert_eq!(&output[16..16 + expected.len()], expected);
                assert!(output[16 + expected.len()..].iter().all(|&b| b == 0xa5));
            }
            assert!(
                output[..16]
                    .iter()
                    .chain(&output[16 + capacity..])
                    .all(|&b| b == 0xa5)
            );
            println!("OBS\t{name}\t{capacity}\t{result:?}");
        }
    }
}
