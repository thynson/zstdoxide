# Retained Ultra complete-state fixture

This immutable `ZURFX01\0` pack records 42 independently audited policy-0/2
native cases from facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Source recipes, observer adaptation and
capture controls precede projection. Upstream attribution is retained in
`NOTICE` and `LICENSE.zstd`; the upstream executable is an oracle only.

The 28853730-byte pack has SHA-256
`26698e24f151d3721f0882229515740aa20e5d4f6dbc3d62b96dee8b0048df09`.
It contains 134 blocks, 207635 complete events, 102307 queries, 543 emitted
sequences and 530 alternative observations, including 348 after the first block.
All 19 predecessor source cases are retained under each policy, plus two new
four-block alternative sources under each policy. Case source, frame and packet
identities and every case/block offset are recorded in `provenance.json`.

Path, frequency and search deltas persist across blocks. Sequence counts reset
per block; committed and pending repeat owners are explicit. Policy-2 alternatives
retain twelve signed 64-bit values. The format ownership and bounded borrowed
reader contract are specified before implementation in `docs/data-structures.md`.

`scripts/pack_ultra_retained_fixture.py` reaudits the complete native capture,
then projects observed state. A separate script,
`scripts/check_ultra_retained_fixture.py`, reconstructs every state byte from
source recipes and independent policy replay. Ordinary and optimized Python
agree exactly. Both reject 30 effective forgeries with the root digest bypassed,
including later-block alternative operands, publication owners, search/frequency
state and schedule coordinates. Neither checker launches a child process.
Native frame bytes and allocation sizes remain independently audited opaque
inputs; the state model does not pretend to predict entropy or allocation sizes.

Projection, initial design/decomposition, all 56 scoped inputs, ordinary/optimized
reconstruction outputs and lossless forgery deltas are retained in
`docs/evidence/2026-10-21-ultra-retained-fixture.{json,tar.gz}`. Archive SHA-256:
`e3fefd1d5f5c35a1b29813f257c7e11c9c0f9a2ec1329724b399b8ab79ecdd4c`.
The original native campaign remains separately pinned by archive identity.
This fixture alone grants no production retained Ultra admission.
