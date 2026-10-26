# Single-use prefixes in stable borrowed output

Module 27 design is recorded before implementation. Keep predecessor source
snapshots frozen and promote this candidate only after Module 26 closes. The
bounded API adds single-use raw/prepared prefixes and boundary-only attachment
replacement to SliceDecoder; it does not add in-place decoding or C workspace
mutation.

Represent selection with a private `Attachment<D>` value enum: None,
Persistent(D), PendingPrefix(D), ActivePrefix(D). Exactly one dictionary handle
or borrowed view is owned at a time. Persistent selection needs no per-frame
clone. A pending prefix becomes active by moving the same D after a complete
standard header parses successfully, before dictionary-ID and resource-limit
checks, matching the existing buffered Decoder's claim point. Skippable frames,
partial headers and headers rejected during parsing do not claim it. A valid
standard header followed by ID/limit/block/checksum failure does claim it.

Completion releases ActivePrefix, including empty standard frames. Failure
retains an active handle until reset or drop, while denying output access.
Reset releases ActivePrefix but preserves PendingPrefix and Persistent. A
pending prefix survives a partial or malformed header followed by reset. A
completed skip leaves it pending. No state stores an interior view into D;
block decoding derives a temporary immutable view and uses the existing
borrowed dictionary sequence sink. `dictionary_retained_capacity()` reports the
one reachable payload for any populated variant and excludes it from scratch
capacity as before.

Add `with_prefix(output,options,SharedDictionary)` and generic
`with_prefix_in(output,options,D,allocator)` constructors. Preserve all previous
constructors and default generic arguments. Add `set_dictionary(Option<D>)`
and `set_prefix(D)` only at a clean Header phase with zero buffered bytes.
Successful replacement releases the old selection and clears pending prefixes
or persistent dictionaries respectively. Rejection leaves session selection,
phase, counters and scratch unchanged; the passed-by-value argument follows
ordinary Rust drop behavior. Completed/failed sealed sessions require reset
before reconfiguration. A frame boundary allows replacement for the next frame
without resetting accumulated output or counters. Destination identity and
capacity remain fixed for the entire session.

The enum and its transitions are ordinary ownership/value operations shared by
safe and unsafe configurations. No artificial unsafe representation is needed.
Actual dictionary reads and output matches remain delegated to the independent
unsafe-dictionary and unsafe-slice-history implementations, with their existing
checked ranges and lifetime proofs. All codec code remains no_std plus alloc;
caller dictionary and scratch allocation domains remain independent.

Relevant pinned upstream counterparts are ZSTD_DCtx_refPrefix_advanced,
ZSTD_DCtx_refDDict, ZSTD_getDDict and its ZSTD_use_once state in
lib/decompress/zstd_decompress.c, and the dictionary/prefix lifecycle groups in
tests/zstreamtest.c:1198,1240,1270,1299. C context reset/parameter-clearing rules
are not identical to this bounded Rust API. Explicit boundary replacement and
reset semantics must be documented and independently tested rather than
assuming opaque-context equivalence. Upstream source remains outside the repo.

Qualification must establish claim timing and lifetime on valid, empty,
skippable, truncated and malformed frames; nonzero dictionary IDs; exact/short
output allowances; prefix-once concatenations; reset before/after claim; and
transactional replacement at each phase. Use shared-owner weak references,
borrowed-view compile failures and independent nonclone allocator identities.
Exercise every scratch failure ordinal after prefix claim and verify reset/drop
release both payload domains without exposing output from failed sessions.

Native normal and ASan/UBSan stable-output prefix helpers must compare whole,
one-byte and seven-byte schedules against independent originals and actual
guards. Record differences in claim/reset semantics explicitly. Extend the
existing dictionary_decompress corpus supplement to its prefix branch without
changing any original tail selector. Its fixed seven-byte streaming schedule
remains an added supplement to a native one-shot target, not a newly discovered
original branch. Retain the earlier one-shot/persistent campaign receipts.

Full safe, every isolated feature and all-feature regression/no_std/Clippy,
safe/all release, strict foreign Miri and concrete no-atomics code generation
must precede performance claims. Freeze prefix-once complete-frame workloads
and whole/seven-byte schedules, compare stable/buffered output and allocation
ledgers, and retain all safe/unsafe binaries plus independently rebuilt repeat
results. Prefix preparation and destination allocation stay outside clocks;
cold context creation, handle clone/drop, scratch and decoding stay inside.
Reused-session performance is a separate scope.
