# Parameter and encoder lifecycle validation

Module 10 is complete within its documented scalar-inspection and supported
encoder-lifecycle scope. Each historical checkpoint below identifies its own
implementation and evidence boundaries. The final immutable matrix and precise
remaining upstream migration gaps are recorded at the end of this report.

## Frozen reference observations

Commit `c30ede1` imports 934 observations from pinned facebook/zstd v1.5.7.
The [fixture README](../tests/upstream/parameters/README.md) distinguishes
public and private adapters, signed clamping, modular-size domains, 64-bit
execution, macro-derived 32-bit bounds, and different context protocols.
[Oracle evidence](evidence/2026-10-09-parameter-oracle.json) records exact
regeneration and provenance checks. This imported data is not itself a Rust
behavioral test or a completed upstream migration.

## Value records and scalar validation

The first Rust checkpoint implements six plain safe value records, concrete
validation, reference strategy conversion, request getters and checked setters,
and reference frame defaults. It introduces no allocations or unsafe operations.
It does not yet implement preset selection, adjustment, or query resolution.

Thirteen focused tests pass under safe, all-feature, no_std safe and no_std with
all twelve unsafe flags: 52 passing executions with no ignores. Both no_std
library checks, strict safe/all-target and all-feature Clippy, owned-file
formatting and diff checks pass. Two standalone usage examples pass under safe
and all-feature builds. The frozen owned source and all ten focused command
logs are preserved in [record evidence](evidence/2026-10-09-parameter-records.json).

The replay checks all 225 recorded raw inputs against C validation results,
without claiming their adjustment outputs. The request trace covers 119 scalar
state rows: 72 actual typed setter calls, 25 unrepresentable-input rejections,
and 22 initialization/reset rows. Thirty C frame-flag rows and one unsupported
parameter ID remain outside this request surface and are explicitly counted.
All 266 query rows supply reference frame-default assertions only at this stage.
Target-width bounds are checked against the labeled fixture records; no foreign
target execution is claimed by this checkpoint.

## Supported live configuration replacement

`Encoder::set_options` installs the supported options only before a header has
started. It checks the phase first, validates before assignment, and retains
dictionary/prefix selection, buffers, counters and allocator owners. Either
error leaves the entire session unchanged. The operation does not allocate,
clone owners or reset an active frame; it accepts no numeric reference profile.

Four new tests cover all supported fields, constructor equivalence, shared and
borrowed dictionaries, unclaimed prefixes, nine started/finished/failed phase
witnesses, complete state snapshots and usable continuation after rejection.
They also check invalid options, the unchanged live pledge domain, independent
custom allocator owners and a still-armed allocation failure after configuration.

Safe/all-feature complete encoder unit suites each pass 22 tests with one
explicit C oracle ignored. The four new tests also pass with no default
features, all twelve unsafe flags without std, and the individual chain and
dictionary features. Both no_std library checks, owned-file formatting and the
final strict all-target/all-feature Clippy run pass. The initial shared lint
failure concerned the concurrently developed scalar test tuple; its local type
alias fix and the successful final lint are both recorded.

The unchanged default-frame fingerprint passes. The existing allocator oracle
now installs options through this new method, and explicit safe/all-feature
release runs each produce 18 frames independently decoded byte-for-byte by
pinned C. All three streaming documentation examples pass under both builds.
[Configuration evidence](evidence/2026-10-09-encoder-configuration.json) retains
commands, logs, the frozen owned source hash and precise counts. Parameter-only
and full reset are not part of this checkpoint.

## Unadjusted preset foundation

All 92 table entries retain the pinned source attribution and match the
independently captured C rows. The internal selector checks known source plus
dictionary and unknown-source dictionary plus 499 arithmetic, rejects the
reserved known-size sentinel, and normalizes negative levels before negation.
It returns an unadjusted record; request resolution must still perform its two
adjustments in the correct order.

Nine new tests verify both stored and selected forms of every preset, inclusive
tier boundaries, source-size knowledge, negative acceleration and exact-limit
arithmetic. Together with the thirteen earlier tests, all 22 pass under safe,
all-feature, no_std safe and no_std with all unsafe flags: 88 executions. Both
no_std library checks, strict safe/all-feature Clippy, formatting and diff checks
pass. The selector is compiled under `cfg(test)` until the complete public query
uses it, so this checkpoint introduces neither an incomplete query nor unused
production lookup code. No new C execution, Miri or benchmark is claimed.
[Preset evidence](evidence/2026-10-09-parameter-presets.json) records the frozen
sources and all ten focused commands and logs.

## Complete scalar inspection

`inspect_compression_level`, raw adjustment and request resolution now use the
production preset selector. Resolution preserves both reference adjustment
passes around explicit overrides; source hints affect unknown sizes only.
Signed 32-bit raw-field clamping and temporary window logs below ten follow the
pinned algorithm. Reserved known size and arithmetic overflow return explicit
Rust errors. Each operation checks only expressions it actually uses, so raw
unknown-size adjustment does not evaluate the preset selector's extra offset.

Ten new tests bring the parameter suite to 32 passing tests in each of four
safe/feature/std configurations, or 128 executions. The normal std suites leave
one external-probe test explicitly ignored. Frozen replay checks 262 exact
query results and four deliberate rejections; 223 exact raw adjustments and two
overflow rejections; and 450 resolved profiles from 150 recorded scalar states.
The last group includes 30 frame-only and one unsupported-operation projection,
which do not claim those setters. Both no_std checks, strict safe/all Clippy,
owned formatting and diff checks pass. All four parameter usage examples pass
under safe and all-feature builds.

A separately prepared C probe supplies 46 additional rows. Explicit safe/all
replays each match 41 ordinary profiles and reject five labeled Rust-domain
cases. It covers neighboring sizes, dictionary contributions, source hints and
override ordering. This checkpoint replays previously generated observations;
the probe metadata separately records its successful C generation. All executed
C observations are 64-bit; no native 32-bit execution, Miri or performance claim
is made for these allocation-free scalar records.

[Query evidence](evidence/2026-10-09-parameter-queries.json) embeds all twelve
focused command logs and identifies the frozen sources. The separate
[probe archive](evidence/2026-10-09-parameter-query-probe.tar.gz) contains only
project-owned probe/reproduction files and captured observations. To reproduce
its original setup, extract those files into `/tmp`, provide the pinned external
checkout and built archive at `/tmp/zstdoxize-upstream-1.5.7`, and run the archived
Python script. Set `ZSTDOXIDE_PARAMETER_REVIEW_PROBE` to the resulting TSV when
running the ignored test named in the evidence. The Rust implementation neither
links C nor enables numeric encoder execution.

## Official scalar assertion adapters

Eleven public integration tests preserve pinned source anchors and the original
adjustment and dictionary-profile assertions. They cover reference level
constants, validation, requested-field independence, automatic-field clearing,
source-hint bounds and precedence, the two-pass empty-source witness and frame
defaults. Public C size zero maps explicitly to `SourceSize::Unknown`.

All eleven pass in safe, all-feature, no_std safe and no_std with every unsafe
backend: 44 passing executions, no ignores and no filtered tests. Focused
safe/all strict Clippy and formatting pass.
[Scalar integration evidence](evidence/2026-10-09-upstream-scalar-parameters.json)
embeds exact commands and logs. The adapters distinguish detached request values
from live context reset, and do not claim a seven-field bulk setter, LDM-enabled
compressed equality or numeric-level execution. Lifecycle integration and
case-inventory promotion remain separate work items.

## Parameter-only and full encoder reset

`reset_parameters` restores Rust defaults and clears dictionary/prefix state at
a fresh boundary or after frame completion. A finished session keeps its phase,
counters and checksum bookkeeping until session reset. Other phases reject
without mutation. `reset_all` accepts every phase, restores defaults and returns
a fresh session. Both clear workspace handles and both dictionary seed families
in place, retaining every codec allocation and its exact allocator owner. They
perform no allocation or allocator clone. C's fresh parameter reset retains its
pledge and differs in checksum defaults; the Rust contract states this explicitly.

Five new tests cover ten concrete phases, sixteen shared dictionary/prefix
combinations, distinct allocator domains, borrowed non-Clone owners, final-handle
release, armed allocation failures, failure recovery before/after prefix claim,
seed invalidation and unchanged next-frame default bytes. The final seed test
probes a strictly later coordinate so equal-position filtering cannot mask stale
state. Its initial coordinate correction and final strengthening remain in
[reset evidence](evidence/2026-10-09-encoder-resets.json).

Complete safe/all streaming suites each pass 26 tests with one explicit oracle
ignored. The focused reset tests also pass under no_std, no_std with all unsafe
backends, and five relevant individual unsafe features. The final seed assertion
passes all nine focused configurations. Both no_std library checks, strict
all-target/all-feature Clippy, formatting and diff checks pass. All four streaming
usage examples pass safe/all. Independent read-only review found no issue in
phase legality, handle release, seed invalidation or allocator preservation.

Two narrow all-feature strict-provenance Miri executions cover allocation failure
around prefix claim and borrowed-owner prefix clearing. They do not cover the
large shared/formatted dictionary or chain phase matrix under Miri. Existing
pinned-C regression runs decode 18 generated frames exactly per safe/all release
build; these are interoperability checks, not identical C/Rust reset protocols.
Source-anchored public lifecycle integration is the next separate checkpoint.

## Scalar width and endian validation

An isolated archive of `6ca8617`, plus the frozen scalar integration file later
committed as `d190d56`, retained all 669 file hashes throughout eight commands.
The three relevant source hashes still match the committed implementation.
The pinned compiler and cached sysroots were reused offline without installation.

Strict-provenance Miri interprets 26 applicable parameter unit tests and eleven
public integration tests on i686 in each safe/all-feature configuration: 74
passing executions. Six host-64-only unit tests contain the captured 64-bit C
values or usize-overflow domains and are explicitly excluded, as is the optional
64-bit external probe reader. The evidence lists every exclusion; they are not
reported as passing 32-bit cases.

Six selected signed-clamp, adjustment-order, row-cap, hint and overflow tests
also pass safe/all on big-endian s390x: twelve further executions. Total selected
Miri execution is 86 passed, zero failed and zero ignored. The thumbv6m library
compiles without std in safe and all-twelve-feature builds using cached build-std.
These are interpreted Rust checks and compile-only ARM checks, not native Linux,
QEMU, firmware or cross-target C execution. No codec benchmark is claimed.

[Platform evidence](evidence/2026-10-09-parameter-platform.json) preserves commands,
compiler/target configuration, selected test names, full logs and hashes. Its
[archive](evidence/2026-10-09-parameter-platform.tar.gz) retains the complete
snapshot manifest and reproduction runner.

## Official lifecycle assertion adapters

Six new public tests preserve dictionary/options ordering, settings across
fragmented processing and rejected mutation, session-reset dictionary retention,
parameter/full-reset clearing, prefix lifetime, frame flags and invalid-window
atomicity. The original 10 KiB dictionary / 100 KiB source dimensions and
dictionary-versus-cleared-frame size inequality are retained. The corpus is an
explicit deterministic RDG adaptation, not the inherited C `CNBuffer` identity.
The prepared-dictionary flag cases separately use the pinned formatted fixture.

Each of four safe/feature/std configurations passes eight tests: six lifecycle
tests plus two shared generator tests. One C oracle is ignored in each normal
run and then executed explicitly in safe/all release builds. Focused strict
Clippy, formatting and diff checks pass. Existing 108-frame streaming and
36-frame one-shot fingerprints pass in both configurations, totaling 288
comparisons without replacing the established baselines.

The C oracle independently decodes thirty complete frames per build, or sixty
exact comparisons and 6,144,000 decoded bytes. Across both builds, 36 frames use
their selected named policy; the other 24 follow parameter/full reset and use
restored Greedy defaults. This is interoperable output from the supported Rust
lifecycle, not C numeric-level or reset-protocol equivalence. All production Rust
sources and the owned test file remained unchanged during the fourteen focused
commands. [Lifecycle integration evidence](evidence/2026-10-09-upstream-parameter-lifecycle.json)
retains their exact commands, full logs, counts and reproduction runner.

## Updated migration inventory

Twenty-five source anchors now reflect the verified scalar and lifecycle tests.
Six complete scalar groups are ported; qualified live/request subsets stay
partial. Four numeric/LDM/decoder-reset groups remain blocked with corrected
descriptions of the available inspection API. Both whole fuzzer and streaming
sources remain partial. Existing unsupported behavior is retained explicitly.

Offline and pinned-source inventory audits pass. The separate complete-migration
gate still exits one, as expected: 188 source units and the external corpus
coverage remain unfinished. Named groups now total 53 ported, 117 partial,
430 blocked and 15 non-applicable; 547 groups remain unfinished. Source counts
remain 14 ported, 34 partial, 149 blocked, five planned and 216 non-applicable.
Both external corpus records remain partial. The
[inventory evidence](evidence/2026-10-09-parameter-migration-inventory.json)
preserves all changed mappings, commands and full audit logs. Passing catalog
integrity is not a claim that the complete-migration gate passes.

## Integrated formatting repair

The first isolated matrix attempt at `e8c2015` stopped at formatting before
running tests. It found macro-layout differences in `benches/allocators.rs`,
whose benchmark checkpoint followed the earlier Module 9 codec snapshot.
Applying the pinned rustfmt to that file resolves the check; complete crate
formatting and strict all-feature Clippy for the benchmark target pass. Every
string literal remains byte-identical and in the same order. No benchmark was
rerun and historical measurements retain their original source hashes.
[Repair evidence](evidence/2026-10-09-benchmark-format-repair.json) preserves the
initial failed matrix record, full formatting diagnostic and focused repair
checks. The complete matrix restarts from the subsequent committed snapshot.

## Final immutable Module 10 matrix

The completed matrix executes committed `0111aef` from an isolated Git archive,
with all 677 archived files unchanged before, during and after its 45 commands.
Concurrent Module 11 work is outside this snapshot. The pinned compiler is
Rust 1.101.0-nightly (`1d81eb4ad`, toolchain `nightly-2026-10-08`) on Apple
AArch64. This is not a stable-toolchain or native foreign-platform claim.

| Validation | Result |
| --- | --- |
| Safe, twelve individual unsafe flags, all-feature debug and all-feature release | 759 passed, zero failed and 52 explicitly ignored in each of fifteen full suites |
| Full-suite aggregate | 11,385 passing executions; 780 ignored executions |
| no_std library compilation | Fourteen configurations passed |
| Strict all-target Clippy and runnable usage examples | Safe/all passed; sixteen examples per build |
| Supported lifecycle C oracle | Thirty complete frames per safe/all build; sixty total and 6,144,000 decoded bytes |
| Independent parameter review probe | Forty-six rows per safe/all build: 41 exact profiles and five deliberate reserved/overflow errors |
| Pinned parameter fixture reproduction | All 934 recorded rows reproduced |
| Snapshot fixture and pinned inventory audits | 319 files in nine collections; all source hashes and anchors valid |
| Complete upstream migration gate | Expected failure: 188 unfinished source units and partial external corpora |

Every focused oracle selector executed exactly one test, with no zero-test
success. An independent post-run check validates all log hashes, positive test
counts, consistent full-suite totals and both thirty-frame lifecycle messages.
The CLI and independent review TSV hashes remain unchanged. Full commands,
logs, source manifests, execution plan and both verification programs are kept
in [final matrix evidence](evidence/2026-10-09-parameter-final-matrix.json).

The module completes reference profile inspection, checked supported live option
replacement, parameter-only/full resets, source-anchored scalar/lifecycle
adaptations and platform qualifications. It does not implement numerical
reference compression strategies or complete the full upstream migration.
Case counts remain 53 ported, 117 partial, 430 blocked and 15 non-applicable.
The 52 ignored tests are not silently treated as successes. Later Module 11
fixtures and code have separate evidence and are not covered by this snapshot.
