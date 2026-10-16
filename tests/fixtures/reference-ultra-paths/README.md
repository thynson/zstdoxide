# Coupled Ultra scripted path observations

Pinned reference: facebook/zstd v1.5.7, commit
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. The original algorithm and derived
observations retain the upstream attribution in `../../../LICENSE.zstd`.
Adapter scope is documented in `../../../docs/reference-ultra-path-adapter-review.md`. The upstream source is
kept outside this repository and is only an oracle.

`cases.bin` is an immutable, little-endian projection of 58 independently audited
source-only scripted requests (29 per policy). It contains 65,672 events, 22,798
queries, 72 signed saved-match alternatives and 143 emitted sequences. Its
8,796,312 bytes have SHA-256
`b5f990623fcc0c09c3d55e86bffa4b3af1a7f9e2decab1a211aaffa70435a7f9`.
`provenance.json` records request/transcript identities and per-case offsets.

The `ZUPFX01` schema, bounds and ownership are specified in
`../../../docs/data-structures.md`. Reproduction uses the frozen
`scripts/capture_ultra_paths.py` command plan, then
`scripts/pack_ultra_path_fixture.py`; `scripts/check_ultra_path_fixture.py`
independently reconstructs every byte from source-only replay. Both Python modes
produce identical packs and provenance and reject 24 reconstruction forgeries
each, including signed operands and publication changes with root digest checks
bypassed. Full native and projection evidence is archived in `docs/evidence`.

These observations use a scripted candidate provider. They do not establish
ordinary match discovery, public Ultra support or complete upstream parity.
