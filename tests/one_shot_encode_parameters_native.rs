//! Supplementary guarded native schedules for parameterized dictionary calls.
//!
//! These typed cases supplement the unit and public integration suites; they
//! are not original fuzzer history. Native compressed bytes, capacity
//! precedence and workspace behavior are not Rust contracts.
#![forbid(unsafe_code)]
use std::alloc::Global;
use std::fs;
use std::path::{Path, PathBuf};
use zstdoxide::{
    CompressionParameterRequest, DecodeOptions, Dictionary, Error, ReferenceStrategy,
    compress_with_parameters_and_dictionary, decompress_with_dictionary_in,
};

const FORMATTED: &[u8] = include_bytes!("upstream/dictionaries/formatted.dict");
const RAW: &[u8] = include_bytes!("upstream/dictionaries/boundary-dictionary.raw");
const TEXT: &[u8] = include_bytes!("upstream/dictionaries/dictionary-treeless-literals.expected");

#[derive(Clone)]
struct Case {
    id: &'static str,
    level: i32,
    strategy: u32,
    window: u8,
    dictionary: &'static str,
    input: &'static str,
    supported: bool,
    observable: bool,
}

fn schedule() -> Vec<Case> {
    let cases = [
        Case {
            id: "raw-fast",
            level: 1,
            strategy: 0,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
        },
        Case {
            id: "raw-double-fast",
            level: 0,
            strategy: 0,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
        },
        Case {
            id: "raw-greedy",
            level: 4,
            strategy: 0,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
        },
        Case {
            id: "raw-lazy",
            level: 5,
            strategy: 0,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
        },
        Case {
            id: "raw-lazy2",
            level: 6,
            strategy: 0,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
        },
        Case {
            id: "raw-tree",
            level: 9,
            strategy: 0,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
        },
        Case {
            id: "prepared-fast",
            level: 1,
            strategy: 0,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            supported: true,
            observable: false,
        },
        Case {
            id: "prepared-greedy",
            level: 4,
            strategy: 0,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            supported: true,
            observable: false,
        },
        Case {
            id: "prepared-tree",
            level: 9,
            strategy: 0,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            supported: true,
            observable: false,
        },
        Case {
            id: "raw-window",
            level: 4,
            strategy: 0,
            window: 11,
            dictionary: "raw-a",
            input: "raw-far",
            supported: true,
            observable: false,
        },
        Case {
            id: "prepared-window",
            level: 4,
            strategy: 0,
            window: 11,
            dictionary: "prepared-a",
            input: "raw-far",
            supported: true,
            observable: false,
        },
        Case {
            id: "raw-empty",
            level: 4,
            strategy: 0,
            window: 0,
            dictionary: "raw-a",
            input: "empty",
            supported: true,
            observable: false,
        },
        Case {
            id: "raw-noise",
            level: 4,
            strategy: 0,
            window: 0,
            dictionary: "raw-a",
            input: "noise",
            supported: true,
            observable: false,
        },
        Case {
            id: "raw-large",
            level: 4,
            strategy: 0,
            window: 0,
            dictionary: "raw-a",
            input: "large",
            supported: true,
            observable: true,
        },
        Case {
            id: "prepared-large",
            level: 4,
            strategy: 0,
            window: 0,
            dictionary: "prepared-a",
            input: "large",
            supported: true,
            observable: true,
        },
        Case {
            id: "optimal-fresh",
            level: 11,
            strategy: 7,
            window: 16,
            dictionary: "prepared-a",
            input: "json",
            supported: false,
            observable: false,
        },
        Case {
            id: "ultra-fresh",
            level: 11,
            strategy: 8,
            window: 16,
            dictionary: "prepared-a",
            input: "json",
            supported: false,
            observable: false,
        },
        Case {
            id: "ultra2-fresh",
            level: 11,
            strategy: 9,
            window: 16,
            dictionary: "prepared-a",
            input: "json",
            supported: false,
            observable: false,
        },
        Case {
            id: "optimal-retained",
            level: 11,
            strategy: 7,
            window: 16,
            dictionary: "raw-a",
            input: "retained",
            supported: false,
            observable: false,
        },
        Case {
            id: "ultra2-retained",
            level: 11,
            strategy: 9,
            window: 16,
            dictionary: "raw-a",
            input: "retained",
            supported: false,
            observable: false,
        },
    ];
    assert_eq!(cases.len(), 20);
    cases.to_vec()
}

fn dictionary(name: &str, inputs: &Path) -> zstdoxide::SharedDictionary {
    let bytes = fs::read(inputs.join(format!("{name}.dict"))).unwrap();
    if name == "prepared-a" {
        Dictionary::from_bytes(&bytes)
    } else {
        Dictionary::raw(&bytes)
    }
    .unwrap()
    .into_shared()
}

fn request(case: &Case) -> CompressionParameterRequest {
    let mut request = CompressionParameterRequest::with_level(case.level);
    if case.strategy != 0 {
        request.set_strategy(Some(ReferenceStrategy::try_from(case.strategy).unwrap()));
    }
    if case.window != 0 {
        request.set_window_log(case.window).unwrap();
    }
    if !case.supported {
        // Match the public unsupported-profile integration caps so both fresh
        // and retained optimal-family plans are admitted for rejection checks.
        request.set_hash_log(12).unwrap();
        request.set_chain_log(12).unwrap();
        request.set_search_log(6).unwrap();
        request.set_min_match(3).unwrap();
        request.set_target_length(1).unwrap();
    }
    request
}

#[test]
#[ignore = "requires ZSTDOXIDE_ONE_SHOT_ENCODE_PARAMETERS_NATIVE and a pinned native oracle"]
fn parameterized_dictionary_schedules_interoperate_with_guarded_native_controls() {
    let root =
        PathBuf::from(std::env::var_os("ZSTDOXIDE_ONE_SHOT_ENCODE_PARAMETERS_NATIVE").unwrap());
    let export = std::env::var_os("ZSTDOXIDE_ONE_SHOT_ENCODE_PARAMETERS_EXPORT").is_some();
    let inputs = root.join("inputs");
    fs::create_dir_all(&inputs).unwrap();
    let cases = schedule();
    if export {
        fs::write(inputs.join("raw-a.dict"), RAW).unwrap();
        fs::write(inputs.join("prepared-a.dict"), FORMATTED).unwrap();
        fs::write(inputs.join("raw.input"), [RAW, RAW].concat()).unwrap();
        fs::write(inputs.join("json.input"), TEXT).unwrap();
        fs::write(inputs.join("raw-far.input"), &RAW[..1024]).unwrap();
        fs::write(inputs.join("empty.input"), []).unwrap();
        fs::write(
            inputs.join("noise.input"),
            (0..1024).map(|i| (i * 37 + 11) as u8).collect::<Vec<_>>(),
        )
        .unwrap();
        fs::write(inputs.join("large.input"), RAW.repeat(32)).unwrap();
        fs::write(inputs.join("retained.input"), RAW.repeat(10)).unwrap();
        let mut steps = String::new();
        for case in &cases {
            steps.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                case.id, case.level, case.strategy, case.window, case.dictionary, case.input
            ));
            let input = fs::read(inputs.join(format!("{}.input", case.input))).unwrap();
            let dictionary = dictionary(case.dictionary, &inputs);
            let result =
                compress_with_parameters_and_dictionary(&input, &request(case), &dictionary);
            if case.supported {
                let frame = result.expect(case.id);
                fs::write(inputs.join(format!("{}.rust", case.id)), frame).unwrap();
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    Error::UnsupportedFeature,
                    "{}",
                    case.id
                );
            }
        }
        fs::write(root.join("steps.tsv"), steps).unwrap();
        return;
    }
    let native = fs::read_to_string(root.join("native.tsv")).unwrap();
    let rows: Vec<_> = native.lines().collect();
    assert_eq!(rows.len(), cases.len());
    for (case, row) in cases.iter().zip(&rows) {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 8, "{}", case.id);
        assert_eq!(fields[0], case.id);
        assert_eq!(fields[1], "ok", "{}", case.id);
        assert_eq!(fields[7], "1", "native roundtrip {}", case.id);
        let input = fs::read(inputs.join(format!("{}.input", case.input))).unwrap();
        let dictionary = dictionary(case.dictionary, &inputs);
        let native_frame = fs::read(root.join("outputs").join(format!("{}.zst", case.id))).unwrap();
        assert_eq!(
            decompress_with_dictionary_in(
                &native_frame,
                &dictionary.view(),
                DecodeOptions::new(input.len()),
                Global,
            )
            .unwrap(),
            input,
            "{}",
            case.id
        );
        if case.supported {
            assert_eq!(fields[5], "1", "native decode of Rust frame {}", case.id);
            if case.observable {
                assert_eq!(fields[6], "1", "native dictionary control {}", case.id);
            }
        } else {
            let result =
                compress_with_parameters_and_dictionary(&input, &request(case), &dictionary);
            assert_eq!(
                result.unwrap_err(),
                Error::UnsupportedFeature,
                "{}",
                case.id
            );
            assert_eq!(fields[5], "-1", "unsupported Rust case {}", case.id);
        }
    }
}
