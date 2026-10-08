//! A checksummed Zstandard frame encoder.

use alloc::vec::Vec;

use crate::constants::{LL_BASE, LL_BITS, LL_DEFAULT, ML_BASE, ML_BITS, ML_DEFAULT, OF_DEFAULT};
use crate::fse::FseTable;
use crate::matchfinder::MatchTable;
use crate::{Error, Result, SharedDictionary, frame::MAX_BLOCK_SIZE, xxhash};

/// Predefined offset tables provide symbols 0..=28. Larger dictionaries remain
/// valid, but the initial encoder searches only this addressable suffix.
const MAX_DICTIONARY_DISTANCE: usize = (1 << 29) - 4;

/// Compresses input into one checksummed Zstandard frame with known content size.
///
/// Corresponds to the one-shot contract of upstream `ZSTD_compress`, using this
/// implementation's own block-selection strategy rather than upstream levels.
pub fn compress(input: &[u8]) -> Result<Vec<u8>> {
    let blocks = input.len().div_ceil(MAX_BLOCK_SIZE).max(1);
    let capacity = blocks
        .checked_mul(3)
        .and_then(|overhead| overhead.checked_add(17))
        .and_then(|overhead| overhead.checked_add(input.len()))
        .ok_or(Error::ResourceLimit)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| Error::AllocationFailed)?;
    write_frame_header(&mut output, input.len());
    let mut encoder = BlockEncoder::new();
    if input.is_empty() {
        encoder.encode(&[], true, &mut output)?;
    }
    for (index, block) in input.chunks(MAX_BLOCK_SIZE).enumerate() {
        encoder.encode(block, index + 1 == blocks, &mut output)?;
    }
    output.extend_from_slice(&(xxhash::checksum(input) as u32).to_le_bytes());
    Ok(output)
}

/// Reusable block-local match and predefined-entropy state.
///
/// Corresponds to the independently owned block-compression workspace of
/// `ZSTD_CCtx`; input references never survive an `encode` call.
pub(crate) struct BlockEncoder {
    match_table: Option<MatchTable>,
    dictionary_seed: Option<MatchTable>,
    active_dictionary: Option<SharedDictionary>,
    frame_position: u64,
    window_size: u64,
    entropy_tables: Option<EncodingTables>,
    literals: Vec<u8>,
    sequences: Vec<Sequence>,
    payload: Vec<u8>,
    writer: BitWriter,
}

impl BlockEncoder {
    pub(crate) const fn new() -> Self {
        Self {
            match_table: None,
            dictionary_seed: None,
            active_dictionary: None,
            frame_position: 0,
            window_size: MAX_BLOCK_SIZE as u64,
            entropy_tables: None,
            literals: Vec::new(),
            sequences: Vec::new(),
            payload: Vec::new(),
            writer: BitWriter::new(),
        }
    }

    /// Prepare an independently owned frame dictionary and its indexed seeds.
    ///
    /// Corresponds to the dictionary hash preparation in `ZSTD_compressBegin`.
    /// The shared handle keeps every candidate's immutable source allocation
    /// alive; only indices enter the hash table. Reusing this workspace for a
    /// different dictionary always rebuilds its seed indices before publication.
    pub(crate) fn start_frame(
        &mut self,
        dictionary: Option<SharedDictionary>,
        window_size: u64,
    ) -> Result<()> {
        if let Some(dictionary) = dictionary.as_deref()
            && dictionary.content().len() >= 4
        {
            let seed = match self.dictionary_seed.as_mut() {
                Some(seed) => seed,
                None => self.dictionary_seed.insert(MatchTable::new()?),
            };
            seed.reset();
            let content = dictionary.content();
            let start = content.len().saturating_sub(MAX_DICTIONARY_DISTANCE);
            for position in start..=content.len() - 4 {
                seed.replace(word_at(content, position), position);
            }
        } else if let Some(seed) = &mut self.dictionary_seed {
            seed.reset();
        }
        self.active_dictionary = dictionary;
        self.frame_position = 0;
        self.window_size = window_size;
        Ok(())
    }

    /// Drop active-frame ownership while retaining reusable workspace storage.
    pub(crate) fn reset(&mut self) {
        self.active_dictionary = None;
        self.frame_position = 0;
        self.window_size = MAX_BLOCK_SIZE as u64;
    }

    /// Append exactly one bounded raw, RLE, or compressed block.
    pub(crate) fn encode(&mut self, block: &[u8], last: bool, output: &mut Vec<u8>) -> Result<()> {
        let next = self
            .frame_position
            .checked_add(block.len() as u64)
            .ok_or(Error::ResourceLimit)?;
        self.encode_block(block, last, output)?;
        self.frame_position = next;
        Ok(())
    }

    fn encode_block(&mut self, block: &[u8], last: bool, output: &mut Vec<u8>) -> Result<()> {
        if block.len() > MAX_BLOCK_SIZE {
            return Err(Error::ResourceLimit);
        }
        output
            .try_reserve(block.len() + 3)
            .map_err(|_| Error::AllocationFailed)?;
        if block.len() > 1 && block.iter().all(|&byte| byte == block[0]) {
            write_block_header(output, block.len(), 1, last);
            output.push(block[0]);
            return Ok(());
        }
        if block.len() >= 8 && self.compress_block(block)? {
            write_block_header(output, self.payload.len(), 2, last);
            output.extend_from_slice(&self.payload);
            return Ok(());
        }
        write_block_header(output, block.len(), 0, last);
        output.extend_from_slice(block);
        Ok(())
    }

    /// Heap capacity owned by the block workspace, excluding this value record
    /// and shared dictionary storage already reported by the owning context.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.match_table
            .as_ref()
            .map_or(0, MatchTable::retained_capacity)
            + self
                .dictionary_seed
                .as_ref()
                .map_or(0, MatchTable::retained_capacity)
            + self.entropy_tables.as_ref().map_or(0, |tables| {
                tables.literals.retained_capacity()
                    + tables.matches.retained_capacity()
                    + tables.offsets.retained_capacity()
            })
            + self.literals.capacity()
            + self.sequences.capacity() * core::mem::size_of::<Sequence>()
            + self.payload.capacity()
            + self.writer.bytes.capacity()
    }
}

/// Writes a single-segment frame header; corresponds to `ZSTD_writeFrameHeader`.
fn write_frame_header(output: &mut Vec<u8>, content_size: usize) {
    output.extend_from_slice(&crate::frame::MAGIC.to_le_bytes());
    let size = content_size as u64;
    let (descriptor, value, count) = if size < 256 {
        (0x24, size, 1)
    } else if size < 65_792 {
        (0x64, size - 256, 2)
    } else if size <= u64::from(u32::MAX) {
        (0xa4, size, 4)
    } else {
        (0xe4, size, 8)
    };
    output.push(descriptor);
    output.extend_from_slice(&value.to_le_bytes()[..count]);
}

/// Writes a three-byte block header; corresponds to `ZSTD_writeLastEmptyBlock`
/// and the block-header writes in `ZSTD_compressBlock_internal`.
fn write_block_header(output: &mut Vec<u8>, size: usize, kind: u8, last: bool) {
    let header = ((size as u32) << 3) | (u32::from(kind) << 1) | u32::from(last);
    output.extend_from_slice(&header.to_le_bytes()[..3]);
}

/// A plain sequence; lengths and distance are bounded by one 128 KiB block.
#[derive(Clone, Copy, Debug)]
struct Sequence {
    literal_length: u32,
    match_length: u32,
    distance: u32,
}

/// Selects matches greedily and encodes them with predefined entropy tables.
///
/// Fulfills the block-compression role of `ZSTD_compressBlock_internal`, using
/// an independent greedy parser and table inversion instead of C table layout.
impl BlockEncoder {
    fn compress_block(&mut self, block: &[u8]) -> Result<bool> {
        let match_table = match self.match_table.as_mut() {
            Some(table) => table,
            None => self.match_table.insert(MatchTable::new()?),
        };
        match_table.reset();
        let literals = &mut self.literals;
        literals.clear();
        literals
            .try_reserve_exact(block.len())
            .map_err(|_| Error::AllocationFailed)?;
        let sequences = &mut self.sequences;
        sequences.clear();
        sequences
            .try_reserve_exact(block.len() / 4)
            .map_err(|_| Error::AllocationFailed)?;
        let mut position = 0;
        let mut literal_start = 0;
        while position + 4 <= block.len() {
            let candidate = match_table.replace(word_at(block, position), position);
            if let Some(previous) = candidate
                && block[previous..previous + 4] == block[position..position + 4]
            {
                let mut match_length = 4;
                while position + match_length < block.len()
                    && block[previous + match_length] == block[position + match_length]
                {
                    match_length += 1;
                }
                literals.extend_from_slice(&block[literal_start..position]);
                sequences.push(Sequence {
                    literal_length: (position - literal_start) as u32,
                    match_length: match_length as u32,
                    distance: (position - previous) as u32,
                });
                let match_end = position + match_length;
                position += 1;
                while position < match_end && position + 4 <= block.len() {
                    match_table.replace(word_at(block, position), position);
                    position += 1;
                }
                position = match_end;
                literal_start = position;
            } else {
                position += 1;
            }
        }
        if sequences.is_empty() {
            return Ok(false);
        }
        literals.extend_from_slice(&block[literal_start..]);
        let tables = match self.entropy_tables.as_ref() {
            Some(tables) => tables,
            None => self.entropy_tables.insert(EncodingTables::new()?),
        };
        encode_sequences(sequences, tables, &mut self.writer)?;
        let stream = &self.writer.bytes;
        let capacity = literals
            .len()
            .checked_add(stream.len())
            .and_then(|size| size.checked_add(7))
            .ok_or(Error::ResourceLimit)?;
        let payload = &mut self.payload;
        payload.clear();
        payload
            .try_reserve_exact(capacity)
            .map_err(|_| Error::AllocationFailed)?;
        write_literal_header(payload, literals.len());
        payload.extend_from_slice(literals);
        write_sequence_count(payload, sequences.len());
        payload.push(0); // All three symbol distributions are predefined.
        payload.extend_from_slice(stream);
        Ok(payload.len() < block.len())
    }
}

/// Reads a four-byte match key from a previously bounded position.
fn word_at(block: &[u8], position: usize) -> [u8; 4] {
    [
        block[position],
        block[position + 1],
        block[position + 2],
        block[position + 3],
    ]
}

/// Writes a raw-literal header; corresponds to the raw branch of
/// `ZSTD_compressLiterals`.
fn write_literal_header(output: &mut Vec<u8>, length: usize) {
    let (header, count) = if length < 32 {
        ((length as u32) << 3, 1)
    } else if length < 4096 {
        (((length as u32) << 4) | 4, 2)
    } else {
        (((length as u32) << 4) | 12, 3)
    };
    output.extend_from_slice(&header.to_le_bytes()[..count]);
}

/// Writes the variable-width count used by `ZSTD_encodeSequences`.
fn write_sequence_count(output: &mut Vec<u8>, count: usize) {
    if count < 128 {
        output.push(count as u8);
    } else if count < 0x7f00 {
        output.extend_from_slice(&[(128 + (count >> 8)) as u8, count as u8]);
    } else {
        output.push(255);
        output.extend_from_slice(&((count - 0x7f00) as u16).to_le_bytes());
    }
}

/// One inverse transition. The numeric value/bit count encode a forward
/// decoding step from `state` to the requested successor state.
#[derive(Clone, Copy, Default)]
struct EncodingTransition {
    state: u16,
    value: u16,
    bits: u8,
}

/// Typed, initialized inverse transitions indexed by symbol and next state.
struct EncodingTable {
    transitions: Vec<EncodingTransition>,
    initial: Vec<u16>,
    state_count: usize,
    table_log: u8,
}

impl EncodingTable {
    fn retained_capacity(&self) -> usize {
        self.transitions.capacity() * core::mem::size_of::<EncodingTransition>()
            + self.initial.capacity() * core::mem::size_of::<u16>()
    }

    /// Inverts the public decoding relation instead of adopting `FSE_CTable`'s
    /// packed representation. Each symbol partitions the successor state set.
    fn new(counts: &[i16], table_log: u8) -> Result<Self> {
        let decoding = FseTable::from_normalized(counts, table_log)?;
        let state_count = 1_usize << table_log;
        let mut transitions = Vec::new();
        transitions
            .try_reserve_exact(counts.len() * state_count)
            .map_err(|_| Error::AllocationFailed)?;
        transitions.resize(counts.len() * state_count, EncodingTransition::default());
        let mut initial = Vec::new();
        initial
            .try_reserve_exact(counts.len())
            .map_err(|_| Error::AllocationFailed)?;
        initial.resize(counts.len(), u16::MAX);
        for state in 0..state_count {
            let entry = decoding.entry(state)?;
            let symbol = usize::from(entry.symbol);
            if initial[symbol] == u16::MAX {
                initial[symbol] = state as u16;
            }
            for value in 0..(1_u16 << entry.bits) {
                let next = usize::from(entry.base + value);
                transitions[symbol * state_count + next] = EncodingTransition {
                    state: state as u16,
                    value,
                    bits: entry.bits,
                };
            }
        }
        Ok(Self {
            transitions,
            initial,
            state_count,
            table_log,
        })
    }

    fn initial(&self, symbol: usize) -> Result<u16> {
        self.initial
            .get(symbol)
            .copied()
            .filter(|&state| state != u16::MAX)
            .ok_or(Error::CorruptData)
    }

    fn transition(&self, symbol: usize, next: u16) -> Result<EncodingTransition> {
        if usize::from(next) >= self.state_count || symbol >= self.initial.len() {
            return Err(Error::CorruptData);
        }
        let index = symbol * self.state_count + usize::from(next);
        #[cfg(not(feature = "unsafe-fse"))]
        let transition = self.transitions[index];
        #[cfg(feature = "unsafe-fse")]
        // SAFETY: construction creates initial.len() * state_count initialized
        // transitions, and both dimensions are checked above. Thus index is
        // within the live owned vector, whose length and allocation never
        // change after construction. Vec supplies proper alignment and this
        // shared copied read neither aliases a mutable reference nor escapes.
        let transition = unsafe { *self.transitions.get_unchecked(index) };
        Ok(transition)
    }
}

struct EncodingTables {
    literals: EncodingTable,
    matches: EncodingTable,
    offsets: EncodingTable,
}

impl EncodingTables {
    fn new() -> Result<Self> {
        Ok(Self {
            literals: EncodingTable::new(&LL_DEFAULT, 6)?,
            matches: EncodingTable::new(&ML_DEFAULT, 6)?,
            offsets: EncodingTable::new(&OF_DEFAULT, 5)?,
        })
    }
}

/// Finds the code whose interval includes a previously bounded length.
fn length_code(length: u32, bases: &[u32]) -> usize {
    bases.partition_point(|&base| base <= length) - 1
}

/// Writes sequences backward so their symbols and extra fields decode forward.
/// Corresponds to the format contract of `ZSTD_encodeSequences`.
fn encode_sequences(
    sequences: &[Sequence],
    tables: &EncodingTables,
    writer: &mut BitWriter,
) -> Result<()> {
    writer.reset();
    let (mut literal_state, mut match_state, mut offset_state) = (0, 0, 0);
    for (index, sequence) in sequences.iter().rev().enumerate() {
        let literal_code = length_code(sequence.literal_length, &LL_BASE);
        let match_code = length_code(sequence.match_length, &ML_BASE);
        let offset_value = sequence.distance + 3;
        let offset_code = (31 - offset_value.leading_zeros()) as usize;
        if index == 0 {
            literal_state = tables.literals.initial(literal_code)?;
            match_state = tables.matches.initial(match_code)?;
            offset_state = tables.offsets.initial(offset_code)?;
        } else {
            // Decoder updates LL, ML, then OF. Write their inverse transitions
            // in the opposite order, before this sequence's extra fields.
            let offset = tables.offsets.transition(offset_code, offset_state)?;
            let matches = tables.matches.transition(match_code, match_state)?;
            let literals = tables.literals.transition(literal_code, literal_state)?;
            writer.write(u32::from(offset.value), offset.bits)?;
            writer.write(u32::from(matches.value), matches.bits)?;
            writer.write(u32::from(literals.value), literals.bits)?;
            offset_state = offset.state;
            match_state = matches.state;
            literal_state = literals.state;
        }
        // Decoder consumes OF, ML, then LL extra fields.
        writer.write(
            sequence.literal_length - LL_BASE[literal_code],
            LL_BITS[literal_code],
        )?;
        writer.write(
            sequence.match_length - ML_BASE[match_code],
            ML_BITS[match_code],
        )?;
        writer.write(offset_value - (1_u32 << offset_code), offset_code as u8)?;
    }
    writer.write(u32::from(match_state), tables.matches.table_log)?;
    writer.write(u32::from(offset_state), tables.offsets.table_log)?;
    writer.write(u32::from(literal_state), tables.literals.table_log)?;
    writer.finish()
}

/// A forward writer for fields consumed by `ReverseBits` in reverse order.
/// Storage is owned; unsafe-bitstream optionally initializes spare capacity
/// directly before increasing the length. Reservoir arithmetic stays shared.
struct BitWriter {
    bytes: Vec<u8>,
    pending: u64,
    used: u8,
}

impl BitWriter {
    const fn new() -> Self {
        Self {
            bytes: Vec::new(),
            pending: 0,
            used: 0,
        }
    }

    fn reset(&mut self) {
        self.bytes.clear();
        self.pending = 0;
        self.used = 0;
    }

    fn write(&mut self, value: u32, count: u8) -> Result<()> {
        if count > 32 || u64::from(value) >= (1_u64 << count) {
            return Err(Error::CorruptData);
        }
        // used < 8 and count <= 32, so at most four bytes will be emitted.
        self.bytes
            .try_reserve(4)
            .map_err(|_| Error::AllocationFailed)?;
        self.pending |= u64::from(value) << self.used;
        self.used += count;
        let emitted = usize::from(self.used / 8);
        #[cfg(not(feature = "unsafe-bitstream"))]
        for _ in 0..emitted {
            self.bytes.push(self.pending as u8);
            self.pending >>= 8;
        }
        #[cfg(feature = "unsafe-bitstream")]
        {
            let previous_length = self.bytes.len();
            for slot in self.bytes.spare_capacity_mut().iter_mut().take(emitted) {
                slot.write(self.pending as u8);
                self.pending >>= 8;
            }
            // SAFETY: used was below 8 before adding count <= 32, so emitted
            // is at most four. try_reserve(4) succeeded above, proving enough
            // spare capacity and no usize overflow in previous_length + 4.
            // The loop writes exactly emitted initialized u8 values, and its
            // exclusive spare-capacity borrow has ended. The original prefix
            // remains initialized; the new length is at most the capacity.
            unsafe { self.bytes.set_len(previous_length + emitted) };
        }
        self.used %= 8;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        self.write(1, 1)?;
        if self.used != 0 {
            self.bytes.push(self.pending as u8);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dictionary;
    use alloc::vec;

    #[test]
    fn frame_dictionary_seed_owns_its_source_and_reset_retains_only_workspace() {
        let first = Dictionary::raw(b"dictionary seed content")
            .unwrap()
            .into_shared();
        let second = Dictionary::raw(b"replacement source bytes")
            .unwrap()
            .into_shared();
        let mut encoder = BlockEncoder::new();
        encoder
            .start_frame(Some(SharedDictionary::clone(&first)), 1024)
            .unwrap();
        assert_eq!(SharedDictionary::strong_count(&first), 2);
        assert_eq!(encoder.window_size, 1024);
        let capacity = encoder.retained_capacity();
        assert!(capacity > 0);
        let mut working = MatchTable::new().unwrap();
        working.reset_from(encoder.dictionary_seed.as_ref().unwrap());
        assert_eq!(working.replace(*b"dict", first.content().len()), Some(0));
        encoder.reset();
        assert_eq!(SharedDictionary::strong_count(&first), 1);
        assert!(encoder.active_dictionary.is_none());
        assert_eq!(encoder.frame_position, 0);
        assert_eq!(encoder.retained_capacity(), capacity);

        encoder
            .start_frame(Some(SharedDictionary::clone(&second)), 4096)
            .unwrap();
        assert_eq!(SharedDictionary::strong_count(&second), 2);
        working.reset_from(encoder.dictionary_seed.as_ref().unwrap());
        assert_eq!(working.replace(*b"repl", second.content().len()), Some(0));
        assert_eq!(working.replace(*b"dict", second.content().len()), None);
        assert_eq!(encoder.retained_capacity(), capacity);
        encoder.start_frame(None, 128 * 1024).unwrap();
        assert_eq!(SharedDictionary::strong_count(&second), 1);
        working.reset_from(encoder.dictionary_seed.as_ref().unwrap());
        assert_eq!(working.replace(*b"repl", 100), None);
    }

    #[test]
    fn frame_position_advances_for_every_block_kind_and_checks_overflow() {
        let mut encoder = BlockEncoder::new();
        let mut output = Vec::new();
        for block in [
            &b"abc"[..],
            &b"xxxxxxxx"[..],
            &b"pattern pattern pattern pattern"[..],
        ] {
            let before = encoder.frame_position;
            encoder.encode(block, false, &mut output).unwrap();
            assert_eq!(encoder.frame_position, before + block.len() as u64);
        }
        encoder.start_frame(None, 1024).unwrap();
        assert_eq!(encoder.frame_position, 0);
        encoder.frame_position = u64::MAX;
        let length = output.len();
        assert_eq!(
            encoder.encode(b"x", true, &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(output.len(), length);
        assert_eq!(encoder.frame_position, u64::MAX);
    }

    #[test]
    fn empty_frame_has_a_final_empty_block_and_checksum() {
        let output = compress(&[]).unwrap();
        assert_eq!(
            output,
            [
                0x28, 0xb5, 0x2f, 0xfd, 0x24, 0, 1, 0, 0, 0x99, 0xe9, 0xd8, 0x51
            ]
        );
    }

    #[test]
    fn content_size_uses_the_smallest_supported_field() {
        for (length, descriptor, header_size) in [
            (0, 0x24, 6),
            (255, 0x24, 6),
            (256, 0x64, 7),
            (65_791, 0x64, 7),
            (65_792, 0xa4, 9),
        ] {
            let output = compress(&vec![7; length]).unwrap();
            assert_eq!(output[4], descriptor);
            let (header, consumed) = crate::frame::FrameHeader::parse(&output).unwrap();
            assert_eq!(consumed, header_size);
            assert_eq!(header.content_size, Some(length as u64));
            assert!(header.has_checksum);
        }
    }

    #[test]
    fn repeated_blocks_use_rle_and_set_only_the_last_flag() {
        let input = vec![42; MAX_BLOCK_SIZE * 2 + 1];
        let output = compress(&input).unwrap();
        let (_, mut position) = crate::frame::FrameHeader::parse(&output).unwrap();
        for (size, kind, last) in [
            (MAX_BLOCK_SIZE, 1, false),
            (MAX_BLOCK_SIZE, 1, false),
            (1, 0, true),
        ] {
            let header = u32::from_le_bytes([
                output[position],
                output[position + 1],
                output[position + 2],
                0,
            ]);
            assert_eq!(header >> 3, size as u32);
            assert_eq!((header >> 1) & 3, kind);
            assert_eq!(header & 1, u32::from(last));
            assert_eq!(output[position + 3], 42);
            position += 4;
        }
        assert_eq!(position + 4, output.len());
    }

    #[test]
    fn distinct_bytes_are_stored_in_a_raw_block() {
        let output = compress(b"abcdef").unwrap();
        assert_eq!(&output[6..9], &[49, 0, 0]);
        assert_eq!(&output[9..15], b"abcdef");
    }

    #[test]
    fn periodic_input_uses_real_compressed_blocks() {
        let input: Vec<_> = (0..MAX_BLOCK_SIZE * 2 + 31)
            .map(|index| b"periodic-pattern"[index % 16])
            .collect();
        let output = compress(&input).unwrap();
        assert!(output.len() < input.len() / 100);
        let (_, header_size) = crate::frame::FrameHeader::parse(&output).unwrap();
        assert_eq!((output[header_size] >> 1) & 3, 2);
        assert_eq!(crate::decompress(&output, input.len()).unwrap(), input);
    }

    #[test]
    fn block_workspace_reuses_capacity_without_preserving_previous_matches() {
        let block: Vec<_> = (0..MAX_BLOCK_SIZE)
            .map(|index| b"periodic-pattern"[index % 16])
            .collect();
        let mut encoder = BlockEncoder::new();
        let mut encoded = Vec::new();
        encoder.encode(&block, true, &mut encoded).unwrap();
        let expected = encoded.clone();
        let capacity = encoder.retained_capacity();
        assert!(capacity > MAX_BLOCK_SIZE);
        for _ in 0..8 {
            encoded.clear();
            encoder
                .encode(b"a different preceding block", false, &mut encoded)
                .unwrap();
            encoded.clear();
            encoder.encode(&block, true, &mut encoded).unwrap();
            assert_eq!(encoded, expected);
            assert_eq!(encoder.retained_capacity(), capacity);
        }
    }

    #[test]
    fn inverse_entropy_transitions_recover_every_successor() {
        for (counts, log) in [
            (&LL_DEFAULT[..], 6),
            (&ML_DEFAULT[..], 6),
            (&OF_DEFAULT[..], 5),
        ] {
            let encoding = EncodingTable::new(counts, log).unwrap();
            let decoding = FseTable::from_normalized(counts, log).unwrap();
            for symbol in 0..counts.len() {
                for next in 0..1_u16 << log {
                    let transition = encoding.transition(symbol, next).unwrap();
                    let entry = decoding.entry(usize::from(transition.state)).unwrap();
                    assert_eq!(usize::from(entry.symbol), symbol);
                    assert_eq!(entry.bits, transition.bits);
                    assert_eq!(entry.base + transition.value, next);
                }
            }
        }
    }

    #[test]
    fn reverse_fields_preserve_zero_and_full_width_values() {
        let fields = [(0, 0), (5, 3), (u32::MAX, 32), (0x1234_5678, 29), (1, 1)];
        let mut writer = BitWriter::new();
        assert!(writer.write(1, 0).is_err());
        assert!(writer.write(8, 3).is_err());
        assert!(writer.write(0, 33).is_err());
        for &(value, count) in &fields {
            writer.write(value, count).unwrap();
        }
        writer.finish().unwrap();
        let mut reader = crate::bitstream::ReverseBits::new(&writer.bytes).unwrap();
        for &(value, count) in fields.iter().rev() {
            assert_eq!(reader.read_bits(count).unwrap(), value);
        }
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn sequence_count_boundaries_and_overlapping_expansion_round_trip() {
        let tables = EncodingTables::new().unwrap();
        for count in [1, 127, 128, 255, 256, 0x7eff, 0x7f00] {
            let mut sequences = vec![
                Sequence {
                    literal_length: 0,
                    match_length: 4,
                    distance: 1
                };
                count
            ];
            sequences[0].literal_length = 1;
            let mut section = Vec::new();
            write_sequence_count(&mut section, count);
            section.push(0);
            let mut writer = BitWriter::new();
            encode_sequences(&sequences, &tables, &mut writer).unwrap();
            section.extend_from_slice(&writer.bytes);
            let length = count * 4 + 1;
            let mut history = crate::history::History::new(length);
            history.start_frame(length);
            crate::sequences::SequenceState::new()
                .decode(&section, b"x", &mut history, MAX_BLOCK_SIZE)
                .unwrap();
            assert_eq!(history.into_bytes(), vec![b'x'; length]);
        }
    }
}
