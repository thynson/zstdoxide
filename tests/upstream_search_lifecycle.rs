// Copyright (c) Meta Platforms, Inc. and affiliates.
// Copyright (c) zstdoxide contributors.
// Supported assertion contracts derive from facebook/zstd v1.5.7
// tests/fuzzer.c under the BSD-3-Clause option; see LICENSE.zstd.
//! Live search-setting subsets of fuzzer.c:1721–1763 and :3001–3031.
//!
//! C's automatic hash value 0, minimum hash log 6, and numeric levels 3/7 belong
//! to the separately tested detached parameter API. This live API has concrete
//! default hash log 16, minimum 10, and independent Greedy/Chain/Lazy policies.
//! It neither selects those numeric levels nor reproduces C's CDict attachment
//! modes. Rust requires an explicit session reset after a completed frame.
//!
//! Ordering preserves the original 10 KiB dictionary / 100 KiB source sizes.
//! The corpus is the declared RDG_genBuffer(100 KiB, 0.5, 0.0, seed 0) campaign,
//! matching upstream_parameter_lifecycle.rs, not inherited mutable CNBuffer
//! bytes. Additional formatted attachment checks use the pinned formatted.dict.
#![forbid(unsafe_code)]

use std::alloc::{Allocator, Global};
use std::sync::OnceLock;

use zstdoxide::{
    CompressionStrategy, DecodeOptions, Dictionary, DictionaryHandle, DictionaryView, Encoder,
    EncoderOptions, EndDirective, FrameHeader, MatchSearchOptions, SharedDictionary, StreamStatus,
    decompress, decompress_with_dictionary_in,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

#[allow(dead_code, clippy::should_implement_trait)]
#[path = "upstream/support/mod.rs"]
mod support;

const SOURCE_SIZE: usize = 100 * 1024;
const DICTIONARY_SIZE: usize = 10 * 1024;
const POLICIES: [CompressionStrategy; 3] = [
    CompressionStrategy::Greedy,
    CompressionStrategy::Chain,
    CompressionStrategy::Lazy,
];

fn source() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| support::noise(SOURCE_SIZE, 0.5, 0.0, 0))
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
    assert!(dictionary.is_formatted());
    assert_ne!(dictionary.id(), 0);
    dictionary.into_shared()
}

fn tracking() -> TrackingAllocator {
    TrackingAllocator::new(AllocatorConfig {
        max_live_allocations: 256,
        fail_at: None,
    })
    .unwrap()
}

fn profile() -> MatchSearchOptions {
    let mut search = MatchSearchOptions::new();
    search.set_hash_log(12).unwrap();
    search.set_chain_log(12).unwrap();
    search.set_key_bytes(5).unwrap();
    search.set_chain_visits(4).unwrap();
    search.set_lazy_visits(7).unwrap();
    search.set_history_bytes(16 * 1024).unwrap();
    search
}

fn finish<A: Allocator + Clone, D: DictionaryHandle>(
    encoder: &mut Encoder<A, D>,
    input: &[u8],
) -> Vec<u8> {
    let options = encoder.options();
    let search = encoder.search_options();
    let mut consumed = 0;
    let mut frame = Vec::new();
    for _ in 0..100_000 {
        let mut output = [0xa5; 259];
        let progress = encoder
            .process(
                &input[consumed..],
                &mut output[1..258],
                EndDirective::Finish,
            )
            .unwrap();
        assert!(progress.consumed <= input.len() - consumed);
        assert!(progress.produced <= 257);
        assert_eq!(output[0], 0xa5);
        assert!(output[1 + progress.produced..].iter().all(|&b| b == 0xa5));
        consumed += progress.consumed;
        frame.extend_from_slice(&output[1..1 + progress.produced]);
        assert_eq!(encoder.options(), options);
        assert_eq!(encoder.search_options(), search);
        if progress.status == StreamStatus::FrameFinished {
            assert_eq!(consumed, input.len());
            assert_eq!(encoder.total_input(), input.len() as u64);
            assert_eq!(encoder.total_output(), frame.len() as u64);
            return frame;
        }
        assert_eq!(progress.status, StreamStatus::NeedOutput);
        assert!(progress.consumed != 0 || progress.produced != 0);
    }
    panic!("bounded source-backed lifecycle frame did not finish");
}

fn verify(frame: &[u8], input: &[u8], options: EncoderOptions, dictionary: DictionaryView<'_>) {
    let (header, _) = FrameHeader::parse(frame).unwrap();
    assert_eq!(header.window_size, 1_u64 << options.window_log);
    assert_eq!(header.has_checksum, options.checksum);
    assert_eq!(header.dictionary_id, dictionary.id());
    assert_eq!(
        header.content_size,
        options
            .pledged_size
            .filter(|_| options.include_content_size)
    );
    assert_eq!(
        decompress_with_dictionary_in(frame, &dictionary, DecodeOptions::new(input.len()), Global)
            .unwrap(),
        input
    );
}

/// fuzzer.c:1721–1763: requested getters survive starting a session and resetting
/// it; resetting parameters restores defaults. This checks concrete Rust search
/// settings and named policies, not C's automatic/minimum/numeric values.
#[test]
fn original_getter_start_session_reset_and_parameter_reset_sequence() {
    for strategy in POLICIES {
        let owner = tracking();
        let monitor = owner.monitor();
        let mut encoder = Encoder::new_in(owner);
        assert_eq!(encoder.options(), EncoderOptions::default());
        assert_eq!(encoder.search_options(), MatchSearchOptions::default());
        assert_eq!(encoder.search_options().hash_log(), 16);
        let initial = monitor.snapshot();
        let mut search = encoder.search_options();
        search.set_hash_log(10).unwrap();
        encoder.set_search_options(search).unwrap();
        assert_eq!(encoder.options(), EncoderOptions::default());
        assert_eq!(encoder.search_options().hash_log(), 10);
        let options = EncoderOptions {
            strategy,
            ..EncoderOptions::default()
        };
        encoder.set_options(options).unwrap();
        assert_eq!(encoder.options(), options);
        assert_eq!(encoder.search_options(), search);
        assert_eq!(
            monitor.snapshot(),
            initial,
            "configuration allocated or cloned"
        );

        // Same empty-buffer start used by the original assertion group. Rust's
        // explicit return contract also identifies its pending header output.
        let progress = encoder
            .process(&[], &mut [], EndDirective::Continue)
            .unwrap();
        assert_eq!((progress.consumed, progress.produced), (0, 0));
        assert_eq!(progress.status, StreamStatus::NeedOutput);
        assert_eq!(encoder.options(), options);
        assert_eq!(encoder.search_options(), search);
        let allocated = monitor.snapshot();
        let capacities = (
            encoder.retained_capacity(),
            encoder.search_retained_capacity(),
        );
        encoder.reset();
        assert_eq!(encoder.options(), options);
        assert_eq!(encoder.search_options(), search);
        assert_eq!(monitor.snapshot(), allocated);
        encoder.reset_parameters().unwrap();
        assert_eq!(encoder.options(), EncoderOptions::default());
        assert_eq!(encoder.search_options(), MatchSearchOptions::default());
        assert_eq!(monitor.snapshot(), allocated);
        assert_eq!(
            (
                encoder.retained_capacity(),
                encoder.search_retained_capacity()
            ),
            capacities
        );
        drop(encoder);
        monitor.assert_released();
        assert_eq!(
            monitor.snapshot().owners_dropped,
            monitor.snapshot().identities_created
        );
    }
}

fn hash_attachment<D: DictionaryHandle>(dictionary: D, strategy: CompressionStrategy) {
    let input = &dictionary.view().content()[..512];
    let owner = tracking();
    let monitor = owner.monitor();
    let options = EncoderOptions {
        strategy,
        ..EncoderOptions::default()
    };
    // The constructor determines D for borrowed and shared handles. Clearing
    // this still-fresh selection creates the same empty configuration boundary;
    // no codec frame or dictionary seed has been prepared yet.
    let mut encoder = Encoder::with_dictionary_in(options, dictionary.clone(), owner).unwrap();
    encoder.set_dictionary(None).unwrap();
    let before = monitor.snapshot();
    let mut search = MatchSearchOptions::new();
    search.set_hash_log(12).unwrap();
    encoder.set_search_options(search).unwrap();
    assert_eq!(encoder.search_options().hash_log(), 12);
    encoder.set_dictionary(Some(dictionary.clone())).unwrap();
    assert_eq!(encoder.search_options(), search);
    assert_eq!(encoder.options(), options);
    encoder.set_search_options(search).unwrap();
    assert_eq!(encoder.search_options().hash_log(), 12);
    assert_eq!(encoder.options(), options);
    assert_eq!(
        monitor.snapshot(),
        before,
        "attachment/configuration allocated or cloned workspace owners"
    );

    let frame = finish(&mut encoder, input);
    verify(&frame, input, options, dictionary.view());
    assert!(
        decompress(&frame, input.len()).is_err(),
        "dictionary must supply actual matches or its ID"
    );
    encoder.reset();
    assert_eq!(encoder.search_options(), search);
    assert_eq!(encoder.options(), options);
    let repeated = finish(&mut encoder, input);
    assert_eq!(
        repeated, frame,
        "session reset lost the selected dictionary or search settings"
    );
    verify(&repeated, input, options, dictionary.view());
    drop(encoder);
    monitor.assert_released();
    assert_eq!(
        monitor.snapshot().owners_dropped,
        monitor.snapshot().identities_created
    );
}

/// fuzzer.c:3001–3012 preserves the literal hash-log value 12 on either side
/// of dictionary attachment. Level 1 and C's CDict attachment implementation
/// remain outside this live test. Rust adds retained getters and decoded frames.
#[test]
fn original_hash_twelve_before_and_after_raw_formatted_shared_and_borrowed_attachment() {
    for dictionary in [raw_dictionary(), formatted_dictionary()] {
        assert!(dictionary.content().len() >= 512);
        for strategy in POLICIES {
            hash_attachment(dictionary.clone(), strategy);
            hash_attachment(dictionary.view(), strategy);
        }
    }
}

fn ordered_frame(
    strategy: CompressionStrategy,
    dictionary_first: bool,
) -> (Vec<u8>, SharedDictionary) {
    let dictionary = raw_dictionary();
    let options = EncoderOptions {
        strategy,
        pledged_size: Some(SOURCE_SIZE as u64),
        ..EncoderOptions::default()
    };
    let search = profile();
    let owner = tracking();
    let monitor = owner.monitor();
    let mut encoder = Encoder::new_in(owner);
    let before = monitor.snapshot();
    if dictionary_first {
        encoder.set_dictionary(Some(dictionary.clone())).unwrap();
    }
    encoder.set_options(options).unwrap();
    encoder.set_search_options(search).unwrap();
    if !dictionary_first {
        encoder.set_dictionary(Some(dictionary.clone())).unwrap();
    }
    assert_eq!(encoder.options(), options);
    assert_eq!(encoder.search_options(), search);
    assert_eq!(
        monitor.snapshot(),
        before,
        "fresh ordering allocated or cloned workspace owners"
    );
    let frame = finish(&mut encoder, source());
    verify(&frame, source(), options, dictionary.view());
    assert!(
        decompress(&frame, SOURCE_SIZE).is_err(),
        "raw dictionary must contribute useful matches"
    );
    drop(encoder);
    monitor.assert_released();
    assert_eq!(
        monitor.snapshot().owners_dropped,
        monitor.snapshot().identities_created
    );
    (frame, dictionary)
}

/// fuzzer.c:3013–3031 originally compares compressed sizes at numeric level 7.
/// Retain the 10/100 KiB dimensions and both configuration orders, strengthening
/// equality to complete bytes under each explicitly independent Rust policy.
#[test]
fn original_dictionary_options_order_with_nondefault_search_preserves_complete_frames() {
    for strategy in POLICIES {
        let (first, _) = ordered_frame(strategy, true);
        let (second, _) = ordered_frame(strategy, false);
        assert_eq!(
            first.len(),
            second.len(),
            "{strategy:?}: original size assertion"
        );
        assert_eq!(first, second, "{strategy:?}: full frame assertion");
        eprintln!(
            "original search/dictionary ordering: {strategy:?}, 10240-byte dictionary, 102400-byte source, {}-byte frame",
            first.len()
        );
    }
}

#[test]
#[ignore = "requires pinned zstd v1.5.7 CLI; set ZSTD_BIN; decodes six complete dictionary/order frames"]
fn official_codec_decodes_both_original_dictionary_orders_for_each_named_policy() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let executable = std::env::var_os("ZSTD_BIN").expect("set ZSTD_BIN");
    let version = Command::new(&executable).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("v1.5.7"));
    let directory = std::env::temp_dir().join(format!(
        "zstdoxide-upstream-search-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("dictionary.raw");
    let mut frames = 0;
    for strategy in POLICIES {
        for dictionary_first in [true, false] {
            let (frame, dictionary) = ordered_frame(strategy, dictionary_first);
            // Pass the complete serialized raw dictionary and retain its owner
            // until the child and its input-writer have both completed.
            std::fs::write(&path, dictionary.as_bytes()).unwrap();
            let mut child = Command::new(&executable)
                .args(["-d", "-q", "-c", "-D"])
                .arg(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let mut stdin = child.stdin.take().unwrap();
            let writer = std::thread::spawn(move || stdin.write_all(&frame).unwrap());
            let result = child.wait_with_output().unwrap();
            writer.join().unwrap();
            assert!(
                result.status.success(),
                "{strategy:?}, dictionary_first={dictionary_first}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, source());
            frames += 1;
        }
    }
    assert_eq!(frames, 6);
    std::fs::remove_dir_all(directory).unwrap();
    eprintln!(
        "official v1.5.7 source-backed search lifecycle: {frames} frames/{} source bytes",
        frames * SOURCE_SIZE
    );
}
