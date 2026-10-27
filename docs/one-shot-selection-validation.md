# One-shot dictionary selection validation

Module 32 is in progress. Focused gates below are complete; full feature/release
regression, measurements and final reconciliation remain separate gates.

## Selection ownership

The private `Selection<D>` record stores none, persistent ownership or next-call
ownership. `claim` borrows persistent ownership or removes a next-call owner into
a local `Claimed` record before any decode/preflight/allocation. No handle clone
or dictionary/history copy occurs. Both records use the same safe representation
for all backends; they do not need artificial unsafe operations.

Four unit tests cover non-Clone move/drop counts, persistent identity, replacement,
borrowed lifetime and unwinding. Ten focused commands pass: safe/cursor/all unit,
true no_std and strict Clippy, plus formatting. A staging declaration is private
and temporarily allows dead code until the public step. The initial public
preparation expected an incorrect contiguous rustfmt comment/declaration layout;
it stopped before public checks. No zero-test or failed preparation is credited.
Evidence: `evidence/2026-10-27-one-shot-selection-core.json`.

## Public one-shot coordinator

`OneShotDecoder<D = SharedDictionary>` holds copied options and selection only.
It accepts sealed shared owners or borrowed dictionary views. The existing
DictionaryHandle contract includes Clone, but this coordinator never invokes it. Direct and suffix-input methods delegate existing checked engines, using
either Global or an independent per-call allocator. No input/output borrow or
scratch allocation is retained. Options/selection replacement has explicit Rust
ownership; native context workspace/reset equivalence is not claimed.

All 26 focused commands pass across safe, in-place movement, slice history,
dictionary and all-feature selections: four units, eight public tests and three
doc examples per configuration, true no_std, strict Clippy and final formatting.
Tests cover raw/prepared/HTTP dictionary dependence, concatenated frames and
skips within one call, subsequent-call consumption, persistent retention,
replacement, errors/limits/checksums/IDs, empty and skippable-only calls, invalid
suffix coordinates, real overlap, borrowed non-Clone allocators, every observed
scratch failure ordinal and owned-handle release. Compile-fail examples prove
borrowed dictionary lifetime and input/output alias rejection.

A next-call prefix is consumed before every attempt and covers all frames in that
call. Incremental frame-scoped prefixes remain a different contract. Existing
provisional-output and successful-suffix guarantees are unchanged.
Evidence: `evidence/2026-10-27-one-shot-selection-public-api.json`.

## Native context lifecycle schedules

External v1.5.7 helpers use actual persistent by-reference DDict selection and
raw/auto `ZSTD_DCtx_refPrefix_advanced` calls. Normal and ASan/UBSan builds run the
same 24 scenarios: 81 setup/decode steps in each direct/in-place mode, including
106 actual decode attempts. Full successful output bytes and true outer/allowance
guards are retained; normal/sanitized ledgers agree. Scenarios include dependent
concatenations, separate subsequent calls, replacement/clearing, empty dictionaries,
errors, skippable-only input and 120 KiB actual raw-buffer overlap.

Five Rust backends replay all 106 attempts and agree on exact result/state ledgers,
complete successful bytes and the stronger Rust unused-suffix guarantee. Four
empty-input attempts return native zero success versus Rust UnexpectedEof; both
consume the pending prefix. Native numeric errors are not equated with typed
Rust error identities. Helpers do not establish dictionary allocation accounting
or leak detection; independent Rust ownership ledgers cover those stated Rust
contracts. macOS sanitizer runs use detect_leaks=0 with bounds/UB checks active.
These are supplemental schedules, not original upstream assertion upgrades.
Evidence: `evidence/2026-10-27-one-shot-selection-native.json`.
