# Ordinary tree helper and offline replay review

The v1 helper preserves the previously qualified ordinary lazy-frame observation
scaffolding and calls unchanged upstream codec functions. Its change set is:

- Fixed strategy 6 with W10..21/H6..19/C6..19/S1..8 admission. Automatic row
  selection must be disabled regardless of window size. Actual applied context
  parameters and arena extents remain independently checked.
- The second initialized arena contains 2^C u32 edge cells, not 2^C nodes. It is
  copied with its true byte extent. Historical row observation branches remain
  in the scaffolding but cannot be selected by the admitted tree parameters.
- Hash heads retain ordinary biased source-position validation. Edge value 1
  is an unsorted marker only in odd cells; other nonzero values still require
  initialized logical source bytes and a preceding sampling frontier.
- Every frame request carries a canonical padding argument, exactly 106 or 195.
  It initializes the entire leading/suffix arena before inserting logical source
  bytes. Existing guards and unchanged-source comparisons remain active.
- Metadata explicitly identifies tree-ordinary-v1, strategy 6 and both padding
  values. The CLI requires `bt_` case names and rejects narrowing, noncanonical
  scalars and every unsupported tuple before frame execution.

No source pointer outside an initialized arena is supplied by the harness.
Tables are owned by normal upstream context initialization, not synthetic
workspace substitution. The existing beginning/continuation/completion APIs
retain their applied-parameter contracts. Whole-compression comparison uses the
same explicit tuple and separately decodes both retained outputs. Header-only
contexts are explicitly incomplete and are not passed to a decoder.

The offline tree validator forks the immutable ordinary Greedy replay rather
than modifying a captured dependency. Sequence byte replay, repeat transitions,
raw/RLE rollback, entropy publication, ordered table deltas and fingerprints,
window/frontier continuity, completed frames and aligned whole payload checks
remain. Tree-specific interpretation fixes branch/strategy binding, disables
row metadata and treats odd-cell marker 1 separately from source positions.
It does not claim to reconstruct tree choice from structural invariants; exact
captured maps remain the algorithm expectations for Rust comparison.

Normal and ASan/UBSan helper syntax checks pass with strict warnings. Both normal
and optimized Python accept four structural synthetic controls and one metadata
control and reject eighty forged records each. An audit hook confirms zero
child-process attempts. These synthetic records are never captured fixtures or
claims about C parser behavior. Full C-unit compilation/instrumentation and
actual normal/instrumented source-padding equality remain capture gates.
