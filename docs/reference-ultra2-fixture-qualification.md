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

## Borrowed Rust reader

The reader borrows names, source bytes and case extents from its input. It checks
all sizes before bounded fallible allocation, canonical sparse indices, complete
source-relative match/repeat reconstruction, exact phase order, original repeats,
discarded token counts/tail, and current versus stale search coordinates. A reset
retains physical cells and frequencies. Independent histogram/distribution and
integer rescaling calculations check the complete following begin state, including
7/8/9 predefined-price thresholds. No production codec state supplies expectations.

Four integration tests pass in both safe and all-feature builds: complete native
inventory and borrowed extents, every state of all 44 predecessor first passes,
malformed extent/type boundaries, and discarded/reset/rebased publication
forgeries. Targeted strict Clippy, repository formatting, and true no_std safe/all
compilation also pass. These reader tests use safe test-owned vectors; they do not
claim qualification of a future production two-pass coordinator or its backends.

The reader archive is `evidence/2026-10-20-ultra2-reader.tar.gz`, SHA-256
`98bb749142ecd638c02f6309427f13cc0abf17d3b07ef3fed9913e77bb905746`.
It retains 229 files, including exact compiled/reader inputs, raw logs and earlier
attempts. The first exploratory borrowing assertion compared distinct Rust const
include-byte promotions; a shared static input fixes the test's identity premise.
All semantic reader checks had passed. The first formal attempt then passed both
test modes but Clippy rejected a redundant constant policy comparison. The
corrected equivalent expression passes all nine final qualification commands.
These failures are preserved, not counted as successful checks. The snapshot
precedes registration of the separate private Ultra2 plan; it qualifies this
reader against its captured production baseline.
