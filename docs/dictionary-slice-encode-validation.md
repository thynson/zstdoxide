# Dictionary-backed caller-owned compression validation

Module 34 is in progress. The internal dictionary-view emitter and public caller-
output wrappers have focused qualification. Native/corpus/platform validation,
complete feature/release regression, measurements and reconciliation remain open.

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
