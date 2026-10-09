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
