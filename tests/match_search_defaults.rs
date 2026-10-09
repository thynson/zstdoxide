//! Offline pre-Module-11 frame fingerprints from committed ba4529c.
//!
//! Expectations were captured before configurable search changes and decoded
//! independently with pinned C. Normal replay needs no C or added dependency.
//! See fixtures/match-search-baseline/README.md for the capture and hash limits.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use zstdoxide::{
    CompressionStrategy, DecodeOptions, Dictionary, Encoder, EncoderOptions, EndDirective,
    FrameHeader, SharedDictionary, StreamStatus, decompress, decompress_with_dictionary,
};

#[allow(dead_code)]
#[path = "upstream/support/mod.rs"]
mod support;

const BLOCK: usize = 131_072;
const OUTPUT_CHUNK: usize = 257;
const EXPECTED: &str = include_str!("fixtures/match-search-baseline/expected.tsv");
const RECIPES: &str = include_str!("fixtures/match-search-baseline/cases.tsv");
const FORMATTED: &[u8] = include_bytes!("upstream/dictionaries/formatted.dict");

#[derive(Debug)]
struct Row(BTreeMap<&'static str, &'static str>);

impl Row {
    fn text(&self, field: &str) -> &'static str {
        self.0[field]
    }

    fn number(&self, field: &str) -> usize {
        self.text(field).parse().unwrap()
    }

    fn fingerprint(&self, field: &str) -> u64 {
        u64::from_str_radix(self.text(field), 16).unwrap()
    }

    fn options(&self) -> EncoderOptions {
        EncoderOptions {
            strategy: strategy(self.text("strategy")),
            window_log: self.number("window_log").try_into().unwrap(),
            checksum: self.text("checksum") == "1",
            pledged_size: match self.text("pledged_size") {
                "none" => None,
                value => Some(value.parse().unwrap()),
            },
            include_content_size: self.text("include_content_size") == "1",
            include_dictionary_id: self.text("include_dictionary_id") == "1",
        }
    }
}

fn rows(input: &'static str, columns: usize) -> Vec<Row> {
    let mut lines = input.lines();
    let header: Vec<_> = lines.next().unwrap().split('\t').collect();
    assert_eq!(header.len(), columns);
    assert_eq!(header.iter().collect::<BTreeSet<_>>().len(), columns);
    lines
        .map(|line| {
            let values: Vec<_> = line.split('\t').collect();
            assert_eq!(values.len(), columns);
            Row(header.iter().copied().zip(values).collect())
        })
        .collect()
}

fn strategy(name: &str) -> CompressionStrategy {
    match name {
        "Greedy" => CompressionStrategy::Greedy,
        "Chain" => CompressionStrategy::Chain,
        "Lazy" => CompressionStrategy::Lazy,
        _ => panic!("unrecognized captured strategy {name}"),
    }
}

fn strategy_name(value: CompressionStrategy) -> &'static str {
    match value {
        CompressionStrategy::Greedy => "Greedy",
        CompressionStrategy::Chain => "Chain",
        CompressionStrategy::Lazy => "Lazy",
    }
}

// Same raw-byte convention as support::tests::noise_matches_upstream_generated_vectors.
fn fingerprint(input: &[u8]) -> u64 {
    input.iter().fold(0xcbf2_9ce4_8422_2325, |value, byte| {
        (value ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn baselines() -> Vec<Row> {
    let rows = rows(EXPECTED, 33);
    assert_eq!(rows.len(), 152);
    for (index, (safe, all)) in rows[..76].iter().zip(&rows[76..]).enumerate() {
        assert_eq!(safe.text("backend"), "safe");
        assert_eq!(all.text("backend"), "all");
        assert_eq!(safe.number("record_index"), index);
        assert_eq!(all.number("record_index"), index);
        for (&key, &value) in &safe.0 {
            if key != "backend" {
                assert_eq!(all.text(key), value, "captured backend mismatch: {key}");
            }
        }
    }
    rows
}

fn expected<'a>(
    rows: &'a [Row],
    name: &str,
    policy: CompressionStrategy,
    role: &str,
) -> [&'a Row; 2] {
    let selected: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.text("case") == name
                && row.text("strategy") == strategy_name(policy)
                && row.text("role") == role
        })
        .collect();
    assert_eq!(selected.len(), 2, "case {name}, {policy:?}, {role}");
    selected.try_into().unwrap()
}

struct Case {
    name: &'static str,
    input: Vec<u8>,
    dictionary: Option<SharedDictionary>,
    dictionary_bytes: Vec<u8>,
    dictionary_kind: &'static str,
    window: u8,
    flush: usize,
}

fn cases() -> Vec<Case> {
    let recipes = rows(RECIPES, 14);
    assert_eq!(recipes.len(), 12);
    assert_eq!(
        recipes
            .iter()
            .map(|row| row.text("case"))
            .collect::<BTreeSet<_>>()
            .len(),
        12
    );
    let history = support::noise(2 * BLOCK, 0.0, 0.0, 0xa55a_1234);
    let dictionary_source = support::noise(65_540, 0.0, 0.0, 1);
    recipes
        .iter()
        .map(|recipe| {
            let name = recipe.text("case");
            let dictionary_kind = recipe.text("dictionary_kind");
            let dictionary_bytes = match name {
                "raw-seed-65538" => dictionary_source[..65_538].to_vec(),
                "raw-seed-65539" => dictionary_source[..65_539].to_vec(),
                "raw-seed-65540" => dictionary_source.clone(),
                "formatted-seed" => FORMATTED.to_vec(),
                "short-flush-dictionary-crossing" => b"xyz".to_vec(),
                _ => Vec::new(),
            };
            // Verify saved dictionary material before parsing or attaching it.
            assert_eq!(
                dictionary_bytes.len(),
                recipe.number("dictionary_serialized_length")
            );
            if dictionary_kind == "none" {
                assert_eq!(recipe.text("dictionary_fnv1a64"), "-");
                assert!(dictionary_bytes.is_empty());
            } else {
                assert_eq!(
                    fingerprint(&dictionary_bytes),
                    recipe.fingerprint("dictionary_fnv1a64")
                );
            }
            let dictionary = match dictionary_kind {
                "none" => None,
                "raw" => Some(Dictionary::raw(&dictionary_bytes).unwrap().into_shared()),
                "formatted" => Some(
                    Dictionary::from_bytes(&dictionary_bytes)
                        .unwrap()
                        .into_shared(),
                ),
                _ => panic!("unrecognized dictionary kind {dictionary_kind}"),
            };
            assert_eq!(
                dictionary.as_ref().map_or(0, |value| value.id()),
                recipe.number("dictionary_id") as u32
            );
            assert_eq!(
                dictionary.as_ref().map_or(0, |value| value.content().len()),
                recipe.number("dictionary_content_length")
            );
            let input = match name {
                "rdg-three-blocks" => support::noise(3 * BLOCK + 17, 0.5, 0.5, 0),
                "history-before-window17"
                | "history-before-window18"
                | "history-at-window17"
                | "history-at-window18"
                | "history-after-window17"
                | "history-after-window18" => {
                    let start = if name.starts_with("history-before-") {
                        BLOCK - 1
                    } else if name.starts_with("history-at-") {
                        BLOCK
                    } else {
                        BLOCK + 1
                    };
                    let mut input = history.clone();
                    input.extend_from_slice(&history[start..start + BLOCK / 2]);
                    input
                }
                "raw-seed-65538" | "raw-seed-65539" | "raw-seed-65540" | "formatted-seed" => {
                    dictionary
                        .as_ref()
                        .unwrap()
                        .content()
                        .iter()
                        .copied()
                        .cycle()
                        .take(2 * BLOCK + 37)
                        .collect()
                }
                "short-flush-dictionary-crossing" => {
                    let mut input = b"ab".to_vec();
                    input.extend(b"xyzab".repeat(100));
                    input
                }
                _ => panic!("unrecognized captured case {name}"),
            };
            // All twelve cases are materialized and verified before any encoder runs.
            assert_eq!(input.len(), recipe.number("input_length"), "{name}");
            assert_eq!(
                fingerprint(&input),
                recipe.fingerprint("input_fnv1a64"),
                "{name}"
            );
            Case {
                name,
                input,
                dictionary,
                dictionary_bytes,
                dictionary_kind,
                window: recipe.number("window_log").try_into().unwrap(),
                flush: recipe.number("flush_after_input_bytes"),
            }
        })
        .collect()
}

fn action(
    encoder: &mut Encoder,
    input: &[u8],
    directive: EndDirective,
    encoded: &mut Vec<u8>,
    peak: &mut usize,
) -> usize {
    let mut position = 0;
    for calls in 1..=8192 {
        let mut output = [0xa5; OUTPUT_CHUNK + 8];
        let progress = encoder
            .process(&input[position..], &mut output[..OUTPUT_CHUNK], directive)
            .unwrap();
        assert!(progress.consumed <= input.len() - position && progress.produced <= OUTPUT_CHUNK);
        assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
        position += progress.consumed;
        encoded.extend_from_slice(&output[..progress.produced]);
        *peak = (*peak).max(encoder.retained_capacity());
        if matches!(
            (directive, progress.status),
            (EndDirective::Finish, StreamStatus::FrameFinished)
                | (EndDirective::Flush, StreamStatus::Flushed)
        ) {
            assert_eq!(position, input.len());
            return calls;
        }
        assert!(
            progress.consumed + progress.produced > 0,
            "no progress with available buffers"
        );
    }
    panic!("capture replay exceeded its bounded progress budget")
}

fn block_layout(encoded: &[u8], case: &Case) -> String {
    let (header, mut position) = FrameHeader::parse(encoded).unwrap();
    assert_eq!(header.window_size, 1_u64 << case.window);
    assert_eq!(header.content_size, None);
    assert!(header.has_checksum);
    assert_eq!(
        header.dictionary_id,
        case.dictionary.as_ref().map_or(0, |value| value.id())
    );
    let mut blocks = Vec::new();
    loop {
        let bytes = encoded.get(position..position + 3).unwrap();
        let bits = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], 0]);
        position += 3;
        let kind = (bits >> 1) & 3;
        let length = (bits >> 3) as usize;
        assert!(kind < 3);
        assert!(length <= BLOCK);
        blocks.push(format!("{kind}:{length}"));
        position += if kind == 1 { 1 } else { length };
        assert!(position <= encoded.len());
        if bits & 1 != 0 {
            break;
        }
    }
    assert_eq!(position + 4, encoded.len());
    if case.input.len() > 2 * BLOCK {
        assert!(blocks.len() >= 3);
    }
    blocks.join(",")
}

fn replay_frame(encoder: &mut Encoder, case: &Case, expected: [&Row; 2]) -> Vec<u8> {
    for row in expected {
        assert_eq!(encoder.options(), row.options());
        assert_eq!(row.number("flush_after_input_bytes"), case.flush);
        assert_eq!(row.number("output_chunk_capacity"), OUTPUT_CHUNK);
        assert_eq!(row.number("input_length"), case.input.len());
        assert_eq!(row.fingerprint("input_fnv1a64"), fingerprint(&case.input));
        assert_eq!(row.text("dictionary_kind"), case.dictionary_kind);
        assert_eq!(
            row.number("dictionary_serialized_length"),
            case.dictionary_bytes.len()
        );
        assert_eq!(
            row.number("dictionary_content_length"),
            case.dictionary
                .as_ref()
                .map_or(0, |value| value.content().len())
        );
        assert_eq!(
            row.number("dictionary_id") as u32,
            case.dictionary.as_ref().map_or(0, |value| value.id())
        );
        if case.dictionary.is_some() {
            assert_eq!(
                row.fingerprint("dictionary_fnv1a64"),
                fingerprint(&case.dictionary_bytes)
            );
        }
    }
    let before = encoder.retained_capacity();
    let mut peak = before;
    let mut encoded = Vec::new();
    let mut calls = 0;
    if case.flush != 0 {
        calls += action(
            encoder,
            &case.input[..case.flush],
            EndDirective::Flush,
            &mut encoded,
            &mut peak,
        );
    }
    calls += action(
        encoder,
        &case.input[case.flush..],
        EndDirective::Finish,
        &mut encoded,
        &mut peak,
    );
    assert_eq!(encoder.total_input(), case.input.len() as u64);
    assert_eq!(encoder.total_output(), encoded.len() as u64);
    assert!(encoder.retained_capacity() >= before && peak >= encoder.retained_capacity());
    let layout = block_layout(&encoded, case);
    for row in expected {
        assert_eq!(encoded.len(), row.number("output_length"), "{row:?}");
        assert_eq!(
            fingerprint(&encoded),
            row.fingerprint("output_fnv1a64"),
            "{row:?}"
        );
        assert_eq!(calls, row.number("process_calls"), "{row:?}");
        assert_eq!(
            layout,
            row.text("block_layout_type_and_payload_length"),
            "{row:?}"
        );
        // Recorded capacities are host observations, not portable expectations.
        // Compare runtime retention relationships instead of their absolute bytes.
        assert_eq!(row.text("narrow_search_retained_capacity"), "not-recorded");
    }
    let decoded = if let Some(dictionary) = &case.dictionary {
        decompress_with_dictionary(&encoded, dictionary, DecodeOptions::new(case.input.len()))
            .unwrap()
    } else {
        decompress(&encoded, case.input.len()).unwrap()
    };
    assert_eq!(decoded, case.input);
    encoded
}

fn replay_policy(policy: CompressionStrategy) {
    let baselines = baselines();
    let cases = cases();
    let mut frames = 0;
    for case in &cases {
        let selected = expected(&baselines, case.name, policy, "fresh");
        let options = selected[0].options();
        assert_eq!(
            options,
            EncoderOptions {
                strategy: policy,
                window_log: case.window,
                ..EncoderOptions::default()
            }
        );
        let mut encoder = Encoder::with_options(options).unwrap();
        assert_eq!(encoder.retained_capacity(), 0);
        encoder.set_dictionary(case.dictionary.clone()).unwrap();
        if case.dictionary.is_none() {
            assert_eq!(encoder.retained_capacity(), 0);
        } else {
            // Before processing, only the selected dictionary is reachable.
            // Its prepared tables can add storage beyond the serialized bytes.
            assert!(encoder.retained_capacity() >= case.dictionary_bytes.len());
        }
        let fresh = replay_frame(&mut encoder, case, selected);
        let capacity = encoder.retained_capacity();
        encoder.reset();
        assert_eq!(encoder.retained_capacity(), capacity);
        assert_eq!(encoder.options(), options);
        assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
        let reused = replay_frame(
            &mut encoder,
            case,
            expected(&baselines, case.name, policy, "reset-reuse"),
        );
        assert_eq!(reused, fresh, "{} {policy:?}", case.name);
        frames += 2;
    }
    assert_eq!(frames, 24);
}

#[test]
fn greedy_default_frames_match_both_captured_backends_after_reset() {
    replay_policy(CompressionStrategy::Greedy);
}
#[test]
fn chain_default_frames_match_both_captured_backends_after_reset() {
    replay_policy(CompressionStrategy::Chain);
}
#[test]
fn lazy_default_frames_match_both_captured_backends_after_reset() {
    replay_policy(CompressionStrategy::Lazy);
}

#[test]
fn strategy_cycle_preserves_default_bytes_and_retains_both_search_families() {
    let baselines = baselines();
    let cases = cases();
    let case = cases
        .iter()
        .find(|case| case.name == "raw-seed-65539")
        .unwrap();
    let mut encoder = Encoder::new();
    encoder.set_dictionary(case.dictionary.clone()).unwrap();
    let mut capacities = Vec::new();
    let mut first_greedy = None;
    for (index, policy) in [
        CompressionStrategy::Greedy,
        CompressionStrategy::Chain,
        CompressionStrategy::Lazy,
        CompressionStrategy::Greedy,
    ]
    .into_iter()
    .enumerate()
    {
        let rows = expected(
            &baselines,
            case.name,
            policy,
            &format!("strategy-cycle-{index}"),
        );
        let before = encoder.retained_capacity();
        encoder.reset_with_options(rows[0].options()).unwrap();
        assert_eq!(encoder.retained_capacity(), before);
        let encoded = replay_frame(&mut encoder, case, rows);
        let fresh = expected(&baselines, case.name, policy, "fresh");
        assert_eq!(encoded.len(), fresh[0].number("output_length"));
        assert_eq!(
            fingerprint(&encoded),
            fresh[0].fingerprint("output_fnv1a64")
        );
        if index == 0 {
            first_greedy = Some(encoded);
        } else if index == 3 {
            assert_eq!(&encoded, first_greedy.as_ref().unwrap());
        }
        capacities.push(encoder.retained_capacity());
    }
    assert_eq!(capacities.len(), 4);
    assert!(
        capacities[1] > capacities[0],
        "Chain must add its independent cache"
    );
    assert_eq!(
        capacities[2], capacities[1],
        "Lazy reuses the warmed Chain cache"
    );
    assert_eq!(
        capacities[3], capacities[2],
        "returning to Greedy retains the Chain cache"
    );
}

#[test]
fn archived_record_schema_counts_and_capacity_qualifications_are_explicit() {
    let records = baselines();
    assert_eq!(fingerprint(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fingerprint(b"foobar"), 0x8594_4171_f739_67e8);
    for backend in ["safe", "all"] {
        let mut identities = BTreeSet::new();
        let mut fresh = 0;
        let mut reuse = 0;
        let mut cycle = 0;
        for row in records.iter().filter(|row| row.text("backend") == backend) {
            assert!(identities.insert((row.text("case"), row.text("strategy"), row.text("role"))));
            match row.text("role") {
                "fresh" => fresh += 1,
                "reset-reuse" => reuse += 1,
                "strategy-cycle-0" | "strategy-cycle-1" | "strategy-cycle-2"
                | "strategy-cycle-3" => cycle += 1,
                role => panic!("unrecognized role {role}"),
            }
            for field in ["input_sha256", "dictionary_sha256", "output_sha256"] {
                let hash = row.text(field);
                if field == "dictionary_sha256" && row.text("dictionary_kind") == "none" {
                    assert_eq!(hash, "-");
                } else {
                    assert_eq!(hash.len(), 64);
                    assert!(
                        hash.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    );
                }
            }
            let dictionary = row.number("selected_dictionary_reachable_bytes");
            assert_eq!(
                row.number("owned_codec_retained_before") + dictionary,
                row.number("retained_capacity_before")
            );
            assert_eq!(
                row.number("owned_codec_retained_after") + dictionary,
                row.number("retained_capacity_after")
            );
            assert!(row.number("peak_reachable_capacity") >= row.number("retained_capacity_after"));
            assert_eq!(row.text("narrow_search_retained_capacity"), "not-recorded");
        }
        assert_eq!((fresh, reuse, cycle, identities.len()), (36, 36, 4, 76));
    }
}
