//! Request-backed incremental frames using the admitted reference parsers.
#![forbid(unsafe_code)]

use alloc::alloc::{Allocator, Global};
use core::fmt;

use crate::{
    CompiledStreamEncoder, CompiledStreamOptions, CompressionParameterRequest, DictionaryView,
    EndDirective, Progress, ResolvedCompressionParameters, Result,
};

/// An incremental reference encoder that needs no compiled dictionary.
///
/// Each frame resolves the construction request from its explicit source
/// pledge, or an unknown source size, before writing its header. Fast,
/// DoubleFast, Greedy, Lazy, Lazy2 and BinaryTreeLazy2 are supported within the
/// existing reference plan bounds. Optimal, Ultra, Ultra2, LDM and workers are
/// not supported. An initial Finish does not infer a source pledge.
///
/// The shared engine owns fallible allocator-backed staging, history, entropy
/// and pending output. Every sealed block rebuilds a parser over its retained
/// history. Native workspace reuse or byte-identical native frames is not
/// promised. Frame headers use a window descriptor even for known small sizes.
pub struct ReferenceStreamEncoder<'a, A: Allocator + Clone = Global> {
    engine: CompiledStreamEncoder<'a, A>,
    request: CompressionParameterRequest,
}

impl<'a> ReferenceStreamEncoder<'a> {
    /// Validate a request and create a frame with default flags.
    pub fn new(request: CompressionParameterRequest) -> Result<Self> {
        Self::with_options(request, CompiledStreamOptions::default())
    }

    /// Validate the initial requested profile and frame flags before allocation.
    pub fn with_options(
        request: CompressionParameterRequest,
        options: CompiledStreamOptions,
    ) -> Result<Self> {
        Self::with_options_in(request, options, Global)
    }
}

impl<'a> ReferenceStreamEncoder<'a> {
    /// Create a frame with a borrowed one-frame prefix, without compiled tables.
    ///
    /// The prefix's matchable content length participates in initial admission.
    /// An empty prefix is treated as no selection. Prepared dictionary views
    /// also contribute their identifier and first-block entropy.
    pub fn with_prefix(
        request: CompressionParameterRequest,
        options: CompiledStreamOptions,
        prefix: DictionaryView<'a>,
    ) -> Result<Self> {
        Self::with_prefix_in(request, options, prefix, Global)
    }
}

impl<'a, A: Allocator + Clone> ReferenceStreamEncoder<'a, A> {
    /// Create a prefix-backed frame with the caller's workspace allocator.
    ///
    /// No prefix bytes or compiled tables are owned or copied at construction.
    /// Each block stages only the suffix within the resolved window.
    pub fn with_prefix_in(
        request: CompressionParameterRequest,
        options: CompiledStreamOptions,
        prefix: DictionaryView<'a>,
        allocator: A,
    ) -> Result<Self> {
        Ok(Self {
            engine: CompiledStreamEncoder::from_request(request, options, Some(prefix), allocator)?,
            request,
        })
    }

    /// Replace the pending prefix at a clean boundary before any header starts.
    ///
    /// An empty prefix detaches the selection. The next frame claims the view
    /// exactly once and resolves its profile using its content length. Profile
    /// admission is deferred until process; boundary errors leave state intact.
    pub fn set_prefix(&mut self, prefix: DictionaryView<'a>) -> Result<()> {
        self.engine.set_prefix(prefix)?;
        Ok(())
    }

    /// Borrow the pending or active one-frame prefix, if any.
    ///
    /// Completion or reset of a started frame releases its claimed prefix;
    /// resetting an unstarted frame retains the pending prefix.
    pub const fn selection(&self) -> Option<DictionaryView<'a>> {
        self.engine.selection()
    }

    /// Create a default frame using the caller's workspace allocator.
    pub fn new_in(request: CompressionParameterRequest, allocator: A) -> Result<Self> {
        Self::with_options_in(request, CompiledStreamOptions::default(), allocator)
    }

    /// Create a frame using the caller's workspace allocator.
    ///
    /// Construction validates the initial profile without allocation. Every
    /// frame validates again before its first header byte because resetting
    /// the pledge can change both strategy and geometry.
    pub fn with_options_in(
        request: CompressionParameterRequest,
        options: CompiledStreamOptions,
        allocator: A,
    ) -> Result<Self> {
        Ok(Self {
            engine: CompiledStreamEncoder::from_request(request, options, None, allocator)?,
            request,
        })
    }

    /// The immutable request resolved at each frame initialization.
    pub const fn request(&self) -> CompressionParameterRequest {
        self.request
    }

    /// Initialized frame parameters, or `None` before its header starts.
    ///
    /// Reset clears this observation. It never reports the previous frame's
    /// profile as the next frame's parameters.
    pub const fn parameters(&self) -> Option<ResolvedCompressionParameters> {
        self.engine.initialized_parameters()
    }

    /// Current frame flags and explicit source pledge.
    pub const fn options(&self) -> CompiledStreamOptions {
        self.engine.options()
    }

    /// Source bytes accepted since reset.
    pub const fn total_input(&self) -> u64 {
        self.engine.total_input()
    }

    /// Encoded bytes delivered since reset.
    pub const fn total_output(&self) -> u64 {
        self.engine.total_output()
    }

    /// Reachable workspace capacity, excluding inline scalar state.
    pub fn retained_capacity(&self) -> usize {
        self.engine.retained_capacity()
    }

    /// Discard the session and pledge, retaining request, flags and capacity.
    ///
    /// This also recovers from a processing failure. The next frame resolves
    /// its profile afresh from unknown size and any configured source hint.
    pub fn reset(&mut self) {
        self.engine.reset();
    }

    /// Discard the session and replace frame flags, retaining the request.
    ///
    /// Profile admission is deferred until the next process call, before any
    /// header byte or workspace allocation. An unsupported new size/profile
    /// poisons that session, just like other processing failures.
    pub fn reset_with_options(&mut self, options: CompiledStreamOptions) -> Result<()> {
        self.engine.reset_with_options(options)
    }

    /// Accept a source prefix and deliver an encoded output prefix.
    ///
    /// Resubmit the unconsumed suffix. Flush and Finish latch the directive
    /// and original offered length until completion; smaller suffix fragments
    /// and empty calls to drain output are accepted. Any error poisons the
    /// session until reset; discard output touched by the failing call.
    /// After completion only empty Finish is accepted, idempotently.
    pub fn process(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        directive: EndDirective,
    ) -> Result<Progress> {
        self.engine.process(input, output, directive)
    }
}

impl<A: Allocator + Clone> fmt::Debug for ReferenceStreamEncoder<'_, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReferenceStreamEncoder")
            .field("request", &self.request)
            .field("engine", &self.engine)
            .finish()
    }
}
