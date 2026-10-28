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

## Foreign targets and bounded dictionary corpus

Eight platform commands pass: safe/all Miri on i686 little-endian and s390x
big-endian, plus safe/all thumbv6m core+alloc checks and concrete ARM IR/object
emission. The actual coordinator executes prepared/raw ownership transitions,
concatenations with skippables, subsequent-call consumption, persistent error
recovery and real 24-byte overlap. The no-atomics probe instantiates in-place
coordinator decoding and emits the movement path. This proves interpreter and
code-generation checks, not execution on foreign hardware. The full codec
requires no_std plus alloc; shared dictionary ownership uses the established
no-atomics fallback.

The additive dictionary_decompress adapter preserves all original input selectors
and the existing incremental-prefix outcome ledger. All 12,063 fixed corpus
members run with safe/all release and no skips. Each additionally compares both
call-scoped output modes against the stateless persistent dictionary view, with
exact Result, complete successful bytes, guards and selection state checked.
Call-scoped prefixes deliberately are not equated to incremental frame prefixes.
The historical ledger still records 11,768 Rust-trained dictionaries, 295 training
fallbacks and 20 successful legacy-adapter decodes; that ledger is not a separate
count of native success or newly coordinated successes. Training remains the
independent Rust algorithm, not byte-identical native fastCover.

Evidence: `evidence/2026-10-27-one-shot-selection-platform.json` and
`evidence/2026-10-27-one-shot-selection-corpus.json`. The archived corpus receipt,
not this prepared report, gates integration of the corpus result.

## Complete feature and release regression

The final frozen candidate passed all 72 commands: safe/all and 21 isolated
unsafe features each have complete tests, true no_std and strict Clippy; safe/all
complete release and formatting also pass. Every full/release suite reports
1,723 passed / 71 ignored across 108 result groups. External adapters retain their
separate explicit execution receipts; ignored tests are not credited as passes.
Focused preparation and borrowed benchmark baseline rejections remain retained.
Native empty-input differences and call-versus-frame prefix scope remain explicit.
Evidence: `evidence/2026-10-27-one-shot-selection-feature-matrix.json`.

## Measured deterministic comparisons

All 23 retained primary backends and independently rebuilt safe/all repeats have
identical clock-free dictionary/scratch ownership and guard qualification and complete raw timing
records. Nine coordinator/setup/restoration modes are timed separately with equal per-operation clock overhead.
Fresh primary/repeat CPU barriers precede sequential measurements. See
`one-shot-selection-benchmarks.md`; the host results do not establish a universal
unsafe speedup. Evidence: `evidence/2026-10-27-one-shot-selection-benchmarks.json`.
