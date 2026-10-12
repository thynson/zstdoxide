# Ordinary Greedy helper v3: retain resolved automatic selection

The first ordinary capture with helper v2 failed at `normal-chain_empty` before
any frame or candidate record was emitted. Its diagnostic was
`applied disabled-control mismatch`. The immutable failed directory remains at
`/private/tmp/zstdoxide-greedy-frames-2026-10-12`; the repository evidence archive
retains all 35 commands' stdout/stderr and the failure report, including
input/compiler/binary hashes. Generated sources, object files and the executable
remain external and are not duplicated in this failure-log archive.

Source review identifies an adapter contract error. In pinned v1.5.7,
`ZSTD_CCtxParams_init_internal` (`zstd_compress.c:385`) resolves automatic row
selection from the explicit strategy and window. `ZSTD_resetCCtx_internal`
requires that switch already resolved (`zstd_compress.c:2120`), and
`ZSTD_compressBegin_advanced_internal` does not resolve it again. The v2 helper
incorrectly reset the switch to `ZSTD_ps_auto` after initialization. The first
chain case was rejected by the helper's applied-state guard. No row case ran;
there is no valid compression, decode or sanitizer-runtime result from this run.

The new helper `scripts/oracles/greedy_frame_v3.c`, SHA256
`0e7505aa18263f1483320fb697d95ea616daa9b471a217c3445ef154d50598c1`, replaces that setter
with a check that the library initializer selected enable exactly when W>14.
It retains the library's selected value and does not duplicate or override its
selection algorithm. This also restores the row chain-allocation exclusion on
which the bounded C resource review depends. No earlier source or failed output
is overwritten. All other helper statements are byte-identical to v2, and the
observation format remains schema 2. Normal and sanitizer-configured strict C11
syntax checks pass before this successor's first execution.

The successor controller pins this helper and review. Its command plan, recipe
identities, validator and resource/path checks are unchanged. The original
controller remains available for historical verification, but live capture must
use `scripts/capture_greedy_frames_v2.py`. Neither this correction nor the failed
capture changes production Rust code or public Greedy admission.
