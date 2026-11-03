//! Supplementary guarded native schedules for streaming compiled frames.
//!
//! The ignored test exports Rust streaming frames and inputs for an external
//! pinned v1.5.7 `ZSTD_compressStream2` + `ZSTD_CCtx_refCDict` oracle, then
//! replays the native frames. Native byte identity and workspace identity are
//! not required; each frame must decode with the dictionary on both sides.
#![forbid(unsafe_code)]
use std::alloc::Global;
use std::fs;
use std::path::PathBuf;
use zstdoxide::{
    CompiledDictionary, CompiledStreamEncoder, CompressionParameterRequest, DecodeOptions,
    Dictionary, EndDirective, Error, Progress, StreamStatus, decompress_with_dictionary_in,
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
    chunk: usize,
    output: usize,
    frames: usize,
    supported: bool,
}

fn schedule() -> Vec<Case> {
    vec![
        Case {
            id: "stream-raw-fast",
            level: 1,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            chunk: 137,
            output: 1,
            frames: 1,
            supported: true,
        },
        Case {
            id: "stream-raw-double-fast",
            level: 3,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            chunk: 501,
            output: 3,
            frames: 1,
            supported: true,
        },
        Case {
            id: "stream-raw-chain",
            level: 4,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            chunk: 1024,
            output: 7,
            frames: 1,
            supported: true,
        },
        Case {
            id: "stream-raw-tree",
            level: 9,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            chunk: 333,
            output: 5,
            frames: 1,
            supported: true,
        },
        Case {
            id: "stream-prepared-fast",
            level: 1,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 211,
            output: 2,
            frames: 1,
            supported: true,
        },
        Case {
            id: "stream-prepared-chain",
            level: 4,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 97,
            output: 4,
            frames: 1,
            supported: true,
        },
        Case {
            id: "stream-prepared-tree",
            level: 9,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 509,
            output: 9,
            frames: 1,
            supported: true,
        },
        Case {
            id: "stream-raw-window",
            level: 4,
            window: 10,
            dictionary: "raw-a",
            input: "raw-first",
            chunk: 256,
            output: 6,
            frames: 1,
            supported: true,
        },
        Case {
            id: "stream-raw-large",
            level: 4,
            window: 0,
            dictionary: "raw-a",
            input: "large",
            chunk: 4093,
            output: 11,
            frames: 1,
            supported: true,
        },
        Case {
            id: "stream-raw-multiframe",
            level: 4,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            chunk: 199,
            output: 8,
            frames: 2,
            supported: true,
        },
        Case {
            id: "stream-optimal",
            level: 11,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 128,
            output: 8,
            frames: 1,
            supported: false,
        },
        Case {
            id: "stream-ultra2",
            level: 17,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 128,
            output: 8,
            frames: 1,
            supported: false,
        },
    ]
}

fn dictionary(name: &str, inputs: &std::path::Path) -> zstdoxide::SharedDictionary {
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

/// Pump one frame in `chunk` input fragments and `output` caller bytes.
fn pump_frame(
    encoder: &mut CompiledStreamEncoder<'_, Global>,
    input: &[u8],
    chunk: usize,
    output_size: usize,
) -> Vec<u8> {
    let mut frame = Vec::new();
    let mut output = vec![0_u8; output_size.max(1)];
    let mut position = 0;
    loop {
        let end = (position + chunk.max(1)).min(input.len());
        let directive = if end == input.len() {
            EndDirective::Finish
        } else {
            EndDirective::Continue
        };
        let mut consumed = 0;
        loop {
            let progress: Progress = encoder
                .process(&input[position + consumed..end], &mut output, directive)
                .unwrap();
            frame.extend_from_slice(&output[..progress.produced]);
            consumed += progress.consumed;
            match progress.status {
                StreamStatus::NeedInput => break,
                StreamStatus::NeedOutput => continue,
                StreamStatus::FrameFinished => {
                    assert_eq!(position + consumed, input.len());
                    return frame;
                }
                StreamStatus::Flushed => unreachable!(),
            }
        }
        position = end;
    }
}

#[test]
#[ignore = "requires ZSTDOXIDE_COMPILED_STREAM_NATIVE and a pinned native oracle"]
fn compiled_stream_schedules_interoperate_with_guarded_native_controls() {
    let root = PathBuf::from(std::env::var_os("ZSTDOXIDE_COMPILED_STREAM_NATIVE").unwrap());
    let export = std::env::var_os("ZSTDOXIDE_COMPILED_STREAM_EXPORT").is_some();
    let inputs = root.join("inputs");
    fs::create_dir_all(&inputs).unwrap();
    let cases = schedule();
    if export {
        fs::write(inputs.join("raw-a.dict"), RAW).unwrap();
        fs::write(inputs.join("prepared-a.dict"), FORMATTED).unwrap();
        fs::write(inputs.join("raw.input"), [RAW, RAW].concat()).unwrap();
        fs::write(inputs.join("json.input"), TEXT).unwrap();
        fs::write(inputs.join("raw-first.input"), &RAW[..1024]).unwrap();
        fs::write(inputs.join("large.input"), RAW.repeat(16)).unwrap();
        let mut steps = String::new();
        for case in &cases {
            steps.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                case.id,
                case.level,
                case.window,
                case.dictionary,
                case.input,
                case.chunk,
                case.output,
                case.frames
            ));
            let input = fs::read(inputs.join(format!("{}.input", case.input))).unwrap();
            let dictionary = dictionary(case.dictionary, &inputs);
            let outcome =
                CompiledDictionary::new(dictionary.view(), &request(case, &input), Global);
            if case.supported {
                let compiled = outcome.unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
                let mut encoder = CompiledStreamEncoder::new_in(compiled, Global)
                    .unwrap_or_else(|error| panic!("{} stream construction: {error:?}", case.id));
                for frame_index in 0..case.frames {
                    let frame = pump_frame(&mut encoder, &input, case.chunk, case.output);
                    fs::write(
                        inputs.join(format!("{}-{}.rust", case.id, frame_index)),
                        &frame,
                    )
                    .unwrap();
                    if frame_index + 1 != case.frames {
                        encoder.reset();
                    }
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
        let input = fs::read(inputs.join(format!("{}.input", case.input))).unwrap();
        let dictionary = dictionary(case.dictionary, &inputs);
        let native_frames: usize = fields[3].parse().unwrap();
        assert_eq!(native_frames, case.frames, "{}", case.id);
        assert_eq!(fields[6], "1", "native frame decoding {}", case.id);
        for frame_index in 0..case.frames {
            let frame = fs::read(
                root.join("outputs")
                    .join(format!("{}-{}.zst", case.id, frame_index)),
            )
            .unwrap();
            assert_eq!(
                decompress_with_dictionary_in(
                    &frame,
                    &dictionary.view(),
                    DecodeOptions::new(input.len()),
                    Global,
                )
                .unwrap(),
                input,
                "{} frame {}",
                case.id,
                frame_index
            );
        }
        if case.supported {
            assert_eq!(fields[5], "1", "native decode of Rust frames {}", case.id);
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
