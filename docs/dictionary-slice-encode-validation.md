# Dictionary-backed caller-owned compression validation

Module 34 is in progress. The internal dictionary-view emitter and public caller-
output wrappers have focused qualification. Complete feature/release regression, measurements and reconciliation remain open.

## Internal immutable selection

The ordinary emitter delegates through an optional copied DictionaryView. Header
ID selection and BlockEncoder setup use the view without copying dictionary
content or cloning a shared owner. One-block staging and FrameSink stay unchanged;
all existing search/entropy/history/copy backend features retain their contracts.
The view is plain borrowed metadata with no artificial unsafe representation.

Seventeen focused commands pass: four dictionary-engine units and six ordinary
predecessor units under safe, isolated sink, isolated dictionary and all 22 features,
plus true no_std, strict Clippy and formatting. Checks compare exact existing
allocating dictionary output for empty/raw/prepared selection, all identifier
widths and multiblock input, verify sufficient bounds, short-output guards and
fresh-call recovery, retain dictionary bytes/shared-owner counts, use independent
borrowed non-Clone dictionary/scratch allocators, and inject every observed
scratch failure ordinal in the selected raw/prepared empty/nonempty workloads.
Every scratch owner is released; dictionary allocation accounting stays unchanged.
These focused checks do not imply full upstream/native or major-module parity.
Evidence: `evidence/2026-10-27-dictionary-slice-encode-engine.json`.

## Public caller-owned dictionary output

compress_into_with_dictionary and its allocator-aware variant borrow immutable
dictionary views and return the exact existing dictionary encoder bytes. The
caller owns output; sufficient compression_bound storage and smaller exact-fit
capacity work. Successful unused suffixes and out-of-range guards are preserved.
Short output can retain earlier complete appends; whole-call atomicity is not
promised. No shared handle clone or dictionary content copy is introduced.

Twenty-five focused commands pass across safe, isolated frame-sink, isolated
dictionary and all22 feature selections: four dictionary engine units, six ordinary
predecessor units, five public tests and two documentation examples (including
borrow rejection), plus true no_std, strict Clippy and format. Public cases cover
raw/prepared/empty dictionaries, identifier widths, empty and multiblock input,
all short capacities, fresh-call recovery, header rejection before scratch,
borrowed non-Clone dictionary/scratch owners and selected allocation failure.
The engine separately retains every-observed-failure qualification. The prepared
fixture and exact source/evidence are retained with upstream attribution.
No broader native, platform, full-feature or performance milestone is implied.
Evidence: `evidence/2026-10-27-dictionary-slice-encode-public-api.json`.

## Guarded native dictionaries and deterministic trained selections

Ninety deterministic raw/empty/prepared dictionary cases include all identifier
widths and one/multiple-block input. Pinned v1.5.7 normal and ASan/UBSan helpers
decode every Rust frame using DDict, then use CDict for nine capacities per case.
All810 attempts check real outer guards and immutable input/dictionary bytes.
Every successful capacity output is retained and independently decoded by four
Rust backends, which also sweep Rust actual-size capacities with exact bytes and
guards. Native compressed-byte identity and capacity/error precedence are not
Rust contracts. macOS leak detection is unsupported and disabled.

Native successful attempts: 164; successes with writes in the unused destination suffix: 72. These observations are retained, not hidden by Rust's stricter suffix contract.

Two existing deterministic trained-dictionary tests additionally qualify persistent
caller-owned compression under safe, isolated dictionary and all22 features.
Original sample/mode/prefix/training-fallback and reduced streaming-bound checks
remain. The48 controlled persistent selections and recorded deterministic-noise
selections compare exact existing bytes, full decode, actual-size-plus-two and
minus-one capacity, guards and immutable dictionary/handle counts. Prefix cases
retain their prior assertions without claiming a new compression-prefix API.
No external rolling corpus rerun or native fastCover equivalence is claimed.
Evidence: `evidence/2026-10-27-dictionary-slice-encode-native.json` and
`evidence/2026-10-27-dictionary-slice-encode-corpus.json`.

## Foreign interpretation and actual no-atomics code generation

Four strict-provenance i686/s390x Miri runs execute raw/prepared dictionary
compression in safe/all builds, checking exact bytes, a four-byte dictionary ID,
known content size, independent decode and short-output guards. Four thumbv6m
core-plus-alloc check/object commands instantiate the borrowed dictionary view
entrypoint and encoded-output copying, with retained LLVM IR and ARM ELF objects.
These are interpreter and code-generation results, not foreign hardware runs.
Evidence: `evidence/2026-10-27-dictionary-slice-encode-platform.json`.

## Complete feature and release qualification

All 75 commands pass on the final frozen candidate: safe/all and every 22 isolated
unsafe feature each execute complete tests, true no_std compilation and strict
Clippy; safe/all release and formatting also pass. Each complete/release suite
reports 1752 passed and 75 ignored across 114 result groups. Native adapters are
explicitly executed in their separate focused capture/replay receipts.

The final candidate deliberately combines qualified public/native/platform/corpus
files and the independently audited post-33 metadata reconciliation. Earlier
focused snapshots retain their original metadata; none was silently rewritten.
Integrated production and all matrix source identities are rechecked before this
item is committed. Whole compression measurements and closure remain open.
Evidence: `evidence/2026-10-27-dictionary-slice-encode-feature-matrix.json`.
