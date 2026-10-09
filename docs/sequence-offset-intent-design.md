# Explicit sequence offset-intent interface

Module 12 interface design, 2026-10-09. The API and checks below are planned,
not implemented or validated results. This refines the
[Fast design](reference-fast-design.md) without admitting Fast execution yet.

Reviewed HEAD: `07bf90745aadfb072107a5a3dc3ff4fc1e7f1c0a`. Source SHA-256:

- `src/sequence_plan.rs`: `9a7041cf32e411764ebc2ce445a0d4bb017861bde5d834a4aab11f7914aa0d06`
- `src/sequence_context.rs`: `e2979c4ba3ab89889c7f3345d7d226dbbdebc09f03e78b4735249ad7ff5491c6`
- `src/sequence_encode.rs`: `fefd122d7f15591ce96977d591a4ad6f8a1762b61b324a967f1c314674c9fc61`

## Recommendation

Keep `LogicalSequence`, `PlannedSequence::new`, and
`SequenceEncoder::prepare(&[LogicalSequence])` unchanged as the canonical
named-policy interface. Add a separate small intent-bearing sequence record for
Fast, a pure checked offset transition, a committed-repeat snapshot, and a
second preparation method. Reuse the existing owned plans, symbols, entropy
tables, section, and writer; add no owner or pending-state field to
`SequenceEncoder`.

Prefer one vector of intent-bearing records over separate parallel logical and
intent vectors. The latter requires equal-length checks, synchronized reserves,
paired insertion/clearing and recovery after only one reservation succeeds.
It creates another owner merely to attach one scalar to each sequence. A
Fast-only record avoids those invariants without increasing the existing named
parser's record size or changing any of its constructors.

## Exact proposed crate-private interface

In `sequence_plan.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OffsetIntent {
    Explicit,
    Repeat1,
    Repeat2,
    Repeat3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedOffset {
    pub(crate) offset_value: u32,
    pub(crate) next_repeat: [u32; 3],
}

impl OffsetIntent {
    pub(crate) fn resolve(
        self,
        literal_length: u32,
        distance: u32,
        repeat: [u32; 3],
    ) -> Result<ResolvedOffset>;
}

impl PlannedSequence {
    // Existing new(LL, ML, distance, repeat) remains canonical.
    pub(crate) fn with_offset(
        literal_length: u32,
        match_length: u32,
        distance: u32,
        intent: OffsetIntent,
        repeat: [u32; 3],
    ) -> Result<Self>;
}
```

In `sequence_context.rs`, alongside the unchanged `LogicalSequence`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SequenceWithOffset {
    pub(crate) sequence: LogicalSequence,
    pub(crate) offset: OffsetIntent,
}

impl<A: Allocator> SequenceEncoder<A> {
    pub(crate) const fn committed_repeats(&self) -> [u32; 3];
}

impl<A: Allocator + Clone> SequenceEncoder<A> {
    // Existing prepare(&[LogicalSequence]) remains canonical.
    pub(crate) fn prepare_with_offsets(
        &mut self,
        sequences: &[SequenceWithOffset],
        initial_repeat: [u32; 3],
    ) -> Result<()>;
}
```

All these are plain Copy values or synchronous borrows, with no pointers to
owned fields, no allocator bound for pure planning, and no additional unsafe
feature. Do not promise enum discriminants, padding or a portable record byte
size. Capacity accounting uses `size_of::<SequenceWithOffset>()` for the new
parser owner; ordinary named `LogicalSequence` storage remains unchanged.

`Repeat1`, `Repeat2` and `Repeat3` name wire offset values 1, 2 and 3. They do not
name fixed indices in the previous-repeat array: literal-length zero changes
their meanings. In particular Repeat1 with zero literals resolves the second
previous offset. This definition follows the format and the stored C offBase,
not a potentially misleading parser-local nickname.

## Checked transition and planning contract

`OffsetIntent::resolve` is the sole shared implementation of explicit/repeat
wire transitions used by Fast's local state and checked sequence preparation.
It does not inspect source bytes or assert match eligibility.

- Reject zero distance or any zero incoming repeat with `CorruptData`.
- Explicit always uses checked `distance + 3`, even when that distance equals
  an existing repeat. Its next triple is `[distance, repeat[0], repeat[1]]`.
- Repeat1/2/3 evaluate the selected wire value under the actual LL-zero rule,
  reject zero/underflow, and require its decoded distance to equal the supplied
  byte-verified distance. Never silently select another valid repeat value.
- A repeated `u32::MAX` distance can remain representable; an explicit
  `u32::MAX` distance cannot. Check explicit arithmetic only in that branch.
- Return the selected wire value and the full next wire-semantic triple.
  Literal length is used only to distinguish zero here; overall block/length
  validity belongs to `PlannedSequence::with_offset` and the section context.

`PlannedSequence::with_offset` preserves every existing length, code and extra
bit bound from `new`, then builds codes from the resolved offset. A shared
private constructor can assemble the existing fields after offset selection.
`new` retains its current smallest-valid-wire-value search and therefore all
existing named output choices. No canonicalization occurs in `with_offset`.

For example, LL=1, ML=4, distance=10 and incoming `[10,20,30]` produce:

| Choice | Wire value | Next full repeat state |
| --- | --- | --- |
| Existing canonical `new` | 1 | `[10,20,30]` |
| Explicit intent | 13 | `[10,10,20]` |

A subsequent zero-LL Repeat1 must resolve distance 10 in the explicit path,
but distance 20 in the canonical path. Comparing decoded bytes alone can miss
the lost intent until a later sequence; preserve this as a direct witness.

## Preparation and publication contract

`committed_repeats()` copies only `self.repeat`. It never exposes
`pending_repeat`, including while a complete candidate exists.

The Fast caller snapshots that triple before parsing and passes the same value
to `prepare_with_offsets`. Preparation first discards any older candidate, then
rejects `initial_repeat != self.repeat` with `InvalidState` before allocating.
This catches using a parse after a context reset, dictionary installation, or
intervening commit. It is a value-consistency check, not a frame-generation
token: source eligibility, frame identity and selected mode remain the caller's
responsibility. Equal triples do not prove those other facts.

After this guard, preparation mirrors the existing flow:

1. Check sequence count. Empty input produces the existing one-byte zero-count
   section and preserves all committed tables and repeats.
2. Reserve the existing plan vector and three symbol vectors before Copy pushes.
3. Starting from the committed snapshot, call `with_offset` in source order,
   check cumulative reconstructed sequence bytes against the block bound, and
   derive the next repeat solely from each checked plan.
4. Reuse existing FSE selection and `encode_sequences`, then reserve and assemble
   the complete section. Only after fallible work succeeds install pending
   replacement tables, modes and the candidate triple.
5. Any malformed intent, length failure, unsupported table or allocation failure
   calls `discard`: committed state is unchanged and `as_bytes()` is empty.

The same existing `commit()` publishes tables/repeats after successful compressed
block output. `discard()` handles raw/RLE fallback or rejection. Both retain
allocated scratch and need no new allocation. Repeated commit remains an
idempotent no-op. A zero-sequence compressed block does not update repeats.

No pending-repeat getter or second committed triple is required. Fast may use
`OffsetIntent::resolve` to maintain a parser-local full candidate triple; it is
temporary and is discarded if the block falls back. The next block starts from
`committed_repeats()`, not that abandoned local value. C's temporary disabled
repeat probes remain separate availability state and must never install zero
into this full wire triple.

The source-byte match/window checks remain the parser's responsibility. The
shared block emitter additionally verifies literal consumption and total block
reconstruction against its original block length. The sequence context itself
has no block input or trailing-literal slice and cannot validate their equality.

## Internal sharing without another allocation

The current preparation loop can share a private helper accepting the exact
sequence count and a monomorphized planning closure `(index, current_repeat)`.
The canonical caller invokes `PlannedSequence::new`; the explicit caller invokes
`with_offset` on its intent-bearing record. Both use the same reserve order,
plan/symbol pushes, FSE choices, serialization and pending publication code.
No trait object, boxed callback, iterator collection, Vec clone or temporary
conversion vector is needed. The private helper is not a new public callback
surface.

Fast owns one reusable `Vec<SequenceWithOffset, A>` with its exact allocator,
rather than both logical and intent vectors. Existing context-owned
`Vec<PlannedSequence, A>` remains reusable. Do not store a complete larger
`PlannedSequence` per parser result merely to pass through checked preparation;
the parser can resolve its offset using the small pure helper.

`sequence_encode.rs` needs no production signature change. It already consumes
checked `PlannedSequence` values and verifies codes, extras, explicit distances,
workload bounds and table support before writer mutation. It intentionally does
not infer incoming repeat state or publish `next_repeat`. Contextual validation
stays above it, where the authoritative initial triple is available.

## Meaningful focused checks and small implementation boundaries

**Planner checkpoint:** Add the value records and explicit checked planner.
Cover all six LL-zero/nonzero repeat branches; explicit distances equal to
repeat0/1/2; duplicate offsets where a different wire choice changes later
state; rep0-minus-one underflow/zero; mismatched intended versus decoded
distance; explicit overflow versus valid large repeat; current block limits.
Retain existing canonical vector equality. Invalid repeat wire values are
unrepresentable in this enum; any future numeric fixture adapter must reject
values outside 1..=3 at its own boundary.

**Context checkpoint:** Add the snapshot and explicit preparation path. Prove
prepare never changes the snapshot, commit changes it once, raw/RLE-style
discard preserves it, empty prepare/commit preserves it, and replacing a
pending candidate restarts from committed state. A stale supplied snapshot
rejects before allocation, discards prior pending bytes, and preserves committed
tables/repeats. Decode a sequence chain that fails if explicit intent was
canonicalized. Inject each observed allocation failure for a small explicit
workload, preserve exact owner identities, and include borrowed non-Clone and
distinct allocator domains. Existing canonical bytes and owner/capacity checks
remain unchanged.

**Fast integration later:** Replay complete C wire intent, independently replay
the full wire triple, compare the two parser-used C repeat cells separately,
and verify compressed/raw/RLE transitions using the actual selected-block
emission path. A standalone parser trace is not publication evidence. Table
sampling may survive emitted raw/RLE blocks; entropy/tree/repeat state does not.

These are planned checks, not claims of completed validation. This interface
proposal makes no new upstream coverage or numeric compression claim.
