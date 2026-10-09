//! Bounded per-channel FSE selection without publishing frame entropy state.

use alloc::vec::Vec;

use crate::entropy_encode::{normalize_frequencies, write_normalized_counts};
use crate::frame::MAX_BLOCK_SIZE;
use crate::fse_encode::FseEncodingTable;
use crate::{Error, Result};

/// Wire values in the sequence section's two-bit channel fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum Mode {
    Predefined = 0,
    Rle = 1,
    Compressed = 2,
    Repeat = 3,
}

/// No candidate clones a table or retains the source symbols. Borrowed choices
/// remain tied to their immutable owners; new/RLE candidates own replacements.
enum TableChoice<'a> {
    Borrowed(&'a FseEncodingTable),
    Owned(FseEncodingTable),
}

pub(crate) struct Selection<'a> {
    mode: Mode,
    table: TableChoice<'a>,
    description: Vec<u8>,
    #[cfg(test)]
    stream_bits: usize,
    bit_cost: usize,
}

impl Selection<'_> {
    pub(crate) fn mode(&self) -> Mode {
        self.mode
    }

    pub(crate) fn table(&self) -> &FseEncodingTable {
        match &self.table {
            TableChoice::Borrowed(table) => table,
            TableChoice::Owned(table) => table,
        }
    }

    pub(crate) fn description(&self) -> &[u8] {
        &self.description
    }

    /// Final state width plus reverse transition widths, without the shared
    /// terminal marker, byte padding, mode/count fields or sequence extra bits.
    #[cfg(test)]
    pub(crate) fn stream_bits(&self) -> usize {
        self.stream_bits
    }

    /// Eight times the description length plus `stream_bits`. All exclusions
    /// documented on `stream_bits` also apply to this exact channel cost.
    #[cfg(test)]
    pub(crate) fn bit_cost(&self) -> usize {
        self.bit_cost
    }

    /// Transfer a selected new/RLE table after its metadata and stream have
    /// been emitted. Predefined/repeat choices retain their existing owners;
    /// the caller uses the mode to publish the appropriate cached table.
    pub(crate) fn into_replacement(self) -> Option<FseEncodingTable> {
        match self.table {
            TableChoice::Borrowed(_) => None,
            TableChoice::Owned(table) => Some(table),
        }
    }
}

/// Select an exact-cost candidate in the role of `ZSTD_selectEncodingType`.
///
/// `predefined` must be the real fixed table of the caller's channel. Its
/// identity is a caller contract; limits and symbol support are checked here.
/// Generic limits permit logs 5..=12; sequence callers supply LL/ML nine and
/// OF eight. Empty inputs have no sequence bitstream and are rejected. Work is
/// bounded to at most one block's maximum sequence count and eight new logs.
///
/// Ties prefer eligible repeat, then predefined, then RLE, then a lower new
/// table log. Missing symbols make borrowed candidates ineligible. New tables
/// use independent Rust normalization, not upstream mode/normalization policy.
/// No emitted table or repeat-offset state is mutated by this function.
pub(crate) fn select<'a>(
    symbols: &[u8],
    predefined: &'a FseEncodingTable,
    previous: Option<&'a FseEncodingTable>,
    maximum_symbol: u8,
    maximum_log: u8,
) -> Result<Selection<'a>> {
    if !(5..=12).contains(&maximum_log) {
        return Err(Error::InvalidParameter);
    }
    if symbols.is_empty() {
        return Err(Error::CorruptData);
    }
    if symbols.len() > MAX_BLOCK_SIZE / 3 {
        return Err(Error::ResourceLimit);
    }
    validate_table(predefined, maximum_symbol, maximum_log)?;
    if let Some(previous) = previous {
        validate_table(previous, maximum_symbol, maximum_log)?;
    }
    let mut frequencies = [0_u64; 256];
    let mut maximum = 0;
    for &symbol in symbols {
        if symbol > maximum_symbol {
            return Err(Error::CorruptData);
        }
        let index = usize::from(symbol);
        maximum = maximum.max(index);
        frequencies[index] = frequencies[index]
            .checked_add(1)
            .ok_or(Error::ResourceLimit)?;
    }
    let frequencies = &frequencies[..=maximum];
    let present = frequencies.iter().filter(|&&count| count != 0).count();
    let mut best = None;
    for (mode, table) in [
        (Mode::Repeat, previous),
        (Mode::Predefined, Some(predefined)),
    ] {
        if let Some(table) = table
            && supports(table, frequencies)
        {
            let stream_bits = stream_cost(table, symbols)?;
            consider(
                &mut best,
                Selection {
                    mode,
                    table: TableChoice::Borrowed(table),
                    description: Vec::new(),
                    #[cfg(test)]
                    stream_bits,
                    bit_cost: stream_bits,
                },
            );
        }
    }
    if present == 1 {
        let symbol = symbols[0];
        let mut description = Vec::new();
        description
            .try_reserve_exact(1)
            .map_err(|_| Error::AllocationFailed)?;
        description.push(symbol);
        consider(
            &mut best,
            owned_choice(
                Mode::Rle,
                FseEncodingTable::rle(symbol),
                description,
                symbols,
            )?,
        );
    } else {
        let ceiling = (usize::BITS - (symbols.len().max(32) - 1).leading_zeros()) as u8;
        for log in 5..=maximum_log.min(ceiling) {
            if present > 1_usize << log {
                continue;
            }
            let normalized = normalize_frequencies(frequencies, log)?;
            let table = FseEncodingTable::new(&normalized, log)?;
            let description = write_normalized_counts(&normalized, log)?;
            consider(
                &mut best,
                owned_choice(Mode::Compressed, table, description, symbols)?,
            );
        }
    }
    best.ok_or(Error::CorruptData)
}

fn validate_table(table: &FseEncodingTable, maximum_symbol: u8, maximum_log: u8) -> Result<()> {
    if table.max_symbol() > maximum_symbol
        || (table.table_log() != 0 && !(5..=maximum_log).contains(&table.table_log()))
    {
        return Err(Error::CorruptData);
    }
    Ok(())
}

fn supports(table: &FseEncodingTable, frequencies: &[u64]) -> bool {
    frequencies
        .iter()
        .enumerate()
        .all(|(symbol, &count)| count == 0 || table.initial(symbol).is_ok())
}

fn stream_cost(table: &FseEncodingTable, symbols: &[u8]) -> Result<usize> {
    let (&last, rest) = symbols.split_last().ok_or(Error::CorruptData)?;
    let mut state = table.initial(usize::from(last))?;
    let mut bits = usize::from(table.table_log());
    for &symbol in rest.iter().rev() {
        let transition = table.transition(usize::from(symbol), state)?;
        bits = bits
            .checked_add(usize::from(transition.bits))
            .ok_or(Error::ResourceLimit)?;
        state = transition.state;
    }
    Ok(bits)
}

fn owned_choice<'a>(
    mode: Mode,
    table: FseEncodingTable,
    description: Vec<u8>,
    symbols: &[u8],
) -> Result<Selection<'a>> {
    let stream_bits = stream_cost(&table, symbols)?;
    let bit_cost = description
        .len()
        .checked_mul(8)
        .and_then(|bits| bits.checked_add(stream_bits))
        .ok_or(Error::ResourceLimit)?;
    Ok(Selection {
        mode,
        table: TableChoice::Owned(table),
        description,
        #[cfg(test)]
        stream_bits,
        bit_cost,
    })
}

fn consider<'a>(best: &mut Option<Selection<'a>>, candidate: Selection<'a>) {
    if best
        .as_ref()
        .is_none_or(|best| candidate.bit_cost < best.bit_cost)
    {
        *best = Some(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitstream::ReverseBits;
    use crate::bitwriter::BitWriter;
    use crate::constants::{LL_DEFAULT, ML_DEFAULT, OF_DEFAULT};
    use crate::fse::FseTable;
    use crate::history::History;
    use crate::sequence_encode::encode_sequences;
    use crate::sequence_plan::PlannedSequence;
    use crate::sequences::SequenceState;
    use alloc::vec;

    fn measured_stream_bits(table: &FseEncodingTable, symbols: &[u8]) -> usize {
        let mut writer = BitWriter::new();
        let mut state = table
            .initial(usize::from(*symbols.last().unwrap()))
            .unwrap();
        for &symbol in symbols[..symbols.len() - 1].iter().rev() {
            let transition = table.transition(usize::from(symbol), state).unwrap();
            writer
                .write(u32::from(transition.value), transition.bits)
                .unwrap();
            state = transition.state;
        }
        writer.write(u32::from(state), table.table_log()).unwrap();
        writer.finish().unwrap();
        ReverseBits::new(writer.as_bytes()).unwrap().remaining()
    }

    fn verify_cost(choice: &Selection<'_>, symbols: &[u8]) {
        let bits = measured_stream_bits(choice.table(), symbols);
        assert_eq!(choice.stream_bits(), bits);
        assert_eq!(choice.bit_cost(), 8 * choice.description().len() + bits);
    }

    #[test]
    fn predefined_rle_new_and_repeat_have_cost_witnesses_without_cloning_borrows() {
        let predefined = FseEncodingTable::new(&LL_DEFAULT, 6).unwrap();
        let short = select(&[0], &predefined, None, 35, 9).unwrap();
        assert_eq!(short.mode(), Mode::Predefined);
        assert_eq!(short.bit_cost(), 6);
        assert!(core::ptr::eq(short.table(), &predefined));
        verify_cost(&short, &[0]);
        assert!(short.into_replacement().is_none());

        let uniform = [0; 256];
        let rle = select(&uniform, &predefined, None, 35, 9).unwrap();
        assert_eq!(rle.mode(), Mode::Rle);
        assert_eq!(rle.description(), [0]);
        assert_eq!((rle.stream_bits(), rle.bit_cost()), (0, 8));
        verify_cost(&rle, &uniform);
        assert_eq!(rle.into_replacement().unwrap().retained_capacity(), 0);

        let alternating: Vec<_> = (0..256).map(|index| (index % 2) as u8).collect();
        let compressed = select(&alternating, &predefined, None, 35, 9).unwrap();
        assert_eq!(compressed.mode(), Mode::Compressed);
        assert_eq!(compressed.table().table_log(), 5);
        verify_cost(&compressed, &alternating);
        let previous = compressed.into_replacement().unwrap();
        let repeated = select(&alternating, &predefined, Some(&previous), 35, 9).unwrap();
        assert_eq!(repeated.mode(), Mode::Repeat);
        assert!(repeated.description().is_empty());
        assert!(core::ptr::eq(repeated.table(), &previous));
        verify_cost(&repeated, &alternating);
        assert!(repeated.into_replacement().is_none());
    }

    #[test]
    fn ties_and_dictionary_singletons_keep_the_actual_repeated_state_width() {
        let predefined = FseEncodingTable::new(&LL_DEFAULT, 6).unwrap();
        let same = select(&[0], &predefined, Some(&predefined), 35, 9).unwrap();
        assert_eq!(same.mode(), Mode::Repeat);
        assert_eq!(same.bit_cost(), 6);
        for (log, expected_mode, expected_bits) in [
            (5, Mode::Repeat, 5),
            (8, Mode::Repeat, 8),
            (9, Mode::Rle, 8),
        ] {
            let header = write_normalized_counts(&[0, 1 << log], log).unwrap();
            let decoding = FseTable::read_dictionary(&header, 1, log).unwrap().0;
            let previous = FseEncodingTable::from_decoding(&decoding).unwrap();
            let symbols = [1; 64];
            let selected = select(&symbols, &predefined, Some(&previous), 35, 9).unwrap();
            assert_eq!(selected.mode(), expected_mode);
            assert_eq!(selected.bit_cost(), expected_bits);
            verify_cost(&selected, &symbols);
            if expected_mode == Mode::Repeat {
                assert_eq!(selected.table().table_log(), log);
                assert_eq!(selected.stream_bits(), usize::from(log));
            }
        }
    }

    #[test]
    fn descriptions_and_state_costs_match_written_bits_across_bounded_new_logs() {
        let predefined = FseEncodingTable::new(&LL_DEFAULT, 6).unwrap();
        for length in [1, 2, 3, 31, 32, 33, 127, 511, 4096] {
            let symbols: Vec<_> = (0..length)
                .map(|index| [0, 1, 2, 16, 35][index % 5])
                .collect();
            let selected = select(&symbols, &predefined, None, 35, 9).unwrap();
            verify_cost(&selected, &symbols);
            if selected.mode() == Mode::Compressed {
                let ceiling = (usize::BITS - (length.max(32) - 1).leading_zeros()) as u8;
                assert!((5..=9.min(ceiling)).contains(&selected.table().table_log()));
                let (decoded, consumed) = FseTable::read(selected.description(), 35, 9).unwrap();
                assert_eq!(consumed, selected.description().len());
                assert_eq!(decoded.table_log(), selected.table().table_log());
            }
        }
        // More than32 present symbols makes log5 ineligible; normalization
        // must advance to a representable candidate rather than dropping any.
        let symbols: Vec<_> = (0..4096).map(|index| (index % 36) as u8).collect();
        for maximum_log in [6, 8, 9, 12] {
            let selected = select(&symbols, &predefined, None, 35, maximum_log).unwrap();
            verify_cost(&selected, &symbols);
            assert!(selected.table().table_log() <= maximum_log);
            for symbol in 0..36 {
                assert!(selected.table().initial(symbol).is_ok());
            }
        }
    }

    #[test]
    fn support_holes_make_reuse_ineligible_and_invalid_limits_fail_before_costing() {
        let predefined = FseEncodingTable::new(&LL_DEFAULT, 6).unwrap();
        let previous = FseEncodingTable::new(&[16, 0, 16], 5).unwrap();
        let symbols: Vec<_> = (0..256).map(|index| (index % 2) as u8).collect();
        let choice = select(&symbols, &predefined, Some(&previous), 35, 9).unwrap();
        assert_ne!(choice.mode(), Mode::Repeat);
        verify_cost(&choice, &symbols);
        let offsets = FseEncodingTable::new(&OF_DEFAULT, 5).unwrap();
        let choice = select(&[31; 2], &offsets, None, 31, 8).unwrap();
        assert_eq!(choice.mode(), Mode::Rle);
        assert_eq!(choice.description(), [31]);

        assert!(matches!(
            select(&[], &predefined, None, 35, 9),
            Err(Error::CorruptData)
        ));
        assert!(matches!(
            select(&[36], &predefined, None, 35, 9),
            Err(Error::CorruptData)
        ));
        for log in [0, 4, 13, u8::MAX] {
            assert!(matches!(
                select(&[0], &predefined, None, 35, log),
                Err(Error::InvalidParameter)
            ));
        }
        for invalid in [
            FseEncodingTable::rle(36),
            FseEncodingTable::new(&[512, 512], 10).unwrap(),
        ] {
            assert!(matches!(
                select(&[0], &invalid, None, 35, 9),
                Err(Error::CorruptData)
            ));
            assert!(matches!(
                select(&[0], &predefined, Some(&invalid), 35, 9),
                Err(Error::CorruptData)
            ));
        }
        assert!(matches!(
            select(&vec![0; MAX_BLOCK_SIZE / 3 + 1], &predefined, None, 35, 9),
            Err(Error::ResourceLimit)
        ));
    }

    fn adaptive_frame() -> (Vec<u8>, Vec<u8>) {
        let defaults = [
            FseEncodingTable::new(&LL_DEFAULT, 6).unwrap(),
            FseEncodingTable::new(&OF_DEFAULT, 5).unwrap(),
            FseEncodingTable::new(&ML_DEFAULT, 6).unwrap(),
        ];
        let mut repeat = [1, 4, 8];
        let plans: Vec<_> = (0..64)
            .map(|index| {
                let plan = PlannedSequence::new(1 + index % 2, 3, 1, repeat).unwrap();
                repeat = plan.next_repeat;
                plan
            })
            .collect();
        let symbols = [
            plans
                .iter()
                .map(|plan| plan.literal_code)
                .collect::<Vec<_>>(),
            plans.iter().map(|plan| plan.offset_code).collect(),
            plans.iter().map(|plan| plan.match_code).collect(),
        ];
        let first = [
            select(&symbols[0], &defaults[0], None, 35, 9).unwrap(),
            select(&symbols[1], &defaults[1], None, 31, 8).unwrap(),
            select(&symbols[2], &defaults[2], None, 52, 9).unwrap(),
        ];
        assert_eq!(
            first.each_ref().map(|choice| choice.mode()),
            [Mode::Compressed, Mode::Rle, Mode::Rle]
        );
        let mut history = History::new(576);
        history.start_frame(576);
        let mut decoder = SequenceState::new();
        let mut expected = Vec::new();
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0xa0];
        frame.extend_from_slice(&576_u32.to_le_bytes());
        let mut append = |choices: &[Selection<'_>; 3], last: bool| {
            let literals: Vec<_> = (0..96)
                .map(|index| (index * 29 + usize::from(last) * 7) as u8)
                .collect();
            let mut position = 0;
            for plan in &plans {
                let end = position + plan.literal_length as usize;
                expected.extend_from_slice(&literals[position..end]);
                let byte = *expected.last().unwrap();
                expected.extend_from_slice(&[byte; 3]);
                position = end;
            }
            let mut writer = BitWriter::new();
            encode_sequences(
                &plans,
                choices[0].table(),
                choices[1].table(),
                choices[2].table(),
                &mut writer,
            )
            .unwrap();
            let bits = ReverseBits::new(writer.as_bytes()).unwrap().remaining();
            assert_eq!(
                bits,
                choices.iter().map(Selection::stream_bits).sum::<usize>()
            );
            let mut section = vec![
                64,
                ((choices[0].mode() as u8) << 6)
                    | ((choices[1].mode() as u8) << 4)
                    | ((choices[2].mode() as u8) << 2),
            ];
            for choice in choices {
                section.extend_from_slice(choice.description());
            }
            section.extend_from_slice(writer.as_bytes());
            decoder
                .decode(&section, &literals, &mut history, MAX_BLOCK_SIZE)
                .unwrap();
            let mut block = (((literals.len() as u16) << 4) | 4).to_le_bytes().to_vec();
            block.extend_from_slice(&literals);
            block.extend_from_slice(&section);
            let header = ((block.len() as u32) << 3) | 4 | u32::from(last);
            frame.extend_from_slice(&header.to_le_bytes()[..3]);
            frame.extend_from_slice(&block);
        };
        append(&first, false);
        let owned = first.map(|choice| choice.into_replacement().unwrap());
        let second = [
            select(&symbols[0], &defaults[0], Some(&owned[0]), 35, 9).unwrap(),
            select(&symbols[1], &defaults[1], Some(&owned[1]), 31, 8).unwrap(),
            select(&symbols[2], &defaults[2], Some(&owned[2]), 52, 9).unwrap(),
        ];
        assert!(second.iter().all(|choice| choice.mode() == Mode::Repeat));
        append(&second, true);
        assert_eq!(history.into_bytes(), expected);
        assert_eq!(crate::decompress(&frame, expected.len()).unwrap(), expected);
        (frame, expected)
    }

    #[test]
    fn independently_selected_channels_decode_new_rle_then_repeat_sections() {
        adaptive_frame();
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN"]
    fn official_codec_decodes_automatically_selected_channel_tables() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let executable = std::env::var_os("ZSTD_BIN").unwrap_or_else(|| "zstd".into());
        let version = Command::new(&executable).arg("--version").output().unwrap();
        assert!(version.status.success());
        assert!(std::string::String::from_utf8_lossy(&version.stdout).contains("1.5.7"));
        let (frame, expected) = adaptive_frame();
        let mut child = Command::new(&executable)
            .args(["-q", "-d", "-c"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&frame).unwrap();
        let decoded = child.wait_with_output().unwrap();
        assert!(
            decoded.status.success(),
            "{}",
            std::string::String::from_utf8_lossy(&decoded.stderr)
        );
        assert_eq!(decoded.stdout, expected);
        std::eprintln!(
            "Official v1.5.7 decoded adaptive new/RLE/repeat frame: {} output bytes",
            expected.len()
        );
    }
}
