//! Incremental frames over one streaming dictionary selection.
//!
//! The coordinator holds either one owned [`CompiledDictionary`](crate::CompiledDictionary)
//! — whose immutable candidates and first-block entropy seed every frame — or
//! a borrowed one-frame [`DictionaryView`] prefix that the next frame
//! initialization claims exactly once, mirroring `ZSTD_CCtx_refCDict`,
//! `ZSTD_CCtx_refPrefix_advanced` and the single-usage prefix of
//! `ZSTD_CCtx_init_compressStream2`. The stream retains at most one input
//! block, one combined staging buffer bounded by the resolved window plus one
//! block, the last window bytes of frame history, and pending output. Every
//! sealed block re-binds a reference search state over the staged selection
//! suffix, the retained history and the block, so staged positions keep their
//! suffix-relative meaning and retained history remains offset-exact.
//!
//! The seven-field profile, window and block geometry are fixed by the
//! compiled construction state and survive every selection change; each
//! compiled frame revalidates them instead of re-resolving parameters. Native
//! attach-versus-load workspace reuse, pledged-source window re-resolution and
//! byte identity with native prefix frames are not implied.
#![forbid(unsafe_code)]

use alloc::alloc::{Allocator, Global};
use alloc::vec::Vec;
use core::fmt;

use crate::block_entropy::BlockEntropyEncoder;
use crate::compiled_dictionary::CompiledDictionary;
use crate::frame::{MAGIC, MAX_BLOCK_SIZE};
use crate::huffman_codebook::HuffmanCodebook;
use crate::literal_encode::LiteralEncoder;
use crate::reference_policy::raw_literals;
use crate::reference_search::{ReferenceSearchPlan, ReferenceSearchState};
use crate::sequence_context::SequenceEncoder;
use crate::xxhash::XxHash64;
use crate::{
    CompressionParameterRequest, DictionaryView, EndDirective, Error, Progress,
    ResolvedCompressionParameters, Result, SourceSize, StreamStatus,
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

/// A reusable incremental encoder over one streaming dictionary selection.
///
/// Construction takes one immutable compiled dictionary, which fixes the
/// profile, window and block geometry for the coordinator's whole life. The
/// selection itself can be replaced by a compiled state with the same profile
/// or by a borrowed one-frame prefix; compiled tables are never cloned,
/// reseeded or mutated, and a prefix is claimed by exactly one frame. Each
/// sealed block builds a per-block parser over the staged selection suffix,
/// the retained frame history and the block; those parser owners are released
/// when the block ends. Input and output slices expire at the end of each
/// call. Allocation is fallible and bounded by the resolved window and one
/// block.
pub struct CompiledStreamEncoder<'a, A: Allocator + Clone = Global, B: Allocator + Clone = Global> {
    compiled: Option<CompiledDictionary<'a, B>>,
    prefix: Option<DictionaryView<'a>>,
    active_prefix: Option<DictionaryView<'a>>,
    profile: ResolvedCompressionParameters,
    request: Option<CompressionParameterRequest>,
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

    /// Validate frame options and create a fresh frame.
    ///
    /// Only scalar frame flags are validated. The strategy, window, tables and
    /// entropy remain the compiled profile's. A profile whose window cannot be
    /// described by the supported reference frame header is rejected with
    /// [`Error::UnsupportedFeature`] before the first allocation. The compiled
    /// state becomes the initial selection and its parameters are the fixed
    /// profile of the coordinator.
    pub fn with_options_in(
        compiled: CompiledDictionary<'a, B>,
        options: CompiledStreamOptions,
        allocator: A,
    ) -> Result<Self> {
        Self::from_profile(
            compiled.parameters(),
            Some(compiled),
            None,
            options,
            allocator,
        )
    }

    fn from_profile(
        profile: ResolvedCompressionParameters,
        compiled: Option<CompiledDictionary<'a, B>>,
        request: Option<CompressionParameterRequest>,
        options: CompiledStreamOptions,
        allocator: A,
    ) -> Result<Self> {
        let window_size = 1_usize
            .checked_shl(u32::from(profile.window_log()))
            .ok_or(Error::ResourceLimit)?;
        // The per-block plan builds `ReferenceFrameSettings`; reject a profile
        // outside that bounded window range before any owner is created.
        if !(10..=21).contains(&profile.window_log()) {
            return Err(Error::UnsupportedFeature);
        }
        let block_limit = window_size.min(MAX_BLOCK_SIZE);
        Ok(Self {
            compiled,
            prefix: None,
            active_prefix: None,
            profile,
            request,
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

    /// Validate a request without allocating or manufacturing compiled tables.
    pub(crate) fn from_request(
        request: CompressionParameterRequest,
        options: CompiledStreamOptions,
        prefix: Option<DictionaryView<'a>>,
        allocator: A,
    ) -> Result<Self> {
        let prefix = prefix.filter(|view| !view.content().is_empty());
        let profile = Self::resolve_request(
            request,
            options,
            prefix.map_or(0, |view| view.content().len()),
        )?;
        let mut engine = Self::from_profile(profile, None, Some(request), options, allocator)?;
        engine.prefix = prefix;
        Ok(engine)
    }

    fn resolve_request(
        request: CompressionParameterRequest,
        options: CompiledStreamOptions,
        prefix_size: usize,
    ) -> Result<ResolvedCompressionParameters> {
        let source = options
            .pledged_size
            .map_or(SourceSize::Unknown, SourceSize::Known);
        let profile = request.resolve(source, prefix_size)?;
        if !(10..=21).contains(&profile.window_log()) {
            return Err(Error::UnsupportedFeature);
        }
        let block_limit = (1_usize << profile.window_log()).min(MAX_BLOCK_SIZE);
        let plan = ReferenceSearchPlan::from_resolved(profile, block_limit)?;
        if !plan.admits_dictionary_staging() {
            return Err(Error::UnsupportedFeature);
        }
        Ok(profile)
    }

    pub(crate) const fn initialized_parameters(&self) -> Option<ResolvedCompressionParameters> {
        if self.header_started {
            Some(self.profile)
        } else {
            None
        }
    }

    /// Borrow the selected dictionary view.
    ///
    /// This is the compiled view while a compiled dictionary is selected, the
    /// pending prefix before the next frame claims it, the active prefix while
    /// its frame is running, and `None` after the prefix frame completed or a
    /// selection was removed.
    pub const fn selection(&self) -> Option<DictionaryView<'a>> {
        if let Some(prefix) = self.active_prefix {
            Some(prefix)
        } else if let Some(prefix) = self.prefix {
            Some(prefix)
        } else {
            match &self.compiled {
                Some(compiled) => Some(compiled.view()),
                None => None,
            }
        }
    }

    /// The fixed profile every frame of this stream uses.
    ///
    /// Selection changes cannot move it; a replacement compiled state must
    /// carry exactly these parameters.
    pub const fn parameters(&self) -> ResolvedCompressionParameters {
        self.profile
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

    /// Reachable workspace capacity, excluding inline state and the selected
    /// dictionary's own tables and content borrow.
    pub fn retained_capacity(&self) -> usize {
        self.block.capacity()
            + self.combined.capacity()
            + self.history.capacity()
            + self.pending.capacity()
            + self.entropy.retained_capacity()
    }

    /// Whether another selection or frame option may be configured now.
    ///
    /// Only a session that never started a header is a clean boundary, the
    /// same rule `Encoder` applies. A finished stream returns to it through
    /// [`Self::reset`].
    fn check_configuration_boundary(&self) -> Result<()> {
        if self.phase == Phase::Active && !self.header_started {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }

    /// Select `compiled` as the persistent dictionary, replacing the current
    /// selection.
    ///
    /// A pending prefix is discarded and the previously selected compiled
    /// state is returned to the caller intact — tables move by value, nothing
    /// is cloned or dropped implicitly. The new state must carry exactly the
    /// construction profile, otherwise [`Error::InvalidParameter`] precedes
    /// every mutation. Only a clean configuration boundary accepts selection
    /// changes. Corresponds to `ZSTD_CCtx_refCDict`, whose referenced CDict
    /// also replaces every other attachment.
    pub fn set_compiled(
        &mut self,
        compiled: CompiledDictionary<'a, B>,
    ) -> Result<Option<CompiledDictionary<'a, B>>> {
        self.check_configuration_boundary()?;
        if compiled.parameters() != self.profile {
            return Err(Error::InvalidParameter);
        }
        self.prefix = None;
        Ok(self.compiled.replace(compiled))
    }

    /// Select a prefix for the next frame, replacing the current selection.
    ///
    /// The first process call that starts a header claims this view for that
    /// frame; later frames run without a selection until a new one is chosen.
    /// The previously selected compiled state is returned to the caller
    /// intact. An empty prefix detaches the selection, exactly like the
    /// upstream empty `ZSTD_CCtx_refPrefix_advanced` call. Only a clean
    /// configuration boundary accepts selection changes. Corresponds to
    /// `ZSTD_CCtx_refPrefix_advanced` with the view's own content kind deciding
    /// identifier and entropy contribution.
    pub fn set_prefix(
        &mut self,
        prefix: DictionaryView<'a>,
    ) -> Result<Option<CompiledDictionary<'a, B>>> {
        self.check_configuration_boundary()?;
        if prefix.content().is_empty() {
            self.prefix = None;
            return Ok(self.compiled.take());
        }
        let previous = self.compiled.take();
        self.prefix = Some(prefix);
        Ok(previous)
    }

    /// Start another frame while retaining the selection, frame options and
    /// workspace allocations.
    ///
    /// The per-frame source pledge is cleared, the active prefix of a
    /// completed frame is released and the next frame installs the selection's
    /// first-block entropy seed again. An unclaimed prefix stays pending and
    /// is consumed by the next frame, like the upstream session-only reset
    /// keeping `prefixDict`. This also recovers a context poisoned by a
    /// processing error. Corresponds to session-only `ZSTD_CCtx_reset`.
    pub fn reset(&mut self) {
        self.options.pledged_size = None;
        self.clear_frame();
    }

    /// Reset the session and install new frame options.
    ///
    /// The selection, any unclaimed prefix and every workspace allocation are
    /// retained. The new options describe the next frame only; the fixed
    /// profile, window and table geometry cannot be changed.
    pub fn reset_with_options(&mut self, options: CompiledStreamOptions) -> Result<()> {
        self.options = options;
        self.clear_frame();
        Ok(())
    }

    /// Restore default frame options and remove the selection.
    ///
    /// Corresponds to the supported role of `ZSTD_reset_parameters`. Only a
    /// clean configuration boundary or a completed frame accepts this
    /// operation; an active, finishing or failed session returns
    /// [`Error::InvalidState`] without any mutation. The previously selected
    /// compiled state is returned to the caller so its tables stay reusable.
    ///
    /// Defaults are Rust's [`CompiledStreamOptions::default`]. C's
    /// parameter-only reset additionally restores default requested
    /// parameters; this coordinator keeps its construction profile because its
    /// geometry is fixed for workspace reuse. A pending or active prefix view
    /// is dropped, which does not shorten the context type's lifetime. No
    /// table is reseeded by or after this call.
    pub fn reset_parameters(&mut self) -> Result<Option<CompiledDictionary<'a, B>>> {
        if self.phase != Phase::Finished {
            self.check_configuration_boundary()?;
        }
        self.options = CompiledStreamOptions::default();
        self.prefix = None;
        Ok(self.compiled.take())
    }

    /// Discard the session, restore default frame options, and remove the
    /// selection.
    ///
    /// Corresponds to `ZSTD_reset_session_and_parameters`. This accepts every
    /// phase, including processing failures, and returns a ready context with
    /// zero counters and no pending source, output, or directive. The
    /// previously selected compiled state is returned to the caller; a
    /// dropped prefix view does not shorten the context type's lifetime.
    pub fn reset_all(&mut self) -> Option<CompiledDictionary<'a, B>> {
        self.options = CompiledStreamOptions::default();
        self.prefix = None;
        let previous = self.compiled.take();
        self.clear_frame();
        previous
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
        self.active_prefix = None;
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
                // The claimed prefix served its one frame; a completed frame
                // leaves no selection, matching `Encoder`'s released handle.
                self.active_prefix = None;
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

    /// Claims the pending prefix, revalidates the fixed profile and writes the
    /// non-single-segment form of `ZSTD_writeFrameHeader`.
    ///
    /// The prefix claim is the upstream single usage at frame initialization:
    /// a pending view moves to the active slot and serves exactly this frame.
    /// A compiled selection is revalidated against its construction gate —
    /// supported window range, loaded suffix within the window and the content
    /// borrow — before any header byte. The active selection's first-block
    /// entropy seed is installed the same way for compiled and formatted
    /// prefix frames.
    fn start_header(&mut self) -> Result<()> {
        if let Some(request) = self.request {
            let prefix_size = self.prefix.map_or(0, |prefix| prefix.content().len());
            let profile = Self::resolve_request(request, self.options, prefix_size)?;
            // Admission precedes mutation; reset cleared all frame positions,
            // so retained vector capacity never carries old geometry or bytes.
            self.profile = profile;
            self.window_size = 1_usize << profile.window_log();
            self.block_limit = self.window_size.min(MAX_BLOCK_SIZE);
        }
        if let Some(prefix) = self.prefix.take() {
            self.active_prefix = Some(prefix);
        }
        if let Some(compiled) = &self.compiled {
            let parameters = compiled.parameters();
            if !(10..=21).contains(&parameters.window_log()) {
                return Err(Error::UnsupportedFeature);
            }
            let loaded = compiled.loaded_len();
            if loaded == 0 || loaded > compiled.content().len() || loaded > self.window_size {
                return Err(Error::InvalidParameter);
            }
        }
        let seed = if let Some(prefix) = self.active_prefix {
            prefix
                .entropy()
                .map(|view| {
                    HuffmanCodebook::from_lengths(&view.huffman.code_lengths(), 12)
                        .map(|table| (view, table))
                })
                .transpose()?
        } else if let Some(compiled) = &self.compiled {
            compiled.first_block_seed()
        } else {
            None
        };
        self.pending
            .try_reserve_exact(18)
            .map_err(|_| Error::AllocationFailed)?;
        let parameters = self.profile;
        let (entropy, literal_table) = match seed {
            Some((entropy, table)) => (Some(entropy), Some(table)),
            None => (None, None),
        };
        self.entropy.start_frame(entropy, literal_table)?;
        for byte in MAGIC.to_le_bytes() {
            self.pending.push(byte);
        }
        let dictionary_id = if self.options.include_dictionary_id {
            if let Some(prefix) = self.active_prefix {
                prefix.id()
            } else if let Some(compiled) = &self.compiled {
                compiled.dictionary_id()
            } else {
                0
            }
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
    ///
    /// The parser binds by selection: a compiled dictionary stages its loaded
    /// suffix and consults the compiled candidates without a seeding pass; a
    /// claimed prefix stages its last in-window bytes and seeds the per-call
    /// tables over the staged suffix plus history, exactly like the upstream
    /// per-frame prefix load; an empty selection stages only the history.
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
        let plan = ReferenceSearchPlan::from_resolved(self.profile, block_len)?;
        let history_len = usize::try_from(self.frame_position)
            .map_err(|_| Error::ResourceLimit)?
            .min(self.window_size);
        if self.history.len() != history_len {
            return Err(Error::InvalidState);
        }
        // Bind the selection staging before any output byte. A compiled
        // dictionary and a claimed prefix are mutually exclusive, and every
        // slice below either carries the `'a` content borrow or a disjoint
        // field borrow, so the combined staging mutation cannot alias them.
        let active_prefix = self.active_prefix;
        let (staged, staged_bytes, candidates) = if let Some(compiled) = self.compiled.as_ref() {
            let loaded = compiled.loaded_len();
            let content: &'a [u8] = compiled.content();
            if loaded == 0
                || loaded > content.len()
                || loaded > self.window_size
                || !plan.admits_dictionary_staging()
            {
                return Err(Error::InvalidParameter);
            }
            let candidates = compiled.search_candidates()?;
            (
                loaded,
                Some(&content[content.len() - loaded..]),
                Some(candidates),
            )
        } else if let Some(prefix) = active_prefix {
            let content: &'a [u8] = prefix.content();
            let staged = content.len().min(self.window_size);
            if staged > 0 && !plan.admits_dictionary_staging() {
                return Err(Error::UnsupportedFeature);
            }
            (staged, Some(&content[content.len() - staged..]), None)
        } else {
            (0, None, None)
        };
        let combined_len = staged
            .checked_add(history_len)
            .and_then(|value| value.checked_add(block_len))
            .ok_or(Error::ResourceLimit)?;
        self.combined.clear();
        self.combined
            .try_reserve_exact(combined_len)
            .map_err(|_| Error::AllocationFailed)?;
        if let Some(bytes) = staged_bytes {
            self.combined.extend_from_slice(bytes);
        }
        self.combined.extend_from_slice(&self.history);
        self.combined.extend_from_slice(&self.block);

        let initial_repeat = self.entropy.sequence_encoder.committed_repeats();
        let mut search = if let Some(candidates) = candidates {
            ReferenceSearchState::new_with_compiled(
                &self.combined,
                staged + history_len,
                plan,
                &self.allocator,
                candidates,
            )?
        } else {
            ReferenceSearchState::new_with_prefix(
                &self.combined,
                staged + history_len,
                plan,
                &self.allocator,
            )?
        };
        let parsed = search.parse_next(block_len, initial_repeat)?;
        let worthwhile = if parsed {
            self.entropy.prepare_reference(
                search.literals(),
                search.sequences(),
                initial_repeat,
                block_len,
                raw_literals(self.profile),
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
            .field(
                "selection",
                &match (&self.compiled, self.prefix, self.active_prefix) {
                    (Some(_), _, _) => "compiled",
                    (_, Some(_), _) => "pending prefix",
                    (_, _, Some(_)) => "active prefix",
                    (None, None, None) => "none",
                },
            )
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

    /// Compare selections by content slices; `DictionaryView` carries no
    /// structural equality and every selection shares the same `'a` borrow.
    fn selection_content<'a>(encoder: &CompiledStreamEncoder<'a, Global>) -> Option<&'a [u8]> {
        encoder.selection().map(|view| view.content())
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
    fn compiled_plus_history_binding_matches_full_prefix_seeding_for_hash_and_chain_families() {
        // The streaming parser stages the compiled suffix, then retained
        // history, then the block. For hash and chain/row families, binding the
        // per-call owners with a compiled view over the suffix plus history
        // seeding produces the same candidate stream as seeding the entire
        // prefix from scratch. The eagerly sorted compiled tree is excluded:
        // it can add candidates that the lazy per-call tree back-fill has not
        // inserted yet, which is a measured family property rather than a
        // history-binding difference.
        let content = pattern(0x3141_5926, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let history = content[2048..3072].to_vec();
        let block = content[1024..1536].to_vec();
        for level in [1, 3, 4, 5, 7] {
            let compiled = compiled(&dictionary, level, Global);
            let loaded = compiled.loaded_len();
            let mut combined = Vec::new();
            combined.extend_from_slice(&content[content.len() - loaded..]);
            combined.extend_from_slice(&history);
            combined.extend_from_slice(&block);
            let plan =
                ReferenceSearchPlan::from_resolved(compiled.parameters(), block.len()).unwrap();
            let mut consulted = ReferenceSearchState::new_with_compiled(
                &combined,
                loaded + history.len(),
                plan,
                &Global,
                compiled.search_candidates().unwrap(),
            )
            .unwrap();
            let mut reference = ReferenceSearchState::new_with_prefix(
                &combined,
                loaded + history.len(),
                plan,
                &Global,
            )
            .unwrap();
            let actual = consulted.parse_next(block.len(), [1, 4, 8]).unwrap();
            let expected = reference.parse_next(block.len(), [1, 4, 8]).unwrap();
            assert_eq!(actual, expected, "level {level}");
            assert_eq!(consulted.literals(), reference.literals(), "level {level}");
            assert_eq!(
                consulted.sequences(),
                reference.sequences(),
                "level {level}"
            );
        }
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

    #[test]
    fn prefix_selection_replaces_the_compiled_state_and_claims_for_one_frame() {
        let content = pattern(0x5eed_c0de, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let compiled = compiled(&dictionary, 4, Global);
        let mut encoder = CompiledStreamEncoder::new_in(compiled, Global).unwrap();
        let mut compiled_frames = Vec::new();
        for _ in 0..2 {
            compiled_frames.push(stream(&mut encoder, &content[..2000], 700, 13));
            encoder.reset();
        }
        assert_eq!(compiled_frames[0], compiled_frames[1]);
        assert_eq!(
            selection_content(&encoder),
            Some(dictionary.view().content())
        );

        // A raw prefix replaces the compiled selection and returns it intact.
        let prefix_owner = Dictionary::raw(&content[..1024]).unwrap();
        let displaced = encoder.set_prefix(prefix_owner.view()).unwrap();
        assert!(displaced.is_some());
        assert_eq!(
            selection_content(&encoder),
            Some(prefix_owner.view().content())
        );
        let prefix_frame = stream(&mut encoder, &content[..2000], 700, 13);
        let (header, _) = FrameHeader::parse(&prefix_frame).unwrap();
        // A raw prefix contributes no identifier.
        assert_eq!(header.dictionary_id, 0);
        assert_eq!(header.window_size, 1 << encoder.parameters().window_log());
        assert_eq!(
            decompress_with_dictionary_in(
                &prefix_frame,
                &prefix_owner.view(),
                DecodeOptions::new(2000),
                Global,
            )
            .unwrap(),
            &content[..2000]
        );
        // The claimed prefix served exactly one frame: the completed frame
        // leaves no selection, and the next frame decodes dictionary-free.
        assert_eq!(selection_content(&encoder), None);
        encoder.reset();
        assert_eq!(selection_content(&encoder), None);
        let plain_frame = stream(&mut encoder, b"plain tail content", 9, 7);
        let (plain_header, _) = FrameHeader::parse(&plain_frame).unwrap();
        assert_eq!(plain_header.dictionary_id, 0);
        assert_eq!(
            crate::decompress_in(&plain_frame, 18, Global).unwrap(),
            b"plain tail content"
        );

        // The displaced compiled state still produces its original frames.
        let mut restored = CompiledStreamEncoder::new_in(displaced.unwrap(), Global).unwrap();
        assert_eq!(
            stream(&mut restored, &content[..2000], 700, 13),
            compiled_frames[0]
        );
    }

    #[test]
    fn selection_changes_reject_every_non_clean_boundary() {
        let content = pattern(0x0d15_ea5e, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let replacement = compiled(&dictionary, 4, Global);
        let prefix_owner = Dictionary::raw(&content[..512]).unwrap();
        let mut encoder =
            CompiledStreamEncoder::new_in(compiled(&dictionary, 1, Global), Global).unwrap();
        let mut output = [0_u8; 128];
        // Mid-frame: part of the input staged, header started.
        encoder
            .process(&content[..40], &mut output, EndDirective::Continue)
            .unwrap();
        assert_eq!(
            selection_content(&encoder),
            Some(dictionary.view().content())
        );
        assert!(matches!(
            encoder.set_prefix(prefix_owner.view()),
            Err(Error::InvalidState)
        ));
        assert!(matches!(
            encoder.set_compiled(replacement),
            Err(Error::InvalidState)
        ));
        assert_eq!(
            selection_content(&encoder),
            Some(dictionary.view().content())
        );
        assert!(
            encoder
                .reset_with_options(CompiledStreamOptions::default())
                .is_ok()
        );
        // Finishing and failed phases are refused without mutation.
        encoder
            .process(&content[..40], &mut [0_u8; 128], EndDirective::Finish)
            .unwrap();
        loop {
            match encoder.process(&[], &mut [0_u8; 128], EndDirective::Finish) {
                Ok(progress) if progress.status == StreamStatus::FrameFinished => break,
                Ok(_) => {}
                Err(error) => panic!("finish failed: {error:?}"),
            }
        }
        assert!(matches!(
            encoder.set_prefix(prefix_owner.view()),
            Err(Error::InvalidState)
        ));
        // The compiled selection survives frame completion until reset.
        assert_eq!(
            selection_content(&encoder),
            Some(dictionary.view().content())
        );
        encoder.reset();
        assert_eq!(
            selection_content(&encoder),
            Some(dictionary.view().content())
        );
        // A poisoned context refuses configuration like any active frame.
        let mut pledged = CompiledStreamEncoder::with_options_in(
            compiled(&dictionary, 1, Global),
            CompiledStreamOptions {
                pledged_size: Some(3),
                ..CompiledStreamOptions::default()
            },
            Global,
        )
        .unwrap();
        assert_eq!(
            pledged.process(&content[..9], &mut output, EndDirective::Finish),
            Err(Error::ContentSizeMismatch)
        );
        assert!(matches!(
            pledged.set_prefix(prefix_owner.view()),
            Err(Error::InvalidState)
        ));
        pledged.reset();
        assert_eq!(
            selection_content(&pledged),
            Some(dictionary.view().content())
        );
    }

    #[test]
    fn set_compiled_requires_the_construction_profile_and_clears_prefix() {
        let content = pattern(0x3c0d_e5ed, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let mut request = CompressionParameterRequest::with_level(4);
        request.set_window_log(15).unwrap();
        let pinned = CompiledDictionary::new(dictionary.view(), &request, Global).unwrap();
        let other_level = compiled(&dictionary, 9, Global);
        let mut encoder = CompiledStreamEncoder::new_in(pinned, Global).unwrap();
        // A different profile is rejected before any mutation.
        assert!(matches!(
            encoder.set_compiled(other_level),
            Err(Error::InvalidParameter)
        ));
        assert_eq!(
            selection_content(&encoder),
            Some(dictionary.view().content())
        );
        // The same profile replaces the selection and clears a pending prefix.
        let prefix_owner = Dictionary::raw(&content[..512]).unwrap();
        assert!(encoder.set_prefix(prefix_owner.view()).unwrap().is_some());
        let twin = CompiledDictionary::new(dictionary.view(), &request, Global).unwrap();
        // Only the pending prefix was selected, so the replacement returns no
        // compiled state while clearing it.
        let previous = encoder.set_compiled(twin).unwrap();
        assert!(previous.is_none());
        assert!(encoder.prefix.is_none());
        assert_eq!(
            selection_content(&encoder),
            Some(dictionary.view().content())
        );
        // The replacement produces identical frames to the construction state.
        let baseline = stream(&mut encoder, &content[..1500], 500, 31);
        let mut reference = CompiledStreamEncoder::with_options_in(
            CompiledDictionary::new(dictionary.view(), &request, Global).unwrap(),
            CompiledStreamOptions::default(),
            Global,
        )
        .unwrap();
        assert_eq!(stream(&mut reference, &content[..1500], 500, 31), baseline);
    }

    #[test]
    fn empty_prefix_detaches_the_selection_like_the_upstream_empty_reference() {
        let content = pattern(0xd2a7_1eaf, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let mut encoder =
            CompiledStreamEncoder::new_in(compiled(&dictionary, 3, Global), Global).unwrap();
        let empty = Dictionary::raw(b"").unwrap();
        let previous = encoder.set_prefix(empty.view()).unwrap();
        assert!(previous.is_some());
        assert_eq!(selection_content(&encoder), None);
        // Frames continue dictionary-free under the fixed profile.
        let frame = stream(&mut encoder, &content[..600], 200, 17);
        let (header, _) = FrameHeader::parse(&frame).unwrap();
        assert_eq!(header.dictionary_id, 0);
        assert_eq!(
            crate::decompress_in(&frame, 600, Global).unwrap(),
            &content[..600]
        );
        // A second empty prefix after reset is accepted and returns no
        // compiled state.
        encoder.reset();
        assert!(encoder.set_prefix(empty.view()).unwrap().is_none());
        assert_eq!(selection_content(&encoder), None);
    }

    #[test]
    fn session_reset_keeps_an_unclaimed_prefix_pending() {
        let content = pattern(0xcafe_babe, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let prefix_owner = Dictionary::raw(&content[2048..]).unwrap();
        let mut encoder =
            CompiledStreamEncoder::new_in(compiled(&dictionary, 4, Global), Global).unwrap();
        assert!(encoder.set_prefix(prefix_owner.view()).unwrap().is_some());
        encoder.reset();
        // The unclaimed prefix survives the session reset and seeds the next
        // frame exactly once.
        assert_eq!(
            selection_content(&encoder),
            Some(prefix_owner.view().content())
        );
        let frame = stream(&mut encoder, &content[..1000], 400, 23);
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &prefix_owner.view(),
                DecodeOptions::new(1000),
                Global,
            )
            .unwrap(),
            &content[..1000]
        );
        assert_eq!(selection_content(&encoder), None);
        // reset_with_options also keeps the selection contract.
        encoder.reset();
        assert!(encoder.set_prefix(prefix_owner.view()).unwrap().is_none());
        encoder
            .reset_with_options(CompiledStreamOptions {
                checksum: true,
                ..CompiledStreamOptions::default()
            })
            .unwrap();
        assert_eq!(
            selection_content(&encoder),
            Some(prefix_owner.view().content())
        );
    }

    #[test]
    fn prefix_frames_roundtrip_across_all_admitted_families_and_wrap_the_window() {
        let content = pattern(0x51ee_d1ce, 8192);
        let dictionary = Dictionary::raw(&content).unwrap();
        // The prefix is larger than the level-10 window of level 1, so the
        // staged suffix caps at the resolved window like an oversized upstream
        // prefix truncating to its table suffix.
        let prefix_owner = Dictionary::raw(&content[..4096]).unwrap();
        for level in [1, 3, 4, 5, 7, 9] {
            let mut encoder =
                CompiledStreamEncoder::new_in(compiled(&dictionary, level, Global), Global)
                    .unwrap();
            encoder.set_prefix(prefix_owner.view()).unwrap().unwrap();
            // Dictionary repeats plus a cross-boundary run force candidates
            // into the staged prefix, the retained history and the block.
            let mut input = Vec::new();
            for _ in 0..9 {
                input.extend_from_slice(&content[0..600]);
                input.extend_from_slice(&content[3000..3600]);
                input.extend_from_slice(b"prefix-block-boundary");
            }
            let frame = stream(&mut encoder, &input, 331, 19);
            let (header, _) = FrameHeader::parse(&frame).unwrap();
            assert_eq!(header.dictionary_id, 0, "level {level}");
            assert_eq!(
                decompress_with_dictionary_in(
                    &frame,
                    &prefix_owner.view(),
                    DecodeOptions::new(input.len()),
                    Global,
                )
                .unwrap(),
                input,
                "level {level}"
            );
            assert_eq!(selection_content(&encoder), None, "level {level}");
        }
    }

    #[test]
    fn formatted_prefix_writes_identifier_and_seeds_the_first_block() {
        let prepared = Dictionary::from_bytes(include_bytes!(
            "../tests/upstream/dictionaries/formatted.dict"
        ))
        .unwrap();
        let text =
            include_bytes!("../tests/upstream/dictionaries/dictionary-treeless-literals.expected");
        let dictionary = Dictionary::raw(&pattern(0xf072_e47e, 4096)).unwrap();
        let mut encoder =
            CompiledStreamEncoder::new_in(compiled(&dictionary, 5, Global), Global).unwrap();
        encoder.set_prefix(prepared.view()).unwrap().unwrap();
        let frame = stream(&mut encoder, text, 250, 29);
        let (header, _) = FrameHeader::parse(&frame).unwrap();
        // A formatted prefix contributes its identifier and prepared entropy
        // seed to exactly its one frame.
        assert_eq!(header.dictionary_id, prepared.view().id());
        assert_eq!(
            decompress_with_dictionary_in(
                &frame,
                &prepared.view(),
                DecodeOptions::new(text.len()),
                Global,
            )
            .unwrap(),
            text
        );
        // The next frame is dictionary-free and carries no identifier.
        encoder.reset();
        let plain = stream(&mut encoder, b"tail without selection", 11, 9);
        let (plain_header, _) = FrameHeader::parse(&plain).unwrap();
        assert_eq!(plain_header.dictionary_id, 0);
        assert_eq!(
            crate::decompress_in(&plain, 22, Global).unwrap(),
            b"tail without selection"
        );
    }

    #[test]
    fn prefix_single_block_body_matches_the_one_shot_parameterized_dictionary_frame() {
        use crate::compress_with_parameters_and_dictionary;

        let content = pattern(0x7071_50ef, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let input = &content[512..3000];
        for level in [1, 3, 4, 5, 7, 9] {
            let mut request = CompressionParameterRequest::with_level(level);
            request
                .set_source_hint(u32::try_from(input.len()).unwrap())
                .unwrap();
            let compiled = CompiledDictionary::new(dictionary.view(), &request, Global).unwrap();
            let prefix_owner = Dictionary::raw(&content[..1024]).unwrap();
            let mut encoder = CompiledStreamEncoder::with_options_in(
                compiled,
                CompiledStreamOptions {
                    pledged_size: Some(input.len() as u64),
                    ..CompiledStreamOptions::default()
                },
                Global,
            )
            .unwrap();
            encoder.set_prefix(prefix_owner.view()).unwrap().unwrap();
            let streamed = stream(&mut encoder, input, input.len(), 64);
            let one_shot =
                compress_with_parameters_and_dictionary(input, &request, &prefix_owner.view())
                    .unwrap();
            let streamed_header = FrameHeader::parse(&streamed).unwrap();
            let one_shot_header = FrameHeader::parse(&one_shot).unwrap();
            // One short block staged `suffix | block` in both paths under the
            // same resolved profile; only the streaming header differs.
            assert_eq!(
                &streamed[streamed_header.1..],
                &one_shot[one_shot_header.1..],
                "level {level}"
            );
            assert_eq!(
                one_shot_header.0.dictionary_id,
                streamed_header.0.dictionary_id
            );
        }
    }

    #[test]
    fn selection_transition_allocator_failure_releases_every_owner() {
        let content = pattern(0x7e1e_c710, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let prefix_owner = Dictionary::raw(&content[..512]).unwrap();
        for fail_after in 0..12 {
            let compiled = compiled(&dictionary, 1, Global);
            let allocator = TrackingAllocator::new(AllocatorConfig {
                fail_at: core::num::NonZeroU64::new(fail_after as u64),
                ..AllocatorConfig::default()
            })
            .unwrap();
            let monitor = allocator.monitor();
            let mut encoder = CompiledStreamEncoder::new_in(compiled, allocator.clone()).unwrap();
            encoder.set_prefix(prefix_owner.view()).unwrap().unwrap();
            let mut output = [0_u8; 128];
            let _ = encoder.process(&content, &mut output, EndDirective::Finish);
            let _ = encoder.process(&content, &mut output, EndDirective::Finish);
            drop(encoder);
            drop(allocator);
            monitor.assert_released();
        }
    }

    #[test]
    fn parameter_reset_boundaries_and_restored_defaults() {
        let content = pattern(0x9a2e_71c0, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let prefix_owner = Dictionary::raw(&content[..512]).unwrap();
        let custom = CompiledStreamOptions {
            checksum: true,
            pledged_size: Some(10),
            include_content_size: false,
            include_dictionary_id: false,
        };
        // Clean Active boundary: the selected prefix is dropped and defaults
        // replace the custom flags. The compiled state had already been
        // returned by `set_prefix`, so no compiled value comes back.
        let mut encoder = CompiledStreamEncoder::with_options_in(
            compiled(&dictionary, 1, Global),
            custom,
            Global,
        )
        .unwrap();
        let compiled_state = encoder.set_prefix(prefix_owner.view()).unwrap().unwrap();
        assert_eq!(compiled_state.parameters(), encoder.parameters());
        assert!(encoder.reset_parameters().unwrap().is_none());
        assert_eq!(selection_content(&encoder), None);
        assert_eq!(encoder.options(), CompiledStreamOptions::default());
        assert_eq!(encoder.total_input(), 0);

        // Finished accepts the reset and keeps the finished phase.
        let mut encoder = CompiledStreamEncoder::with_options_in(
            compiled(&dictionary, 4, Global),
            CompiledStreamOptions {
                checksum: true,
                ..CompiledStreamOptions::default()
            },
            Global,
        )
        .unwrap();
        let frame = stream(&mut encoder, &content[..600], 250, 41);
        let progress = encoder
            .process(&[], &mut [0_u8; 128], EndDirective::Finish)
            .unwrap();
        assert_eq!(progress.status, StreamStatus::FrameFinished);
        let previous = encoder.reset_parameters().unwrap();
        assert!(previous.is_some());
        assert_eq!(previous.unwrap().parameters(), encoder.parameters());
        assert_eq!(encoder.options(), CompiledStreamOptions::default());
        assert_eq!(encoder.total_input(), 600);
        assert!(matches!(
            encoder.process(&frame, &mut [0_u8; 128], EndDirective::Continue),
            Err(Error::InvalidState)
        ));

        // Mid-frame: refused without any mutation.
        let mid_frame_options = CompiledStreamOptions {
            checksum: true,
            ..CompiledStreamOptions::default()
        };
        let mut encoder = CompiledStreamEncoder::with_options_in(
            compiled(&dictionary, 1, Global),
            mid_frame_options,
            Global,
        )
        .unwrap();
        encoder
            .process(&content[..40], &mut [0_u8; 128], EndDirective::Continue)
            .unwrap();
        assert!(matches!(
            encoder.reset_parameters(),
            Err(Error::InvalidState)
        ));
        assert!(encoder.selection().is_some());
        assert_eq!(encoder.options(), mid_frame_options);

        // Finishing with pending output: refused without mutation.
        let mut encoder = CompiledStreamEncoder::with_options_in(
            compiled(&dictionary, 1, Global),
            CompiledStreamOptions {
                checksum: true,
                ..CompiledStreamOptions::default()
            },
            Global,
        )
        .unwrap();
        encoder
            .process(&content[..40], &mut [0_u8; 1], EndDirective::Finish)
            .unwrap();
        assert!(matches!(
            encoder.reset_parameters(),
            Err(Error::InvalidState)
        ));
        assert!(encoder.selection().is_some());

        // A failed session refuses recovery through the parameter reset.
        let mut encoder = CompiledStreamEncoder::with_options_in(
            compiled(&dictionary, 1, Global),
            CompiledStreamOptions {
                pledged_size: Some(3),
                ..CompiledStreamOptions::default()
            },
            Global,
        )
        .unwrap();
        assert_eq!(
            encoder.process(&content[..9], &mut [0_u8; 128], EndDirective::Finish),
            Err(Error::ContentSizeMismatch)
        );
        assert!(matches!(
            encoder.reset_parameters(),
            Err(Error::InvalidState)
        ));
        assert!(encoder.selection().is_some());
    }

    #[test]
    fn reset_all_recovers_every_phase_and_returns_the_compiled_state() {
        let content = pattern(0x7e70_a115, 4096);
        let dictionary = Dictionary::raw(&content).unwrap();
        let prefix_owner = Dictionary::raw(&content[1024..2048]).unwrap();
        let baseline_compiled = compiled(&dictionary, 2, Global);
        // Baseline bytes from the same compiled state for the reuse check.
        let mut baseline = CompiledStreamEncoder::new_in(baseline_compiled, Global).unwrap();
        let expected = stream(&mut baseline, &content[..500], 200, 23);
        drop(baseline);

        let mut encoder = CompiledStreamEncoder::with_options_in(
            compiled(&dictionary, 2, Global),
            CompiledStreamOptions {
                pledged_size: Some(3),
                checksum: true,
                ..CompiledStreamOptions::default()
            },
            Global,
        )
        .unwrap();
        // The prefix displaced the compiled state before the failure, so the
        // reset returns no compiled value while dropping the prefix.
        let compiled_state = encoder.set_prefix(prefix_owner.view()).unwrap().unwrap();
        assert_eq!(
            encoder.process(&content[..9], &mut [0_u8; 128], EndDirective::Finish),
            Err(Error::ContentSizeMismatch)
        );
        // The failed session recovers, loses the prefix and the selection.
        let previous = encoder.reset_all();
        assert!(previous.is_none());
        assert_eq!(selection_content(&encoder), None);
        assert_eq!(encoder.options(), CompiledStreamOptions::default());
        assert_eq!(encoder.total_input(), 0);
        assert_eq!(encoder.total_output(), 0);
        // The recovered stream continues dictionary-free.
        let frame = stream(&mut encoder, &content[..500], 200, 23);
        assert_eq!(FrameHeader::parse(&frame).unwrap().0.dictionary_id, 0);
        assert_eq!(
            crate::decompress_in(&frame, 500, Global).unwrap(),
            &content[..500]
        );
        // The displaced compiled state reproduces the baseline bytes exactly.
        let mut restored = CompiledStreamEncoder::new_in(compiled_state, Global).unwrap();
        assert_eq!(stream(&mut restored, &content[..500], 200, 23), expected);
    }
}
