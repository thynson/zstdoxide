# Ordinary Greedy helper v2: pre-execution floor correction

The original helper and review remain preserved and **unexecuted**. Further
source review before capture identified an overstrong tiny-call assertion:
`ZSTD_compress_frameChunk` clamps nextToUpdate to window.lowLimit at
`zstd_compress.c:4633`, before `ZSTD_buildSeqStore` can return for length<7.
Thus a tiny call can advance the sampling frontier even though it neither
performs the later 384/192 adjustment nor enters Greedy parsing.

The reviewed successor is `scripts/oracles/greedy_frame_v2.c`, SHA256
`46f3d60e7d65616c163e6938f13d1cf2883a6525897b8755b47f1c47de65d91d`.
It identifies metadata schema 2, retaining the same observation fields and CLI.
The tiny check now expects exactly `max(incoming nextToUpdate, window.lowLimit)`;
all other sampling scalars, cache cells and both selected tables must remain
unchanged. Zero calls still return before window updates and preserve the whole
sampling state. Sampling equality compares named fields and initialized cache
elements, avoiding any reliance on padding preservation during C struct copies.

The no-dictionary floor is derived from the raw block-start coordinate in
`ZSTD_window_enforceMaxDist` (`zstd_compress_internal.h:1256`), not the block end.
Its update raises lowLimit monotonically and raises dictLimit to match it.
After this floor clamp, fresh length>=7 calls apply the separate 384/192
next-to-update adjustment. Both transitions precede the direct parser; neither
belongs in its per-probe chain/row insertion function. The future Rust retained
context and offline ordinary validator must preserve that ordering.

All allocation, extent, source-lifetime, alignment, replay and output contracts
from the [original review](reference-greedy-frame-helper-review.md) remain in
force. The successor adds no owner or source read; it changes one expectation
and compares existing initialized fields explicitly. Strict normal and
sanitizer-configured syntax checks pass. No helper mode or sanitizer runtime
has executed. Capture must pin this successor, while the original source and
review evidence remain unchanged.

Additive source-only cases will place lengths1..6 after two uniform
window-sized blocks in both automatic branches, then follow with noise. This
is a pre-execution hypothesis for the missing floor-clamp boundary, not a
claimed C observation or a replacement for the original 126 recipes.
