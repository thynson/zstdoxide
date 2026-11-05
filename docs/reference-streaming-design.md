# Request-backed reference streaming design

Module 40 decision, before implementation; see
[the pinned-source analysis](reference-streaming-analysis.md).

## Public owner and shared engine

`ReferenceStreamEncoder<'a, A = Global>` owns a private
`CompiledStreamEncoder<'a, A, Global>` as the shared frame engine. Its compiled
selection is always absent. Construction accepts a copied
`CompressionParameterRequest` and `CompiledStreamOptions`; the options are
shared frame flags, despite their historical name. `new`, `with_options` and
allocator-taking variants require no dictionary or seeded tables. Prefix-only
construction will use `with_prefix`/`with_prefix_in`. No `Deref` or compiled
selection setter exposes the private engine.

The engine gains an optional copied request. `None` preserves the existing
compiled coordinator's fixed profile contract. `Some(request)` re-resolves
from the exact pledge (or Unknown) and selected prefix's matchable content
length before each header. Before a header byte or workspace allocation, a
scalar admission helper checks the window and constructs a full-block plan;
only the six families with dictionary/history staging are admitted. A reset
may change the next frame's admissibility; process reports that error and
poisons the session before publishing output. No unsupported strategy is
silently substituted. Construction also validates the initial request/options.

`parameters()` on the public request-backed wrapper returns `None` before
initialization and `Some(resolved)` after it. This avoids presenting a stale
previous frame profile after a reset or prefix change. `request()` returns
the immutable construction request; parameter replacement/default resets are
not introduced. Session reset, reset-with-options, prefix replacement,
selection introspection, counters, capacity and `process` forward the existing
contracts. Reset-with-options is infallible in practice but retains the shared
engine's Result signature; deferred per-frame admission is documented.

## Ownership and invariants

The wrapper owns one engine; the request is a plain value with no allocation
or unsafe alternate representation. All workspace vectors remain owned by the
caller-selected `A`, are fallibly grown, and retain capacity across frames.
Per-block parser owners continue to use their existing separately selectable
safe/unsafe backends. A prefix borrows caller storage for `'a`; the pending
view is claimed exactly once and released on completion/reset. Selection is
always prefix-or-none, never a compiled table in the request-backed wrapper.

At header initialization, resolution and plan admission precede publishing the
new profile, window and block limit. The frame is empty at this point, so no
buffer position or repeat offset survives a geometry change. Throughout a
frame, geometry is fixed. History length equals min(frame position, window),
block length is at most min(window, 128 KiB), and every sealed block rebuilds
a plan from that same resolved profile. Prefix suffixes are capped to the
resolved window, retaining the existing conservative distance contract.

## Header and failures

The shared non-single-segment writer emits the per-frame resolved window log,
optional exact pledge, dictionary ID and checksum. Known zero remains known;
source hints affect geometry only. Initial Finish does not infer or override
a pledge. Compiled construction's window behavior remains unchanged.
Allocation, pledge, admission and directive failures poison the session and
require reset; configuration boundary errors do not mutate live frame state.
Output touched by a failed process call must be discarded.

## Qualification

Separate items implement dictionary-free construction and then prefix-only
construction. Tests cover six explicit families, known/unknown and hidden
pledges, empty input, window shrink/growth across reset, source hints, flush,
zero/tiny output, invalid directives, unsupported plans before allocation,
allocation failure/recovery and prefix consumption. Guarded native schedules
cross-decode frames and inspect window/content-size fields rather than demand
byte identity. The isolated-feature/all-feature, release, no_std, Clippy and
atomic-free ARM matrix and independent safe/all timing repeats remain separate
gates. No new unsafe code is planned, so the standing directive skips Miri.
