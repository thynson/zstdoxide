//! Guarded native replacement/reset schedules for streaming selections.
//!
//! The ignored test exports Rust frames and inputs for an external pinned
//! v1.5.7 oracle and then replays native frames. Both sides run the same
//! lifecycle sequence per case: `R` selects the compiled dictionary,
//! `P` selects a one-frame prefix, `E` selects an empty prefix (detach),
//! `C` resets the session only, `Q` is the parameter-only reset, `A` is the
//! session-and-parameters reset, `H` feeds half of the source in a still-open
//! frame, `F` completes one frame, and lowercase `p`/`q` assert the mid-frame
//! reference and parameter-reset errors. Frame attachments are tracked on
//! both sides; native byte identity is not required, only that every frame
//! decodes with the attachment that produced it on both sides.
#![forbid(unsafe_code)]
use std::alloc::Global;
use std::fs;
use std::path::PathBuf;
use zstdoxide::{
    CompiledDictionary, CompiledStreamEncoder, CompressionParameterRequest, DecodeOptions,
    Dictionary, EndDirective, Error, StreamStatus, decompress_in, decompress_with_dictionary_in,
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
    ops: &'static str,
    supported: bool,
}

fn schedule() -> Vec<Case> {
    vec![
        Case {
            id: "select-cdict-then-prefix-raw",
            level: 1,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            chunk: 137,
            output: 1,
            ops: "RFCPFCF",
            supported: true,
        },
        Case {
            id: "select-cdict-then-prefix-prepared",
            level: 4,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 501,
            output: 3,
            ops: "RFCPFCF",
            supported: true,
        },
        Case {
            id: "select-prefix-then-cdict-tree",
            level: 9,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            chunk: 1024,
            output: 7,
            ops: "PFCRF",
            supported: true,
        },
        Case {
            id: "select-prefix-twice-then-cdict",
            level: 4,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 333,
            output: 5,
            ops: "PFCPFCRFCF",
            supported: true,
        },
        Case {
            id: "select-empty-prefix-detaches",
            level: 1,
            window: 0,
            dictionary: "raw-a",
            input: "raw-first",
            chunk: 211,
            output: 2,
            ops: "PFCECF",
            supported: true,
        },
        Case {
            id: "select-parameter-reset-dict-free",
            level: 4,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            chunk: 97,
            output: 4,
            ops: "RFCQF",
            supported: true,
        },
        Case {
            id: "select-session-reset-reuses-cdict",
            level: 3,
            window: 0,
            dictionary: "raw-a",
            input: "large",
            chunk: 509,
            output: 9,
            ops: "RFCF",
            supported: true,
        },
        Case {
            id: "select-prefix-error-midstream",
            level: 1,
            window: 0,
            dictionary: "raw-a",
            input: "raw",
            chunk: 199,
            output: 8,
            ops: "RHpFCF",
            supported: true,
        },
        Case {
            id: "select-parameter-error-midstream",
            level: 4,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 256,
            output: 6,
            ops: "RHqFCF",
            supported: true,
        },
        Case {
            id: "select-session-and-parameters-reset",
            level: 4,
            window: 0,
            dictionary: "raw-a",
            input: "raw-first",
            chunk: 4093,
            output: 11,
            ops: "RFAF",
            supported: true,
        },
        Case {
            id: "select-optimal-prefix",
            level: 13,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 128,
            output: 8,
            ops: "PF",
            supported: false,
        },
        Case {
            id: "select-ultra2-prefix",
            level: 17,
            window: 0,
            dictionary: "prepared-a",
            input: "json",
            chunk: 128,
            output: 8,
            ops: "PF",
            supported: false,
        },
    ]
}

fn dictionary(name: &str, inputs: &std::path::Path) -> Dictionary {
    let bytes = fs::read(inputs.join(format!("{name}.dict"))).unwrap();
    if name == "prepared-a" {
        Dictionary::from_bytes(&bytes)
    } else {
        Dictionary::raw(&bytes)
    }
    .unwrap()
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

/// Feed `input` fully with `directive`, pumping bounded output windows and
/// collecting every produced byte into `frame`.
fn feed(
    encoder: &mut CompiledStreamEncoder<'_, Global>,
    input: &[u8],
    directive: EndDirective,
    chunk: usize,
    output_size: usize,
    frame: &mut Vec<u8>,
) {
    let mut output = vec![0_u8; output_size.max(1)];
    let mut consumed = 0;
    loop {
        let end = (consumed + chunk.max(1)).min(input.len());
        let progress = encoder
            .process(&input[consumed..end], &mut output, directive)
            .unwrap();
        frame.extend_from_slice(&output[..progress.produced]);
        consumed += progress.consumed;
        match progress.status {
            StreamStatus::NeedInput => {
                if consumed == input.len() {
                    if directive != EndDirective::Finish {
                        return;
                    }
                    continue;
                }
            }
            StreamStatus::NeedOutput => continue,
            StreamStatus::FrameFinished => {
                assert_eq!(consumed, input.len());
                return;
            }
            StreamStatus::Flushed => unreachable!(),
        }
    }
}

/// Complete one frame from `position`, appending the frame bytes to `frame`.
/// The final fragment uses `Finish`; earlier fragments `Continue`. A
/// continuing frame (after `H`) appends to the pending bytes collected so far,
/// because the coordinator emits its frame header on the first process call.
fn frame_from(
    encoder: &mut CompiledStreamEncoder<'_, Global>,
    input: &[u8],
    position: usize,
    chunk: usize,
    output_size: usize,
    frame: &mut Vec<u8>,
) {
    let mut output = vec![0_u8; output_size.max(1)];
    let mut consumed = position;
    loop {
        let end = (consumed + chunk.max(1)).min(input.len());
        let directive = if end == input.len() {
            EndDirective::Finish
        } else {
            EndDirective::Continue
        };
        loop {
            let progress = encoder
                .process(&input[consumed..end], &mut output, directive)
                .unwrap();
            frame.extend_from_slice(&output[..progress.produced]);
            consumed += progress.consumed;
            match progress.status {
                StreamStatus::NeedInput => break,
                StreamStatus::NeedOutput => continue,
                StreamStatus::FrameFinished => return,
                StreamStatus::Flushed => unreachable!(),
            }
        }
    }
}

fn attachment_name(state: u8, dictionary: &str) -> &'static str {
    match state {
        0 => "none",
        _ if dictionary == "prepared-a" => "prepared-a",
        _ => "raw-a",
    }
}

/// Decode one frame with the attachment that produced it.
fn decode_with_attachment(
    attachment: &str,
    frame: &[u8],
    dictionary: &Dictionary,
    input: &[u8],
) -> bool {
    let expected = input.len();
    if attachment == "none" {
        return decompress_in(frame, expected, Global)
            .map(|decoded| decoded == input)
            .unwrap_or(false);
    }
    decompress_with_dictionary_in(
        frame,
        &dictionary.view(),
        DecodeOptions::new(expected),
        Global,
    )
    .map(|decoded| decoded == input)
    .unwrap_or(false)
}

fn frame_count(ops: &str) -> usize {
    ops.chars().filter(|op| *op == 'F').count()
}

#[test]
#[ignore = "requires ZSTDOXIDE_COMPILED_STREAM_SELECTION_NATIVE and a pinned native oracle"]
fn compiled_stream_selection_schedules_interoperate_with_guarded_native_controls() {
    let root =
        PathBuf::from(std::env::var_os("ZSTDOXIDE_COMPILED_STREAM_SELECTION_NATIVE").unwrap());
    let export = std::env::var_os("ZSTDOXIDE_COMPILED_STREAM_SELECTION_EXPORT").is_some();
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
        let mut rust_rows = String::new();
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
                case.ops
            ));
            let input = fs::read(inputs.join(format!("{}.input", case.input))).unwrap();
            let dictionary = dictionary(case.dictionary, &inputs);
            let outcome =
                CompiledDictionary::new(dictionary.view(), &request(case, &input), Global);
            if !case.supported {
                assert_eq!(
                    outcome.err(),
                    Some(Error::UnsupportedFeature),
                    "{}",
                    case.id
                );
                rust_rows.push_str(&format!("{}\t-1\t0\t-\t-1\n", case.id));
                continue;
            }
            let compiled = outcome.unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
            let mut encoder = CompiledStreamEncoder::new_in(compiled, Global)
                .unwrap_or_else(|error| panic!("{} construction: {error:?}", case.id));
            let empty = Dictionary::raw(b"").unwrap();
            let mut displaced: Option<CompiledDictionary<'_, Global>> = None;
            let mut state: u8 = 1;
            let mut attachments: Vec<String> = Vec::new();
            let mut frames: Vec<Vec<u8>> = Vec::new();
            // One frame may span an `H` segment and its `F` completion; the
            // coordinator emits the frame header on the first process call,
            // so both segments collect into the same pending buffer.
            let mut pending_frame = Vec::new();
            let mut position = 0_usize;
            let half = input.len() / 2;
            for op in case.ops.chars() {
                match op {
                    'R' => {
                        // The compiled selection is active from construction;
                        // reselection only has work after a displacement.
                        if let Some(compiled_state) = displaced.take() {
                            let previous = encoder
                                .set_compiled(compiled_state)
                                .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
                            assert!(previous.is_none(), "{}", case.id);
                        } else {
                            assert!(
                                encoder.selection().is_some(),
                                "{}: reselection without a compiled state",
                                case.id
                            );
                        }
                        state = 1;
                    }
                    'P' => {
                        // A prefix only displaces a selected compiled state;
                        // an already-detached selection returns nothing and
                        // the previously displaced state stays held.
                        if let Some(previous) = encoder
                            .set_prefix(dictionary.view())
                            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id))
                        {
                            displaced = Some(previous);
                        }
                        state = 2;
                    }
                    'E' => {
                        if let Some(previous) = encoder
                            .set_prefix(empty.view())
                            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id))
                        {
                            displaced = Some(previous);
                        }
                        state = 0;
                    }
                    'C' => encoder.reset(),
                    'Q' => {
                        if let Some(previous) = encoder
                            .reset_parameters()
                            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id))
                        {
                            displaced = Some(previous);
                        }
                        state = 0;
                    }
                    'A' => {
                        if let Some(previous) = encoder.reset_all() {
                            displaced = Some(previous);
                        }
                        state = 0;
                    }
                    'H' => {
                        feed(
                            &mut encoder,
                            &input[position..half],
                            EndDirective::Continue,
                            case.chunk,
                            case.output,
                            &mut pending_frame,
                        );
                        position = half;
                    }
                    'F' => {
                        frame_from(
                            &mut encoder,
                            &input,
                            position,
                            case.chunk,
                            case.output,
                            &mut pending_frame,
                        );
                        // Every frame carries the whole source: a continuing
                        // frame starts at `position` (set by `H`), and the
                        // next frame re-feeds from the beginning after the
                        // session boundary.
                        position = 0;
                        attachments.push(attachment_name(state, case.dictionary).to_string());
                        frames.push(std::mem::take(&mut pending_frame));
                        // A prefix frame is single usage; a compiled frame keeps.
                        if state == 2 {
                            state = 0;
                        }
                    }
                    'p' => {
                        assert!(
                            matches!(
                                encoder.set_prefix(dictionary.view()),
                                Err(Error::InvalidState)
                            ),
                            "{}",
                            case.id
                        );
                    }
                    'q' => {
                        assert!(
                            matches!(encoder.reset_parameters(), Err(Error::InvalidState)),
                            "{}",
                            case.id
                        );
                    }
                    other => panic!("{}: unknown op {other}", case.id),
                }
            }
            assert_eq!(frames.len(), frame_count(case.ops), "{}", case.id);
            for (index, frame) in frames.iter().enumerate() {
                fs::write(inputs.join(format!("{}-{}.rust", case.id, index)), frame).unwrap();
            }
            let attachment_list = if attachments.is_empty() {
                "-".to_string()
            } else {
                attachments.join(",")
            };
            rust_rows.push_str(&format!(
                "{}\t{}\t{}\t{}\t1\n",
                case.id,
                frames.iter().map(|frame| frame.len()).sum::<usize>(),
                frames.len(),
                attachment_list
            ));
        }
        fs::write(root.join("steps.tsv"), steps).unwrap();
        fs::write(root.join("rust.tsv"), rust_rows).unwrap();
        return;
    }
    let native = fs::read_to_string(root.join("native.tsv")).unwrap();
    let rows: Vec<_> = native.lines().collect();
    assert_eq!(rows.len(), cases.len());
    for (case, row) in cases.iter().zip(&rows) {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 7, "{}", case.id);
        assert_eq!(fields[0], case.id);
        assert_eq!(fields[1], "ok", "{}", case.id);
        let frames: usize = fields[3].parse().unwrap();
        assert_eq!(frames, frame_count(case.ops), "{}", case.id);
        // Native frames always decode through the pinned decompressor.
        assert_eq!(fields[6], "1", "native frame decoding {}", case.id);
        let input = fs::read(inputs.join(format!("{}.input", case.input))).unwrap();
        let dictionary = dictionary(case.dictionary, &inputs);
        if case.supported {
            assert_eq!(fields[5], "1", "native decode of Rust frames {}", case.id);
            let attachments: Vec<_> = fields[4].split(',').collect();
            assert_eq!(attachments.len(), frames, "{}", case.id);
            for (index, attachment) in attachments.iter().enumerate() {
                let frame = fs::read(
                    root.join("outputs")
                        .join(format!("{}-{}.zst", case.id, index)),
                )
                .unwrap();
                assert!(
                    decode_with_attachment(attachment, &frame, &dictionary, &input),
                    "{} frame {index} attachment {attachment}",
                    case.id
                );
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
            // Native prefix frames of unsupported families still decode in
            // Rust: decompression is independent of the strategy.
            let attachments: Vec<_> = fields[4].split(',').collect();
            for (index, attachment) in attachments.iter().enumerate() {
                let frame = fs::read(
                    root.join("outputs")
                        .join(format!("{}-{}.zst", case.id, index)),
                )
                .unwrap();
                assert!(
                    decode_with_attachment(attachment, &frame, &dictionary, &input),
                    "{} frame {index}",
                    case.id
                );
            }
        }
    }
}
