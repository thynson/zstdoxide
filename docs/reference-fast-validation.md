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
