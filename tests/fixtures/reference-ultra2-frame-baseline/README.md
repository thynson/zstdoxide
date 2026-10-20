# Complete Ultra2 frame baseline

These 26 cases were selected from immutable source-only hypotheses before their
new native observations. The selection and projector identity are retained in
`preselection.json`. `cases.bin` (ZU2B0001) stores each request, resolved tuple,
complete source and complete Rust frame; `provenance.json` binds native capture
replay, source/frame identities and both independent C decoders.

The source set covers default levels 16/17 at 64/2048 bytes with uniform, periodic
and noise inputs; minimum matches 3..7 at target 1/4095; wider horizon/window
cases and the two sources with changed seeded paths. No timing or owner survey
is implied by this projection.

Pack SHA-256:
`5800d0e13d324f2401f2739e1482ad8d9eb8afe42f2118933afa42b7b1b3896f`.
Provenance SHA-256:
`2b234e63470bcba505dc6ed459537011613e37635aa7205610597afbf357e1b3`.
`python3 scripts/check_ultra2_frame_baseline.py` independently validates all
fields, exact byte bounds, immutable digests and source-only recipe agreement.
The old Ultra baseline is unchanged.

Qualification archive: `docs/evidence/2026-10-20-ultra2-frame-baseline.tar.gz`,
SHA-256 `9460dc99f255934d5f3001102432cfbc73321b4ba44aeb1d38286f334d070de5`.
The raw projection and normal/optimized checker logs are retained. The wrapper
records these observed results without inventing command environment metadata.
