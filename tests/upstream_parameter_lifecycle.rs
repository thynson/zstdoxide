// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Assertion contracts derive from facebook/zstd v1.5.7 tests/fuzzer.c and
// tests/zstreamtest.c under the BSD-3-Clause option; see LICENSE.zstd.
//! Supported encoder lifecycle adaptations of the original parameter tests.
//!
//! Ordering/reset cases retain the original 10-KiB raw dictionary and 100-KiB
//! source dimensions. Source bytes are an explicitly selected RDG campaign:
//! RDG_genBuffer(100 KiB, 0.5, 0.0, seed 0), with its first 10 KiB as dictionary.
//! This does not claim the original mutable CNBuffer's inherited contents.
//! Greedy/Chain/Lazy are independent Rust policies, not upstream numeric levels.
//! Formatted-dictionary flag checks separately reuse the pinned formatted.dict.
//! Rust requires explicit reset after Finished, enables checksum by default,
//! and clears pledges when restoring defaults; these differ from the C API.
#![forbid(unsafe_code)]

#[path = "upstream/support/mod.rs"]
#[allow(
    dead_code,
    reason = "the shared upstream generator module also contains unrelated helpers"
)]
mod upstream_support;

use std::sync::OnceLock;
use zstdoxide::{
    CompressionStrategy, DecodeOptions, Dictionary, Encoder, EncoderOptions, EndDirective, Error,
    FrameHeader, Progress, SharedDictionary, StreamStatus, decompress, decompress_with_dictionary,
};

const SOURCE_SIZE: usize = 100 * 1024;
const DICTIONARY_SIZE: usize = 10 * 1024;
const OUTPUT_CHUNK: usize = 257;
const STRATEGIES: [CompressionStrategy; 3] = [
    CompressionStrategy::Greedy,
    CompressionStrategy::Chain,
    CompressionStrategy::Lazy,
];

fn source() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| upstream_support::noise(SOURCE_SIZE, 0.5, 0.0, 0))
}

fn raw_dictionary() -> SharedDictionary {
    assert_eq!(source().len(), SOURCE_SIZE);
    let dictionary = Dictionary::raw(&source()[..DICTIONARY_SIZE]).unwrap();
    assert_eq!(dictionary.as_bytes().len(), DICTIONARY_SIZE);
    dictionary.into_shared()
}

fn formatted_dictionary() -> SharedDictionary {
    let dictionary =
        Dictionary::from_bytes(include_bytes!("upstream/dictionaries/formatted.dict")).unwrap();
    assert_ne!(dictionary.id(), 0);
    dictionary.into_shared()
}

fn settings(strategy: CompressionStrategy) -> EncoderOptions {
    EncoderOptions {
        strategy,
        window_log: 17,
        pledged_size: Some(SOURCE_SIZE as u64),
        ..EncoderOptions::default()
    }
}

fn step(
    encoder: &mut Encoder,
    input: &[u8],
    directive: EndDirective,
    capacity: usize,
    frame: &mut Vec<u8>,
) -> Progress {
    assert!(capacity <= OUTPUT_CHUNK);
    let mut output = [0xa5; OUTPUT_CHUNK + 8];
    let before = (
        encoder.total_input(),
        encoder.total_output(),
        encoder.options(),
    );
    let progress = encoder
        .process(input, &mut output[..capacity], directive)
        .unwrap();
    assert!(progress.consumed <= input.len());
    assert!(progress.produced <= capacity);
    assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
    assert_eq!(encoder.total_input(), before.0 + progress.consumed as u64);
    assert_eq!(encoder.total_output(), before.1 + progress.produced as u64);
    assert_eq!(encoder.options(), before.2);
    frame.extend_from_slice(&output[..progress.produced]);
    progress
}

fn action(encoder: &mut Encoder, input: &[u8], directive: EndDirective, frame: &mut Vec<u8>) {
    let mut consumed = 0;
    for call in 1..100_000 {
        let capacity = if call % 7 == 0 { 0 } else { OUTPUT_CHUNK };
        let progress = step(encoder, &input[consumed..], directive, capacity, frame);
        consumed += progress.consumed;
        let complete = match directive {
            EndDirective::Continue => progress.status == StreamStatus::NeedInput,
            EndDirective::Flush => progress.status == StreamStatus::Flushed,
            EndDirective::Finish => progress.status == StreamStatus::FrameFinished,
        };
        if complete {
            assert_eq!(consumed, input.len());
            return;
        }
        assert_eq!(progress.status, StreamStatus::NeedOutput);
        assert!(capacity == 0 || progress.consumed != 0 || progress.produced != 0);
    }
    panic!("bounded lifecycle operation did not finish");
}

fn finish(encoder: &mut Encoder) -> Vec<u8> {
    let mut frame = Vec::new();
    action(encoder, source(), EndDirective::Finish, &mut frame);
    assert_eq!(encoder.total_input(), SOURCE_SIZE as u64);
    assert_eq!(encoder.total_output(), frame.len() as u64);
    frame
}

fn verify(frame: &[u8], options: EncoderOptions, dictionary: Option<&SharedDictionary>) {
    let (header, _) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.window_size, 1_u64 << options.window_log);
    assert_eq!(header.has_checksum, options.checksum);
    assert_eq!(
        header.content_size,
        options
            .pledged_size
            .filter(|_| options.include_content_size)
    );
    assert_eq!(
        header.dictionary_id,
        if options.include_dictionary_id {
            dictionary.map_or(0, |dictionary| dictionary.id())
        } else {
            0
        }
    );
    let decoded = match dictionary {
        Some(dictionary) => {
            decompress_with_dictionary(frame, dictionary, DecodeOptions::new(SOURCE_SIZE))
        }
        None => decompress(frame, SOURCE_SIZE),
    }
    .unwrap();
    assert_eq!(decoded, source());
}

/// fuzzer.c:3001-3031: dictionary/options ordering remains equivalent.
/// The original level 1/hash 12 and level 7 live controls are not implemented;
/// each named policy uses a supported option record and shared dictionary.
#[test]
fn dictionary_and_supported_options_order_produce_identical_complete_frames() {
    for strategy in STRATEGIES {
        let dictionary = raw_dictionary();
        let options = EncoderOptions {
            checksum: false,
            ..settings(strategy)
        };
        let mut dictionary_first = Encoder::new();
        dictionary_first
            .set_dictionary(Some(dictionary.clone()))
            .unwrap();
        dictionary_first.set_options(options).unwrap();
        let first = finish(&mut dictionary_first);
        let mut options_first = Encoder::new();
        options_first.set_options(options).unwrap();
        options_first
            .set_dictionary(Some(dictionary.clone()))
            .unwrap();
        let second = finish(&mut options_first);
        assert_eq!(first, second, "strategy={strategy:?}");
        verify(&first, options, Some(&dictionary));
        // The dictionary must affect real matches, not merely be stored.
        assert!(decompress(&first, SOURCE_SIZE).is_err());
    }
}

fn reject_configuration_without_changing_observable_state(encoder: &mut Encoder) {
    let before = (
        encoder.options(),
        encoder.total_input(),
        encoder.total_output(),
        encoder.retained_capacity(),
    );
    assert_eq!(
        encoder.set_options(EncoderOptions::default()),
        Err(Error::InvalidState)
    );
    assert_eq!(
        encoder.set_options(EncoderOptions {
            window_log: 9,
            ..EncoderOptions::default()
        }),
        Err(Error::InvalidState)
    );
    assert_eq!(encoder.reset_parameters(), Err(Error::InvalidState));
    assert_eq!(
        (
            encoder.options(),
            encoder.total_input(),
            encoder.total_output(),
            encoder.retained_capacity()
        ),
        before
    );
}

fn fragmented_frame(strategy: CompressionStrategy, reject_changes: bool) -> Vec<u8> {
    let dictionary = raw_dictionary();
    let options = EncoderOptions {
        include_content_size: false,
        ..settings(strategy)
    };
    let mut encoder = Encoder::with_dictionary(options, dictionary.clone()).unwrap();
    let mut frame = Vec::new();
    let first = step(
        &mut encoder,
        &source()[..31],
        EndDirective::Continue,
        1,
        &mut frame,
    );
    assert_eq!((first.consumed, first.produced), (31, 1));
    assert_eq!(frame, [0x28]);
    assert_eq!(first.status, StreamStatus::NeedOutput);
    if reject_changes {
        reject_configuration_without_changing_observable_state(&mut encoder);
    }
    action(
        &mut encoder,
        &source()[31..SOURCE_SIZE / 2],
        EndDirective::Continue,
        &mut frame,
    );
    if reject_changes {
        reject_configuration_without_changing_observable_state(&mut encoder);
    }
    let flush = step(&mut encoder, &[], EndDirective::Flush, 0, &mut frame);
    assert_eq!(flush.status, StreamStatus::NeedOutput);
    if reject_changes {
        reject_configuration_without_changing_observable_state(&mut encoder);
    }
    action(&mut encoder, &[], EndDirective::Flush, &mut frame);
    action(
        &mut encoder,
        &source()[SOURCE_SIZE / 2..],
        EndDirective::Continue,
        &mut frame,
    );
    let end = step(&mut encoder, &[], EndDirective::Finish, 0, &mut frame);
    assert_eq!(end.status, StreamStatus::NeedOutput);
    if reject_changes {
        reject_configuration_without_changing_observable_state(&mut encoder);
    }
    action(&mut encoder, &[], EndDirective::Finish, &mut frame);
    assert_eq!(encoder.options(), options);
    assert_eq!(encoder.total_input(), SOURCE_SIZE as u64);
    assert_eq!(encoder.total_output(), frame.len() as u64);
    assert_eq!(encoder.set_options(options), Err(Error::InvalidState));
    verify(&frame, options, Some(&dictionary));
    frame
}

/// fuzzer.c:1721-1760 and zstreamtest.c:1599-1620/:3090/:3186 preservation
/// assertions adapted to supported settings. Rust rejects all active mutation;
/// C's individual effort setters and automatic next-frame protocol differ.
#[test]
fn settings_survive_partial_output_flush_finish_and_rejected_configuration() {
    for strategy in STRATEGIES {
        assert_eq!(
            fragmented_frame(strategy, false),
            fragmented_frame(strategy, true)
        );
    }
}

/// fuzzer.c:3088-3123: session reset preserves a useful 10 KiB dictionary;
/// parameter reset clears it. Rust's parameter-only reset preserves Finished,
/// so the next frame needs a separate session reset. Pledges clear on reset.
#[test]
fn session_reset_preserves_dictionary_and_parameter_reset_clears_it() {
    for strategy in STRATEGIES {
        let dictionary = raw_dictionary();
        let options = settings(strategy);
        let mut encoder = Encoder::with_dictionary(options, dictionary.clone()).unwrap();
        let first = finish(&mut encoder);
        verify(&first, options, Some(&dictionary));
        assert!(decompress(&first, SOURCE_SIZE).is_err());
        let capacity = encoder.retained_capacity();
        encoder.reset();
        assert_eq!(
            encoder.options(),
            EncoderOptions {
                pledged_size: None,
                ..options
            }
        );
        assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
        assert_eq!(encoder.retained_capacity(), capacity);
        encoder.set_options(options).unwrap();
        assert_eq!(finish(&mut encoder), first);
        let counters = (encoder.total_input(), encoder.total_output());
        encoder.reset_parameters().unwrap();
        assert_eq!(encoder.options(), EncoderOptions::default());
        assert_eq!((encoder.total_input(), encoder.total_output()), counters);
        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
        assert_eq!(encoder.set_options(options), Err(Error::InvalidState));
        let finished = encoder.process(&[], &mut [], EndDirective::Finish).unwrap();
        assert_eq!(finished.status, StreamStatus::FrameFinished);
        assert_eq!((finished.consumed, finished.produced), (0, 0));
        encoder.reset();
        let plain = finish(&mut encoder);
        verify(&plain, EncoderOptions::default(), None);
        // Preserve the original clearing case's size inequality as well as its
        // useful dictionary dependency above. Reset restores Greedy defaults,
        // even when the dictionary-backed frame used Chain or Lazy.
        assert_ne!(first.len(), plain.len(), "starting strategy={strategy:?}");
        assert_eq!(plain, finish(&mut Encoder::new()));
    }
}

/// Dictionary/prefix reset qualification for fuzzer.c:3032-3104. A prefix is
/// single-use; session reset preserves only an unclaimed one. Parameter-only
/// reset clears an unclaimed prefix without starting a frame.
#[test]
fn session_reset_preserves_unclaimed_prefix_and_parameter_reset_removes_it() {
    for strategy in STRATEGIES {
        let dictionary = raw_dictionary();
        let options = settings(strategy);
        let mut encoder = Encoder::with_options(options).unwrap();
        encoder.set_prefix(dictionary.clone()).unwrap();
        encoder.reset();
        encoder.set_options(options).unwrap();
        let prefixed = finish(&mut encoder);
        verify(&prefixed, options, Some(&dictionary));
        assert!(decompress(&prefixed, SOURCE_SIZE).is_err());
        encoder.reset();
        encoder.set_options(options).unwrap();
        let plain = finish(&mut encoder);
        verify(&plain, options, None);
        assert_eq!(plain, finish(&mut Encoder::with_options(options).unwrap()));
        encoder.reset();
        encoder.set_prefix(dictionary.clone()).unwrap();
        encoder.reset_parameters().unwrap();
        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
        let cleared = finish(&mut encoder);
        verify(&cleared, EncoderOptions::default(), None);
        assert_eq!(cleared, finish(&mut Encoder::new()));
    }
}

/// fuzzer.c:3104 clearing semantics, expanded at Rust's header-claim boundary.
/// Full reset discards both selected and already-claimed prefixes and pending
/// output, then restores ready defaults; no source buffer is retained.
#[test]
fn full_reset_discards_unclaimed_and_claimed_prefixes_before_plain_encoding() {
    for strategy in STRATEGIES {
        for claimed in [false, true] {
            let dictionary = raw_dictionary();
            let mut encoder = Encoder::with_options(settings(strategy)).unwrap();
            encoder.set_prefix(dictionary.clone()).unwrap();
            if claimed {
                let mut discarded = Vec::new();
                let progress = step(
                    &mut encoder,
                    &source()[..31],
                    EndDirective::Continue,
                    1,
                    &mut discarded,
                );
                assert_eq!((progress.consumed, progress.produced), (31, 1));
            }
            encoder.reset_all();
            assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
            assert_eq!(encoder.options(), EncoderOptions::default());
            assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
            let frame = finish(&mut encoder);
            verify(&frame, EncoderOptions::default(), None);
            assert_eq!(frame, finish(&mut Encoder::new()));
        }
    }
}

/// fuzzer.c:1808-1890 supported frame-setting subset. The original FParams
/// {contentSizeFlag 0, checksumFlag 1, noDictIDFlag 1} maps to false/true/false.
/// Invalid live windows reject atomically; this is not seven-field CParams or
/// combined CParams/FParams bulk setter equivalence. The prepared dictionary
/// below is a separate pinned fixture, not the 10 KiB raw corpus above.
#[test]
fn frame_flags_and_invalid_windows_preserve_atomic_supported_configuration() {
    for strategy in STRATEGIES {
        for flags in [(true, false, true), (false, true, false)] {
            let dictionary = formatted_dictionary();
            let mut encoder = Encoder::new();
            encoder.set_dictionary(Some(dictionary.clone())).unwrap();
            let before = (
                encoder.options(),
                encoder.retained_capacity(),
                SharedDictionary::strong_count(&dictionary),
            );
            for window_log in [9, 32] {
                assert_eq!(
                    encoder.set_options(EncoderOptions {
                        window_log,
                        ..settings(strategy)
                    }),
                    Err(Error::InvalidParameter)
                );
                assert_eq!(
                    (
                        encoder.options(),
                        encoder.retained_capacity(),
                        SharedDictionary::strong_count(&dictionary)
                    ),
                    before
                );
                assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
            }
            let options = EncoderOptions {
                include_content_size: flags.0,
                checksum: flags.1,
                include_dictionary_id: flags.2,
                ..settings(strategy)
            };
            encoder.set_options(options).unwrap();
            assert_eq!(encoder.options(), options);
            let frame = finish(&mut encoder);
            verify(&frame, options, Some(&dictionary));
        }
    }
}

#[test]
#[ignore = "requires ZSTD_BIN pointing to the pinned official v1.5.7 CLI"]
fn official_codec_decodes_configured_session_and_parameter_reset_frames() {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Temporary(PathBuf);
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let executable = std::env::var_os("ZSTD_BIN").expect("set ZSTD_BIN to the pinned v1.5.7 CLI");
    let version = Command::new(&executable).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("v1.5.7"));
    let directory = Temporary(std::env::temp_dir().join(format!(
        "zstdoxide-parameter-lifecycle-{}-{}-{}", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&directory.0).unwrap();
    let mut cases = 0;
    for strategy in STRATEGIES {
        for (kind, dictionary) in [
            ("raw", raw_dictionary()),
            ("formatted", formatted_dictionary()),
        ] {
            let dictionary_path = directory.0.join("dictionary");
            fs::write(&dictionary_path, dictionary.as_bytes()).unwrap();
            let options = EncoderOptions {
                checksum: kind == "raw",
                ..settings(strategy)
            };
            let mut encoder =
                Encoder::with_dictionary(EncoderOptions::default(), dictionary.clone()).unwrap();
            encoder.set_options(options).unwrap();
            for operation in [
                "configured",
                "session_reset",
                "parameter_reset",
                "prefix",
                "full_reset",
            ] {
                let selected = match operation {
                    "configured" => true,
                    "session_reset" => {
                        encoder.reset();
                        encoder.set_options(options).unwrap();
                        true
                    }
                    "parameter_reset" => {
                        encoder.reset_parameters().unwrap();
                        encoder.reset();
                        false
                    }
                    "prefix" => {
                        encoder.reset();
                        encoder
                            .set_options(EncoderOptions {
                                include_dictionary_id: false,
                                ..options
                            })
                            .unwrap();
                        encoder.set_prefix(dictionary.clone()).unwrap();
                        true
                    }
                    "full_reset" => {
                        encoder.reset_all();
                        false
                    }
                    _ => unreachable!(),
                };
                let emitted_options = encoder.options();
                let frame = finish(&mut encoder);
                verify(&frame, emitted_options, selected.then_some(&dictionary));
                let frame_path = directory.0.join("frame.zst");
                fs::write(&frame_path, frame).unwrap();
                let mut command = Command::new(&executable);
                command.args(["-q", "-d", "-c"]);
                if selected {
                    command.arg("-D").arg(&dictionary_path);
                }
                let output = command.arg(&frame_path).output().unwrap();
                assert!(
                    output.status.success(),
                    "{strategy:?}/{kind}/{operation}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(output.stdout, source(), "{strategy:?}/{kind}/{operation}");
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 30);
    eprintln!(
        "official lifecycle decode: {cases} complete frames / {} decoded bytes",
        cases * SOURCE_SIZE
    );
    eprintln!(
        "18 frames use their selected named policy; 12 parameter/full-reset frames restore Greedy defaults"
    );
}
