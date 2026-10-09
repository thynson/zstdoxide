# Compression parameters and context lifecycle

## Scope and completion boundary

Module 10 adds allocation-free parameter inspection and the configuration/reset
contracts of the already supported encoder. It does **not** add operational
numeric compression levels. Its reference is facebook/zstd v1.5.7, commit
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`.

Two interfaces remain distinct:

- Reference parameter inspection reports the requested and resolved settings of
  the pinned reference, within the documented size domain. It can describe
  algorithms that the Rust compressor does not implement.
- Live encoder configuration controls only mechanisms implemented by
  `EncoderOptions` and `CompressionStrategy`. It does not accept a reference
  strategy or numeric level and silently substitute a named Rust policy.

The existing `Greedy`, `Chain`, and `Lazy` policies, default options, enabled
checksum, 128 KiB retained output history, and default encoded bytes remain
unchanged. Module 10 completion is not numerical-level support, C context API
equivalence, or completion of the official test migration.

## Existing implementation and reference counterparts

`EncoderOptions` currently stores concrete frame/window settings and an
independent named policy. `Encoder::reset()` clears the per-frame pledge and
session state while retaining persistent settings, dictionaries, unclaimed
prefixes, and allocations. `reset_with_options()` validates before replacement
and retains dictionary selection. Neither method means "reset parameters to
defaults and remove dictionaries".

The reference instead keeps `requestedParams` distinct from applied compression
parameters. Explicit tuning overrides survive changes to the requested level;
source size, dictionary size, and preparation mode affect resolution. Relevant
definitions are `lib/zstd.h:322-415`,
`lib/compress/zstd_compress_internal.h:373`, and
`lib/compress/zstd_compress.c:1623-1650`.

At the Module 10 design checkpoint, Rust `Greedy` probes one four-byte hash candidate. `Chain` examines
eight candidates, and `Lazy` examines sixteen at current/next positions. Heads,
node limits, key width, and history retention are fixed; seeds/history are
rebuilt for each block. These names are not aliases for the reference's nine
strategies. Exposing reference hash/search/chain parameters without changing
their consumers would create ineffective controls.

Module 11 subsequently made those named-policy dimensions and efforts
operationally configurable; see [match-search design](match-search-design.md).

## Value records and ownership

Names below define the intended API roles; final public method names should be
settled in the first implementation item and then used consistently.

| Record | Ownership and invariants | Reference counterpart |
| --- | --- | --- |
| `SourceSize::{Unknown, Known(u64)}` | Plain value; empty input is distinct from unknown. Reserved-sentinel and overflow policy is checked before arithmetic. | Source-size arguments and `ZSTD_CONTENTSIZE_UNKNOWN` |
| `ReferenceStrategy` | Separate nine-variant enum: Fast, DoubleFast, Greedy, Lazy, Lazy2, BinaryTreeLazy2, Optimal, Ultra, Ultra2. Never converted implicitly to `CompressionStrategy`. | `ZSTD_strategy`, `lib/zstd.h:335` |
| `CompressionParameterRequest` | Private fields, checked setters/getters, requested level, automatic/explicit overrides, optional source hint. Setter failure leaves every field unchanged. | `ZSTD_CCtx_params`, requested values |
| `RawCompressionParameters` | Seven public `u32` fields. Validation rejects invalid concrete inputs; adjustment explicitly applies the pinned signed-32 clamp. No validated invariant is implied by construction. | Unvalidated `ZSTD_compressionParameters` boundary |
| `ResolvedCompressionParameters` | Private, concrete validated window/hash/chain/search logs, minimum-match search setting, target length, and reference strategy. No source ownership or allocation promise. | `ZSTD_compressionParameters` |
| `ReferenceFrameParameters` | Separate value record for reference content-size/checksum/dictionary-ID defaults. Does not change `EncoderOptions::default()`. | `ZSTD_frameParameters` and `ZSTD_getParams` |
| Reset mode/configuration snapshot | Plain enum and values. Session state and dictionary handles stay in the existing encoder owner. Active frame settings cannot change retroactively. | `ZSTD_ResetDirective`, requested/applied separation |

These records contain no pointers, links, buffers, or dynamic storage. They use
safe Rust under every feature configuration and need no artificial unsafe
counterpart. Existing allocator owners and access backends remain responsible
for live codec storage. New search structures are deferred and require their
own prior ownership analysis and independently selectable access backend where
performance-sensitive.

Prefer private validated fields over public structs that can represent
arbitrary invalid resolved settings. Automatic values use `Option` or a small
`Auto/Explicit` enum, not an ambiguous zero in the internal representation.
Unvalidated values needed to adapt C's wide adjustment contract are temporary
boundary inputs; they must not be presented as validated resolved records.

## Pure inspection contracts

The module provides three distinguishable operations:

1. Validate concrete parameter values without clamping them.
2. Inspect level-derived parameters and reference frame defaults.
3. Adjust parameter values for source/dictionary sizes, with the reference's
   initial clamping and subsequent resolution rules.

Request resolution first adjusts the selected level preset for the supplied
source/dictionary information, then applies explicit overrides, then adjusts
again. The two adjustments cannot be collapsed: for level three, known empty
input and an explicit Ultra2 override, the first pass downsizes the chain log
to six; applying the override before a single pass would incorrectly yield
seven. Resolution returns a new value without changing the request. A getter
for a request reports the requested override, not an effective value substituted
during compression planning.

The pinned level table has four size tiers, bounded at 16 KiB, 128 KiB, and
256 KiB, and a default tier above them. It is in
`lib/compress/clevels.h:25-126`. Import derived constants with attribution and a
source hash; use idiomatic Rust lookup and checked arithmetic. Level zero means
reference default level three; negative levels select the fast baseline and
acceleration; excessive levels clamp according to the pinned contract
(`zstd_compress.c:7673-7674`, `:7759-7780`). Avoid negating `i32::MIN` before
clamping.

The same positive level can resolve to different strategies: level two selects
Fast or DoubleFast, and level seven selects Lazy or Lazy2. A successful profile
query therefore says nothing about whether a live Rust encoder can execute it.
Numeric encoder construction and profile admission are not part of this module.

`ZSTD_getParams` enables content size and dictionary ID and disables checksum
(`lib/zstd.h:1946-1949`). Those are reference-query defaults. Existing Rust
encoder defaults continue enabling checksum.

### Source-size and platform policy

- Internal typed resolution treats `Known(0)` as empty input. Test adapters for
  public C `ZSTD_getCParams` and `ZSTD_adjustCParams` convert a zero argument to
  `Unknown`, as those entry points do at `zstd_compress.c:1614-1618` and
  `:7784-7790`. The API must name or document the distinction; do not overload one
  zero-valued Rust argument with both meanings.
- `Known(u64::MAX)` is rejected because that value is the reserved unknown
  sentinel in the pinned reference. Ordinary known-source/dictionary sums are
  checked; unrepresentable sums return `ResourceLimit` before mutation.
- Unknown source with no dictionary retains unknown tier selection. Unknown
  source with a dictionary uses the reference's effective dictionary-size-plus-
  499 rule, checked for overflow. The C expression at
  `zstd_compress.c:7735-7750` obtains this through unsigned sentinel arithmetic.
  Rust should express the case directly. C also wraps some extreme known-size
  combinations; record those oracle results as intentional domain differences,
  not accidental debug/release differences or claimed parity.
- C's public concrete bounds depend on pointer width: maximum window log 30/31
  and maximum chain log 29/30 on 32/64-bit targets (`lib/zstd.h:1263-1273`).
  Reference inspection follows documented target bounds. This does not narrow
  the separately supported Rust frame-header/window-advertisement contract.
- Public `adjustCParams` clamps invalid concrete fields before adjustment;
  `checkCParams` rejects them (`zstd_compress.c:1386`, `:1610`). Automatic request
  fields must be resolved before concrete validation. Adjustment can further
  reduce memory-related values for small sources/dictionaries.

Dictionary preparation/attachment modes affect resolution. The current Rust
`Dictionary` owns validated content and entropy, not C's compression-profile-
specific `CDict` search tables. Module 10 must not imply compiled-dictionary
parameter inheritance. The initial inspection contract should cover the public
reference query mode and typed no-attached-dictionary request resolution;
additional preparation/attachment inspection modes require their own explicit
fixtures before exposure.

## Supported live configuration and reset

Keep `EncoderOptions` and named policies as the live configuration boundary.
Add a checked operation to replace supported settings at a clean configuration
boundary without discarding a live frame. Invalid values or an invalid phase
leave options, dictionaries, counters, buffers, and poison state unchanged.
All fields that affect an active frame remain fixed until its next reset.

Provide narrow, clearly named parameter-only and session-plus-parameter reset
methods in addition to the existing session reset. They do not add inactive
numeric tuning fields to the live encoder. Their effects must be separate:

| Operation | Session and counters | Settings | Dictionary/prefix selection |
| --- | --- | --- | --- |
| Existing `reset()` | Discard session, clear pledge, recover from processing error | Preserve persistent settings | Preserve persistent dictionary/unclaimed prefix; drop claimed active handle |
| Existing `reset_with_options(options)` | Validate, then discard session | Install supplied options | Preserve existing selection contract |
| Parameter-only reset | Reject an active/failed session; do not silently recover it | Restore existing Rust defaults | Clear dictionary and prefix references |
| Session-plus-parameter reset | Discard session and recover | Restore existing Rust defaults | Clear dictionary and prefix references |

Parameter-only reset may run in a fresh clean context or after a completed
frame. If the encoder is `Finished`, it remains `Finished`; a session reset is
still required before processing another frame. This preserves the existing
Rust explicit-reset protocol. It is not C's automatic next-frame protocol.
Session-plus-parameter reset returns a ready context. None of these operations
implicitly releases retained allocations or implements automatic shrinking.
Restoring Rust defaults clears `pledged_size` and enables checksum; the pinned
C parameter-only reset instead preserves a fresh context's existing pledge and
restores its own checksum-disabled defaults. These are intentionally different
contracts, not evidence of C reset equivalence.
Clearing dictionary selection releases every corresponding handle in the
outer configuration, active frame, block workspace and prepared seed state;
clearing only the outer `Option<D>` is insufficient. Retained scratch capacity
can remain, but no cleared borrowed/shared dictionary may stay referenced by a
workspace. Verify this with ownership/drop observations as well as output tests.

This lifecycle item is encoder-specific. A decoder's explicit caller output and
window limits must not be replaced by invented defaults merely to resemble C's
parameter reset. Any decoder reset extension needs its own resource policy.

The reference reset anchors are `lib/zstd.h:595-608` and
`zstd_compress.c:1367-1382`. Preserve the distinction between sticky parameters,
the single-use pledge, persistent dictionaries, and a consumed prefix.

Do not infer live mutation behavior solely from header prose. In v1.5.7,
`lib/zstd.h:2421` describes mid-compression bulk parameter application, while
`zstd_compress.c:1178-1188` rejects non-init state and an attached CDict.
Individual effort setters at `:708-719` can update requested state after start;
successful getters do not prove an effect on the current single-threaded frame.
Module 10 retains an explicit supported Rust phase contract and records the C
behavior in oracle fixtures. Worker/job-boundary updates are deferred.

## Ordered implementation and validation items

Each item is a separate reviewable checkpoint; the coordinator records its
checkbox in `TODO.md` and commits after its focused validation.

1. **Freeze oracle semantics first.** Add a small pinned-source C probe and
   reproducible fixture importer for bounds, level lookup, adjustment, getter
   mutations, and reset outcomes. Record public-zero conversion separately from
   typed internal empty-size resolution. The completed fixture checkpoint is
   documented in `tests/upstream/parameters/README.md`: 934 rows across seven
   TSV files, with pinned provenance and reproducible byte-for-byte regeneration.
2. **Add value records and validation.** Implement the nine-strategy reference
   enum, source-size type, private concrete fields and checked constructors.
   Verify every parameter boundary, unsupported enum input in the adapter, and
   all-field mutation atomicity. Keep this allocation-free and no_std.
3. **Implement preset selection.** Add attributed table constants, tier choice,
   level normalization/acceleration, source-size policy, and independent
   reference frame defaults. Verify the unadjusted presets against the pinned
   table and oracle-emitted rows. Keep this selection internal until the next
   item supplies complete public level-query adjustment.
4. **Implement adjustment, level queries, and requested overrides.** Match all
   seven fields of frozen public C query/adjustment vectors within the declared
   size domain, including platform bounds and extreme-size rejection witnesses.
   Preserve separate requested/effective values; validate clamping, source
   hints, override ordering, and dictionary-size effects. No query changes an
   encoder or claims that the resulting search plan can execute.
5. **Add live supported configuration replacement.** Validate before mutation,
   preserve existing defaults and dictionary ownership, and reject changes to
   an active/failed/finished frame according to the supported phase contract.
6. **Add explicit reset modes.** Verify partial header/input/output, pending
   Flush/Finish, processing failure, known/unknown pledge, persistent dictionary,
   claimed/unclaimed prefix, retained capacity and allocator-owner lifetimes.
7. **Port the matching assertions and preserve defaults.** Separate exact pure
   scalar fixtures from adapted Rust lifecycle assertions and blocked numeric
   compression branches. Retain default encoded fingerprints and meaningful
   independent C decoding for supported settings; reject zero-test selectors.
8. **Close the parameter milestone.** Run safe/individual/all-feature tests,
   no_std, formatting, Clippy, appropriate width checks, and fixture audits.
   Update source dispositions with their precise remaining branches. No new
   unsafe access family means no invented unsafe benchmark or Miri claim.

Useful delegation boundaries are: one owner for the C probe/importer/fixtures,
one for pure records/table resolution, and one for live context lifecycle/tests.
The coordinator owns public exports, task tracking, shared documentation and
commits. Agree on value APIs before concurrent implementation; oracle generation
precedes implementing the corresponding scalar algorithm.

### Exact upstream assertion anchors

| Source | Assertions to retain or qualify |
| --- | --- |
| `tests/fuzzer.c:570-586`, `:680-710`, `:804-809` | Concrete validation, min/default level, adjustment example, valid derived parameters |
| `tests/fuzzer.c:1722-1760` | Requested getters, explicit hash override surviving level changes, session reset preserving requests, parameter reset restoring defaults; numeric live-context branch remains qualified |
| `tests/fuzzer.c:3001-3032`, `:3088-3120` | Dictionary/parameter ordering, persistent dictionary across session reset, clearing on parameter reset |
| `tests/fuzzer.c:3468-3527` | Negative-level API equivalence and setter-order invariance; actual numeric compression and LDM execution remain later work |
| `tests/fuzzer.c:3641-3667`, `:4332-4355` | Decoder parameter resets, dictionary-sensitive profile lookup and adjustment; decoder extensions remain separately scoped |
| `tests/zstreamtest.c:1600-1628` | Requested parameter preservation and source-hint bounds |
| `tests/zstreamtest.c:2833-3075`, `:3090`, `:3186` | Individual/bulk request setup and post-operation preservation; randomized unsupported algorithm/worker/LDM branches are not silently skipped or called ported |

Oracle inputs include minimum/maximum/outside bounds; zero, negative and all
positive reference levels; tier boundary minus/equal/plus one; empty versus
unknown; dictionary-adjusted boundaries; extreme sizes; explicit-zero-to-auto
transitions; setter order; and failure-before-state-change cases. Compare all
seven compression fields, reference frame flags, returned classification, and
requested state. Do not require C-identical encoded bytes for an independent
Rust compression algorithm, or infer monotonically improving ratios from
increasing a numeric level.

## Subsequent milestones and retained migration gaps

| Milestone | Required structures/contracts | Upstream test anchors and limits |
| --- | --- | --- |
| Configurable existing storage/history | Checked hash/chain dimensions, visit/lookahead budgets, separately bounded search history, fallible owner-preserving reconfiguration and seed compatibility | Current fixed knobs cannot implement reference hash/search/chain semantics; preserve named defaults while expanding actual consumers |
| Fast, DoubleFast and chain families | Sampling/skip policy, repeat probes, distinct short/long hash candidates, configurable chain search and Lazy2; numeric admission only for completely supported resolved profiles | `clevels.h:25-126` changes strategy by source tier; minimum-match settings often select hash-key width rather than a hard emitted-match floor |
| Tree and optimal parsing | Indexed binary-tree links, bounded search, candidate lengths, cost/price tables and dynamic-programming predecessor records, with explicit budgets | `fuzzer.c:1398`, `:1691`, `:3265`; do not map these levels to the existing Lazy policy |
| Stable borrowed sessions | Immutable source/exclusive output borrows, logical visibility and progress indices, direct-history/output paths, explicit destination exhaustion | `zstreamtest.c:786-863`, `:915-1079`; actual buffer elimination must be measured. Compression checks equal remaining output capacity at `zstd_compress.c:6326`; decoding checks exact buffer state at `zstd_decompress.c:2035`. Source append visibility and deferred initialization at `zstd_compress.c:6461` need explicit contracts |
| Long-distance matching | Bounded rolling-hash buckets, position tags, retained source windows, dictionary eviction and sequence integration | `fuzzer.c:1063-1083`, `:1199-1267`, then the heavy ratio case `:1277-1333`; `longmatch.c:39-95` is a separate 8 GiB logical-index test, not LDM |
| Thread pool, then codec workers | Bounded owned job queues, drain/join/resize, overlap ownership, job boundaries and ordered output publication; std only | `poolTests.c:43-85`, `:117-149`, `:175-205`, `:224-253`; `fuzzer.c:393-474`, `zstreamtest.c:1497-1547`. A pool is not single-frame parallel compression |
| I/O and format adapters | Progress-aware Read/Write draining, EOF/truncation, file/CLI contracts, separate seekable index and wrapper state | `examples/streaming_compression.c:58-90`, `streaming_decompression.c:29-74`; CLI long/worker/window scripts, gzip/zlib, seekable and pzstd remain distinct suites |

The larger roadmap also retains static caller workspaces, estimators and
shrinking, external sequence production, advanced training controls, magicless
and legacy coverage where still missing, full external target adapters, and
the strict complete-migration audit. Completing this parameter foundation does
not remove any of those obligations.
