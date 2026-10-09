# Parameter and encoder lifecycle validation

Module 10 is in progress. Each checkpoint below identifies its implementation
and evidence boundaries; the complete feature/width matrix and official test
migration update remain pending.

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
