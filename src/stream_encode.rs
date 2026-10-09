//! Bounded incremental frame compression with reusable owned block workspaces.

use alloc::vec::Vec;
use core::fmt;

use crate::encode::BlockEncoder;
use crate::frame::{MAGIC, MAX_BLOCK_SIZE};
use crate::xxhash::XxHash64;
use crate::{
    CompressionStrategy, EndDirective, Error, Progress, Result, SharedDictionary, StreamStatus,
};

/// Frame settings for an [`Encoder`].
///
/// Corresponds to the supported frame/window subset of `ZSTD_CCtx_params`.
/// Named strategies have independent bounded search policies. They do not
/// correspond to upstream numeric compression levels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncoderOptions {
    /// Bounded match-search policy (default [`CompressionStrategy::Greedy`]).
    pub strategy: CompressionStrategy,
    /// Base-two logarithm of the advertised window, in `10..=31` (default 17).
    /// Blocks are at most the smaller of this window and 128 KiB.
    pub window_log: u8,
    /// Append the low 32 bits of the content's XXH64 digest (default true).
    pub checksum: bool,
    /// Exact source byte count, enforced even if absent from the frame header.
    /// `None` supports sources whose length is not known before encoding.
    pub pledged_size: Option<u64>,
    /// Include a supplied pledge in the frame header (default true).
    pub include_content_size: bool,
    /// Include a selected formatted dictionary's nonzero ID (default true).
    /// Omitting the ID does not disable dictionary content or entropy usage.
    pub include_dictionary_id: bool,
}

impl EncoderOptions {
    fn validate(self) -> Result<Self> {
        if !(10..=31).contains(&self.window_log) {
            return Err(Error::InvalidParameter);
        }
        Ok(self)
    }
}

impl Default for EncoderOptions {
    fn default() -> Self {
        Self {
            strategy: CompressionStrategy::Greedy,
            window_log: 17,
            checksum: true,
            pledged_size: None,
            include_content_size: true,
            include_dictionary_id: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Active,
    Finishing,
    Finished,
    Failed,
}

/// An in-progress flush/finish owns only a count, never a caller slice.
#[derive(Clone, Copy, Debug)]
struct LatchedDirective {
    directive: EndDirective,
    remaining: usize,
}

/// A reusable encoder that retains at most one input block and pending output.
///
/// Corresponds to the streaming role of `ZSTD_CCtx`/`ZSTD_CStream`. All borrowed
/// slices expire at the end of each call. Internal allocations are fallible and
/// bounded by the maximum block size plus fixed match/entropy workspace; memory
/// use does not grow with the total source length. Existing optional unsafe
/// block backends retain the same checked contract.
pub struct Encoder {
    options: EncoderOptions,
    block: Vec<u8>,
    pending: Vec<u8>,
    pending_position: usize,
    workspace: BlockEncoder,
    checksum: XxHash64,
    total_input: u64,
    total_output: u64,
    header_started: bool,
    phase: Phase,
    latched: Option<LatchedDirective>,
    dictionary: Option<SharedDictionary>,
    prefix: Option<SharedDictionary>,
    active_dictionary: Option<SharedDictionary>,
}

impl Encoder {
    /// Create a fresh frame using the default options without allocating.
    /// Corresponds to `ZSTD_createCStream` followed by default initialization.
    pub fn new() -> Self {
        Self::from_options(EncoderOptions::default())
    }

    /// Validate options and create a fresh frame without allocating.
    pub fn with_options(options: EncoderOptions) -> Result<Self> {
        Ok(Self::from_options(options.validate()?))
    }

    /// Create a context with a persistent dictionary for each new frame.
    pub fn with_dictionary(options: EncoderOptions, dictionary: SharedDictionary) -> Result<Self> {
        let mut encoder = Self::with_options(options)?;
        encoder.dictionary = Some(dictionary);
        Ok(encoder)
    }

    /// Replace the persistent dictionary and clear any unclaimed prefix.
    ///
    /// Call before the first process operation or after reset. An active,
    /// finished or failed frame rejects configuration without changing state.
    /// Corresponds to `ZSTD_CCtx_refCDict` with immutable shared ownership.
    pub fn set_dictionary(&mut self, dictionary: Option<SharedDictionary>) -> Result<()> {
        self.check_configuration_boundary()?;
        self.dictionary = dictionary;
        self.prefix = None;
        Ok(())
    }

    /// Replace the persistent dictionary with a prefix for one frame.
    ///
    /// The first process call that starts a header claims this handle. Reset
    /// preserves an unclaimed prefix and drops a claimed one. For raw prefixes,
    /// construct the handle with [`crate::Dictionary::raw`]. Corresponds to
    /// `ZSTD_CCtx_refPrefix_advanced` with owned lifetime.
    pub fn set_prefix(&mut self, prefix: SharedDictionary) -> Result<()> {
        self.check_configuration_boundary()?;
        self.dictionary = None;
        self.prefix = Some(prefix);
        Ok(())
    }

    fn check_configuration_boundary(&self) -> Result<()> {
        if self.phase == Phase::Active && !self.header_started {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }

    fn from_options(options: EncoderOptions) -> Self {
        Self {
            options,
            block: Vec::new(),
            pending: Vec::new(),
            pending_position: 0,
            workspace: BlockEncoder::new(),
            checksum: XxHash64::new(),
            total_input: 0,
            total_output: 0,
            header_started: false,
            phase: Phase::Active,
            latched: None,
            dictionary: None,
            prefix: None,
            active_dictionary: None,
        }
    }

    /// Return the settings of the current frame.
    pub fn options(&self) -> EncoderOptions {
        self.options
    }

    /// Source bytes accepted since the last reset.
    pub fn total_input(&self) -> u64 {
        self.total_input
    }

    /// Encoded bytes delivered to callers since the last reset.
    pub fn total_output(&self) -> u64 {
        self.total_output
    }

    /// Reachable heap capacity, excluding inline state and caller buffers.
    /// Shared dictionary allocations are counted once for this context.
    pub fn retained_capacity(&self) -> usize {
        self.block.capacity()
            + self.pending.capacity()
            + self.workspace.retained_capacity()
            + self
                .active_dictionary
                .as_ref()
                .or(self.dictionary.as_ref())
                .or(self.prefix.as_ref())
                .map_or(0, |dictionary| dictionary.retained_capacity())
    }

    /// Start another frame, retaining allocations and persistent frame settings.
    ///
    /// The per-frame source pledge is cleared. This also recovers a context
    /// poisoned by a processing error; corresponds to session-only `ZSTD_CCtx_reset`.
    /// Persistent dictionaries and unclaimed prefixes remain configured.
    pub fn reset(&mut self) {
        self.options.pledged_size = None;
        self.clear_frame();
    }

    /// Validate new options, then reset while retaining owned allocations.
    /// Invalid options leave the current context untouched.
    pub fn reset_with_options(&mut self, options: EncoderOptions) -> Result<()> {
        self.options = options.validate()?;
        self.clear_frame();
        Ok(())
    }

    fn clear_frame(&mut self) {
        self.block.clear();
        self.pending.clear();
        self.pending_position = 0;
        self.checksum.reset();
        self.total_input = 0;
        self.total_output = 0;
        self.header_started = false;
        self.phase = Phase::Active;
        self.latched = None;
        self.active_dictionary = None;
        self.workspace.reset();
    }

    /// Accept a source prefix and write an encoded output prefix.
    ///
    /// Corresponds to `ZSTD_compressStream2`. Resubmit every unconsumed suffix.
    /// A flush or finish latches its directive and the original input length;
    /// until completion, use that same directive and supply at most the still
    /// unconsumed suffix. Empty input may drain output, and smaller suffix
    /// fragments are accepted. Completion requires both accepting the original
    /// input and draining the operation's output.
    ///
    /// Empty output can still accept input into the bounded staging block.
    /// Any error poisons the context until reset. After `FrameFinished`, only an
    /// empty `Finish` is accepted, idempotently returning the same boundary.
    pub fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        directive: EndDirective,
    ) -> Result<Progress> {
        let result = self.process_inner(input, output, directive);
        if result.is_err() {
            self.phase = Phase::Failed;
        }
        result
    }

    fn process_inner(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        directive: EndDirective,
    ) -> Result<Progress> {
        if self.phase == Phase::Failed {
            return Err(Error::InvalidState);
        }
        if self.phase == Phase::Finished {
            return if input.is_empty() && directive == EndDirective::Finish {
                Ok(Progress {
                    consumed: 0,
                    produced: 0,
                    status: StreamStatus::FrameFinished,
                })
            } else {
                Err(Error::InvalidState)
            };
        }
        if let Some(latched) = self.latched
            && (directive != latched.directive || input.len() > latched.remaining)
        {
            return Err(Error::InvalidState);
        }
        let offered_total = self
            .total_input
            .checked_add(u64::try_from(input.len()).map_err(|_| Error::ResourceLimit)?)
            .ok_or(Error::ResourceLimit)?;
        if let Some(pledge) = self.options.pledged_size
            && (offered_total > pledge
                || (directive == EndDirective::Finish
                    && self.latched.is_none()
                    && offered_total != pledge))
        {
            return Err(Error::ContentSizeMismatch);
        }
        if self.latched.is_none() && directive != EndDirective::Continue {
            self.latched = Some(LatchedDirective {
                directive,
                remaining: input.len(),
            });
        }
        if !self.header_started {
            self.start_header()?;
        }

        let block_limit = (1_usize << self.options.window_log).min(MAX_BLOCK_SIZE);
        let mut consumed = 0;
        let mut produced = 0;
        loop {
            // Staging precedes output drainage: a pending header/block must not
            // prevent accepting a bounded source prefix when output is empty.
            if self.phase == Phase::Active && consumed < input.len() {
                let count = (input.len() - consumed).min(block_limit - self.block.len());
                if count != 0 {
                    self.block
                        .try_reserve_exact(block_limit - self.block.len())
                        .map_err(|_| Error::AllocationFailed)?;
                    let source = &input[consumed..consumed + count];
                    self.block.extend_from_slice(source);
                    if self.options.checksum {
                        self.checksum.update(source);
                    }
                    self.total_input += count as u64; // Entire offered input was checked above.
                    consumed += count;
                    if let Some(latched) = &mut self.latched {
                        latched.remaining -= count;
                    }
                }
            }
            let count = (self.pending.len() - self.pending_position).min(output.len() - produced);
            self.total_output = self
                .total_output
                .checked_add(count as u64)
                .ok_or(Error::ResourceLimit)?;
            output[produced..produced + count].copy_from_slice(
                &self.pending[self.pending_position..self.pending_position + count],
            );
            self.pending_position += count;
            produced += count;
            if self.pending_position != self.pending.len() {
                return Ok(Progress {
                    consumed,
                    produced,
                    status: StreamStatus::NeedOutput,
                });
            }
            self.pending.clear();
            self.pending_position = 0;
            if self.phase == Phase::Finishing {
                self.phase = Phase::Finished;
                self.latched = None;
                self.active_dictionary = None;
                self.workspace.reset();
                return Ok(Progress {
                    consumed,
                    produced,
                    status: StreamStatus::FrameFinished,
                });
            }

            let remaining = self.latched.map_or(0, |latched| latched.remaining);
            if self.block.len() == block_limit {
                self.seal_block(directive == EndDirective::Finish && remaining == 0)?;
                continue;
            }
            if consumed < input.len() {
                // Draining a previous block made room to keep accepting input.
                continue;
            }
            let status = match directive {
                EndDirective::Continue => StreamStatus::NeedInput,
                _ if remaining != 0 => StreamStatus::NeedInput,
                EndDirective::Flush if self.block.is_empty() => {
                    self.latched = None;
                    StreamStatus::Flushed
                }
                EndDirective::Flush => {
                    self.seal_block(false)?;
                    continue;
                }
                EndDirective::Finish => {
                    self.seal_block(true)?;
                    continue;
                }
            };
            return Ok(Progress {
                consumed,
                produced,
                status,
            });
        }
    }

    /// Writes the non-single-segment form of `ZSTD_writeFrameHeader`.
    fn start_header(&mut self) -> Result<()> {
        self.workspace.set_strategy(self.options.strategy);
        self.pending
            .try_reserve_exact(18)
            .map_err(|_| Error::AllocationFailed)?;
        self.pending.extend_from_slice(&MAGIC.to_le_bytes());
        self.active_dictionary = self.prefix.take().or_else(|| self.dictionary.clone());
        self.workspace.start_frame(
            self.active_dictionary.clone(),
            1_u64 << self.options.window_log,
        )?;
        let dictionary_id = self
            .active_dictionary
            .as_ref()
            .filter(|_| self.options.include_dictionary_id)
            .map_or(0, |dictionary| dictionary.id());
        let (dictionary_flag, dictionary_bytes) = match dictionary_id {
            0 => (0, 0),
            1..=255 => (1, 1),
            256..=65535 => (2, 2),
            _ => (3, 4),
        };
        let size = self
            .options
            .pledged_size
            .filter(|_| self.options.include_content_size);
        let (flag, value, count) = match size {
            None => (0, 0, 0),
            Some(size @ 256..=65_791) => (1, size - 256, 2),
            Some(size) if size <= u64::from(u32::MAX) => (2, size, 4),
            Some(size) => (3, size, 8),
        };
        self.pending
            .push((flag << 6) | (u8::from(self.options.checksum) << 2) | dictionary_flag);
        self.pending.push((self.options.window_log - 10) << 3);
        self.pending
            .extend_from_slice(&dictionary_id.to_le_bytes()[..dictionary_bytes]);
        self.pending
            .extend_from_slice(&value.to_le_bytes()[..count]);
        self.header_started = true;
        Ok(())
    }

    fn seal_block(&mut self, last: bool) -> Result<()> {
        if last
            && self
                .options
                .pledged_size
                .is_some_and(|pledge| pledge != self.total_input)
        {
            return Err(Error::ContentSizeMismatch);
        }
        self.pending
            .try_reserve_exact(self.block.len() + 7)
            .map_err(|_| Error::AllocationFailed)?;
        self.workspace
            .encode(&self.block, last, &mut self.pending)?;
        self.block.clear();
        if last {
            if self.options.checksum {
                self.pending
                    .extend_from_slice(&(self.checksum.digest() as u32).to_le_bytes());
            }
            self.phase = Phase::Finishing;
        }
        Ok(())
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Encoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Encoder")
            .field("options", &self.options)
            .field("phase", &self.phase)
            .field("total_input", &self.total_input)
            .field("total_output", &self.total_output)
            .field("retained_capacity", &self.retained_capacity())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameHeader, decompress};
    use alloc::vec;

    #[test]
    fn dictionary_header_widths_and_omission_preserve_dictionary_configuration() {
        let bytes = include_bytes!("../tests/upstream/dictionaries/formatted.dict");
        for id in [0_u32, 1, 255, 256, 65535, 65536, u32::MAX] {
            let mut bytes = bytes.to_vec();
            bytes[4..8].copy_from_slice(&id.to_le_bytes());
            let dictionary = crate::Dictionary::from_bytes(&bytes).unwrap().into_shared();
            for include_dictionary_id in [false, true] {
                let mut encoder = Encoder::with_dictionary(
                    EncoderOptions {
                        include_dictionary_id,
                        pledged_size: Some(3),
                        ..EncoderOptions::default()
                    },
                    dictionary.clone(),
                )
                .unwrap();
                let encoded = finish(&mut encoder, b"abc", 1);
                let header = FrameHeader::parse(&encoded).unwrap().0;
                assert_eq!(
                    header.dictionary_id,
                    if include_dictionary_id { id } else { 0 }
                );
                assert_eq!(
                    encoded[4] & 3,
                    match header.dictionary_id {
                        0 => 0,
                        1..=255 => 1,
                        256..=65535 => 2,
                        _ => 3,
                    }
                );
                assert_eq!(
                    crate::decompress_with_dictionary(
                        &encoded,
                        &dictionary,
                        crate::DecodeOptions::new(3)
                    )
                    .unwrap(),
                    b"abc"
                );
                assert!(encoder.dictionary.is_some());
                encoder.reset();
                assert!(encoder.dictionary.is_some());
                assert_eq!(encoder.options.include_dictionary_id, include_dictionary_id);
            }
        }
    }

    #[test]
    fn prefix_lifetime_and_failed_encoder_configuration_preserve_existing_state() {
        let dictionary = crate::Dictionary::from_bytes(include_bytes!(
            "../tests/upstream/dictionaries/formatted.dict"
        ))
        .unwrap()
        .into_shared();
        let mut encoder = Encoder::new();
        encoder.set_prefix(dictionary.clone()).unwrap();
        encoder.reset();
        assert!(encoder.prefix.is_some());
        let encoded = finish(&mut encoder, b"", 1);
        assert_eq!(
            FrameHeader::parse(&encoded).unwrap().0.dictionary_id,
            dictionary.id()
        );
        assert!(encoder.prefix.is_none());
        assert!(encoder.active_dictionary.is_none());
        assert_eq!(
            encoder.set_dictionary(Some(dictionary.clone())),
            Err(Error::InvalidState)
        );
        encoder.reset();
        assert_eq!(
            FrameHeader::parse(&finish(&mut encoder, b"", 1))
                .unwrap()
                .0
                .dictionary_id,
            0
        );
        encoder.reset();
        encoder.set_dictionary(Some(dictionary.clone())).unwrap();
        encoder
            .process(b"abc", &mut [], EndDirective::Continue)
            .unwrap();
        assert_eq!(encoder.set_dictionary(None), Err(Error::InvalidState));
        assert_eq!(encoder.set_prefix(dictionary), Err(Error::InvalidState));
        assert!(encoder.dictionary.is_some());
        assert_eq!(encoder.total_input(), 3);
        encoder.reset();
        encoder.set_dictionary(None).unwrap();
        assert!(encoder.dictionary.is_none());
    }

    fn finish(encoder: &mut Encoder, source: &[u8], output_chunk: usize) -> Vec<u8> {
        let mut position = 0;
        let mut encoded = Vec::new();
        for _ in 0..source.len() + 1024 {
            let mut output = vec![0xa5; output_chunk + 8];
            let progress = encoder
                .process(
                    &source[position..],
                    &mut output[..output_chunk],
                    EndDirective::Finish,
                )
                .unwrap();
            assert!(progress.consumed <= source.len() - position);
            assert!(progress.produced <= output_chunk);
            assert!(output[progress.produced..].iter().all(|&byte| byte == 0xa5));
            position += progress.consumed;
            encoded.extend_from_slice(&output[..progress.produced]);
            if progress.status == StreamStatus::FrameFinished {
                assert_eq!(position, source.len());
                return encoded;
            }
            assert_eq!(progress.status, StreamStatus::NeedOutput);
            assert!(progress.consumed != 0 || progress.produced != 0);
        }
        panic!("encoder did not finish with available output");
    }

    #[test]
    fn empty_output_stages_input_and_completion_waits_for_checksum_drain() {
        let mut encoder = Encoder::new();
        let progress = encoder
            .process(b"aa", &mut [], EndDirective::Continue)
            .unwrap();
        assert_eq!(
            progress,
            Progress {
                consumed: 2,
                produced: 0,
                status: StreamStatus::NeedOutput,
            }
        );
        let progress = encoder.process(&[], &mut [], EndDirective::Finish).unwrap();
        assert_eq!(progress.status, StreamStatus::NeedOutput);
        assert_eq!((progress.consumed, progress.produced), (0, 0));
        let encoded = finish(&mut encoder, &[], 1);
        assert_eq!(decompress(&encoded, 2).unwrap(), b"aa");
        assert_eq!(encoder.total_input(), 2);
        assert_eq!(encoder.total_output(), encoded.len() as u64);
        assert_eq!(
            encoder.process(&[], &mut [], EndDirective::Finish).unwrap(),
            Progress {
                consumed: 0,
                produced: 0,
                status: StreamStatus::FrameFinished,
            }
        );
        assert_eq!(
            encoder.process(&[], &mut [], EndDirective::Continue),
            Err(Error::InvalidState)
        );
        assert_eq!(
            encoder.process(&[], &mut [], EndDirective::Finish),
            Err(Error::InvalidState)
        );
    }

    #[test]
    fn non_single_segment_header_encodes_every_content_size_field_boundary() {
        for window_log in [10, 17, 31] {
            for pledge in [
                None,
                Some(0),
                Some(255),
                Some(256),
                Some(65_791),
                Some(65_792),
                Some(u64::from(u32::MAX)),
                Some(u64::from(u32::MAX) + 1),
                Some(u64::MAX),
            ] {
                for include_content_size in [false, true] {
                    let mut encoder = Encoder::with_options(EncoderOptions {
                        window_log,
                        checksum: false,
                        pledged_size: pledge,
                        include_content_size,
                        include_dictionary_id: true,
                        ..EncoderOptions::default()
                    })
                    .unwrap();
                    let mut output = [0; 18];
                    let progress = encoder
                        .process(&[], &mut output, EndDirective::Continue)
                        .unwrap();
                    assert_eq!(progress.status, StreamStatus::NeedInput);
                    let (header, length) =
                        FrameHeader::parse(&output[..progress.produced]).unwrap();
                    assert_eq!(length, progress.produced);
                    assert_eq!(output[4] & 0x20, 0);
                    assert_eq!(header.window_size, 1_u64 << window_log);
                    assert_eq!(header.content_size, pledge.filter(|_| include_content_size));
                    assert!(!header.has_checksum);
                    assert_eq!(header.dictionary_id, 0);
                }
            }
        }
    }

    #[test]
    fn empty_frame_has_independent_known_checksum_and_optional_pledge() {
        for checksum in [false, true] {
            let mut encoder = Encoder::with_options(EncoderOptions {
                checksum,
                pledged_size: Some(0),
                ..EncoderOptions::default()
            })
            .unwrap();
            let encoded = finish(&mut encoder, &[], 1);
            let (header, offset) = FrameHeader::parse(&encoded).unwrap();
            assert_eq!(header.content_size, Some(0));
            assert_eq!(&encoded[offset..offset + 3], [1, 0, 0]);
            assert_eq!(
                &encoded[offset + 3..],
                if checksum {
                    &[0x99, 0xe9, 0xd8, 0x51][..]
                } else {
                    &[]
                }
            );
            assert_eq!(decompress(&encoded, 0).unwrap(), b"");
        }
    }

    #[test]
    fn latched_finish_allows_smaller_suffixes_and_empty_output_drain_calls() {
        let source: Vec<_> = (0..5137).map(|index| (index * 37) as u8).collect();
        let mut encoder = Encoder::with_options(EncoderOptions {
            window_log: 10,
            pledged_size: Some(source.len() as u64),
            ..EncoderOptions::default()
        })
        .unwrap();
        let first = encoder
            .process(&source, &mut [], EndDirective::Finish)
            .unwrap();
        assert_eq!(first.consumed, 1024);
        let mut position = first.consumed;
        let mut encoded = Vec::new();
        for call in 0..100_000 {
            let end = if call % 5 == 0 {
                position
            } else {
                (position + 137).min(source.len())
            };
            let mut output = [0; 11];
            let progress = encoder
                .process(&source[position..end], &mut output, EndDirective::Finish)
                .unwrap();
            position += progress.consumed;
            encoded.extend_from_slice(&output[..progress.produced]);
            if progress.status == StreamStatus::FrameFinished {
                assert_eq!(position, source.len());
                assert_eq!(decompress(&encoded, source.len()).unwrap(), source);
                return;
            }
            assert!(matches!(
                progress.status,
                StreamStatus::NeedInput | StreamStatus::NeedOutput
            ));
        }
        panic!("fragmented finish never completed");
    }

    #[test]
    fn latched_directive_rejects_switches_and_extra_source_until_reset() {
        for extra_input in [false, true] {
            let mut encoder = Encoder::new();
            encoder.process(b"a", &mut [], EndDirective::Flush).unwrap();
            let result = if extra_input {
                encoder.process(b"b", &mut [0; 32], EndDirective::Flush)
            } else {
                encoder.process(&[], &mut [0; 32], EndDirective::Finish)
            };
            assert_eq!(result, Err(Error::InvalidState));
            encoder.reset();
            assert_eq!(
                decompress(&finish(&mut encoder, b"new frame", 3), 9).unwrap(),
                b"new frame"
            );
        }
    }

    #[test]
    fn pledges_are_enforced_even_when_not_emitted_and_reset_clears_them() {
        for include_content_size in [false, true] {
            let options = EncoderOptions {
                pledged_size: Some(2),
                include_content_size,
                ..EncoderOptions::default()
            };
            let mut encoder = Encoder::with_options(options).unwrap();
            assert_eq!(
                encoder.process(b"abc", &mut [], EndDirective::Continue),
                Err(Error::ContentSizeMismatch)
            );
            assert_eq!(encoder.total_input(), 0);
            encoder.reset_with_options(options).unwrap();
            assert_eq!(
                encoder.process(b"a", &mut [], EndDirective::Finish),
                Err(Error::ContentSizeMismatch)
            );
            encoder.reset();
            assert_eq!(
                encoder.options(),
                EncoderOptions {
                    pledged_size: None,
                    ..options
                }
            );
            assert_eq!(
                decompress(&finish(&mut encoder, b"unpledged", 1), 9).unwrap(),
                b"unpledged"
            );
        }
    }

    #[test]
    fn rejected_reset_options_preserve_the_live_frame_and_its_staged_source() {
        let mut encoder = Encoder::new();
        encoder
            .process(b"kept", &mut [], EndDirective::Continue)
            .unwrap();
        let capacity = encoder.retained_capacity();
        for window_log in [0, 9, 32, 255] {
            assert_eq!(
                encoder.reset_with_options(EncoderOptions {
                    window_log,
                    ..EncoderOptions::default()
                }),
                Err(Error::InvalidParameter)
            );
            assert_eq!(encoder.total_input(), 4);
            assert_eq!(encoder.retained_capacity(), capacity);
        }
        let encoded = finish(&mut encoder, b" source", 1);
        assert_eq!(decompress(&encoded, 11).unwrap(), b"kept source");
    }

    #[test]
    fn chunked_checksums_cover_all_accepted_source_and_detect_corruption() {
        let source: Vec<_> = (0..MAX_BLOCK_SIZE + 513)
            .map(|index| b"independent periodic checksum"[index % 29])
            .collect();
        let mut encoder = Encoder::new();
        let mut encoded = Vec::new();
        for chunk in source.chunks(19) {
            let mut output = [0; 4096];
            let progress = encoder
                .process(chunk, &mut output, EndDirective::Continue)
                .unwrap();
            assert_eq!(progress.consumed, chunk.len());
            encoded.extend_from_slice(&output[..progress.produced]);
        }
        encoded.extend_from_slice(&finish(&mut encoder, &[], 1));
        assert_eq!(decompress(&encoded, source.len()).unwrap(), source);
        let checksum = crate::xxhash::checksum(&source) as u32;
        assert_eq!(&encoded[encoded.len() - 4..], checksum.to_le_bytes());
        *encoded.last_mut().unwrap() ^= 1;
        assert_eq!(
            decompress(&encoded, source.len()),
            Err(Error::ChecksumMismatch)
        );
    }
}
