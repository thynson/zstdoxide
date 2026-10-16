# Wider retained native capture

The 2026-10-16 native capture passed all 124 planned commands. Seven recipes
produce 37,977 events, 12,510 queries and 31 sequences per build/padding pass.
All 28 complete frames and 28 allocator ledgers agree across normal/sanitized
builds and both padding values; 24 malformed packets are rejected per build.
The independent artifact gate checks the exact 110-artifact inventory. Normal
and optimized offline controls each accept two unmodified captures and reject
70 forgeries without launching children. Initial control-adapter attempts used
an obsolete executable filename and failed before the artifact mutation; their
logs remain beside the corrected passing runs in the evidence archive. Capture
inputs were unchanged. The archive contains 463 individually hashed members,
including all native logs and immutable source snapshots. Wider production
admission and fixture projection remain separate subsequent work items.

An initial documentation commit appended this report to the frozen protocol
file. The fixture projector rejected its source hash before producing output.
The original protocol bytes were restored and this report was moved to a
separate file; no native capture was rerun or altered.

The projected ZORFW02 fixture has seven cases, 37,977 events, 12,510 queries,
31 sequences and all complete frames. Its 5,307,187 bytes hash to
`ee53ed6f93e5c289f5793370482f1b2b5b8d076339207a7c059cc15e4633503d`.
Normal and optimized Python reproduce identical bytes after checking every
archive member, frozen source identity and independently replayed native state.
