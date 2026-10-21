# Retained Ultra observation protocol

`ZURTS01` is an additive test-only transport for pinned zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It does not expose production
retained Ultra support. The independent request/model and source-only recipes
precede native observation. The complete existing `ZOPRS03` packet is nested
unchanged at byte 12; no predecessor helper or capture is overwritten.

| Byte offset | Field |
| --- | --- |
| 0 | Eight bytes `ZURTS01\0` |
| 8 | Little-endian u32 price policy, exactly 0 or 2 |
| 12 | Eight bytes `ZOPRS03\0` |
| 20 | Source byte count, 128..32768 |
| 24 | Block count, 2..4 |
| 28, 32, 36, 40 | Window, hash, chain, search logs |
| 44, 48, 52 | Minimum match, target, literal mode |
| 56 | The declared number of little-endian u32 block lengths |
| 56 + 4 * block count | Exact immutable source bytes |

Complete packets occupy 192..32840 bytes. Blocks each contain 64..8192 bytes
and sum exactly to the source length. Window logs are 10..16, hash/chain logs
6..12, search logs 1..6, minimum-match settings 3..7, target 0..4095 and literal
mode 0/1. The complete source fits the selected window. Trailing data, booleans
in Python value requests, malformed geometry and unknown policies are rejected.

The frozen 42-request manifest has SHA-256
`888f364acc26b310d82b397dd5508a35afb2d000a311858377fc99ce5a715636`.
Nineteen predecessor inputs are retained under each policy; two added four-block
alternative sources are also exercised under each policy. Policy 0 preserves all
nineteen complete predecessor state hypotheses. There are 207635 logical events
per padding pass across the corpus, including 348 policy-2 alternative events
after the first block. These are source-derived expectations, not native results.
Each manifest row binds packet/source hashes, exact parameters/schedule, explicit
publication hypotheses, complete state hash, branch counts and output budget.

Observations are newline-terminated JSON: one metadata row, two guarded padding
passes with block-begin/event/block-publish records, then one completion row.
Each row includes an exact integer price policy. Events retain all predecessor
path/search/frequency/sequence/repeat fields and add `alternative`, either null
or twelve signed integer fields: seven saved-match cells, LL-price increment,
with-one-literal price, with-more-literals price, old next-cell price and
predecessor index. The increment may be negative. JSON booleans/floats are not
integer substitutes. Duplicate fields, nonfinite values and trailing records
are invalid. Sparse baselines persist across blocks within one padding pass.

The observer retains all 4099 path cells and complete bounded search owners.
Per-block event and sequence budgets are 32768 and 2731. Per-request stdout is
bounded by 256 MiB, and source-derived conservative bounds are additionally
checked before execution. The largest frozen bound is 90603078 bytes; the sum
for both normal/sanitized builds is 1230723400 bytes. The aggregate campaign
limit remains 2 GiB including diagnostic/compiler output. Each source-sized
frame buffer has 64 additional bytes, covering frame and block headers plus the
native empty final block; it is not the former 32768-byte fresh-frame buffer.

Native wire kinds are checked against the frozen publication hypotheses and
complete independent state replay. The model's publication input does not force
C or Rust entropy decisions. Actual Rust complete-frame publication remains a
later independent gate. Native frame equality/decoding and allocator ledgers do
not substitute for complete parser-state reconciliation.
