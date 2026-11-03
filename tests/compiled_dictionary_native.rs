//! Supplementary guarded native schedules for compiled dictionary calls.
#![forbid(unsafe_code)]
use std::alloc::Global;
use std::fs;
use std::path::{Path, PathBuf};
use zstdoxide::{
    CompiledDictionary, CompressionParameterRequest, DecodeOptions, Dictionary, Error,
    compress_with_compiled, compress_with_parameters, decompress_with_dictionary_in,
};

const FORMATTED: &[u8] = include_bytes!("upstream/dictionaries/formatted.dict");
const RAW: &[u8] = include_bytes!("upstream/dictionaries/boundary-dictionary.raw");
const TEXT: &[u8] = include_bytes!("upstream/dictionaries/dictionary-treeless-literals.expected");

#[derive(Clone)]
struct Case {
    id: &'static str,
    level: i32,
    window: u8,
    dictionary: &'static str,
    input: &'static str,
    supported: bool,
    observable: bool,
    window_expired: bool,
}

fn schedule() -> Vec<Case> {
    vec![
        Case {
            id: "raw-fast",
            level: 1,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
            window_expired: false,
        },
        Case {
            id: "raw-double-fast",
            level: 3,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
            window_expired: false,
        },
        Case {
            id: "raw-chain",
            level: 4,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
            window_expired: false,
        },
        Case {
            id: "raw-tree",
            level: 9,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            supported: true,
            observable: true,
            window_expired: false,
        },
        Case {
            id: "prepared-fast",
            level: 1,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            supported: true,
            observable: false,
            window_expired: false,
        },
        Case {
            id: "prepared-chain",
            level: 4,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            supported: true,
            observable: false,
            window_expired: false,
        },
        Case {
            id: "prepared-tree",
            level: 9,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            supported: true,
            observable: false,
            window_expired: false,
        },
        Case {
            id: "raw-window",
            level: 4,
            window: 10,
            dictionary: "raw-a",
            input: "raw-first",
            supported: true,
            observable: false,
            window_expired: true,
        },
        Case {
            id: "prepared-window",
            level: 4,
            window: 10,
            dictionary: "prepared-a",
            input: "raw-first",
            supported: true,
            observable: false,
            window_expired: false,
        },
        Case {
            id: "raw-noise",
            level: 4,
            window: 0,
            dictionary: "raw-a",
            input: "noise",
            supported: true,
            observable: false,
            window_expired: false,
        },
        Case {
            id: "raw-large",
            level: 4,
            window: 0,
            dictionary: "raw-a",
            input: "large",
            supported: true,
            observable: true,
            window_expired: false,
        },
        Case {
            id: "optimal",
            level: 11,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            supported: false,
            observable: false,
            window_expired: false,
        },
        Case {
            id: "ultra2",
            level: 17,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            supported: false,
            observable: false,
            window_expired: false,
        },
    ]
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

fn request(case: &Case, input: &[u8]) -> CompressionParameterRequest {
    let mut request = CompressionParameterRequest::with_level(case.level);
    if case.window != 0 {
        request.set_window_log(case.window).unwrap();
    }
    request
        .set_source_hint(u32::try_from(input.len()).unwrap())
        .unwrap();
    request
}

#[test]
#[ignore = "requires ZSTDOXIDE_COMPILED_NATIVE and a pinned native oracle"]
fn compiled_dictionary_schedules_interoperate_with_guarded_native_controls() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_COMPILED_NATIVE").unwrap());
    let export = std::env::var_os("ZSTDOXIDE_COMPILED_EXPORT").is_some();
    let inputs = root.join("inputs");
    fs::create_dir_all(&inputs).unwrap();
    let cases = schedule();
    if export {
        fs::write(inputs.join("raw-a.dict"), RAW).unwrap();
        fs::write(inputs.join("prepared-a.dict"), FORMATTED).unwrap();
        fs::write(inputs.join("raw.input"), [RAW, RAW].concat()).unwrap();
        fs::write(inputs.join("json.input"), TEXT).unwrap();
        fs::write(inputs.join("raw-first.input"), &RAW[..1024]).unwrap();
        fs::write(
            inputs.join("noise.input"),
            (0..4096).map(|i| (i * 37 + 11) as u8).collect::<Vec<_>>(),
        )
        .unwrap();
        fs::write(inputs.join("large.input"), RAW.repeat(16)).unwrap();
        let mut steps = String::new();
        for case in &cases {
            steps.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                case.id, case.level, case.window, case.dictionary, case.input
            ));
            let input = fs::read(inputs.join(format!("{}.input", case.input))).unwrap();
            let dictionary = dictionary(case.dictionary, &inputs);
            let outcome =
                CompiledDictionary::new(dictionary.view(), &request(case, &input), Global);
            if case.supported {
                let compiled = outcome.expect(case.id);
                let frame =
                    compress_with_compiled(&input, &request(case, &input), &compiled).unwrap();
                fs::write(inputs.join(format!("{}.rust", case.id)), &frame).unwrap();
                if case.window_expired {
                    let plain = compress_with_parameters(&input, &request(case, &input)).unwrap();
                    assert_eq!(frame, plain, "{} must expire the dictionary", case.id);
                }
            } else {
                assert_eq!(
                    outcome.err(),
                    Some(Error::UnsupportedFeature),
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
            let outcome =
                CompiledDictionary::new(dictionary.view(), &request(case, &input), Global);
            assert_eq!(
                outcome.err(),
                Some(Error::UnsupportedFeature),
                "{}",
                case.id
            );
            assert_eq!(fields[5], "-1", "unsupported Rust case {}", case.id);
        }
    }
}
