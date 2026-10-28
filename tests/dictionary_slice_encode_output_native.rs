#![cfg(feature = "std")]
use std::{fs, path::PathBuf};
use zstdoxide::{
    DecodeOptions, Dictionary, Error, SharedDictionary, compress_into_with_dictionary,
    compress_with_dictionary, compression_bound, decompress_with_dictionary,
};

fn dictionaries() -> Vec<(String, SharedDictionary)> {
    let mut dictionaries = vec![
        ("empty".into(), Dictionary::raw(b"").unwrap().into_shared()),
        (
            "raw".into(),
            Dictionary::raw(b"a useful dictionary pattern for bounded output ")
                .unwrap()
                .into_shared(),
        ),
    ];
    for id in [0, 1, 255, 256, 65_535, 65_536, u32::MAX] {
        let mut bytes = include_bytes!("upstream/dictionaries/formatted.dict").to_vec();
        bytes[4..8].copy_from_slice(&id.to_le_bytes());
        dictionaries.push((
            format!("prepared-{id}"),
            Dictionary::from_bytes(&bytes).unwrap().into_shared(),
        ));
    }
    dictionaries
}

fn inputs(dictionary: &SharedDictionary) -> Vec<(String, Vec<u8>)> {
    let mut cases = Vec::new();
    for length in [0, 1, 255, 256, 131_073] {
        for kind in 0..2 {
            let mut seed = 0x6a09e667u32;
            let input = (0..length)
                .map(|i| {
                    if kind == 0 {
                        let content = dictionary.content();
                        if content.is_empty() {
                            b"pattern "[i % 8]
                        } else {
                            content[i % content.len()]
                        }
                    } else {
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
#[ignore = "exports deterministic dictionary caller-output frames for pinned native capture"]
fn export_dictionary_slice_frames() {
    let root = PathBuf::from(
        std::env::var_os("ZSTDOXIDE_DICTIONARY_SLICE_ENCODE_EXPORT").expect("export directory"),
    );
    fs::create_dir_all(&root).unwrap();
    let mut names = String::new();
    for (kind, dictionary) in dictionaries() {
        for (case, input) in inputs(&dictionary) {
            let name = format!("{kind}-{case}");
            let expected = compress_with_dictionary(&input, &dictionary).unwrap();
            let mut output = vec![0xa5; expected.len() + 32];
            assert_eq!(
                compress_into_with_dictionary(
                    &input,
                    &mut output[16..16 + expected.len()],
                    &dictionary
                ),
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
            fs::write(root.join(format!("{name}.dict")), dictionary.as_bytes()).unwrap();
            names.push_str(&name);
            names.push('\n');
        }
    }
    fs::write(root.join("cases.txt"), names).unwrap();
}

#[test]
#[ignore = "requires normal and sanitized pinned native dictionary compression captures"]
fn replay_dictionary_slice_frames_and_capacity_outputs() {
    let root = PathBuf::from(
        std::env::var_os("ZSTDOXIDE_DICTIONARY_SLICE_ENCODE_CASES").expect("capture directory"),
    );
    let rows = fs::read_to_string(root.join("normal.tsv")).unwrap();
    for (kind, dictionary) in dictionaries() {
        let before = dictionary.as_bytes().to_vec();
        for (case, input) in inputs(&dictionary) {
            let name = format!("{kind}-{case}");
            assert_eq!(
                fs::read(root.join("inputs").join(format!("{name}.input"))).unwrap(),
                input
            );
            assert_eq!(
                fs::read(root.join("inputs").join(format!("{name}.dict"))).unwrap(),
                before
            );
            let expected = fs::read(root.join("inputs").join(format!("{name}.rust"))).unwrap();
            let native = fs::read(root.join("normal-outputs").join(format!("{name}.zst"))).unwrap();
            assert_eq!(
                decompress_with_dictionary(&native, &dictionary, DecodeOptions::new(input.len()))
                    .unwrap(),
                input
            );
            let mut observed = 0;
            for row in rows.lines() {
                let fields: Vec<_> = row.split('\t').collect();
                assert_eq!(fields.len(), 7);
                if fields[0] != name {
                    continue;
                }
                observed += 1;
                if fields[3] == "ok" {
                    let native = fs::read(
                        root.join("normal-outputs")
                            .join(format!("{name}-cap{}.zst", fields[1])),
                    )
                    .unwrap();
                    assert_eq!(native.len(), fields[4].parse::<usize>().unwrap());
                    assert_eq!(
                        decompress_with_dictionary(
                            &native,
                            &dictionary,
                            DecodeOptions::new(input.len())
                        )
                        .unwrap(),
                        input
                    );
                }
            }
            assert_eq!(observed, 9);
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
                let result = compress_into_with_dictionary(
                    &input,
                    &mut output[16..16 + capacity],
                    &dictionary,
                );
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
                assert_eq!(dictionary.as_bytes(), before);
                assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
                println!("OBS\t{name}\t{capacity}\t{result:?}");
            }
        }
    }
}
