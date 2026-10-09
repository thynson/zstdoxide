//! A checksummed Zstandard frame encoder.

use alloc::vec::Vec;

use crate::constants::{LL_BASE, LL_BITS, LL_DEFAULT, ML_BASE, ML_BITS, ML_DEFAULT, OF_DEFAULT};
use crate::fse::FseTable;
use crate::matchfinder::MatchTable;
use crate::ring_history::RingHistory;
use crate::{Dictionary, Error, Result, SharedDictionary, frame::MAX_BLOCK_SIZE, xxhash};

/// Predefined offset tables provide symbols 0..=28. Larger dictionaries remain
/// valid, but the initial encoder searches only this addressable suffix.
const MAX_DICTIONARY_DISTANCE: usize = (1 << 29) - 4;

/// Compresses input into one checksummed Zstandard frame with known content size.
///
/// Corresponds to the one-shot contract of upstream `ZSTD_compress`, using this
/// implementation's own block-selection strategy rather than upstream levels.
pub fn compress(input: &[u8]) -> Result<Vec<u8>> {
    compress_frame(input, None)
}

/// Compresses input into a checksummed frame using owned dictionary content.
///
/// A formatted dictionary's nonzero identifier is written using its smallest
/// field; a raw dictionary has no identifier. Input size excludes dictionary
/// bytes. Matches use this encoder's greedy parser and predefined entropy
/// tables, independently of prepared dictionary entropy and repeat offsets.
/// Corresponds to the one-shot contract of upstream `ZSTD_compress_usingCDict`.
pub fn compress_with_dictionary(input: &[u8], dictionary: &SharedDictionary) -> Result<Vec<u8>> {
    compress_frame(input, Some(dictionary))
}

fn compress_frame(input: &[u8], dictionary: Option<&SharedDictionary>) -> Result<Vec<u8>> {
    let blocks = input.len().div_ceil(MAX_BLOCK_SIZE).max(1);
    let dictionary_id = dictionary.map_or(0, |dictionary| dictionary.id());
    let (_, dictionary_bytes) = dictionary_field(dictionary_id);
    let capacity = blocks
        .checked_mul(3)
        .and_then(|overhead| overhead.checked_add(17))
        .and_then(|overhead| overhead.checked_add(dictionary_bytes))
        .and_then(|overhead| overhead.checked_add(input.len()))
        .ok_or(Error::ResourceLimit)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| Error::AllocationFailed)?;
    write_frame_header(&mut output, input.len(), dictionary_id);
    let mut encoder = BlockEncoder::new();
    encoder.start_frame(dictionary.cloned(), input.len() as u64)?;
    if input.is_empty() {
        encoder.encode(&[], true, &mut output)?;
    }
    for (index, block) in input.chunks(MAX_BLOCK_SIZE).enumerate() {
        encoder.encode(block, index + 1 == blocks, &mut output)?;
    }
    output.extend_from_slice(&(xxhash::checksum(input) as u32).to_le_bytes());
    Ok(output)
}

/// Reusable frame match history and block-local predefined-entropy state.
///
/// Corresponds to the independently owned block-compression workspace of
/// `ZSTD_CCtx`; input references never survive an `encode` call.
pub(crate) struct BlockEncoder {
    match_table: Option<MatchTable>,
    dictionary_seed: Option<MatchTable>,
    active_dictionary: Option<SharedDictionary>,
    history: Option<RingHistory>,
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
            history: None,
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
        if let Some(history) = &mut self.history {
            history.clear();
        }
        self.frame_position = 0;
        self.window_size = window_size;
        Ok(())
    }

    /// Drop active-frame ownership while retaining reusable workspace storage.
    pub(crate) fn reset(&mut self) {
        self.active_dictionary = None;
        if let Some(history) = &mut self.history {
            history.clear();
        }
        self.frame_position = 0;
        self.window_size = MAX_BLOCK_SIZE as u64;
    }

    /// Append exactly one bounded raw, RLE, or compressed block.
    pub(crate) fn encode(&mut self, block: &[u8], last: bool, output: &mut Vec<u8>) -> Result<()> {
        let next = self
            .frame_position
            .checked_add(block.len() as u64)
            .ok_or(Error::ResourceLimit)?;
        if block.len() > MAX_BLOCK_SIZE || block.len() as u64 > self.window_size {
            return Err(Error::ResourceLimit);
        }
        if !block.is_empty() {
            let history = self
                .history
                .get_or_insert_with(|| RingHistory::new(MAX_BLOCK_SIZE));
            if history.window_size() == 0 {
                let horizon = self.window_size.min(MAX_BLOCK_SIZE as u64) as usize;
                history.start_frame(horizon)?;
            }
            if history.frame_len() != self.frame_position {
                return Err(Error::InvalidState);
            }
        }
        self.encode_block(block, last, output)?;
        if let Some(history) = &mut self.history {
            // The matching frame counter, checked complete advancement and
            // reserved positive window above establish commit's bounds before
            // encoded output is appended. Commit copies original input for all
            // block kinds and performs no new allocation.
            history.commit(block)?;
        }
        self.frame_position = next;
        Ok(())
    }

    fn encode_block(&mut self, block: &[u8], last: bool, output: &mut Vec<u8>) -> Result<()> {
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
            + self
                .history
                .as_ref()
                .map_or(0, RingHistory::retained_capacity)
    }
}

/// Writes a single-segment frame header; corresponds to `ZSTD_writeFrameHeader`.
fn write_frame_header(output: &mut Vec<u8>, content_size: usize, dictionary_id: u32) {
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
    let (dictionary_flag, dictionary_bytes) = dictionary_field(dictionary_id);
    output.push(descriptor | dictionary_flag);
    output.extend_from_slice(&dictionary_id.to_le_bytes()[..dictionary_bytes]);
    output.extend_from_slice(&value.to_le_bytes()[..count]);
}

fn dictionary_field(dictionary_id: u32) -> (u8, usize) {
    match dictionary_id {
        0 => (0, 0),
        1..=255 => (1, 1),
        256..=65535 => (2, 2),
        _ => (3, 4),
    }
}

/// Writes a three-byte block header; corresponds to `ZSTD_writeLastEmptyBlock`
/// and the block-header writes in `ZSTD_compressBlock_internal`.
fn write_block_header(output: &mut Vec<u8>, size: usize, kind: u8, last: bool) {
    let header = ((size as u32) << 3) | (u32::from(kind) << 1) | u32::from(last);
    output.extend_from_slice(&header.to_le_bytes()[..3]);
}

/// A plain sequence with block-bounded lengths and an encodable match distance.
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
        let dictionary = self.active_dictionary.as_deref().filter(|dictionary| {
            !dictionary.content().is_empty() && self.frame_position <= self.window_size
        });
        let source = MatchSource {
            block,
            dictionary,
            dictionary_size: dictionary.map_or(0, |dictionary| dictionary.content().len()),
            history: self.history.as_ref(),
            history_size: self.history.as_ref().map_or(0, RingHistory::available),
            frame_position: self.frame_position,
            window_size: self.window_size,
        };
        let match_table = match self.match_table.as_mut() {
            Some(table) => table,
            None => self.match_table.insert(MatchTable::new()?),
        };
        if dictionary.is_some()
            && let Some(seed) = &self.dictionary_seed
        {
            match_table.reset_from(seed);
        } else {
            match_table.reset();
        }
        if dictionary.is_some() && source.retains_frame_start() {
            // Keys may straddle dictionary end and retained frame[0]. This also
            // makes one-, two-, and three-byte raw prefixes useful.
            for position in source.dictionary_size.saturating_sub(3)..source.dictionary_size {
                let word = source.dictionary_boundary_word(position)?;
                match_table.replace(word, position);
            }
        }
        for position in 0..source.history_size {
            // Include the final three history positions: their four-byte keys
            // can cross into current input, which remains borrowed immutably.
            match_table.replace(
                source.output_word(position)?,
                source
                    .dictionary_size
                    .checked_add(position)
                    .ok_or(Error::ResourceLimit)?,
            );
        }
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
            let virtual_position = source.virtual_position(position)?;
            let candidate = match_table.replace(word_at(block, position), virtual_position);
            if let Some(previous) = candidate
                && let Some((distance, match_length)) = source.find_match(previous, position)?
            {
                literals.extend_from_slice(&block[literal_start..position]);
                sequences.push(Sequence {
                    literal_length: (position - literal_start) as u32,
                    match_length: match_length as u32,
                    distance: distance as u32,
                });
                let match_end = position + match_length;
                position += 1;
                while position < match_end && position + 4 <= block.len() {
                    match_table
                        .replace(word_at(block, position), source.virtual_position(position)?);
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

/// A temporary indexed view of dictionary content, retained output, and input.
///
/// No reference escapes the current encode call. Dictionary positions are in
/// `0..D`, retained history in `D..D+H`, and block positions use checked
/// `D + H + local_position` coordinates. Frame position remains a separate u64
/// so long streams never truncate into indices. The ring is immutable until
/// matching completes; later current-block positions may still refer to its
/// oldest byte if the advertised window permits that complete distance.
struct MatchSource<'a> {
    block: &'a [u8],
    dictionary: Option<&'a Dictionary>,
    dictionary_size: usize,
    history: Option<&'a RingHistory>,
    history_size: usize,
    frame_position: u64,
    window_size: u64,
}

impl MatchSource<'_> {
    fn virtual_position(&self, position: usize) -> Result<usize> {
        self.dictionary_size
            .checked_add(self.history_size)
            .and_then(|start| start.checked_add(position))
            .ok_or(Error::ResourceLimit)
    }

    fn retains_frame_start(&self) -> bool {
        self.frame_position == self.history_size as u64
    }

    /// Read a four-byte key across dictionary/frame-zero while it remains live.
    fn dictionary_boundary_word(&self, position: usize) -> Result<[u8; 4]> {
        if !self.retains_frame_start() {
            return Err(Error::InvalidState);
        }
        let mut word = [0; 4];
        for (offset, byte) in word.iter_mut().enumerate() {
            let index = position.checked_add(offset).ok_or(Error::ResourceLimit)?;
            *byte = if index < self.dictionary_size {
                self.dictionary
                    .ok_or(Error::InvalidState)?
                    .content_byte(index)?
            } else {
                self.frame_byte((index - self.dictionary_size) as u64)?
            };
        }
        Ok(word)
    }

    /// Read a frame byte without mutating the retained prefix or input block.
    fn frame_byte(&self, position: u64) -> Result<u8> {
        if position < self.frame_position {
            self.history.ok_or(Error::CorruptData)?.byte_at(position)
        } else {
            let local =
                usize::try_from(position - self.frame_position).map_err(|_| Error::CorruptData)?;
            self.block.get(local).copied().ok_or(Error::CorruptData)
        }
    }

    fn output_byte(&self, position: usize) -> Result<u8> {
        if position >= self.history_size {
            self.block
                .get(position - self.history_size)
                .copied()
                .ok_or(Error::CorruptData)
        } else {
            let frame_start = self
                .frame_position
                .checked_sub(self.history_size as u64)
                .ok_or(Error::CorruptData)?;
            self.frame_byte(frame_start + position as u64)
        }
    }

    fn output_word(&self, position: usize) -> Result<[u8; 4]> {
        let mut word = [0; 4];
        for (offset, byte) in word.iter_mut().enumerate() {
            *byte = self.output_byte(position.checked_add(offset).ok_or(Error::ResourceLimit)?)?;
        }
        Ok(word)
    }

    /// Verify a hash candidate and return its explicit distance and match length.
    fn find_match(&self, previous: usize, position: usize) -> Result<Option<(usize, usize)>> {
        let available = self
            .block
            .len()
            .checked_sub(position)
            .ok_or(Error::CorruptData)?;
        if available < 4 {
            return Ok(None);
        }
        if previous >= self.dictionary_size {
            let previous = previous - self.dictionary_size;
            let current = self
                .history_size
                .checked_add(position)
                .ok_or(Error::ResourceLimit)?;
            if previous >= current {
                return Ok(None);
            }
            let distance = current - previous;
            if distance as u64 > self.window_size {
                return Ok(None);
            }
            if previous >= self.history_size {
                let previous = previous - self.history_size;
                if self.block[previous..previous + 4] != self.block[position..position + 4] {
                    return Ok(None);
                }
                let mut length = 4;
                while length < available
                    && self.block[previous + length] == self.block[position + length]
                {
                    length += 1;
                }
                return Ok(Some((distance, length)));
            }
            let mut length = 0;
            while length < available
                && self.output_byte(previous + length)? == self.block[position + length]
            {
                length += 1;
            }
            return Ok((length >= 4).then_some((distance, length)));
        }
        let produced = self
            .frame_position
            .checked_add(position as u64)
            .ok_or(Error::ResourceLimit)?;
        if produced > self.window_size {
            return Ok(None);
        }
        let tail = self.dictionary_size - previous;
        let distance = produced
            .checked_add(tail as u64)
            .ok_or(Error::ResourceLimit)?;
        if distance > MAX_DICTIONARY_DISTANCE as u64 {
            return Ok(None);
        }
        let dictionary = self.dictionary.ok_or(Error::InvalidState)?;
        // The bounded ring may evict frame byte zero even while a larger
        // advertised window still allows dictionary references. In that case,
        // clip at dictionary end; this is a search limit, not a format limit.
        let maximum = if self.retains_frame_start() {
            available
        } else {
            available.min(tail)
        };
        let mut length = 0;
        while length < maximum {
            let expected = if length < tail {
                dictionary.content_byte(previous + length)?
            } else {
                self.frame_byte((length - tail) as u64)?
            };
            if expected != self.block[position + length] {
                break;
            }
            length += 1;
        }
        Ok((length >= 4).then_some((distance as usize, length)))
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
    use crate::{DecodeOptions, Dictionary, decompress_with_dictionary};
    use alloc::vec;

    #[test]
    fn retained_candidates_cross_input_boundaries_and_obey_the_advertised_window() {
        let mut history = RingHistory::new(8);
        history.start_frame(8).unwrap();
        history.commit(b"abababab").unwrap();
        let mut source = MatchSource {
            block: b"abababab",
            dictionary: None,
            dictionary_size: 0,
            history: Some(&history),
            history_size: 8,
            frame_position: 8,
            window_size: 8,
        };
        assert_eq!(source.output_word(6).unwrap(), *b"abab");
        assert_eq!(source.find_match(6, 0).unwrap(), Some((2, 8)));
        assert_eq!(source.find_match(0, 0).unwrap(), Some((8, 8)));
        source.block = b"!!!!abab";
        assert_eq!(source.find_match(0, 4).unwrap(), None);
        source.window_size = 16;
        assert_eq!(source.find_match(0, 4).unwrap(), Some((12, 4)));
        assert_eq!(source.find_match(usize::MAX, 0).unwrap(), None);
        assert_eq!(
            source.virtual_position(usize::MAX),
            Err(Error::ResourceLimit)
        );

        history.commit(b"cdef").unwrap();
        let source = MatchSource {
            block: b"ababcdef",
            dictionary: None,
            dictionary_size: 0,
            history: Some(&history),
            history_size: 8,
            frame_position: 12,
            window_size: 8,
        };
        assert_eq!(source.find_match(0, 0).unwrap(), Some((8, 8)));
        assert_eq!(source.frame_byte(3), Err(Error::CorruptData));
    }

    #[test]
    fn dictionary_crossing_clips_only_after_frame_zero_leaves_the_retained_horizon() {
        let dictionary = Dictionary::raw(b"abcd").unwrap();
        let mut history = RingHistory::new(MAX_BLOCK_SIZE);
        history.start_frame(MAX_BLOCK_SIZE).unwrap();
        history.commit(&vec![0; MAX_BLOCK_SIZE]).unwrap();
        for (extra, expected_length) in [(0, 8), (1, 4)] {
            history.commit(&vec![0; extra]).unwrap();
            let source = MatchSource {
                block: b"abcd\0\0\0\0",
                dictionary: Some(&dictionary),
                dictionary_size: 4,
                history: Some(&history),
                history_size: history.available(),
                frame_position: history.frame_len(),
                window_size: 1 << 20,
            };
            // The immutable ring still contains this source even though its
            // distance after four current-block bytes exceeds 128 KiB.
            assert_eq!(
                source.find_match(4, 4).unwrap(),
                Some((MAX_BLOCK_SIZE + 4, 4))
            );
            assert_eq!(
                source.find_match(0, 0).unwrap(),
                Some((MAX_BLOCK_SIZE + extra + 4, expected_length))
            );
        }
    }

    #[test]
    fn old_output_is_not_searched_after_a_full_horizon_of_other_input() {
        let pattern: Vec<_> = (0..256).map(|index| index as u8).collect();
        let mut encoder = BlockEncoder::new();
        encoder.start_frame(None, 1 << 20).unwrap();
        let mut encoded = Vec::new();
        encoder.encode(&pattern, false, &mut encoded).unwrap();
        encoder
            .encode(&vec![7; MAX_BLOCK_SIZE], false, &mut encoded)
            .unwrap();
        encoded.clear();
        encoder.encode(&pattern, true, &mut encoded).unwrap();
        assert_eq!((encoded[0] >> 1) & 3, 0);
        let history = encoder.history.as_ref().unwrap();
        assert_eq!(history.window_size(), MAX_BLOCK_SIZE);
        assert_eq!(history.available(), MAX_BLOCK_SIZE);
        assert_eq!(history.byte_at(0), Err(Error::CorruptData));
    }

    #[test]
    fn history_commits_every_block_kind_and_reset_reuses_bounded_storage() {
        let mut encoder = BlockEncoder::new();
        encoder.start_frame(None, 1_u64 << 31).unwrap();
        assert!(encoder.history.is_none());
        let blocks = [
            b"abc".to_vec(),
            vec![42; 37],
            b"periodic-pattern".repeat(100),
        ];
        let mut output = Vec::new();
        let mut source = Vec::new();
        for (block, expected_kind) in blocks.iter().zip([0, 1, 2]) {
            output.clear();
            encoder.encode(block, false, &mut output).unwrap();
            assert_eq!((output[0] >> 1) & 3, expected_kind);
            source.extend_from_slice(block);
            let history = encoder.history.as_ref().unwrap();
            assert_eq!(history.window_size(), MAX_BLOCK_SIZE);
            assert_eq!(history.frame_len(), source.len() as u64);
            for (position, &byte) in source.iter().enumerate() {
                assert_eq!(history.byte_at(position as u64).unwrap(), byte);
            }
        }
        let capacity = encoder.retained_capacity();
        let history_capacity = encoder.history.as_ref().unwrap().retained_capacity();
        encoder.reset();
        assert_eq!(encoder.retained_capacity(), capacity);
        let history = encoder.history.as_ref().unwrap();
        assert_eq!(history.frame_len(), 0);
        assert_eq!(history.available(), 0);
        assert_eq!(history.byte_at(0), Err(Error::CorruptData));

        encoder.start_frame(None, 1024).unwrap();
        let block: Vec<_> = (0..1024).map(|index| index as u8).collect();
        encoder.encode(&block, false, &mut output).unwrap();
        encoder.encode(b"xyz", true, &mut output).unwrap();
        let history = encoder.history.as_ref().unwrap();
        assert_eq!(history.window_size(), 1024);
        assert_eq!(history.available(), 1024);
        assert_eq!(history.frame_len(), 1027);
        assert_eq!(history.byte_at(2), Err(Error::CorruptData));
        assert_eq!(history.byte_at(3).unwrap(), 3);
        assert_eq!(history.byte_at(1026).unwrap(), b'z');
        assert_eq!(history.retained_capacity(), history_capacity);
    }

    #[test]
    fn rejected_block_bounds_do_not_allocate_or_commit_history() {
        let mut encoder = BlockEncoder::new();
        encoder.start_frame(None, 7).unwrap();
        let mut output = vec![9];
        assert_eq!(
            encoder.encode(b"too long", true, &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(output, [9]);
        assert!(encoder.history.is_none());
        assert_eq!(encoder.frame_position, 0);
        encoder.encode(&[], true, &mut output).unwrap();
        assert!(encoder.history.is_none());
    }

    #[test]
    fn one_shot_dictionary_header_has_exact_identifier_and_content_size_fields() {
        let bytes = include_bytes!("../tests/upstream/dictionaries/formatted.dict");
        for (id, field_size) in [
            (0_u32, 0),
            (1, 1),
            (255, 1),
            (256, 2),
            (65535, 2),
            (65536, 4),
            (u32::MAX, 4),
        ] {
            let mut bytes = bytes.to_vec();
            bytes[4..8].copy_from_slice(&id.to_le_bytes());
            let dictionary = Dictionary::from_bytes(&bytes).unwrap().into_shared();
            for (length, size_field) in
                [(0, 1), (1, 1), (255, 1), (256, 2), (65_791, 2), (65_792, 4)]
            {
                let source = vec![42; length];
                let encoded = compress_with_dictionary(&source, &dictionary).unwrap();
                let (header, header_size) = crate::FrameHeader::parse(&encoded).unwrap();
                assert_eq!(header.dictionary_id, id);
                assert_eq!(header.content_size, Some(length as u64));
                assert_eq!(header.window_size, length as u64);
                assert!(header.has_checksum);
                assert_eq!(header_size, 5 + field_size + size_field);
                assert_eq!(
                    decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(length))
                        .unwrap(),
                    source
                );
            }
        }
    }

    #[test]
    fn one_shot_dictionary_content_matches_across_block_and_empty_boundaries() {
        for content in [&b""[..], &b"ab"[..], &b"a larger dictionary pattern "[..]] {
            let dictionary = Dictionary::raw(content).unwrap().into_shared();
            let source: Vec<_> = b"a larger dictionary pattern "
                .iter()
                .copied()
                .cycle()
                .take(MAX_BLOCK_SIZE * 2 + 19)
                .collect();
            let count = SharedDictionary::strong_count(&dictionary);
            let encoded = compress_with_dictionary(&source, &dictionary).unwrap();
            assert_eq!(SharedDictionary::strong_count(&dictionary), count);
            assert_eq!(
                decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                    .unwrap(),
                source
            );
            if content.is_empty() {
                assert_eq!(encoded, compress(&source).unwrap());
            }
        }
        let dictionary = Dictionary::raw(b"xy").unwrap().into_shared();
        let source = b"xy".repeat(200);
        let encoded = compress_with_dictionary(&source, &dictionary).unwrap();
        assert!(crate::decompress(&encoded, source.len()).is_err());
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
    }

    fn dictionary_blocks(blocks: &[&[u8]], dictionary: &SharedDictionary) -> (Vec<u8>, Vec<u8>) {
        let source: Vec<_> = blocks
            .iter()
            .flat_map(|block| block.iter().copied())
            .collect();
        let mut encoded = vec![0x28, 0xb5, 0x2f, 0xfd, 4, 0]; // 1 KiB window, no ID, checksum.
        let mut encoder = BlockEncoder::new();
        encoder
            .start_frame(Some(SharedDictionary::clone(dictionary)), 1024)
            .unwrap();
        for (index, block) in blocks.iter().enumerate() {
            encoder
                .encode(block, index + 1 == blocks.len(), &mut encoded)
                .unwrap();
        }
        encoded.extend_from_slice(&(xxhash::checksum(&source) as u32).to_le_bytes());
        (source, encoded)
    }

    #[test]
    fn raw_dictionary_references_can_exceed_the_advertised_output_window() {
        let mut state = 0x1234_5678_u32;
        let content: Vec<_> = (0..4096)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 24) as u8
            })
            .collect();
        let dictionary = Dictionary::raw(&content).unwrap().into_shared();
        let (source, encoded) = dictionary_blocks(&[&content[..512]], &dictionary);
        assert!(encoded.len() < source.len() / 4);
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
        assert!(crate::decompress(&encoded, source.len()).is_err());
    }

    #[test]
    fn formatted_dictionary_matches_ignore_custom_initial_repeat_offsets() {
        let bytes = include_bytes!("../tests/upstream/inline-regressions/fixed-training.dict");
        let mut changed = bytes.to_vec();
        for position in (103..115).step_by(4) {
            changed[position..position + 4].copy_from_slice(&10_u32.to_le_bytes());
        }
        let dictionary = Dictionary::from_bytes(&changed).unwrap().into_shared();
        let source = dictionary.content().repeat(16);
        let (source, encoded) = dictionary_blocks(&[&source], &dictionary);
        assert!(encoded.len() < source.len() / 4);
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
        assert!(crate::decompress(&encoded, source.len()).is_err());
    }

    #[test]
    fn short_raw_prefix_keys_cross_dictionary_end_and_expand_with_overlap() {
        for bytes in [&b"ab"[..], &b"xyz"[..]] {
            let dictionary = Dictionary::raw(bytes).unwrap().into_shared();
            let source = bytes.repeat(100);
            let (source, encoded) = dictionary_blocks(&[&source], &dictionary);
            assert_eq!(
                decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                    .unwrap(),
                source
            );
            assert!(crate::decompress(&encoded, source.len()).is_err());
        }
    }

    #[test]
    fn dictionary_candidate_lifetime_and_predefined_offset_limits_are_checked_at_start() {
        let dictionary = Dictionary::raw(b"abcd").unwrap();
        let mut source = MatchSource {
            block: b"abcdabcdabcd",
            dictionary: Some(&dictionary),
            dictionary_size: 4,
            history: None,
            history_size: 0,
            frame_position: 0,
            window_size: 1024,
        };
        assert_eq!(source.find_match(0, 0).unwrap(), Some((4, 12)));
        source.frame_position = 1024;
        assert_eq!(source.find_match(0, 0).unwrap(), Some((1028, 4)));
        source.frame_position = 1025;
        assert_eq!(source.find_match(0, 0).unwrap(), None);
        source.frame_position = MAX_DICTIONARY_DISTANCE as u64;
        source.window_size = source.frame_position;
        assert_eq!(source.find_match(0, 0).unwrap(), None);
        source.frame_position = u64::MAX;
        source.window_size = u64::MAX;
        assert_eq!(source.find_match(0, 1), Err(Error::ResourceLimit));
    }

    #[test]
    fn later_block_dictionary_matches_expire_after_window_position() {
        let content: Vec<_> = (0..256).map(|index| index as u8).collect();
        let dictionary = Dictionary::raw(&content).unwrap().into_shared();
        let mut encoder = BlockEncoder::new();
        encoder.start_frame(Some(dictionary.clone()), 1024).unwrap();
        let mut output = Vec::new();
        encoder.encode(&vec![0; 1024], false, &mut output).unwrap();
        output.clear();
        encoder.encode(&content, false, &mut output).unwrap();
        assert_eq!((output[0] >> 1) & 3, 2); // P == W can still reference the dictionary.
        output.clear();
        encoder.encode(&content, true, &mut output).unwrap();
        assert_eq!((output[0] >> 1) & 3, 2); // P > W can now use ordinary retained output.
        assert!(
            encoder
                .sequences
                .iter()
                .all(|sequence| sequence.distance <= 1024)
        );
        let (source, encoded) =
            dictionary_blocks(&[&vec![0; 1024], &content, &content], &dictionary);
        assert_eq!(
            decompress_with_dictionary(&encoded, &dictionary, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
    }

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
    fn block_workspace_reuses_capacity_and_reset_discards_previous_matches() {
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
            encoder.reset();
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
