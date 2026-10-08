//! Decoder adaptations of v1.5.7's FSE and Huffman fuzz contracts.
//! Fixtures were generated and verified by the independent upstream codec.

use alloc::vec::Vec;

use crate::fse::{FseTable, read_normalized_counts};
use crate::huffman::{HuffmanTable, decode_literals};

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
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes(bytes.try_into().unwrap()))
            .collect();
        let header = input.take(header_size);
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
