# Fresh startup capture v2: sanitizer workspace accounting

The v1 capture completed all 122 native commands, including sanitizer runs, but
its final auditor rejected allocation equality. Retain that failed run as such.
The pinned upstream intentionally changes workspace sizing when ASan is enabled:
`ZSTD_cwksp_alloc_size` adds two 128-byte redzones to every nonempty non-table
allocation. `ZSTD_sizeof_matchState` contributes six optimal buffers and
`ZSTD_estimateCCtxSize_usingCCtxParams_internal` contributes five token buffers,
one temporary workspace and two block states. All are nonempty for 64..512 bytes.
The resulting workspace difference is 14 * 2 * 128 = 3,584 bytes. Stream, LDM,
external producer buffers and the workspace-local CCtx allocation are absent;
the ordinary separately allocated CCtx has identical size in both builds.

V2 keeps the exact C helper, instrumentation, requests, compiler flags and command
inventory. Effective predefines must show ZSTD_ADDRESS_SANITIZER as zero/one for
the two builds, ZSTD_CWKSP_ASAN_REDZONE_SIZE as 128, and no poison-disable macro.
Each padding pair retains exact ledger equality. Across builds, both have exactly
two allocations, identical first allocation, second allocation differing by 3,584,
peak equal to their sum, and complete release. Frame and state byte equality
remain mandatory. Fresh compilation/capture runs into a new directory and retains
the original failed evidence separately. No sanitizer findings were suppressed.
