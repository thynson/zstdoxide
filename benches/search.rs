//! A 27-row public search benchmark: Global timing, separate allocation survey.
//! Run through scripts/benchmark_search.py; clock-free smoke is the default.
//! Full output checks, fingerprints, corpus/fixture preparation and caller buffers
//! stay outside clocks. Timing includes reset, seeding and history rebuilding.
//! Individual feature builds can enable paths used only by untimed verification
//! or absent from these encoder cases; timing differences do not prove that each
//! backend optimization was exercised. Separate microbench evidence stays separate.
#![forbid(unsafe_code)]

use std::alloc::{Allocator, Global};
use std::hint::black_box;
use std::time::{Duration, Instant};
use zstdoxide::{
    CompressionStrategy, DecodeOptions, Dictionary, DictionaryHandle, DictionaryView, Encoder,
    EncoderOptions, EndDirective, MatchSearchOptions, StreamStatus,
};
use zstdoxide_test_allocator::{AllocationSnapshot, AllocatorConfig, TrackingAllocator};

const SEED: u64 = 0x5eed_5ea2_c120_2026;
const SMALL_BYTES: usize = 32 * 1024;
const HISTORY_BLOCK: usize = 128 * 1024;
const INPUT_CHUNK: usize = 4096;
const OUTPUT_CHUNK: usize = 1024;
const LEDGER_SLOTS: usize = 512;
const FORMATTED: &[u8] = include_bytes!("../tests/upstream/dictionaries/formatted.dict");
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

fn snapshot_json(snapshot: AllocationSnapshot) -> String {
    let s = snapshot;
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

#[derive(Clone, Copy)]
enum Workload {
    Block,
    History,
    Dictionary,
}
const WORKLOADS: [Workload; 3] = [Workload::Block, Workload::History, Workload::Dictionary];
impl Workload {
    fn name(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::History => "history",
            Self::Dictionary => "dictionary",
        }
    }
    fn source(self, corpus: &Corpus) -> &[u8] {
        match self {
            Self::Block => &corpus.block,
            Self::History => &corpus.history,
            Self::Dictionary => &corpus.dictionary_source,
        }
    }
    fn block_count(self) -> usize {
        if matches!(self, Self::History) { 3 } else { 1 }
    }
    fn block_end(self, index: usize) -> usize {
        if matches!(self, Self::History) {
            HISTORY_BLOCK * (index + 1)
        } else {
            SMALL_BYTES
        }
    }
    fn dictionary_bytes(self) -> &'static [u8] {
        if matches!(self, Self::Dictionary) {
            FORMATTED
        } else {
            &[]
        }
    }
}

#[derive(Clone, Copy)]
enum Profile {
    Default,
    Compact,
    Wide,
}
const PROFILES: [Profile; 3] = [Profile::Default, Profile::Compact, Profile::Wide];
impl Profile {
    fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Compact => "compact",
            Self::Wide => "wide",
        }
    }
    fn options(self) -> MatchSearchOptions {
        let mut result = MatchSearchOptions::new();
        let values = match self {
            Self::Default => (16, 18, 4, 8, 16, 131_072),
            Self::Compact => (10, 10, 4, 1, 1, 8192),
            Self::Wide => (18, 19, 8, 32, 64, 1_048_576),
        };
        result.set_hash_log(values.0).unwrap();
        result.set_chain_log(values.1).unwrap();
        result.set_key_bytes(values.2).unwrap();
        result.set_chain_visits(values.3).unwrap();
        result.set_lazy_visits(values.4).unwrap();
        result.set_history_bytes(values.5).unwrap();
        result
    }
}
const POLICIES: [CompressionStrategy; 3] = [
    CompressionStrategy::Greedy,
    CompressionStrategy::Chain,
    CompressionStrategy::Lazy,
];
fn policy_name(strategy: CompressionStrategy) -> &'static str {
    match strategy {
        CompressionStrategy::Greedy => "greedy",
        CompressionStrategy::Chain => "chain",
        CompressionStrategy::Lazy => "lazy",
    }
}
#[derive(Clone, Copy)]
struct Case {
    workload: Workload,
    profile: Profile,
    strategy: CompressionStrategy,
}
fn fingerprint(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |sum, &byte| {
        (sum ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    })
}
fn next(state: &mut u64) -> u8 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state as u8
}
struct Corpus {
    block: Vec<u8>,
    history: Vec<u8>,
    dictionary: Dictionary,
    dictionary_source: Vec<u8>,
}
impl Corpus {
    fn new() -> Self {
        let mut state = SEED;
        let text = b"configured-zstandard-search-workload";
        let mut block = Vec::with_capacity(SMALL_BYTES);
        for index in 0..SMALL_BYTES {
            let random = next(&mut state);
            block.push(if index >= 8192 && index % 509 != 0 {
                block[index - 8192]
            } else if index % 11 < 8 {
                text[index % text.len()]
            } else {
                random
            });
        }
        let mut first_state = SEED ^ 1;
        let mut second_state = SEED ^ 2;
        let first: Vec<_> = (0..HISTORY_BLOCK).map(|_| next(&mut first_state)).collect();
        let second: Vec<_> = (0..HISTORY_BLOCK)
            .map(|_| next(&mut second_state))
            .collect();
        assert_ne!(first, second);
        let history = [&first[..], &second[..], &first[..]].concat();
        let dictionary = Dictionary::from_bytes(FORMATTED).unwrap();
        let content = dictionary.content();
        assert!(content.len() >= 512);
        let mut dictionary_source = Vec::with_capacity(SMALL_BYTES);
        for segment in 0..SMALL_BYTES / 512 {
            let start = (segment * 137 + 29) % (content.len() - 512 + 1);
            dictionary_source.extend_from_slice(&content[start..start + 512]);
        }
        Self {
            block,
            history,
            dictionary,
            dictionary_source,
        }
    }
}

// One context is retained per case and reused. Keeping its variants inline
// avoids adding an unrelated Global box to the allocator survey's setup.
#[allow(
    clippy::large_enum_variant,
    reason = "one reused inline benchmark context"
)]
enum Context<'a, A: Allocator> {
    Plain(Encoder<A>),
    Dictionary(Encoder<A, DictionaryView<'a>>),
}
impl<A: Allocator + Clone> Context<'_, A> {
    fn capacities(&self) -> (usize, usize) {
        match self {
            Self::Plain(context) => (
                context.search_retained_capacity(),
                context.retained_capacity(),
            ),
            Self::Dictionary(context) => (
                context.search_retained_capacity(),
                context.retained_capacity(),
            ),
        }
    }
}
struct Operation<'a, A: Allocator + Clone> {
    case: Case,
    corpus: &'a Corpus,
    context: Context<'a, A>,
    output: Vec<u8>,
    produced: usize,
}
impl<'a, A: Allocator + Clone> Operation<'a, A> {
    fn new(case: Case, corpus: &'a Corpus, allocator: A) -> Self {
        let options = EncoderOptions {
            window_log: 20,
            strategy: case.strategy,
            checksum: true,
            pledged_size: None,
            include_content_size: true,
            include_dictionary_id: true,
        };
        let context = if matches!(case.workload, Workload::Dictionary) {
            let mut value =
                Encoder::with_dictionary_in(options, corpus.dictionary.view(), allocator).unwrap();
            value.set_search_options(case.profile.options()).unwrap();
            Context::Dictionary(value)
        } else {
            let mut value = Encoder::with_options_in(options, allocator).unwrap();
            value.set_search_options(case.profile.options()).unwrap();
            Context::Plain(value)
        };
        // Fixed caller-owned Global buffer is outside codec allocation surveys
        // and clocks. Every frame admits raw fallback plus bounded framing.
        Self {
            case,
            corpus,
            context,
            output: vec![0; 2 * case.workload.source(corpus).len() + 1024],
            produced: 0,
        }
    }
    fn run(&mut self) {
        let source = self.case.workload.source(self.corpus);
        self.produced = match &mut self.context {
            Context::Plain(context) => {
                context.reset();
                encode(context, self.case.workload, source, &mut self.output)
            }
            Context::Dictionary(context) => {
                context.reset();
                encode(context, self.case.workload, source, &mut self.output)
            }
        };
    }
    fn bytes(&self) -> &[u8] {
        &self.output[..self.produced]
    }
}
fn encode<A: Allocator + Clone, D: DictionaryHandle>(
    encoder: &mut Encoder<A, D>,
    workload: Workload,
    input: &[u8],
    output: &mut [u8],
) -> usize {
    let mut consumed = 0;
    let mut produced = 0;
    for block in 0..workload.block_count() {
        let block_end = workload.block_end(block);
        loop {
            let end = (consumed + INPUT_CHUNK).min(block_end);
            let directive = if end < block_end {
                EndDirective::Continue
            } else if block + 1 == workload.block_count() {
                EndDirective::Finish
            } else {
                EndDirective::Flush
            };
            assert!(produced < output.len(), "fixed caller buffer exhausted");
            let output_end = (produced + OUTPUT_CHUNK).min(output.len());
            let progress = encoder
                .process(
                    black_box(&input[consumed..end]),
                    &mut output[produced..output_end],
                    directive,
                )
                .unwrap();
            consumed += progress.consumed;
            produced += progress.produced;
            if matches!(
                progress.status,
                StreamStatus::Flushed | StreamStatus::FrameFinished
            ) {
                assert_eq!(consumed, block_end);
                break;
            }
            assert!(
                progress.consumed != 0 || progress.produced != 0,
                "encoder stalled"
            );
        }
    }
    produced
}
fn verify(case: Case, corpus: &Corpus, frame: &[u8]) {
    let source = case.workload.source(corpus);
    let decoded = if matches!(case.workload, Workload::Dictionary) {
        zstdoxide::decompress_with_dictionary_in(
            frame,
            &corpus.dictionary.view(),
            DecodeOptions::new(source.len()),
            Global,
        )
        .unwrap()
    } else {
        zstdoxide::decompress(frame, source.len()).unwrap()
    };
    assert_eq!(decoded, source);
}
struct Reference {
    bytes: Vec<u8>,
    cold: (usize, usize),
    reused: (usize, usize),
    external_dictionary_capacity: usize,
}
fn reference(case: Case, corpus: &Corpus) -> Reference {
    let mut operation = Operation::new(case, corpus, Global);
    let (search, external_dictionary_capacity) = operation.context.capacities();
    assert_eq!(search, 0);
    if !matches!(case.workload, Workload::Dictionary) {
        assert_eq!(external_dictionary_capacity, 0);
    }
    operation.run();
    verify(case, corpus, operation.bytes());
    let bytes = operation.bytes().to_vec();
    let cold = operation.context.capacities();
    operation.run();
    assert_eq!(operation.bytes(), bytes);
    verify(case, corpus, operation.bytes());
    let reused = operation.context.capacities();
    Reference {
        bytes,
        cold,
        reused,
        external_dictionary_capacity,
    }
}
struct Survey {
    before: AllocationSnapshot,
    cold: AllocationSnapshot,
    reused: AllocationSnapshot,
    final_drop: AllocationSnapshot,
}
fn checked(snapshot: AllocationSnapshot) -> AllocationSnapshot {
    assert!(!snapshot.violations.any());
    assert_eq!(snapshot.failures, 0);
    snapshot
}
fn survey(case: Case, corpus: &Corpus, reference: &Reference) -> Survey {
    let owner = TrackingAllocator::new(AllocatorConfig {
        max_live_allocations: LEDGER_SLOTS,
        fail_at: None,
    })
    .unwrap();
    let monitor = owner.monitor();
    let before = checked(monitor.snapshot());
    let mut operation = Operation::new(case, corpus, owner);
    assert_eq!(monitor.snapshot().live_bytes, 0);
    assert_eq!(
        operation.context.capacities(),
        (0, reference.external_dictionary_capacity)
    );
    operation.run();
    assert_eq!(operation.bytes(), reference.bytes);
    let cold = checked(monitor.snapshot());
    assert_eq!(operation.context.capacities(), reference.cold);
    assert_eq!(
        cold.live_bytes + reference.external_dictionary_capacity as u128,
        reference.cold.1 as u128
    );
    operation.run();
    assert_eq!(operation.bytes(), reference.bytes);
    let reused = checked(monitor.snapshot());
    assert_eq!(operation.context.capacities(), reference.reused);
    assert_eq!(
        reused.live_bytes + reference.external_dictionary_capacity as u128,
        reference.reused.1 as u128
    );
    drop(operation);
    monitor.assert_released();
    let final_drop = checked(monitor.snapshot());
    assert_eq!(final_drop.owners_dropped, final_drop.identities_created);
    Survey {
        before,
        cold,
        reused,
        final_drop,
    }
}
struct Settings {
    timed: bool,
    samples: usize,
    duration: Duration,
}
impl Settings {
    fn read() -> Self {
        fn number(name: &str, default: usize, min: usize, max: usize) -> usize {
            let value = std::env::var(name)
                .map_or(default, |s| s.parse().expect("invalid benchmark scalar"));
            assert!((min..=max).contains(&value));
            value
        }
        let timed = match std::env::var("ZSTDOXIDE_SEARCH_BENCH_TIMED").as_deref() {
            Err(_) | Ok("0") => false,
            Ok("1") => true,
            Ok(_) => panic!("expected explicit timed0/1"),
        };
        Self {
            timed,
            samples: number("ZSTDOXIDE_SEARCH_BENCH_SAMPLES", 5, 5, 50),
            duration: Duration::from_millis(
                number("ZSTDOXIDE_SEARCH_BENCH_MS", 100, 1, 60_000) as u64
            ),
        }
    }
}
struct Sample {
    iterations: u64,
    elapsed_ns: u128,
}
fn measure(settings: &Settings, case: Case, corpus: &Corpus, expected: &[u8]) -> Vec<Sample> {
    if !settings.timed {
        return Vec::new();
    } // Smoke starts no Instant clock.
    let mut operation = Operation::new(case, corpus, Global);
    operation.run();
    assert_eq!(operation.bytes(), expected);
    let mut samples = Vec::with_capacity(settings.samples);
    for _ in 0..settings.samples {
        let start = Instant::now();
        let mut iterations = 0_u64;
        let elapsed_ns = loop {
            operation.run();
            black_box(operation.bytes());
            iterations += 1;
            let elapsed = start.elapsed();
            if elapsed >= settings.duration {
                break elapsed.as_nanos();
            }
        };
        // Verify the final result of THIS sample after capturing elapsed time;
        // no scan/hash/allocator snapshot/formatting executes inside its clock.
        assert_eq!(operation.bytes(), expected);
        samples.push(Sample {
            iterations,
            elapsed_ns,
        });
    }
    samples
}
fn print_row(
    settings: &Settings,
    case: Case,
    corpus: &Corpus,
    reference: &Reference,
    survey: Survey,
    samples: Vec<Sample>,
) {
    let source = case.workload.source(corpus);
    let search = case.profile.options();
    let mut values: Vec<_> = samples
        .iter()
        .map(|s| s.elapsed_ns as f64 / s.iterations as f64 / source.len() as f64)
        .collect();
    values.sort_by(f64::total_cmp);
    let timing = if values.is_empty() {
        "null".to_owned()
    } else {
        let middle = values.len() / 2;
        let median = if values.len() % 2 == 0 {
            (values[middle - 1] + values[middle]) / 2.0
        } else {
            values[middle]
        };
        let min = values[0];
        let max = values[values.len() - 1];
        // Throughput has its own sample distribution. For even sample counts,
        // median(1 / cost) is not 1 / median(cost).
        let mut rates: Vec<_> = values
            .iter()
            .map(|value| 1e9 / (value * 1048576.0))
            .collect();
        rates.sort_by(f64::total_cmp);
        let median_rate = if rates.len() % 2 == 0 {
            (rates[middle - 1] + rates[middle]) / 2.0
        } else {
            rates[middle]
        };
        format!(
            "{{\"minimum_ns_per_byte\":{min},\"median_ns_per_byte\":{median},\"maximum_ns_per_byte\":{max},\"minimum_mib_per_second\":{},\"median_mib_per_second\":{},\"maximum_mib_per_second\":{}}}",
            rates[0],
            median_rate,
            rates[rates.len() - 1]
        )
    };
    let samples = samples
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
            "{{\"record\":\"case\",\"workload\":\"{}\",\"profile\":\"{}\",\"strategy\":\"{}\",\"mode\":\"{}\",",
            "\"timed_allocator\":\"global\",\"timing_scope\":\"warm_reset_seed_rebuild_process\",\"unit\":\"uncompressed_byte\",",
            "\"search\":{{\"hash_log\":{},\"chain_log\":{},\"key_bytes\":{},\"chain_visits\":{},\"lazy_visits\":{},\"history_bytes\":{}}},",
            "\"frame_options\":{{\"window_log\":20,\"checksum\":true,\"pledged_size\":null,\"include_content_size\":true,\"include_dictionary_id\":true}},",
            "\"input_chunk\":{},\"output_chunk\":{},\"block_count\":{},\"source_bytes\":{},\"source_fnv1a64\":\"{:016x}\",",
            "\"dictionary_bytes\":{},\"dictionary_fnv1a64\":\"{:016x}\",\"frame_bytes\":{},\"frame_fnv1a64\":\"{:016x}\",\"compressed_size_ratio\":{},",
            "\"external_dictionary_capacity\":{},\"cold_search_capacity\":{},\"cold_total_capacity\":{},\"reused_search_capacity\":{},\"reused_total_capacity\":{},",
            "\"allocation_survey\":{{\"allocator\":\"tracking_owned_global_backed\",\"before\":{},\"cold\":{},\"reused\":{},\"final_drop\":{}}},\"samples\":[{}],\"timing\":{}}}"
        ),
        case.workload.name(),
        case.profile.name(),
        policy_name(case.strategy),
        if settings.timed { "timed" } else { "smoke" },
        search.hash_log(),
        search.chain_log(),
        search.key_bytes(),
        search.chain_visits(),
        search.lazy_visits(),
        search.history_bytes(),
        INPUT_CHUNK,
        OUTPUT_CHUNK,
        case.workload.block_count(),
        source.len(),
        fingerprint(source),
        case.workload.dictionary_bytes().len(),
        fingerprint(case.workload.dictionary_bytes()),
        reference.bytes.len(),
        fingerprint(&reference.bytes),
        reference.bytes.len() as f64 / source.len() as f64,
        reference.external_dictionary_capacity,
        reference.cold.0,
        reference.cold.1,
        reference.reused.0,
        reference.reused.1,
        snapshot_json(survey.before),
        snapshot_json(survey.cold),
        snapshot_json(survey.reused),
        snapshot_json(survey.final_drop),
        samples,
        timing
    );
}
fn main() {
    let settings = Settings::read();
    let mut features = FEATURES
        .iter()
        .filter_map(|(name, enabled)| enabled.then_some(*name))
        .collect::<Vec<_>>();
    if features.is_empty() {
        features.push("safe");
    }
    let features = features
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"record\":\"metadata\",\"schema_version\":1,\"features\":[{features}],\"seed\":\"{SEED:016x}\",\"samples\":{},\"minimum_sample_ms\":{},\"ledger_slots\":{LEDGER_SLOTS},\"small_bytes\":{SMALL_BYTES},\"history_block_bytes\":{HISTORY_BLOCK},\"dictionary_segment_bytes\":512,\"dictionary_start_stride\":137,\"dictionary_start_offset\":29,\"expected_rows\":27}}",
        settings.samples,
        settings.duration.as_millis()
    );
    let corpus = Corpus::new();
    let mut rows = 0;
    for workload in WORKLOADS {
        for profile in PROFILES {
            for strategy in POLICIES {
                let case = Case {
                    workload,
                    profile,
                    strategy,
                };
                let reference = reference(case, &corpus);
                let survey = survey(case, &corpus, &reference);
                let samples = measure(&settings, case, &corpus, &reference.bytes);
                print_row(&settings, case, &corpus, &reference, survey, samples);
                rows += 1;
            }
        }
    }
    assert_eq!(rows, 27);
}
