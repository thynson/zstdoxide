# One-shot encode selection validation

Module 35 is in progress. Only the internal call-scoped selection coordinator
is qualified and integrated. Public coordinator methods, native/corpus/platform
validation, complete feature/release regression, measurements and reconciliation
remain open.

## Internal clone-free coordinator

OneShotEncoder wraps the existing plain Selection/Claimed ownership records over
any dictionary handle type. A persistent selection is borrowed for every call; a
pending prefix is claimed by an exclusive replacement before any codec or
allocator operation, and the claimed owner lives until the attempt returns or
unwinds. No handle is cloned, no dictionary content is copied, and no encoder
workspace is retained between calls. The private module is exposed only by the
subsequent qualified API item. Selection and Claimed remain plain ownership
records with one safe representation.

Seventeen focused commands pass: the four coordinator units and the four
selection-record units under safe, isolated frame-sink, isolated dictionary and
all 22 feature selections, each with true no_std compilation and strict Clippy,
plus formatting. The claim consumes prefixes before output admission, empty
input, short output and every observed allocation failure, while the
independent dictionary owner's snapshot stays unchanged.
Evidence: `evidence/2026-10-28-one-shot-encode-engine.json`.
