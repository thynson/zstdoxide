# One-shot encode selection validation

Module 35 is in progress. The internal coordinator and public caller-owned
methods have focused qualification. Native/corpus/platform validation,
complete feature/release regression, measurements and reconciliation remain
open.

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

## Public caller-owned selection methods

OneShotEncoder::new/without_dictionary/with_dictionary/with_prefix construct and
replace call-scoped selections; dictionary()/prefix_pending() borrow immutable
selection metadata. compress_into/compress_into_in compress one frame into
independent caller storage with Global or supplied scratch, consume a pending
prefix before output admission or any allocator operation, and retain a
persistent selection across calls and errors. Documented lifetime compile-fail
examples establish owner and borrow non-overlap. No handle clone, dictionary
copy, retained workspace or whole-call output atomicity is introduced.

Twenty-one focused commands pass across safe, isolated frame-sink, isolated
dictionary and all 22 feature selections: four coordinator units, six public
integration tests and three doctest groups each, with true no_std compilation
and strict Clippy, plus formatting. Attempt-exact consumption, short-output
recovery, replacement/clearing drops and every observed scratch failure keep
independent owner domains.
Evidence: `evidence/2026-10-28-one-shot-encode-public-api.json`.
