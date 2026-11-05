# Dictionary-free reference streaming analysis

Module 40 studies facebook/zstd v1.5.7 at
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Line references below are
`lib/compress/zstd_compress.c` in the external pinned checkout.

## Frame initialization

`ZSTD_CCtx_init_compressStream2` (6349-6438) copies requested parameters,
claims the pending prefix exactly once (6354-6357), and resolves cParams from
the pledged source size and the prefix size (6368-6375). An initial `e_end`
infers the pledge from that call's input (6366); later calls cannot change an
initialized frame's geometry. `ZSTD_getCParamsFromCCtxParams` (1637-1652)
uses a source hint only for unknown source size, chooses a preset, applies
explicit overrides, and adjusts again. The existing Rust request resolver
already implements that no-attached-dictionary sequence.

`ZSTD_adjustCParams_internal` (1473-1572) reduces the window using known
source plus dictionary size, then limits hash/chain geometry. Without a
dictionary `ZSTD_dictAndWindowLog` (1439-1466) leaves the adjusted window
unchanged. Known zero is distinct from unknown. The minimum format window
log is ten. A source hint influences geometry but does not establish an
exact pledge or content-size field.

## Header and block constraints

`ZSTD_writeFrameHeader` (4695-4759) writes a window descriptor unless the
content-size flag and a known pledge permit a single segment. The latter
uses the pledge itself as the decoder window. Content-size widths change at
256, 65,792 and 0xffffffff; dictionary identifier widths change at 256 and
65,536. Unknown size disables content-size emission. A checksum is independent
of all these choices. `ZSTD_CCtx_init_compressStream2` sets the buffered block
target from the resolved block maximum (6429-6435).

Rust will retain the existing legal non-single-segment header representation:
the descriptor will track the newly resolved window, including when content
size emission is disabled. This avoids narrowing the decoder window below
the parser's distance contract for small prefix frames. Header byte identity
with native output is explicitly not promised. Explicit Rust pledges remain
authoritative: an initial Finish does not replace a supplied pledge, and an
absent pledge stays unknown. These are deliberate streaming API differences.

## Reuse and limits

The existing compiled coordinator already stages `history | block` with no
selection and `prefix suffix | history | block` with a prefix. Its compiled
construction fixes geometry for life, so a request-backed owner must distinguish
that invariant from per-frame resolution. Reuse this state machine, block
parser, entropy state and history implementation without synthesizing an empty
compiled dictionary or duplicating the process loop. Parser plans must be
admitted before header publication, including empty frames. Only Fast,
DoubleFast, Greedy, Lazy, Lazy2 and BinaryTreeLazy2 admit the staged-history
contract. Optimal, Ultra and Ultra2 remain unsupported here even though
bounded one-shot APIs support them. LDM, workers, native workspace reuse and
complete upstream test migration remain outside this module.
