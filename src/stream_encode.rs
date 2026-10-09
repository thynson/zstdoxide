//! Bounded incremental frame compression with reusable owned block workspaces.

use alloc::alloc::{Allocator, Global};
use alloc::vec::Vec;
use core::fmt;

use crate::dictionary::DictionaryHandle;
use crate::encode::BlockEncoder;
use crate::frame::{MAGIC, MAX_BLOCK_SIZE};
use crate::match_search::{MatchSearchOptions, SearchShape};
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
/// Corresponds to the streaming role of `ZSTD_CCtx`/`ZSTD_CStream`. Input/output
/// slices expire at the end of each call; a borrowed dictionary view retains
/// its explicit lifetime. Internal allocations are fallible and
/// bounded by the maximum block size, configured match-search/history storage,
/// and entropy workspace; memory use does not grow with total source length.
/// Existing optional unsafe block backends retain the same checked contract.
pub struct Encoder<A: Allocator = Global, D: DictionaryHandle = SharedDictionary> {
    options: EncoderOptions,
    block: Vec<u8, A>,
    pending: Vec<u8, A>,
    pending_position: usize,
    workspace: BlockEncoder<A, D>,
    checksum: XxHash64,
    total_input: u64,
    total_output: u64,
    header_started: bool,
    phase: Phase,
    latched: Option<LatchedDirective>,
    dictionary: Option<D>,
    prefix: Option<D>,
    active_dictionary: Option<D>,
}

impl Encoder<Global, SharedDictionary> {
    /// Create a fresh frame using the default options without allocating.
    /// Corresponds to `ZSTD_createCStream` followed by default initialization.
    pub fn new() -> Self {
        Self::new_in(Global)
    }

    /// Validate options and create a fresh frame without allocating.
    pub fn with_options(options: EncoderOptions) -> Result<Self> {
        Self::with_options_in(options, Global)
    }

    /// Create a context with a persistent dictionary for each new frame.
    pub fn with_dictionary(options: EncoderOptions, dictionary: SharedDictionary) -> Result<Self> {
        Self::with_dictionary_in(options, dictionary, Global)
    }
}

impl<A: Allocator + Clone> Encoder<A, SharedDictionary> {
    /// Construct empty workspace owners with default settings. No codec payload
    /// is allocated. Fresh allocator clones own staging and pending storage;
    /// the original value remains with the block workspace's allocation factory.
    /// Ordinary allocator cloning retains the caller's allocation/panic policy.
    pub fn new_in(allocator: A) -> Self {
        Self::from_options_in(EncoderOptions::default(), allocator)
    }

    /// Validate settings before cloning the allocator or creating storage owners.
    pub fn with_options_in(options: EncoderOptions, allocator: A) -> Result<Self> {
        Ok(Self::from_options_in(options.validate()?, allocator))
    }
}

impl<A: Allocator + Clone, D: DictionaryHandle> Encoder<A, D> {
    /// Attach a persistent shared dictionary or borrowed dictionary view, using
    /// an independent allocator for codec workspace. Attachment and reset borrow
    /// prepared tables without copying dictionary allocations. A borrowed view
    /// must remain valid for the context's lifetime; shared owners retain their
    /// existing Global Arc/Rc control allocation outside the selected domain.
    pub fn with_dictionary_in(
        options: EncoderOptions,
        dictionary: D,
        allocator: A,
    ) -> Result<Self> {
        let mut encoder = Self::from_options_in(options.validate()?, allocator);
        encoder.dictionary = Some(dictionary);
        Ok(encoder)
    }

    /// Replace the persistent dictionary and clear any unclaimed prefix.
    ///
    /// Call before the first process operation or after reset. An active,
    /// finished or failed frame rejects configuration without changing state.
    /// Corresponds to `ZSTD_CCtx_refCDict` with the immutable attachment lifetime.
    pub fn set_dictionary(&mut self, dictionary: Option<D>) -> Result<()> {
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
    /// `ZSTD_CCtx_refPrefix_advanced` with the attachment's enforced lifetime.
    pub fn set_prefix(&mut self, prefix: D) -> Result<()> {
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

    fn from_options_in(options: EncoderOptions, allocator: A) -> Self {
        Self {
            options,
            block: Vec::new_in(allocator.clone()),
            pending: Vec::new_in(allocator.clone()),
            pending_position: 0,
            workspace: BlockEncoder::new_in(allocator),
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

    /// Return the configured settings for this session or its next frame.
    /// After a completed frame's parameter reset, these settings do not describe
    /// the previously emitted frame.
    pub fn options(&self) -> EncoderOptions {
        self.options
    }

    /// Return the configured named-policy search and storage settings.
    ///
    /// These operational controls are independent of reference compression
    /// parameter queries and do not enable numeric compression levels.
    pub fn search_options(&self) -> MatchSearchOptions {
        self.workspace.search_options()
    }

    /// Replace search settings before the first process call or after reset.
    ///
    /// A session whose frame has started, finished or failed returns
    /// [`Error::InvalidState`] before shape validation. At a clean boundary the
    /// checked shape is stored without allocating or cloning any allocator or
    /// dictionary handle. Errors preserve the entire context. Frame settings,
    /// counters, selected dictionary/prefix and existing capacities are unchanged.
    ///
    /// Storage is prepared when the next frame or block needs it. A deferred
    /// allocation failure then follows [`Self::process`]'s poison/reset contract.
    /// Changing settings alone does not resize or release existing caches.
    pub fn set_search_options(&mut self, options: MatchSearchOptions) -> Result<()> {
        self.check_configuration_boundary()?;
        SearchShape::new(
            options,
            self.options.strategy,
            1_u64 << self.options.window_log,
        )?;
        self.workspace.set_search_options(options);
        Ok(())
    }

    /// Replace supported settings before the first process call or after reset.
    ///
    /// Corresponds to the configuration-boundary role of
    /// `ZSTD_CCtx_setParametersUsingCCtxParams`, restricted to this encoder's
    /// implemented frame options and named Rust strategies. An active, finished,
    /// or failed session returns [`Error::InvalidState`] before validating the
    /// options. At a clean boundary, invalid options return
    /// [`Error::InvalidParameter`]. Either error leaves the entire context
    /// unchanged and does not poison a healthy session.
    ///
    /// This operation preserves separate search settings, dictionary/prefix
    /// selection, counters, buffers, and their allocator owners. It neither
    /// allocates nor resets a session.
    /// Unlike individual C effort setters, it cannot alter requested settings
    /// after frame processing starts, even if no input or output was transferred.
    pub fn set_options(&mut self, options: EncoderOptions) -> Result<()> {
        self.check_configuration_boundary()?;
        self.options = options.validate()?;
        Ok(())
    }

    /// Source bytes accepted since the last reset.
    pub fn total_input(&self) -> u64 {
        self.total_input
    }

    /// Encoded bytes delivered to callers since the last reset.
    pub fn total_output(&self) -> u64 {
        self.total_output
    }

    /// Retained search allocation capacity, including inactive strategy caches.
    ///
    /// Counts both Greedy position tables, both Chain head/node pairs shared
    /// by Chain and Lazy, and retained output history. It excludes dictionary
    /// payload/tables, staging, pending output, literal/sequence/entropy scratch,
    /// inline state, allocator bookkeeping and shared outer control blocks.
    ///
    /// This is an observed capacity, not requested logical bytes, a memory limit
    /// or a process-memory measurement. Previous larger configurations can keep
    /// a high-water allocation until the context is dropped.
    pub fn search_retained_capacity(&self) -> usize {
        self.workspace.search_retained_capacity()
    }

    /// Reachable heap capacity, excluding inline state and caller buffers.
    /// Includes [`Self::search_retained_capacity`] plus the other owned codec
    /// buffers and reachable dictionary allocations.
    /// Attached dictionary payloads are counted once even when borrowed or shared
    /// elsewhere. This is reachable capacity, not exclusive ownership; shared
    /// Arc/Rc outer storage and allocator bookkeeping are excluded.
    pub fn retained_capacity(&self) -> usize {
        self.block.capacity()
            + self.pending.capacity()
            + self.workspace.retained_capacity()
            + self
                .active_dictionary
                .as_ref()
                .or(self.dictionary.as_ref())
                .or(self.prefix.as_ref())
                .map_or(0, |dictionary| dictionary.view().retained_capacity())
    }

    /// Start another frame, retaining allocations, search settings and persistent
    /// frame settings.
    ///
    /// The per-frame source pledge is cleared. This also recovers a context
    /// poisoned by a processing error; corresponds to session-only `ZSTD_CCtx_reset`.
    /// Persistent dictionaries and unclaimed prefixes remain configured.
    pub fn reset(&mut self) {
        self.options.pledged_size = None;
        self.clear_frame();
    }

    /// Validate new frame options, then reset while retaining owned allocations
    /// and separate search settings. Invalid options leave the context untouched.
    pub fn reset_with_options(&mut self, options: EncoderOptions) -> Result<()> {
        self.options = options.validate()?;
        self.clear_frame();
        Ok(())
    }

    /// Restore default frame/search settings and remove dictionary/prefix selection.
    ///
    /// Corresponds to the supported role of `ZSTD_reset_parameters`. Only a
    /// clean configuration boundary or a completed frame accepts this operation.
    /// An active or failed session returns [`Error::InvalidState`] without any
    /// mutation or recovery. A completed session remains finished and retains
    /// its counters: call [`Self::reset`] before processing another frame.
    ///
    /// Defaults are Rust's [`EncoderOptions::default`] and
    /// [`MatchSearchOptions::default`], including checksum enabled and no source
    /// pledge. C's parameter-only reset can retain a
    /// fresh session's pledge and uses different checksum defaults.
    ///
    /// Every selected/active dictionary handle and cached seed is cleared,
    /// including workspace attachments. Owned allocations and their exact
    /// allocator values are retained; this neither allocates nor clones them.
    /// Clearing a borrowed view does not shorten the context type's lifetime.
    pub fn reset_parameters(&mut self) -> Result<()> {
        if self.phase != Phase::Finished {
            self.check_configuration_boundary()?;
        }
        self.options = EncoderOptions::default();
        self.workspace
            .set_search_options(MatchSearchOptions::default());
        self.dictionary = None;
        self.prefix = None;
        self.active_dictionary = None;
        self.workspace.reset();
        self.workspace.clear_dictionary_seeds();
        Ok(())
    }

    /// Discard the session, restore frame/search defaults, and remove
    /// dictionary/prefix state.
    ///
    /// Corresponds to `ZSTD_reset_session_and_parameters`. This accepts every
    /// phase, including processing failures, and returns a ready context with
    /// zero counters and no pending source, output, or directive. All reusable
    /// allocations retain their exact allocator owners without allocating or
    /// cloning. Dictionary-backed reachable capacity may fall when its handle
    /// is released; owned codec capacity is retained. A borrowed dictionary's
    /// compile-time lifetime remains part of the context type after its fields clear.
    pub fn reset_all(&mut self) {
        self.options = EncoderOptions::default();
        self.workspace
            .set_search_options(MatchSearchOptions::default());
        self.dictionary = None;
        self.prefix = None;
        self.clear_frame();
        self.workspace.clear_dictionary_seeds();
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
    /// Any error poisons the context until reset; output touched by a failing
    /// call must be discarded. After `FrameFinished`, only an
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
                    // The complete block capacity was reserved before accepting bytes.
                    for &byte in source {
                        self.block.push(byte);
                    }
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
        // Eighteen bytes cover every supported header field combination.
        for byte in MAGIC.to_le_bytes() {
            self.pending.push(byte);
        }
        self.active_dictionary = self.prefix.take().or_else(|| self.dictionary.clone());
        self.workspace.start_frame(
            self.active_dictionary.clone(),
            1_u64 << self.options.window_log,
        )?;
        let dictionary_id = self
            .active_dictionary
            .as_ref()
            .filter(|_| self.options.include_dictionary_id)
            .map_or(0, |dictionary| dictionary.view().id());
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
        for &byte in &dictionary_id.to_le_bytes()[..dictionary_bytes] {
            self.pending.push(byte);
        }
        for &byte in &value.to_le_bytes()[..count] {
            self.pending.push(byte);
        }
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
                // The initial reservation includes the four checksum bytes.
                for byte in (self.checksum.digest() as u32).to_le_bytes() {
                    self.pending.push(byte);
                }
            }
            self.phase = Phase::Finishing;
        }
        Ok(())
    }
}

impl Default for Encoder<Global, SharedDictionary> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Allocator + Clone, D: DictionaryHandle> fmt::Debug for Encoder<A, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Encoder")
            .field("options", &self.options)
            .field("search_options", &self.search_options())
            .field("phase", &self.phase)
            .field("total_input", &self.total_input)
            .field("total_output", &self.total_output)
            .field("retained_capacity", &self.retained_capacity())
            .field("search_retained_capacity", &self.search_retained_capacity())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameHeader, decompress};
    use alloc::vec;
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn small_search_options() -> MatchSearchOptions {
        let mut options = MatchSearchOptions::new();
        options.set_hash_log(10).unwrap();
        options.set_chain_log(10).unwrap();
        options.set_key_bytes(6).unwrap();
        options.set_chain_visits(3).unwrap();
        options.set_lazy_visits(5).unwrap();
        options.set_history_bytes(1024).unwrap();
        options
    }

    #[test]
    fn search_and_frame_setters_are_order_independent_and_keep_selected_handles() {
        let dictionary = crate::Dictionary::raw(b"selected dictionary bytes")
            .unwrap()
            .into_shared();
        for strategy in [
            CompressionStrategy::Greedy,
            CompressionStrategy::Chain,
            CompressionStrategy::Lazy,
        ] {
            for search_first in [false, true] {
                for prefix in [false, true] {
                    let frame = EncoderOptions {
                        strategy,
                        window_log: 10,
                        checksum: false,
                        pledged_size: Some(0),
                        include_content_size: false,
                        include_dictionary_id: false,
                    };
                    let search = small_search_options();
                    let mut encoder = Encoder::new();
                    assert_eq!(encoder.search_options(), MatchSearchOptions::default());
                    assert_eq!(encoder.search_retained_capacity(), 0);
                    if prefix {
                        encoder.set_prefix(dictionary.clone()).unwrap();
                    } else {
                        encoder.set_dictionary(Some(dictionary.clone())).unwrap();
                    }
                    let owners = SharedDictionary::strong_count(&dictionary);
                    let capacity = encoder.retained_capacity();
                    if search_first {
                        encoder.set_search_options(search).unwrap();
                        encoder.set_options(frame).unwrap();
                    } else {
                        encoder.set_options(frame).unwrap();
                        encoder.set_search_options(search).unwrap();
                    }
                    assert_eq!(encoder.options(), frame);
                    assert_eq!(encoder.search_options(), search);
                    assert_eq!(encoder.search_retained_capacity(), 0);
                    assert_eq!(encoder.retained_capacity(), capacity);
                    assert_eq!(SharedDictionary::strong_count(&dictionary), owners);
                    assert_eq!(encoder.prefix.is_some(), prefix);
                    assert_eq!(encoder.dictionary.is_some(), !prefix);
                    assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
                    assert_eq!(encoder.phase, Phase::Active);
                    assert!(!encoder.header_started);
                }
            }
        }
    }

    #[test]
    fn search_setter_rejects_every_started_phase_before_mutation() {
        let source = b"abcabcabcabcabcabc";
        for case in 0..9 {
            let mut encoder = Encoder::new();
            let search = small_search_options();
            encoder.set_search_options(search).unwrap();
            match case {
                0 => {
                    encoder
                        .process(&[], &mut [], EndDirective::Continue)
                        .unwrap();
                }
                1 => {
                    encoder
                        .process(&[], &mut [0; 1], EndDirective::Continue)
                        .unwrap();
                }
                2 => {
                    encoder
                        .process(source, &mut [0; 64], EndDirective::Continue)
                        .unwrap();
                }
                3 => {
                    encoder
                        .process(source, &mut [0; 64], EndDirective::Flush)
                        .unwrap();
                }
                4 => {
                    encoder
                        .process(source, &mut [], EndDirective::Flush)
                        .unwrap();
                }
                5 => {
                    encoder
                        .process(source, &mut [0; 6], EndDirective::Finish)
                        .unwrap();
                    assert_eq!(encoder.phase, Phase::Finishing);
                }
                6 => {
                    finish(&mut encoder, source, 1);
                    assert_eq!(encoder.phase, Phase::Finished);
                }
                7 => {
                    encoder
                        .process(source, &mut [], EndDirective::Flush)
                        .unwrap();
                    assert_eq!(
                        encoder.process(&[], &mut [], EndDirective::Finish),
                        Err(Error::InvalidState)
                    );
                    assert_eq!(encoder.phase, Phase::Failed);
                }
                8 => {
                    encoder
                        .process(source, &mut [], EndDirective::Finish)
                        .unwrap();
                    assert_eq!(encoder.phase, Phase::Active);
                }
                _ => unreachable!(),
            }
            reject_configuration_without_mutation(
                &mut encoder,
                |encoder| encoder.set_search_options(MatchSearchOptions::default()),
                Error::InvalidState,
            );
            assert_eq!(encoder.search_options(), search);
            // A phase-only rejection does not recover or poison the context.
            encoder.reset();
            assert_eq!(encoder.search_options(), search);
            encoder
                .set_search_options(MatchSearchOptions::default())
                .unwrap();
            assert_eq!(
                finish(&mut encoder, source, 1),
                finish(&mut Encoder::new(), source, 7)
            );
        }
    }

    #[test]
    fn search_session_resets_preserve_settings_and_parameter_resets_restore_defaults() {
        let source = b"session search settings";
        let search = small_search_options();
        let mut encoder = Encoder::new();
        encoder.set_search_options(search).unwrap();
        encoder
            .set_options(EncoderOptions {
                pledged_size: Some(source.len() as u64),
                ..EncoderOptions::default()
            })
            .unwrap();
        finish(&mut encoder, source, 1);
        let capacity = encoder.retained_capacity();
        let search_capacity = encoder.search_retained_capacity();
        encoder.reset();
        assert_eq!(encoder.search_options(), search);
        assert_eq!(encoder.options().pledged_size, None);
        assert_eq!(encoder.retained_capacity(), capacity);
        assert_eq!(encoder.search_retained_capacity(), search_capacity);
        let frame = EncoderOptions {
            strategy: CompressionStrategy::Lazy,
            window_log: 10,
            checksum: false,
            ..EncoderOptions::default()
        };
        encoder.reset_with_options(frame).unwrap();
        assert_eq!(encoder.search_options(), search);
        assert_eq!(encoder.options(), frame);
        assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
        reject_configuration_without_mutation(
            &mut encoder,
            |encoder| {
                encoder.reset_with_options(EncoderOptions {
                    window_log: 9,
                    ..frame
                })
            },
            Error::InvalidParameter,
        );
        finish(&mut encoder, source, 1);
        let completed = (
            encoder.total_input(),
            encoder.total_output(),
            encoder.phase,
            encoder.header_started,
            encoder.search_retained_capacity(),
        );
        encoder.reset_parameters().unwrap();
        assert_eq!(encoder.search_options(), MatchSearchOptions::default());
        assert_eq!(encoder.options(), EncoderOptions::default());
        assert_eq!(
            (
                encoder.total_input(),
                encoder.total_output(),
                encoder.phase,
                encoder.header_started,
                encoder.search_retained_capacity()
            ),
            completed
        );
        assert_eq!(encoder.set_search_options(search), Err(Error::InvalidState));
        encoder.reset();
        encoder.set_search_options(search).unwrap();
        encoder.reset_all();
        assert_eq!(encoder.search_options(), MatchSearchOptions::default());
        assert_eq!(encoder.options(), EncoderOptions::default());
        assert_eq!(encoder.search_retained_capacity(), completed.4);
        assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
    }

    #[test]
    fn search_setters_keep_allocator_failures_armed_and_dictionary_domains_unchanged() {
        fn check<A: Allocator + Clone>(
            allocator: A,
            monitor: &zstdoxide_test_allocator::AllocationMonitor,
        ) {
            let dictionary_owner = tracking().into_non_clone();
            let dictionary_monitor = dictionary_owner.monitor();
            let dictionary =
                crate::Dictionary::raw_in(b"dictionary content", &dictionary_owner).unwrap();
            let mut encoder = Encoder::with_dictionary_in(
                EncoderOptions::default(),
                dictionary.view(),
                allocator,
            )
            .unwrap();
            encoder.set_search_options(small_search_options()).unwrap();
            finish(&mut encoder, b"abcabcabcabc", 1);
            encoder.reset();
            let before = monitor.snapshot();
            let dictionary_before = dictionary_monitor.snapshot();
            let capacities = (
                encoder.retained_capacity(),
                encoder.search_retained_capacity(),
            );
            let pointers = (encoder.block.as_ptr(), encoder.pending.as_ptr());
            let identity = (
                encoder.block.allocator() as *const A,
                encoder.pending.allocator() as *const A,
            );
            assert!(monitor.fail_next());
            let mut larger = small_search_options();
            larger.set_hash_log(18).unwrap();
            larger.set_chain_log(19).unwrap();
            larger.set_key_bytes(8).unwrap();
            larger.set_history_bytes(1 << 20).unwrap();
            encoder.set_search_options(larger).unwrap();
            encoder
                .set_options(EncoderOptions {
                    strategy: CompressionStrategy::Chain,
                    window_log: 20,
                    ..EncoderOptions::default()
                })
                .unwrap();
            assert_eq!(encoder.search_options(), larger);
            assert_eq!(
                (
                    encoder.retained_capacity(),
                    encoder.search_retained_capacity()
                ),
                capacities
            );
            assert_eq!((encoder.block.as_ptr(), encoder.pending.as_ptr()), pointers);
            assert_eq!(
                (
                    encoder.block.allocator() as *const A,
                    encoder.pending.allocator() as *const A
                ),
                identity
            );
            assert_eq!(
                encoder.dictionary.as_ref().unwrap().content(),
                dictionary.content()
            );
            assert_eq!(monitor.snapshot(), before);
            assert_eq!(dictionary_monitor.snapshot(), dictionary_before);
            assert_eq!(
                encoder.process(b"x", &mut [], EndDirective::Continue),
                Err(Error::AllocationFailed)
            );
            assert_eq!(encoder.phase, Phase::Failed);
            assert_eq!(monitor.snapshot().attempts, before.attempts + 1);
            let failed = monitor.snapshot();
            encoder.reset_all();
            assert_eq!(monitor.snapshot(), failed);
            assert_eq!(encoder.search_options(), MatchSearchOptions::default());
            assert!(encoder.dictionary.is_none());
            drop(encoder);
            monitor.assert_released();
            drop(dictionary);
            dictionary_monitor.assert_released();
        }
        let allocator = tracking();
        let monitor = allocator.monitor();
        check(allocator, &monitor);
        let allocator = tracking().into_non_clone();
        let monitor = allocator.monitor();
        check(&allocator, &monitor);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn search_capacity_tracks_inactive_cache_families_without_dictionary_or_staging() {
        let owner = tracking();
        let monitor = owner.monitor();
        let dictionary = crate::Dictionary::raw(b"abcdefghabcdefgh")
            .unwrap()
            .into_shared();
        let dictionary_capacity = dictionary.retained_capacity();
        let mut encoder =
            Encoder::with_dictionary_in(EncoderOptions::default(), dictionary.clone(), owner)
                .unwrap();
        assert_eq!(encoder.search_retained_capacity(), 0);
        assert_eq!(encoder.retained_capacity(), dictionary_capacity);
        let source = b"abcdefgh".repeat(256);
        let mut capacities = Vec::new();
        for (index, strategy) in [
            CompressionStrategy::Greedy,
            CompressionStrategy::Chain,
            CompressionStrategy::Lazy,
            CompressionStrategy::Greedy,
        ]
        .into_iter()
        .enumerate()
        {
            encoder
                .reset_with_options(EncoderOptions {
                    strategy,
                    ..EncoderOptions::default()
                })
                .unwrap();
            let mut search = small_search_options();
            if index == 1 {
                search.set_hash_log(11).unwrap();
                search.set_chain_log(11).unwrap();
            }
            search.set_key_bytes([4, 6, 8, 4][index]).unwrap();
            let retained = encoder.search_retained_capacity();
            let before = monitor.snapshot();
            encoder.set_search_options(search).unwrap();
            assert_eq!(encoder.search_retained_capacity(), retained);
            assert_eq!(monitor.snapshot(), before);
            let encoded = finish(&mut encoder, &source, 257);
            assert_eq!(
                crate::decompress_with_dictionary(
                    &encoded,
                    &dictionary,
                    crate::DecodeOptions::new(source.len())
                )
                .unwrap(),
                source
            );
            let narrow = encoder.search_retained_capacity();
            assert_eq!(narrow, encoder.workspace.search_retained_capacity());
            assert!(narrow > 0);
            assert!(encoder.retained_capacity() > narrow + dictionary_capacity);
            assert!(
                encoder.retained_capacity()
                    >= narrow
                        + dictionary_capacity
                        + encoder.block.capacity()
                        + encoder.pending.capacity()
            );
            assert_eq!(
                monitor.snapshot().live_bytes as usize,
                encoder.retained_capacity() - dictionary_capacity
            );
            capacities.push(narrow);
        }
        assert!(capacities[1] > capacities[0]);
        assert_eq!(capacities[2], capacities[1]);
        assert_eq!(capacities[3], capacities[2]);
        let total = encoder.retained_capacity();
        let before = monitor.snapshot();
        encoder.reset_parameters().unwrap();
        assert_eq!(encoder.search_retained_capacity(), capacities[3]);
        assert_eq!(encoder.retained_capacity(), total - dictionary_capacity);
        assert_eq!(monitor.snapshot(), before);
        drop(encoder);
        monitor.assert_released();
    }

    #[test]
    fn set_options_applies_supported_fields_and_preserves_selected_handles() {
        let dictionary = crate::Dictionary::from_bytes(include_bytes!(
            "../tests/upstream/dictionaries/formatted.dict"
        ))
        .unwrap()
        .into_shared();
        let source = b"a supported configuration is applied before its frame starts";
        for (strategy, window_log) in [
            (CompressionStrategy::Greedy, 10),
            (CompressionStrategy::Chain, 17),
            (CompressionStrategy::Lazy, 31),
        ] {
            for flags in 0..8 {
                let options = EncoderOptions {
                    strategy,
                    window_log,
                    checksum: flags & 1 != 0,
                    pledged_size: Some(source.len() as u64),
                    include_content_size: flags & 2 != 0,
                    include_dictionary_id: flags & 4 != 0,
                };
                let mut encoder = Encoder::new();
                let mut reference = Encoder::with_options(options).unwrap();
                if flags & 1 == 0 {
                    encoder.set_dictionary(Some(dictionary.clone())).unwrap();
                    reference.set_dictionary(Some(dictionary.clone())).unwrap();
                } else {
                    encoder.set_prefix(dictionary.clone()).unwrap();
                    reference.set_prefix(dictionary.clone()).unwrap();
                }
                let owners = SharedDictionary::strong_count(&dictionary);
                encoder.set_options(options).unwrap();
                assert_eq!(encoder.options(), options);
                assert_eq!(SharedDictionary::strong_count(&dictionary), owners);
                assert_eq!(encoder.dictionary.is_some(), flags & 1 == 0);
                assert_eq!(encoder.prefix.is_some(), flags & 1 != 0);
                let encoded = finish(&mut encoder, source, 1);
                assert_eq!(encoded, finish(&mut reference, source, 7));
                let header = FrameHeader::parse(&encoded).unwrap().0;
                assert_eq!(header.window_size, 1_u64 << window_log);
                assert_eq!(header.has_checksum, options.checksum);
                assert_eq!(
                    header.content_size,
                    options
                        .pledged_size
                        .filter(|_| options.include_content_size)
                );
                assert_eq!(
                    header.dictionary_id,
                    if options.include_dictionary_id {
                        dictionary.id()
                    } else {
                        0
                    }
                );
                assert_eq!(
                    crate::decompress_with_dictionary(
                        &encoded,
                        &dictionary,
                        crate::DecodeOptions {
                            max_window_size: 1_u64 << window_log,
                            ..crate::DecodeOptions::new(source.len())
                        }
                    )
                    .unwrap(),
                    source
                );
            }
        }
    }

    /// Snapshot plain values, caller-visible state, and owned byte contents.
    /// All snapshot allocations belong to the test's Global domain. Dictionary
    /// addresses are compared only; the snapshot never dereferences a pointer.
    fn reject_options_without_mutation<A: Allocator + Clone>(
        encoder: &mut Encoder<A>,
        options: EncoderOptions,
        error: Error,
    ) {
        reject_configuration_without_mutation(
            encoder,
            |encoder| encoder.set_options(options),
            error,
        );
    }

    fn reject_configuration_without_mutation<A: Allocator + Clone>(
        encoder: &mut Encoder<A>,
        operation: impl FnOnce(&mut Encoder<A>) -> Result<()>,
        error: Error,
    ) {
        let snapshot = |encoder: &Encoder<A>| {
            (
                (encoder.options, encoder.search_options()),
                (
                    encoder.phase,
                    encoder.header_started,
                    encoder.checksum.digest(),
                ),
                (encoder.total_input, encoder.total_output),
                encoder
                    .latched
                    .map(|latch| (latch.directive, latch.remaining)),
                (
                    encoder.block.to_vec(),
                    encoder.pending.to_vec(),
                    encoder.pending_position,
                ),
                (encoder.block.as_ptr(), encoder.pending.as_ptr()),
                (
                    encoder.block.capacity(),
                    encoder.pending.capacity(),
                    encoder.workspace.retained_capacity(),
                    encoder.search_retained_capacity(),
                ),
                [
                    encoder.dictionary.as_ref().map(SharedDictionary::as_ptr),
                    encoder.prefix.as_ref().map(SharedDictionary::as_ptr),
                    encoder
                        .active_dictionary
                        .as_ref()
                        .map(SharedDictionary::as_ptr),
                ],
            )
        };
        let before = snapshot(encoder);
        assert_eq!(operation(encoder), Err(error));
        assert_eq!(snapshot(encoder), before);
    }

    #[test]
    fn set_options_rejects_each_started_phase_without_poisoning_or_mutation() {
        let source = b"abcabcabcabcabcabc";
        let dictionary = crate::Dictionary::from_bytes(include_bytes!(
            "../tests/upstream/dictionaries/formatted.dict"
        ))
        .unwrap()
        .into_shared();
        for case in 0..9 {
            let mut encoder =
                Encoder::with_dictionary(EncoderOptions::default(), dictionary.clone()).unwrap();
            let mut encoded = Vec::new();
            let has_source;
            if case == 7 {
                encoded = finish(&mut encoder, source, 1);
                has_source = true;
                assert_eq!(encoder.phase, Phase::Finished);
            } else {
                let (input, capacity, directive) = match case {
                    0 => (&[][..], 0, EndDirective::Continue),
                    1 => (&[][..], 1, EndDirective::Continue),
                    2 => (&source[..], 64, EndDirective::Continue),
                    3 => (&source[..], 64, EndDirective::Flush),
                    4 | 8 => (&source[..], 0, EndDirective::Flush),
                    5 => (&source[..], 0, EndDirective::Finish),
                    // The formatted ID has four bytes, so the full header is
                    // ten bytes. Exhaust output exactly before the last block.
                    6 => (&source[..], 10, EndDirective::Finish),
                    _ => unreachable!(),
                };
                let mut output = [0; 64];
                let progress = encoder
                    .process(input, &mut output[..capacity], directive)
                    .unwrap();
                assert_eq!(progress.consumed, input.len());
                encoded.extend_from_slice(&output[..progress.produced]);
                has_source = !input.is_empty();
                assert!(encoder.header_started);
                assert_eq!(
                    encoder.phase,
                    if case == 6 {
                        Phase::Finishing
                    } else {
                        Phase::Active
                    }
                );
                if case == 8 {
                    assert_eq!(
                        encoder.process(&[], &mut [], EndDirective::Finish),
                        Err(Error::InvalidState)
                    );
                    assert_eq!(encoder.phase, Phase::Failed);
                }
            }
            for window_log in [9, 10, 32, 255] {
                reject_options_without_mutation(
                    &mut encoder,
                    EncoderOptions {
                        window_log,
                        strategy: CompressionStrategy::Lazy,
                        checksum: false,
                        pledged_size: Some(999),
                        include_content_size: false,
                        include_dictionary_id: false,
                    },
                    Error::InvalidState,
                );
            }
            if case == 8 {
                assert_eq!(
                    encoder.process(&[], &mut [], EndDirective::Finish),
                    Err(Error::InvalidState)
                );
                encoder.reset();
                encoded = finish(&mut encoder, source, 1);
            } else {
                if case == 4 {
                    let mut output = [0; 128];
                    let progress = encoder
                        .process(&[], &mut output, EndDirective::Flush)
                        .unwrap();
                    assert_eq!(progress.status, StreamStatus::Flushed);
                    encoded.extend_from_slice(&output[..progress.produced]);
                }
                encoded.extend_from_slice(&finish(
                    &mut encoder,
                    if has_source { &[] } else { source },
                    1,
                ));
            }
            assert_eq!(
                crate::decompress_with_dictionary(
                    &encoded,
                    &dictionary,
                    crate::DecodeOptions::new(source.len())
                )
                .unwrap(),
                source
            );
        }
    }

    #[test]
    fn set_options_invalid_values_are_atomic_and_pledge_policy_is_unchanged() {
        let mut encoder = Encoder::new();
        for window_log in [0, 9, 32, 255] {
            reject_options_without_mutation(
                &mut encoder,
                EncoderOptions {
                    window_log,
                    checksum: false,
                    pledged_size: Some(0),
                    ..EncoderOptions::default()
                },
                Error::InvalidParameter,
            );
        }
        // A pledge is a frame contract, not the reference query's SourceSize
        // sentinel. This setter must preserve the existing live options domain.
        let options = EncoderOptions {
            pledged_size: Some(u64::MAX),
            ..EncoderOptions::default()
        };
        encoder.set_options(options).unwrap();
        assert_eq!(encoder.options(), options);
        encoder.set_options(EncoderOptions::default()).unwrap();
        assert_eq!(
            finish(&mut encoder, b"default", 1),
            finish(&mut Encoder::new(), b"default", 1)
        );
    }

    #[test]
    fn set_options_preserves_custom_owners_capacity_and_pending_allocation_failure() {
        fn check<A: Allocator + Clone>(
            allocator: A,
            monitor: &zstdoxide_test_allocator::AllocationMonitor,
        ) {
            let dictionary_allocator = tracking().into_non_clone();
            let dictionary_monitor = dictionary_allocator.monitor();
            let dictionary =
                crate::Dictionary::raw_in(b"dictionary content", &dictionary_allocator).unwrap();
            let mut encoder = Encoder::with_dictionary_in(
                EncoderOptions::default(),
                dictionary.view(),
                allocator,
            )
            .unwrap();
            finish(&mut encoder, b"abcabcabcabc", 1);
            encoder.reset();
            let before = monitor.snapshot();
            let dictionary_before = dictionary_monitor.snapshot();
            let capacity = encoder.retained_capacity();
            let pointers = (encoder.block.as_ptr(), encoder.pending.as_ptr());
            assert!(monitor.fail_next());
            for strategy in [
                CompressionStrategy::Chain,
                CompressionStrategy::Lazy,
                CompressionStrategy::Greedy,
            ] {
                let options = EncoderOptions {
                    strategy,
                    window_log: 10,
                    checksum: false,
                    ..EncoderOptions::default()
                };
                encoder.set_options(options).unwrap();
                assert_eq!(encoder.options(), options);
                assert_eq!(
                    encoder.dictionary.as_ref().unwrap().content(),
                    dictionary.content()
                );
                assert_eq!(encoder.total_input(), 0);
                assert_eq!(encoder.total_output(), 0);
                assert_eq!(encoder.retained_capacity(), capacity);
                assert_eq!((encoder.block.as_ptr(), encoder.pending.as_ptr()), pointers);
                assert_eq!(monitor.snapshot(), before);
                assert_eq!(dictionary_monitor.snapshot(), dictionary_before);
            }
            // Selecting Chain does not construct it. Its first frame setup is
            // still the next allocation and must see the still-armed failure.
            encoder
                .set_options(EncoderOptions {
                    strategy: CompressionStrategy::Chain,
                    ..EncoderOptions::default()
                })
                .unwrap();
            assert_eq!(
                encoder.process(b"x", &mut [], EndDirective::Continue),
                Err(Error::AllocationFailed)
            );
            drop(encoder);
            monitor.assert_released();
            drop(dictionary);
            dictionary_monitor.assert_released();
        }
        let allocator = tracking();
        let monitor = allocator.monitor();
        check(allocator, &monitor);
        let allocator = tracking().into_non_clone();
        let monitor = allocator.monitor();
        check(&allocator, &monitor);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn parameter_reset_preserves_completed_session_but_all_reset_recovers_every_phase() {
        let source = b"abcabcabcabcabcabc";
        for case in 0..10 {
            let mut encoder = Encoder::with_options(EncoderOptions {
                strategy: CompressionStrategy::Lazy,
                window_log: 10,
                checksum: false,
                pledged_size: Some(source.len() as u64),
                include_content_size: false,
                include_dictionary_id: false,
            })
            .unwrap();
            encoder.set_search_options(small_search_options()).unwrap();
            match case {
                0 => {} // A fresh context accepts parameter-only reset.
                1 => {
                    encoder
                        .process(&[], &mut [], EndDirective::Continue)
                        .unwrap();
                }
                2 => {
                    encoder
                        .process(&[], &mut [0; 1], EndDirective::Continue)
                        .unwrap();
                }
                3 => {
                    encoder
                        .process(source, &mut [0; 64], EndDirective::Continue)
                        .unwrap();
                }
                4 => {
                    assert_eq!(
                        encoder
                            .process(source, &mut [0; 64], EndDirective::Flush)
                            .unwrap()
                            .status,
                        StreamStatus::Flushed
                    );
                }
                5 => {
                    encoder
                        .process(source, &mut [], EndDirective::Flush)
                        .unwrap();
                }
                6 => {
                    encoder
                        .process(source, &mut [], EndDirective::Finish)
                        .unwrap();
                }
                7 => {
                    encoder
                        .process(source, &mut [0; 6], EndDirective::Finish)
                        .unwrap();
                    assert_eq!(encoder.phase, Phase::Finishing);
                }
                8 => {
                    finish(&mut encoder, source, 1);
                    assert_eq!(encoder.phase, Phase::Finished);
                }
                9 => {
                    assert_eq!(
                        encoder.process(&[], &mut [], EndDirective::Finish),
                        Err(Error::ContentSizeMismatch)
                    );
                    assert_eq!(encoder.phase, Phase::Failed);
                }
                _ => unreachable!(),
            }
            if case == 0 || case == 8 {
                let session = (
                    encoder.phase,
                    encoder.header_started,
                    encoder.total_input(),
                    encoder.total_output(),
                    encoder.checksum.digest(),
                );
                encoder.reset_parameters().unwrap();
                assert_eq!(encoder.options(), EncoderOptions::default());
                assert_eq!(encoder.search_options(), MatchSearchOptions::default());
                assert_eq!(
                    (
                        encoder.phase,
                        encoder.header_started,
                        encoder.total_input(),
                        encoder.total_output(),
                        encoder.checksum.digest()
                    ),
                    session
                );
                if case == 8 {
                    assert_eq!(
                        encoder
                            .process(&[], &mut [], EndDirective::Finish)
                            .unwrap()
                            .status,
                        StreamStatus::FrameFinished
                    );
                    assert_eq!(
                        encoder.set_options(EncoderOptions::default()),
                        Err(Error::InvalidState)
                    );
                }
            } else {
                reject_configuration_without_mutation(
                    &mut encoder,
                    Encoder::reset_parameters,
                    Error::InvalidState,
                );
            }
            let capacity = encoder.retained_capacity();
            let pointers = (encoder.block.as_ptr(), encoder.pending.as_ptr());
            encoder.reset_all();
            assert_eq!(encoder.options(), EncoderOptions::default());
            assert_eq!(encoder.search_options(), MatchSearchOptions::default());
            assert_eq!(encoder.phase, Phase::Active);
            assert!(!encoder.header_started);
            assert!(encoder.block.is_empty() && encoder.pending.is_empty());
            assert_eq!(encoder.pending_position, 0);
            assert!(encoder.latched.is_none());
            assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
            assert_eq!(encoder.checksum.digest(), XxHash64::new().digest());
            assert_eq!(encoder.retained_capacity(), capacity);
            assert_eq!((encoder.block.as_ptr(), encoder.pending.as_ptr()), pointers);
            assert_eq!(
                finish(&mut encoder, b"independent default frame", 1),
                finish(&mut Encoder::new(), b"independent default frame", 7)
            );
        }
    }

    #[test]
    fn parameter_resets_release_all_shared_dictionary_handles_without_reallocating() {
        let bytes = include_bytes!("../tests/upstream/dictionaries/formatted.dict");
        for strategy in [CompressionStrategy::Greedy, CompressionStrategy::Chain] {
            for prefix in [false, true] {
                // Fresh and Finished allow parameter-only reset; the two
                // in-progress cases require full reset and retain workspace
                // handles even after a single-use prefix has been claimed.
                for phase in 0..4 {
                    let dictionary_allocator = tracking();
                    let dictionary_monitor = dictionary_allocator.monitor();
                    let dictionary = crate::Dictionary::from_bytes_in(bytes, dictionary_allocator)
                        .unwrap()
                        .into_shared();
                    let weak = SharedDictionary::downgrade(&dictionary);
                    let dictionary_capacity = dictionary.retained_capacity();
                    let owner = tracking();
                    let monitor = owner.monitor();
                    let mut encoder = Encoder::with_dictionary_in(
                        EncoderOptions {
                            strategy,
                            ..EncoderOptions::default()
                        },
                        dictionary.clone(),
                        owner,
                    )
                    .unwrap();
                    if prefix {
                        encoder.set_prefix(dictionary.clone()).unwrap();
                    }
                    drop(dictionary);
                    match phase {
                        0 => {}
                        1 => {
                            finish(&mut encoder, b"dictionary frame", 1);
                        }
                        2 => {
                            encoder
                                .process(b"dictionary frame", &mut [], EndDirective::Continue)
                                .unwrap();
                        }
                        3 => {
                            encoder
                                .process(b"dictionary frame", &mut [], EndDirective::Flush)
                                .unwrap();
                            assert_eq!(
                                encoder.process(&[], &mut [], EndDirective::Finish),
                                Err(Error::InvalidState)
                            );
                        }
                        _ => unreachable!(),
                    }
                    // A completed prefix frame already released its last handle.
                    assert_eq!(weak.upgrade().is_some(), !(prefix && phase == 1));
                    let selected_capacity = if weak.upgrade().is_some() {
                        dictionary_capacity
                    } else {
                        0
                    };
                    let before_capacity = encoder.retained_capacity();
                    let before = monitor.snapshot();
                    let pointers = (encoder.block.as_ptr(), encoder.pending.as_ptr());
                    assert!(monitor.fail_next());
                    if phase < 2 {
                        encoder.reset_parameters().unwrap();
                    } else {
                        encoder.reset_all();
                    }
                    assert!(
                        encoder.dictionary.is_none()
                            && encoder.prefix.is_none()
                            && encoder.active_dictionary.is_none()
                    );
                    assert!(
                        weak.upgrade().is_none(),
                        "a workspace must not retain a cleared shared handle"
                    );
                    dictionary_monitor.assert_released();
                    assert_eq!(
                        dictionary_monitor.snapshot().identities_created,
                        dictionary_monitor.snapshot().owners_dropped
                    );
                    assert_eq!(
                        encoder.retained_capacity(),
                        before_capacity - selected_capacity
                    );
                    assert_eq!((encoder.block.as_ptr(), encoder.pending.as_ptr()), pointers);
                    assert_eq!(monitor.snapshot(), before);
                    // Reset did not consume the pending request. Borrowing the
                    // retained exact owner needs no fresh allocator clone.
                    let mut witness = Vec::<u8, _>::new_in(encoder.block.allocator());
                    assert!(witness.try_reserve_exact(1).is_err());
                    drop(witness);
                    assert_eq!(monitor.snapshot().attempts, before.attempts + 1);
                    if phase == 1 {
                        assert_eq!(encoder.phase, Phase::Finished);
                        encoder.reset();
                    }
                    let encoded = finish(&mut encoder, b"without dictionary", 1);
                    assert_eq!(
                        encoded,
                        finish(&mut Encoder::new(), b"without dictionary", 7)
                    );
                    assert_eq!(decompress(&encoded, 18).unwrap(), b"without dictionary");
                    drop(encoder);
                    monitor.assert_released();
                    assert_eq!(
                        monitor.snapshot().identities_created,
                        monitor.snapshot().owners_dropped
                    );
                }
            }
        }
    }

    #[test]
    fn parameter_reset_all_recovers_allocation_failure_before_and_after_prefix_claim() {
        for ordinal in [1, 2] {
            let dictionary_owner = tracking().into_non_clone();
            let dictionary_monitor = dictionary_owner.monitor();
            let dictionary = crate::Dictionary::raw_in(b"prefix", dictionary_owner)
                .unwrap()
                .into_shared();
            let weak = SharedDictionary::downgrade(&dictionary);
            let owner = TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 128,
                fail_at: NonZeroU64::new(ordinal),
            })
            .unwrap();
            let monitor = owner.monitor();
            let mut encoder = Encoder::with_dictionary_in(
                EncoderOptions {
                    checksum: false,
                    window_log: 10,
                    ..EncoderOptions::default()
                },
                dictionary.clone(),
                owner,
            )
            .unwrap();
            encoder.set_prefix(dictionary).unwrap();
            assert_eq!(
                encoder.process(b"xxx", &mut [], EndDirective::Continue),
                Err(Error::AllocationFailed)
            );
            assert_eq!(encoder.phase, Phase::Failed);
            assert_eq!(encoder.prefix.is_some(), ordinal == 1);
            assert_eq!(encoder.active_dictionary.is_some(), ordinal == 2);
            assert_eq!(encoder.reset_parameters(), Err(Error::InvalidState));
            assert!(weak.upgrade().is_some());
            let before = monitor.snapshot();
            let owned_capacity = encoder.block.capacity()
                + encoder.pending.capacity()
                + encoder.workspace.retained_capacity();
            encoder.reset_all();
            assert_eq!(monitor.snapshot(), before);
            assert_eq!(encoder.retained_capacity(), owned_capacity);
            assert!(weak.upgrade().is_none());
            dictionary_monitor.assert_released();
            assert_eq!(encoder.options(), EncoderOptions::default());
            assert_eq!((encoder.total_input(), encoder.total_output()), (0, 0));
            let encoded = finish(&mut encoder, b"xxx", 1);
            assert_eq!(encoded, finish(&mut Encoder::new(), b"xxx", 1));
            assert_eq!(decompress(&encoded, 3).unwrap(), b"xxx");
            drop(encoder);
            monitor.assert_released();
            assert_eq!(
                monitor.snapshot().identities_created,
                monitor.snapshot().owners_dropped
            );
        }
    }

    #[test]
    fn parameter_resets_clear_borrowed_prefixes_without_changing_owner_domains() {
        let dictionary_owner = tracking().into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let dictionary =
            crate::Dictionary::raw_in(b"borrowed prefix content", &dictionary_owner).unwrap();
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let mut encoder =
            Encoder::with_dictionary_in(EncoderOptions::default(), dictionary.view(), &owner)
                .unwrap();
        for claimed in [false, true] {
            encoder.reset_all();
            encoder.set_prefix(dictionary.view()).unwrap();
            // Existing session-only reset preserves an unclaimed prefix.
            encoder.reset();
            assert!(encoder.prefix.is_some());
            if claimed {
                encoder
                    .process(&[], &mut [], EndDirective::Continue)
                    .unwrap();
                assert!(encoder.prefix.is_none() && encoder.active_dictionary.is_some());
            }
            let before = monitor.snapshot();
            let dictionary_before = dictionary_monitor.snapshot();
            if claimed {
                encoder.reset_all();
            } else {
                encoder.reset_parameters().unwrap();
            }
            assert!(
                encoder.dictionary.is_none()
                    && encoder.prefix.is_none()
                    && encoder.active_dictionary.is_none()
            );
            assert_eq!(monitor.snapshot(), before);
            assert_eq!(dictionary_monitor.snapshot(), dictionary_before);
            let encoded = finish(&mut encoder, b"borrow no longer selected", 1);
            assert_eq!(
                encoded,
                finish(&mut Encoder::new(), b"borrow no longer selected", 1)
            );
        }
        // Runtime clearing does not shorten D's borrowed lifetime: keep the
        // dictionary owner alive until the typed context has been dropped.
        drop(encoder);
        monitor.assert_released();
        assert_eq!(monitor.snapshot().identities_created, 1);
        drop(owner);
        drop(dictionary);
        dictionary_monitor.assert_released();
    }

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

    fn finish<A: Allocator + Clone, D: DictionaryHandle>(
        encoder: &mut Encoder<A, D>,
        source: &[u8],
        output_chunk: usize,
    ) -> Vec<u8> {
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
    fn tracking() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 128,
            ..AllocatorConfig::default()
        })
        .unwrap()
    }

    /// All sources and returned bytes belong to the test, outside codec owners.
    /// Flush boundaries and periodic empty output exercise resumed directives.
    fn fragmented<A: Allocator + Clone, D: DictionaryHandle>(
        encoder: &mut Encoder<A, D>,
        source: &[u8],
        input_chunk: usize,
        output_chunk: usize,
        flush: bool,
    ) -> Result<Vec<u8>> {
        assert!(input_chunk > 0 && (1..=257).contains(&output_chunk));
        let mut encoded = Vec::new();
        let mut position = 0;
        let mut calls = 0;
        loop {
            let end = source.len().min(position + input_chunk);
            let directive = if end == source.len() {
                EndDirective::Finish
            } else if flush {
                EndDirective::Flush
            } else {
                EndDirective::Continue
            };
            loop {
                let capacity = if calls % 7 == 0 { 0 } else { output_chunk };
                let mut guarded = [0xa5; 259];
                let result = encoder.process(
                    &source[position..end],
                    &mut guarded[1..1 + capacity],
                    directive,
                );
                assert_eq!(guarded[0], 0xa5);
                assert!(guarded[1 + capacity..].iter().all(|&byte| byte == 0xa5));
                let progress = result?;
                assert!(progress.consumed <= end - position);
                assert!(progress.produced <= capacity);
                assert!(
                    guarded[1 + progress.produced..]
                        .iter()
                        .all(|&byte| byte == 0xa5)
                );
                encoded.extend_from_slice(&guarded[1..1 + progress.produced]);
                position += progress.consumed;
                calls += 1;
                assert!(calls <= 32 * (source.len() + 1024));
                match progress.status {
                    StreamStatus::FrameFinished => {
                        assert_eq!(position, source.len());
                        assert_eq!(encoder.total_input(), source.len() as u64);
                        assert_eq!(encoder.total_output(), encoded.len() as u64);
                        return Ok(encoded);
                    }
                    StreamStatus::NeedInput | StreamStatus::Flushed => {
                        assert_eq!(position, end);
                        break;
                    }
                    StreamStatus::NeedOutput => {}
                }
            }
        }
    }

    #[test]
    fn allocator_constructor_validation_precedes_clones_and_reset_keeps_buffer_owners() {
        for with_dictionary in [false, true] {
            let allocator = tracking();
            let monitor = allocator.monitor();
            let options = EncoderOptions {
                window_log: 9,
                ..EncoderOptions::default()
            };
            let result = if with_dictionary {
                Encoder::with_dictionary_in(
                    options,
                    crate::Dictionary::raw(b"").unwrap().into_shared(),
                    allocator,
                )
            } else {
                Encoder::with_options_in(options, allocator)
            };
            assert!(matches!(result, Err(Error::InvalidParameter)));
            assert_eq!(monitor.snapshot().attempts, 0);
            assert_eq!(monitor.snapshot().identities_created, 1);
            assert_eq!(monitor.snapshot().owners_dropped, 1);
            monitor.assert_released();
        }
        let allocator = tracking();
        let identity = allocator.identity();
        let monitor = allocator.monitor();
        let mut encoder = Encoder::new_in(allocator);
        assert_eq!(monitor.snapshot().attempts, 0);
        let block_identity = encoder.block.allocator().identity();
        let pending_identity = encoder.pending.allocator().identity();
        assert_ne!(block_identity, pending_identity);
        assert_ne!(block_identity, identity);
        assert_ne!(pending_identity, identity);
        let expected = fragmented(&mut encoder, b"xxx", 3, 1, false).unwrap();
        let before = monitor.snapshot();
        let capacity = encoder.retained_capacity();
        assert_eq!(before.live_bytes, capacity as u128);
        encoder.reset();
        assert_eq!(encoder.retained_capacity(), capacity);
        assert_eq!(encoder.block.allocator().identity(), block_identity);
        assert_eq!(encoder.pending.allocator().identity(), pending_identity);
        assert_eq!(
            fragmented(&mut encoder, b"xxx", 3, 7, false).unwrap(),
            expected
        );
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        assert_eq!(
            monitor.snapshot().identities_created,
            before.identities_created
        );
        drop(encoder);
        monitor.assert_released();
        assert_eq!(
            monitor.snapshot().owners_dropped,
            monitor.snapshot().identities_created
        );
    }

    #[test]
    fn allocator_stream_bytes_preserve_the_pre_migration_baseline() {
        // Captured with the Global stream implementation before this migration;
        // lengths delimit every frame in the fingerprint's byte sequence.
        let dictionary = crate::Dictionary::from_bytes(include_bytes!(
            "../tests/upstream/dictionaries/formatted.dict"
        ))
        .unwrap()
        .into_shared();
        let mut fingerprint = 0xcbf29ce484222325_u64;
        let mut cases = 0;
        for strategy in [
            CompressionStrategy::Greedy,
            CompressionStrategy::Chain,
            CompressionStrategy::Lazy,
        ] {
            for length in [0, 1, 17, 1024, 2053, 131089] {
                for kind in 0..3 {
                    for selected in [false, true] {
                        let input: Vec<u8> = (0..length)
                            .map(|i| match kind {
                                0 => b'x',
                                1 => ((i * 17 + i / 13) % 251) as u8,
                                _ => ((i % 11) * 19) as u8,
                            })
                            .collect();
                        let options = EncoderOptions {
                            strategy,
                            window_log: 10,
                            pledged_size: Some(length as u64),
                            ..EncoderOptions::default()
                        };
                        let owner = tracking();
                        let monitor = owner.monitor();
                        let mut encoder = Encoder::with_options_in(options, owner).unwrap();
                        if selected {
                            encoder.set_dictionary(Some(dictionary.clone())).unwrap();
                        }
                        let encoded = finish(&mut encoder, &input, 129);
                        for byte in (encoded.len() as u64)
                            .to_le_bytes()
                            .iter()
                            .chain(encoded.iter())
                        {
                            fingerprint =
                                (fingerprint ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
                        }
                        drop(encoder);
                        monitor.assert_released();
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 108);
        assert_eq!(fingerprint, 7_858_392_785_807_785_414);
    }

    #[test]
    fn allocator_borrowed_context_and_dictionary_are_independent_across_flush_schedules() {
        for bytes in [
            b"".as_slice(),
            b"raw source dictionary data",
            include_bytes!("../tests/upstream/dictionaries/formatted.dict").as_slice(),
        ] {
            let dictionary_owner = tracking().into_non_clone();
            let dictionary_monitor = dictionary_owner.monitor();
            let dictionary = crate::Dictionary::from_bytes_in(bytes, &dictionary_owner).unwrap();
            let before = dictionary_monitor.snapshot();
            assert!(dictionary_monitor.fail_next());
            let input: Vec<u8> = (0..2053)
                .map(|index| {
                    dictionary
                        .content()
                        .get(index % dictionary.content().len().max(1))
                        .copied()
                        .unwrap_or((index % 113) as u8)
                })
                .collect();
            for strategy in [
                CompressionStrategy::Greedy,
                CompressionStrategy::Chain,
                CompressionStrategy::Lazy,
            ] {
                for (input_chunk, output_chunk, flush) in
                    [(1, 1, false), (127, 7, true), (1024, 257, false)]
                {
                    let options = EncoderOptions {
                        strategy,
                        window_log: 10,
                        ..EncoderOptions::default()
                    };
                    let mut global =
                        Encoder::with_dictionary_in(options, dictionary.view(), Global).unwrap();
                    let expected =
                        fragmented(&mut global, &input, input_chunk, output_chunk, flush).unwrap();
                    let owner = tracking().into_non_clone();
                    let monitor = owner.monitor();
                    let mut encoder =
                        Encoder::with_dictionary_in(options, dictionary.view(), &owner).unwrap();
                    let actual =
                        fragmented(&mut encoder, &input, input_chunk, output_chunk, flush).unwrap();
                    assert_eq!(actual, expected);
                    assert_eq!(
                        crate::decompress_with_dictionary_in(
                            &actual,
                            &dictionary.view(),
                            crate::DecodeOptions::new(input.len()),
                            Global
                        )
                        .unwrap(),
                        input
                    );
                    encoder.reset();
                    assert_eq!(
                        fragmented(&mut encoder, &input, input_chunk, output_chunk, flush).unwrap(),
                        expected
                    );
                    assert_eq!(monitor.snapshot().identities_created, 1);
                    drop(encoder);
                    monitor.assert_released();
                }
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
    fn allocator_shared_dictionary_outlives_original_handle_and_releases_on_replacement() {
        let dictionary_owner = tracking();
        let dictionary_monitor = dictionary_owner.monitor();
        let dictionary = crate::Dictionary::from_bytes_in(
            include_bytes!("../tests/upstream/dictionaries/formatted.dict"),
            dictionary_owner,
        )
        .unwrap()
        .into_shared();
        let weak = SharedDictionary::downgrade(&dictionary);
        let before = dictionary_monitor.snapshot();
        let allocator = tracking();
        let monitor = allocator.monitor();
        let mut encoder =
            Encoder::with_dictionary_in(EncoderOptions::default(), dictionary, allocator).unwrap();
        let encoded = fragmented(
            &mut encoder,
            b"some source bytes repeated some source bytes repeated",
            11,
            7,
            true,
        )
        .unwrap();
        assert_eq!(
            FrameHeader::parse(&encoded).unwrap().0.dictionary_id,
            0x12345678
        );
        assert!(weak.upgrade().is_some());
        encoder.reset();
        assert_eq!(dictionary_monitor.snapshot().attempts, before.attempts);
        assert_eq!(
            dictionary_monitor.snapshot().identities_created,
            before.identities_created
        );
        encoder.set_dictionary(None).unwrap();
        assert!(weak.upgrade().is_none());
        dictionary_monitor.assert_released();
        drop(encoder);
        monitor.assert_released();
    }

    #[test]
    fn allocator_every_observed_stream_failure_poison_resets_and_releases() {
        for bytes in [
            b"".as_slice(),
            include_bytes!("../tests/upstream/dictionaries/formatted.dict").as_slice(),
        ] {
            let dictionary = crate::Dictionary::from_bytes(bytes).unwrap();
            let input: Vec<u8> = (0..1537).map(|i| ((i % 131) * 17) as u8).collect();
            for strategy in [
                CompressionStrategy::Greedy,
                CompressionStrategy::Chain,
                CompressionStrategy::Lazy,
            ] {
                let options = EncoderOptions {
                    strategy,
                    window_log: 10,
                    ..EncoderOptions::default()
                };
                let owner = tracking();
                let monitor = owner.monitor();
                let mut baseline =
                    Encoder::with_dictionary_in(options, dictionary.view(), owner).unwrap();
                let expected = fragmented(&mut baseline, &input, 701, 257, true).unwrap();
                let attempts = monitor.snapshot().attempts;
                assert!(attempts >= 8);
                drop(baseline);
                monitor.assert_released();
                for ordinal in 1..=attempts {
                    let owner = TrackingAllocator::new(AllocatorConfig {
                        max_live_allocations: 128,
                        fail_at: NonZeroU64::new(ordinal),
                    })
                    .unwrap();
                    let monitor = owner.monitor();
                    let mut encoder = Encoder::with_dictionary_in(
                        EncoderOptions::default(),
                        dictionary.view(),
                        owner,
                    )
                    .unwrap();
                    encoder.set_options(options).unwrap();
                    assert_eq!(
                        fragmented(&mut encoder, &input, 701, 257, true),
                        Err(Error::AllocationFailed),
                        "{strategy:?} request {ordinal}"
                    );
                    assert_eq!(monitor.snapshot().attempts, ordinal);
                    assert_eq!(monitor.snapshot().failures, 1);
                    assert_eq!(
                        encoder.process(&[], &mut [], EndDirective::Finish),
                        Err(Error::InvalidState)
                    );
                    assert_eq!(encoder.set_dictionary(None), Err(Error::InvalidState));
                    assert_eq!(
                        encoder.set_prefix(dictionary.view()),
                        Err(Error::InvalidState)
                    );
                    encoder.reset();
                    assert_eq!(encoder.total_input(), 0);
                    assert_eq!(encoder.total_output(), 0);
                    assert!(encoder.dictionary.is_some());
                    assert!(encoder.active_dictionary.is_none());
                    assert_eq!(
                        fragmented(&mut encoder, &input, 701, 257, true).unwrap(),
                        expected
                    );
                    drop(encoder);
                    monitor.assert_released();
                }
            }
        }
    }

    #[test]
    fn allocator_header_failure_claims_prefix_only_after_pending_storage_succeeds() {
        for ordinal in [1, 2] {
            let dictionary_owner = tracking().into_non_clone();
            let dictionary_monitor = dictionary_owner.monitor();
            let dictionary = crate::Dictionary::raw_in(b"prefix", dictionary_owner)
                .unwrap()
                .into_shared();
            let weak = SharedDictionary::downgrade(&dictionary);
            let owner = TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 128,
                fail_at: NonZeroU64::new(ordinal),
            })
            .unwrap();
            let monitor = owner.monitor();
            let mut encoder =
                Encoder::with_dictionary_in(EncoderOptions::default(), dictionary.clone(), owner)
                    .unwrap();
            encoder.set_prefix(dictionary).unwrap();
            assert_eq!(
                encoder.process(b"abc", &mut [], EndDirective::Continue),
                Err(Error::AllocationFailed)
            );
            assert_eq!(monitor.snapshot().attempts, ordinal);
            assert_eq!(encoder.prefix.is_some(), ordinal == 1);
            assert_eq!(encoder.active_dictionary.is_some(), ordinal == 2);
            encoder.reset();
            assert_eq!(weak.upgrade().is_some(), ordinal == 1);
            if ordinal == 1 {
                encoder.set_dictionary(None).unwrap();
            }
            assert!(weak.upgrade().is_none());
            dictionary_monitor.assert_released();
            assert_eq!(
                decompress(&fragmented(&mut encoder, b"xxx", 3, 7, false).unwrap(), 3).unwrap(),
                b"xxx"
            );
            drop(encoder);
            monitor.assert_released();
        }
    }

    #[test]
    fn allocator_small_raw_frame_failures_recover_after_move_and_option_validation() {
        // RLE input avoids large match/entropy tables, keeping this exhaustive
        // pending/staging/history failure and context-move witness suitable for Miri.
        let options = EncoderOptions {
            window_log: 10,
            pledged_size: Some(3),
            ..EncoderOptions::default()
        };
        let owner = tracking();
        let monitor = owner.monitor();
        let mut baseline = Encoder::with_options_in(options, owner).unwrap();
        let expected = fragmented(&mut baseline, b"xxx", 3, 7, false).unwrap();
        let attempts = monitor.snapshot().attempts;
        assert_eq!(attempts, 3);
        drop(baseline);
        monitor.assert_released();
        for ordinal in 1..=attempts {
            let owner = TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 16,
                fail_at: NonZeroU64::new(ordinal),
            })
            .unwrap();
            let monitor = owner.monitor();
            let mut encoder = Encoder::with_options_in(options, owner).unwrap();
            assert_eq!(
                fragmented(&mut encoder, b"xxx", 3, 7, false),
                Err(Error::AllocationFailed)
            );
            let mut slot = Some(encoder);
            let mut moved = slot.take().unwrap();
            assert!(slot.is_none());
            let before = monitor.snapshot();
            assert_eq!(
                moved.reset_with_options(EncoderOptions {
                    window_log: 9,
                    ..options
                }),
                Err(Error::InvalidParameter)
            );
            assert_eq!(moved.options(), options);
            assert_eq!(monitor.snapshot().attempts, before.attempts);
            assert_eq!(
                moved.process(&[], &mut [], EndDirective::Finish),
                Err(Error::InvalidState)
            );
            moved.reset_with_options(options).unwrap();
            assert_eq!(
                fragmented(&mut moved, b"xxx", 3, 7, false).unwrap(),
                expected
            );
            drop(moved);
            monitor.assert_released();
        }
    }
    #[test]
    fn allocator_small_borrowed_context_uses_no_dictionary_storage_after_attachment() {
        let dictionary_owner = tracking().into_non_clone();
        let dictionary_monitor = dictionary_owner.monitor();
        let dictionary = crate::Dictionary::raw_in(b"xyz", &dictionary_owner).unwrap();
        let before = dictionary_monitor.snapshot();
        assert!(dictionary_monitor.fail_next());
        let owner = tracking().into_non_clone();
        let monitor = owner.monitor();
        let options = EncoderOptions {
            window_log: 10,
            ..EncoderOptions::default()
        };
        let mut encoder = Encoder::with_dictionary_in(options, dictionary.view(), &owner).unwrap();
        let encoded = fragmented(&mut encoder, b"xxx", 3, 1, false).unwrap();
        encoder.reset();
        assert_eq!(
            fragmented(&mut encoder, b"xxx", 3, 7, false).unwrap(),
            encoded
        );
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(dictionary_monitor.snapshot().attempts, before.attempts);
        drop(encoder);
        monitor.assert_released();
        drop(dictionary);
        dictionary_monitor.assert_released();
    }

    #[cfg(feature = "std")]
    #[test]
    #[ignore = "requires ZSTD_BIN pointing to the pinned upstream CLI"]
    fn official_codec_decodes_custom_allocator_streams_with_independent_dictionary_owners() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let binary = std::env::var_os("ZSTD_BIN").expect("set ZSTD_BIN to the pinned upstream CLI");
        let cases = [
            None,
            Some("boundary-dictionary.raw"),
            Some("formatted.dict"),
        ];
        let mut frames = 0;
        for name in cases {
            let path = name.map(|name| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/upstream/dictionaries")
                    .join(name)
            });
            let bytes = path
                .as_ref()
                .map(|path| std::fs::read(path).unwrap())
                .unwrap_or_default();
            let dictionary_owner = tracking().into_non_clone();
            let dictionary_monitor = dictionary_owner.monitor();
            let dictionary = crate::Dictionary::from_bytes_in(&bytes, &dictionary_owner).unwrap();
            let source: Vec<u8> = (0..2053)
                .map(|index| {
                    dictionary
                        .content()
                        .get(index % dictionary.content().len().clamp(1, 256))
                        .copied()
                        .unwrap_or((index % 127) as u8)
                })
                .collect();
            for strategy in [
                CompressionStrategy::Greedy,
                CompressionStrategy::Chain,
                CompressionStrategy::Lazy,
            ] {
                for include_dictionary_id in [false, true] {
                    let owner = tracking();
                    let monitor = owner.monitor();
                    let options = EncoderOptions {
                        strategy,
                        window_log: 10,
                        include_dictionary_id,
                        ..EncoderOptions::default()
                    };
                    let mut encoder =
                        Encoder::with_dictionary_in(options, dictionary.view(), owner).unwrap();
                    let encoded = fragmented(&mut encoder, &source, 701, 17, true).unwrap();
                    let mut command = Command::new(&binary);
                    command.args(["-d", "-q", "-c"]);
                    if let Some(path) = &path {
                        command.arg("-D").arg(path);
                    }
                    let mut child = command
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .unwrap();
                    child.stdin.take().unwrap().write_all(&encoded).unwrap();
                    let output = child.wait_with_output().unwrap();
                    assert!(
                        output.status.success(),
                        "upstream failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    assert_eq!(output.stdout, source);
                    drop(encoder);
                    monitor.assert_released();
                    frames += 1;
                }
            }
            drop(dictionary);
            dictionary_monitor.assert_released();
        }
        assert_eq!(frames, 18);
        std::eprintln!(
            "C decoded {frames} custom allocator streams exactly across three strategies, empty/raw/prepared dictionaries, and both dictionary-ID policies"
        );
    }
}
