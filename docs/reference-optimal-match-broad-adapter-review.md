# Broader Optimal match adapter review

The helper includes unchanged pinned v1.5.7 zstd_opt.c, revision
f8745da6ff1ad1e7bab384bd1f9d742439278e99, replacing only its separate object.
The guarded tiny observer remains unchanged. The broader helper differs only in
source/query/window limits, the exact CCtx short-key log rule, independent table
capacities and protocol identity. Main heads/paired cells remain4096; short-key
heads and their snapshot copy grow to131072. Each table initializer/checker gets
its own explicit capacity. Source capacity524288 and packet capacity524716 are
separate constants; all length arithmetic follows header-range validation.

Every query leaves eight physical source bytes, all repeats are bounded by the
native current position and4096, and length-to-beat cannot exceed remaining
source. The two-byte initialized prefix and logical end stay unchanged. Short
heads are optional: minMatch3 requires exactly min(window,17), all other minima
require zero and a null upstream short-table pointer. Unused short storage and
all active/inactive boundaries retain checked sentinels. No arbitrary seeds or
external dictionary state enter the observer. The static assertion includes all
named owners, before-state copies, state, packet, source and I/O buffer plus a
64KiB stack allowance under4MiB; libc/sanitizer internals are outside this ledger.

The planned180commands compile all26used codec units in normal and fully
ASan/UBSan-instrumented modes, with DEBUGLEVEL=2 and disabled assembly, legacy
support and multithreading. Each build executes37positive and22malformed packets.
Timeouts remain120seconds per command and1800seconds per campaign. Positive
stdout is capped64MiB, other stdout2MiB, stderr1MiB, aggregate output256MiB.
The independent broader parser caps lines8MiB and complete stdout64MiB. Two
padding passes and both C builds must agree in every complete map and candidate.

Before C execution, the chunked independent suffix comparison must match the
scalar tiny model for every tiny operation and deterministic chunk-boundary
sources. Wider synthetic checks separately derive the strict2^18distance cutoff
and repeat threshold equality. Those model results are not C observations.
Preflight must verify pinned dependencies, exact commands, syntax in both modes,
malformed transport and no-child plan forgeries before capture starts.

Ordinary parser startup positions, path decisions, large production primary/node
geometry, dictionary modes and public Optimal admission remain outside scope.
Fresh1/4/8repeat values are only passed at pointer-safe positions>=6.
