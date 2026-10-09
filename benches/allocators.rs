//! Allocator-aware public API comparisons; run through benchmark_allocators.py.
//!
//! Plain value records describe cases, snapshots, and samples. Codec owners keep
//! their exact generic allocator; borrowed prepared views keep the corpus alive.
//! TrackingAllocator wraps Global and its timings include ledger/lock and owner
//! bookkeeping. Its preallocated ledger, all corpus data and caller buffers are
//! outside timing and payload accounting. It is not a different heap allocator.
//! Streaming timings reset/reuse contexts; other timings allocate/drop a fresh
//! result. Full-byte verification and fingerprints are outside every clock.

use std::alloc::{Allocator, Global};
use std::hint::black_box;
use std::time::{Duration, Instant};
use zstdoxide::{
    CompressionStrategy, DecodeOptions, Decoder, Dictionary, DictionaryHandle,
    DictionaryTrainingOptions, DictionaryView, Encoder, EncoderOptions, EndDirective,
    RawTrainingOptions, StreamStatus,
};
use zstdoxide_test_allocator::{
    AllocationMonitor, AllocationSnapshot, AllocatorConfig, TrackingAllocator,
};

const SEED: u64 = 0x5eed_a110_ca70_2026;
const SOURCE_SIZE: usize = 32 * 1024;
const LEDGER_SLOTS: usize = 512;
const ENCODE_INPUT: usize = 4096;
const ENCODE_OUTPUT: usize = 1024;
const DECODE_INPUT: usize = 257;
const DECODE_OUTPUT: usize = 4096;
const FORMATTED: &[u8] = include_bytes!("../tests/upstream/dictionaries/formatted.dict");
const HTTP_DICTIONARY: &[u8] =
    include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
const HTTP_FRAME: &[u8] = include_bytes!("../tests/upstream/data/generated/http-dictionary.zst");
const HTTP_SOURCE: &[u8] = include_bytes!("../tests/upstream/data/golden-compression/http");
const TRAINING: &[u8] = include_bytes!("../tests/upstream/dictionaries/training-samples.bin");
const FEATURES: &[(&str, bool)] = &[
    ("unsafe-byte-cursor", cfg!(feature = "unsafe-byte-cursor")),
    ("unsafe-bitstream", cfg!(feature = "unsafe-bitstream")),
    ("unsafe-history", cfg!(feature = "unsafe-history")),
    ("unsafe-ring-history", cfg!(feature = "unsafe-ring-history")),
    ("unsafe-fse", cfg!(feature = "unsafe-fse")),
    ("unsafe-fse-encoder", cfg!(feature = "unsafe-fse-encoder")),
    ("unsafe-huffman", cfg!(feature = "unsafe-huffman")),
    (
        "unsafe-huffman-codebook",
        cfg!(feature = "unsafe-huffman-codebook"),
    ),
    ("unsafe-match-table", cfg!(feature = "unsafe-match-table")),
    ("unsafe-match-chain", cfg!(feature = "unsafe-match-chain")),
    ("unsafe-dictionary", cfg!(feature = "unsafe-dictionary")),
    ("unsafe-training", cfg!(feature = "unsafe-training")),
];

#[derive(Clone, Copy)]
enum Case {
    Compress,
    Decompress,
    StreamGreedy,
    StreamChain,
    StreamLazy,
    StreamDecode,
    PrepareDictionary,
    DictionaryEncode,
    DictionaryDecode,
    RawTraining,
    FormattedTraining,
}

const CASES: [Case; 11] = [
    Case::Compress,
    Case::Decompress,
    Case::StreamGreedy,
    Case::StreamChain,
    Case::StreamLazy,
    Case::StreamDecode,
    Case::PrepareDictionary,
    Case::DictionaryEncode,
    Case::DictionaryDecode,
    Case::RawTraining,
    Case::FormattedTraining,
];

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Compress => "one_shot_encode",
            Self::Decompress => "one_shot_decode",
            Self::StreamGreedy => "stream_encode_greedy",
            Self::StreamChain => "stream_encode_chain",
            Self::StreamLazy => "stream_encode_lazy",
            Self::StreamDecode => "stream_decode",
            Self::PrepareDictionary => "prepare_dictionary",
            Self::DictionaryEncode => "dictionary_stream_encode",
            Self::DictionaryDecode => "dictionary_stream_decode",
            Self::RawTraining => "raw_training",
            Self::FormattedTraining => "formatted_training",
        }
    }

    fn streaming(self) -> bool {
        matches!(
            self,
            Self::StreamGreedy
                | Self::StreamChain
                | Self::StreamLazy
                | Self::StreamDecode
                | Self::DictionaryEncode
                | Self::DictionaryDecode
        )
    }

    fn source(self, corpus: &Corpus) -> &[u8] {
        match self {
            Self::PrepareDictionary => FORMATTED,
            Self::DictionaryEncode => &corpus.dictionary_source,
            Self::DictionaryDecode => HTTP_SOURCE,
            Self::RawTraining | Self::FormattedTraining => &TRAINING[..SOURCE_SIZE],
            _ => &corpus.source,
        }
    }

    fn input(self, corpus: &Corpus) -> &[u8] {
        match self {
            Self::Decompress | Self::StreamDecode => &corpus.frame,
            Self::DictionaryDecode => HTTP_FRAME,
            _ => self.source(corpus),
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Self::PrepareDictionary => "serialized_dictionary_byte",
            Self::RawTraining | Self::FormattedTraining => "sample_byte",
            _ => "uncompressed_byte",
        }
    }

    fn dictionary_bytes(self) -> &'static [u8] {
        match self {
            Self::DictionaryEncode => FORMATTED,
            Self::DictionaryDecode => HTTP_DICTIONARY,
            _ => &[],
        }
    }

    fn retained_scope(self) -> &'static str {
        match self {
            Self::DictionaryEncode | Self::DictionaryDecode => {
                "reachable_context_including_external_borrowed_dictionary"
            }
            _ if self.streaming() => "context_owned_heap_capacity",
            Self::Compress | Self::Decompress => "result_vector_capacity",
            _ => "not_exposed_by_public_dictionary_api",
        }
    }
}

struct Corpus {
    source: Vec<u8>,
    frame: Vec<u8>,
    dictionary: Dictionary,
    http_dictionary: Dictionary,
    dictionary_source: Vec<u8>,
    samples: Vec<&'static [u8]>,
}

impl Corpus {
    fn new() -> Self {
        let mut state = SEED;
        let mut source = Vec::with_capacity(SOURCE_SIZE);
        for index in 0..SOURCE_SIZE {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let byte = if index >= 8192 && index % 509 != 0 {
                source[index - 8192]
            } else if index % 11 < 8 {
                b"allocator-aware-zstandard-ownership"[index % 34]
            } else {
                state as u8
            };
            source.push(byte);
        }
        let frame = zstdoxide::compress(&source).unwrap();
        assert_eq!(zstdoxide::decompress(&frame, source.len()).unwrap(), source);
        let dictionary = Dictionary::from_bytes(FORMATTED).unwrap();
        let http_dictionary = Dictionary::from_bytes(HTTP_DICTIONARY).unwrap();
        let content = dictionary.content();
        let dictionary_source = (0..SOURCE_SIZE)
            .map(|index| content[(index * 17 + index / 127) % content.len()])
            .collect();
        let samples = TRAINING[..SOURCE_SIZE]
            .as_chunks::<512>()
            .0
            .iter()
            .map(|sample| sample.as_slice())
            .collect();
        Self {
            source,
            frame,
            dictionary,
            http_dictionary,
            dictionary_source,
            samples,
        }
    }
}

fn training_options() -> RawTrainingOptions {
    RawTrainingOptions {
        dictionary_size: 2048,
        max_sample_bytes: SOURCE_SIZE,
        segment_size: 64,
    }
}

// Generic contexts retain their own allocator values. DictionaryView erases
// only the prepared owner's allocator type, never allocation ownership.
// Keep the selected context inline: boxing would introduce a benchmark-owned
// Global allocation and indirection absent from the public API under comparison.
#[allow(clippy::large_enum_variant)]
enum Context<'a, A: Allocator> {
    None,
    Encode(Encoder<A>),
    Decode(Decoder<A>),
    DictionaryEncode(Encoder<A, DictionaryView<'a>>),
    DictionaryDecode(Decoder<A, DictionaryView<'a>>),
}

enum Product<A: Allocator> {
    None,
    Bytes(Vec<u8, A>),
    Dictionary(Dictionary<A>),
}

struct Operation<'a, A: Allocator + Clone> {
    case: Case,
    corpus: &'a Corpus,
    allocator: A,
    context: Context<'a, A>,
    product: Product<A>,
    caller_output: Vec<u8>,
    produced: usize,
}

impl<'a, A: Allocator + Clone> Operation<'a, A> {
    fn new(case: Case, corpus: &'a Corpus, allocator: A) -> Self {
        let strategy = match case {
            Case::StreamChain => CompressionStrategy::Chain,
            Case::StreamLazy => CompressionStrategy::Lazy,
            _ => CompressionStrategy::Greedy,
        };
        let options = EncoderOptions {
            strategy,
            ..EncoderOptions::default()
        };
        let context = match case {
            Case::StreamGreedy | Case::StreamChain | Case::StreamLazy => {
                Context::Encode(Encoder::with_options_in(options, allocator.clone()).unwrap())
            }
            Case::StreamDecode => Context::Decode(Decoder::new_in(
                DecodeOptions::new(SOURCE_SIZE),
                allocator.clone(),
            )),
            Case::DictionaryEncode => Context::DictionaryEncode(
                Encoder::with_dictionary_in(options, corpus.dictionary.view(), allocator.clone())
                    .unwrap(),
            ),
            Case::DictionaryDecode => Context::DictionaryDecode(Decoder::with_dictionary_in(
                DecodeOptions::new(HTTP_SOURCE.len()),
                corpus.http_dictionary.view(),
                allocator.clone(),
            )),
            _ => Context::None,
        };
        // Global caller-owned buffers are fixed before timing or payload surveys.
        let caller_output = if case.streaming() {
            vec![0; SOURCE_SIZE * 2 + 1024]
        } else {
            Vec::new()
        };
        Self {
            case,
            corpus,
            allocator,
            context,
            product: Product::None,
            caller_output,
            produced: 0,
        }
    }

    fn run(&mut self) {
        self.product = Product::None;
        match &mut self.context {
            Context::Encode(encoder) => {
                encoder.reset();
                self.produced = encode_frame(encoder, &self.corpus.source, &mut self.caller_output);
            }
            Context::Decode(decoder) => {
                decoder.reset();
                self.produced = decode_frame(decoder, &self.corpus.frame, &mut self.caller_output);
            }
            Context::DictionaryEncode(encoder) => {
                encoder.reset();
                self.produced = encode_frame(
                    encoder,
                    &self.corpus.dictionary_source,
                    &mut self.caller_output,
                );
            }
            Context::DictionaryDecode(decoder) => {
                decoder.reset();
                self.produced = decode_frame(decoder, HTTP_FRAME, &mut self.caller_output);
            }
            Context::None => {
                self.product = match self.case {
                    Case::Compress => Product::Bytes(
                        zstdoxide::compress_in(
                            black_box(&self.corpus.source),
                            self.allocator.clone(),
                        )
                        .unwrap(),
                    ),
                    Case::Decompress => Product::Bytes(
                        zstdoxide::decompress_in(
                            black_box(&self.corpus.frame),
                            SOURCE_SIZE,
                            self.allocator.clone(),
                        )
                        .unwrap(),
                    ),
                    Case::PrepareDictionary => Product::Dictionary(
                        Dictionary::from_bytes_in(black_box(FORMATTED), self.allocator.clone())
                            .unwrap(),
                    ),
                    Case::RawTraining => Product::Dictionary(
                        zstdoxide::train_raw_dictionary_in(
                            black_box(&self.corpus.samples),
                            training_options(),
                            self.allocator.clone(),
                        )
                        .unwrap(),
                    ),
                    Case::FormattedTraining => Product::Dictionary(
                        zstdoxide::train_dictionary_in(
                            black_box(&self.corpus.samples),
                            DictionaryTrainingOptions {
                                selection: training_options(),
                                dictionary_id: Some(0x0012_3456),
                            },
                            self.allocator.clone(),
                        )
                        .unwrap(),
                    ),
                    _ => unreachable!(),
                };
            }
        }
    }

    fn bytes(&self) -> &[u8] {
        match &self.product {
            Product::Bytes(bytes) => bytes,
            Product::Dictionary(dictionary) => dictionary.as_bytes(),
            Product::None => &self.caller_output[..self.produced],
        }
    }

    fn capacity(&self) -> Option<usize> {
        match &self.context {
            Context::Encode(value) => Some(value.retained_capacity()),
            Context::Decode(value) => Some(value.retained_capacity()),
            Context::DictionaryEncode(value) => Some(value.retained_capacity()),
            Context::DictionaryDecode(value) => Some(value.retained_capacity()),
            Context::None => match &self.product {
                Product::Bytes(bytes) => Some(bytes.capacity()),
                _ => None,
            },
        }
    }

    fn reset(&mut self) {
        match &mut self.context {
            Context::Encode(value) => value.reset(),
            Context::Decode(value) => value.reset(),
            Context::DictionaryEncode(value) => value.reset(),
            Context::DictionaryDecode(value) => value.reset(),
            Context::None => {}
        }
    }

    fn timed_iteration(&mut self) {
        self.run();
        // No scan, hash, allocation snapshot, or correctness comparison here.
        black_box(self.bytes());
        // Cold operations include destruction. Streaming contexts stay warm.
        self.product = Product::None;
    }
}

fn encode_frame<A: Allocator + Clone, D: DictionaryHandle>(
    encoder: &mut Encoder<A, D>,
    input: &[u8],
    output: &mut [u8],
) -> usize {
    let mut consumed = 0;
    let mut produced = 0;
    loop {
        let end = (consumed + ENCODE_INPUT).min(input.len());
        let end_output = (produced + ENCODE_OUTPUT).min(output.len());
        let directive = if end == input.len() {
            EndDirective::Finish
        } else {
            EndDirective::Continue
        };
        let progress = encoder
            .process(
                black_box(&input[consumed..end]),
                &mut output[produced..end_output],
                directive,
            )
            .unwrap();
        consumed += progress.consumed;
        produced += progress.produced;
        if progress.status == StreamStatus::FrameFinished {
            return produced;
        }
        assert!(
            progress.consumed != 0 || progress.produced != 0,
            "encoder stalled"
        );
    }
}

fn decode_frame<A: Allocator + Clone, D: DictionaryHandle>(
    decoder: &mut Decoder<A, D>,
    input: &[u8],
    output: &mut [u8],
) -> usize {
    let mut consumed = 0;
    let mut produced = 0;
    loop {
        let end = (consumed + DECODE_INPUT).min(input.len());
        let end_output = (produced + DECODE_OUTPUT).min(output.len());
        let progress = decoder
            .process(
                black_box(&input[consumed..end]),
                &mut output[produced..end_output],
            )
            .unwrap();
        consumed += progress.consumed;
        produced += progress.produced;
        if progress.status == StreamStatus::FrameFinished {
            decoder.finish().unwrap();
            return produced;
        }
        assert!(
            progress.consumed != 0 || progress.produced != 0,
            "decoder stalled"
        );
    }
}

fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |value, byte| {
        (value ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn reference(case: Case, corpus: &Corpus) -> Vec<u8> {
    let mut operation = Operation::new(case, corpus, Global);
    operation.run();
    let result = operation.bytes().to_vec();
    match case {
        Case::Compress | Case::StreamGreedy | Case::StreamChain | Case::StreamLazy => assert_eq!(
            zstdoxide::decompress(&result, SOURCE_SIZE).unwrap(),
            corpus.source
        ),
        Case::DictionaryEncode => assert_eq!(
            zstdoxide::decompress_with_dictionary_in(
                &result,
                &corpus.dictionary.view(),
                DecodeOptions::new(SOURCE_SIZE),
                Global
            )
            .unwrap(),
            corpus.dictionary_source
        ),
        Case::Decompress | Case::StreamDecode => assert_eq!(result, corpus.source),
        Case::DictionaryDecode => assert_eq!(result, HTTP_SOURCE),
        Case::PrepareDictionary => assert_eq!(result, FORMATTED),
        Case::RawTraining => assert!((8..=2048).contains(&result.len())),
        Case::FormattedTraining => {
            let dictionary = Dictionary::from_bytes(&result).unwrap();
            assert!(dictionary.is_formatted());
            assert_eq!(dictionary.id(), 0x0012_3456);
        }
    }
    result
}

struct Survey {
    before: Option<AllocationSnapshot>,
    cold: Option<AllocationSnapshot>,
    reset: Option<AllocationSnapshot>,
    final_drop: Option<AllocationSnapshot>,
    cold_capacity: Option<usize>,
    reset_capacity: Option<usize>,
}

fn checked_snapshot(monitor: Option<&AllocationMonitor>) -> Option<AllocationSnapshot> {
    monitor.map(|monitor| {
        let snapshot = monitor.snapshot();
        assert!(!snapshot.violations.any());
        assert_eq!(snapshot.failures, 0);
        snapshot
    })
}

fn survey<A: Allocator + Clone>(
    case: Case,
    corpus: &Corpus,
    expected: &[u8],
    allocator: A,
    monitor: Option<&AllocationMonitor>,
) -> Survey {
    let before = checked_snapshot(monitor);
    let mut operation = Operation::new(case, corpus, allocator);
    operation.run();
    assert_eq!(operation.bytes(), expected);
    let cold_capacity = operation.capacity();
    let cold = checked_snapshot(monitor);
    let (reset, reset_capacity) = if case.streaming() {
        operation.reset();
        (checked_snapshot(monitor), operation.capacity())
    } else {
        (None, None)
    };
    drop(operation);
    if let Some(monitor) = monitor {
        monitor.assert_released();
    }
    Survey {
        before,
        cold,
        reset,
        final_drop: checked_snapshot(monitor),
        cold_capacity,
        reset_capacity,
    }
}

struct Settings {
    smoke: bool,
    samples: usize,
    duration: Duration,
}
impl Settings {
    fn read() -> Self {
        fn number(name: &str, default: usize, range: std::ops::RangeInclusive<usize>) -> usize {
            let value = std::env::var(name).map_or(default, |value| {
                value.parse().expect("invalid benchmark number")
            });
            assert!(
                range.contains(&value),
                "out-of-range benchmark setting: {name}"
            );
            value
        }
        Self {
            smoke: std::env::var("ZSTDOXIDE_ALLOCATOR_BENCH_SMOKE").is_ok_and(|value| value == "1"),
            samples: number("ZSTDOXIDE_ALLOCATOR_BENCH_SAMPLES", 5, 5..=50),
            duration: Duration::from_millis(
                number("ZSTDOXIDE_ALLOCATOR_BENCH_MS", 150, 1..=60_000) as u64,
            ),
        }
    }
}

#[derive(Debug)]
struct Sample {
    iterations: u64,
    elapsed_ns: u128,
}

fn measurements<A: Allocator + Clone>(
    settings: &Settings,
    case: Case,
    corpus: &Corpus,
    expected: &[u8],
    allocator: A,
    monitor: Option<&AllocationMonitor>,
) -> Vec<Sample> {
    let mut operation = Operation::new(case, corpus, allocator);
    operation.run();
    assert_eq!(operation.bytes(), expected);
    operation.product = Product::None;
    let mut samples = Vec::with_capacity(settings.samples);
    if !settings.smoke {
        for _ in 0..settings.samples {
            let start = Instant::now();
            let mut iterations = 0_u64;
            loop {
                operation.timed_iteration();
                iterations += 1;
                if start.elapsed() >= settings.duration {
                    break;
                }
            }
            samples.push(Sample {
                iterations,
                elapsed_ns: start.elapsed().as_nanos(),
            });
        }
    }
    // Verify another complete result after every timed campaign (or in smoke).
    operation.run();
    assert_eq!(operation.bytes(), expected);
    drop(operation);
    if let Some(monitor) = monitor {
        monitor.assert_released();
        checked_snapshot(Some(monitor));
    }
    samples
}

fn tracking() -> TrackingAllocator {
    TrackingAllocator::new(AllocatorConfig {
        max_live_allocations: LEDGER_SLOTS,
        fail_at: None,
    })
    .unwrap()
}

fn snapshot_json(snapshot: Option<AllocationSnapshot>) -> String {
    let Some(s) = snapshot else {
        return "null".to_owned();
    };
    format!(
        concat!(
            "{{\"allocate_calls\":{},\"allocate_zeroed_calls\":{},\"grow_calls\":{},",
            "\"grow_zeroed_calls\":{},\"shrink_calls\":{},\"deallocate_calls\":{},",
            "\"attempts\":{},\"successes\":{},\"failures\":{},\"zero_size_requests\":{},",
            "\"requested_bytes\":{},\"live_blocks\":{},\"peak_live_blocks\":{},",
            "\"live_requested_bytes\":{},\"peak_requested_bytes\":{},\"live_bytes\":{},",
            "\"peak_bytes\":{},\"identities_created\":{},\"owners_dropped\":{},\"violations\":false}}"
        ),
        s.allocate_calls,
        s.allocate_zeroed_calls,
        s.grow_calls,
        s.grow_zeroed_calls,
        s.shrink_calls,
        s.deallocate_calls,
        s.attempts,
        s.successes,
        s.failures,
        s.zero_size_requests,
        s.requested_bytes,
        s.live_blocks,
        s.peak_live_blocks,
        s.live_requested_bytes,
        s.peak_requested_bytes,
        s.live_bytes,
        s.peak_bytes,
        s.identities_created,
        s.owners_dropped
    )
}

fn optional(value: Option<usize>) -> String {
    value.map_or_else(|| "null".into(), |value| value.to_string())
}

fn print_row(
    settings: &Settings,
    case: Case,
    mode: &str,
    corpus: &Corpus,
    expected: &[u8],
    survey: Survey,
    samples: Vec<Sample>,
) {
    let units = case.source(corpus).len();
    let mut values: Vec<_> = samples
        .iter()
        .map(|sample| sample.elapsed_ns as f64 / sample.iterations as f64 / units as f64)
        .collect();
    values.sort_by(f64::total_cmp);
    let stats = if values.is_empty() {
        "null".to_owned()
    } else {
        let middle = values.len() / 2;
        let median = if values.len() % 2 == 0 {
            (values[middle - 1] + values[middle]) / 2.0
        } else {
            values[middle]
        };
        format!(
            "{{\"median_ns_per_byte\":{median},\"minimum_ns_per_byte\":{},\"maximum_ns_per_byte\":{}}}",
            values[0],
            values[values.len() - 1]
        )
    };
    let samples_json = samples
        .iter()
        .map(|s| {
            format!(
                "{{\"iterations\":{},\"elapsed_ns\":{}}}",
                s.iterations, s.elapsed_ns
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        concat!(
            "{{\"record\":\"case\",\"case\":\"{}\",\"allocator\":\"{}\",\"mode\":\"{}\",",
            "\"timing_scope\":\"{}\",\"unit\":\"{}\",\"input_bytes\":{},\"input_fnv1a64\":\"{:016x}\",\"source_bytes\":{},\"source_fnv1a64\":\"{:016x}\",",
            "\"dictionary_bytes\":{},\"dictionary_fnv1a64\":\"{:016x}\",\"output_bytes\":{},\"output_fnv1a64\":\"{:016x}\",",
            "\"retained_scope\":\"{}\",\"cold_capacity\":{},\"reset_capacity\":{},",
            "\"allocation_survey\":{{\"before\":{},\"cold\":{},\"reset\":{},\"final_drop\":{}}},",
            "\"samples\":[{}],\"timing\":{}}}"
        ),
        case.name(),
        mode,
        if settings.smoke { "smoke" } else { "timed" },
        if case.streaming() {
            "warm_reset_process"
        } else {
            "fresh_allocate_drop"
        },
        case.unit(),
        case.input(corpus).len(),
        fingerprint(case.input(corpus)),
        units,
        fingerprint(case.source(corpus)),
        case.dictionary_bytes().len(),
        fingerprint(case.dictionary_bytes()),
        expected.len(),
        fingerprint(expected),
        case.retained_scope(),
        optional(survey.cold_capacity),
        optional(survey.reset_capacity),
        snapshot_json(survey.before),
        snapshot_json(survey.cold),
        snapshot_json(survey.reset),
        snapshot_json(survey.final_drop),
        samples_json,
        stats
    );
}

fn main() {
    let settings = Settings::read();
    let features = FEATURES
        .iter()
        .filter_map(|(name, enabled)| enabled.then_some(*name))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"record\":\"metadata\",\"schema_version\":1,\"features\":\"{}\",\"seed\":\"{SEED:016x}\",\"samples\":{},\"minimum_sample_ms\":{},\"ledger_slots\":{LEDGER_SLOTS},\"source_size\":{SOURCE_SIZE},\"training_samples\":64,\"training_sample_bytes\":512,\"dictionary_size_cap\":2048,\"training_segment_size\":64}}",
        if features.is_empty() {
            "safe"
        } else {
            &features
        },
        settings.samples,
        settings.duration.as_millis()
    );
    let corpus = Corpus::new();
    for case in CASES {
        let expected = reference(case, &corpus);
        let global_survey = survey(case, &corpus, &expected, Global, None);
        let samples = measurements(&settings, case, &corpus, &expected, Global, None);
        print_row(
            &settings,
            case,
            "global",
            &corpus,
            &expected,
            global_survey,
            samples,
        );

        let allocator = tracking();
        let monitor = allocator.monitor();
        let owned_survey = survey(case, &corpus, &expected, allocator, Some(&monitor));
        let timer_allocator = tracking();
        let timer_monitor = timer_allocator.monitor();
        let samples = measurements(
            &settings,
            case,
            &corpus,
            &expected,
            timer_allocator,
            Some(&timer_monitor),
        );
        print_row(
            &settings,
            case,
            "tracking_owned",
            &corpus,
            &expected,
            owned_survey,
            samples,
        );

        let allocator = tracking().into_non_clone();
        let monitor = allocator.monitor();
        let mut borrowed_survey = survey(case, &corpus, &expected, &allocator, Some(&monitor));
        drop(allocator);
        borrowed_survey.final_drop = checked_snapshot(Some(&monitor));
        monitor.assert_released();
        let timer_allocator = tracking().into_non_clone();
        let timer_monitor = timer_allocator.monitor();
        let samples = measurements(
            &settings,
            case,
            &corpus,
            &expected,
            &timer_allocator,
            Some(&timer_monitor),
        );
        drop(timer_allocator);
        timer_monitor.assert_released();
        print_row(
            &settings,
            case,
            "tracking_borrowed",
            &corpus,
            &expected,
            borrowed_survey,
            samples,
        );
    }
}
