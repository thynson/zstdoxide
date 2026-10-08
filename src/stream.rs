//! Borrowed-buffer progress shared by the incremental codec APIs.

/// The resource or boundary reached by one successful streaming operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamStatus {
    /// Supply more input; previously accepted input remains in the context.
    NeedInput,
    /// Supply output space and retry with the unconsumed input suffix.
    NeedOutput,
    /// One complete standard or skippable frame has been delivered/consumed.
    FrameFinished,
    /// Every input byte of the requested encoder flush has reached the output.
    Flushed,
}

/// Exact slice progress for one call, corresponding to upstream buffer positions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Progress {
    /// Input prefix accepted by this call; never exceeds the supplied length.
    pub consumed: usize,
    /// Output prefix initialized by this call; never exceeds the supplied length.
    pub produced: usize,
    /// The boundary reached or resource needed to resume.
    pub status: StreamStatus,
}

/// The encoder action, corresponding to upstream `ZSTD_EndDirective`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndDirective {
    /// Accept input, emitting full blocks and retaining a final partial block.
    Continue,
    /// Emit accepted input as complete blocks, keeping the frame open.
    Flush,
    /// End the frame after all supplied input, including its optional checksum.
    Finish,
}
