# Complete Fast regression baseline

Nine logical cases preserve seven complete zstdoxide frames (1,090,164 distinct
bytes), resolved parameters and every recorded allocation survey field from
the reviewed 2026-10-09 `smoke-5` benchmark capture. The canonical complete frames
and counters agree between safe and all twelve unsafe backends enabled together.
The capture uses codec revision `00ef0d4` plus the recorded benchmark overlays;
its full source manifest and compiler identity are retained in provenance.

This is a clock-free capture, not a performance result. It is available before
DoubleFast implementation, so activating this regression does not depend on a
new timing campaign. No encoder is run to extract or regenerate expectations.
These exact nine frame bytes have Rust decoding evidence; independent C
interoperability evidence elsewhere uses separately identified cases.

Run `cargo test --test reference_fast_defaults` and
`python3 scripts/check_reference_fast_baseline.py`. The Cargo target regenerates
three independently pinned inputs and compares all nine complete frame extents,
headers, exact decoded bytes, requests, ownership and final release. It compares
every captured survey field on aarch64-apple-darwin/64-bit with the pinned
nightly-2026-10-08 toolchain. Other targets still execute all nine complete
frames and portable allocator category/ownership/release assertions; their
absolute allocation bytes are not claimed to equal this target's layouts.

Survey sizes are allocator-visible requested/returned payload sizes, including
the returned frame's residency. They do not measure RSS, backing allocator
metadata or temporary old/new storage overlap inside a resize. A toolchain
change may require explicit review of these exact expectations.

The stdlib-only offline auditor reads the immutable original archive in memory,
checks its pinned SHA256 and bounded member set, then reconstructs the exact
TSV, deduplicated frame pack and provenance from stored observations. It never
executes current or archived benchmark code, imports an encoder, launches a
process, extracts an archive to disk or rewrites an expectation. Historical
source hashes are not compared to current codec files: planned refactors must
be able to prove unchanged outputs without pretending their source is unchanged.

Keep the seven existing named-policy tests and their historical fixtures intact.
Deliberate output or allocation changes require an explicit review and a new
capture; never silently refresh this pre-change baseline.
