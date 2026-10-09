//! Independent bounded match-search policies; these are not upstream levels.

/// Match-search effort for a streaming [`crate::Encoder`].
///
/// Every policy uses the same format checks and entropy selection. Retained
/// output is bounded by both [`crate::MatchSearchOptions::history_bytes`] and the
/// advertised frame window: up to 1 MiB, with a default of 128 KiB. These scalar
/// choices share a safe representation; the selected indexed storage has its
/// own optional unsafe backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompressionStrategy {
    /// Search the newest hash candidate at the selected key width (default four).
    #[default]
    Greedy,
    /// Search a bounded number of candidates, choosing longest then nearest.
    ///
    /// The default effort is eight visits, with 262,144 working nodes and
    /// 65,536 hash heads. [`crate::MatchSearchOptions`] configures effort and
    /// dimensions. Dictionary preparation retains at most 65,536 seed positions
    /// in separate storage. On 64-bit targets the default requested table payload
    /// is 9 MiB plus 3 MiB for a seed. Tables allocate only when used and remain
    /// reusable across resets.
    Chain,
    /// Search candidates at the current and next byte positions.
    ///
    /// Each probe's default effort is sixteen visits, configurable through
    /// [`crate::MatchSearchOptions`].
    /// Delay a match by one literal only when the next match is longer by more
    /// than one byte; ties keep the current match. Uses Chain's selected storage.
    Lazy,
}
