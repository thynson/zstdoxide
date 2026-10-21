# Retained Ultra complete-frame baseline

`cases.bin` contains 26 source/request/resolved-parameter/complete-frame records
selected before timing from the independently audited 1593-request public
campaign. `provenance.json` records their names, public recipe indices, offsets,
byte lengths, block kinds and SHA-256 identities. The pack begins with
`ZURB0001` and a little-endian record count; fields are decoded by the independent
Python checker and bounded Rust benchmark reader.

Ordinary and optimized Python projection produce identical bytes. Each selected
safe/all Rust frame is compared exactly with its captured frame, and both normal
and sanitized native adapters decode it to the full original source. The native
reference is facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, used only as an oracle under its
BSD/GPL dual-license distribution. Generated source recipes and Rust output
frames are retained here; no C codec is linked into the Rust implementation.

The qualified native archive SHA-256 is
`4bf5ba6509ea0096696994765e6d4c034d71b8480713374dcd1c7cfaadf332db`.
The 824348-byte pack SHA-256 is
`9be9a2de5b1d4ea624185d45c7ebbfd2abbe18aa12af29c35175b1c525db82a2`.
Exact per-file pins live in `scripts/check_retained_ultra_frame_baseline.py`.
The 688138 source bytes and 132267 frame bytes describe this fixed selection,
not a representative compression-ratio claim or a timing result.
