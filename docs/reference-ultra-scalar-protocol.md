# Ultra scalar observation protocol v1

This synthetic scalar adapter reaches unchanged functions from pinned v1.5.7
`zstd_opt.c`; it does not call a frame encoder, match finder, path parser,
statistics update or dictionary API. It binds both optLevel 0 and 2 explicitly.
Existing Optimal captures are not reinterpreted.

## Input and validation

A regular non-symlink file contains eight magic bytes `ZUSCAL01`, a little-endian
u32 count 1..4096, then exactly that many ten-u32 rows. The maximum is 163852
bytes. A row is `[mode, tag, a, b, c, d, e, f, g, h]`; mode is exactly 0 or 2.
All unused words must be zero, all boolean flags exactly 0 or 1. The complete
packet and all rows are validated before any upstream call or stdout record.

| Tag | Query | Payload |
| --- | --- | --- |
| 0 | Weight | a=raw statistic |
| 1 | LL price | a=length, b=sum, c=selected frequency, d=predefined |
| 2 | Match price | a=encoded offset, b=length, c=offset sum, d=offset frequency, e=ML sum, f=ML frequency, g=predefined |
| 3 | Raw literals | a=count, b=sum, c=byte-0 frequency, d=byte-1 frequency, e=disabled, f=predefined |
| 4 | Signed LL increment | a=new length, b=sum, c=previous-code frequency, d=next-code frequency, e=predefined |

LL lengths are 0..131072; increments require 1..131072; matches are 3..131072;
offsets are nonzero u32 sequence intents. Raw literal bytes alternate 0 and 1
starting at zero. Statistics are 0..4294967294 for mode 0 and 0..8388606 for
mode 2. Selected frequencies fit the sum; two distinct selected symbols must
fit together. If consecutive LL lengths use the same code their frequencies
must match. Ignored predefined/disabled statistics must be zero. Nonempty
dynamic raw literals require a cached base of at least 256. Empty raw queries
retain the upstream zero-cost fast path. Products are below the 2^30 finite
sentinel throughout the admitted domain.

The model derives LL/ML intervals from format widths. Native validation derives
the LL code only after scalar length validation, using the adapter's own bounded
interval computation, before invoking any upstream function. Native calls use
upstream symbol lookup and price arithmetic unchanged.

## Storage and output

The helper owns a bounded packet buffer, initialized 131072-byte alternating
literal source, four frequency arrays totaling 377 cells, and a zero-initialized
`optState_t`. Selected frequencies are assigned to the required symbols; remaining
mass is assigned to an unused symbol, making actual table sums equal supplied
sums. This is explicitly synthetic scalar state, not ordinary CCtx state.
Each query gets freshly zeroed state/tables. All statistic caches are set by the
upstream base-price refresh with the selected mode; disabled literals leave their
cache at zero. The selected scalar call is then made without modifying source,
tables or state. Copies and prefix/suffix guards are checked after every query.

No adapter-owned heap allocation is required. A compile-time bound covers all
static buffers plus a 64 KiB stack allowance, separately from process RSS and
libc I/O allocation. Guard checks detect writes, not arbitrary logical overreads.
ASan/UBSan cover allocated-object and arithmetic boundaries; full independent
scalar replay checks observable values.

Output is bounded JSONL: one request record with revision/schema/count; one
scalar record per row containing exact index/mode/tag/signed integer value; one
end record with count and `guards_unchanged: true`. Success requires exact output
records, zero stderr and exit zero. Invalid transport returns exit one and no
stdout; its diagnostic is fixed and bounded. Internal guard failures are not
acceptable malformed-input rejections.

## Native capture gates

Before observation, pin every library C/header source against the repository's
upstream manifest and require the exact clean upstream revision. Compile all 26
common/compress/decompress C units: include the unchanged `zstd_opt.c` in the
adapter translation unit, and compile the other 25 separately. Both normal and
ASan/UBSan builds retain debug assertions, disable assembly and multithreading,
and use the exact reviewed command inventory. Instrument every used codec unit,
not only the adapter. Keep dependencies, predefines, compiler identity, all
objects/executables, stdout/stderr, source snapshots and input bytes.

The bounded controller must enforce per-command time/output caps and campaign
caps, record actual start/end/exit/log hashes, reject unexpected commands and
recheck all source identities after execution. Offline validation may launch no
process and must compare complete observed results against the frozen independent
model, verify normal/instrumented equality, and reject forged records even when
an outer log hash is updated. Malformed transport and forged evidence are
separate controls. A native transcript is not a complete Ultra parser claim.
