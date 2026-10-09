//! Complete literal-section selection with unpublished table ownership.

use alloc::vec::Vec;

use crate::bitwriter::BitWriter;
use crate::frame::MAX_BLOCK_SIZE;
use crate::huffman_codebook::HuffmanCodebook;
use crate::huffman_encode::build_sparse_literal_table;
use crate::literal_header::LiteralHeader;
use crate::{Error, Result};

/// Reusable owned scratch. Table lifetime belongs to the enclosing block
/// encoder, which publishes a returned codebook only when it emits this section.
pub(crate) struct LiteralEncoder {
    writer: BitWriter,
    streams: Vec<u8>,
    section: Vec<u8>,
}

struct FormatChoice {
    header: LiteralHeader,
    four_streams: bool,
    payload_size: usize,
    section_size: usize,
}

impl LiteralEncoder {
    pub(crate) const fn new() -> Self {
        Self {
            writer: BitWriter::new(),
            streams: Vec::new(),
            section: Vec::new(),
        }
    }

    /// Select by complete serialized size, including all headers and markers.
    /// Ties preserve raw/RLE first, then an existing tree, then a new tree;
    /// one stream wins a tie against four. The previous codebook is immutable.
    /// A returned table is pending: discarding this block must also discard it.
    pub(crate) fn select(
        &mut self,
        input: &[u8],
        previous: Option<&HuffmanCodebook>,
    ) -> Result<Option<HuffmanCodebook>> {
        if input.len() > MAX_BLOCK_SIZE {
            return Err(Error::CorruptData);
        }
        let repeated = input.len() > 1 && input.iter().all(|&symbol| symbol == input[0]);
        let header = LiteralHeader::raw(input.len(), repeated)?;
        self.section.clear();
        self.section
            .try_reserve_exact(input.len() + 5)
            .map_err(|_| Error::AllocationFailed)?;
        self.section.extend_from_slice(header.as_bytes());
        self.section
            .extend_from_slice(if repeated { &input[..1] } else { input });
        if input.len() < 2 || repeated {
            return Ok(None);
        }
        if let Some(previous) = previous
            && previous.covers(input)
            && let Some(choice) = choose_format(previous, input, 0, true)?
            && choice.section_size < self.section.len()
        {
            self.write_candidate(previous, input, &[], choice)?;
        }
        let mut frequencies = [0_u64; 256];
        for &symbol in input {
            frequencies[usize::from(symbol)] += 1;
        }
        let table = build_sparse_literal_table(&frequencies)?;
        if let Some(choice) =
            choose_format(table.codebook(), input, table.description().len(), false)?
            && choice.section_size < self.section.len()
        {
            self.write_candidate(table.codebook(), input, table.description(), choice)?;
            return Ok(Some(table.into_codebook()));
        }
        Ok(None)
    }

    fn write_candidate(
        &mut self,
        codebook: &HuffmanCodebook,
        input: &[u8],
        description: &[u8],
        choice: FormatChoice,
    ) -> Result<()> {
        if choice.four_streams {
            codebook.encode_four_streams(input, &mut self.writer, &mut self.streams)?;
        } else {
            codebook.encode_stream(input, &mut self.writer)?;
        }
        let encoded = if choice.four_streams {
            &self.streams[..]
        } else {
            self.writer.as_bytes()
        };
        if encoded.len() + description.len() != choice.payload_size {
            return Err(Error::InvalidState);
        }
        // Every selected candidate is smaller than the raw/RLE section whose
        // initialized capacity was reserved before selection. Reserve explicitly
        // as well so future independent selection changes keep this fallible.
        self.section
            .try_reserve(choice.section_size.saturating_sub(self.section.len()))
            .map_err(|_| Error::AllocationFailed)?;
        self.section.clear();
        self.section.extend_from_slice(choice.header.as_bytes());
        self.section.extend_from_slice(description);
        self.section.extend_from_slice(encoded);
        Ok(())
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.section
    }

    pub(crate) fn retained_capacity(&self) -> usize {
        self.section.capacity() + self.streams.capacity() + self.writer.retained_capacity()
    }
}

/// Predict exact stream lengths from code widths before encoding a candidate.
/// Block bounds limit arithmetic to at most twelve bits per input byte.
fn choose_format(
    codebook: &HuffmanCodebook,
    input: &[u8],
    description_size: usize,
    reused: bool,
) -> Result<Option<FormatChoice>> {
    let quarter = input.len().div_ceil(4).max(1);
    let mut bits = [0_usize; 4];
    for (index, &symbol) in input.iter().enumerate() {
        bits[(index / quarter).min(3)] += usize::from(codebook.code(symbol)?.bits);
    }
    let single_size = (bits.iter().sum::<usize>() + 1).div_ceil(8) + description_size;
    let mut selected = LiteralHeader::huffman(input.len(), single_size, false, reused)
        .ok()
        .map(|header| {
            let section_size = header.as_bytes().len() + single_size;
            FormatChoice {
                header,
                four_streams: false,
                payload_size: single_size,
                section_size,
            }
        });
    if input.len() >= 6 {
        let four_size = bits
            .iter()
            .map(|&size| (size + 1).div_ceil(8))
            .sum::<usize>()
            + 6
            + description_size;
        if let Ok(header) = LiteralHeader::huffman(input.len(), four_size, true, reused) {
            let section_size = header.as_bytes().len() + four_size;
            if selected
                .as_ref()
                .is_none_or(|previous| section_size < previous.section_size)
            {
                selected = Some(FormatChoice {
                    header,
                    four_streams: true,
                    payload_size: four_size,
                    section_size,
                });
            }
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::huffman::{HuffmanTable, decode_literals};
    use alloc::vec;

    #[test]
    fn raw_rle_new_and_repeated_tables_round_trip_with_exact_section_costs() {
        let mut encoder = LiteralEncoder::new();
        let mut previous_decode = None;
        for input in [vec![], vec![7], (0..=255).collect(), vec![42; 4096]] {
            assert!(encoder.select(&input, None).unwrap().is_none());
            let (decoded, consumed) =
                decode_literals(encoder.as_bytes(), &mut previous_decode, MAX_BLOCK_SIZE).unwrap();
            assert_eq!(decoded, input);
            assert_eq!(consumed, encoder.as_bytes().len());
        }
        let input: Vec<_> = (0..4096).map(|index| (index % 16) as u8).collect();
        let current = encoder.select(&input, None).unwrap().unwrap();
        assert_eq!(encoder.as_bytes()[0] & 3, 2);
        assert!(encoder.as_bytes().len() < input.len() * 3 / 4);
        assert_eq!(
            decode_literals(encoder.as_bytes(), &mut previous_decode, MAX_BLOCK_SIZE)
                .unwrap()
                .0,
            input
        );
        let new_size = encoder.as_bytes().len();
        assert!(encoder.select(&input, Some(&current)).unwrap().is_none());
        assert_eq!(encoder.as_bytes()[0] & 3, 3);
        assert!(encoder.as_bytes().len() < new_size);
        assert_eq!(
            decode_literals(encoder.as_bytes(), &mut previous_decode, MAX_BLOCK_SIZE)
                .unwrap()
                .0,
            input
        );
        assert!(encoder.retained_capacity() >= encoder.as_bytes().len());
    }

    #[test]
    fn raw_fallback_leaves_the_callers_reusable_tree_unchanged() {
        let (table, _) = HuffmanTable::read(&[128, 0x10]).unwrap();
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        let mut encoder = LiteralEncoder::new();
        assert!(
            encoder
                .select(&[0, 1, 0], Some(&codebook))
                .unwrap()
                .is_none()
        );
        assert_eq!(encoder.as_bytes()[0] & 3, 0);
        assert!(codebook.covers(&[0, 1]));
        assert!(!codebook.covers(&[2]));
        assert!(
            encoder
                .select(&[3; 100], Some(&codebook))
                .unwrap()
                .is_none()
        );
        assert_eq!(encoder.as_bytes()[0] & 3, 1);
        assert!(
            encoder
                .select(&vec![0; MAX_BLOCK_SIZE + 1], Some(&codebook))
                .is_err()
        );
        assert_eq!(codebook.table_log(), 1);
    }

    #[test]
    fn estimated_stream_sizes_equal_encoded_sizes_across_width_boundaries() {
        let dictionary =
            include_bytes!("../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols");
        let (table, _) = HuffmanTable::read_dictionary(&dictionary[8..]).unwrap();
        let codebook = HuffmanCodebook::from_decoding_table(&table).unwrap();
        let mut writer = BitWriter::new();
        let mut streams = Vec::new();
        for size in [
            0,
            1,
            5,
            6,
            7,
            9,
            11,
            12,
            255,
            256,
            1023,
            1024,
            16383,
            16384,
            MAX_BLOCK_SIZE,
        ] {
            let source = vec![0; size];
            let choice = choose_format(&codebook, &source, 0, true).unwrap().unwrap();
            let actual = if choice.four_streams {
                codebook
                    .encode_four_streams(&source, &mut writer, &mut streams)
                    .unwrap();
                streams.len()
            } else {
                codebook.encode_stream(&source, &mut writer).unwrap();
                writer.as_bytes().len()
            };
            assert_eq!(choice.payload_size, actual);
            assert_eq!(choice.section_size, actual + choice.header.as_bytes().len());
        }
    }
}
