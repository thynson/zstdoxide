# Bounded reference Optimal validation

Strategy 7 remains unsupported. The current work qualifies the optLevel=0
statistics and scalar price primitives before search/path implementation. It is
not an Optimal frame implementation or a complete upstream migration.

## Independent price capture

The 147 source-only requests cover 9,835,338 source bytes and 2,018 transitions.
Normal and ASan/UBSan builds each run every transition and reject all 17 malformed
packets before producing observation records. The instrumented build includes
all 26 used codec source units plus the adapter; assembly and workers are disabled,
leak sanitizer is not claimed, and upstream sanitizer annotations are retained.
All complete 377-entry frequency tables, sums, cached prices and query results
match independent replay. Both C build transcripts are byte-identical.

Normal and optimized offline audits each reject 30 modified envelopes, commands,
flags, environments, artifacts or coherently rehashed state/price records without
child execution. The captured auditor remains immutable; an additive strict v2
wrapper also rejects evidence-schema numeric substitutions, unknown envelope
fields and a saved self-report inconsistent with independently computed results.
These checks do not authenticate arbitrary coherent replacement of an entire
campaign; the immutable archived bytes and recorded execution provenance are the
basis for fixture trust.

`evidence/2026-10-13-optimal-price-capture.json` binds the 390 commands and a
9,484,451-byte lossless archive. All 1,017 retained files / 35,639,560 logical bytes
reconstruct exactly, including raw logs, packets, local source, C objects and both
executables. Upstream reference source itself remains outside the repository.
No production Rust price implementation or public admission is claimed here.
