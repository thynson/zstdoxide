//! Bounded frame state machine with caller-owned output and indexed history.

use crate::cursor::ByteCursor;
use crate::frame::{BlockHeader, BlockKind, MAX_BLOCK_SIZE};
use crate::ring_history::RingHistory;
use crate::xxhash::XxHash64;
use crate::{DecodeOptions, Error, FrameHeader, Progress, Result, StreamStatus};
#[cfg(test)]
use alloc::vec::Vec;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Header,
    BlockHeader,
    Raw,
    RleByte,
    Rle,
    SkipLength,
    Skip,
    Checksum,
    Ended,
    Failed,
}

/// Incrementally decode concatenated standard and skippable frames.
///
/// The context owns its bounded window and block storage, never caller slices.
/// Every frame boundary is reported before consuming the next frame's magic.
/// Output is provisional until that frame's checksum has been validated. An
/// error poisons the context until reset; output touched by a failing call must
/// be discarded. Corresponds to `ZSTD_DCtx` and `ZSTD_decompressStream`.
pub struct Decoder {
    options: DecodeOptions,
    phase: Phase,
    scratch: [u8; 18],
    filled: usize,
    frame: Option<FrameHeader>,
    max_block: usize,
    last_block: bool,
    remaining: usize,
    rle_byte: u8,
    history: RingHistory,
    checksum: XxHash64,
    decoded_total: u64,
    input_total: u64,
    output_total: u64,
    completed_frames: u64,
}

impl Decoder {
    /// Create an allocation-free context with explicit stream resource limits.
    pub fn new(options: DecodeOptions) -> Self {
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
            history: RingHistory::new(
                usize::try_from(options.max_window_size).unwrap_or(usize::MAX),
            ),
            checksum: XxHash64::new(),
            decoded_total: 0,
            input_total: 0,
            output_total: 0,
            completed_frames: 0,
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
    pub fn reset(&mut self) {
        self.phase = Phase::Header;
        self.filled = 0;
        self.frame = None;
        self.max_block = 0;
        self.last_block = false;
        self.remaining = 0;
        self.rle_byte = 0;
        self.history.clear();
        self.checksum.reset();
        self.decoded_total = 0;
        self.input_total = 0;
        self.output_total = 0;
        self.completed_frames = 0;
    }

    /// Reset and apply new limits. A changed window cap releases old storage.
    pub fn reset_with_options(&mut self, options: DecodeOptions) {
        if options.max_window_size != self.options.max_window_size {
            self.history =
                RingHistory::new(usize::try_from(options.max_window_size).unwrap_or(usize::MAX));
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
    /// Owned heap capacity in bytes, excluding inline state and caller buffers.
    pub fn retained_capacity(&self) -> usize {
        self.history.retained_capacity()
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
                    if header.dictionary_id != 0 {
                        return Err(Error::DictionaryRequired);
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
                        BlockKind::Compressed => return Err(Error::UnsupportedFeature),
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
        self.history.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
