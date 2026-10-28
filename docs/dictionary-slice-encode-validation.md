# Dictionary-backed caller-owned compression validation

Module 34 is in progress. Only the internal dictionary-view emitter is qualified
and integrated. Public dictionary output APIs, native/corpus/platform validation,
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
