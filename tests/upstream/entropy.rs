//! Adaptations of v1.5.7's FSE and Huffman fuzz contracts.
//! Fixtures were generated and verified by the independent upstream codec.

use alloc::vec::Vec;

use crate::entropy_encode::{normalize_frequencies, write_normalized_counts};
use crate::fse::{FseTable, read_normalized_counts};
#[cfg(feature = "std")]
use crate::huffman::decode_streams;
use crate::huffman::{HuffmanTable, decode_literals};

#[path = "fuzz_support.rs"]
#[allow(dead_code)] // The shared producer also serves the other upstream adapters.
pub(super) mod fuzz_support;

use fuzz_support::DataProducer;

struct Records<'a> {
    remaining: &'a [u8],
}

impl<'a> Records<'a> {
    fn take(&mut self, count: usize) -> &'a [u8] {
        let (value, rest) = self.remaining.split_at(count);
        self.remaining = rest;
        value
    }

    fn u32(&mut self) -> usize {
        u32::from_le_bytes(self.take(4).try_into().unwrap()) as usize
    }
}

/// `tests/fuzzer.c:4172-4189`, PR#1255: 32 unit counts at table log five
/// must not trigger a division by zero. The original C test ignores the return
/// value; v1.5.7 rejects this requested log before normalization. Our independent
/// normalizer accepts the exactly representable distribution, so its successful
/// mass/readback assertions do not claim identical C normalization policy.
#[test]
fn fuzzer_pr1255_unit_counts_normalize_without_division_by_zero() {
    let counts = [1_u64; 32];
    let normalized = normalize_frequencies(&counts, 5).unwrap();
    assert_eq!(normalized, [-1; 32]);
    assert_eq!(
        normalized
            .iter()
            .map(|count| usize::from(count.unsigned_abs()))
            .sum::<usize>(),
        32
    );
    let encoded = write_normalized_counts(&normalized, 5).unwrap();
    let decoded = read_normalized_counts(&encoded, 31, 5).unwrap();
    assert_eq!(decoded.table_log, 5);
    assert_eq!(decoded.symbols, 32);
    assert_eq!(decoded.bytes_read, encoded.len());
    assert_eq!(&decoded.counts[..32], &normalized);
}

/// `tests/fuzzer.c:4190-4205`, PR#2779: preserve the exact sparse distribution.
/// The header below was independently emitted by v1.5.7 FSE_writeNCount with
/// 512 bytes of destination slack, log five and maximum symbol ten, then checked
/// with FSE_readNCount. Reference revision:
/// f8745da6ff1ad1e7bab384bd1f9d742439278e99; BSD option in LICENSE.zstd.
/// The original C call with a nine-byte destination returns dstSize_tooSmall
/// despite this nine-byte final header. Rust returns owned bytes, so this test
/// does not claim the original caller-buffer capacity/error contract is ported.
#[test]
fn fuzzer_pr2779_sparse_counts_match_the_exact_nine_byte_header() {
    const COUNTS: [i16; 11] = [1, 0, 1, 0, 1, 0, 1, 0, 1, 9, 18];
    const EXPECTED: [u8; 9] = [0x20, 0x02, 0x22, 0x20, 0x02, 0x22, 0x10, 0xea, 0x03];
    let encoded = write_normalized_counts(&COUNTS, 5).unwrap();
    assert_eq!(encoded, EXPECTED);
    let decoded = read_normalized_counts(&encoded, 10, 5).unwrap();
    assert_eq!(decoded.table_log, 5);
    assert_eq!(decoded.symbols, 11);
    assert_eq!(decoded.bytes_read, 9);
    assert_eq!(&decoded.counts[..11], COUNTS);
    for end in 0..encoded.len() {
        assert!(read_normalized_counts(&encoded[..end], 10, 5).is_err());
    }
}

/// `fse_read_ncount.c`: preserve counts, maximum symbol, log and exact length.
#[test]
fn normalized_counts_match_all_official_vectors_and_suffixes() {
    let mut input = Records {
        remaining: include_bytes!("entropy-data/fse.bin"),
    };
    assert_eq!(input.take(8), b"ZFSE0001");
    let count = input.u32();
    assert_eq!(count, 512);
    for case in 0..count {
        let log = input.take(1)[0];
        let maximum = usize::from(input.take(1)[0]);
        let header_size = usize::from(u16::from_le_bytes(input.take(2).try_into().unwrap()));
        let expected: Vec<_> = input
            .take(2 * (maximum + 1))
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| i16::from_le_bytes(*bytes))
            .collect();
        let header = input.take(header_size);
        assert_eq!(
            write_normalized_counts(&expected, log).unwrap(),
            header,
            "independent C normalized-count bytes, case {case}"
        );
        for suffix_size in [0, 1, 2, 7, 31, 512 - header_size] {
            let mut with_suffix = header.to_vec();
            with_suffix.extend((0..suffix_size).map(|index| (index * 37 + case) as u8));
            let actual = read_normalized_counts(&with_suffix, 255, 12)
                .unwrap_or_else(|error| panic!("case {case}, suffix {suffix_size}: {error}"));
            assert_eq!(actual.table_log, log, "case {case}");
            assert_eq!(actual.symbols, maximum + 1, "case {case}");
            assert_eq!(actual.bytes_read, header_size, "case {case}");
            assert_eq!(&actual.counts[..actual.symbols], expected, "case {case}");
        }
        for end in 0..header.len() {
            assert!(read_normalized_counts(&header[..end], 255, 12).is_err());
        }
        if maximum != 0 {
            assert!(read_normalized_counts(header, maximum - 1, 12).is_err());
        }
        assert!(read_normalized_counts(header, maximum, log - 1).is_err());
        if expected.iter().filter(|&&value| value != 0).count() >= 2 {
            let (actual, consumed) = FseTable::read(header, maximum, log).unwrap();
            let reference = FseTable::from_normalized(&expected, log).unwrap();
            assert_eq!(consumed, header_size);
            for state in 0..1 << log {
                assert_eq!(actual.entry(state), reference.entry(state));
            }
        }
    }
    assert!(input.remaining.is_empty());
}

/// The exact parameter/count selection in `tests/fuzz/fse_read_ncount.c`.
/// The producer borrows input, the fixed count array owns initialized cells,
/// and subtracting each absolute weight keeps the remaining mass nonnegative.
fn produce_normalized_counts(producer: &mut DataProducer<'_>) -> ([i16; 256], u8, usize) {
    let log = producer.unsigned_range(5, 12) as u8;
    let maximum = producer.unsigned_range(0, 255) as usize;
    let mut remaining = (1_u32 << log) - 1;
    let mut counts = [0_i16; 256];
    for count in &mut counts[..maximum] {
        if remaining == 0 {
            break;
        }
        *count = producer.signed_range(-1, remaining as i32) as i16;
        remaining -= u32::from(count.unsigned_abs());
    }
    counts[maximum] = (remaining + 1) as i16;
    if counts[maximum] == 1 && producer.unsigned_range(0, 1) == 1 {
        counts[maximum] = -1;
    }
    (counts, log, maximum)
}

/// Port of all normalized writer/reader and random-suffix assertions in
/// facebook/zstd v1.5.7 `tests/fuzz/fse_read_ncount.c:28-100`.
/// Source-derived control flow is used under root LICENSE.zstd's BSD option.
fn normalized_count_fuzz_round_trip(input: &[u8]) -> (u8, usize) {
    let mut producer = DataProducer::new(input);
    let (counts, log, maximum) = produce_normalized_counts(&mut producer);
    assert_ne!(counts[maximum], 0);
    assert_eq!(
        counts
            .iter()
            .map(|count| usize::from(count.unsigned_abs()))
            .sum::<usize>(),
        1 << log
    );
    // Preserve FSE_NCountWriteBound's allocation assertion. Maximum zero asks
    // for its default 512-byte bound, even for a single-symbol distribution.
    let write_bound = if maximum == 0 {
        512
    } else {
        ((maximum + 1) * usize::from(log) + 6) / 8 + 3
    };
    assert!(write_bound <= 512);
    let encoded = write_normalized_counts(&counts[..=maximum], log).unwrap();
    assert!(encoded.len() <= write_bound);
    let header_size = encoded.len();
    let buffer_size = producer.unsigned_range(header_size as u32, 512) as usize;
    let mut buffer = encoded;
    for _ in header_size..buffer_size {
        buffer.push(producer.unsigned_range(0, 255) as u8);
    }
    let decoded = read_normalized_counts(&buffer, 255, 12).unwrap();
    assert_eq!(decoded.bytes_read, header_size);
    assert_eq!(decoded.symbols, maximum + 1);
    assert_eq!(decoded.table_log, log);
    assert_eq!(&decoded.counts[..=maximum], &counts[..=maximum]);
    (log, maximum)
}

#[test]
fn normalized_count_producer_preserves_signed_ranges_and_final_symbol_selection() {
    let (counts, log, maximum) = produce_normalized_counts(&mut DataProducer::new(&[]));
    assert_eq!((log, maximum, counts[0]), (5, 0, 32));
    assert!(counts[1..].iter().all(|&count| count == 0));

    // Tail consumption: log=5, maximum=2, counts=-1 then0, final mass=31.
    let (counts, log, maximum) = produce_normalized_counts(&mut DataProducer::new(&[1, 0, 2, 0]));
    assert_eq!((log, maximum), (5, 2));
    assert_eq!(&counts[..3], &[-1, 0, 31]);

    // Consuming all available mass stops the loop; the final selector can
    // encode the required last unit as -1 while skipped cells stay zero.
    let (counts, log, maximum) = produce_normalized_counts(&mut DataProducer::new(&[1, 32, 3, 0]));
    assert_eq!((log, maximum), (5, 3));
    assert_eq!(&counts[..4], &[31, 0, 0, -1]);

    let (counts, log, maximum) = produce_normalized_counts(&mut DataProducer::new(&[0, 7]));
    assert_eq!((log, maximum, counts[0]), (12, 0, 4096));
}

#[test]
fn normalized_count_fuzz_target_roundtrips_every_log_and_maximum_symbol() {
    assert_eq!(normalized_count_fuzz_round_trip(&[]), (5, 0));
    for log in 5..=12 {
        for maximum in 0..=255 {
            // Exhaustion selects -1 repeatedly; longer zero and all-one
            // tails exercise sparse runs, full mass and random suffix sizes.
            for (length, byte) in [(0, 0), (512, 0), (512, 255)] {
                let mut input = alloc::vec![byte; length];
                input.extend_from_slice(&[maximum as u8, log - 5]);
                assert_eq!(normalized_count_fuzz_round_trip(&input), (log, maximum));
            }
        }
    }
    let mut state = 0x8c73_e965;
    for _ in 0..4096 {
        let length = (random(&mut state) >> 16) as usize % 2049;
        let input: Vec<_> = (0..length)
            .map(|_| (random(&mut state) >> 24) as u8)
            .collect();
        normalized_count_fuzz_round_trip(&input);
    }
}

/// Replay the complete normalized-count target on each external input.
/// The flat corpus is sorted deterministically; entries must be regular files,
/// with at most 65,536 files, 1 MiB per input and 64 MiB in total. No entry is
/// silently skipped or truncated, and every generated header/suffix is <=512 B.
#[cfg(feature = "std")]
#[test]
#[ignore = "requires pinned extracted fse_read_ncount corpus in ZSTDOXIDE_FSE_CORPUS"]
fn replays_official_normalized_count_corpus() {
    use std::fs;
    use std::io::Read;
    use std::path::PathBuf;

    const MAX_INPUT: usize = 1024 * 1024;
    const MAX_TOTAL: usize = 64 * 1024 * 1024;
    let directory =
        PathBuf::from(std::env::var_os("ZSTDOXIDE_FSE_CORPUS").expect("set ZSTDOXIDE_FSE_CORPUS"));
    assert!(fs::symlink_metadata(&directory).unwrap().is_dir());
    let mut paths = Vec::new();
    for entry in fs::read_dir(&directory).unwrap() {
        assert!(paths.len() < 65_536, "corpus file-count limit");
        paths.push(entry.unwrap().path());
    }
    paths.sort();
    assert!(!paths.is_empty(), "empty corpus");
    let mut total_bytes = 0_usize;
    for path in &paths {
        std::eprintln!("fse_read_ncount: {}", path.display());
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(metadata.is_file(), "corpus must contain regular files only");
        assert!(metadata.len() <= MAX_INPUT as u64, "corpus input limit");
        let mut input = Vec::new();
        fs::File::open(path)
            .unwrap()
            .take((MAX_INPUT + 1) as u64)
            .read_to_end(&mut input)
            .unwrap();
        assert!(input.len() <= MAX_INPUT, "corpus input limit");
        total_bytes = total_bytes.checked_add(input.len()).unwrap();
        assert!(total_bytes <= MAX_TOTAL, "corpus total-byte limit");
        normalized_count_fuzz_round_trip(&input);
    }
    std::eprintln!(
        "replayed {} normalized-count inputs / {total_bytes} source bytes; no skips",
        paths.len()
    );
}

fn literals_section(header: &[u8], stream: &[u8], output_size: usize) -> Vec<u8> {
    // Four streams, 18-bit regenerated and compressed lengths, compressed kind.
    let size = header.len() + stream.len();
    assert!(size < 1 << 18 && output_size < 1 << 18);
    let prefix = 0x0e | ((output_size as u64) << 4) | ((size as u64) << 22);
    let mut section = prefix.to_le_bytes()[..5].to_vec();
    section.extend_from_slice(header);
    section.extend_from_slice(stream);
    section
}

/// Decoder portion of `huf_round_trip.c`, using the official encoder as oracle.
#[test]
fn huffman_one_and_four_streams_match_official_encoder() {
    let mut input = Records {
        remaining: include_bytes!("entropy-data/huffman.bin"),
    };
    assert_eq!(input.take(8), b"ZHUF0001");
    let count = input.u32();
    assert_eq!(count, 30);
    let mut weight_modes = [false; 2];
    for case in 0..count {
        let length = input.u32();
        let header_size = input.u32();
        let single_size = input.u32();
        let four_size = input.u32();
        let original = input.take(length);
        let header = input.take(header_size);
        let single = input.take(single_size);
        let four = input.take(four_size);
        weight_modes[usize::from(header[0] >= 128)] = true;
        let (table, consumed) = HuffmanTable::read(header).unwrap();
        assert_eq!(consumed, header.len());
        assert_eq!(
            table.decode_stream(single, length).unwrap(),
            original,
            "case {case}"
        );
        assert!(
            table
                .decode_stream(&single[..single.len() - 1], length)
                .is_err()
        );
        assert!(table.decode_stream(single, length - 1).is_err());
        assert!(table.decode_stream(single, length + 1).is_err());
        let section = literals_section(header, four, length);
        let mut previous = None;
        let (output, consumed) = decode_literals(&section, &mut previous, length).unwrap();
        assert_eq!(output, original, "case {case}");
        assert_eq!(consumed, section.len());
        assert!(decode_literals(&section, &mut None, length - 1).is_err());
        // Reusing the same tree is another contract from huf_round_trip.c.
        let mut repeated = literals_section(&[], four, length);
        repeated[0] |= 1;
        assert_eq!(
            decode_literals(&repeated, &mut previous, length).unwrap().0,
            original
        );
        assert!(decode_literals(&repeated, &mut None, length).is_err());
    }
    assert_eq!(weight_modes, [true, true]);
    assert!(input.remaining.is_empty());
}

fn random(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state
}

/// `huf_decompress.c`: bounded arbitrary tables and 1X/4X decoding never panic.
/// No expectation of rejection: arbitrary bytes can encode a valid stream.
#[test]
fn arbitrary_entropy_inputs_keep_outputs_bounded() {
    let mut state = 0x62bd_4431;
    let mut table_successes = 0;
    for case in 0..4096 {
        let length = (random(&mut state) as usize >> 8) % 1025;
        let mut data: Vec<_> = (0..length)
            .map(|_| (random(&mut state) >> 24) as u8)
            .collect();
        if data.len() >= 2 && case % 4 == 0 {
            // A valid two-symbol tree makes payload paths reachable as well.
            data[..2].copy_from_slice(&[128, 0x10]);
        }
        let maximum = (random(&mut state) as usize >> 8) % (8 * length + 501);
        if let Ok((table, consumed)) = HuffmanTable::read(&data) {
            table_successes += 1;
            assert!(consumed <= data.len());
            for stream in [&data[..], &data[consumed..]] {
                if let Ok(output) = table.decode_stream(stream, maximum) {
                    assert_eq!(output.len(), maximum);
                }
            }
        }
        let section = literals_section(&[], &data, maximum);
        if let Ok((output, consumed)) = decode_literals(&section, &mut None, maximum) {
            assert_eq!(output.len(), maximum);
            assert_eq!(consumed, section.len());
        }
        if let Ok(parsed) = read_normalized_counts(&data, 255, 12) {
            assert!(parsed.bytes_read <= data.len());
            assert!((1..=256).contains(&parsed.symbols));
            assert_eq!(
                parsed
                    .counts
                    .iter()
                    .map(|count| usize::from(count.unsigned_abs()))
                    .sum::<usize>(),
                1 << parsed.table_log
            );
        }
    }
    assert!(table_successes > 900);
}

/// File replay of `huf_decompress.c`, including both CPU-flag consumption paths.
/// The decoder's Zstandard table-log cap remains 11; upstream's standalone
/// entropy API can additionally accept log 12 and has X1/X2/CPU variants.
#[cfg(feature = "std")]
#[test]
#[ignore = "requires pinned extracted huf_decompress corpus in ZSTDOXIDE_HUF_CORPUS"]
fn replays_official_huffman_decoder_corpus() {
    use std::fs;
    use std::path::PathBuf;

    // FUZZ_dataProducer_uint32Range consumes bytes from the input tail.
    fn parameter(data: &[u8], remaining: &mut usize, minimum: u32, maximum: u32) -> u32 {
        let range = maximum - minimum;
        let mut rolling = range;
        let mut result = 0_u32;
        while rolling != 0 && *remaining != 0 {
            *remaining -= 1;
            result = (result << 8) | u32::from(data[*remaining]);
            rolling >>= 8;
        }
        minimum + result % (range + 1)
    }

    let directory =
        PathBuf::from(std::env::var_os("ZSTDOXIDE_HUF_CORPUS").expect("set ZSTDOXIDE_HUF_CORPUS"));
    let mut paths = Vec::new();
    for entry in fs::read_dir(&directory).unwrap() {
        assert!(paths.len() < 65536, "corpus file-count limit");
        paths.push(entry.unwrap().path());
    }
    paths.sort();
    assert!(!paths.is_empty(), "empty corpus");
    let mut parsed = 0;
    for path in &paths {
        std::eprintln!("huf_decompress: {}", path.display());
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(metadata.is_file(), "corpus must contain regular files only");
        assert!(metadata.len() <= 1024 * 1024, "corpus input limit");
        let data = fs::read(path).unwrap();
        assert!(data.len() <= 1024 * 1024, "corpus input limit");
        for bmi2 in [false, true] {
            let mut remaining = data.len();
            let four_streams = parameter(&data, &mut remaining, 0, 1) != 0;
            let _symbol_variant = parameter(&data, &mut remaining, 0, 1);
            if bmi2 {
                let _flag = parameter(&data, &mut remaining, 0, 1);
            }
            for _ in 0..5 {
                let _flag = parameter(&data, &mut remaining, 0, 1);
            }
            let maximum =
                parameter(&data, &mut remaining, 0, (8 * data.len() + 500) as u32) as usize;
            let _table_log = parameter(&data, &mut remaining, 1, 12);
            let payload = &data[..remaining];
            if let Ok((table, _consumed)) = HuffmanTable::read(payload) {
                parsed += 1;
                // The upstream target passes the whole input, including
                // table bytes, to the usingDTable decoder. Preserve that.
                if let Ok(output) = decode_streams(&table, payload, maximum, four_streams) {
                    assert_eq!(output.len(), maximum);
                }
            }
        }
    }
    std::eprintln!(
        "replayed {} Huffman inputs with both flag-consumption paths; {parsed} tables parsed",
        paths.len()
    );
}
