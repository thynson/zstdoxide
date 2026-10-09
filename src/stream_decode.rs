//! Bounded frame state machine with caller-owned output and indexed history.

#[cfg(test)]
use crate::Dictionary;
use crate::cursor::ByteCursor;
use crate::dictionary::{DictionaryHandle, DictionaryView};
use crate::frame::{BlockHeader, BlockKind, MAX_BLOCK_SIZE};
use crate::history::SequenceOutput;
use crate::huffman::{HuffmanTable, decode_literals_with_dictionary_view_in};
use crate::ring_history::RingHistory;
use crate::sequences::SequenceState;
use crate::xxhash::XxHash64;
use crate::{DecodeOptions, Error, FrameHeader, Progress, Result, SharedDictionary, StreamStatus};
use alloc::alloc::{Allocator, Global};
use alloc::vec::Vec;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Header,
    BlockHeader,
    Raw,
    RleByte,
    Rle,
    Compressed,
    Drain,
    SkipLength,
    Skip,
    Checksum,
    Ended,
    Failed,
}

/// Incrementally decode concatenated standard and skippable frames.
///
/// The context owns its bounded window and block storage. Input/output slices
/// are never retained; a borrowed dictionary handle retains its explicit borrow.
/// Every frame boundary is reported before consuming the next frame's magic.
/// Output is provisional until that frame's checksum has been validated. An
/// error poisons the context until reset; output touched by a failing call must
/// be discarded. Corresponds to `ZSTD_DCtx` and `ZSTD_decompressStream`.
pub struct Decoder<A: Allocator = Global, D: DictionaryHandle = SharedDictionary> {
    allocator: A,
    options: DecodeOptions,
    phase: Phase,
    scratch: [u8; 18],
    filled: usize,
    frame: Option<FrameHeader>,
    max_block: usize,
    last_block: bool,
    remaining: usize,
    rle_byte: u8,
    history: RingHistory<A>,
    huffman: Option<HuffmanTable<A>>,
    sequences: SequenceState<A>,
    encoded: Vec<u8, A>,
    pending: Vec<u8, A>,
    pending_position: usize,
    checksum: XxHash64,
    decoded_total: u64,
    input_total: u64,
    output_total: u64,
    completed_frames: u64,
    dictionary: Option<D>,
    prefix: Option<D>,
    active_dictionary: Option<D>,
}

impl Decoder<Global, SharedDictionary> {
    /// Create an allocation-free context with explicit stream resource limits.
    pub fn new(options: DecodeOptions) -> Self {
        Self::new_in(options, Global)
    }

    /// Create a context with a persistent immutable dictionary for every frame.
    pub fn with_dictionary(options: DecodeOptions, dictionary: SharedDictionary) -> Self {
        Self::with_dictionary_in(options, dictionary, Global)
    }
}

impl<A: Allocator + Clone> Decoder<A, SharedDictionary> {
    /// Retain an allocator and construct empty workspace owners without
    /// allocating their payloads. Each cloned allocator owns only fresh storage;
    /// ordinary allocator cloning has the caller's own panic/allocation policy.
    pub fn new_in(options: DecodeOptions, allocator: A) -> Self {
        Self::empty_in(options, allocator)
    }
}

impl<A: Allocator + Clone, D: DictionaryHandle> Decoder<A, D> {
    /// Attach a persistent shared owner or borrowed dictionary view independently
    /// of the workspace allocator. Prepared tables are borrowed during decoding;
    /// neither attachment nor frame reset copies their allocations. A borrowed
    /// view must remain valid for the context's lifetime. Shared ownership keeps
    /// its existing Global Arc/Rc control allocation outside the selected domain.
    pub fn with_dictionary_in(options: DecodeOptions, dictionary: D, allocator: A) -> Self {
        let mut decoder = Self::empty_in(options, allocator);
        decoder.dictionary = Some(dictionary);
        decoder
    }

    fn empty_in(options: DecodeOptions, allocator: A) -> Self {
        Self {
            options,
            phase: Phase::Header,
            scratch: [0; 18],
            filled: 0,
            frame: None,
            max_block: 0,
            last_block: false,
            remaining: 0,
            rle_byte: 0,
            history: RingHistory::new_in(
                usize::try_from(options.max_window_size).unwrap_or(usize::MAX),
                allocator.clone(),
            ),
            huffman: None,
            sequences: SequenceState::new_in(allocator.clone()),
            encoded: Vec::new_in(allocator.clone()),
            pending: Vec::new_in(allocator.clone()),
            pending_position: 0,
            checksum: XxHash64::new(),
            decoded_total: 0,
            input_total: 0,
            output_total: 0,
            completed_frames: 0,
            dictionary: None,
            prefix: None,
            active_dictionary: None,
            allocator,
        }
    }

    /// Replace the persistent dictionary and clear an unclaimed prefix.
    ///
    /// Configure only at a clean frame boundary, before any next header byte
    /// has been accepted. Limits and counters remain unchanged. Rejection leaves
    /// the context unchanged; a failed or sealed context must first be reset.
    /// Corresponds to `ZSTD_DCtx_refDDict` with the attachment's enforced lifetime.
    pub fn set_dictionary(&mut self, dictionary: Option<D>) -> Result<()> {
        self.check_configuration_boundary()?;
        self.dictionary = dictionary;
        self.prefix = None;
        Ok(())
    }

    /// Configure a prefix for the next standard frame and clear the dictionary.
    ///
    /// Skippable frames do not claim this handle. Once a standard frame claims
    /// it, completion or reset releases it. Reset preserves an unclaimed prefix.
    /// Configuration is accepted only at a clean boundary, as for
    /// [`Self::set_dictionary`]. Use [`crate::Dictionary::raw`] for uninterpreted bytes.
    /// Corresponds to `ZSTD_DCtx_refPrefix_advanced` with the attachment's lifetime.
    pub fn set_prefix(&mut self, prefix: D) -> Result<()> {
        self.check_configuration_boundary()?;
        self.dictionary = None;
        self.prefix = Some(prefix);
        Ok(())
    }

    fn check_configuration_boundary(&self) -> Result<()> {
        if self.phase == Phase::Header && self.filled == 0 {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }

    /// Accept input and deliver output until a resource or frame boundary.
    ///
    /// Resubmit only the unconsumed suffix of `input`. Empty input can drain
    /// buffered output; empty output can consume headers and skippable frames.
    /// Counters include progress in a failing call, but such a call returns no
    /// slice positions and requires reset before reuse.
    pub fn process(&mut self, input: &[u8], output: &mut [u8]) -> Result<Progress> {
        if matches!(self.phase, Phase::Ended | Phase::Failed) {
            self.phase = Phase::Failed;
            return Err(Error::InvalidState);
        }
        let mut consumed = 0;
        let mut produced = 0;
        let result = self.process_inner(input, output, &mut consumed, &mut produced);
        let totals = self
            .input_total
            .checked_add(consumed as u64)
            .zip(self.output_total.checked_add(produced as u64));
        let result = match totals {
            Some((read, written)) => {
                self.input_total = read;
                self.output_total = written;
                result
            }
            None => Err(Error::ResourceLimit),
        };
        match result {
            Ok(status) => Ok(Progress {
                consumed,
                produced,
                status,
            }),
            Err(error) => {
                self.phase = Phase::Failed;
                Err(error)
            }
        }
    }

    /// Validate end of input and seal the context until reset.
    ///
    /// At least one complete standard or skippable frame is required. Call
    /// only after pending output has drained; any partial frame is truncated.
    pub fn finish(&mut self) -> Result<()> {
        if self.phase == Phase::Ended {
            return Ok(());
        }
        if self.phase == Phase::Failed {
            return Err(Error::InvalidState);
        }
        if self.phase == Phase::Header && self.filled == 0 && self.completed_frames != 0 {
            self.phase = Phase::Ended;
            Ok(())
        } else {
            self.phase = Phase::Failed;
            Err(Error::UnexpectedEof)
        }
    }

    /// Clear the session while retaining its limits and reusable allocation.
    /// Persistent dictionaries and unclaimed prefixes are preserved; an active
    /// single-use prefix is released along with its partial frame.
    pub fn reset(&mut self) {
        self.phase = Phase::Header;
        self.filled = 0;
        self.frame = None;
        self.active_dictionary = None;
        self.max_block = 0;
        self.last_block = false;
        self.remaining = 0;
        self.rle_byte = 0;
        self.history.clear();
        self.huffman = None;
        self.sequences.reset([1, 4, 8]);
        self.encoded.clear();
        self.pending.clear();
        self.pending_position = 0;
        self.checksum.reset();
        self.decoded_total = 0;
        self.input_total = 0;
        self.output_total = 0;
        self.completed_frames = 0;
    }

    /// Reset and apply new limits. A changed window cap releases old storage.
    pub fn reset_with_options(&mut self, options: DecodeOptions) {
        if options.max_window_size != self.options.max_window_size {
            self.history = RingHistory::new_in(
                usize::try_from(options.max_window_size).unwrap_or(usize::MAX),
                self.allocator.clone(),
            );
        }
        self.options = options;
        self.reset();
    }

    /// Limits currently applied to this session.
    pub fn options(&self) -> DecodeOptions {
        self.options
    }
    /// Accepted input bytes since the last reset.
    pub fn total_input(&self) -> u64 {
        self.input_total
    }
    /// Delivered output bytes since the last reset.
    pub fn total_output(&self) -> u64 {
        self.output_total
    }
    /// Reachable heap capacity, excluding inline state and caller buffers.
    /// Attached dictionary payloads are included once even when borrowed or
    /// shared elsewhere. This is reachable capacity, not ownership accounting;
    /// shared Arc/Rc control storage is excluded.
    pub fn retained_capacity(&self) -> usize {
        self.history.retained_capacity()
            + self.encoded.capacity()
            + self.pending.capacity()
            + self
                .huffman
                .as_ref()
                .map_or(0, HuffmanTable::retained_capacity)
            + self.sequences.retained_capacity()
            + self
                .active_dictionary
                .as_ref()
                .or(self.dictionary.as_ref())
                .or(self.prefix.as_ref())
                .map_or(0, |dictionary| dictionary.view().retained_capacity())
    }
    /// Initialized bytes currently available for backward references.
    pub fn history_size(&self) -> usize {
        self.history.available()
    }
    /// Active history window; zero between completed frames.
    pub fn window_size(&self) -> usize {
        self.history.window_size()
    }

    fn fill(&mut self, input: &[u8], consumed: &mut usize, target: usize) -> bool {
        if self.filled >= target {
            return true;
        }
        let count = (target - self.filled).min(input.len() - *consumed);
        self.scratch[self.filled..self.filled + count]
            .copy_from_slice(&input[*consumed..*consumed + count]);
        self.filled += count;
        *consumed += count;
        self.filled == target
    }

    fn process_inner(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        consumed: &mut usize,
        produced: &mut usize,
    ) -> Result<StreamStatus> {
        loop {
            match self.phase {
                Phase::Header => {
                    while self.filled < 4 {
                        if !self.fill(input, consumed, self.filled + 1) {
                            return Ok(StreamStatus::NeedInput);
                        }
                        let prefix = &self.scratch[..self.filled];
                        let standard = [0x28, 0xb5, 0x2f, 0xfd];
                        let skip = [self.scratch[0], 0x2a, 0x4d, 0x18];
                        if prefix != &standard[..self.filled]
                            && !((0x50..=0x5f).contains(&self.scratch[0])
                                && prefix == &skip[..self.filled])
                        {
                            return Err(Error::InvalidMagic);
                        }
                    }
                    if (0x50..=0x5f).contains(&self.scratch[0]) {
                        self.phase = Phase::SkipLength;
                        continue;
                    }
                    if !self.fill(input, consumed, 5) {
                        return Ok(StreamStatus::NeedInput);
                    }
                    let descriptor = self.scratch[4];
                    if descriptor & 8 != 0 {
                        return Err(Error::CorruptData);
                    }
                    let single = descriptor & 32 != 0;
                    let size_bytes = match descriptor >> 6 {
                        0 => usize::from(single),
                        1 => 2,
                        2 => 4,
                        _ => 8,
                    };
                    let size = 5
                        + usize::from(!single)
                        + [0, 1, 2, 4][usize::from(descriptor & 3)]
                        + size_bytes;
                    if !self.fill(input, consumed, size) {
                        return Ok(StreamStatus::NeedInput);
                    }
                    let (header, _) = FrameHeader::parse(&self.scratch[..size])?;
                    self.active_dictionary = self.prefix.take().or_else(|| self.dictionary.clone());
                    if header.dictionary_id != 0 {
                        let dictionary = self
                            .active_dictionary
                            .as_ref()
                            .ok_or(Error::DictionaryRequired)?;
                        if dictionary.view().id() != header.dictionary_id {
                            return Err(Error::DictionaryMismatch);
                        }
                    }
                    if header.window_size > self.options.max_window_size {
                        return Err(Error::ResourceLimit);
                    }
                    let budget = self.options.max_output_size as u64 - self.decoded_total;
                    if header.content_size.is_some_and(|size| size > budget) {
                        return Err(Error::ResourceLimit);
                    }
                    let window =
                        usize::try_from(header.window_size).map_err(|_| Error::ResourceLimit)?;
                    // No legal backward reference can exceed all output allowed
                    // in this session. Avoid reserving unreachable window bytes.
                    self.history.start_frame(window.min(budget as usize))?;
                    self.max_block = window.min(MAX_BLOCK_SIZE);
                    self.sequences.reset(
                        self.active_dictionary
                            .as_ref()
                            .and_then(|dictionary| dictionary.view().entropy())
                            .map_or([1, 4, 8], |entropy| entropy.repeat),
                    );
                    self.frame = Some(header);
                    self.checksum.reset();
                    self.filled = 0;
                    self.phase = Phase::BlockHeader;
                }
                Phase::BlockHeader => {
                    if !self.fill(input, consumed, 3) {
                        return Ok(StreamStatus::NeedInput);
                    }
                    let block = BlockHeader::read(
                        &mut ByteCursor::new(&self.scratch[..3]),
                        self.max_block,
                    )?;
                    self.filled = 0;
                    self.last_block = block.last;
                    self.remaining = block.size;
                    self.phase = match block.kind {
                        BlockKind::Raw => Phase::Raw,
                        BlockKind::Rle => Phase::RleByte,
                        BlockKind::Compressed => {
                            self.encoded.clear();
                            self.encoded
                                .try_reserve_exact(block.size)
                                .map_err(|_| Error::AllocationFailed)?;
                            Phase::Compressed
                        }
                    };
                }
                Phase::Raw | Phase::Rle => {
                    if self.remaining == 0 {
                        if self.end_block()? {
                            return Ok(StreamStatus::FrameFinished);
                        }
                        continue;
                    }
                    if self.phase == Phase::Raw && *consumed == input.len() {
                        return Ok(StreamStatus::NeedInput);
                    }
                    if *produced == output.len() {
                        return Ok(StreamStatus::NeedOutput);
                    }
                    let mut count = self.remaining.min(output.len() - *produced);
                    if self.phase == Phase::Raw {
                        count = count.min(input.len() - *consumed);
                    }
                    self.check_output(count)?;
                    let destination = &mut output[*produced..*produced + count];
                    if self.phase == Phase::Raw {
                        self.history
                            .append(&input[*consumed..*consumed + count], destination)?;
                        *consumed += count;
                    } else {
                        self.history.repeat(self.rle_byte, count, destination)?;
                    }
                    self.checksum.update(destination);
                    self.decoded_total += count as u64;
                    *produced += count;
                    self.remaining -= count;
                }
                Phase::Compressed => {
                    let count = self.remaining.min(input.len() - *consumed);
                    // BlockHeader reserved the full bounded encoded size;
                    // individual Copy pushes cannot allocate within that capacity.
                    for &byte in &input[*consumed..*consumed + count] {
                        self.encoded.push(byte);
                    }
                    *consumed += count;
                    self.remaining -= count;
                    if self.remaining != 0 {
                        return Ok(StreamStatus::NeedInput);
                    }
                    let budget =
                        (self.options.max_output_size as u64 - self.decoded_total) as usize;
                    let dictionary = self.active_dictionary.as_ref().map(DictionaryHandle::view);
                    let initial = dictionary.and_then(|dictionary| dictionary.entropy());
                    let (literals, offset) = decode_literals_with_dictionary_view_in(
                        &self.encoded,
                        &mut self.huffman,
                        self.max_block.min(budget),
                        initial.map(|entropy| entropy.huffman),
                        self.allocator.clone(),
                    )?;
                    self.pending.clear();
                    self.pending
                        .try_reserve_exact(self.max_block.min(budget))
                        .map_err(|_| Error::AllocationFailed)?;
                    let frame_remaining = self
                        .frame
                        .and_then(|frame| frame.content_size)
                        .map(|size| size.saturating_sub(self.history.frame_len()));
                    let mut sink = BlockSink {
                        history: &self.history,
                        dictionary,
                        window: self.frame.ok_or(Error::InvalidState)?.window_size,
                        output: &mut self.pending,
                        budget,
                        frame_remaining,
                        max_block: self.max_block,
                    };
                    self.sequences.decode_with_dictionary_view(
                        &self.encoded[offset..],
                        &literals,
                        &mut sink,
                        self.max_block,
                        initial,
                    )?;
                    self.history.commit(&self.pending)?;
                    self.decoded_total += self.pending.len() as u64;
                    self.checksum.update(&self.pending);
                    self.pending_position = 0;
                    self.phase = Phase::Drain;
                }
                Phase::Drain => {
                    let count =
                        (self.pending.len() - self.pending_position).min(output.len() - *produced);
                    output[*produced..*produced + count].copy_from_slice(
                        &self.pending[self.pending_position..self.pending_position + count],
                    );
                    self.pending_position += count;
                    *produced += count;
                    if self.pending_position != self.pending.len() {
                        return Ok(StreamStatus::NeedOutput);
                    }
                    self.pending.clear();
                    self.pending_position = 0;
                    if self.end_block()? {
                        return Ok(StreamStatus::FrameFinished);
                    }
                }
                Phase::RleByte => {
                    let Some(&byte) = input.get(*consumed) else {
                        return Ok(StreamStatus::NeedInput);
                    };
                    self.rle_byte = byte;
                    *consumed += 1;
                    self.phase = Phase::Rle;
                }
                Phase::SkipLength => {
                    if !self.fill(input, consumed, 8) {
                        return Ok(StreamStatus::NeedInput);
                    }
                    self.remaining = u32::from_le_bytes(
                        self.scratch[4..8]
                            .try_into()
                            .map_err(|_| Error::CorruptData)?,
                    ) as usize;
                    self.filled = 0;
                    self.phase = Phase::Skip;
                }
                Phase::Skip => {
                    let count = self.remaining.min(input.len() - *consumed);
                    *consumed += count;
                    self.remaining -= count;
                    if self.remaining != 0 {
                        return Ok(StreamStatus::NeedInput);
                    }
                    self.complete_frame()?;
                    return Ok(StreamStatus::FrameFinished);
                }
                Phase::Checksum => {
                    if !self.fill(input, consumed, 4) {
                        return Ok(StreamStatus::NeedInput);
                    }
                    let expected = u32::from_le_bytes(
                        self.scratch[..4]
                            .try_into()
                            .map_err(|_| Error::CorruptData)?,
                    );
                    if expected != self.checksum.digest() as u32 {
                        return Err(Error::ChecksumMismatch);
                    }
                    self.complete_frame()?;
                    return Ok(StreamStatus::FrameFinished);
                }
                Phase::Ended | Phase::Failed => return Err(Error::InvalidState),
            }
        }
    }

    fn check_output(&self, count: usize) -> Result<()> {
        let total = self
            .decoded_total
            .checked_add(count as u64)
            .ok_or(Error::ResourceLimit)?;
        if total > self.options.max_output_size as u64 {
            return Err(Error::ResourceLimit);
        }
        if self
            .frame
            .and_then(|frame| frame.content_size)
            .is_some_and(|size| self.history.frame_len().saturating_add(count as u64) > size)
        {
            return Err(Error::ContentSizeMismatch);
        }
        Ok(())
    }

    fn end_block(&mut self) -> Result<bool> {
        self.filled = 0;
        if !self.last_block {
            self.phase = Phase::BlockHeader;
            return Ok(false);
        }
        let frame = self.frame.ok_or(Error::InvalidState)?;
        if frame
            .content_size
            .is_some_and(|size| size != self.history.frame_len())
        {
            return Err(Error::ContentSizeMismatch);
        }
        if frame.has_checksum {
            self.phase = Phase::Checksum;
            Ok(false)
        } else {
            self.complete_frame()?;
            Ok(true)
        }
    }

    fn complete_frame(&mut self) -> Result<()> {
        self.completed_frames = self
            .completed_frames
            .checked_add(1)
            .ok_or(Error::ResourceLimit)?;
        self.phase = Phase::Header;
        self.filled = 0;
        self.frame = None;
        self.active_dictionary = None;
        self.history.clear();
        self.huffman = None;
        self.sequences.reset([1, 4, 8]);
        Ok(())
    }
}

/// A compressed block reads unchanged prior history plus its own pending bytes.
/// The complete block is committed only after sequence execution succeeds.
/// Keeping old history unchanged also preserves dictionary-to-frame sources.
/// Vector growth is checked before mutation and never exceeds one block.
struct BlockSink<'a, A: Allocator> {
    history: &'a RingHistory<A>,
    dictionary: Option<DictionaryView<'a>>,
    window: u64,
    output: &'a mut Vec<u8, A>,
    budget: usize,
    frame_remaining: Option<u64>,
    max_block: usize,
}

impl<A: Allocator> BlockSink<'_, A> {
    fn extend(&mut self, count: usize) -> Result<usize> {
        let start = self.output.len();
        let end = start.checked_add(count).ok_or(Error::ResourceLimit)?;
        if end > self.max_block {
            return Err(Error::CorruptData);
        }
        if end > self.budget {
            return Err(Error::ResourceLimit);
        }
        if self.frame_remaining.is_some_and(|size| end as u64 > size) {
            return Err(Error::ContentSizeMismatch);
        }
        self.output
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        // Initialization follows the successful reservation; each push stays
        // within capacity and cannot invoke another allocator request.
        for _ in start..end {
            self.output.push(0);
        }
        Ok(start)
    }

    /// Read already-produced frame data without changing the previous window.
    fn frame_byte(&self, position: u64, initialized: usize) -> Result<u8> {
        let base = self.history.frame_len();
        if position < base {
            self.history.byte_at(position)
        } else {
            let offset = usize::try_from(position - base).map_err(|_| Error::CorruptData)?;
            if offset >= initialized {
                return Err(Error::CorruptData);
            }
            self.output.get(offset).copied().ok_or(Error::CorruptData)
        }
    }
}

impl<A: Allocator> SequenceOutput for BlockSink<'_, A> {
    fn len(&self) -> usize {
        self.output.len()
    }

    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        let start = self.extend(bytes.len())?;
        self.output[start..].copy_from_slice(bytes);
        Ok(())
    }

    fn copy_match(&mut self, distance: usize, length: usize) -> Result<()> {
        let position = self
            .history
            .frame_len()
            .checked_add(self.output.len() as u64)
            .ok_or(Error::ResourceLimit)?;
        if distance == 0 {
            return Err(Error::CorruptData);
        }
        if distance as u64 <= position {
            if distance as u64 > self.window {
                return Err(Error::CorruptData);
            }
            let source = position - distance as u64;
            if length != 0 {
                self.frame_byte(source, self.output.len())?;
            }
            let start = self.extend(length)?;
            for offset in 0..length {
                let byte = self.frame_byte(source + offset as u64, start + offset)?;
                self.output[start + offset] = byte;
            }
        } else {
            // Dictionary content is a separate logical region ending at frame
            // position zero. Its initial distances may exceed Window_Size.
            // The format permits access while produced <= Window_Size; see
            // docs/dictionary-design.md for the reference assertion discrepancy.
            if position > self.window {
                return Err(Error::CorruptData);
            }
            let dictionary = self.dictionary.ok_or(Error::CorruptData)?;
            let before_frame =
                usize::try_from(distance as u64 - position).map_err(|_| Error::CorruptData)?;
            let source = dictionary
                .content()
                .len()
                .checked_sub(before_frame)
                .ok_or(Error::CorruptData)?;
            let start = self.extend(length)?;
            for offset in 0..length {
                let byte = if offset < before_frame {
                    dictionary.content_byte(source + offset)?
                } else {
                    self.frame_byte((offset - before_frame) as u64, start + offset)?
                };
                self.output[start + offset] = byte;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    // Fixture records contain only borrowed bytes/scalars; they own no codec storage.
    #[derive(Clone, Copy)]
    struct Case {
        dictionary: &'static [u8],
        encoded: &'static [u8],
        expected: &'static [u8],
    }

    macro_rules! fixture {
        ($name:literal) => {
            include_bytes!(concat!("../tests/upstream/dictionaries/", $name))
        };
    }

    fn prepared_cases() -> [Case; 9] {
        [
            Case {
                dictionary: fixture!("boundary-dictionary.raw"),
                encoded: fixture!("raw-small-window.zst"),
                expected: fixture!("raw-small-window.expected"),
            },
            Case {
                dictionary: fixture!("formatted.dict"),
                encoded: fixture!("formatted-id.zst"),
                expected: fixture!("formatted-id.expected"),
            },
            Case {
                dictionary: fixture!("formatted.dict"),
                encoded: fixture!("persistent-concatenation.zst"),
                expected: fixture!("persistent-concatenation.expected"),
            },
            Case {
                dictionary: fixture!("formatted.dict"),
                encoded: fixture!("dictionary-treeless-literals.zst"),
                expected: fixture!("dictionary-treeless-literals.expected"),
            },
            Case {
                dictionary: fixture!("formatted.dict"),
                encoded: fixture!("dictionary-treeless-four-streams.zst"),
                expected: fixture!("dictionary-treeless-four-streams.expected"),
            },
            Case {
                dictionary: fixture!("trained.dict"),
                encoded: fixture!("dictionary-repeat-sequences.zst"),
                expected: fixture!("dictionary-repeat-sequences.expected"),
            },
            Case {
                dictionary: fixture!("duplicate-reps.dict"),
                encoded: fixture!("duplicate-reps-initial.zst"),
                expected: fixture!("duplicate-reps-initial.expected"),
            },
            Case {
                dictionary: fixture!("boundary-dictionary.raw"),
                encoded: fixture!("dictionary-to-frame-eviction.zst"),
                expected: fixture!("dictionary-to-frame-eviction.expected"),
            },
            Case {
                dictionary: include_bytes!(
                    "../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols"
                ),
                encoded: include_bytes!("../tests/upstream/data/generated/http-dictionary.zst"),
                expected: include_bytes!("../tests/upstream/data/golden-compression/http"),
            },
        ]
    }

    fn tracking() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig::default()).unwrap()
    }

    /// Compare incremental output directly, without a hidden output allocation.
    /// Periodic empty input/output calls test state-only progress and draining.
    fn check_decode<A: Allocator + Clone, D: DictionaryHandle>(
        decoder: &mut Decoder<A, D>,
        encoded: &[u8],
        expected: &[u8],
        input_chunk: usize,
        output_chunk: usize,
    ) -> Result<usize> {
        assert!(input_chunk > 0 && (1..=129).contains(&output_chunk));
        let before_input = decoder.total_input();
        let before_output = decoder.total_output();
        let mut consumed = 0;
        let mut produced = 0;
        let mut frames = 0;
        for call in 0..16 * (encoded.len() + expected.len() + 1) {
            let end = if call % 5 == 0 {
                consumed
            } else {
                encoded.len().min(consumed + input_chunk)
            };
            let capacity = if call % 7 == 0 { 0 } else { output_chunk };
            let mut guarded = [0xa5; 131];
            let result = decoder.process(&encoded[consumed..end], &mut guarded[1..1 + capacity]);
            assert_eq!(guarded[0], 0xa5);
            assert!(guarded[1 + capacity..].iter().all(|&byte| byte == 0xa5));
            let progress = result?;
            assert!(progress.consumed <= end - consumed);
            assert!(progress.produced <= capacity);
            assert!(
                guarded[1 + progress.produced..]
                    .iter()
                    .all(|&byte| byte == 0xa5)
            );
            let next = produced + progress.produced;
            assert_eq!(
                &guarded[1..1 + progress.produced],
                expected.get(produced..next).unwrap()
            );
            consumed += progress.consumed;
            produced = next;
            if progress.status == StreamStatus::FrameFinished {
                frames += 1;
            }
            if consumed == encoded.len()
                && matches!(
                    progress.status,
                    StreamStatus::FrameFinished | StreamStatus::NeedInput
                )
            {
                decoder.finish()?;
                assert_eq!(produced, expected.len());
                assert_eq!(decoder.total_input() - before_input, encoded.len() as u64);
                assert_eq!(
                    decoder.total_output() - before_output,
                    expected.len() as u64
                );
                return Ok(frames);
            }
        }
        panic!("decoder failed to make bounded progress")
    }

    #[test]
    fn custom_decoder_construction_reset_and_window_changes_keep_exact_owners() {
        let owner = tracking();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let mut decoder = Decoder::new_in(DecodeOptions::new(3), owner);
        assert_eq!(monitor.snapshot().attempts, 0);
        assert_eq!(monitor.snapshot().identities_created, 5);
        assert_eq!(decoder.allocator.identity(), identity);
        assert_ne!(decoder.encoded.allocator().identity(), identity);
        assert_ne!(
            decoder.pending.allocator().identity(),
            decoder.encoded.allocator().identity()
        );
        let frame = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 25, 0, 0, b'a', b'b', b'c'];
        check_decode(&mut decoder, &frame, b"abc", 1, 1).unwrap();
        let capacity = decoder.retained_capacity();
        let before = monitor.snapshot();
        decoder.reset();
        assert_eq!(decoder.retained_capacity(), capacity);
        check_decode(&mut decoder, &frame, b"abc", 2, 3).unwrap();
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        assert_eq!(
            monitor.snapshot().identities_created,
            before.identities_created
        );
        assert!(monitor.fail_next());
        decoder.reset_with_options(DecodeOptions {
            max_output_size: 3,
            max_window_size: 1024,
        });
        assert_eq!(decoder.history.retained_capacity(), 0);
        assert_eq!(monitor.snapshot().live_blocks, 0);
        assert_eq!(
            monitor.snapshot().identities_created,
            before.identities_created + 1
        );
        assert_eq!(monitor.snapshot().owners_dropped, before.owners_dropped + 1);
        assert_eq!(
            check_decode(&mut decoder, &frame, b"abc", 2, 3),
            Err(Error::AllocationFailed)
        );
        assert_eq!(decoder.process(&[], &mut []), Err(Error::InvalidState));
        decoder.reset();
        check_decode(&mut decoder, &frame, b"abc", 2, 3).unwrap();
        assert_eq!(decoder.allocator.identity(), identity);
        drop(decoder);
        monitor.assert_released();
    }

    #[test]
    fn custom_decoder_borrows_all_prepared_fixture_tables_across_fragment_schedules() {
        for case in prepared_cases() {
            let dictionary_owner = tracking().into_non_clone();
            let dictionary_monitor = dictionary_owner.monitor();
            let dictionary = Dictionary::from_bytes_in(case.dictionary, &dictionary_owner).unwrap();
            let before = dictionary_monitor.snapshot();
            assert!(dictionary_monitor.fail_next());
            for (input_chunk, output_chunk) in [(1, 1), (2, 3), (17, 7), (128, 129)] {
                let owner = tracking().into_non_clone();
                let monitor = owner.monitor();
                let mut decoder = Decoder::with_dictionary_in(
                    DecodeOptions::new(case.expected.len()),
                    dictionary.view(),
                    &owner,
                );
                assert_eq!(decoder.retained_capacity(), dictionary.retained_capacity());
                let frames = check_decode(
                    &mut decoder,
                    case.encoded,
                    case.expected,
                    input_chunk,
                    output_chunk,
                )
                .unwrap();
                assert!(frames >= 1);
                decoder.reset();
                check_decode(
                    &mut decoder,
                    case.encoded,
                    case.expected,
                    input_chunk,
                    output_chunk,
                )
                .unwrap();
                assert_eq!(monitor.snapshot().identities_created, 1);
                drop(decoder);
                monitor.assert_released();
            }
            assert_eq!(dictionary_monitor.snapshot().attempts, before.attempts);
            assert_eq!(
                dictionary_monitor.snapshot().identities_created,
                before.identities_created
            );
            drop(dictionary);
            dictionary_monitor.assert_released();
        }
    }

    #[test]
    fn custom_decoder_shared_dictionary_keeps_its_distinct_allocator_alive() {
        let case = prepared_cases()[4];
        let dictionary_owner = tracking();
        let dictionary_monitor = dictionary_owner.monitor();
        let shared = Dictionary::from_bytes_in(case.dictionary, dictionary_owner)
            .unwrap()
            .into_shared();
        let dictionary_before = dictionary_monitor.snapshot();
        let weak = SharedDictionary::downgrade(&shared);
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let mut decoder =
            Decoder::with_dictionary_in(DecodeOptions::new(case.expected.len()), shared, &owner);
        assert!(weak.upgrade().is_some());
        check_decode(&mut decoder, case.encoded, case.expected, 3, 7).unwrap();
        decoder.reset();
        check_decode(&mut decoder, case.encoded, case.expected, 17, 129).unwrap();
        assert_eq!(
            dictionary_monitor.snapshot().attempts,
            dictionary_before.attempts
        );
        assert_eq!(
            dictionary_monitor.snapshot().identities_created,
            dictionary_before.identities_created
        );
        decoder.reset();
        decoder.set_dictionary(None).unwrap();
        assert!(weak.upgrade().is_none());
        dictionary_monitor.assert_released();
        drop(decoder);
        monitor.assert_released();
    }

    #[test]
    fn custom_decoder_every_observed_allocation_failure_poison_resets_and_releases() {
        let plain = Case {
            dictionary: b"",
            encoded: include_bytes!("../tests/upstream/data/generated/http.zst"),
            expected: include_bytes!("../tests/upstream/data/golden-compression/http"),
        };
        for case in [
            plain,
            prepared_cases()[1],
            prepared_cases()[4],
            prepared_cases()[5],
        ] {
            let dictionary = Dictionary::from_bytes(case.dictionary).unwrap();
            let owner = tracking();
            let monitor = owner.monitor();
            let mut baseline = Decoder::with_dictionary_in(
                DecodeOptions::new(case.expected.len()),
                dictionary.view(),
                owner,
            );
            check_decode(&mut baseline, case.encoded, case.expected, 17, 129).unwrap();
            let attempts = monitor.snapshot().attempts;
            assert!(attempts >= 3);
            drop(baseline);
            monitor.assert_released();
            for ordinal in 1..=attempts {
                let owner = TrackingAllocator::new(AllocatorConfig {
                    fail_at: NonZeroU64::new(ordinal),
                    ..AllocatorConfig::default()
                })
                .unwrap();
                let monitor = owner.monitor();
                let mut decoder = Decoder::with_dictionary_in(
                    DecodeOptions::new(case.expected.len()),
                    dictionary.view(),
                    owner,
                );
                assert_eq!(
                    check_decode(&mut decoder, case.encoded, case.expected, 17, 129),
                    Err(Error::AllocationFailed),
                    "request {ordinal}"
                );
                assert_eq!(monitor.snapshot().attempts, ordinal);
                assert_eq!(monitor.snapshot().failures, 1);
                assert_eq!(decoder.process(&[], &mut []), Err(Error::InvalidState));
                assert_eq!(decoder.finish(), Err(Error::InvalidState));
                assert_eq!(decoder.set_dictionary(None), Err(Error::InvalidState));
                decoder.reset();
                assert_eq!(decoder.total_input(), 0);
                assert_eq!(decoder.total_output(), 0);
                assert!(decoder.active_dictionary.is_none());
                assert!(decoder.dictionary.is_some());
                check_decode(&mut decoder, case.encoded, case.expected, 17, 129).unwrap();
                drop(decoder);
                monitor.assert_released();
            }
        }
    }

    #[test]
    fn custom_decoder_small_failure_sweep_preserves_shared_owner_and_recovers() {
        // Three raw literals and one RLE-coded sequence: the complete six-byte
        // output is abcabc. This bounded frame isolates the four workspace
        // allocations without external fixture or filesystem dependencies.
        let frame = [
            0x28, 0xb5, 0x2f, 0xfd, 0, 0, 85, 0, 0, 24, b'a', b'b', b'c', 1, 0x54, 3, 2, 0, 6,
        ];
        let dictionary_owner = tracking().into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let dictionary = Dictionary::raw_in(b"unused", dictionary_owner)
            .unwrap()
            .into_shared();
        let weak = SharedDictionary::downgrade(&dictionary);
        for ordinal in 1..=4 {
            let owner = TrackingAllocator::new(AllocatorConfig {
                fail_at: NonZeroU64::new(ordinal),
                ..AllocatorConfig::default()
            })
            .unwrap();
            let monitor = owner.monitor();
            let mut decoder =
                Decoder::with_dictionary_in(DecodeOptions::new(6), dictionary.clone(), owner);
            assert_eq!(
                check_decode(&mut decoder, &frame, b"abcabc", 3, 7),
                Err(Error::AllocationFailed)
            );
            assert_eq!(monitor.snapshot().attempts, ordinal);
            assert_eq!(decoder.finish(), Err(Error::InvalidState));
            decoder.reset();
            assert!(weak.upgrade().is_some());
            check_decode(&mut decoder, &frame, b"abcabc", 3, 7).unwrap();
            drop(decoder);
            monitor.assert_released();
        }
        drop(dictionary);
        assert!(weak.upgrade().is_none());
        dictionary_monitor.assert_released();
    }

    #[test]
    fn custom_decoder_prefix_claim_failure_and_skippable_frames_preserve_lifetimes() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let dictionary_owner = tracking();
        let dictionary_monitor = dictionary_owner.monitor();
        let shared = Dictionary::raw_in(fixture!("boundary-dictionary.raw"), dictionary_owner)
            .unwrap()
            .into_shared();
        let weak = SharedDictionary::downgrade(&shared);
        let mut decoder =
            Decoder::with_dictionary_in(DecodeOptions::new(512), shared.clone(), &owner);
        decoder.set_prefix(shared).unwrap();
        decoder.reset();
        let skip = [0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0];
        assert_eq!(
            decoder.process(&skip, &mut []).unwrap().status,
            StreamStatus::FrameFinished
        );
        assert!(decoder.prefix.is_some());
        assert!(monitor.fail_next());
        assert_eq!(
            decoder.process(fixture!("prefix-first.zst"), &mut []),
            Err(Error::AllocationFailed)
        );
        assert!(decoder.prefix.is_none());
        assert!(decoder.active_dictionary.is_some());
        assert!(weak.upgrade().is_some());
        decoder.reset();
        assert!(weak.upgrade().is_none());
        dictionary_monitor.assert_released();
        assert!(decoder.dictionary.is_none());
        assert!(decoder.prefix.is_none());
        drop(decoder);
        monitor.assert_released();

        let dictionary_owner = tracking().into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let dictionary =
            Dictionary::raw_in(fixture!("boundary-dictionary.raw"), &dictionary_owner).unwrap();
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let mut decoder =
            Decoder::with_dictionary_in(DecodeOptions::new(512), dictionary.view(), &owner);
        decoder.set_prefix(dictionary.view()).unwrap();
        check_decode(
            &mut decoder,
            fixture!("prefix-once-concatenation.zst"),
            fixture!("prefix-once-concatenation.expected"),
            1,
            7,
        )
        .unwrap();
        assert!(
            decoder.prefix.is_none()
                && decoder.active_dictionary.is_none()
                && decoder.dictionary.is_none()
        );
        drop(decoder);
        monitor.assert_released();
        drop(dictionary);
        dictionary_monitor.assert_released();
    }

    #[test]
    fn custom_decoder_resource_checks_precede_allocation_and_errors_require_reset() {
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let mut decoder = Decoder::new_in(
            DecodeOptions {
                max_output_size: 2,
                max_window_size: 2,
            },
            &owner,
        );
        let raw = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 25, 0, 0, b'a', b'b', b'c'];
        assert!(monitor.fail_next());
        assert_eq!(
            decoder.process(&raw, &mut [0; 3]),
            Err(Error::ResourceLimit)
        );
        assert_eq!(monitor.snapshot().attempts, 0);
        decoder.reset_with_options(DecodeOptions::new(3));
        assert_eq!(
            decoder.process(&raw, &mut [0; 3]),
            Err(Error::AllocationFailed)
        );
        decoder.reset();
        check_decode(&mut decoder, &raw, b"abc", 1, 1).unwrap();
        decoder.reset();
        assert_eq!(decoder.process(&[0xff], &mut []), Err(Error::InvalidMagic));
        assert_eq!(decoder.process(&raw, &mut [0; 3]), Err(Error::InvalidState));
        decoder.reset();
        decoder.process(&raw[..5], &mut []).unwrap();
        assert_eq!(decoder.finish(), Err(Error::UnexpectedEof));
        assert_eq!(decoder.process(&raw, &mut [0; 3]), Err(Error::InvalidState));
        decoder.reset();
        check_decode(&mut decoder, &raw, b"abc", 17, 7).unwrap();
        drop(decoder);
        monitor.assert_released();
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "requires official zstd v1.5.7 CLI; set ZSTD_BIN"]
    fn official_frames_decode_through_custom_shared_and_borrowed_allocators() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let executable =
            std::env::var_os("ZSTD_BIN").expect("set ZSTD_BIN to the pinned v1.5.7 CLI");
        let version = Command::new(&executable).arg("--version").output().unwrap();
        assert!(version.status.success());
        assert!(std::string::String::from_utf8_lossy(&version.stdout).contains("v1.5.7"));
        let dictionary_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/upstream/dictionaries/formatted.dict");
        let mut checked = 0;
        for dictionary_enabled in [false, true] {
            for length in [0, 1, 4096, 8193] {
                let source: Vec<u8> = (0..length)
                    .map(|index| ((index * 79) ^ (index >> 4)) as u8 & 31)
                    .collect();
                let mut command = Command::new(&executable);
                command.args(["-q", "-f", "-c", "--check", "-3"]);
                if dictionary_enabled {
                    command.arg("-D").arg(&dictionary_path);
                }
                let mut child = command
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let mut input = child.stdin.take().unwrap();
                let sent = source.clone();
                let writer = std::thread::spawn(move || input.write_all(&sent));
                let output = child.wait_with_output().unwrap();
                writer.join().unwrap().unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    std::string::String::from_utf8_lossy(&output.stderr)
                );
                let dictionary_bytes: &[u8] = if dictionary_enabled {
                    fixture!("formatted.dict")
                } else {
                    b""
                };
                let dictionary_owner = tracking();
                let dictionary_monitor = dictionary_owner.monitor();
                let dictionary = Dictionary::from_bytes_in(dictionary_bytes, dictionary_owner)
                    .unwrap()
                    .into_shared();
                let before = dictionary_monitor.snapshot();
                let owner = tracking().into_non_clone();
                let monitor = owner.monitor();
                let mut borrowed = Decoder::with_dictionary_in(
                    DecodeOptions::new(source.len()),
                    dictionary.view(),
                    &owner,
                );
                check_decode(&mut borrowed, &output.stdout, &source, 3, 7).unwrap();
                drop(borrowed);
                monitor.assert_released();
                let mut shared = Decoder::with_dictionary_in(
                    DecodeOptions::new(source.len()),
                    dictionary.clone(),
                    &owner,
                );
                check_decode(&mut shared, &output.stdout, &source, 17, 129).unwrap();
                drop(shared);
                monitor.assert_released();
                assert_eq!(dictionary_monitor.snapshot().attempts, before.attempts);
                drop(dictionary);
                dictionary_monitor.assert_released();
                checked += 2;
            }
        }
        std::eprintln!(
            "Decoded 8 official checksum frames through 16 custom-allocator borrowed/shared contexts; no dictionary table copies"
        );
        assert_eq!(checked, 16);
    }

    #[test]
    fn dictionary_configuration_is_transactional_and_prefix_claims_only_standard_frames() {
        let dictionary = Dictionary::raw(b"dictionary").unwrap().into_shared();
        let mut decoder = Decoder::with_dictionary(DecodeOptions::new(0), dictionary.clone());
        decoder.process(&[0x28], &mut []).unwrap();
        assert_eq!(decoder.set_dictionary(None), Err(Error::InvalidState));
        assert_eq!(
            decoder.set_prefix(dictionary.clone()),
            Err(Error::InvalidState)
        );
        assert!(decoder.dictionary.is_some());
        assert_eq!(decoder.total_input(), 1);
        decoder.reset();
        assert!(decoder.dictionary.is_some());
        decoder.set_prefix(dictionary).unwrap();
        decoder.reset();
        assert!(decoder.prefix.is_some());
        assert!(decoder.dictionary.is_none());
        let skip = [0x50, 0x2a, 0x4d, 0x18, 0, 0, 0, 0];
        decoder.process(&skip, &mut []).unwrap();
        assert!(decoder.prefix.is_some());
        decoder
            .process(&[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0], &mut [])
            .unwrap();
        assert!(decoder.prefix.is_none());
        assert!(decoder.active_dictionary.is_some());
        decoder.reset();
        assert!(decoder.prefix.is_none());
        assert!(decoder.active_dictionary.is_none());
        decoder.process(&skip, &mut []).unwrap();
        decoder.finish().unwrap();
        assert_eq!(decoder.set_dictionary(None), Err(Error::InvalidState));
    }

    #[test]
    fn explicit_dictionary_ids_are_checked_before_block_processing() {
        let dictionary = Dictionary::from_bytes(include_bytes!(
            "../tests/upstream/data/golden-dictionaries/http-dict-missing-symbols"
        ))
        .unwrap()
        .into_shared();
        let mut frame = alloc::vec![0x28, 0xb5, 0x2f, 0xfd, 0x23];
        frame.extend_from_slice(&dictionary.id().to_le_bytes());
        frame.extend_from_slice(&[0, 1, 0, 0]);
        let mut decoder = Decoder::new(DecodeOptions::new(0));
        assert_eq!(
            decoder.process(&frame, &mut []),
            Err(Error::DictionaryRequired)
        );
        decoder.reset();
        decoder
            .set_dictionary(Some(Dictionary::raw(b"wrong").unwrap().into_shared()))
            .unwrap();
        assert_eq!(
            decoder.process(&frame, &mut []),
            Err(Error::DictionaryMismatch)
        );
        decoder.reset();
        decoder.set_dictionary(Some(dictionary)).unwrap();
        assert_eq!(
            decoder.process(&frame, &mut []).unwrap().status,
            StreamStatus::FrameFinished
        );
    }

    #[test]
    fn headers_raw_and_rle_resume_at_single_byte_boundaries() {
        for (encoded, expected) in [
            (
                &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 25, 0, 0, b'a', b'b', b'c'][..],
                &b"abc"[..],
            ),
            (
                &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 3, 27, 0, 0, b'x'][..],
                &b"xxx"[..],
            ),
        ] {
            let mut decoder = Decoder::new(DecodeOptions::new(3));
            let mut position = 0;
            let mut actual = Vec::new();
            loop {
                let mut output = [0; 1];
                let end = (position + 1).min(encoded.len());
                let progress = decoder
                    .process(&encoded[position..end], &mut output)
                    .unwrap();
                position += progress.consumed;
                actual.extend_from_slice(&output[..progress.produced]);
                if progress.status == StreamStatus::FrameFinished {
                    break;
                }
            }
            decoder.finish().unwrap();
            assert_eq!(actual, expected);
            assert_eq!(position, encoded.len());
        }
    }
    #[test]
    fn skip_does_not_consume_the_following_prefix() {
        let mut decoder = Decoder::new(DecodeOptions::new(0));
        let progress = decoder
            .process(&[0x5f, 0x2a, 0x4d, 0x18, 1, 0, 0, 0, 42, 0xff], &mut [])
            .unwrap();
        assert_eq!(
            progress,
            Progress {
                consumed: 9,
                produced: 0,
                status: StreamStatus::FrameFinished
            }
        );
        decoder.finish().unwrap();
    }

    #[test]
    fn every_partial_header_and_checksum_split_is_resumable() {
        // Non-single-segment frame with an eight-byte content size and checksum.
        let mut frame = alloc::vec![0x28, 0xb5, 0x2f, 0xfd, 0xc4, 0];
        frame.extend_from_slice(&3_u64.to_le_bytes());
        frame.extend_from_slice(&[25, 0, 0, b'a', b'b', b'c']);
        frame.extend_from_slice(&(crate::xxhash::checksum(b"abc") as u32).to_le_bytes());
        for split in 0..frame.len() {
            let mut decoder = Decoder::new(DecodeOptions::new(3));
            let mut first = [0; 3];
            let before = decoder.process(&frame[..split], &mut first).unwrap();
            assert_eq!(before.consumed, split);
            let mut second = [0; 3];
            let after = decoder.process(&frame[split..], &mut second).unwrap();
            assert_eq!(after.status, StreamStatus::FrameFinished);
            let actual = [&first[..before.produced], &second[..after.produced]].concat();
            assert_eq!(actual, b"abc");
            decoder.finish().unwrap();
        }
    }

    #[test]
    fn processing_after_eof_poisoning_requires_reset() {
        let empty = [0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0];
        let mut decoder = Decoder::new(DecodeOptions::new(0));
        decoder.process(&empty, &mut []).unwrap();
        decoder.finish().unwrap();
        decoder.finish().unwrap();
        assert_eq!(decoder.process(&[], &mut []), Err(Error::InvalidState));
        assert_eq!(decoder.finish(), Err(Error::InvalidState));
        decoder.reset();
        assert_eq!(
            decoder.process(&empty, &mut []).unwrap().status,
            StreamStatus::FrameFinished
        );
        decoder.finish().unwrap();
    }
}
