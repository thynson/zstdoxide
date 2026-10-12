# Bounded reference DoubleFast measurements

## Frozen workloads and measurement contract

The separate `double_fast_benchmark` example and `benchmark_double_fast.py`
driver preserve the existing Fast harness unchanged. Four deterministic sources
use its xorshift64 seed0x5eedfa5720261009 and exact phrase/skew distribution.
The original skewed32KiB, history384KiB and mixed512KiB bytes retain their old
SHA256 identities. A new far-history source contains one131072-byte noise block
(seed xor1), eleven noise blocks (seed xor3), the first block again, and four
copies of the existing skewed131072-byte block:2,228,224 bytes. Its SHA256 is
71ba4eaf6ed525768048f439ba3341e20f8a17c52b1503e540aa508d5a98a78c.
The copied first block is beyond Fast's default window19 and within window21.

Each source uses four requests in fixed order: reference level1, level3,
explicit DoubleFast window21/hash18/chain6/minimum4, and explicit DoubleFast
window21/hash6/chain18/minimum4. Overrides start from level3. Source-size
adjustment is expected: the32KiB input reduces hash18 to16 or chain18 to15;
its search log remains2, while the other sources use1. The independent Python
expectations come from pinned v1.5.7 `clevels.h` and known-source normalization.
These are sixteen fixed cases, including twelve DoubleFast cases. The first
three historical level1 frames and allocation records must match the unchanged
pre-DoubleFast fixture. The new far-history Fast case has no historical baseline.

Input bytes and parameter tuples are frozen before execution; current encoder
output is not used to create those expectations. All sources and saved frames
are owned initialized Vec values in this std-only test harness. No new unsafe
storage or codec API is introduced. Each backend must produce identical complete
frames and allocation counters. The Rust memory layouts are two usize
tables of2^hash_log and2^chain_log entries for DoubleFast, one for Fast, plus
independently owned output/search/entropy scratch. The survey measures allocator
requests/returns, live peaks and exact owner release; it excludes allocator
metadata and transient old/new storage overlap during growth.

Default execution is clock-free. Timed execution measures only global one-shot
request resolution, allocation, encoding and output destruction. Two untimed
calls establish complete deterministic output. Each of five samples per case
runs for at least100ms, followed by a separate full-output comparison. Source
construction, decoding, fingerprints, saved files and TrackingAllocator surveys
are outside clocks. All requested executables are built before any sample.
The coordinator must finish task-owned correctness/Miri/build jobs before a
timed campaign. Other desktop workloads, thermal/frequency changes and fixed
configuration order remain measurement limitations.

The complete comparison uses safe, each of twelve individual unsafe features,
and all together. Some features do not participate in this compression path;
their measurements cannot establish a backend's benefit. Keep every sample,
including unfavorable results, and repeat safe/all to test stability. No C
throughput, C ratio parity, universal speedup or foreign-native performance is
implied by this bounded desktop experiment. Timing has not yet run.

## Verified clock-free harness

Safe and all-feature builds pass all sixteen cases, with identical complete
frames and allocation surveys. Three original level1 frames and their qualified
allocation fields agree with the immutable Fast fixture. Normal and optimized
Python independently replay the capture and each pass two positive and36
rejection controls with child processes disabled. Controls include coherent
changes to both backends' Fast frames or allocator peaks; the historical fixture
still rejects them. Strict example Clippy safe/all and formatting pass.

The [harness evidence](evidence/2026-10-12-double-fast-benchmark-harness.json)
and [reconstruction archive](evidence/2026-10-12-double-fast-benchmark-harness.tar.gz)
preserve the frozen sources, nine build/setup/run commands, all output bytes,
raw surveys, validator controls and logs. No sample clock ran. The archive's
source design document precedes this checkpoint paragraph; its input and
measurement contract is unchanged. That archived prose calls the position elements
u32; the actual Rust storage and this document use usize. The captured requested
byte counters already reflect the real Rust element width, not the C width.

```sh
python3 scripts/benchmark_double_fast.py /tmp/new-double-fast-smoke --configs safe,all
# Finish other task-owned CPU work before enabling clocks.
python3 scripts/benchmark_double_fast.py /tmp/new-double-fast-timing --timed
python3 scripts/benchmark_double_fast.py /tmp/new-double-fast-timing --validate-only
python3 -O scripts/benchmark_double_fast.py /tmp/new-double-fast-timing --validate-only
```
