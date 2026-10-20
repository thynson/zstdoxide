# Independent Ultra2 fixture and Rust reader qualification

This private fixture represents the complete unchanged native Ultra2 parser
lifecycle, without entropy/frame output or public strategy-9 admission. Its binary
layout was specified before projection in `reference-ultra2-fixture.md`; ownership
and checked coordinate rules are recorded in `data-structures.md`.

## Native projection and source reconstruction

The immutable 53-case fixture contains 394656 events and is 49004443 bytes long,
SHA-256 `1583da05247179243a25a55cfce6869ac6a13f874bfcf8cc493311f636f61b1a`.
Its source envelope is `tests/fixtures/reference-ultra2/provenance.json`; the
native capture envelope SHA-256 is
`dfb59ef260a0123c76db4c64bebb2ae5d551478b6c0c4f3a72b0bbe4573fdcc7`.
Projection first audits the complete capture, verifies both builds and padding
runs, and serializes all first-padding eligibility/pass/event/reset records.
Search/path deltas and frequencies retain their physical baselines across reset.
Native allocation sizes remain opaque qualified observations.

Normal and optimized independent source reconstruction each reproduce every byte
of the native projection, with exact per-case hashes across all 53 requests.
Every worker derives eligibility, pass boundaries, all parser/search/frequency
states and reset records from the source model; it does not consume native state
rows as expectations. Six Python workers run per mode, with child execution
prohibited inside each worker. All source snapshots match before/after hashes.
Both modes reject 57 malformed or coherently changed fixture records while
bypassing the outer SHA-256 gate. Every forgery is retained as an exact replayable
byte delta with its diagnostic. Full worker bodies/layouts and orchestration logs
remain available in the archive.

The projection/reconstruction archive is
`evidence/2026-10-20-ultra2-fixture.tar.gz`, SHA-256
`ee21a88a686dd4cbab126439d8714b1cf97aa9926dc5b91c8434fd752cc173b9`.
It retains 577 regular files (332074117 expanded bytes), including every source
snapshot, reconstructed case body/layout, forged byte delta and driver/log.
Independent archive replay reconstructs and hashes every retained file, followed
by a second unchanged-original read. No model or observer was rewritten to fit
native output.
