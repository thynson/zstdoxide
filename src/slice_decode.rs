//! Fixed-destination incremental decoding with borrowed frame-local history.

use crate::cursor::ByteCursor;
use crate::frame::{BlockHeader, BlockKind, MAX_BLOCK_SIZE};
use crate::history::SequenceOutput;
use crate::huffman::{HuffmanTable, decode_literals_in};
use crate::sequences::SequenceState;
use crate::slice_history::SliceHistory;
use crate::xxhash::checksum;
use crate::{DecodeOptions, Error, FrameHeader, Progress, Result, StreamStatus};
use alloc::alloc::{Allocator, Global};
use alloc::vec::Vec;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Header,
    BlockHeader,
    Raw,
    RleByte,
    Rle,
    Compressed,
    SkipLength,
    Skip,
    Checksum,
    Ended,
    Failed,
}

/// Temporary borrowed output coordinates; neither owner is moved or retained.
#[cfg(test)]
struct DictionaryOutput<'history, 'output, 'dictionary> {
    history: &'history mut SliceHistory<'output>,
    dictionary: crate::DictionaryView<'dictionary>,
}

#[cfg(test)]
impl SequenceOutput for DictionaryOutput<'_, '_, '_> {
    fn len(&self) -> usize {
        self.history.len()
    }
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        self.history.append(bytes)
    }
    fn copy_match(&mut self, offset: usize, length: usize) -> Result<()> {
        self.history
            .copy_dictionary_match(self.dictionary, offset, length)
    }
}

/// Incrementally decode into one exclusively borrowed, fixed-capacity destination.
///
/// The slice remains borrowed for the session and supplies all frame-local match
/// history. Input fragments are not retained. Compressed blocks can allocate
/// fallible input, literal and entropy scratch, but no output vector or history
/// copy. Standard, skippable and concatenated frames are supported; dictionary
/// IDs are rejected. Corresponds to upstream `ZSTD_decompressStream` with
/// `ZSTD_d_stableOutBuffer`.
///
/// Output is provisional until its frame passes content-size and checksum
/// validation. Errors poison the session until reset and may leave partial bytes
/// within the effective allowance; bytes beyond that allowance stay unchanged.
/// Safe borrowing prevents replacement or mutation of the destination between
/// calls. Drop the decoder to regain exclusive access to the complete slice.
///
/// ```
/// use zstdoxide::{compress, DecodeOptions, SliceDecoder};
/// let frame = compress(b"hello")?;
/// let mut output = [0xa5; 16];
/// let mut decoder = SliceDecoder::new(&mut output, DecodeOptions::new(5));
/// for fragment in frame.chunks(3) {
///     let progress = decoder.process(fragment)?;
///     assert_eq!(progress.consumed, fragment.len());
/// }
/// decoder.finish()?;
/// assert_eq!(decoder.output()?, b"hello");
/// drop(decoder);
/// assert_eq!(&output[5..], &[0xa5; 11]);
/// # Ok::<(), zstdoxide::Error>(())
/// ```
///
/// The destination cannot be modified while the session still uses its borrow:
///
/// ```compile_fail,E0506
/// use zstdoxide::{DecodeOptions, SliceDecoder};
/// let mut output = [0_u8; 32];
/// let mut decoder = SliceDecoder::new(&mut output, DecodeOptions::new(32));
/// output[0] = 1;
/// decoder.process(&[]).unwrap();
/// ```
///
/// A view of written history cannot remain in use across a mutating call:
///
/// ```compile_fail,E0502
/// use zstdoxide::{DecodeOptions, SliceDecoder};
/// let mut output = [0_u8; 32];
/// let mut decoder = SliceDecoder::new(&mut output, DecodeOptions::new(32));
/// let written = decoder.output().unwrap();
/// decoder.reset();
/// assert!(written.is_empty());
/// ```
pub struct SliceDecoder<'output, A: Allocator = Global> {
    allocator: A,
    options: DecodeOptions,
    history: SliceHistory<'output>,
    phase: Phase,
    scratch: [u8; 18],
    filled: usize,
    frame: Option<FrameHeader>,
    max_block: usize,
    last_block: bool,
    remaining: usize,
    rle_byte: u8,
    encoded: Vec<u8, A>,
    huffman: Option<HuffmanTable<A>>,
    sequences: SequenceState<A>,
    input_total: u64,
    completed_frames: u64,
}

impl<'output> SliceDecoder<'output> {
    /// Borrow a destination and construct empty codec owners without allocation.
    /// The output allowance is the smaller of capacity and `max_output_size`;
    /// the advertised-window limit remains independent.
    pub fn new(output: &'output mut [u8], options: DecodeOptions) -> Self {
        Self::new_in(output, options, Global)
    }
}

impl<'output, A: Allocator + Clone> SliceDecoder<'output, A> {
    /// Construct scratch owners using the supplied allocator or borrowed owner.
    /// The destination always belongs to the caller. Allocator cloning retains
    /// the allocator implementation's own allocation and panic semantics.
    pub fn new_in(output: &'output mut [u8], mut options: DecodeOptions, allocator: A) -> Self {
        options.max_output_size = options.max_output_size.min(output.len());
        Self {
            options,
            history: SliceHistory::new(output, options.max_output_size),
            phase: Phase::Header,
            scratch: [0; 18],
            filled: 0,
            frame: None,
            max_block: 0,
            last_block: false,
            remaining: 0,
            rle_byte: 0,
            encoded: Vec::new_in(allocator.clone()),
            huffman: None,
            sequences: SequenceState::new_in(allocator.clone()),
            allocator,
            input_total: 0,
            completed_frames: 0,
        }
    }

    /// Accept a temporary input fragment and write directly into the destination.
    ///
    /// Resubmit only the unconsumed input suffix. Returns `NeedInput` or
    /// `FrameFinished`, stopping before the next frame's magic. Insufficient
    /// fixed output is `ResourceLimit`; this API never returns `NeedOutput`.
    /// Empty input can finish a pending zero-sized block and otherwise requests
    /// more input. An error returns no usable slice progress and requires reset.
    pub fn process(&mut self, input: &[u8]) -> Result<Progress> {
        if matches!(self.phase, Phase::Ended | Phase::Failed) {
            self.phase = Phase::Failed;
            return Err(Error::InvalidState);
        }
        let mut consumed = 0;
        let before = self.history.len();
        let result = self.process_inner(input, &mut consumed);
        let result = match self.input_total.checked_add(consumed as u64) {
            Some(total) => {
                self.input_total = total;
                result
            }
            None => Err(Error::ResourceLimit),
        };
        match result {
            Ok(status) => Ok(Progress {
                consumed,
                produced: self.history.len() - before,
                status,
            }),
            Err(error) => {
                self.phase = Phase::Failed;
                Err(error)
            }
        }
    }

    /// Seal a clean boundary after at least one complete standard/skippable frame.
    /// Empty or partial streams fail with `UnexpectedEof` and poison the session.
    /// Repeated successful finish is allowed; processing again requires reset.
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

    /// Borrow the written prefix, excluding untouched destination bytes.
    /// Bytes of an unfinished frame remain provisional. Failed sessions return
    /// `InvalidState`; a borrowed view prevents mutation of the session while used.
    pub fn output(&self) -> Result<&[u8]> {
        if self.phase == Phase::Failed {
            return Err(Error::InvalidState);
        }
        Ok(self.history.written())
    }

    /// Restart the same destination and limits, retaining reusable scratch.
    /// Resets output position without erasing bytes; old bytes cease to be output
    /// or valid match history. Releases frame-local Huffman tables.
    pub fn reset(&mut self) {
        self.history.reset();
        self.phase = Phase::Header;
        self.filled = 0;
        self.frame = None;
        self.max_block = 0;
        self.last_block = false;
        self.remaining = 0;
        self.rle_byte = 0;
        self.encoded.clear();
        self.huffman = None;
        self.sequences.reset([1, 4, 8]);
        self.input_total = 0;
        self.completed_frames = 0;
    }

    /// Effective limits, with output allowance clamped to destination capacity.
    pub fn options(&self) -> DecodeOptions {
        self.options
    }

    /// Accepted input since reset, including progress in a failing call.
    pub fn total_input(&self) -> u64 {
        self.input_total
    }

    /// Written bytes since reset, including provisional writes in a failing call.
    pub fn total_output(&self) -> u64 {
        self.history.len() as u64
    }

    /// Codec-owned heap capacity, excluding inline state and the borrowed output.
    pub fn retained_capacity(&self) -> usize {
        self.encoded.capacity()
            + self
                .huffman
                .as_ref()
                .map_or(0, HuffmanTable::retained_capacity)
            + self.sequences.retained_capacity()
    }

    fn fill(&mut self, input: &[u8], consumed: &mut usize, target: usize) -> bool {
        let count = (target - self.filled).min(input.len() - *consumed);
        self.scratch[self.filled..self.filled + count]
            .copy_from_slice(&input[*consumed..*consumed + count]);
        self.filled += count;
        *consumed += count;
        self.filled == target
    }

    fn process_inner(&mut self, input: &[u8], consumed: &mut usize) -> Result<StreamStatus> {
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
                    // A previously buffered descriptor/header may exceed five bytes.
                    if self.filled < 5 && !self.fill(input, consumed, 5) {
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
                    if header.dictionary_id != 0 {
                        return Err(Error::DictionaryRequired);
                    }
                    if header.window_size > self.options.max_window_size {
                        return Err(Error::ResourceLimit);
                    }
                    let budget = self.options.max_output_size - self.history.len();
                    if header.content_size.is_some_and(|size| size > budget as u64) {
                        return Err(Error::ResourceLimit);
                    }
                    let window =
                        usize::try_from(header.window_size).map_err(|_| Error::ResourceLimit)?;
                    self.history.start_frame(window);
                    self.max_block = window.min(MAX_BLOCK_SIZE);
                    self.frame = Some(header);
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
                            Phase::Compressed
                        }
                    };
                }
                Phase::Raw => {
                    let count = self.remaining.min(input.len() - *consumed);
                    self.history.append(&input[*consumed..*consumed + count])?;
                    *consumed += count;
                    self.remaining -= count;
                    if self.remaining != 0 {
                        return Ok(StreamStatus::NeedInput);
                    }
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
                Phase::Rle => {
                    self.history.repeat(self.rle_byte, self.remaining)?;
                    self.remaining = 0;
                    if self.end_block()? {
                        return Ok(StreamStatus::FrameFinished);
                    }
                }
                Phase::Compressed => {
                    let count = self.remaining.min(input.len() - *consumed);
                    if count == 0 && self.remaining != 0 {
                        return Ok(StreamStatus::NeedInput);
                    }
                    let payload = &input[*consumed..*consumed + count];
                    let direct = self.encoded.is_empty() && count == self.remaining;
                    if !direct {
                        self.encoded
                            .try_reserve_exact(self.remaining)
                            .map_err(|_| Error::AllocationFailed)?;
                        // Reservation covers the complete remaining payload; copying
                        // these bytes cannot grow beyond that fallible reservation.
                        self.encoded.extend_from_slice(payload);
                    }
                    *consumed += count;
                    self.remaining -= count;
                    if self.remaining != 0 {
                        return Ok(StreamStatus::NeedInput);
                    }
                    let encoded = if direct { payload } else { &self.encoded };
                    let literal_limit = self
                        .max_block
                        .min(self.options.max_output_size - self.history.len());
                    let (literals, offset) = decode_literals_in(
                        encoded,
                        &mut self.huffman,
                        literal_limit,
                        self.allocator.clone(),
                    )?;
                    self.sequences.decode(
                        &encoded[offset..],
                        &literals,
                        &mut self.history,
                        self.max_block,
                    )?;
                    self.encoded.clear();
                    if self.end_block()? {
                        return Ok(StreamStatus::FrameFinished);
                    }
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
                    if expected != checksum(self.history.frame_bytes()) as u32 {
                        return Err(Error::ChecksumMismatch);
                    }
                    self.complete_frame()?;
                    return Ok(StreamStatus::FrameFinished);
                }
                Phase::Ended | Phase::Failed => return Err(Error::InvalidState),
            }
        }
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
            .is_some_and(|size| size != self.history.frame_bytes().len() as u64)
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
        self.history.start_frame(0);
        self.huffman = None;
        self.sequences.reset([1, 4, 8]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &[u8] = &[0x28, 0xb5, 0x2f, 0xfd, 0x20, 0, 1, 0, 0];

    #[test]
    fn dictionary_sink_preserves_frame_coordinates_and_forward_crossing() {
        let dictionary = crate::Dictionary::raw(b"dictionary").unwrap();
        let mut output = [0xa5; 64];
        let mut history = SliceHistory::new(&mut output[1..63], 32);
        history.append(b"old").unwrap();
        history.start_frame(2);
        history.append(b"ab").unwrap();
        let mut logical = b"dictionaryab".to_vec();
        for _ in 0..12 {
            logical.push(logical[logical.len() - 6]);
        }
        {
            let mut sink = DictionaryOutput {
                history: &mut history,
                dictionary: dictionary.view(),
            };
            assert_eq!(sink.len(), 5);
            sink.copy_match(6, 12).unwrap();
            assert_eq!(sink.len(), 17);
        }
        assert_eq!(history.frame_bytes(), &logical[10..]);
        history.append(b"!").unwrap();
        assert_eq!(&output[1..4], b"old");
        assert_eq!(&output[4..18], &logical[10..]);
        assert_eq!(output[18], b'!');
        assert_eq!(output[0], 0xa5);
        assert!(output[19..].iter().all(|&b| b == 0xa5));
    }

    #[test]
    fn dictionary_sink_rejects_capacity_range_and_expired_history_without_writes() {
        let dictionary = crate::Dictionary::raw(b"dict").unwrap();
        for (distance, length, window, expected) in [
            (0, 0, 2, Error::CorruptData),
            (7, 1, 2, Error::CorruptData),
            (3, 1, 1, Error::CorruptData),
            (6, usize::MAX, 2, Error::ResourceLimit),
        ] {
            let mut output = [0xa5; 16];
            let mut history = SliceHistory::new(&mut output[1..15], 8);
            history.start_frame(window);
            history.append(b"ab").unwrap();
            {
                let mut sink = DictionaryOutput {
                    history: &mut history,
                    dictionary: dictionary.view(),
                };
                assert_eq!(sink.copy_match(distance, length), Err(expected));
                assert_eq!(sink.len(), 2);
            }
            assert_eq!(history.frame_bytes(), b"ab");
            assert_eq!(output[0], 0xa5);
            assert!(output[3..].iter().all(|&b| b == 0xa5));
        }
    }

    #[test]
    fn exhausted_counters_reject_without_wrapping_or_allowing_continuation() {
        let mut output = [];
        let mut decoder = SliceDecoder::new(&mut output, DecodeOptions::new(0));
        decoder.input_total = u64::MAX;
        assert_eq!(decoder.process(EMPTY), Err(Error::ResourceLimit));
        assert_eq!(decoder.input_total, u64::MAX);
        assert_eq!(decoder.process(EMPTY), Err(Error::InvalidState));
        decoder.reset();
        decoder.completed_frames = u64::MAX;
        assert_eq!(decoder.process(EMPTY), Err(Error::ResourceLimit));
        assert_eq!(decoder.completed_frames, u64::MAX);
        decoder.reset();
        assert_eq!(
            decoder.process(EMPTY).unwrap().status,
            StreamStatus::FrameFinished
        );
        decoder.finish().unwrap();
    }
}
