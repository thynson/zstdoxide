//! Supplied-table sequence bit output, without changing emitted frame state.

use crate::bitwriter::BitWriter;
use crate::constants::{LL_BASE, LL_BITS, ML_BASE, ML_BITS};
use crate::frame::MAX_BLOCK_SIZE;
use crate::fse_encode::FseEncodingTable;
use crate::sequence_plan::PlannedSequence;
use crate::{Error, Result};

/// Encode a nonempty sequence list using three immutable selected tables.
///
/// Corresponds to the bit ordering in upstream `ZSTD_encodeSequences_body`.
/// The caller writes the count, mode byte, and table descriptions separately;
/// a zero-sequence section has no bitstream and must not call this function.
/// Codes, extras, workload size and table support are checked before resetting
/// the reusable writer. A later allocation failure may leave partial scratch
/// output, which the caller must discard. Borrowed plans and tables never change.
///
/// Contextual repeat correctness belongs to forward planning: this function
/// neither validates the incoming repeat triple nor publishes `next_repeat`.
pub(crate) fn encode_sequences(
    sequences: &[PlannedSequence],
    literal_lengths: &FseEncodingTable,
    offsets: &FseEncodingTable,
    match_lengths: &FseEncodingTable,
    writer: &mut BitWriter,
) -> Result<()> {
    if sequences.is_empty() {
        return Err(Error::CorruptData);
    }
    for (table, maximum_symbol, maximum_log) in [
        (literal_lengths, 35, 9),
        (offsets, 31, 8),
        (match_lengths, 52, 9),
    ] {
        if table.max_symbol() > maximum_symbol
            || (table.table_log() != 0 && !(5..=maximum_log).contains(&table.table_log()))
        {
            return Err(Error::CorruptData);
        }
    }
    let mut generated = 0_u32;
    for sequence in sequences {
        validate_sequence(sequence)?;
        generated = generated
            .checked_add(sequence.literal_length)
            .and_then(|length| length.checked_add(sequence.match_length))
            .filter(|&length| length <= MAX_BLOCK_SIZE as u32)
            .ok_or(Error::CorruptData)?;
        literal_lengths.initial(usize::from(sequence.literal_code))?;
        offsets.initial(usize::from(sequence.offset_code))?;
        match_lengths.initial(usize::from(sequence.match_code))?;
    }

    writer.reset();
    let (mut literal_state, mut offset_state, mut match_state) = (0, 0, 0);
    for (index, sequence) in sequences.iter().rev().enumerate() {
        let literal_code = usize::from(sequence.literal_code);
        let offset_code = usize::from(sequence.offset_code);
        let match_code = usize::from(sequence.match_code);
        if index == 0 {
            literal_state = literal_lengths.initial(literal_code)?;
            offset_state = offsets.initial(offset_code)?;
            match_state = match_lengths.initial(match_code)?;
        } else {
            // The decoder updates LL, ML, then OF after executing a sequence.
            // Append their inverse fields in the opposite order.
            let offset = offsets.transition(offset_code, offset_state)?;
            let matches = match_lengths.transition(match_code, match_state)?;
            let literals = literal_lengths.transition(literal_code, literal_state)?;
            writer.write(u32::from(offset.value), offset.bits)?;
            writer.write(u32::from(matches.value), matches.bits)?;
            writer.write(u32::from(literals.value), literals.bits)?;
            offset_state = offset.state;
            match_state = matches.state;
            literal_state = literals.state;
        }
        // The decoder consumes OF, ML, then LL extra fields.
        writer.write(sequence.literal_extra, LL_BITS[literal_code])?;
        writer.write(sequence.match_extra, ML_BITS[match_code])?;
        writer.write(sequence.offset_extra, sequence.offset_code)?;
    }
    // Initial decoding state fields are consumed in LL, OF, ML order.
    writer.write(u32::from(match_state), match_lengths.table_log())?;
    writer.write(u32::from(offset_state), offsets.table_log())?;
    writer.write(u32::from(literal_state), literal_lengths.table_log())?;
    writer.finish()
}

fn validate_sequence(sequence: &PlannedSequence) -> Result<()> {
    let literal_code = usize::from(sequence.literal_code);
    let match_code = usize::from(sequence.match_code);
    let literal_bits = *LL_BITS.get(literal_code).ok_or(Error::CorruptData)?;
    let match_bits = *ML_BITS.get(match_code).ok_or(Error::CorruptData)?;
    if sequence.offset_code > 31
        || sequence.distance == 0
        || u64::from(sequence.literal_extra) >= 1_u64 << literal_bits
        || u64::from(sequence.match_extra) >= 1_u64 << match_bits
        || u64::from(sequence.offset_extra) >= 1_u64 << sequence.offset_code
        || LL_BASE[literal_code] + sequence.literal_extra != sequence.literal_length
        || ML_BASE[match_code] + sequence.match_extra != sequence.match_length
        || (1_u32 << sequence.offset_code) + sequence.offset_extra != sequence.offset_value
        || (sequence.offset_value > 3 && sequence.offset_value - 3 != sequence.distance)
    {
        return Err(Error::CorruptData);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{LL_DEFAULT, ML_DEFAULT, OF_DEFAULT};
    use crate::dictionary::DictionaryEntropy;
    use crate::entropy_encode::{normalize_frequencies, write_normalized_counts};
    use crate::fse::FseTable;
    use crate::history::History;
    use crate::sequences::SequenceState;
    use alloc::vec;
    use alloc::vec::Vec;

    struct Table {
        encoding: FseEncodingTable,
        description: Vec<u8>,
        mode: u8,
    }

    fn plans(specifications: &[(u32, u32, u32)], mut repeat: [u32; 3]) -> Vec<PlannedSequence> {
        specifications
            .iter()
            .map(|&(literals, matches, distance)| {
                let plan = PlannedSequence::new(literals, matches, distance, repeat).unwrap();
                repeat = plan.next_repeat;
                plan
            })
            .collect()
    }

    fn tables(sequences: &[PlannedSequence], modes: [u8; 3]) -> [Table; 3] {
        core::array::from_fn(|channel| {
            let (defaults, log) = match channel {
                0 => (&LL_DEFAULT[..], 6),
                1 => (&OF_DEFAULT[..], 5),
                _ => (&ML_DEFAULT[..], 6),
            };
            let mode = modes[channel];
            if mode == 0 {
                return Table {
                    encoding: FseEncodingTable::new(defaults, log).unwrap(),
                    description: Vec::new(),
                    mode,
                };
            }
            let mut frequencies = vec![0_u64; defaults.len()];
            for sequence in sequences {
                let symbol = match channel {
                    0 => sequence.literal_code,
                    1 => sequence.offset_code,
                    _ => sequence.match_code,
                };
                frequencies[usize::from(symbol)] += 1;
            }
            let first = frequencies.iter().position(|&count| count != 0).unwrap();
            if mode == 1 {
                assert_eq!(frequencies.iter().filter(|&&count| count != 0).count(), 1);
                return Table {
                    encoding: FseEncodingTable::rle(first as u8),
                    description: vec![first as u8],
                    mode,
                };
            }
            assert_eq!(mode, 2);
            // Inline normalized tables need at least two symbols; a spare
            // supported symbol also tests unused but valid transition rows.
            if frequencies.iter().filter(|&&count| count != 0).count() == 1 {
                let other = (first + 1) % frequencies.len();
                frequencies[other] = 1;
            }
            let counts = normalize_frequencies(&frequencies, 5).unwrap();
            Table {
                encoding: FseEncodingTable::new(&counts, 5).unwrap(),
                description: write_normalized_counts(&counts, 5).unwrap(),
                mode,
            }
        })
    }

    fn section(sequences: &[PlannedSequence], tables: &[Table; 3], reuse: bool) -> Vec<u8> {
        assert!(sequences.len() < 128);
        let before = sequences.to_vec();
        let mut writer = BitWriter::new();
        encode_sequences(
            sequences,
            &tables[0].encoding,
            &tables[1].encoding,
            &tables[2].encoding,
            &mut writer,
        )
        .unwrap();
        assert_eq!(sequences, before);
        let mut section = vec![sequences.len() as u8];
        section.push(if reuse {
            0xfc
        } else {
            (tables[0].mode << 6) | (tables[1].mode << 4) | (tables[2].mode << 2)
        });
        if !reuse {
            for table in tables {
                section.extend_from_slice(&table.description);
            }
        }
        section.extend_from_slice(writer.as_bytes());
        section
    }

    fn mode_frame(modes: [u8; 3]) -> (Vec<u8>, Vec<u8>) {
        let sequences = plans(&[(1, 3, 1); 3], [1, 4, 8]);
        let tables = tables(&sequences, modes);
        let mut state = SequenceState::new();
        let mut history = History::new(24);
        history.start_frame(24);
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0x20, 24];
        for (index, literals) in [b"abc", b"def"].into_iter().enumerate() {
            let section = section(&sequences, &tables, index != 0);
            state
                .decode(&section, literals, &mut history, MAX_BLOCK_SIZE)
                .unwrap();
            let mut payload = vec![3 << 3]; // Three raw literals.
            payload.extend_from_slice(literals);
            payload.extend_from_slice(&section);
            let header = ((payload.len() as u32) << 3) | 4 | u32::from(index == 1);
            frame.extend_from_slice(&header.to_le_bytes()[..3]);
            frame.extend_from_slice(&payload);
        }
        let expected = b"aaaabbbbccccddddeeeeffff".to_vec();
        assert_eq!(history.into_bytes(), expected);
        assert_eq!(crate::decompress(&frame, expected.len()).unwrap(), expected);
        (frame, expected)
    }

    #[test]
    fn every_predefined_new_rle_combination_decodes_and_reuses_each_channel() {
        for literal_mode in 0..=2 {
            for offset_mode in 0..=2 {
                for match_mode in 0..=2 {
                    mode_frame([literal_mode, offset_mode, match_mode]);
                }
            }
        }
    }

    #[test]
    fn maximum_channel_logs_and_offset_code_preserve_full_width_fields() {
        let sequences = plans(&[(1, 3, 1); 3], [1, 4, 8]);
        let tables = [9, 8, 9].map(|log| {
            let counts = [1 << (log - 1); 2];
            Table {
                encoding: FseEncodingTable::new(&counts, log).unwrap(),
                description: write_normalized_counts(&counts, log).unwrap(),
                mode: 2,
            }
        });
        let section = section(&sequences, &tables, false);
        let mut history = History::new(12);
        history.start_frame(12);
        SequenceState::new()
            .decode(&section, b"abc", &mut history, MAX_BLOCK_SIZE)
            .unwrap();
        assert_eq!(history.into_bytes(), b"aaaabbbbcccc");

        // This is the wire's representability limit, independent of the
        // encoder's smaller retained match-search horizon.
        let sequence = PlannedSequence::new(1, 3, (1 << 31) - 3, [1, 4, 8]).unwrap();
        assert_eq!(sequence.offset_code, 31);
        let mut writer = BitWriter::new();
        encode_sequences(
            &[sequence],
            &FseEncodingTable::rle(1),
            &FseEncodingTable::rle(31),
            &FseEncodingTable::rle(0),
            &mut writer,
        )
        .unwrap();
        assert_eq!(writer.as_bytes(), [0, 0, 0, 128]);
    }

    #[test]
    fn all_rle_zero_bit_states_emit_only_the_terminal_marker() {
        let sequences = plans(&[(0, 3, 10); 3], [10; 3]);
        let tables = tables(&sequences, [1; 3]);
        let section = section(&sequences, &tables, false);
        assert_eq!(section, [3, 0x54, 0, 0, 0, 1]);
        let mut history = History::new(19);
        history.start_frame(19);
        history.append(b"abcdefghij").unwrap();
        SequenceState::with_repeat([10; 3])
            .decode(&section, b"", &mut history, MAX_BLOCK_SIZE)
            .unwrap();
        assert_eq!(history.into_bytes(), b"abcdefghijabcdefghi");
    }

    #[test]
    fn long_literals_zero_literals_and_duplicate_repeats_preserve_exact_output() {
        let sequences = plans(
            &[(65_536, 4, 10), (0, 9, 9), (0, 5, 10), (3, 4, 11)],
            [10; 3],
        );
        assert_eq!(sequences[0].literal_code, 35);
        assert_eq!(sequences[0].offset_value, 1);
        assert_eq!(sequences[1].offset_value, 3);
        let prefix: Vec<_> = (0..64).collect();
        let literals: Vec<_> = (0..65_546).map(|index| (index * 17 + 19) as u8).collect();
        let mut expected = prefix.clone();
        let mut position = 0;
        for sequence in &sequences {
            let end = position + sequence.literal_length as usize;
            expected.extend_from_slice(&literals[position..end]);
            for _ in 0..sequence.match_length {
                expected.push(expected[expected.len() - sequence.distance as usize]);
            }
            position = end;
        }
        expected.extend_from_slice(&literals[position..]);
        for modes in [[0; 3], [2; 3]] {
            let tables = tables(&sequences, modes);
            let section = section(&sequences, &tables, false);
            let mut history = History::new(expected.len());
            history.start_frame(MAX_BLOCK_SIZE);
            history.append(&prefix).unwrap();
            SequenceState::with_repeat([10; 3])
                .decode(&section, &literals, &mut history, MAX_BLOCK_SIZE)
                .unwrap();
            assert_eq!(history.into_bytes(), expected);
        }
    }

    #[test]
    fn reused_dictionary_singletons_keep_nonzero_state_widths() {
        fn singleton(symbol: usize, log: u8) -> FseTable {
            let mut counts = vec![0; symbol + 1];
            counts[symbol] = 1 << log;
            let header = write_normalized_counts(&counts, log).unwrap();
            FseTable::read_dictionary(&header, symbol, log).unwrap().0
        }
        let entropy = DictionaryEntropy {
            huffman: crate::huffman::HuffmanTable::read(&[128, 0x10]).unwrap().0,
            literal_lengths: singleton(1, 5),
            offsets: singleton(0, 7),
            match_lengths: singleton(0, 6),
            repeat: [1, 4, 8],
        };
        let sequences = plans(&[(1, 3, 1); 3], [1, 4, 8]);
        let tables = [
            &entropy.literal_lengths,
            &entropy.offsets,
            &entropy.match_lengths,
        ]
        .map(|table| FseEncodingTable::from_decoding(table).unwrap());
        let mut writer = BitWriter::new();
        encode_sequences(&sequences, &tables[0], &tables[1], &tables[2], &mut writer).unwrap();
        assert_eq!(writer.as_bytes(), [0, 0, 4]); // 5+7+6 state bits, then marker.
        let mut section = vec![3, 0xfc];
        section.extend_from_slice(writer.as_bytes());
        let mut history = History::new(12);
        history.start_frame(12);
        SequenceState::with_repeat(entropy.repeat)
            .decode_with_dictionary(
                &section,
                b"abc",
                &mut history,
                MAX_BLOCK_SIZE,
                Some(&entropy),
            )
            .unwrap();
        assert_eq!(history.into_bytes(), b"aaaabbbbcccc");
    }

    #[test]
    fn malformed_plans_table_limits_and_support_fail_before_resetting_scratch() {
        let sequence = PlannedSequence::new(1, 3, 1, [1, 4, 8]).unwrap();
        let tables = tables(&[sequence], [0; 3]);
        let defaults = [
            &tables[0].encoding,
            &tables[1].encoding,
            &tables[2].encoding,
        ];
        let mut writer = BitWriter::new();
        writer.write(0xab, 8).unwrap();
        writer.finish().unwrap();
        let mut reject = |sequences: &[PlannedSequence], chosen: [&FseEncodingTable; 3]| {
            assert_eq!(
                encode_sequences(sequences, chosen[0], chosen[1], chosen[2], &mut writer),
                Err(Error::CorruptData)
            );
            assert_eq!(writer.as_bytes(), [0xab, 1]);
        };
        reject(&[], defaults);
        for field in 0..9 {
            let mut malformed = sequence;
            match field {
                0 => malformed.literal_code = 36,
                1 => malformed.match_code = 53,
                2 => malformed.offset_code = 32,
                3 => malformed.literal_extra = 1,
                4 => malformed.match_extra = 1,
                5 => malformed.offset_extra = 1,
                6 => malformed.offset_value = 2,
                7 => malformed.distance = 0,
                _ => malformed.literal_length = u32::MAX,
            }
            reject(&[malformed], defaults);
        }
        let full = PlannedSequence::new(65_536, 65_536, 1, [1, 4, 8]).unwrap();
        reject(&[full; 2], defaults);
        for (channel, max_symbol, excessive_log) in [(0, 36, 10), (1, 32, 9), (2, 53, 10)] {
            let used_symbol = if channel == 0 { 1 } else { 0 };
            let mut counts = vec![0; max_symbol + 1];
            counts[used_symbol] = 16;
            counts[max_symbol] = 16;
            let unsupported_symbol = FseEncodingTable::new(&counts, 5).unwrap();
            let mut chosen = defaults;
            chosen[channel] = &unsupported_symbol;
            reject(&[sequence], chosen);
            let high_log =
                FseEncodingTable::new(&[1 << (excessive_log - 1); 2], excessive_log).unwrap();
            chosen[channel] = &high_log;
            reject(&[sequence], chosen);
        }
        let sequences = plans(&[(10, 3, 7), (1, 3, 7)], [1, 4, 8]);
        assert_eq!((sequences[0].offset_code, sequences[1].offset_code), (3, 0));
        let last_symbol_only = FseEncodingTable::rle(0);
        reject(&sequences, [defaults[0], &last_symbol_only, defaults[2]]);
    }

    #[test]
    fn absent_repeat_context_and_incomplete_or_overlong_streams_fail_decoding() {
        let sequences = plans(&[(1, 3, 1); 3], [1, 4, 8]);
        let tables = tables(&sequences, [2; 3]);
        let fresh = section(&sequences, &tables, false);
        let reused = section(&sequences, &tables, true);
        let mut damaged = fresh.clone();
        damaged.push(1);
        for malformed in [&reused[..], &fresh[..fresh.len() - 1], &damaged[..]] {
            let mut history = History::new(12);
            history.start_frame(12);
            assert!(
                SequenceState::new()
                    .decode(malformed, b"abc", &mut history, MAX_BLOCK_SIZE)
                    .is_err()
            );
        }
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN"]
    fn official_codec_decodes_every_supplied_table_mode_and_repeat_combination() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let executable = std::env::var_os("ZSTD_BIN").unwrap_or_else(|| "zstd".into());
        let version = Command::new(&executable).arg("--version").output().unwrap();
        assert!(version.status.success());
        assert!(std::string::String::from_utf8_lossy(&version.stdout).contains("1.5.7"));
        let mut cases = 0;
        for literal_mode in 0..=2 {
            for offset_mode in 0..=2 {
                for match_mode in 0..=2 {
                    let modes = [literal_mode, offset_mode, match_mode];
                    let (frame, expected) = mode_frame(modes);
                    let mut child = Command::new(&executable)
                        .args(["-q", "-d", "-c"])
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .unwrap();
                    child.stdin.take().unwrap().write_all(&frame).unwrap();
                    let result = child.wait_with_output().unwrap();
                    assert!(
                        result.status.success(),
                        "modes {modes:?}: {}",
                        std::string::String::from_utf8_lossy(&result.stderr)
                    );
                    assert_eq!(result.stdout, expected);
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 27);
        std::eprintln!(
            "Official v1.5.7 decoded {cases} frames /54 supplied-table and repeat sections"
        );
    }
}
