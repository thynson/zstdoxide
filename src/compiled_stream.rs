//! Incremental compiled-dictionary frames over the reference block engine.
//!
//! One borrowed [`CompiledDictionary`](crate::CompiledDictionary) supplies the
//! immutable candidates and first-block entropy for every frame. The stream
//! retains at most one input block, one combined staging buffer bounded by the
//! resolved window plus one block, the last window bytes of frame history, and
//! pending output. Every sealed block re-binds a reference search state over
//! `compiled suffix | retained history | block`, so compiled positions keep
//! their suffix-relative meaning and retained history remains offset-exact.
//!
//! Upstream counterparts are the CDict attach and per-frame initialization of
//! `ZSTD_CCtx_refCDict` and `ZSTD_compressStream2`; native attach-versus-load
//! workspace reuse, parameter resets and prefix replacement are not implied.
#![forbid(unsafe_code)]

use alloc::alloc::{Allocator, Global};
use alloc::vec::Vec;
use core::fmt;

use crate::block_entropy::BlockEntropyEncoder;
use crate::compiled_dictionary::CompiledDictionary;
use crate::frame::{MAGIC, MAX_BLOCK_SIZE};
use crate::literal_encode::LiteralEncoder;
use crate::reference_policy::raw_literals;
use crate::reference_search::{ReferenceSearchPlan, ReferenceSearchState};
use crate::sequence_context::SequenceEncoder;
use crate::xxhash::XxHash64;
use crate::{
    DictionaryView, EndDirective, Error, Progress, ResolvedCompressionParameters, Result,
    StreamStatus,
};

/// Frame settings for a [`CompiledStreamEncoder`].
///
/// These are the frame-level flags a compiled stream may still choose. The
/// strategy, table geometry and window are fixed by the compiled profile and
/// cannot be overridden here. Defaults match `ZSTD_getParams` for a compiled
/// dictionary: content size and dictionary ID are included, checksum is off.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledStreamOptions {
    /// Append the low 32 bits of the content's XXH64 digest (default false).
    pub checksum: bool,
    /// Exact source byte count, enforced even if absent from the frame header.
    /// `None` supports sources whose length is not known before encoding.
    pub pledged_size: Option<u64>,
    /// Include a supplied pledge in the frame header (default true).
    pub include_content_size: bool,
    /// Include the compiled dictionary's nonzero ID (default true).
    pub include_dictionary_id: bool,
}

impl Default for CompiledStreamOptions {
    fn default() -> Self {
        Self {
            checksum: false,
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

/// A reusable incremental encoder over one immutable compiled dictionary.
///
/// The compiled state is borrowed for the whole stream and is never cloned,
/// reseeded or mutated. Each sealed block builds a per-block parser over the
/// compiled suffix, the retained frame history and the block; those parser
/// owners are released when the block ends. Input and output slices expire at
/// the end of each call. Allocation is fallible and bounded by the resolved
/// window and one block.
pub struct CompiledStreamEncoder<'a, A: Allocator + Clone = Global, B: Allocator + Clone = Global> {
    compiled: CompiledDictionary<'a, B>,
    options: CompiledStreamOptions,
    block: Vec<u8, A>,
    combined: Vec<u8, A>,
    history: Vec<u8, A>,
    pending: Vec<u8, A>,
    pending_position: usize,
    entropy: BlockEntropyEncoder<A>,
    checksum: XxHash64,
    total_input: u64,
    total_output: u64,
    frame_position: u64,
    window_size: usize,
    block_limit: usize,
    header_started: bool,
    phase: Phase,
    latched: Option<LatchedDirective>,
    allocator: A,
}

impl<'a, B: Allocator + Clone> CompiledStreamEncoder<'a, Global, B> {
    /// Create a fresh frame over `compiled` with default frame options.
    ///
    /// The compiled profile is validated before any workspace allocation. No
    /// compiled table is copied or seeded.
    pub fn new(compiled: CompiledDictionary<'a, B>) -> Result<Self> {
        Self::with_options(compiled, CompiledStreamOptions::default())
    }

    /// Validate frame options and create a fresh frame over `compiled`.
    pub fn with_options(
        compiled: CompiledDictionary<'a, B>,
        options: CompiledStreamOptions,
    ) -> Result<Self> {
        Self::with_options_in(compiled, options, Global)
    }
}

impl<'a, A: Allocator + Clone, B: Allocator + Clone> CompiledStreamEncoder<'a, A, B> {
    /// Create a fresh frame using `allocator` for every workspace owner.
    ///
    /// The allocator is independent of the compiled state's own allocation
    /// domain: compiled tables stay with their constructing allocator, while
    /// staging, history, pending output and per-block parsers use this one.
    pub fn new_in(compiled: CompiledDictionary<'a, B>, allocator: A) -> Result<Self> {
        Self::with_options_in(compiled, CompiledStreamOptions::default(), allocator)
    }

    /// Validate options and create a fresh frame.
    ///
    /// Only scalar frame flags are validated. The strategy, window, tables and
    /// entropy remain the compiled profile's. A profile whose window cannot be
    /// described by the supported reference frame header is rejected with
    /// [`Error::UnsupportedFeature`] before the first allocation.
    pub fn with_options_in(
        compiled: CompiledDictionary<'a, B>,
        options: CompiledStreamOptions,
        allocator: A,
    ) -> Result<Self> {
        let parameters = compiled.parameters();
        let window_size = 1_usize
            .checked_shl(u32::from(parameters.window_log()))
            .ok_or(Error::ResourceLimit)?;
        // The per-block plan builds `ReferenceFrameSettings`; reject a profile
        // outside that bounded window range before any owner is created.
        if !(10..=21).contains(&parameters.window_log()) {
            return Err(Error::UnsupportedFeature);
        }
        let block_limit = window_size.min(MAX_BLOCK_SIZE);
        Ok(Self {
            compiled,
            options,
            block: Vec::new_in(allocator.clone()),
            combined: Vec::new_in(allocator.clone()),
            history: Vec::new_in(allocator.clone()),
            pending: Vec::new_in(allocator.clone()),
            pending_position: 0,
            entropy: BlockEntropyEncoder::from_parts(
                LiteralEncoder::new_in(allocator.clone()),
                SequenceEncoder::new_in(allocator.clone()),
                Vec::new_in(allocator.clone()),
            ),
            checksum: XxHash64::new(),
            total_input: 0,
            total_output: 0,
            frame_position: 0,
            window_size,
            block_limit,
            header_started: false,
            phase: Phase::Active,
            latched: None,
            allocator,
        })
    }

    /// Borrow the immutable compiled dictionary view.
    pub const fn dictionary(&self) -> DictionaryView<'a> {
        self.compiled.view()
    }

    /// The fixed profile every frame of this stream uses.
    pub const fn parameters(&self) -> ResolvedCompressionParameters {
        self.compiled.parameters()
    }

    /// Current frame settings.
    pub const fn options(&self) -> CompiledStreamOptions {
        self.options
    }

    /// Source bytes accepted since the last reset.
    pub const fn total_input(&self) -> u64 {
        self.total_input
    }

    /// Encoded bytes delivered to callers since the last reset.
    pub const fn total_output(&self) -> u64 {
        self.total_output
    }

    /// Reachable workspace capacity, excluding inline state and the compiled
    /// dictionary's own tables and content borrow.
    pub fn retained_capacity(&self) -> usize {
        self.block.capacity()
            + self.combined.capacity()
            + self.history.capacity()
            + self.pending.capacity()
            + self.entropy.retained_capacity()
    }

    /// Start another frame while retaining compiled selection, frame options
    /// and workspace allocations.
    ///
    /// The per-frame source pledge is cleared and the next frame installs the
    /// compiled first-block entropy seed again. This also recovers a context
    /// poisoned by a processing error. Corresponds to session-only
    /// `ZSTD_CCtx_reset`, with the compiled borrow still selected.
    pub fn reset(&mut self) {
        self.options.pledged_size = None;
        self.clear_frame();
    }

    /// Reset the session and install new frame options.
    ///
    /// The compiled selection and every workspace allocation are retained. The
    /// new options describe the next frame only; the compiled profile, window
    /// and table geometry cannot be changed.
    pub fn reset_with_options(&mut self, options: CompiledStreamOptions) -> Result<()> {
        self.options = options;
        self.clear_frame();
        Ok(())
    }

    fn clear_frame(&mut self) {
        self.block.clear();
        self.combined.clear();
        self.history.clear();
        self.pending.clear();
        self.pending_position = 0;
        self.checksum.reset();
        self.entropy.reset();
        self.total_input = 0;
        self.total_output = 0;
        self.frame_position = 0;
        self.header_started = false;
        self.phase = Phase::Active;
        self.latched = None;
    }

    /// Accept a source prefix and write an encoded output prefix.
    ///
    /// Corresponds to `ZSTD_compressStream2` over a referenced compiled
    /// dictionary. Resubmit every unconsumed suffix. A flush or finish latches
    /// its directive and the original input length; until completion, use that
    /// same directive and supply at most the still unconsumed suffix. Empty
    /// input may drain output, and smaller suffix fragments are accepted.
    ///
    /// Any error poisons the context until [`Self::reset`]; output touched by a
    /// failing call must be discarded. After `FrameFinished`, only an empty
    /// `Finish` is accepted, idempotently returning the same boundary.
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

        let mut consumed = 0;
        let mut produced = 0;
        loop {
            // Staging precedes output drainage: a pending header/block must not
            // prevent accepting a bounded source prefix when output is empty.
            if self.phase == Phase::Active && consumed < input.len() {
                let count = (input.len() - consumed).min(self.block_limit - self.block.len());
                if count != 0 {
                    self.block
                        .try_reserve_exact(self.block_limit - self.block.len())
                        .map_err(|_| Error::AllocationFailed)?;
                    let source = &input[consumed..consumed + count];
                    for &byte in source {
                        self.block.push(byte);
                    }
                    if self.options.checksum {
                        self.checksum.update(source);
                    }
                    self.total_input += count as u64; // Offered total was checked above.
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
                return Ok(Progress {
                    consumed,
                    produced,
                    status: StreamStatus::FrameFinished,
                });
            }

            let remaining = self.latched.map_or(0, |latched| latched.remaining);
            if self.block.len() == self.block_limit {
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

    /// Writes the non-single-segment form of `ZSTD_writeFrameHeader` and
    /// installs the compiled first-block entropy seed.
    fn start_header(&mut self) -> Result<()> {
        self.pending
            .try_reserve_exact(18)
            .map_err(|_| Error::AllocationFailed)?;
        let parameters = self.compiled.parameters();
        let seed = self.compiled.first_block_seed();
        let (entropy, literal_table) = match seed {
            Some((entropy, table)) => (Some(entropy), Some(table)),
            None => (None, None),
        };
        self.entropy.start_frame(entropy, literal_table)?;
        for byte in MAGIC.to_le_bytes() {
            self.pending.push(byte);
        }
        let dictionary_id = if self.options.include_dictionary_id {
            self.compiled.dictionary_id()
        } else {
            0
        };
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
        self.pending.push((parameters.window_log() - 10) << 3);
        for &byte in &dictionary_id.to_le_bytes()[..dictionary_bytes] {
            self.pending.push(byte);
        }
        for &byte in &value.to_le_bytes()[..count] {
            self.pending.push(byte);
        }
        self.header_started = true;
        Ok(())
    }

    /// Emit the staged block through a fresh reference search state.
    ///
    /// Every fallible step precedes the first output byte: plan admission,
    /// combined staging, parser binding, entropy preparation and pending
    /// capacity. A failure therefore appends nothing for this block, and the
    /// caller poisons the stream because parser/entropy scratch may already
    /// have advanced.
    fn seal_block(&mut self, last: bool) -> Result<()> {
        if last
            && self
                .options
                .pledged_size
                .is_some_and(|pledge| pledge != self.total_input)
        {
            return Err(Error::ContentSizeMismatch);
        }
        let block_len = self.block.len();
        if block_len == 0 {
            // A completed empty frame still emits one last empty raw block.
            self.pending
                .try_reserve_exact(3)
                .map_err(|_| Error::AllocationFailed)?;
            self.pending.extend_from_slice(&[1, 0, 0]);
            if self.options.checksum {
                for byte in (self.checksum.digest() as u32).to_le_bytes() {
                    self.pending.push(byte);
                }
            }
            self.phase = Phase::Finishing;
            return Ok(());
        }
        let plan = ReferenceSearchPlan::from_resolved(self.compiled.parameters(), block_len)?;
        let candidates = self.compiled.search_candidates()?;
        let loaded = self.compiled.loaded_len();
        let history_len = usize::try_from(self.frame_position)
            .map_err(|_| Error::ResourceLimit)?
            .min(self.window_size);
        if self.history.len() != history_len {
            return Err(Error::InvalidState);
        }
        let combined_len = loaded
            .checked_add(history_len)
            .and_then(|value| value.checked_add(block_len))
            .ok_or(Error::ResourceLimit)?;
        self.combined.clear();
        self.combined
            .try_reserve_exact(combined_len)
            .map_err(|_| Error::AllocationFailed)?;
        let content = self.compiled.content();
        if loaded == 0
            || loaded > content.len()
            || loaded > self.window_size
            || !plan.admits_dictionary_staging()
        {
            return Err(Error::InvalidParameter);
        }
        self.combined
            .extend_from_slice(&content[content.len() - loaded..]);
        self.combined.extend_from_slice(&self.history);
        self.combined.extend_from_slice(&self.block);

        let initial_repeat = self.entropy.sequence_encoder.committed_repeats();
        let mut search = ReferenceSearchState::new_with_compiled(
            &self.combined,
            loaded + history_len,
            plan,
            &self.allocator,
            candidates,
        )?;
        let parsed = search.parse_next(block_len, initial_repeat)?;
        let worthwhile = if parsed {
            self.entropy.prepare_reference(
                search.literals(),
                search.sequences(),
                initial_repeat,
                block_len,
                raw_literals(self.compiled.parameters()),
            )?
        } else {
            false
        };
        // The reference gate is after entropy work: rejected gain has effective
        // compressed size zero, and a short payload also loses to block RLE.
        let rle = parsed
            && self.frame_position != 0
            && (!worthwhile || self.entropy.as_bytes().len() < 25)
            && self.block.iter().all(|&byte| byte == self.block[0]);
        let (kind, payload) = if rle {
            (1_u8, &self.block[..1])
        } else if worthwhile {
            (2_u8, self.entropy.as_bytes())
        } else {
            (0_u8, &self.block[..])
        };
        self.pending
            .try_reserve_exact(block_len + 3)
            .map_err(|_| Error::AllocationFailed)?;
        self.history
            .try_reserve_exact(self.window_size.saturating_sub(self.history.len()))
            .map_err(|_| Error::AllocationFailed)?;
        let value = (u32::try_from(payload.len()).map_err(|_| Error::ResourceLimit)? << 3)
            | (u32::from(kind) << 1)
            | u32::from(last);
        for byte in &value.to_le_bytes()[..3] {
            self.pending.push(*byte);
        }
        for &byte in payload {
            self.pending.push(byte);
        }
        if kind == 2 {
            self.entropy.commit();
        } else {
            self.entropy.discard();
        }
        self.advance_history(block_len, history_len)?;
        self.block.clear();
        self.frame_position = self
            .frame_position
            .checked_add(block_len as u64)
            .ok_or(Error::ResourceLimit)?;
        if last {
            if self.options.checksum {
                for byte in (self.checksum.digest() as u32).to_le_bytes() {
                    self.pending.push(byte);
                }
            }
            self.phase = Phase::Finishing;
        }
        Ok(())
    }

    /// Retain the last `min(emitted, window)` frame bytes contiguously.
    ///
    /// Capacity for one full window was reserved by [`Self::seal_block`] before
    /// output was appended, so this performs no fallible operation.
    fn advance_history(&mut self, block_len: usize, history_len: usize) -> Result<()> {
        let next_len = history_len
            .checked_add(block_len)
            .ok_or(Error::ResourceLimit)?;
        if next_len > self.window_size {
            let drop_count = next_len - self.window_size;
            if drop_count >= history_len {
                // The block alone fills or exceeds the window: retain exactly
                // its last window bytes.
                let start = drop_count - history_len;
                self.history.clear();
                self.history.extend_from_slice(&self.block[start..]);
            } else {
                self.history.copy_within(drop_count.., 0);
                self.history.truncate(history_len - drop_count);
                self.history.extend_from_slice(&self.block);
            }
        } else {
            self.history.extend_from_slice(&self.block);
        }
        debug_assert!(self.history.len() <= self.window_size);
        Ok(())
    }
}

impl<A: Allocator + Clone, B: Allocator + Clone> fmt::Debug for CompiledStreamEncoder<'_, A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompiledStreamEncoder")
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
    use crate::{
        CompressionParameterRequest, DecodeOptions, Dictionary, FrameHeader,
        decompress_with_dictionary_in,
    };
    use alloc::vec;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn pattern(mut state: u32, length: usize) -> Vec<u8> {
        (0..length)
            .map(|_| {
                state ^= state.wrapping_shl(13);
                state ^= state >> 17;
                state ^= state.wrapping_shl(5);
                state as u8
            })
            .collect()
    }

    fn compiled<'a>(
        dictionary: &'a Dictionary,
        level: i32,
        allocator: Global,
    ) -> CompiledDictionary<'a, Global> {
        CompiledDictionary::new(
            dictionary.view(),
            &CompressionParameterRequest::with_level(level),
            allocator,
        )
        .unwrap()
    }

    /// Feed `input` in `chunk`-sized pieces with a small caller output buffer,
    /// exercising staging, output drainage and latched directives.
    fn stream(
        encoder: &mut CompiledStreamEncoder<'_, Global>,
        input: &[u8],
        chunk: usize,
        output_chunk: usize,
    ) -> Vec<u8> {
        let mut frame = Vec::new();
        let mut output = vec![0_u8; output_chunk.max(1)];
        let mut position = 0;
        loop {
            let end = (position + chunk.max(1)).min(input.len());
            let directive = if end == input.len() {
                EndDirective::Finish
            } else {
                EndDirective::Continue
            };
            let mut consumed = 0;
            loop {
                let progress = encoder
                    .process(&input[position + consumed..end], &mut output, directive)
                    .unwrap();
                frame.extend_from_slice(&output[..progress.produced]);
                consumed += progress.consumed;
                match progress.status {
                    StreamStatus::NeedInput => break,
                    StreamStatus::NeedOutput => continue,
                    StreamStatus::FrameFinished => {
                        assert_eq!(position + consumed, input.len());
                        return frame;
                    }
                    StreamStatus::Flushed => unreachable!(),
                }
            }
            position = end;
        }
    }

    fn roundtrip(level: i32, content_len: usize, input: &[u8], options: CompiledStreamOptions) {
        let content = pattern(0x9e37_79b9 ^ level as u32, content_len);
        let dictionary = Dictionary::raw(&content).unwrap();
        let compiled = compiled(&dictionary, level, Global);
        let mut encoder =
            CompiledStreamEncoder::with_options_in(compiled, options, Global).unwrap();
        let frame = stream(&mut encoder, input, 333, 17);
        let header = FrameHeader::parse(&frame).unwrap().0;
        assert_eq!(header.dictionary_id, dictionary.view().id());
        assert_eq!(header.content_size, options.pledged_size);
        assert_eq!(header.has_checksum, options.checksum);
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &dictionary.view(),
                DecodeOptions::new(input.len()),
                Global,
            )
            .unwrap(),
            input,
            "level {level}"
        );
    }

    #[test]
    fn every_compiled_family_streams_multiple_blocks_with_and_without_checksums() {
        let content = pattern(0x51ce_5eed, 4096);
        // Repeats of dictionary slices plus a cross-block run force candidates
        // into the compiled suffix, retained history and the current block.
        let mut input = Vec::new();
        for _ in 0..7 {
            input.extend_from_slice(&content[1024..2048]);
            input.extend_from_slice(&content[0..777]);
            input.extend_from_slice(b"streamed-compiled-block-boundary");
        }
        let input = input.repeat(9);
        for level in [1, 3, 4, 5, 7, 9] {
            for checksum in [false, true] {
                roundtrip(
                    level,
                    4096,
                    &input,
                    CompiledStreamOptions {
                        checksum,
                        pledged_size: Some(input.len() as u64),
                        ..CompiledStreamOptions::default()
                    },
                );
            }
        }
    }

    #[test]
    fn empty_and_tiny_frames_emit_truthful_headers_and_decode() {
        for level in [1, 3, 4, 9] {
            for input in [b"".as_slice(), b"a", b"abc"] {
                roundtrip(
                    level,
                    4096,
                    input,
                    CompiledStreamOptions {
                        checksum: true,
                        pledged_size: Some(input.len() as u64),
                        ..CompiledStreamOptions::default()
                    },
                );
            }
        }
    }

    #[test]
    fn session_reset_keeps_the_compiled_selection_for_the_next_frame() {
        let content = pattern(0x0bad_c0de, 8192);
        let dictionary = Dictionary::raw(&content).unwrap();
        let compiled = compiled(&dictionary, 4, Global);
        let mut encoder = CompiledStreamEncoder::new_in(compiled, Global).unwrap();
        let mut first = Vec::new();
        for _ in 0..2 {
            let frame = stream(&mut encoder, &content[..5000], 1024, 7);
            let decoded = decompress_with_dictionary_in(
                &frame,
                &dictionary.view(),
                DecodeOptions::new(5000),
                Global,
            )
            .unwrap();
            assert_eq!(decoded, &content[..5000]);
            first.push(frame);
            encoder.reset();
        }
        // The compiled selection and workspace survive the session reset, so a
        // second frame with the same input is identical.
        assert_eq!(first[0], first[1]);
        assert_eq!(encoder.total_input(), 0);
        assert_eq!(encoder.total_output(), 0);
    }

    #[test]
    fn pledged_size_and_directive_guards_poison_until_reset() {
        let content = pattern(0x1234_5678, 2048);
        let dictionary = Dictionary::raw(&content).unwrap();
        let compiled = compiled(&dictionary, 1, Global);
        let mut encoder = CompiledStreamEncoder::with_options_in(
            compiled,
            CompiledStreamOptions {
                pledged_size: Some(9),
                ..CompiledStreamOptions::default()
            },
            Global,
        )
        .unwrap();
        let mut output = [0_u8; 64];
        assert_eq!(
            encoder.process(b"0123456789", &mut output, EndDirective::Finish),
            Err(Error::ContentSizeMismatch)
        );
        assert_eq!(
            encoder.process(b"", &mut output, EndDirective::Finish),
            Err(Error::InvalidState)
        );
        encoder.reset();
        assert_eq!(encoder.total_input(), 0);
        assert_eq!(encoder.options().pledged_size, None);
        let frame = stream(&mut encoder, b"0123456789", 4, 8);
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &dictionary.view(),
                DecodeOptions::new(10),
                Global,
            )
            .unwrap(),
            b"0123456789"
        );
    }

    #[test]
    fn single_block_payload_matches_the_one_shot_compiled_frame() {
        use crate::compress_with_compiled;

        let content = pattern(0x5eed_1234, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let input = &content[512..3000];
        for level in [1, 3, 4, 5, 7, 9] {
            let mut request = CompressionParameterRequest::with_level(level);
            request
                .set_source_hint(u32::try_from(input.len()).unwrap())
                .unwrap();
            let compiled = CompiledDictionary::new(dictionary.view(), &request, Global).unwrap();
            let one_shot = compress_with_compiled(input, &request, &compiled).unwrap();
            let one_shot_used = FrameHeader::parse(&one_shot).unwrap().1;
            let mut encoder = CompiledStreamEncoder::with_options_in(
                compiled,
                CompiledStreamOptions {
                    pledged_size: Some(input.len() as u64),
                    ..CompiledStreamOptions::default()
                },
                Global,
            )
            .unwrap();
            let streamed = stream(&mut encoder, input, input.len(), 64);
            let streamed_used = FrameHeader::parse(&streamed).unwrap().1;
            // Both paths stage exactly the compiled suffix before one short
            // block and share the plan, parser and entropy, so the block bytes
            // are identical even though the streaming header is not
            // single-segment. This freezes the per-block parser binding.
            assert_eq!(
                &streamed[streamed_used..],
                &one_shot[one_shot_used..],
                "level {level}"
            );
        }
    }

    #[test]
    fn flush_emits_a_complete_nonlast_block_and_finishes_later() {
        let content = pattern(0x0f1e_2d3c, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let compiled = compiled(&dictionary, 4, Global);
        let mut encoder = CompiledStreamEncoder::new_in(compiled, Global).unwrap();
        let first = &content[..1000];
        let second = b"flush boundary tail";
        let mut output = [0xa5_u8; 256];
        let mut frame = Vec::new();
        let progress = encoder
            .process(first, &mut output, EndDirective::Flush)
            .unwrap();
        assert_eq!(progress.status, StreamStatus::Flushed);
        assert_eq!(progress.consumed, first.len());
        frame.extend_from_slice(&output[..progress.produced]);
        let mut guard = 0;
        loop {
            guard += 1;
            assert!(guard < 10_000, "flush did not finish");
            let progress = encoder
                .process(second, &mut output, EndDirective::Finish)
                .unwrap();
            frame.extend_from_slice(&output[..progress.produced]);
            if progress.status == StreamStatus::FrameFinished {
                break;
            }
            if progress.status == StreamStatus::NeedInput {
                panic!("finish latched less input than supplied");
            }
        }
        let expected: Vec<u8> = first.iter().chain(second.iter()).copied().collect();
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &dictionary.view(),
                DecodeOptions::new(expected.len()),
                Global,
            )
            .unwrap(),
            expected
        );
    }

    #[test]
    fn multi_block_body_matches_one_shot_below_window_wrap() {
        use crate::compress_with_compiled;

        let content = pattern(0x7711_2233, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let mut input = Vec::new();
        for index in 0..10_000 {
            let at = (index * 97) % 3000;
            input.extend_from_slice(&content[at..at + 20]);
        }
        // Window log 18 keeps the whole source single-segment in the one-shot
        // header while the block limit stays 128 KiB, so this exercises two
        // streaming blocks without ever dropping the compiled suffix.
        for level in [1, 3, 4, 5, 7, 9] {
            let mut request = CompressionParameterRequest::with_level(level);
            request.set_window_log(18).unwrap();
            request
                .set_source_hint(u32::try_from(input.len()).unwrap())
                .unwrap();
            let compiled = CompiledDictionary::new(dictionary.view(), &request, Global).unwrap();
            let one_shot = compress_with_compiled(&input, &request, &compiled).unwrap();
            let mut encoder = CompiledStreamEncoder::with_options_in(
                compiled,
                CompiledStreamOptions {
                    pledged_size: Some(input.len() as u64),
                    ..CompiledStreamOptions::default()
                },
                Global,
            )
            .unwrap();
            let streamed = stream(&mut encoder, &input, 5000, 4096);
            let one_shot_header = FrameHeader::parse(&one_shot).unwrap();
            let streamed_header = FrameHeader::parse(&streamed).unwrap();
            // The one-shot header is single-segment (the window is inferred
            // from the content size); the streaming header is not, so only the
            // block bodies are compared. Re-binding one parser per block with
            // retained history reproduces the single-state sampling output for
            // these below-window geometries, including the sorted tree.
            assert_eq!(one_shot_header.0.content_size, Some(input.len() as u64));
            assert_eq!(streamed_header.0.content_size, Some(input.len() as u64));
            assert_eq!(streamed_header.0.window_size, 1 << 18, "level {level}");
            assert_eq!(
                one_shot_header.0.window_size,
                input.len() as u64,
                "level {level}"
            );
            assert_eq!(
                &streamed[streamed_header.1..],
                &one_shot[one_shot_header.1..],
                "level {level}"
            );
        }
    }

    #[test]
    fn direct_stream_allocator_failure_releases_every_owner() {
        let content = pattern(0xfeed_face, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        for fail_after in 0..12 {
            let compiled = compiled(&dictionary, 1, Global);
            let allocator = TrackingAllocator::new(AllocatorConfig {
                fail_at: core::num::NonZeroU64::new(fail_after as u64),
                ..AllocatorConfig::default()
            })
            .unwrap();
            let monitor = allocator.monitor();
            let mut encoder = CompiledStreamEncoder::new_in(compiled, allocator.clone()).unwrap();
            let mut output = [0_u8; 128];
            let _ = encoder.process(&content, &mut output, EndDirective::Finish);
            drop(encoder);
            drop(allocator);
            monitor.assert_released();
        }
    }
}
