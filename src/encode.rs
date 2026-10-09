//! A checksummed Zstandard frame encoder.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::huffman_codebook::HuffmanCodebook;
use crate::literal_encode::LiteralEncoder;
use crate::match_key::MatchKey;
use crate::match_parse::{parse_chain, parse_lazy};
use crate::match_search::SearchKeyLayout;
use crate::matchchain::MatchChain;
use crate::matchfinder::MatchTable;
use crate::ring_history::RingHistory;
use crate::sequence_context::{LogicalSequence, SequenceEncoder};
use crate::{
    CompressionStrategy, DictionaryHandle, DictionaryView, Error, Result, SharedDictionary,
    frame::MAX_BLOCK_SIZE, xxhash,
};

/// Dictionary search retains the initial parser's bounded distance policy.
/// Adaptive tables support wider offset codes, but searching more distant
/// content is a separate strategy decision; larger dictionaries remain valid.
const MAX_DICTIONARY_DISTANCE: usize = (1 << 29) - 4;
const CHAIN_NODES: usize = 262_144;
const CHAIN_SEED_NODES: usize = 65_536;

/// Compresses input into one checksummed Zstandard frame with known content size.
///
/// Corresponds to the one-shot contract of upstream `ZSTD_compress`, using this
/// implementation's own block-selection strategy rather than upstream levels.
pub fn compress(input: &[u8]) -> Result<Vec<u8>> {
    compress_in(input, Global)
}

/// Compresses one frame into the supplied allocator, including all temporary
/// match, literal, and entropy storage. The returned vector retains the original
/// allocator value; scratch owns fresh clones and is released before return.
/// A borrowed non-Clone allocator can be supplied as `&allocator`.
///
/// Frame and compression behavior matches [`compress`]. Collection requests are
/// fallible, and failure releases partial output and scratch. Ordinary allocator
/// cloning may allocate or panic outside these fallible collection requests.
pub fn compress_in<A: Allocator + Clone>(input: &[u8], allocator: A) -> Result<Vec<u8, A>> {
    compress_frame_in::<A, SharedDictionary>(input, None, allocator)
}

/// Compresses input into a checksummed frame using owned dictionary content.
///
/// A formatted dictionary's nonzero identifier is written using its smallest
/// field; a raw dictionary has no identifier. Input size excludes dictionary
/// bytes. Matches use this encoder's greedy parser. Literal and sequence tables
/// may reuse the dictionary's exact prepared entropy; repeat offsets are planned
/// forward and published only when their compressed block is emitted.
/// Corresponds to the one-shot contract of upstream `ZSTD_compress_usingCDict`.
pub fn compress_with_dictionary(input: &[u8], dictionary: &SharedDictionary) -> Result<Vec<u8>> {
    compress_with_dictionary_in(input, dictionary, Global)
}

/// Compresses one frame using an independently allocated dictionary and output.
///
/// Accepts a shared dictionary owner or a borrowed [`DictionaryView`] through the
/// sealed handle contract. The dictionary remains unchanged. The returned vector
/// owns the original allocator and compressed bytes; it borrows neither input nor
/// dictionary. Every temporary uses fresh allocator clones. A borrowed dictionary
/// view avoids the standard shared handle's Global outer allocation.
///
/// Frame behavior matches [`compress_with_dictionary`]. Failure releases partial
/// output and scratch. Allocator cloning has the caller's own allocation/panic
/// semantics, as documented for [`compress_in`].
pub fn compress_with_dictionary_in<A: Allocator + Clone, D: DictionaryHandle>(
    input: &[u8],
    dictionary: &D,
    allocator: A,
) -> Result<Vec<u8, A>> {
    compress_frame_in(input, Some(dictionary), allocator)
}

fn compress_frame_in<A: Allocator + Clone, D: DictionaryHandle>(
    input: &[u8],
    dictionary: Option<&D>,
    allocator: A,
) -> Result<Vec<u8, A>> {
    let blocks = input.len().div_ceil(MAX_BLOCK_SIZE).max(1);
    let dictionary_id = dictionary.map_or(0, |dictionary| dictionary.view().id());
    let (_, dictionary_bytes) = dictionary_field(dictionary_id);
    let capacity = blocks
        .checked_mul(3)
        .and_then(|overhead| overhead.checked_add(17))
        .and_then(|overhead| overhead.checked_add(dictionary_bytes))
        .and_then(|overhead| overhead.checked_add(input.len()))
        .ok_or(Error::ResourceLimit)?;
    let mut output = Vec::new_in(allocator);
    output
        .try_reserve_exact(capacity)
        .map_err(|_| Error::AllocationFailed)?;
    write_frame_header(&mut output, input.len(), dictionary_id);
    let mut encoder = BlockEncoder::new_in(output.allocator().clone());
    encoder.start_frame(dictionary.cloned(), input.len() as u64)?;
    if input.is_empty() {
        encoder.encode(&[], true, &mut output)?;
    }
    for (index, block) in input.chunks(MAX_BLOCK_SIZE).enumerate() {
        encoder.encode(block, index + 1 == blocks, &mut output)?;
    }
    // The checked raw-block upper bound reserves four checksum bytes in addition
    // to the largest header and every block. Selected blocks never exceed raw.
    for byte in (xxhash::checksum(input) as u32).to_le_bytes() {
        output.push(byte);
    }
    Ok(output)
}

/// Reusable frame match history, literal trees, and sequence-table workspace.
///
/// Corresponds to the independently owned block-compression workspace of
/// `ZSTD_CCtx`; input references never survive an `encode` call.
pub(crate) struct BlockEncoder<A: Allocator = Global, D: DictionaryHandle = SharedDictionary> {
    strategy: CompressionStrategy,
    match_table: Option<MatchTable<A>>,
    dictionary_seed: Option<MatchTable<A>>,
    chain: Option<MatchChain<A>>,
    chain_seed: Option<MatchChain<A>>,
    active_dictionary: Option<D>,
    history: Option<RingHistory<A>>,
    frame_position: u64,
    window_size: u64,
    literals: Vec<u8, A>,
    literal_encoder: LiteralEncoder<A>,
    literal_table: Option<HuffmanCodebook>,
    pending_literal_table: Option<HuffmanCodebook>,
    sequences: Vec<LogicalSequence, A>,
    sequence_encoder: SequenceEncoder<A>,
    payload: Vec<u8, A>,
    allocator: A,
}

impl BlockEncoder<Global, SharedDictionary> {
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "const Global convenience constructor")
    )]
    pub(crate) const fn new() -> Self {
        Self {
            strategy: CompressionStrategy::Greedy,
            match_table: None,
            dictionary_seed: None,
            chain: None,
            chain_seed: None,
            active_dictionary: None,
            history: None,
            frame_position: 0,
            window_size: MAX_BLOCK_SIZE as u64,
            literals: Vec::new(),
            literal_encoder: LiteralEncoder::new(),
            literal_table: None,
            pending_literal_table: None,
            sequences: Vec::new(),
            sequence_encoder: SequenceEncoder::new(),
            payload: Vec::new(),
            allocator: Global,
        }
    }
}

impl<A: Allocator + Clone, D: DictionaryHandle> BlockEncoder<A, D> {
    /// Give each fresh workspace owner its own allocator value. Clones need
    /// not be equivalent; complete owners move with their actual allocator.
    /// Ordinary Clone may allocate or panic independently of fallible reserves.
    pub(crate) fn new_in(allocator: A) -> Self {
        Self {
            strategy: CompressionStrategy::Greedy,
            match_table: None,
            dictionary_seed: None,
            chain: None,
            chain_seed: None,
            active_dictionary: None,
            history: None,
            frame_position: 0,
            window_size: MAX_BLOCK_SIZE as u64,
            literals: Vec::new_in(allocator.clone()),
            literal_encoder: LiteralEncoder::new_in(allocator.clone()),
            literal_table: None,
            pending_literal_table: None,
            sequences: Vec::new_in(allocator.clone()),
            sequence_encoder: SequenceEncoder::new_in(allocator.clone()),
            payload: Vec::new_in(allocator.clone()),
            allocator,
        }
    }

    /// Prepare a dictionary attachment and independently owned indexed seeds.
    ///
    /// Corresponds to the dictionary hash preparation in `ZSTD_compressBegin`.
    /// The attachment preserves shared ownership or the caller's source borrow;
    /// only indices enter the hash table. Reusing this workspace for a
    /// different dictionary always rebuilds its seed indices before publication.
    /// A seed can change before a later setup error. After any error the caller
    /// must reset and successfully start a frame before encoding again; setup
    /// does not promise rollback of the previous frame's scratch state.
    pub(crate) fn start_frame(&mut self, dictionary: Option<D>, window_size: u64) -> Result<()> {
        let dictionary_view = dictionary.as_ref().map(DictionaryHandle::view);
        let entropy = dictionary_view.and_then(|view| view.entropy());
        let literal_table = entropy
            .map(|entropy| {
                // Prepared dictionaries allow twelve-bit codes. The codebook
                // copies plain values, retaining no source-table borrow.
                HuffmanCodebook::from_lengths(&entropy.huffman.code_lengths(), 12)
            })
            .transpose()?;
        if let Some(dictionary) = dictionary_view
            && dictionary.content().len() >= 4
        {
            let content = dictionary.content();
            match self.strategy {
                CompressionStrategy::Greedy => {
                    let seed = match self.dictionary_seed.as_mut() {
                        Some(seed) => seed,
                        None => self
                            .dictionary_seed
                            .insert(MatchTable::new_in(self.allocator.clone())?),
                    };
                    seed.reset();
                    let start = content.len().saturating_sub(MAX_DICTIONARY_DISTANCE);
                    for position in start..=content.len() - 4 {
                        seed.replace(word_at(content, position), position)?;
                    }
                }
                CompressionStrategy::Chain | CompressionStrategy::Lazy => {
                    let seed = match self.chain_seed.as_mut() {
                        Some(seed) => seed,
                        None => self.chain_seed.insert(MatchChain::new_in(
                            CHAIN_SEED_NODES,
                            self.allocator.clone(),
                        )?),
                    };
                    seed.reset();
                    let start = content.len().saturating_sub(CHAIN_SEED_NODES + 3);
                    for position in start..=content.len() - 4 {
                        seed.insert(word_at(content, position), position as u64)?;
                    }
                }
            }
        } else {
            if let Some(seed) = &mut self.dictionary_seed {
                seed.reset();
            }
            if let Some(seed) = &mut self.chain_seed {
                seed.reset();
            }
        }
        self.sequence_encoder.start_frame_with_entropy(entropy)?;
        self.literal_table = literal_table;
        self.pending_literal_table = None;
        self.active_dictionary = dictionary;
        if let Some(history) = &mut self.history {
            history.clear();
        }
        self.frame_position = 0;
        self.window_size = window_size;
        Ok(())
    }

    /// Append exactly one bounded raw, RLE, or compressed block.
    /// A failure can change match/entropy scratch but never accepts this block.
    /// The caller must reset before reuse; committed entropy and prior output
    /// remain intact, and history advances only after successful emission.
    pub(crate) fn encode<O: Allocator>(
        &mut self,
        block: &[u8],
        last: bool,
        output: &mut Vec<u8, O>,
    ) -> Result<()> {
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
                .get_or_insert_with(|| RingHistory::new_in(MAX_BLOCK_SIZE, self.allocator.clone()));
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

    fn encode_block<O: Allocator>(
        &mut self,
        block: &[u8],
        last: bool,
        output: &mut Vec<u8, O>,
    ) -> Result<()> {
        self.pending_literal_table = None;
        self.sequence_encoder.discard();
        output
            .try_reserve(block.len() + 3)
            .map_err(|_| Error::AllocationFailed)?;
        if block.len() > 1 && block.iter().all(|&byte| byte == block[0]) {
            write_block_header(output, block.len(), 1, last);
            output.push(block[0]);
            return Ok(());
        }
        if block.len() >= 6 && self.compress_block(block)? {
            write_block_header(output, self.payload.len(), 2, last);
            for &byte in &self.payload {
                output.push(byte);
            }
            // Output capacity was reserved before candidate construction. This
            // infallible transfer publishes only the tree described by emitted
            // bytes; raw/RLE fallback leaves the previous tree unchanged.
            if let Some(table) = self.pending_literal_table.take() {
                self.literal_table = Some(table);
            }
            self.sequence_encoder.commit();
            return Ok(());
        }
        self.pending_literal_table = None;
        self.sequence_encoder.discard();
        write_block_header(output, block.len(), 0, last);
        for &byte in block {
            output.push(byte);
        }
        Ok(())
    }
}

impl<A: Allocator, D: DictionaryHandle> BlockEncoder<A, D> {
    /// Select the next frame's parser before calling start_frame.
    pub(crate) fn set_strategy(&mut self, strategy: CompressionStrategy) {
        self.strategy = strategy;
    }

    /// Drop active-frame ownership while retaining reusable workspace storage.
    pub(crate) fn reset(&mut self) {
        self.active_dictionary = None;
        self.literal_table = None;
        self.pending_literal_table = None;
        self.sequence_encoder.reset();
        if let Some(history) = &mut self.history {
            history.clear();
        }
        self.frame_position = 0;
        self.window_size = MAX_BLOCK_SIZE as u64;
    }

    /// Invalidate cached dictionary indices after resetting the active frame.
    ///
    /// Seeds own only initialized indices/tags, never dictionary references.
    /// Parameter resets clear those values while preserving both storage
    /// families and their exact allocator owners for later frame preparation.
    pub(crate) fn clear_dictionary_seeds(&mut self) {
        if let Some(seed) = &mut self.dictionary_seed {
            seed.reset();
        }
        if let Some(seed) = &mut self.chain_seed {
            seed.reset();
        }
    }

    /// Codec payload capacity owned by the workspace, excluding inline values,
    /// opaque allocator bookkeeping, and dictionary/control-block storage whose
    /// ownership or borrow is accounted for separately by the enclosing context.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.match_table
            .as_ref()
            .map_or(0, MatchTable::retained_capacity)
            + self
                .dictionary_seed
                .as_ref()
                .map_or(0, MatchTable::retained_capacity)
            + self.chain.as_ref().map_or(0, MatchChain::retained_capacity)
            + self
                .chain_seed
                .as_ref()
                .map_or(0, MatchChain::retained_capacity)
            + self.literals.capacity()
            + self.literal_encoder.retained_capacity()
            + self.sequences.capacity() * core::mem::size_of::<LogicalSequence>()
            + self.sequence_encoder.retained_capacity()
            + self.payload.capacity()
            + self
                .history
                .as_ref()
                .map_or(0, RingHistory::retained_capacity)
    }
}

/// Writes a single-segment frame header; corresponds to `ZSTD_writeFrameHeader`.
/// The caller reserves the complete header before this infallible append.
fn write_frame_header<A: Allocator>(
    output: &mut Vec<u8, A>,
    content_size: usize,
    dictionary_id: u32,
) {
    for byte in crate::frame::MAGIC.to_le_bytes() {
        output.push(byte);
    }
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
    for &byte in &dictionary_id.to_le_bytes()[..dictionary_bytes] {
        output.push(byte);
    }
    for &byte in &value.to_le_bytes()[..count] {
        output.push(byte);
    }
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
fn write_block_header<O: Allocator>(output: &mut Vec<u8, O>, size: usize, kind: u8, last: bool) {
    let header = ((size as u32) << 3) | (u32::from(kind) << 1) | u32::from(last);
    for byte in &header.to_le_bytes()[..3] {
        output.push(*byte);
    }
}

/// Selects matches greedily, chooses literal sections, and encodes sequences.
///
/// Fulfills the block-compression role of `ZSTD_compressBlock_internal`, using
/// an independent greedy parser and table inversion instead of C table layout.
impl<A: Allocator + Clone, D: DictionaryHandle> BlockEncoder<A, D> {
    fn compress_block(&mut self, block: &[u8]) -> Result<bool> {
        let dictionary = self
            .active_dictionary
            .as_ref()
            .map(DictionaryHandle::view)
            .filter(|dictionary| {
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
        match self.strategy {
            CompressionStrategy::Greedy => {
                let table = match self.match_table.as_mut() {
                    Some(table) => table,
                    None => self
                        .match_table
                        .insert(MatchTable::new_in(self.allocator.clone())?),
                };
                parse_greedy(
                    &source,
                    table,
                    self.dictionary_seed.as_ref(),
                    &mut self.literals,
                    &mut self.sequences,
                )?;
            }
            CompressionStrategy::Chain | CompressionStrategy::Lazy => {
                let table = match self.chain.as_mut() {
                    Some(table) => table,
                    None => self
                        .chain
                        .insert(MatchChain::new_in(CHAIN_NODES, self.allocator.clone())?),
                };
                parse_seeded_chain(
                    &source,
                    table,
                    self.chain_seed.as_ref(),
                    self.strategy,
                    &mut self.literals,
                    &mut self.sequences,
                )?;
            }
        }
        self.pending_literal_table = self
            .literal_encoder
            .select(&self.literals, self.literal_table.as_ref())?;
        self.sequence_encoder.prepare(&self.sequences)?;
        let stream = self.sequence_encoder.as_bytes();
        let section = self.literal_encoder.as_bytes();
        let capacity = section
            .len()
            .checked_add(stream.len())
            .ok_or(Error::ResourceLimit)?;
        let payload = &mut self.payload;
        payload.clear();
        payload
            .try_reserve_exact(capacity)
            .map_err(|_| Error::AllocationFailed)?;
        for &byte in section {
            payload.push(byte);
        }
        for &byte in stream {
            payload.push(byte);
        }
        Ok(payload.len() < block.len())
    }
}

/// Keep the original single-candidate policy independent of chain storage.
fn parse_greedy<H: Allocator, M: Allocator, B: Allocator, L: Allocator, S: Allocator>(
    source: &MatchSource<'_, H>,
    match_table: &mut MatchTable<M>,
    seed: Option<&MatchTable<B>>,
    literals: &mut Vec<u8, L>,
    sequences: &mut Vec<LogicalSequence, S>,
) -> Result<()> {
    let block = source.block;
    if source.dictionary.is_some()
        && let Some(seed) = seed
    {
        match_table.reset_from(seed)?;
    } else {
        match_table.reset();
    }
    if source.dictionary.is_some() && source.retains_frame_start() {
        // Keys may straddle dictionary end and retained frame[0]. This also
        // makes one-, two-, and three-byte raw prefixes useful.
        for position in source.dictionary_size.saturating_sub(3)..source.dictionary_size {
            let word = source.dictionary_boundary_word(position)?;
            match_table.replace(word, position)?;
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
        )?;
    }
    literals.clear();
    literals
        .try_reserve_exact(block.len())
        .map_err(|_| Error::AllocationFailed)?;
    sequences.clear();
    sequences
        .try_reserve_exact(block.len() / 4)
        .map_err(|_| Error::AllocationFailed)?;
    let mut position = 0;
    let mut literal_start = 0;
    while position + 4 <= block.len() {
        let virtual_position = source.virtual_position(position)?;
        let candidate = match_table.replace(word_at(block, position), virtual_position)?;
        if let Some(previous) = candidate
            && let Some((distance, match_length)) = source.find_match(previous, position)?
        {
            for &byte in &block[literal_start..position] {
                literals.push(byte);
            }
            sequences.push(LogicalSequence {
                literal_length: (position - literal_start) as u32,
                match_length: match_length as u32,
                distance: distance as u32,
            });
            let match_end = position + match_length;
            position += 1;
            while position < match_end && position + 4 <= block.len() {
                match_table
                    .replace(word_at(block, position), source.virtual_position(position)?)?;
                position += 1;
            }
            position = match_end;
            literal_start = position;
        } else {
            position += 1;
        }
    }
    for &byte in &block[literal_start..] {
        literals.push(byte);
    }
    Ok(())
}

/// Seed only live dictionary/history coordinates; every match is verified by
/// the same source view as Greedy, independently of candidate-storage policy.
fn parse_seeded_chain<H: Allocator, M: Allocator, B: Allocator, L: Allocator, S: Allocator>(
    source: &MatchSource<'_, H>,
    chain: &mut MatchChain<M>,
    seed: Option<&MatchChain<B>>,
    strategy: CompressionStrategy,
    literals: &mut Vec<u8, L>,
    sequences: &mut Vec<LogicalSequence, S>,
) -> Result<()> {
    if source.dictionary.is_some()
        && let Some(seed) = seed
    {
        chain.reset_from(seed)?;
    } else {
        chain.reset();
    }
    if source.dictionary.is_some() && source.retains_frame_start() {
        for position in source.dictionary_size.saturating_sub(3)..source.dictionary_size {
            chain.insert(source.dictionary_boundary_word(position)?, position as u64)?;
        }
    }
    for position in 0..source.history_size {
        let virtual_position = source
            .dictionary_size
            .checked_add(position)
            .ok_or(Error::ResourceLimit)?;
        chain.insert(source.output_word(position)?, virtual_position as u64)?;
    }
    let resolve = |previous, position| {
        let previous = usize::try_from(previous).map_err(|_| Error::ResourceLimit)?;
        source
            .find_match(previous, position)?
            .map(|(distance, length)| {
                Ok((
                    u32::try_from(distance).map_err(|_| Error::ResourceLimit)?,
                    u32::try_from(length).map_err(|_| Error::ResourceLimit)?,
                ))
            })
            .transpose()
    };
    let base = source.virtual_position(0)? as u64;
    match strategy {
        CompressionStrategy::Chain => {
            parse_chain(source.block, chain, base, 8, resolve, literals, sequences)
        }
        CompressionStrategy::Lazy => {
            parse_lazy(source.block, chain, base, resolve, literals, sequences)
        }
        CompressionStrategy::Greedy => Err(Error::InvalidState),
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
struct MatchSource<'a, A: Allocator = Global> {
    block: &'a [u8],
    dictionary: Option<DictionaryView<'a>>,
    dictionary_size: usize,
    history: Option<&'a RingHistory<A>>,
    history_size: usize,
    frame_position: u64,
    window_size: u64,
}

impl<A: Allocator> MatchSource<'_, A> {
    fn virtual_position(&self, position: usize) -> Result<usize> {
        self.dictionary_size
            .checked_add(self.history_size)
            .and_then(|start| start.checked_add(position))
            .ok_or(Error::ResourceLimit)
    }

    fn retains_frame_start(&self) -> bool {
        self.frame_position == self.history_size as u64
    }

    /// Checked indexed extents before assembling a variable-width key.
    ///
    /// Dictionary/history/input indices remain usize; the absolute frame end
    /// stays u64. A very long frame does not need to fit usize when its borrowed
    /// dictionary, retained history and current input still have a valid local
    /// indexed extent. These checks do not change either borrowed owner.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "variable-width parser integration follows this source checkpoint"
        )
    )]
    fn key_extents(&self) -> Result<(usize, usize)> {
        let output_size = self
            .history_size
            .checked_add(self.block.len())
            .ok_or(Error::ResourceLimit)?;
        let virtual_size = self
            .dictionary_size
            .checked_add(output_size)
            .ok_or(Error::ResourceLimit)?;
        let history_size = u64::try_from(self.history_size).map_err(|_| Error::ResourceLimit)?;
        self.frame_position
            .checked_sub(history_size)
            .ok_or(Error::CorruptData)?;
        let input_size = u64::try_from(self.block.len()).map_err(|_| Error::ResourceLimit)?;
        self.frame_position
            .checked_add(input_size)
            .ok_or(Error::ResourceLimit)?;
        Ok((output_size, virtual_size))
    }

    /// Copy exactly the selected bytes across dictionary and retained frame zero.
    ///
    /// Starts in 0..=D are valid; D denotes the frame-zero boundary, including
    /// an empty dictionary with no dictionary owner. A start beyond D is
    /// CorruptData. Frame zero must still be retained, including at that endpoint,
    /// or this boundary operation returns InvalidState. Dictionary-only matching
    /// has separate eligibility and does not depend on this helper.
    /// A valid tail shorter than the selected width returns None. Arithmetic
    /// overflow returns ResourceLimit before copying any bytes. Missing source
    /// storage is reported by the same checked dictionary/history byte readers.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "variable-width parser integration follows this source checkpoint"
        )
    )]
    fn dictionary_boundary_key(
        &self,
        position: usize,
        layout: SearchKeyLayout,
    ) -> Result<Option<MatchKey>> {
        let (_, virtual_size) = self.key_extents()?;
        if position > self.dictionary_size {
            return Err(Error::CorruptData);
        }
        if !self.retains_frame_start() {
            return Err(Error::InvalidState);
        }
        let width = usize::from(layout.key_bytes());
        if virtual_size - position < width {
            return Ok(None);
        }
        let mut bytes = [0; 8];
        for (offset, byte) in bytes[..width].iter_mut().enumerate() {
            let index = position.checked_add(offset).ok_or(Error::ResourceLimit)?;
            *byte = if index < self.dictionary_size {
                self.dictionary
                    .ok_or(Error::InvalidState)?
                    .content_byte(index)?
            } else {
                self.output_byte(index - self.dictionary_size)?
            };
        }
        // Only the selected initialized prefix is read. Zeroed scratch bytes
        // beyond width are never used to pad an absent logical source byte.
        Ok(layout.read_key(&bytes[..width]))
    }

    /// Copy a key starting in retained-output/current-input coordinates.
    ///
    /// Positions in 0..=H+B are valid; the exact endpoint is an empty tail.
    /// A valid tail shorter than width returns None, while a position beyond
    /// the indexed region is CorruptData. Extent or absolute-frame overflow is
    /// ResourceLimit. A malformed retained-history coordinate remains an error
    /// from the checked output reader rather than being replaced with padding.
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "variable-width parser integration follows this source checkpoint"
        )
    )]
    fn output_key(&self, position: usize, layout: SearchKeyLayout) -> Result<Option<MatchKey>> {
        let (output_size, _) = self.key_extents()?;
        if position > output_size {
            return Err(Error::CorruptData);
        }
        let width = usize::from(layout.key_bytes());
        if output_size - position < width {
            return Ok(None);
        }
        let mut bytes = [0; 8];
        for (offset, byte) in bytes[..width].iter_mut().enumerate() {
            *byte = self.output_byte(position.checked_add(offset).ok_or(Error::ResourceLimit)?)?;
        }
        Ok(layout.read_key(&bytes[..width]))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{LL_DEFAULT, ML_DEFAULT, OF_DEFAULT};
    use crate::fse::FseTable;
    use crate::fse_encode::FseEncodingTable as EncodingTable;
    use crate::{DecodeOptions, Dictionary, decompress_with_dictionary};
    use alloc::vec;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn block_allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 128,
            fail_at: None,
        })
        .unwrap()
    }

    #[test]
    fn parameter_reset_clears_both_dictionary_seed_families_without_allocating() {
        let owner = block_allocator();
        let monitor = owner.monitor();
        let dictionary = Dictionary::raw(b"abcdabcdabcdabcd").unwrap().into_shared();
        let mut encoder = BlockEncoder::<_, SharedDictionary>::new_in(owner);
        encoder.start_frame(Some(dictionary.clone()), 1024).unwrap();
        assert!(
            encoder
                .dictionary_seed
                .as_mut()
                .unwrap()
                .replace(*b"abcd", 128)
                .unwrap()
                .is_some()
        );
        encoder.set_strategy(CompressionStrategy::Chain);
        encoder.start_frame(Some(dictionary.clone()), 1024).unwrap();
        assert!(
            encoder
                .chain_seed
                .as_ref()
                .unwrap()
                .candidates(*b"abcd", u64::MAX, 8)
                .unwrap()
                .next()
                .is_some()
        );
        let before = monitor.snapshot();
        let capacity = encoder.retained_capacity();
        assert!(monitor.fail_next());
        encoder.reset();
        encoder.clear_dictionary_seeds();
        assert_eq!(monitor.snapshot(), before);
        assert_eq!(encoder.retained_capacity(), capacity);
        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
        assert_eq!(
            encoder
                .dictionary_seed
                .as_mut()
                .unwrap()
                .replace(*b"abcd", 129)
                .unwrap(),
            None
        );
        assert_eq!(
            encoder
                .chain_seed
                .as_ref()
                .unwrap()
                .candidates(*b"abcd", u64::MAX, 8)
                .unwrap()
                .next(),
            None
        );
        let mut witness = Vec::<u8, _>::new_in(&encoder.allocator);
        assert!(witness.try_reserve_exact(1).is_err());
        drop(witness);
        assert_eq!(monitor.snapshot().attempts, before.attempts + 1);
        drop(encoder);
        monitor.assert_released();
        assert_eq!(
            monitor.snapshot().identities_created,
            monitor.snapshot().owners_dropped
        );
    }

    // The independent pre-migration helper intentionally repeats only the first
    // 48 bytes, not the entire byte string. Its 36 length/dictionary cases were
    // captured before this allocator migration (helper SHA-256
    // dac089eff3f99cb2074afca1fa670b27484eaa15522e27175820640b4c4a1f5d).
    fn one_shot_baseline_source(length: usize) -> Vec<u8> {
        (0..length)
            .map(|index| b"abcaBc0123456789 -- repeated dictionary content --"[index % 48])
            .collect()
    }

    fn one_shot_baseline_frames(mut inspect: impl FnMut(&[u8], &[u8], Option<&SharedDictionary>)) {
        let raw =
            Dictionary::raw(b"abcaBc0123456789 -- repeated dictionary content -- abcaBc0123456789")
                .unwrap()
                .into_shared();
        let prepared = allocator_dictionary(true).into_shared();
        let mut digest = 0xcbf2_9ce4_8422_2325_u64;
        let mut cases = 0;
        for length in [
            0, 1, 7, 255, 256, 257, 65_791, 65_792, 65_793, 131_071, 131_072, 131_073,
        ] {
            let source = one_shot_baseline_source(length);
            for dictionary in [None, Some(&raw), Some(&prepared)] {
                let owner = block_allocator();
                let identity = owner.identity();
                let monitor = owner.monitor();
                let frame = match dictionary {
                    None => compress_in(&source, owner).unwrap(),
                    Some(dictionary) => {
                        compress_with_dictionary_in(&source, dictionary, owner).unwrap()
                    }
                };
                assert_eq!(frame.allocator().identity(), identity);
                let snapshot = monitor.snapshot();
                assert_eq!(snapshot.live_blocks, 1);
                assert_eq!(snapshot.live_bytes, frame.capacity() as u128);
                assert_eq!(snapshot.owners_dropped + 1, snapshot.identities_created);
                for byte in (frame.len() as u64)
                    .to_le_bytes()
                    .iter()
                    .chain(frame.iter())
                {
                    digest = (digest ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
                }
                inspect(&source, &frame, dictionary);
                drop(frame);
                monitor.assert_released();
                cases += 1;
            }
        }
        assert_eq!(cases, 36);
        assert_eq!(digest, 5_968_133_839_619_914_848);
    }

    #[test]
    fn allocator_one_shot_preserves_36_pre_migration_frames_and_original_output_owner() {
        one_shot_baseline_frames(|source, frame, dictionary| {
            let (header, _) = crate::FrameHeader::parse(frame).unwrap();
            assert_eq!(header.content_size, Some(source.len() as u64));
            assert_eq!(header.dictionary_id, dictionary.map_or(0, |dict| dict.id()));
            assert!(header.has_checksum);
            let decoded = match dictionary {
                None => crate::decompress(frame, source.len()).unwrap(),
                Some(dictionary) => {
                    decompress_with_dictionary(frame, dictionary, DecodeOptions::new(source.len()))
                        .unwrap()
                }
            };
            assert_eq!(decoded, source);
        });
    }

    #[test]
    fn allocator_one_shot_owned_output_outlives_input_and_independent_dictionary() {
        for prepared in [false, true] {
            for shared in [false, true] {
                let serialized = allocator_dictionary(prepared).as_bytes().to_vec();
                let dictionary_owner = block_allocator();
                let dictionary_monitor = dictionary_owner.monitor();
                let output_owner = block_allocator();
                let output_identity = output_owner.identity();
                let output_monitor = output_owner.monitor();
                let (frame, expected, pointer) = {
                    let source =
                        b"a useful raw dictionary pattern abcdefghijklmnopqrstuvwxyz".repeat(4);
                    let dictionary =
                        Dictionary::from_bytes_in(&serialized, dictionary_owner).unwrap();
                    let before = dictionary_monitor.snapshot();
                    let expected =
                        compress_with_dictionary_in(&source, &dictionary.view(), Global).unwrap();
                    let frame = if shared {
                        let dictionary = dictionary.into_shared();
                        let frame = compress_with_dictionary_in(&source, &dictionary, output_owner)
                            .unwrap();
                        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
                        frame
                    } else {
                        compress_with_dictionary_in(&source, &dictionary.view(), output_owner)
                            .unwrap()
                    };
                    // Compression never allocates through the dictionary's owner.
                    assert_eq!(dictionary_monitor.snapshot().attempts, before.attempts);
                    assert_eq!(frame, expected);
                    let pointer = frame.as_ptr();
                    (frame, expected, pointer)
                };
                dictionary_monitor.assert_released();
                assert_eq!(frame.as_ptr(), pointer);
                assert_eq!(frame, expected);
                assert_eq!(frame.allocator().identity(), output_identity);
                let remaining = output_monitor.snapshot();
                assert_eq!(remaining.live_blocks, 1);
                assert_eq!(remaining.live_bytes, frame.capacity() as u128);
                assert_eq!(remaining.owners_dropped + 1, remaining.identities_created);
                drop(frame);
                output_monitor.assert_released();
            }
        }
    }

    #[test]
    fn allocator_one_shot_borrows_nonclone_output_and_dictionary_allocators_independently() {
        let dictionary_owner = block_allocator().into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let output_owner = block_allocator().into_non_clone();
        let output_monitor = output_owner.monitor();
        let serialized = allocator_dictionary(true).as_bytes().to_vec();
        let frame = {
            let dictionary = Dictionary::from_bytes_in(&serialized, &dictionary_owner).unwrap();
            let source = b"nonclone allocation owners and unchanged dictionary input".repeat(2);
            let before = dictionary_monitor.snapshot();
            let expected =
                compress_with_dictionary_in(&source, &dictionary.view(), Global).unwrap();
            let frame =
                compress_with_dictionary_in(&source, &dictionary.view(), &output_owner).unwrap();
            assert_eq!(frame, expected);
            assert_eq!(dictionary_monitor.snapshot(), before);
            assert!(core::ptr::eq(*frame.allocator(), &output_owner));
            frame
        };
        dictionary_monitor.assert_released();
        assert_eq!(output_monitor.snapshot().live_blocks, 1);
        assert_eq!(
            output_monitor.snapshot().live_bytes,
            frame.capacity() as u128
        );
        assert_eq!(output_monitor.snapshot().identities_created, 1);
        drop(frame);
        output_monitor.assert_released();
    }

    #[test]
    fn allocator_one_shot_shared_raw_dictionary_owns_a_nonclone_allocator() {
        let dictionary_owner = block_allocator().into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let content = b"independent dictionary with an owned nonclone allocator";
        let dictionary = Dictionary::raw_in(content, dictionary_owner)
            .unwrap()
            .into_shared();
        let before = dictionary_monitor.snapshot();
        let output_owner = block_allocator();
        let output_monitor = output_owner.monitor();
        let source = content.repeat(4);
        let frame = compress_with_dictionary_in(&source, &dictionary, output_owner).unwrap();
        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
        assert_eq!(dictionary_monitor.snapshot(), before);
        drop(dictionary);
        dictionary_monitor.assert_released();
        assert_eq!(output_monitor.snapshot().live_blocks, 1);
        assert_eq!(
            output_monitor.snapshot().live_bytes,
            frame.capacity() as u128
        );
        let reference = Dictionary::raw(content).unwrap().into_shared();
        assert_eq!(
            decompress_with_dictionary(&frame, &reference, DecodeOptions::new(source.len()))
                .unwrap(),
            source
        );
        drop(frame);
        output_monitor.assert_released();
    }

    #[test]
    fn allocator_one_shot_every_allocation_failure_releases_partial_output_and_scratch() {
        for source in [
            &b""[..],
            &b"a useful raw dictionary pattern abcdefghijklmnopqrstuvwxyz".repeat(2),
        ] {
            for dictionary_kind in 0..3 {
                let dictionary_owner = block_allocator();
                let dictionary_monitor = dictionary_owner.monitor();
                let bytes = allocator_dictionary(dictionary_kind == 2)
                    .as_bytes()
                    .to_vec();
                let dictionary = Dictionary::from_bytes_in(&bytes, dictionary_owner).unwrap();
                let view = (dictionary_kind != 0).then(|| dictionary.view());
                let source_before = source.to_vec();
                let dictionary_before = dictionary_monitor.snapshot();
                let owner = block_allocator();
                let monitor = owner.monitor();
                let expected = compress_frame_in(source, view.as_ref(), owner).unwrap();
                let attempts = monitor.snapshot().attempts;
                #[cfg(feature = "std")]
                std::println!(
                    "one-shot failure sweep: dictionary kind {dictionary_kind}, source bytes {}, allocation ordinals 1..={attempts}",
                    source.len()
                );
                assert!(attempts > 0);
                assert_eq!(monitor.snapshot().live_blocks, 1);
                assert_eq!(monitor.snapshot().live_bytes, expected.capacity() as u128);
                drop(expected);
                monitor.assert_released();
                for ordinal in 1..=attempts {
                    let owner = block_allocator();
                    let monitor = owner.monitor();
                    monitor.set_fail_at(NonZeroU64::new(ordinal));
                    let result = compress_frame_in(source, view.as_ref(), owner);
                    assert_eq!(
                        result.err(),
                        Some(Error::AllocationFailed),
                        "dictionary kind {dictionary_kind}, source length {}, ordinal {ordinal}",
                        source.len()
                    );
                    let snapshot = monitor.snapshot();
                    assert_eq!(snapshot.attempts, ordinal);
                    assert_eq!(snapshot.failures, 1);
                    assert_eq!(snapshot.owners_dropped, snapshot.identities_created);
                    monitor.assert_released();
                    assert_eq!(dictionary_monitor.snapshot(), dictionary_before);
                    assert_eq!(dictionary.as_bytes(), bytes);
                    assert_eq!(source, source_before);
                }
                drop(dictionary);
                dictionary_monitor.assert_released();
            }
        }
    }

    #[test]
    fn allocator_one_shot_headers_use_all_id_widths_without_allocating_after_reserve() {
        let mut sizes = vec![0, 255, 256, 65_791, 65_792, u32::MAX as usize];
        if usize::BITS > 32 {
            sizes.push((u64::from(u32::MAX) + 1) as usize);
        }
        for size in sizes {
            for id in [0, 1, 255, 256, 65_535, 65_536, u32::MAX] {
                let owner = block_allocator().into_non_clone();
                let monitor = owner.monitor();
                let mut bytes = Vec::new_in(owner);
                bytes.try_reserve_exact(17).unwrap();
                let before = monitor.snapshot();
                assert!(monitor.fail_next());
                write_frame_header(&mut bytes, size, id);
                assert_eq!(monitor.snapshot(), before);
                let (header, consumed) = crate::FrameHeader::parse(&bytes).unwrap();
                assert_eq!(consumed, bytes.len());
                assert_eq!(header.content_size, Some(size as u64));
                assert_eq!(header.dictionary_id, id);
                assert_eq!((bytes[4] & 3, dictionary_field(id).1), dictionary_field(id));
                drop(bytes);
                monitor.assert_released();
            }
        }
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN"]
    fn official_codec_decodes_allocator_one_shot_frames() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let executable = std::env::var_os("ZSTD_BIN").expect("set ZSTD_BIN");
        let version = Command::new(&executable).arg("--version").output().unwrap();
        assert!(version.status.success());
        assert!(std::string::String::from_utf8_lossy(&version.stdout).contains("1.5.7"));
        struct File(std::path::PathBuf);
        impl Drop for File {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let path = File(std::env::temp_dir().join(std::format!(
            "zstdoxide-allocator-one-shot-{}-{}-{}.dict",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )));
        let mut cases = 0;
        one_shot_baseline_frames(|source, frame, dictionary| {
            let mut command = Command::new(&executable);
            command.args(["-q", "-d", "-c"]);
            if let Some(dictionary) = dictionary {
                std::fs::write(&path.0, dictionary.as_bytes()).unwrap();
                command.arg("-D").arg(&path.0);
            }
            let mut child = command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(frame).unwrap();
            let decoded = child.wait_with_output().unwrap();
            assert!(
                decoded.status.success(),
                "case {cases}: {}",
                std::string::String::from_utf8_lossy(&decoded.stderr)
            );
            assert_eq!(decoded.stdout, source);
            cases += 1;
        });
        assert_eq!(cases, 36);
        std::println!("official C decoded {cases} allocator-owned one-shot frames exactly");
    }

    fn allocator_dictionary(prepared: bool) -> Dictionary {
        if prepared {
            Dictionary::from_bytes(include_bytes!(
                "../tests/upstream/dictionaries/formatted.dict"
            ))
            .unwrap()
        } else {
            Dictionary::raw(b"a useful raw dictionary pattern abcdefghijklmnopqrstuvwxyz").unwrap()
        }
    }

    fn allocator_block_case(
        strategy: CompressionStrategy,
        prepared: bool,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let serialized = allocator_dictionary(prepared).as_bytes().to_vec();
        let dictionary_owner = block_allocator();
        let dictionary_monitor = dictionary_owner.monitor();
        let dictionary = Dictionary::from_bytes_in(&serialized, dictionary_owner).unwrap();
        let dictionary_before = dictionary_monitor.snapshot();
        let workspace_owner = block_allocator();
        let workspace_monitor = workspace_owner.monitor();
        let mut encoder = BlockEncoder::<_, DictionaryView<'_>>::new_in(workspace_owner);
        encoder.set_strategy(strategy);
        encoder.start_frame(Some(dictionary.view()), 1024).unwrap();
        let mut reference = BlockEncoder::<_, DictionaryView<'_>>::new_in(Global);
        reference.set_strategy(strategy);
        reference
            .start_frame(Some(dictionary.view()), 1024)
            .unwrap();
        let output_owner = block_allocator().into_non_clone();
        let output_monitor = output_owner.monitor();
        let mut encoded = Vec::new_in(output_owner);
        let mut expected = Vec::new();
        let pattern = b"a useful raw dictionary pattern abcdefghijklmnopqrstuvwxyz".repeat(4);
        let blocks: [&[u8]; 4] = [b"raw", &[0xaa; 32], &pattern, &pattern];
        let mut source = Vec::new();
        for (index, block) in blocks.into_iter().enumerate() {
            encoder.encode(block, index == 3, &mut encoded).unwrap();
            reference.encode(block, index == 3, &mut expected).unwrap();
            assert_eq!(encoded, expected);
            assert_eq!(encoder.frame_position, reference.frame_position);
            assert_eq!(
                encoder.history.as_ref().unwrap().frame_len(),
                encoder.frame_position
            );
            assert_eq!(
                workspace_monitor.snapshot().live_bytes,
                encoder.retained_capacity() as u128
            );
            source.extend_from_slice(block);
        }
        assert_eq!(dictionary_monitor.snapshot(), dictionary_before);
        assert_eq!(output_monitor.snapshot().identities_created, 1);
        assert_eq!(
            output_monitor.snapshot().live_bytes,
            encoded.capacity() as u128
        );
        let before_reset = workspace_monitor.snapshot();
        let retained = encoder.retained_capacity();
        let mut moved = encoder;
        moved.reset();
        assert_eq!(workspace_monitor.snapshot(), before_reset);
        assert_eq!(moved.retained_capacity(), retained);
        assert!(moved.active_dictionary.is_none());
        drop(moved);
        workspace_monitor.assert_released();
        drop(reference);

        // Frame assembly and oracle storage are separate test infrastructure;
        // the independently allocated block output is compared before copying.
        let mut frame = Vec::new();
        frame.try_reserve_exact(source.len() + 64).unwrap();
        write_frame_header(&mut frame, source.len(), dictionary.id());
        frame.extend_from_slice(&encoded);
        frame.extend_from_slice(&(xxhash::checksum(&source) as u32).to_le_bytes());
        assert_eq!(
            crate::decompress_with_dictionary_in(
                &frame,
                &dictionary.view(),
                DecodeOptions::new(source.len()),
                Global,
            )
            .unwrap(),
            source
        );
        drop(encoded);
        output_monitor.assert_released();
        drop(dictionary);
        dictionary_monitor.assert_released();
        (frame, source, serialized)
    }

    #[test]
    fn allocator_block_all_strategies_keep_dictionary_workspace_and_output_domains_independent() {
        for strategy in [
            CompressionStrategy::Greedy,
            CompressionStrategy::Chain,
            CompressionStrategy::Lazy,
        ] {
            for prepared in [false, true] {
                allocator_block_case(strategy, prepared);
            }
        }
    }

    #[test]
    fn allocator_block_borrowed_nonclone_context_reuses_history_and_releases_shared_source() {
        let owner = block_allocator().into_non_clone();
        let monitor = owner.monitor();
        let dictionary = Dictionary::raw(b"abcabcabc!").unwrap().into_shared();
        let mut encoder = BlockEncoder::<_, SharedDictionary>::new_in(&owner);
        encoder.start_frame(Some(dictionary.clone()), 32).unwrap();
        assert_eq!(SharedDictionary::strong_count(&dictionary), 2);
        let mut reference = BlockEncoder::new();
        reference.start_frame(Some(dictionary.clone()), 32).unwrap();
        let (mut output, mut expected) = (Vec::new(), Vec::new());
        for (index, block) in [&b"abcabcabc!"[..], &b"abcabcabc!tail"[..]]
            .into_iter()
            .enumerate()
        {
            encoder.encode(block, index == 1, &mut output).unwrap();
            reference.encode(block, index == 1, &mut expected).unwrap();
            assert_eq!(output, expected);
        }
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(
            monitor.snapshot().live_bytes,
            encoder.retained_capacity() as u128
        );
        let before = monitor.snapshot();
        let retained = encoder.retained_capacity();
        encoder.reset();
        assert_eq!(monitor.snapshot(), before);
        assert_eq!(encoder.retained_capacity(), retained);
        assert_eq!(SharedDictionary::strong_count(&dictionary), 2); // Caller + reference.
        encoder.start_frame(None, 32).unwrap();
        assert!(encoder.active_dictionary.is_none());
        drop(encoder);
        monitor.assert_released();
        drop(reference);
        assert_eq!(SharedDictionary::strong_count(&dictionary), 1);
    }

    #[test]
    fn allocator_block_every_setup_and_encode_failure_recovers_after_reset() {
        let dictionary = allocator_dictionary(true);
        let source = b"a useful raw dictionary pattern abcdefghijklmnopqrstuvwxyz".repeat(4);
        for strategy in [
            CompressionStrategy::Greedy,
            CompressionStrategy::Chain,
            CompressionStrategy::Lazy,
        ] {
            let baseline = block_allocator().into_non_clone();
            let baseline_monitor = baseline.monitor();
            let mut probe = BlockEncoder::<_, DictionaryView<'_>>::new_in(&baseline);
            probe.set_strategy(strategy);
            let mut expected = vec![0xee, 0x55];
            probe.start_frame(Some(dictionary.view()), 1024).unwrap();
            probe.encode(&source, true, &mut expected).unwrap();
            let attempts = baseline_monitor.snapshot().attempts;
            assert!(attempts > 10);
            drop(probe);
            baseline_monitor.assert_released();
            for ordinal in 1..=attempts {
                let owner = TrackingAllocator::new(AllocatorConfig {
                    max_live_allocations: 128,
                    fail_at: NonZeroU64::new(ordinal),
                })
                .unwrap()
                .into_non_clone();
                let monitor = owner.monitor();
                let mut encoder = BlockEncoder::<_, DictionaryView<'_>>::new_in(&owner);
                encoder.set_strategy(strategy);
                let mut output = vec![0xee, 0x55];
                let result = encoder
                    .start_frame(Some(dictionary.view()), 1024)
                    .and_then(|()| encoder.encode(&source, true, &mut output));
                assert_eq!(
                    result,
                    Err(Error::AllocationFailed),
                    "{strategy:?}, allocation {ordinal}"
                );
                assert_eq!(monitor.snapshot().attempts, ordinal);
                assert_eq!(monitor.snapshot().failures, 1);
                assert_eq!(output, [0xee, 0x55]);
                assert_eq!(encoder.frame_position, 0);
                assert_eq!(
                    encoder.history.as_ref().map_or(0, RingHistory::frame_len),
                    0
                );
                assert_eq!(
                    monitor.snapshot().live_bytes,
                    encoder.retained_capacity() as u128
                );
                // Setup/parse failures may alter seeds and scratch. Recovery
                // obeys the existing reset contract instead of assuming rollback.
                monitor.clear_failure();
                encoder.reset();
                encoder.start_frame(Some(dictionary.view()), 1024).unwrap();
                encoder.encode(&source, true, &mut output).unwrap();
                assert_eq!(output, expected);
                assert_eq!(
                    monitor.snapshot().live_bytes,
                    encoder.retained_capacity() as u128
                );
                drop(encoder);
                monitor.assert_released();
            }
        }
    }

    #[test]
    fn allocator_block_output_failure_keeps_prior_bytes_and_committed_history() {
        let owner = block_allocator().into_non_clone();
        let monitor = owner.monitor();
        let mut encoder = BlockEncoder::<_, SharedDictionary>::new_in(&owner);
        encoder.start_frame(None, 1024).unwrap();
        let output_owner = block_allocator().into_non_clone();
        let output_monitor = output_owner.monitor();
        let mut output = Vec::new_in(output_owner);
        let first = b"abcd".repeat(16);
        encoder.encode(&first, false, &mut output).unwrap();
        let before = output.to_vec();
        let output_pointer = output.as_ptr();
        let output_capacity = output.capacity();
        let workspace_before = monitor.snapshot();
        assert!(output_monitor.fail_next());
        assert_eq!(
            encoder.encode(&[42; 512], true, &mut output),
            Err(Error::AllocationFailed)
        );
        assert_eq!(output, before);
        assert_eq!(output.as_ptr(), output_pointer);
        assert_eq!(output.capacity(), output_capacity);
        assert_eq!(encoder.frame_position, first.len() as u64);
        assert_eq!(
            encoder.history.as_ref().unwrap().frame_len(),
            first.len() as u64
        );
        assert_eq!(monitor.snapshot(), workspace_before);
        output_monitor.clear_failure();
        encoder.reset();
        encoder.start_frame(None, 1024).unwrap();
        output.clear();
        encoder.encode(&first, false, &mut output).unwrap();
        assert_eq!(output, before);
        drop(output);
        drop(encoder);
        output_monitor.assert_released();
        monitor.assert_released();
    }

    #[test]
    fn allocator_block_failed_dictionary_setup_requires_reset_after_seed_mutation() {
        let original = allocator_dictionary(true);
        let mut bytes = original.as_bytes().to_vec();
        let content_start = bytes.len() - original.content().len();
        bytes[content_start..].fill(b'Z');
        let replacement = Dictionary::from_bytes(&bytes).unwrap();
        let first = Dictionary::raw(b"aaaaaaaaaaaaaaaa").unwrap();
        let owner = block_allocator().into_non_clone();
        let monitor = owner.monitor();
        let mut encoder = BlockEncoder::<_, DictionaryView<'_>>::new_in(&owner);
        encoder.start_frame(Some(first.view()), 1024).unwrap();
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        assert_eq!(
            encoder.start_frame(Some(replacement.view()), 1024),
            Err(Error::AllocationFailed)
        );
        assert_eq!(
            encoder.active_dictionary.as_ref().unwrap().as_bytes(),
            first.as_bytes()
        );
        assert_eq!(monitor.snapshot().live_bytes, before.live_bytes);
        let mut inspection = MatchTable::new().unwrap();
        inspection
            .reset_from(encoder.dictionary_seed.as_ref().unwrap())
            .unwrap();
        assert!(inspection.replace(*b"ZZZZ", usize::MAX).unwrap().is_some());
        assert_eq!(inspection.replace(*b"aaaa", usize::MAX).unwrap(), None);
        monitor.clear_failure();
        encoder.reset();
        encoder.start_frame(Some(first.view()), 1024).unwrap();
        inspection
            .reset_from(encoder.dictionary_seed.as_ref().unwrap())
            .unwrap();
        assert!(inspection.replace(*b"aaaa", usize::MAX).unwrap().is_some());
        let mut output = Vec::new();
        encoder.encode(first.content(), true, &mut output).unwrap();
        let mut reference = BlockEncoder::<_, DictionaryView<'_>>::new_in(Global);
        reference.start_frame(Some(first.view()), 1024).unwrap();
        let mut expected = Vec::new();
        reference
            .encode(first.content(), true, &mut expected)
            .unwrap();
        assert_eq!(output, expected);
        drop(encoder);
        monitor.assert_released();
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN"]
    fn official_codec_decodes_allocator_owned_blocks_with_borrowed_dictionaries() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let executable = std::env::var_os("ZSTD_BIN").expect("set ZSTD_BIN");
        let version = Command::new(&executable).arg("--version").output().unwrap();
        assert!(version.status.success());
        assert!(std::string::String::from_utf8_lossy(&version.stdout).contains("1.5.7"));
        struct File(std::path::PathBuf);
        impl Drop for File {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let path = File(std::env::temp_dir().join(std::format!(
            "zstdoxide-allocator-block-{}-{}-{}.dict",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        )));
        let mut cases = 0;
        for strategy in [
            CompressionStrategy::Greedy,
            CompressionStrategy::Chain,
            CompressionStrategy::Lazy,
        ] {
            for prepared in [false, true] {
                let (frame, source, dictionary) = allocator_block_case(strategy, prepared);
                std::fs::write(&path.0, &dictionary).unwrap();
                let mut child = Command::new(&executable)
                    .args(["-q", "-d", "-c", "-D"])
                    .arg(&path.0)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                child.stdin.take().unwrap().write_all(&frame).unwrap();
                let decoded = child.wait_with_output().unwrap();
                assert!(
                    decoded.status.success(),
                    "{strategy:?}/{prepared}: {}",
                    std::string::String::from_utf8_lossy(&decoded.stderr)
                );
                assert_eq!(decoded.stdout, source);
                cases += 1;
            }
        }
        assert_eq!(cases, 6);
        std::eprintln!(
            "Official v1.5.7 decoded {cases} allocator-owned frames with raw/prepared dictionary views"
        );
    }

    #[test]
    fn sequence_state_is_published_only_for_accepted_blocks_and_reset_for_new_frames() {
        let probe = [LogicalSequence {
            literal_length: 1,
            match_length: 4,
            distance: 4,
        }];
        let mut initial = SequenceEncoder::new();
        initial.prepare(&probe).unwrap();
        let initial_bytes = initial.as_bytes().to_vec();
        let mut encoder = BlockEncoder::new();
        let mut output = Vec::new();
        // This candidate has a real sequence but cannot repay its headers.
        encoder.encode(b"abcdabcd", false, &mut output).unwrap();
        assert!(!encoder.sequences.is_empty());
        assert_eq!((output[0] >> 1) & 3, 0);
        encoder.sequence_encoder.prepare(&probe).unwrap();
        assert_eq!(encoder.sequence_encoder.as_bytes(), initial_bytes);
        output.clear();
        encoder.encode(&[42; 32], false, &mut output).unwrap();
        assert_eq!((output[0] >> 1) & 3, 1);
        encoder.sequence_encoder.prepare(&probe).unwrap();
        assert_eq!(encoder.sequence_encoder.as_bytes(), initial_bytes);

        encoder.reset();
        let block = b"abcd".repeat(1024);
        output.clear();
        encoder.encode(&block, false, &mut output).unwrap();
        assert_eq!((output[0] >> 1) & 3, 2);
        let mut expected = SequenceEncoder::new();
        expected.prepare(&encoder.sequences).unwrap();
        expected.commit();
        expected.prepare(&probe).unwrap();
        encoder.sequence_encoder.prepare(&probe).unwrap();
        let committed_bytes = encoder.sequence_encoder.as_bytes().to_vec();
        assert_eq!(committed_bytes, expected.as_bytes());
        assert_ne!(committed_bytes, initial_bytes);
        for block in [&b"xyz"[..], &[9; 32][..]] {
            output.clear();
            encoder.encode(block, false, &mut output).unwrap();
            encoder.sequence_encoder.prepare(&probe).unwrap();
            assert_eq!(encoder.sequence_encoder.as_bytes(), committed_bytes);
        }
        encoder.reset();
        encoder.sequence_encoder.prepare(&probe).unwrap();
        assert_eq!(encoder.sequence_encoder.as_bytes(), initial_bytes);
        encoder.start_frame(None, 1024).unwrap();
        encoder.sequence_encoder.prepare(&probe).unwrap();
        assert_eq!(encoder.sequence_encoder.as_bytes(), initial_bytes);
    }

    #[test]
    fn only_emitted_compressed_blocks_publish_pending_literal_tables() {
        let mut lengths = [0; 256];
        lengths[2] = 1;
        lengths[3] = 1;
        let mut encoder = BlockEncoder::new();
        encoder.literal_table = Some(HuffmanCodebook::from_lengths(&lengths, 11).unwrap());
        // The new seven-byte literal section beats its nine-byte raw section,
        // but its zero sequence byte makes the entire eight-byte block a tie.
        let rejected = [0, 0, 0, 0, 1, 0, 0, 1];
        assert!(!encoder.compress_block(&rejected).unwrap());
        assert!(encoder.pending_literal_table.is_some());
        assert!(encoder.literal_table.as_ref().unwrap().covers(&[2, 3]));
        assert!(!encoder.literal_table.as_ref().unwrap().covers(&[0, 1]));
        let mut encoded = Vec::new();
        encoder.encode(&rejected, false, &mut encoded).unwrap();
        assert_eq!((encoded[0] >> 1) & 3, 0);
        assert!(encoder.pending_literal_table.is_none());
        assert!(encoder.literal_table.as_ref().unwrap().covers(&[2, 3]));
        encoded.clear();
        encoder.encode(&[42; 32], false, &mut encoded).unwrap();
        assert_eq!((encoded[0] >> 1) & 3, 1);
        assert!(encoder.literal_table.as_ref().unwrap().covers(&[2, 3]));

        encoder.reset();
        assert!(encoder.literal_table.is_none());
        assert!(encoder.pending_literal_table.is_none());
        let accepted = [0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 0, 1, 1, 1, 1];
        encoded.clear();
        encoder.encode(&accepted, false, &mut encoded).unwrap();
        assert_eq!((encoded[0] >> 1) & 3, 2);
        assert!(encoder.pending_literal_table.is_none());
        assert!(encoder.literal_table.as_ref().unwrap().covers(&[0, 1]));
        encoder.start_frame(None, MAX_BLOCK_SIZE as u64).unwrap();
        assert!(encoder.literal_table.is_none());
    }

    #[test]
    fn literal_only_compression_omits_sequence_modes_and_state_bits() {
        fn de_bruijn(t: usize, period: usize, word: &mut [u8; 5], output: &mut Vec<u8>) {
            if t == 5 {
                if 4 % period == 0 {
                    output.extend_from_slice(&word[1..=period]);
                }
                return;
            }
            word[t] = word[t - period];
            de_bruijn(t + 1, period, word, output);
            for symbol in word[t - period] + 1..16 {
                word[t] = symbol;
                de_bruijn(t + 1, t, word, output);
            }
        }
        let mut source = Vec::new();
        de_bruijn(1, 1, &mut [0; 5], &mut source);
        assert_eq!(source.len(), 65_536);
        let mut seen = vec![false; 65_536];
        for word in source.windows(4) {
            let index = word
                .iter()
                .fold(0, |value, &symbol| (value << 4) | usize::from(symbol));
            assert!(!seen[index]);
            seen[index] = true;
        }
        let mut encoder = BlockEncoder::new();
        let mut output = Vec::new();
        encoder.encode(&source, true, &mut output).unwrap();
        assert!(encoder.sequences.is_empty());
        assert_eq!((output[0] >> 1) & 3, 2);
        assert!(output.len() < source.len() * 3 / 4);
        let (literals, consumed) =
            crate::huffman::decode_literals(&output[3..], &mut None, MAX_BLOCK_SIZE).unwrap();
        assert_eq!(literals, source);
        assert_eq!(&output[3 + consumed..], &[0]);
        let frame = compress(&source).unwrap();
        assert_eq!(crate::decompress(&frame, source.len()).unwrap(), source);
    }

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
                dictionary: Some(dictionary.view()),
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
    fn formatted_dictionary_matches_plan_custom_initial_repeat_offsets() {
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
    fn dictionary_candidate_lifetime_and_search_distance_limits_are_checked_at_start() {
        let dictionary = Dictionary::raw(b"abcd").unwrap();
        let mut source = MatchSource::<Global> {
            block: b"abcdabcdabcd",
            dictionary: Some(dictionary.view()),
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
        working
            .reset_from(encoder.dictionary_seed.as_ref().unwrap())
            .unwrap();
        assert_eq!(
            working.replace(*b"dict", first.content().len()).unwrap(),
            Some(0)
        );
        encoder.reset();
        assert_eq!(SharedDictionary::strong_count(&first), 1);
        assert!(encoder.active_dictionary.is_none());
        assert_eq!(encoder.frame_position, 0);
        assert_eq!(encoder.retained_capacity(), capacity);

        encoder
            .start_frame(Some(SharedDictionary::clone(&second)), 4096)
            .unwrap();
        assert_eq!(SharedDictionary::strong_count(&second), 2);
        working
            .reset_from(encoder.dictionary_seed.as_ref().unwrap())
            .unwrap();
        assert_eq!(
            working.replace(*b"repl", second.content().len()).unwrap(),
            Some(0)
        );
        assert_eq!(
            working.replace(*b"dict", second.content().len()).unwrap(),
            None
        );
        assert_eq!(encoder.retained_capacity(), capacity);
        encoder.start_frame(None, 128 * 1024).unwrap();
        assert_eq!(SharedDictionary::strong_count(&second), 1);
        working
            .reset_from(encoder.dictionary_seed.as_ref().unwrap())
            .unwrap();
        assert_eq!(working.replace(*b"repl", 100).unwrap(), None);
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
        // Warm both literal distributions: the short intervening block has
        // more unmatched literals than this highly repetitive full block.
        encoder
            .encode(b"a different preceding block", false, &mut encoded)
            .unwrap();
        encoder.reset();
        encoded.clear();
        encoder.encode(&block, true, &mut encoded).unwrap();
        assert_eq!(encoded, expected);
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
    fn sequence_count_boundaries_and_overlapping_expansion_round_trip() {
        for count in [1, 127, 128, 255, 256, 0x7eff, 0x7f00] {
            let mut sequences = vec![
                LogicalSequence {
                    literal_length: 0,
                    match_length: 4,
                    distance: 1
                };
                count
            ];
            sequences[0].literal_length = 1;
            let mut context = SequenceEncoder::new();
            context.prepare(&sequences).unwrap();
            let section = context.as_bytes();
            let length = count * 4 + 1;
            let mut history = crate::history::History::new(length);
            history.start_frame(length);
            crate::sequences::SequenceState::new()
                .decode(section, b"x", &mut history, MAX_BLOCK_SIZE)
                .unwrap();
            assert_eq!(history.into_bytes(), vec![b'x'; length]);
        }
    }

    #[test]
    fn source_keys_join_dictionary_history_and_input_at_every_selected_width() {
        let dictionary = Dictionary::raw(b"ab").unwrap();
        let mut history = RingHistory::new(8);
        history.start_frame(8).unwrap();
        history.commit(b"cd").unwrap();
        let source = MatchSource {
            block: b"efghijkl",
            dictionary: Some(dictionary.view()),
            dictionary_size: 2,
            history: Some(&history),
            history_size: 2,
            frame_position: 2,
            window_size: 8,
        };
        let joined = b"abcdefghijkl";
        for width in 4..=8 {
            let layout = SearchKeyLayout::new(10, width).unwrap();
            for start in 0..=2 {
                assert_eq!(
                    source.dictionary_boundary_key(start, layout).unwrap(),
                    layout.read_key(&joined[start..])
                );
            }
            for start in 0..=10 {
                assert_eq!(
                    source.output_key(start, layout).unwrap(),
                    layout.read_key(&joined[2 + start..])
                );
            }
        }
        let layout = SearchKeyLayout::new(18, 8).unwrap();
        // This key crosses both boundaries in a single eight-byte extraction.
        assert_eq!(
            source.dictionary_boundary_key(0, layout).unwrap(),
            layout.read_key(b"abcdefgh")
        );
        // D itself is the frame-zero boundary, with both H and B contributing.
        assert_eq!(
            source.dictionary_boundary_key(2, layout).unwrap(),
            layout.read_key(b"cdefghij")
        );
        assert_eq!(source.dictionary_boundary_word(0).unwrap(), *b"abcd");
        assert_eq!(source.output_word(0).unwrap(), *b"cdef");
        assert_eq!(history.frame_len(), 2);
        assert_eq!(dictionary.content(), b"ab");
    }

    #[test]
    fn source_keys_read_wrapped_history_and_current_input_without_stored_pointers() {
        let mut history = RingHistory::new(5);
        history.start_frame(5).unwrap();
        history.commit(b"abcde").unwrap();
        history.commit(b"fg").unwrap();
        let source = MatchSource {
            block: b"hijklmn",
            dictionary: None,
            dictionary_size: 0,
            history: Some(&history),
            history_size: 5,
            frame_position: 7,
            window_size: 16,
        };
        let expected = b"cdefghijklmn";
        for width in 4..=8 {
            let layout = SearchKeyLayout::new(16, width).unwrap();
            for start in 0..=expected.len() {
                assert_eq!(
                    source.output_key(start, layout).unwrap(),
                    layout.read_key(&expected[start..])
                );
            }
        }
        assert_eq!(source.output_word(3).unwrap(), *b"fghi");
        assert_eq!(history.byte_at(1), Err(Error::CorruptData));
        assert_eq!(history.byte_at(2).unwrap(), b'c');
        assert_eq!(history.byte_at(6).unwrap(), b'g');
    }

    #[test]
    fn source_keys_short_flush_tails_return_none_without_padding() {
        let bytes = b"abcdefgh";
        for length in 0..=bytes.len() {
            let source = MatchSource::<Global> {
                block: &bytes[..length],
                dictionary: None,
                dictionary_size: 0,
                history: None,
                history_size: 0,
                frame_position: 0,
                window_size: 1024,
            };
            for width in 4..=8 {
                let layout = SearchKeyLayout::new(10, width).unwrap();
                assert_eq!(
                    source.dictionary_boundary_key(0, layout).unwrap(),
                    layout.read_key(&bytes[..length])
                );
                for start in 0..=length {
                    assert_eq!(
                        source.output_key(start, layout).unwrap(),
                        layout.read_key(&bytes[start..length])
                    );
                }
                if length < usize::from(width) {
                    assert_eq!(source.output_key(0, layout), Ok(None));
                }
            }
        }
        let dictionary = Dictionary::raw(b"a").unwrap();
        let mut history = RingHistory::new(1);
        history.start_frame(1).unwrap();
        history.commit(b"b").unwrap();
        for length in 0..=6 {
            let source = MatchSource {
                block: &b"cdefgh"[..length],
                dictionary: Some(dictionary.view()),
                dictionary_size: 1,
                history: Some(&history),
                history_size: 1,
                frame_position: 1,
                window_size: 1024,
            };
            for width in 4..=8 {
                let layout = SearchKeyLayout::new(18, width).unwrap();
                assert_eq!(
                    source.dictionary_boundary_key(0, layout).unwrap(),
                    layout.read_key(&bytes[..2 + length])
                );
                assert_eq!(
                    source.output_key(0, layout).unwrap(),
                    layout.read_key(&bytes[1..2 + length])
                );
            }
        }
    }

    #[test]
    fn source_keys_empty_regions_and_exact_endpoints_have_explicit_results() {
        let empty = Dictionary::raw(&[]).unwrap();
        let mut history = RingHistory::new(8);
        history.start_frame(8).unwrap();
        let layout = SearchKeyLayout::new(16, 4).unwrap();
        let mut source = MatchSource {
            block: &[],
            dictionary: Some(empty.view()),
            dictionary_size: 0,
            history: Some(&history),
            history_size: 0,
            frame_position: 0,
            window_size: 8,
        };
        for _ in 0..2 {
            assert_eq!(source.dictionary_boundary_key(0, layout), Ok(None));
            assert_eq!(source.output_key(0, layout), Ok(None));
            assert_eq!(
                source.dictionary_boundary_key(1, layout),
                Err(Error::CorruptData)
            );
            assert_eq!(source.output_key(1, layout), Err(Error::CorruptData));
            source.dictionary = None;
            source.history = None;
        }
        let dictionary = Dictionary::raw(b"abcd").unwrap();
        source.dictionary = Some(dictionary.view());
        source.dictionary_size = 4;
        assert_eq!(
            source.dictionary_boundary_key(0, layout).unwrap(),
            layout.read_key(b"abcd")
        );
        assert_eq!(source.dictionary_boundary_key(4, layout), Ok(None));
        assert_eq!(source.output_key(0, layout), Ok(None));
        assert_eq!(
            source.dictionary_boundary_key(5, layout),
            Err(Error::CorruptData)
        );
        assert_eq!(source.output_key(1, layout), Err(Error::CorruptData));
    }

    #[test]
    fn source_keys_reject_evicted_frame_zero_while_dictionary_only_matches_remain_live() {
        let dictionary = Dictionary::raw(b"abcd").unwrap();
        let mut history = RingHistory::new(4);
        history.start_frame(4).unwrap();
        history.commit(b"abcd").unwrap();
        let layout = SearchKeyLayout::new(16, 8).unwrap();
        {
            let source = MatchSource {
                block: b"abcdabcd",
                dictionary: Some(dictionary.view()),
                dictionary_size: 4,
                history: Some(&history),
                history_size: 4,
                frame_position: 4,
                window_size: 64,
            };
            assert_eq!(
                source.dictionary_boundary_key(0, layout).unwrap(),
                layout.read_key(b"abcdabcd")
            );
            assert_eq!(source.find_match(0, 0).unwrap(), Some((8, 8)));
        }
        history.commit(b"ef").unwrap();
        let source = MatchSource {
            block: b"abcdabcd",
            dictionary: Some(dictionary.view()),
            dictionary_size: 4,
            history: Some(&history),
            history_size: 4,
            frame_position: 6,
            window_size: 64,
        };
        for width in 4..=8 {
            let layout = SearchKeyLayout::new(16, width).unwrap();
            for position in 0..=4 {
                assert_eq!(
                    source.dictionary_boundary_key(position, layout),
                    Err(Error::InvalidState)
                );
            }
        }
        assert_eq!(
            source.output_key(0, layout).unwrap(),
            layout.read_key(b"cdefabcd")
        );
        assert_eq!(source.find_match(0, 0).unwrap(), Some((10, 4)));
        assert_eq!(dictionary.content(), b"abcd");
    }

    #[test]
    fn source_keys_validate_index_extents_and_keep_absolute_frame_coordinates_u64() {
        let layout = SearchKeyLayout::new(10, 8).unwrap();
        let mut source = MatchSource::<Global> {
            block: b"abcdefgh",
            dictionary: None,
            dictionary_size: 0,
            history: None,
            history_size: 0,
            frame_position: (1_u64 << 32) + 5,
            window_size: 1 << 20,
        };
        assert_eq!(
            source.output_key(0, layout).unwrap(),
            layout.read_key(b"abcdefgh")
        );
        assert_eq!(source.frame_position, (1_u64 << 32) + 5);
        assert_eq!(
            source.dictionary_boundary_key(0, layout),
            Err(Error::InvalidState)
        );
        for position in [9, usize::MAX] {
            assert_eq!(source.output_key(position, layout), Err(Error::CorruptData));
            assert_eq!(
                source.dictionary_boundary_key(position, layout),
                Err(Error::CorruptData)
            );
        }
        source.frame_position = u64::MAX - 8;
        assert_eq!(
            source.output_key(0, layout).unwrap(),
            layout.read_key(b"abcdefgh")
        );
        source.frame_position += 1;
        assert_eq!(source.output_key(0, layout), Err(Error::ResourceLimit));
        assert_eq!(
            source.dictionary_boundary_key(0, layout),
            Err(Error::ResourceLimit)
        );
        source.block = &[];
        source.frame_position = u64::MAX;
        assert_eq!(source.output_key(0, layout), Ok(None));
        for (dictionary_size, history_size, block) in [
            (usize::MAX, 0, &b"x"[..]),
            (0, usize::MAX, &b"x"[..]),
            (usize::MAX - 2, 1, &b"xy"[..]),
        ] {
            source.dictionary_size = dictionary_size;
            source.history_size = history_size;
            source.block = block;
            assert_eq!(source.output_key(0, layout), Err(Error::ResourceLimit));
            assert_eq!(
                source.dictionary_boundary_key(0, layout),
                Err(Error::ResourceLimit)
            );
        }
        source.dictionary_size = 0;
        source.history_size = 4;
        source.frame_position = 3;
        source.block = b"abcdefgh";
        assert_eq!(source.output_key(0, layout), Err(Error::CorruptData));
        assert_eq!(
            source.dictionary_boundary_key(0, layout),
            Err(Error::CorruptData)
        );
        // Enough logical bytes with unavailable storage must be an error, not a
        // zero-padded key or a valid short tail. No invalid memory is constructed.
        source.frame_position = 4;
        assert_eq!(source.output_key(0, layout), Err(Error::CorruptData));
        source.history_size = 0;
        source.frame_position = 0;
        source.dictionary_size = 4;
        assert_eq!(
            source.dictionary_boundary_key(0, layout),
            Err(Error::InvalidState)
        );
    }

    #[test]
    fn source_keys_borrow_independent_dictionary_and_ring_allocator_domains() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let dictionary_owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let history_owner = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let history_monitor = history_owner.monitor();
        let dictionary = Dictionary::raw_in(b"ab", &dictionary_owner).unwrap();
        let mut history = RingHistory::new_in(8, &history_owner);
        history.start_frame(8).unwrap();
        history.commit(b"cd").unwrap();
        let dictionary_before = dictionary_monitor.snapshot();
        let history_before = history_monitor.snapshot();
        let capacity = history.retained_capacity();
        assert!(dictionary_monitor.fail_next());
        assert!(history_monitor.fail_next());
        {
            let source = MatchSource {
                block: b"efghijkl",
                dictionary: Some(dictionary.view()),
                dictionary_size: 2,
                history: Some(&history),
                history_size: 2,
                frame_position: 2,
                window_size: 8,
            };
            for width in 4..=8 {
                let layout = SearchKeyLayout::new(10, width).unwrap();
                assert_eq!(
                    source.dictionary_boundary_key(0, layout).unwrap(),
                    layout.read_key(b"abcdefghijkl")
                );
                assert_eq!(
                    source.output_key(0, layout).unwrap(),
                    layout.read_key(b"cdefghijkl")
                );
                assert_eq!(source.output_key(10, layout), Ok(None));
                assert_eq!(
                    source.dictionary_boundary_key(3, layout),
                    Err(Error::CorruptData)
                );
            }
        }
        assert_eq!(dictionary_monitor.snapshot(), dictionary_before);
        assert_eq!(history_monitor.snapshot(), history_before);
        assert_eq!(dictionary.content(), b"ab");
        assert_eq!(history.frame_len(), 2);
        assert_eq!(history.byte_at(0).unwrap(), b'c');
        assert_eq!(history.byte_at(1).unwrap(), b'd');
        assert_eq!(history.retained_capacity(), capacity);
        drop(dictionary);
        drop(history);
        drop(dictionary_owner);
        drop(history_owner);
        dictionary_monitor.assert_released();
        history_monitor.assert_released();
    }
}
