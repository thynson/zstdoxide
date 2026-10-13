# Frozen ordinary lazy-family observations

The 510 source-only recipes were captured from unchanged facebook/zstd v1.5.7
with the reviewed version-one helper and controller. All 26 used codec units and
the helper were compiled separately in normal and ASan/UBSan builds. Every
observation and complete frame agrees across builds. All 138 ordinary depth-zero
predecessors retain exact records (apart from the documented name prefix) and
complete frame bytes.

`capture.json` indexes consecutive SHA256-bound extents in `artifacts.bin`:
510 NDJSON trajectories, 510 scheduled frames, 39 aligned whole frames, one
metadata record and 45 qualified incomplete-header observations. The pack is
103,211,609 bytes. Run `python3 scripts/check_lazy_frames.py --check` to replay
all maps, sequences, publications and frames independently without executing C.

Actual coverage: 1,377 positive blocks (633 raw, 114 RLE, 630 compressed), 1,293
fresh candidates and 201 changed-repeat fallback witnesses (165 raw, 36 RLE).
Each build decoded 549 complete frames / 41,165,274 source bytes and rejected
51 malformed commands. Header-only observations are incomplete and undecoded.
The manifest identifies the complete raw capture archive and offline controls.
Concatenate its three archive parts in listed order to recover the gzip stream;
the full SHA256 and all part identities are retained and checked on publication.

This is internal explicit-parameter evidence. It does not establish public
parameter resolution, per-probe tracing, collector equivalence, performance or
complete Zstandard replacement support. Source hypotheses are retained even
when their named outcome was not observed.
