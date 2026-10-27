//! Supplemental native context schedules with observable subsequent-call state.
#![forbid(unsafe_code)]
use std::{fs, path::PathBuf};
use zstdoxide::{DecodeOptions, Dictionary, Error, OneShotDecoder};
#[test]
#[ignore = "requires native schedules in ZSTDOXIDE_ONE_SHOT_SELECTION_CASES"]
fn pinned_context_schedules_preserve_call_consumption_and_complete_output() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_ONE_SHOT_SELECTION_CASES").unwrap());
    let text = fs::read_to_string(root.join("steps.tsv")).unwrap();
    let steps: Vec<_> = text.lines().collect();
    assert_eq!(steps.len(), 81);
    let observed = fs::read_to_string(root.join("normal.tsv")).unwrap();
    let rows: Vec<_> = observed.lines().collect();
    assert_eq!(rows.len(), 162);
    let mut differences = 0;
    let mut decodes = 0;
    for mode in 0..2 {
        let mut previous = "";
        let mut decoder = OneShotDecoder::new(DecodeOptions::new(0));
        let mut retained = false;
        let mut prefix = false;
        for (index, line) in steps.iter().enumerate() {
            let c: Vec<_> = line.split('\t').collect();
            assert_eq!(c.len(), 7);
            let native: Vec<_> = rows[mode * steps.len() + index].split('\t').collect();
            assert_eq!(native.len(), 5);
            assert_eq!(c[0], native[0]);
            assert_eq!(mode, native[1].parse::<usize>().unwrap());
            if previous != c[1] {
                decoder = OneShotDecoder::new(DecodeOptions::new(0));
                previous = c[1];
                retained = false;
                prefix = false;
            }
            if c[2] != "decode" {
                assert_eq!(native[2], "setup");
                if c[2] == "clear" {
                    decoder.set_dictionary(None);
                    retained = false;
                    prefix = false;
                } else {
                    let bytes =
                        fs::read(root.join("inputs").join(format!("{}.dict", c[3]))).unwrap();
                    let dictionary = if c[2].ends_with("raw") {
                        Dictionary::raw(&bytes)
                    } else {
                        Dictionary::from_bytes(&bytes)
                    }
                    .unwrap()
                    .into_shared();
                    if c[2].starts_with("prefix-") {
                        decoder.set_prefix(Some(dictionary));
                        prefix = true;
                    } else {
                        decoder.set_dictionary(Some(dictionary));
                        prefix = false;
                    }
                    retained = true;
                }
            } else {
                let frame = fs::read(root.join("inputs").join(format!("{}.zst", c[4]))).unwrap();
                let limit = c[5].parse::<usize>().unwrap();
                let capacity = c[6].parse::<usize>().unwrap();
                assert!(frame.len() <= capacity && limit <= capacity);
                let mut storage = vec![0xa5; capacity + 32];
                let start = capacity - frame.len();
                if mode == 1 {
                    storage[16 + start..16 + capacity].copy_from_slice(&frame);
                }
                let before = storage.clone();
                decoder.set_options(DecodeOptions::new(limit));
                let result = if mode == 0 {
                    decoder.decompress_into(&frame, &mut storage[16..16 + capacity])
                } else {
                    decoder.decompress_in_place(&mut storage[16..16 + capacity], start)
                };
                if prefix {
                    retained = false;
                    prefix = false;
                }
                assert_eq!(&storage[..16], &before[..16]);
                assert_eq!(&storage[16 + limit..], &before[16 + limit..]);
                if frame.is_empty() {
                    assert_eq!(native[2], "ok");
                    assert_eq!(native[3], "0");
                    assert_eq!(result, Err(Error::UnexpectedEof));
                    differences += 1;
                } else {
                    assert_eq!(
                        result.is_ok(),
                        native[2] == "ok",
                        "{} mode{mode} {result:?} {native:?}",
                        c[0]
                    );
                }
                if let Ok(n) = result {
                    assert_eq!(n, native[3].parse::<usize>().unwrap());
                    assert_eq!(&storage[16 + n..], &before[16 + n..]);
                    let expected =
                        fs::read(root.join("inputs").join(format!("{}.expected", c[4]))).unwrap();
                    assert_eq!(&storage[16..16 + n], expected);
                    let native_bytes = fs::read(
                        root.join("normal-outputs")
                            .join(format!("{}-{mode}.bin", c[0])),
                    )
                    .unwrap();
                    assert_eq!(&storage[16..16 + n], native_bytes);
                }
                println!("OBS\t{}\t{mode}\t{result:?}\t{retained}\t{prefix}", c[0]);
                decodes += 1;
            }
            assert_eq!(decoder.dictionary().is_some(), retained);
            assert_eq!(decoder.prefix_pending(), prefix);
        }
    }
    assert_eq!(decodes, 106);
    assert_eq!(differences, 4);
}
