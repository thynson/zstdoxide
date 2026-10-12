# Bounded reference Greedy validation in progress

Module 14 is in progress. Source design, pre-Greedy regression baselines and the
first direct C oracle checkpoint are complete. Rust reference chain/row storage,
Greedy parsing, ordinary-frame state, public Greedy admission and broader test
migration remain pending. The existing independent named Greedy policy is not
an implementation of this reference strategy.

## Preserved behavior before implementation

Sixteen clock-free DoubleFast/Fast cases preserve twelve distinct complete
frames (5,680,125 bytes) and twelve allocator survey fields per case. Five new
regression tests pass on the safe default, each of twelve unsafe features alone
and all features together. The four original Fast baseline tests also pass on
safe/all. Strict Clippy for the new target, safe/all no_std checks, formatting
and unchanged Fast/named offline baselines pass. Exact allocator bytes are
qualified to the pinned aarch64-apple-darwin compiler/target; portable checks
retain complete frames, owner lifetimes and release invariants.

The [baseline evidence](evidence/2026-10-12-reference-double-fast-baseline.json)
and its source fixture retain the original clock-free capture identity. Both
normal Python and `-O` rejected fifteen baseline mutations with process creation
disabled. These tests establish unchanged prior behavior, not new Greedy support.

## First direct C checkpoint, 2026-10-12

The [reviewed adapter](reference-greedy-adapter-review.md) implements the
[bounded packet contract](reference-greedy-oracle.md). Thirty-six recipes were
frozen before execution: seventeen chain cases, seventeen corresponding row
cases and two additional salted row cases. Fresh and prefixed tail boundaries,
periodic input, minMatch dispatch and unavailable repeats are covered. Each
recipe runs with two source-padding patterns in both C builds, yielding 144
complete sequence/table observations. All direct outputs are byte-identical
between normal and ASan/UBSan builds.

Two separately allocated fresh contexts per build use distinct nonzero payload
fills. All four contexts observe initial salt/entropy zero, then fresh row salt
`0x835892b4bb2cae74`, entropy zero and zeroed tables after initialization. The
applied tuple is W15/C6/H6/S1/M4/T0/Greedy with row mode explicitly enabled.
The independent bitmix calculation agrees. Context reuse and dictionary salt
contracts are not established by these observations.

All 26 linked C translation units plus the helper are built from the pinned
v1.5.7 sources in each mode. No support archive is linked. ASan/UBSan instruments
the helper and every linked unit, with LeakSanitizer disabled and upstream
per-function pointer-overflow exclusions retained. This is not a claim that
sanitizers check every upstream expression. Compiler is Apple clang 17.0.0,
target arm64-apple-darwin24.6.0; full arguments, dependency closure, executable
and object identities are preserved. Thirty-seven malformed packets reject
before parser execution per build, for 74 rejections. The qualified campaign
contains 210 recorded commands.

The first full campaign passed all per-case checks but failed its final generic
cross-build equality. Only the opaque context's guarded allocation peak differed:
149,198 bytes normally and 151,502 under ASan, with two allocations and complete
release in both modes. Pinned `zstd_cwksp.h` adds internal ASan redzones; direct
arena allocations and results were unchanged. The successor controller excludes
only this mode-dependent memory field from cross-build fresh-salt equality,
retains both exact values, and still compares every other salt/state/owner field.
The helper, input recipes and per-record validator did not change. The original
failed report, raw logs, input packets and exact first controller source remain
archived; it has not been relabeled as a successful campaign.

The independent offline verifier replays all 210 command records, every compile
and link argument, source/dependency identities, raw input packets, completion
markers, full sequence replay and complete before/after maps. It compares live
observations against a separately pinned immutable fixture, so coherently
rewriting a log and its local hash cannot refresh expectations. Normal Python
and `-O` each pass two positive controls and reject thirty forged captures with
child-process creation disabled. The negative controls include missing compiled
units, altered sanitizer flags, archive linkage, changed fields/maps/salt,
changed packets and the qualified context memory values.

The permanent fixture contains 147,120 NDJSON bytes in 38 complete groups:
36 direct cases plus normal and instrumented fresh-context observations. Its
capture manifest SHA256 is
`18e0ff3d695627c20ef159ecda1ee027ce535d98a55e0b84c261a0ef34c0ffa4`;
the observation pack is
`a2bac02eeeb65292b92892605d15b043097ed94651b4be494fec0427b73b6fff`.
Normal checks consume it without invoking C or importing a codec.

The [capture report](evidence/2026-10-12-greedy-tiny.json) records the evidence
archive: 1,079 regular members, 7,648,534 uncompressed bytes, 1,140,444 archive
bytes, SHA256
`26b361a9b10bec58fd1f74d58eaa1146359a63c8b01683de07393e01fd918ccc`.
It retains both campaigns, all logs/packets, 63 used C source/header files,
local source closures, controls and complete observations. Compiled objects and
executables are omitted with their identities retained. Every archived member
was reread and compared to its original bytes.

## Remaining scope

These are constructed direct-state observations, not ordinary context/frame
trajectories or a trace of every internal probe. Row widths 32/64, larger
hash/chain domains, cyclic wrap, tie/collision witnesses, gaps, skipping,
window floors, entropy wrap and broader repeats still need separately frozen
recipes. No new official source-unit obligation is marked complete. The next
items are broader direct observations, Rust safe/unsafe storage and parsing,
then ordinary-frame/public admission and their related official assertions.
Final every-feature/platform/interoperability matrices and Greedy timing remain
open; the complete roadmap and all upstream migration obligations remain open.
