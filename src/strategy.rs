//! Independent bounded match-search policies; these are not upstream levels.

/// Match-search effort for a streaming [`crate::Encoder`].
///
/// Every policy uses the same format checks, entropy selection, and at most
/// 128 KiB of retained output. These scalar choices share a safe representation;
/// the selected indexed storage has its own optional unsafe backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionStrategy {
    /// Search the newest four-byte hash candidate (default).
    #[default]
    Greedy,
    /// Search up to eight candidates, choosing longest then nearest matches.
    ///
    /// The chain has 262,144 working nodes and 65,536 hash heads. Dictionary
    /// preparation retains at most 65,536 seed positions in separate storage.
    /// On 64-bit targets these allocations use 9 MiB plus 3 MiB for a seed;
    /// they are allocated only when used and remain reusable across resets.
    Chain,
    /// Search up to sixteen candidates at the current and next byte positions.
    ///
    /// Delay a match by one literal only when the next match is longer by more
    /// than one byte; ties keep the current match. Uses Chain's storage bounds.
    Lazy,
}
