# Complete DoubleFast regression baseline

Sixteen logical cases preserve twelve complete zstdoxide frames (5,680,125
distinct bytes), resolved parameters and all twelve allocation survey fields
from the reviewed 2026-10-12 clock-free DoubleFast benchmark. Twelve cases use
DoubleFast; four use Fast as controls. Safe and all twelve existing unsafe
backends produced identical complete frames and allocation records. The
capture uses codec revision `b505237` plus the benchmark overlays; provenance
retains the complete source manifest and pinned compiler identity.

The four input recipes cover skewed literals, three-block history, mixed
raw/RLE/compressed blocks and a seventeen-block source with a repeated block
at distance 1.5 MiB. Four profiles cover levels 1 and 3 and independently large
long/short tables. Expectations are extracted only from the immutable original
archive, before Greedy implementation. No codec is used to generate or refresh
these expectations. These exact frames have Rust decoding evidence; independent
C interoperability elsewhere uses separately identified cases.

Run `cargo test --test reference_double_fast_defaults` and
`python3 scripts/check_reference_double_fast_baseline.py`. The five Cargo tests
regenerate the pinned sources, compare all sixteen complete frames, resolved
parameters, headers, exact decoded bytes, allocator categories, owner lifetimes
and final release. On aarch64-apple-darwin/64-bit with the pinned
nightly-2026-10-08 toolchain they also compare every captured allocation field.
Other targets retain full frame and portable ownership checks; their absolute
layout sizes are not assumed to equal the capture target's sizes.

Allocator sizes are requested/returned payload sizes, including the returned
frame's residency. They do not measure process RSS, allocator metadata or
temporary old/new backing overlap inside a resize. A compiler change may
require explicit review of these exact observations.

The stdlib-only offline auditor checks the fixed archive hash and bounded
member catalog, verifies content-addressed chunks, reconstructs the selected
clock-free files in memory and derives the exact TSV, deduplicated frame pack
and provenance. It never executes archived code, imports an encoder, starts a
process, extracts to disk or rewrites an expectation. Current codec source may
change; the historical source manifest records the capture's provenance.

Keep existing Fast and named-policy baselines intact. Deliberate changes to
complete outputs or qualified allocation behavior require explicit review and
a new capture, never an automatic refresh of this pre-Greedy baseline.
