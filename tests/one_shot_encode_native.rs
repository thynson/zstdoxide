//! Supplemental typed selection schedules, not original fuzzer history.
#![forbid(unsafe_code)]
use std::alloc::Global;
use std::{fs, path::PathBuf};
use zstdoxide::{
    DecodeOptions, Dictionary, Error, OneShotEncoder, SharedDictionary, compress,
    compress_with_dictionary_in, compression_bound, decompress, decompress_with_dictionary_in,
};

const PREPARED: &[u8] = include_bytes!("upstream/dictionaries/formatted.dict");
const DICTS: [&str; 6] = [
    "raw-a",
    "raw-b",
    "prepared-a",
    "prepared-b",
    "rawmagic-a",
    "rawmagic-b",
];
fn bytes(mut state: u32) -> Vec<u8> {
    (0..1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}
fn dictionary(name: &str, bytes: &[u8]) -> SharedDictionary {
    if name.starts_with("prepared-") {
        Dictionary::from_bytes(bytes)
    } else {
        Dictionary::raw(bytes)
    }
    .unwrap()
    .into_shared()
}
#[derive(Clone)]
struct Step {
    id: String,
    scenario: String,
    op: &'static str,
    dictionary: String,
    input: String,
    capacity: &'static str,
    expected_mode: &'static str,
    expected_dictionary: String,
}
fn schedule() -> Vec<Step> {
    let mut all = Vec::new();
    for kind in ["raw", "prepared", "rawmagic"] {
        let a = format!("{kind}-a");
        let b = format!("{kind}-b");
        for scenario in [
            "persistent",
            "prefix",
            "prefix-short",
            "prefix-empty",
            "persistent-short",
            "prefix-to-persistent",
            "persistent-to-prefix",
            "clear-prefix",
            "clear-persistent",
        ] {
            let scenario_id = format!("{kind}-{scenario}");
            let mut current: Option<String> = None;
            let mut pending = false;
            let mut index = 0;
            let mut append = |op, dict: &str, input: &str, capacity| {
                let (mode, expected) = if op == "encode" {
                    (
                        if current.is_none() {
                            "none"
                        } else if pending {
                            "prefix"
                        } else {
                            "persistent"
                        },
                        current.clone().unwrap_or_else(|| "-".to_owned()),
                    )
                } else {
                    ("-", "-".to_owned())
                };
                all.push(Step {
                    id: format!("{scenario_id}-{index}"),
                    scenario: scenario_id.clone(),
                    op,
                    dictionary: dict.to_owned(),
                    input: input.to_owned(),
                    capacity,
                    expected_mode: mode,
                    expected_dictionary: expected,
                });
                index += 1;
                match op {
                    "persistent" => {
                        current = Some(dict.to_owned());
                        pending = false
                    }
                    "prefix" => {
                        current = Some(dict.to_owned());
                        pending = true
                    }
                    "clear" => {
                        current = None;
                        pending = false
                    }
                    "encode" if pending => {
                        current = None;
                        pending = false
                    }
                    "encode" => {}
                    _ => unreachable!(),
                }
            };
            append(
                if scenario.starts_with("persistent") || scenario == "clear-persistent" {
                    "persistent"
                } else {
                    "prefix"
                },
                &a,
                "-",
                "-",
            );
            match scenario {
                "prefix-to-persistent" => append("persistent", &b, "-", "-"),
                "persistent-to-prefix" => append("prefix", &b, "-", "-"),
                "clear-prefix" | "clear-persistent" => append("clear", "-", "-", "-"),
                _ => {}
            }
            append(
                "encode",
                "-",
                if scenario == "prefix-empty" {
                    "empty"
                } else {
                    kind
                },
                if scenario.ends_with("short") {
                    "zero"
                } else {
                    "bound"
                },
            );
            if !scenario.starts_with("clear-") {
                append("encode", "-", kind, "bound");
            }
        }
    }
    assert_eq!(all.len(), 87);
    assert_eq!(all.iter().filter(|s| s.op == "encode").count(), 48);
    all
}

#[test]
#[ignore = "requires ZSTDOXIDE_ONE_SHOT_ENCODE_NATIVE and a pinned native oracle"]
fn typed_context_schedules_preserve_attempt_ownership_and_complete_frames() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_ONE_SHOT_ENCODE_NATIVE").unwrap());
    let export = std::env::var_os("ZSTDOXIDE_ONE_SHOT_ENCODE_EXPORT").is_some();
    let inputs = root.join("inputs");
    fs::create_dir_all(&inputs).unwrap();
    let steps = schedule();
    if export {
        for (i, name) in DICTS.iter().enumerate() {
            let mut data = if name.starts_with("prepared-") {
                PREPARED.to_vec()
            } else {
                bytes(if i % 2 == 0 { 0x1234_5678 } else { 0x9abc_def0 })
            };
            if name.starts_with("prepared-") {
                data[4..8].copy_from_slice(
                    &(if i % 2 == 0 {
                        0x0102_0304_u32
                    } else {
                        0xa0b0_c0d0_u32
                    })
                    .to_le_bytes(),
                );
            }
            if name.starts_with("rawmagic-") {
                data[..4].copy_from_slice(&0xec30_a437_u32.to_le_bytes());
            }
            fs::write(inputs.join(format!("{name}.dict")), data).unwrap();
        }
        for kind in ["raw", "prepared", "rawmagic"] {
            let a = fs::read(inputs.join(format!("{kind}-a.dict"))).unwrap();
            let b = fs::read(inputs.join(format!("{kind}-b.dict"))).unwrap();
            let a = dictionary(&format!("{kind}-a"), &a);
            let b = dictionary(&format!("{kind}-b"), &b);
            let mut source = a.content().to_vec();
            source.extend_from_slice(b.content());
            let source = source.repeat(2);
            fs::write(inputs.join(format!("{kind}.input")), source).unwrap();
        }
        fs::write(inputs.join("empty.input"), []).unwrap();
        let text = steps
            .iter()
            .map(|s| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    s.id,
                    s.scenario,
                    s.op,
                    s.dictionary,
                    s.input,
                    s.capacity,
                    s.expected_mode,
                    s.expected_dictionary
                )
            })
            .collect::<String>();
        fs::write(root.join("steps.tsv"), text).unwrap();
    }
    let native = if export {
        String::new()
    } else {
        fs::read_to_string(root.join("normal.tsv")).unwrap()
    };
    let native_rows: Vec<_> = native.lines().collect();
    if !export {
        assert_eq!(native_rows.len(), 87);
    }
    let owners: Vec<_> = DICTS
        .iter()
        .map(|name| {
            let bytes = fs::read(inputs.join(format!("{name}.dict"))).unwrap();
            (name, dictionary(name, &bytes), bytes)
        })
        .collect();
    let mut previous = String::new();
    let mut encoder = OneShotEncoder::new();
    let mut attempts = 0;
    let mut successes = 0;
    let mut native_differences = 0;
    for (index, step) in steps.iter().enumerate() {
        if previous != step.scenario {
            encoder = OneShotEncoder::new();
            previous = step.scenario.clone();
        }
        let native: Vec<_> = if export {
            Vec::new()
        } else {
            native_rows[index].split('\t').collect()
        };
        if !export {
            assert_eq!(native.len(), 8);
            assert_eq!(native[0], step.id);
        }
        if step.op != "encode" {
            match step.op {
                "clear" => encoder.set_dictionary(None),
                "prefix" | "persistent" => {
                    let d = &owners
                        .iter()
                        .find(|(n, _, _)| **n == step.dictionary)
                        .unwrap()
                        .1;
                    if step.op == "prefix" {
                        encoder.set_prefix(Some(d.clone()))
                    } else {
                        encoder.set_dictionary(Some(d.clone()))
                    }
                }
                _ => unreachable!(),
            }
            if !export {
                assert_eq!(native[1], "setup");
            }
            continue;
        }
        attempts += 1;
        let input = fs::read(inputs.join(format!("{}.input", step.input))).unwrap();
        let before = input.clone();
        let selected = owners
            .iter()
            .find(|(n, _, _)| **n == step.expected_dictionary)
            .map(|(_, d, _)| d);
        assert_eq!(encoder.dictionary().is_some(), selected.is_some());
        assert_eq!(encoder.prefix_pending(), step.expected_mode == "prefix");
        let expected = match selected {
            Some(d) => compress_with_dictionary_in(&input, &d.view(), Global).unwrap(),
            None => compress(&input).unwrap(),
        };
        let cap = if step.capacity == "zero" {
            0
        } else {
            compression_bound(input.len()).unwrap()
        };
        let mut storage = vec![0xa5; cap + 32];
        let result = encoder.compress_into(&input, &mut storage[16..16 + cap]);
        if cap == 0 {
            assert_eq!(result, Err(Error::ResourceLimit));
        } else {
            let n = result.unwrap();
            successes += 1;
            assert_eq!(&storage[16..16 + n], expected);
            assert!(storage[16 + n..].iter().all(|&b| b == 0xa5));
            if export {
                fs::write(
                    inputs.join(format!("{}.rust", step.id)),
                    &storage[16..16 + n],
                )
                .unwrap();
            }
        }
        assert!(
            storage[..16]
                .iter()
                .chain(&storage[16 + cap..])
                .all(|&b| b == 0xa5)
        );
        assert_eq!(input, before);
        assert!(!encoder.prefix_pending());
        assert_eq!(
            encoder.dictionary().is_some(),
            step.expected_mode == "persistent"
        );
        for (_, owner, bytes) in &owners {
            assert_eq!(owner.as_bytes(), bytes);
            let retained = encoder
                .dictionary()
                .is_some_and(|d| core::ptr::eq(d.as_bytes().as_ptr(), owner.as_bytes().as_ptr()));
            assert_eq!(
                SharedDictionary::strong_count(owner),
                if retained { 2 } else { 1 }
            );
        }
        if !export {
            assert_eq!(
                native[1] == "ok",
                result.is_ok(),
                "{} {:?}",
                step.id,
                native
            );
            let native_storage = fs::read(
                root.join("normal-outputs")
                    .join(format!("{}.storage", step.id)),
            )
            .unwrap();
            assert_eq!(native_storage.len(), cap + 32);
            assert!(
                native_storage[..16]
                    .iter()
                    .chain(&native_storage[16 + cap..])
                    .all(|&b| b == 0xa5)
            );
            if result.is_ok() {
                assert_eq!(native[3], "0");
                let native_size = native[2].parse::<usize>().unwrap();
                assert!(native_size <= cap);
                assert_eq!(
                    native_storage[16 + native_size..16 + cap]
                        .iter()
                        .filter(|&&b| b != 0xa5)
                        .count(),
                    native[4].parse::<usize>().unwrap()
                );
                let frame =
                    fs::read(root.join("normal-outputs").join(format!("{}.zst", step.id))).unwrap();
                assert_eq!(frame.len(), native[2].parse::<usize>().unwrap());
                let decoder = native[7].parse::<usize>().unwrap();
                let decoded = if decoder == 0 {
                    decompress(&frame, input.len()).unwrap()
                } else {
                    decompress_with_dictionary_in(
                        &frame,
                        &owners[decoder - 1].1.view(),
                        DecodeOptions::new(input.len()),
                        Global,
                    )
                    .unwrap()
                };
                assert_eq!(decoded, input);
                if native[5] != "1" {
                    native_differences += 1;
                }
            }
        }
    }
    assert_eq!(attempts, 48);
    assert_eq!(successes, 42);
    println!("OBS\t27\t87\t{attempts}\t{successes}\t{native_differences}");
}
