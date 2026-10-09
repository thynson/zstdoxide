//! Public witnesses for the independent Rust match-search controls.
//!
//! Deterministic workloads isolate storage, key, effort, and retained-history
//! settings through complete public frames. They do not reproduce C candidate
//! selection, numeric strategies, or compression ratios. The optional pinned-C
//! oracle independently decodes emitted frames with their complete dictionaries.
#![forbid(unsafe_code)]

use std::alloc::Allocator;
use std::num::NonZeroU64;

use zstdoxide::{
    CompressionStrategy, DecodeOptions, Dictionary, Encoder, EncoderOptions, EndDirective, Error,
    FrameHeader, MatchSearchOptions, Result, SharedDictionary, StreamStatus, decompress,
    decompress_with_dictionary,
};
use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

#[allow(dead_code, clippy::should_implement_trait)]
#[path = "upstream/support/mod.rs"]
mod support;

const POLICIES: [CompressionStrategy; 3] = [
    CompressionStrategy::Greedy,
    CompressionStrategy::Chain,
    CompressionStrategy::Lazy,
];

#[derive(Clone)]
struct Case {
    name: String,
    strategy: CompressionStrategy,
    search: MatchSearchOptions,
    dictionary: Option<SharedDictionary>,
    blocks: Vec<Vec<u8>>,
    window_log: u8,
}

impl Case {
    fn options(&self) -> EncoderOptions {
        EncoderOptions {
            strategy: self.strategy,
            window_log: self.window_log,
            ..EncoderOptions::default()
        }
    }

    fn source(&self) -> Vec<u8> {
        self.blocks.concat()
    }
}

struct Observed {
    frame: Vec<u8>,
    search_capacity: usize,
    total_capacity: usize,
}

fn append<A: Allocator + Clone>(
    encoder: &mut Encoder<A>,
    source: &[u8],
    directive: EndDirective,
    output_chunk: usize,
    encoded: &mut Vec<u8>,
) -> Result<()> {
    assert!(output_chunk != 0);
    let mut consumed = 0;
    let mut destination = vec![0xa5; output_chunk + 2];
    for _ in 0..1_000_000 {
        destination.fill(0xa5);
        let progress = encoder.process(
            &source[consumed..],
            &mut destination[1..=output_chunk],
            directive,
        )?;
        assert!(progress.consumed <= source.len() - consumed);
        assert!(progress.produced <= output_chunk);
        assert_eq!(destination[0], 0xa5);
        assert_eq!(destination[output_chunk + 1], 0xa5);
        consumed += progress.consumed;
        encoded.extend_from_slice(&destination[1..1 + progress.produced]);
        match progress.status {
            StreamStatus::FrameFinished | StreamStatus::Flushed => {
                assert_eq!(consumed, source.len());
                return Ok(());
            }
            StreamStatus::NeedInput if directive == EndDirective::Continue => {
                assert_eq!(consumed, source.len());
                return Ok(());
            }
            _ => assert!(progress.consumed != 0 || progress.produced != 0),
        }
    }
    panic!("bounded frame drain did not finish")
}

fn run_blocks<A: Allocator + Clone>(
    encoder: &mut Encoder<A>,
    blocks: &[Vec<u8>],
    input_chunk: usize,
    output_chunk: usize,
) -> Result<Vec<u8>> {
    assert!(input_chunk != 0 && !blocks.is_empty());
    let mut frame = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        for part in block.chunks(input_chunk) {
            append(
                encoder,
                part,
                EndDirective::Continue,
                output_chunk,
                &mut frame,
            )?;
        }
        append(
            encoder,
            &[],
            if index + 1 == blocks.len() {
                EndDirective::Finish
            } else {
                EndDirective::Flush
            },
            output_chunk,
            &mut frame,
        )?;
    }
    assert_eq!(
        encoder.total_input(),
        blocks.iter().map(|block| block.len() as u64).sum::<u64>()
    );
    assert_eq!(encoder.total_output(), frame.len() as u64);
    Ok(frame)
}

fn check_frame(case: &Case, frame: &[u8]) {
    let source = case.source();
    let (header, _) = FrameHeader::parse(frame).unwrap();
    assert_eq!(
        header.window_size,
        1_u64 << case.window_log,
        "{}",
        case.name
    );
    assert!(header.has_checksum);
    let decoded = match &case.dictionary {
        Some(dictionary) => {
            decompress_with_dictionary(frame, dictionary, DecodeOptions::new(source.len())).unwrap()
        }
        None => decompress(frame, source.len()).unwrap(),
    };
    assert_eq!(decoded, source, "{}", case.name);
}

fn encode_case(case: &Case, input_chunk: usize, output_chunk: usize) -> Observed {
    let mut encoder = match &case.dictionary {
        Some(dictionary) => Encoder::with_dictionary(case.options(), dictionary.clone()).unwrap(),
        None => Encoder::with_options(case.options()).unwrap(),
    };
    encoder.set_search_options(case.search).unwrap();
    let frame = run_blocks(&mut encoder, &case.blocks, input_chunk, output_chunk).unwrap();
    check_frame(case, &frame);
    Observed {
        frame,
        search_capacity: encoder.search_retained_capacity(),
        total_capacity: encoder.retained_capacity(),
    }
}

fn compact_search() -> MatchSearchOptions {
    let mut search = MatchSearchOptions::new();
    search.set_hash_log(14).unwrap();
    search.set_chain_log(12).unwrap();
    search.set_history_bytes(4096).unwrap();
    search
}

fn width_cases(tail_length: usize) -> Vec<Case> {
    // Declared Rust public-API workload: pinned RDG implementation, seed 17,
    // 512 incompressible bytes. This is not an original upstream test schedule.
    let body = support::noise(512, 0.0, 0.0, 17);
    let tail = &b"KEYbytes"[..tail_length];
    let mut content = tail.to_vec();
    content.push(0xff);
    content.extend_from_slice(&body);
    content.push(tail[0] ^ 0xff); // Stop the long match before the isolated tail.
    let dictionary = Dictionary::raw(&content).unwrap().into_shared();
    let mut source = body;
    source.extend_from_slice(tail);
    [4, 5, 8]
        .into_iter()
        .map(|width| {
            let mut search = compact_search();
            search.set_key_bytes(width).unwrap();
            Case {
                name: format!("tail{tail_length}-width{width}"),
                strategy: CompressionStrategy::Chain,
                search,
                dictionary: Some(dictionary.clone()),
                blocks: vec![source.clone()],
                window_log: 17,
            }
        })
        .collect()
}

fn effort_cases(lazy: bool) -> Vec<Case> {
    let pattern: Vec<u8> = (0..80).collect();
    let mut content = Vec::new();
    // Every record starts with the same four bytes. Newest is shortest, so
    // larger effort reaches older, longer independently verifiable matches.
    for rank in (0..64).rev() {
        content.extend_from_slice(&pattern[..4 + rank]);
        content.push(0xff);
    }
    let source = if lazy {
        // Current has a four-byte match. The next-position ladder makes Lazy's
        // own effort observable independently of Chain's selected visit field.
        content.push(b'X');
        content.extend_from_slice(&pattern[..3]);
        content.push(0xff);
        let mut source = vec![b'X'];
        source.extend_from_slice(&pattern);
        source
    } else {
        pattern
    };
    let dictionary = Dictionary::raw(&content).unwrap().into_shared();
    [1, 8, 16, 64]
        .into_iter()
        .map(|visits| {
            let mut search = compact_search();
            if lazy {
                search.set_lazy_visits(visits).unwrap();
            } else {
                search.set_chain_visits(visits).unwrap();
            }
            Case {
                name: format!("{}-visits{visits}", if lazy { "lazy" } else { "chain" }),
                strategy: if lazy {
                    CompressionStrategy::Lazy
                } else {
                    CompressionStrategy::Chain
                },
                search,
                dictionary: Some(dictionary.clone()),
                blocks: vec![source.clone()],
                window_log: 17,
            }
        })
        .collect()
}

fn history_cases(strategy: CompressionStrategy) -> Vec<Case> {
    let first = support::noise(32 * 1024, 0.0, 0.0, 0x7135);
    let middle = support::noise(32 * 1024, 0.0, 0.0, 0x294a);
    [8 * 1024, 64 * 1024]
        .into_iter()
        .map(|history| {
            let mut search = compact_search();
            search.set_hash_log(18).unwrap();
            search.set_chain_log(17).unwrap();
            search.set_history_bytes(history).unwrap();
            Case {
                name: format!("{strategy:?}-history{history}"),
                strategy,
                search,
                dictionary: None,
                blocks: vec![first.clone(), middle.clone(), first.clone()],
                window_log: 18,
            }
        })
        .collect()
}

fn hash_collision_cases() -> Vec<Case> {
    let mut source = support::noise(128, 0.0, 0.0, 41);
    source[..4].fill(0);
    let mut content = source.clone();
    content.push(0xff);
    // Independently observed with the pinned C hash4/direct/generic zero-salt
    // wrappers: these bytes and the zero prefix have buckets 0/0 at log 10
    // and 0/4 at log 14. Full byte verification must reject the false candidate.
    // This known hash relation predicts no encoded bytes or compression ratio.
    content.extend_from_slice(&[0, 0, 0x10, 0xf5]);
    let dictionary = Dictionary::raw(&content).unwrap().into_shared();
    [10, 14]
        .into_iter()
        .map(|log| {
            let mut search = compact_search();
            search.set_hash_log(log).unwrap();
            Case {
                name: format!("greedy-hash-collision-log{log}"),
                strategy: CompressionStrategy::Greedy,
                search,
                dictionary: Some(dictionary.clone()),
                blocks: vec![source.clone()],
                window_log: 17,
            }
        })
        .collect()
}

fn node_eviction_cases() -> Vec<Case> {
    // More than 1024 starts are reseeded in ascending order. A 1024-node ring
    // cannot retain the first source start, while 4096 nodes can. Fixed history
    // and wire windows admit that source under both configurations.
    let first = support::noise(2048, 0.0, 0.0, 0x7493);
    let repeated = first[..128].to_vec();
    [10, 12]
        .into_iter()
        .map(|log| {
            let mut search = compact_search();
            search.set_hash_log(18).unwrap();
            search.set_chain_log(log).unwrap();
            search.set_history_bytes(4096).unwrap();
            Case {
                name: format!("chain-eviction-log{log}"),
                strategy: CompressionStrategy::Chain,
                search,
                dictionary: None,
                blocks: vec![first.clone(), repeated.clone()],
                window_log: 17,
            }
        })
        .collect()
}

fn block_sections(frame: &[u8]) -> Vec<&[u8]> {
    // Independent framing inspection isolates the repeated block. This reads
    // public three-byte block headers; it does not predict matching decisions.
    let (header, mut position) = FrameHeader::parse(frame).unwrap();
    let mut result = Vec::new();
    loop {
        let start = position;
        let bytes = frame.get(position..position + 3).unwrap();
        let word = u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16);
        let kind = (word >> 1) & 3;
        assert_ne!(kind, 3);
        let length = if kind == 1 { 1 } else { (word >> 3) as usize };
        position += 3 + length;
        result.push(frame.get(start..position).unwrap());
        if word & 1 != 0 {
            break;
        }
    }
    assert_eq!(
        position + if header.has_checksum { 4 } else { 0 },
        frame.len()
    );
    result
}

#[test]
fn search_hash_dimension_changes_greedy_candidate_selection_on_a_designed_collision() {
    let cases = hash_collision_cases();
    let small = encode_case(&cases[0], 17, 11);
    let large = encode_case(&cases[1], 17, 11);
    assert_ne!(
        small.frame, large.frame,
        "hash dimension did not separate the later false candidate from the earlier full match"
    );
    assert!(large.search_capacity > small.search_capacity);
    eprintln!(
        "hash collision: log 10/14 frames {}/{} bytes, search capacity {}/{} bytes",
        small.frame.len(),
        large.frame.len(),
        small.search_capacity,
        large.search_capacity
    );
}

#[test]
fn search_chain_node_dimension_changes_the_repeated_block_after_cyclic_eviction() {
    let cases = node_eviction_cases();
    let small = encode_case(&cases[0], 113, 31);
    let large = encode_case(&cases[1], 113, 31);
    let small_blocks = block_sections(&small.frame);
    let large_blocks = block_sections(&large.frame);
    assert_eq!(small_blocks.len(), 2);
    assert_eq!(large_blocks.len(), 2);
    assert_eq!(
        small_blocks[0], large_blocks[0],
        "the common initialized history must have identical encoding"
    );
    assert_ne!(
        small_blocks[1], large_blocks[1],
        "node eviction did not change the repeated block's candidate choice"
    );
    assert!(large.search_capacity > small.search_capacity);
    eprintln!(
        "chain eviction: log 10/12 repeated block {}/{} bytes, search capacity {}/{} bytes",
        small_blocks[1].len(),
        large_blocks[1].len(),
        small.search_capacity,
        large.search_capacity
    );
}

#[test]
fn search_hash_and_chain_dimensions_change_actual_retained_search_storage() {
    let source = support::noise(1024, 0.15, 0.0, 91);
    for vary_hash in [true, false] {
        let strategy = if vary_hash {
            CompressionStrategy::Greedy
        } else {
            CompressionStrategy::Chain
        };
        let mut capacities = Vec::new();
        for large in [false, true] {
            let mut search = compact_search();
            search
                .set_hash_log(if vary_hash && large { 14 } else { 10 })
                .unwrap();
            search
                .set_chain_log(if !vary_hash && large { 12 } else { 10 })
                .unwrap();
            let case = Case {
                name: format!("dimension-hash{vary_hash}-large{large}"),
                strategy,
                search,
                dictionary: None,
                blocks: vec![source.clone()],
                window_log: 17,
            };
            let observed = encode_case(&case, 53, 47);
            assert!(observed.search_capacity > 0);
            assert!(observed.search_capacity <= observed.total_capacity);
            capacities.push(observed.search_capacity);
        }
        assert!(
            capacities[1] > capacities[0],
            "{strategy:?}: {capacities:?}"
        );
    }
}

#[test]
fn search_key_width_changes_whether_isolated_short_dictionary_tails_are_searched() {
    for tail in [4, 5] {
        let cases = width_cases(tail);
        let frames: Vec<_> = cases
            .iter()
            .map(|case| encode_case(case, 13, 7).frame)
            .collect();
        if tail == 4 {
            assert_ne!(
                frames[0], frames[1],
                "width5 must not probe a four-byte tail"
            );
            assert_eq!(
                frames[1], frames[2],
                "both wider keys leave this same tail literal"
            );
        } else {
            assert_eq!(
                frames[0], frames[1],
                "both complete keys find the same five-byte tail"
            );
            assert_ne!(
                frames[1], frames[2],
                "width8 must not probe a five-byte tail"
            );
        }
        eprintln!(
            "width tail {tail}: widths 4/5/8 frames {:?} bytes",
            frames.iter().map(Vec::len).collect::<Vec<_>>()
        );
        for (case, expected) in cases.iter().zip(frames) {
            assert_eq!(encode_case(case, 257, 97).frame, expected, "{}", case.name);
        }
    }
}

#[test]
fn search_chain_effort_changes_frames_on_the_older_longer_dictionary_ladder() {
    check_effort_effect(false);
}

#[test]
fn search_lazy_effort_changes_frames_on_the_next_position_dictionary_ladder() {
    check_effort_effect(true);
}

fn check_effort_effect(lazy: bool) {
    let cases = effort_cases(lazy);
    let frames: Vec<_> = cases
        .iter()
        .map(|case| encode_case(case, 11, 5).frame)
        .collect();
    for (index, pair) in frames.windows(2).enumerate() {
        assert_ne!(
            pair[0],
            pair[1],
            "effort did not change {} vs {}",
            cases[index].name,
            cases[index + 1].name
        );
    }
    eprintln!(
        "{} effort: visits 1/8/16/64 frames {:?} bytes",
        if lazy { "Lazy" } else { "Chain" },
        frames.iter().map(Vec::len).collect::<Vec<_>>()
    );
    let mut independent = cases[1].clone();
    if lazy {
        independent.search.set_chain_visits(64).unwrap();
    } else {
        independent.search.set_lazy_visits(64).unwrap();
    }
    assert_eq!(
        encode_case(&independent, 29, 31).frame,
        frames[1],
        "unselected strategy's visit field affected this frame"
    );
}

#[test]
fn search_history_horizon_reaches_repeated_blocks_beyond_the_short_retained_suffix() {
    for strategy in POLICIES {
        let cases = history_cases(strategy);
        let small = encode_case(&cases[0], 701, 257);
        let large = encode_case(&cases[1], 701, 257);
        assert!(large.search_capacity > small.search_capacity);
        eprintln!(
            "history {strategy:?}: 8/64 KiB frames {}/{} bytes, search capacity {}/{} bytes",
            small.frame.len(),
            large.frame.len(),
            small.search_capacity,
            large.search_capacity
        );
        assert!(
            large.frame.len() + 16 * 1024 < small.frame.len(),
            "{strategy:?}: short{} long{}",
            small.frame.len(),
            large.frame.len()
        );
    }
}

fn allocation_cycle_cases() -> Vec<Case> {
    let rich = Dictionary::raw(b"abcdefghijklmnop:dictionary-seed")
        .unwrap()
        .into_shared();
    let short = Dictionary::raw(b"ab").unwrap().into_shared();
    [
        (CompressionStrategy::Greedy, 10, 10, 4, 512, rich.clone()),
        (CompressionStrategy::Chain, 11, 11, 8, 2048, rich),
        (CompressionStrategy::Lazy, 10, 10, 5, 256, short.clone()),
        (CompressionStrategy::Greedy, 12, 12, 8, 4096, short),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(index, (strategy, hash, chain, width, history, dictionary))| {
            let mut search = compact_search();
            search.set_hash_log(hash).unwrap();
            search.set_chain_log(chain).unwrap();
            search.set_key_bytes(width).unwrap();
            search.set_history_bytes(history).unwrap();
            let mut source = b"abcdefghijklmnop".repeat(8);
            source.extend_from_slice(b"end");
            Case {
                name: format!("allocation-stage{index}-{strategy:?}"),
                strategy,
                search,
                dictionary: Some(dictionary),
                blocks: vec![source],
                window_log: 17,
            }
        },
    )
    .collect()
}

fn run_allocation_cycle<A: Allocator + Clone>(
    encoder: &mut Encoder<A>,
    cases: &[Case],
) -> Result<Vec<Vec<u8>>> {
    let mut frames = Vec::new();
    for case in cases {
        encoder.reset_with_options(case.options()).unwrap();
        encoder.set_dictionary(case.dictionary.clone()).unwrap();
        encoder.set_search_options(case.search).unwrap();
        let frame = run_blocks(encoder, &case.blocks, 19, 13)?;
        check_frame(case, &frame);
        frames.push(frame);
    }
    Ok(frames)
}

fn tracking(failure: Option<u64>) -> TrackingAllocator {
    TrackingAllocator::new(AllocatorConfig {
        max_live_allocations: 256,
        fail_at: failure.map(|ordinal| NonZeroU64::new(ordinal).unwrap()),
    })
    .unwrap()
}

#[test]
fn search_every_observed_allocation_failure_in_shape_cycles_recovers_and_releases_owners() {
    let cases = allocation_cycle_cases();
    let owner = tracking(None);
    let monitor = owner.monitor();
    let mut encoder = Encoder::new_in(owner);
    let expected = run_allocation_cycle(&mut encoder, &cases).unwrap();
    let attempts = monitor.snapshot().attempts;
    assert!(
        attempts > 0 && attempts <= 512,
        "keep the complete ordinal campaign bounded: {attempts}"
    );
    drop(encoder);
    monitor.assert_released();
    assert_eq!(
        monitor.snapshot().owners_dropped,
        monitor.snapshot().identities_created
    );

    for ordinal in 1..=attempts {
        let owner = tracking(Some(ordinal));
        let monitor = owner.monitor();
        let mut encoder = Encoder::new_in(owner);
        assert_eq!(
            run_allocation_cycle(&mut encoder, &cases),
            Err(Error::AllocationFailed),
            "ordinal{ordinal}/{attempts}"
        );
        assert_eq!(monitor.snapshot().failures, 1);
        assert!(!monitor.snapshot().violations.any());
        assert_eq!(
            encoder.process(&[], &mut [], EndDirective::Continue),
            Err(Error::InvalidState)
        );
        monitor.clear_failure();
        encoder.reset();
        encoder.set_dictionary(None).unwrap();
        // Only codec allocations remain reachable after clearing the distinct
        // Global dictionary domain. Every surviving owner must be accounted for.
        assert_eq!(
            monitor.snapshot().live_bytes,
            encoder.retained_capacity() as u128
        );
        assert_eq!(
            run_allocation_cycle(&mut encoder, &cases).unwrap(),
            expected
        );
        drop(encoder);
        monitor.assert_released();
        assert_eq!(
            monitor.snapshot().owners_dropped,
            monitor.snapshot().identities_created
        );
    }
    eprintln!(
        "search allocation campaign: {attempts} observed ordinals, each rejected then recovered; {} stages per cycle",
        cases.len()
    );
}

#[test]
#[ignore = "requires pinned zstd v1.5.7 CLI; set ZSTD_BIN; independently decodes selected complete frames"]
fn official_codec_decodes_configured_width_effort_history_and_prepared_dictionary_frames() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
    let executable = std::env::var_os("ZSTD_BIN").expect("set ZSTD_BIN");
    let version = Command::new(&executable).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("v1.5.7"));
    let directory = std::env::temp_dir().join(format!(
        "zstdoxide-search-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let dictionary_path = directory.join("dictionary");
    let mut cases = hash_collision_cases();
    cases.extend(node_eviction_cases());
    cases.extend(width_cases(4));
    cases.extend(width_cases(5));
    cases.extend(effort_cases(false));
    cases.extend(effort_cases(true));
    for strategy in POLICIES {
        cases.extend(history_cases(strategy));
    }
    let prepared = Dictionary::from_bytes(include_bytes!("upstream/dictionaries/formatted.dict"))
        .unwrap()
        .into_shared();
    for strategy in POLICIES {
        let mut search = compact_search();
        search.set_key_bytes(8).unwrap();
        search.set_chain_visits(64).unwrap();
        search.set_lazy_visits(64).unwrap();
        cases.push(Case {
            name: format!("{strategy:?}-prepared"),
            strategy,
            search,
            dictionary: Some(prepared.clone()),
            blocks: vec![prepared.content().to_vec()],
            window_log: 17,
        });
    }
    assert_eq!(cases.len(), 27);
    let mut bytes = 0;
    for case in &cases {
        let encoded = encode_case(case, 97, 31).frame;
        let mut command = Command::new(&executable);
        command.args(["-d", "-q", "-c"]);
        if let Some(dictionary) = &case.dictionary {
            std::fs::write(&dictionary_path, dictionary.as_bytes()).unwrap();
            command.arg("-D").arg(&dictionary_path);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let writer = std::thread::spawn(move || stdin.write_all(&encoded).unwrap());
        let result = child.wait_with_output().unwrap();
        writer.join().unwrap();
        assert!(
            result.status.success(),
            "{}: {}",
            case.name,
            String::from_utf8_lossy(&result.stderr)
        );
        let expected = case.source();
        assert_eq!(result.stdout, expected, "{}", case.name);
        bytes += expected.len();
    }
    std::fs::remove_dir_all(directory).unwrap();
    eprintln!(
        "official v1.5.7 search-option decoding: {} frames/{bytes} source bytes",
        cases.len()
    );
}
