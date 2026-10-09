# Bounded reference Fast validation

Module 12 is in progress. The [design](reference-fast-design.md) limits the first
numeric path to admitted whole-buffer, no-dictionary Fast profiles. Existing
named encoders and their saved default frames remain compatibility inputs.
These foundation checkpoints do not yet expose a numeric compression API or
complete official test migration.

## Private small layouts and sampled table access

`SearchKeyLayout::for_fast` admits hash logs six through eighteen and exact key
widths four through seven; minMatch-three dispatch is resolved separately.
Named public hash/key bounds remain unchanged. Smaller hashes are checked
against 1,024 high-bit projections of the existing independent pinned C vectors,
not newly captured C observations.

`MatchTable` now separates a nonmutating lookup from a checked store so a future
Fast parser can retain its precise delayed-store order under collisions. Lookup
returns a copied nonempty candidate without imposing a cursor; the caller must
still validate its source/window range and bytes. The sentinel clears a bucket.
Both safe and opt-in unsafe access reject incompatible widths and indices beyond
initialized length before mutation, including spare-capacity test states.
Existing named `replace` behavior is unchanged. Smaller layouts preserve the
exact borrowed allocator, make no request for in-capacity reshaping, and leave
logical state intact when a later growth fails.

All twenty final commands pass on a frozen copy of `e6942ff` plus these three
owned source files. Four feature/std configurations each pass 32 focused tests;
safe/all default replay passes seven tests each, release unsafe bounds passes
one, and strict-provenance host Miri passes three new sampled-access tests.
This is 146 passing test executions. Strict all-target/all-feature Clippy,
formatting and no_std safe/all compilation pass. The original default fixture
expectations are unchanged.

[Foundation evidence](evidence/2026-10-09-fast-table-foundation.json) preserves
all logs, exact commands, compiler, source manifest and reproduction runners.
It also retains an initial broad selector's 140 passing tests and five explicit
oracle ignores: the runner stopped on its overly strict no-ignore guard before
switching to focused selectors. No codec failure or correction is hidden by that
runner change. This checkpoint introduces no unsafe parser, new allocation owner,
C throughput measurement or complete foreign-platform execution claim.

## Explicit wire-offset intent

`OffsetIntent` names explicit distance or wire repeat values one, two and three.
Its checked transition validates the byte-verified distance against literal-zero
semantics and returns the complete next repeat triple. `PlannedSequence::with_offset`
preserves that choice even when another wire value describes the same distance.
The existing canonical constructor still chooses the smallest valid repeat value;
its emitted symbol and extra-bit planning now share the same checked construction.
These plain value records introduce neither allocation nor unsafe code.

Six new tests distinguish all zero/nonzero-literal repeat branches, full-width
literal lengths, duplicate repeat values, explicit-equals-repeat choices and their
effect on a following sequence, invalid intents and arithmetic/block boundaries.
All fourteen commands pass: eleven planner tests in each of four safe/feature/
no_std configurations, 23 existing context/emitter tests under all features and
seven original default-frame tests each in safe/all release builds. This totals
81 passing executions with two existing explicit C-oracle ignores. Strict
all-target Clippy safe/all, no_std safe/all compilation and owned formatting pass.
Baseline fixture hashes remain unchanged.

[Offset evidence](evidence/2026-10-09-fast-offset-planning.json) preserves exact
commands, logs, source hashes and the runner. The owned planner source and baseline
inputs stayed frozen; independent working-tree edits mean this is not the final
immutable whole-repository matrix. Context snapshots and intent-bearing sequence
preparation remain their own item. No additional C, Miri or performance execution
is attributed to these tests.

## Known-size Fast profile admission

`FastExecutionPlan` resolves the existing request against actual known input
length and dictionary size zero before admitting a strategy or allocating any
owner. It derives a checked private layout, actual usize-cell table payload,
window/block limits, paired sampling step, literal policy and reference frame
defaults. Valid non-Fast strategies return UnsupportedFeature; Fast window/hash
requirements beyond the documented execution caps return ResourceLimit. Existing
scalar validity and known-size rules remain in the reference resolver.

Thirteen tests distinguish the captured four source tiers, negative levels,
level-two Fast/DoubleFast transitions, explicit strategy overrides, actual size
versus misleading hints, resource caps after adjustment and unused-but-validated
chain/search fields. Fixture-backed observations and additional source-derived
combinations are labeled separately. The new execution plan neither changes the
named policy APIs nor allocates a chain for unused reference fields.

All nine checks pass with 52 unchanged recorded source/dependency inputs: each
of four safe/feature/no_std configurations passes all thirteen tests, strict
library Clippy safe/all passes, both no_std library checks pass and owned
formatting passes. [Admission evidence](evidence/2026-10-09-fast-profile-admission.json)
retains commands, complete logs, compiler/environment, before/after hashes and
count guards. No new parser, public compression function or C campaign is
claimed by this allocation-free foundation.

## First bounded direct-C Fast observations

The [Fast parser smoke collection](../tests/upstream/fast-parser/README.md)
freezes nine deterministic recipes with two initialized padding patterns each.
The unchanged pinned `ZSTD_compressBlock_fast` produces sequences, literal bytes,
raw parser repeats, full independently replayed wire repeats and final hash
buckets. Explicit prior-prefix, duplicate-repeat, tiny-input, maximum acceleration
and expanded long-match metadata witnesses are included. Table observations do
not claim chronological probe/store traces or coverage of every dispatch branch.

The original final normal/sanitized captures each pass eighteen observations;
portable regeneration repeats both builds and matches every complete case record.
ASan/UBSan instrument the helper, unchanged Fast source and inline helpers;
archive support objects and leak detection remain outside that claim. Earlier
successful captures and metadata/path-review corrections retain their separate
counts and logs. No Rust Fast algorithm was used to generate expectations.

Normal and optimized Python offline checks pass four positive controls and 78
expected malformed/schema/path-overlap rejections, without a compiler search
path. Independent review tightened report/output path separation before file
creation and bound captured metadata to the observation header. The semantic
checker is registered in both check runners; generic integrity now verifies
325 fixture files in eleven required collections. No source migration status is
promoted merely because this helper collection exists.

[Smoke evidence](evidence/2026-10-09-fast-parser-smoke.json) and its
[compact archive](evidence/2026-10-09-fast-parser-smoke.tar.gz) preserve source
attribution, original/portable commands, complete logs, record hashes and
instrumentation limits. Executable/debug bytes are omitted while their recorded
hashes remain. Broader direct families, the sequence collector and ordinary
frame publication are still open prerequisite oracle items before Rust Fast
parser implementation.

## Shared block entropy ownership

`BlockEntropyEncoder` now owns the existing literal encoder, current/candidate
Huffman trees, sequence encoder and payload vector. The named workspace retains
its parser vectors and search/history owners. `from_parts` moves complete child
owners without a new factory, clone or allocation; the named constructor creates
locals in the original interleaved order. A focused compatibility witness fixes
the original allocator identity one, literal-buffer identity two, sequence-buffer
identity six, thirteen clones/fourteen total owners and zero initial requests.
A separate test assembles children from three independent allocator domains and
verifies unchanged snapshots, moved payload address and correct final release.

Dictionary literal validation still precedes seed preparation; fallible sequence
setup still follows it. Output reservation, early RLE, literal/sequence selection,
payload reserve/append, compressed publication, fallback and history advancement
retain their previous order. Default bytes and capacity accounting are unchanged.
Grouping fields changes their destruction interleaving; exact allocator identity
and complete release are preserved, without promising the old field-drop order.
This neutral extraction also preserves internal stale payload/pending scratch
semantics. The later standalone Fast preparation must enforce the stronger
candidate-validity/reconstruction contract before exposing its output.

All 23 commands pass across two explicit frozen snapshots. Four encoder-family
suites each pass 162 tests; safe/all original default replay passes fourteen
executions and public control/failure campaigns pass twenty. After adding the
two ownership tests, four configurations pass all eight new executions. The
aggregate is 690 passes and twelve existing explicit oracle ignores. Strict
all-target Clippy safe/all, no_std safe/all and formatting pass; the test-only
supplement also passes strict Clippy/all and formatting. No new C, Miri or
throughput campaign is attributed to this structural extraction.

[Ownership evidence](evidence/2026-10-09-block-entropy-ownership.json) and its
[archive](evidence/2026-10-09-block-entropy-ownership.tar.gz) preserve logs, both
manifests, initial/final owned sources, independent review and runners. Concurrent
sequence-context work was excluded from both snapshots.

## Transactional intent-bearing sequence preparation

`SequenceWithOffset` retains each logical match and its chosen wire value.
`SequenceEncoder::committed_repeats` exposes only the emitted frame's triple;
`prepare_with_offsets` discards an older candidate and rejects a mismatched
snapshot before any allocation, including empty input. This is a value check,
not a frame identity token. Canonical and explicit preparation share the existing
reservation/planning/selection/serialization flow through a monomorphized helper,
with no new context field, allocation owner or unsafe operation.

Six new tests exercise stale snapshots without allocation/cloning, empty and
replaced candidates, malformed intent, actual decoded wire choices and complete
repeat-state publication. The decode witness establishes a triple, emits an
explicit offset equal to an existing repeat, then probes both literal-zero
Repeat1 and Repeat3 so an incorrect third slot changes decoded output. Failure
recovery covers all 28 fresh-context and thirteen committed-context allocation
ordinals with exact owners, including borrowed non-Clone allocators.

The isolated `7c2e928` snapshot plus the owned context overlay passes seventeen
commands: nineteen tests in seven configurations, fourteen original default-frame
executions safe/all release and three exact host/all-feature strict-provenance
Miri checks, totaling 150 passes and five existing oracle ignores. No_std,
strict all-target Clippy safe/all and formatting pass. Every recorded snapshot
input remains unchanged. Three later current-tree integration checks with the
extracted entropy owner also pass: nineteen context tests, seven unchanged default
replays and strict library Clippy/all, with one existing oracle ignore.

[Context evidence](evidence/2026-10-09-fast-sequence-context.json) preserves both
scopes, exact commands/logs, compiler environment, source hashes and the runner.
Miri includes the bounded 41-ordinal failure sweep; its elapsed duration is not a
performance result. No new C execution or numeric compression API is claimed.

## Known-size frame metadata and raw output bound

`FastFrameHeader` is an allocation-free fixed-array value built from an admitted
plan. It emits standard magic, no checksum/dictionary field, known content size
and a single-segment flag only when the entire source fits the selected window.
Otherwise its explicit power-of-two window descriptor is truthful. The content
size switches to eight bytes at 0xffffffff, matching the source rule of the
pinned C writer; the named convenience header retains its original behavior.
The checked complete raw-frame bound includes all actual block headers and one
final empty block for empty input, rejecting overflow without allocating a
fabricated huge source or output.

Five tests cover field boundaries, every admitted window exponent, large scalar
sizes, target-width overflow and complete Rust decoding of real raw-block frames.
Expected header bytes are explicitly source-derived rather than new captured C
observations. Four configurations pass all twenty test executions, safe/all
original default replay passes fourteen, and two exact i686 strict-provenance
Miri tests cover the u32 field transition and complete-size overflow. All thirteen
commands pass, including no_std safe/all, strict all-target Clippy safe/all and
formatting; all snapshot inputs remain unchanged.

[Header evidence](evidence/2026-10-09-fast-frame-header.json) retains the immutable
snapshot manifest, full logs, commands, compiler and runner. The i686 checks are
interpreted scalar tests, not native foreign-platform execution. Public C-path
header observations and actual Fast frame emission remain later integration
checks; no new parser or public numeric API is claimed here.

## Fast literal admission

The literal encoder shares its existing owners and selection flow through a
plain policy value. Positive target length selects raw bytes before uniform
scanning, previous-tree coverage, frequency counting or table construction.
Otherwise each candidate must independently beat the strict Fast payload gain
limit before complete-section comparison. Payload includes tree description,
jump sizes, streams and end markers, but excludes the literal header. Existing
named tie rules, reserve order and default bytes remain unchanged. A Rust
codebook does not represent C's separate HUF-valid state or minimum attempted
input scheduling, and this is not a claim of identical C entropy choices.

Six new tests cover ownership, exact gain equality, header exclusion, each
payload cost, unpublished trees, borrowed non-Clone allocation and complete
fresh/warmed selection failure recovery. The isolated `a5ec297` snapshot with
only the literal overlay passes nineteen commands: fourteen focused tests in
nine configurations, fourteen original default replays in safe/all release, and
three exact host/all-feature strict-provenance Miri tests. The total is 143
passes and no ignored tests. Strict all-target Clippy safe/all, no_std safe/all
and formatting pass; all 495 snapshot inputs and the owned workspace source
remain unchanged.

[Literal evidence](evidence/2026-10-09-fast-literal-selection.json) and its
[archive](evidence/2026-10-09-fast-literal-selection.tar.gz) retain commands,
logs, source, compiler and runner. Initial test-only calls to an unavailable
codebook `Clone` were replaced with immutable codeword snapshots; the original
diagnostic and corrected run are preserved separately from the final campaign.
No new C execution or throughput measurement is attributed to this item.

## Transactional Fast block preparation

`BlockEntropyEncoder::prepare_fast` first discards candidates and clears exposed
payload, then checks the supplied committed-repeat snapshot, complete literal/
match reconstruction and each explicit offset intent before requesting storage.
The parser remains responsible for source and window eligibility. Tiny blocks
bypass entropy work after validation. Other candidates use Fast literal
selection and intent-bearing sequence preparation, then apply strict whole-block
gain including both section headers and excluding the outer block header.
Errors and insufficient gain clear pending publication and payload while
preserving committed trees, tables, repeats and allocator owners. The named
path retains its existing selection/reservation order through a shared join.

Eight new tests cover complete two-block decoding, final literals, stale
snapshot priority, malformed lengths/intent, exact gain boundaries, every
observed fresh allocation failure, borrowed non-Clone recovery and retained
committed state. Two independently added witnesses begin with an emitted tree
and nondefault repeat triple: stale/malformed replacement discards a different
pending tree, and a late warmed allocation failure cannot publish it. Recovery
produces identical bytes and a complete decodable frame. This does not repeat
the separate sequence context's already recorded 41-ordinal campaign.

The immutable `a5ec297` snapshot plus the frozen literal and block overlays
passes nineteen commands: nine owner/preparation tests in nine configurations,
fourteen original default replays safe/all, and three exact host/all-feature
strict-provenance Miri checks, totaling 98 passes and no ignored tests. Strict
all-target Clippy safe/all, no_std safe/all and formatting pass. All 495 snapshot
inputs and both owned workspace sources remain unchanged. Independent review
found no production issue; the additional committed-state tests needed no fix.

[Preparation evidence](evidence/2026-10-09-fast-block-preparation.json) and its
[archive](evidence/2026-10-09-fast-block-preparation.tar.gz) retain both overlays,
manifest, commands, logs, compiler and runner. This item adds no allocation
owner or unsafe operation. Persistent parsing, block choice and public Fast
frame emission remain separate integration work.

## Expanded direct parser families

The separate `fast-parser-families` collection freezes 142 deterministic recipes
and 284 initialized-padding observations. It preserves requested/effective key
widths, explicit wire intent, full literal/match replay, separate parser/wire
repeat triples and complete reconstructible hash-table state. Larger tables use
canonical little-endian full-table fingerprints and sorted deltas without
discarding slots. The original nine-recipe smoke collection is unchanged.

The actual records include collision lookup/store consequences, an isolated
step-four/five next-key difference, several immediate zero-literal repeats,
disabled-offset restoration, distinct hash/repeat lower-bound behavior and both
long-length flags. Twenty-three relational checks per build compare width3/4,
equal-bound window18/19, target0/1 and the conditional-store witness. Recipe
names do not independently prove every intended branch: these are unchanged-C
sequence and final-state observations, not chronological instruction traces.

The original final capture and its independent portable regeneration each pass
568 C case executions: 142 recipes times two paddings times normal/sanitized
builds. Complete observations agree across padding/builds and with the frozen
fixture. The portable runner passes 289 commands, preserves source identities,
and observes host layout separately. ASan/UBSan instrument the helper, unchanged
Fast and inline helpers; other archive support remains uninstrumented and leak
checking is disabled. Native execution is arm64 macOS only.

Original malformed controls pass 114 commands (eight valid controls and 106
expected rejections), including independently byte-replaying but invalid window/
prefix matches. Portable offline validation passes 54 commands under ordinary
and optimized Python with an unavailable executable search path: two positive
controls and 52 expected rejections. Frozen catalog hashes bind even a jointly
modified fixture/manifest; source, host metadata, scope, counts and output-path
isolation are checked. Both routine check scripts run the offline semantic
checker, and the generic audit verifies 468 fixtures in twelve collections.

[Family evidence](evidence/2026-10-09-fast-parser-families.json) and its
[archive](evidence/2026-10-09-fast-parser-families.tar.gz) preserve exact helpers,
recipes, all original/portable logs and controls, earlier qualified captures and
review. Executable/debug bytes are omitted with hashes retained. Repeated runs
add validation executions, not new distinct corpus coverage. Collector and
ordinary-frame state publication are separate prerequisites; Rust Fast and
complete upstream migration are not established by this fixture checkpoint.

## Dependency-free Rust access to direct observations

`tests/fixtures/reference-fast/cases.bin` projects all 142 recipes into a
1,104,309-byte versioned little-endian stream. It preserves source bytes, seeds,
literal/sequence records with explicit offBase, parser/wire repeats, long-length
metadata and full reconstructible table state. One representative of each
previously verified equal padding pair is stored. The converter first runs the
frozen collection's complete offline validation; its `--check` then requires
identical binary bytes and provenance. Both routine check scripts include it.

The test-only safe Rust reader checks structural lengths/counts before slicing
or fallible allocation, validates domains and requires exact record/stream
exhaustion. Separate tests replay all source bytes, repeat transitions and
long-length markers, then reconstruct every seed/table delta and compare bucket
values and canonical hashes using the existing Rust key primitive. No C/Python
process or new Cargo dependency is required for these tests. This validates
fixture access and existing hash semantics; it does not run a Rust Fast parser.

Eleven commands pass on a frozen `f1dc0b1` plus owned-overlay snapshot. Four
tests pass in safe/all/no_std-safe/no_std-all configurations, and one exact i686
strict-provenance Miri check exercises count and record-end guards: seventeen
test executions, no ignores. Strict all-target Clippy safe/all, no_std library
compilation, formatting and ordinary/optimized Python packing checks pass.
All snapshot inputs and workspace overlays remain unchanged. Independent
review found no structural/serialization issue.

[Rust fixture evidence](evidence/2026-10-09-fast-rust-fixtures.json) and its
[archive](evidence/2026-10-09-fast-rust-fixtures.tar.gz) retain overlays, provenance,
commands, compiler, logs and runner. The i686 check is interpreted, not native
foreign execution. No new C observation or production ownership is introduced.
