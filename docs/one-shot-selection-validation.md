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
