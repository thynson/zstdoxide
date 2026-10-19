# Fail-fast public Ultra evidence validation

The original and v2 full positive audits both accepted the unchanged native
campaign. Their incomplete adversarial campaigns are retained as interrupted
runs, not counted as passing controls. V3 is additive; all earlier scripts,
source pins, raw commands, responses and diagnostic records stay unchanged.

V3 checks the independently specified totals, equivalence count and all 1345
pairs of resolved Rust metadata tuples before expensive payload replay. Integer
fields require actual integers, rejecting JSON booleans and equal-valued floats.
These expected values come from the frozen source-only recipes, not observed
responses. The existing payload-versus-metadata comparisons still follow.

The unconditional artifact inventory is moved to the end of successful semantic
replay. Every accepted capture still undergoes every original regular-file,
bounded-length and complete SHA-256 check, including otherwise unused compiler
stdout/stderr. No file is cached or omitted. A malformed record can now fail
before scanning unrelated payloads, instead of repeatedly reading the complete
campaign before rejecting a known-wrong tuple. V2's checked direct-directory
containment remains in force. No native codec is executed by these audits.

The v3 controls preserve all 53 v2 cases and add boolean/floating resolved
metadata and floating totals with otherwise equal values. Normal and optimized
Python are dispatched independently against the same immutable capture. The
completed reports, exact sources and source-bound positive results are retained
under `audit-successors/v3` and the capture root. Prior interrupted mutation
snapshots remain under `preserved-controls-v1` and `preserved-controls-v2`.
The original complete auditor remains the frame projection prerequisite.
