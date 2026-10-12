# Bounded reference DoubleFast measurements

## Frozen workloads and measurement contract

The separate `double_fast_benchmark` example and `benchmark_double_fast.py`
driver preserve the existing Fast harness unchanged. Four deterministic sources
use its xorshift64 seed `0x5eedfa5720261009` and exact phrase/skew distribution.
The original skewed (32 KiB), history (384 KiB) and mixed (512 KiB) bytes retain
their old SHA256 identities. A new far-history source contains one 131,072-byte
noise block (seed xor 1), eleven noise blocks (seed xor 3), the first block again,
and four copies of the existing skewed 131,072-byte block: 2,228,224 bytes. Its
SHA256 is `71ba4eaf6ed525768048f439ba3341e20f8a17c52b1503e540aa508d5a98a78c`.
The copied first block is beyond Fast's default window log 19 and within window
log 21.

Each source uses four requests in fixed order: reference level 1, level 3,
explicit DoubleFast window21/hash18/chain6/minimum4, and explicit DoubleFast
window21/hash6/chain18/minimum4. Overrides start from level 3. Source-size
adjustment is expected: the 32 KiB input reduces hash log 18 to 16 or chain log
18 to 15; its search log remains 2, while the other sources use 1. Independent
Python expectations come from pinned v1.5.7 `clevels.h` and known-source
normalization. These are sixteen fixed cases, including twelve DoubleFast
cases. The three historical level 1 frames and allocation records must match
the unchanged pre-DoubleFast fixture. The new far-history Fast case has no
historical baseline.

Input bytes and parameter tuples are frozen before execution; current encoder
output is not used to create those expectations. All sources and saved frames
are owned initialized Vec values in this std-only test harness. No new unsafe
storage or codec API is introduced. Each backend must produce identical complete
frames and allocation counters. The Rust memory layouts are two usize tables
of `2^hash_log` and `2^chain_log` entries for DoubleFast, one for Fast, plus
independently owned output/search/entropy scratch. The survey measures allocator
requests/returns, live peaks and exact owner release; it excludes allocator
metadata and transient old/new storage overlap during growth.

Default execution is clock-free. Timed execution measures only global one-shot
request resolution, allocation, encoding and output destruction. Two untimed
calls establish complete deterministic output. Each of five samples per case
runs for at least 100 ms, followed by a separate full-output comparison. Source
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
implied by this bounded desktop experiment. The verified campaigns below retain
their complete samples and qualifications.

## Verified clock-free harness

Safe and all-feature builds pass all sixteen cases, with identical complete
frames and allocation surveys. Three original level 1 frames and their qualified
allocation fields agree with the immutable Fast fixture. Normal and optimized
Python independently replay the capture and each pass two positive and 36
rejection controls with child processes disabled. Controls include coherent
changes to both backends' Fast frames or allocator peaks; the historical fixture
still rejects them. Strict example Clippy safe/all and formatting pass.

The [harness evidence](evidence/2026-10-12-double-fast-benchmark-harness.json)
and [reconstruction archive](evidence/2026-10-12-double-fast-benchmark-harness.tar.gz)
preserve the frozen sources, nine build/setup/run commands, all output bytes,
raw surveys, validator controls and logs. No sample clock ran. The archive's
source design document precedes this checkpoint paragraph; its input and
measurement contract is unchanged. That archived prose calls the position
elements u32; the actual Rust storage and this document use usize. The captured
requested byte counters already reflect the real Rust element width, not the
C width.

```sh
python3 scripts/benchmark_double_fast.py /tmp/new-double-fast-smoke --configs safe,all
# Finish other task-owned CPU work before enabling clocks.
python3 scripts/benchmark_double_fast.py /tmp/new-double-fast-timing --timed
python3 scripts/benchmark_double_fast.py /tmp/new-double-fast-timing --validate-only
python3 -O scripts/benchmark_double_fast.py /tmp/new-double-fast-timing --validate-only
```


## Measured campaigns on 2026-10-12

The full campaign builds all fourteen configurations before measurement and
records 224 cases with 1,120 samples. A subsequent safe/all campaign adds 32
cases and 160 samples using identical executable hashes. All 256 complete
frame/allocation records match the clock-free capture. Four normal/optimized
Python replays pass, and the independent review recomputes every five-sample
median and throughput from raw elapsed times and iteration counts. Normal and
optimized Python each pass two positive and fourteen timing-specific rejection
controls, with child processes disabled.

There is no repeatable all-feature speedup in these two campaigns. The full
safe/all median-cost ratio spans 0.898–1.047; the repeat spans 0.908–1.048.
The ratio is safe ns/byte divided by all-feature ns/byte, so values above one
mean all features were faster in that observation. The only full-campaign case
above one reverses in the repeat, and several other cases reverse direction.
These results do not support changing the safe default or assigning a causal
speed ranking to individual backends. All individual-feature values remain
available in the [complete backend table](evidence/2026-10-12-double-fast-backends.tsv).

The host reports macOS 15.7.7 and aarch64-apple-darwin, with the pinned
nightly-2026-10-08 compiler. Task-owned correctness, Miri and build processes
finished before clocks. A pre-existing cargo-release process in another project
was sleeping at 0% CPU at the barrier and was left untouched. The desktop was
not isolated from unrelated work, scheduling, thermal or frequency changes.
The configuration and case order were fixed. No task-owned builds/tests/Miri
ran concurrently with sample clocks; only brief progress reads occurred.

| Workload / request | Full safe MiB/s | Full all MiB/s | Repeat safe MiB/s | Repeat all MiB/s |
| --- | ---: | ---: | ---: | ---: |
| far-history / level-1 | 312.54 | 303.78 | 307.02 | 308.73 |
| far-history / level-3 | 342.35 | 324.13 | 334.29 | 332.34 |
| far-history / long-18-short-6 | 334.68 | 316.83 | 332.29 | 328.73 |
| far-history / long-6-short-18 | 340.86 | 307.55 | 331.82 | 322.58 |
| skewed / level-1 | 135.03 | 121.24 | 125.08 | 127.12 |
| skewed / level-3 | 81.86 | 77.83 | 79.55 | 80.13 |
| skewed / long-18-short-6 | 87.22 | 83.56 | 89.06 | 90.38 |
| skewed / long-6-short-18 | 107.84 | 99.94 | 102.74 | 102.19 |
| history / level-1 | 361.17 | 355.56 | 350.07 | 363.80 |
| history / level-3 | 341.59 | 324.15 | 350.75 | 318.38 |
| history / long-18-short-6 | 311.50 | 326.23 | 331.15 | 328.58 |
| history / long-6-short-18 | 310.64 | 308.20 | 302.35 | 306.68 |
| mixed / level-1 | 377.36 | 372.10 | 375.14 | 375.67 |
| mixed / level-3 | 404.86 | 401.06 | 398.66 | 393.87 |
| mixed / long-18-short-6 | 404.97 | 400.67 | 396.79 | 367.27 |
| mixed / long-6-short-18 | 436.62 | 428.86 | 409.28 | 429.06 |

Frame sizes and allocation surveys are identical across all configurations and
both campaigns. Peaks below are allocator-visible returned bytes, including the
returned frame while it is resident, under the scope described above.

| Workload / request | Source bytes | Frame bytes | Peak bytes | Allocations / grows |
| --- | ---: | ---: | ---: | ---: |
| far-history / level-1 | 2,228,224 | 1,709,887 | 3,684,243 | 373 / 86 |
| far-history / level-3 | 2,228,224 | 1,578,459 | 5,047,077 | 453 / 100 |
| far-history / long-18-short-6 | 2,228,224 | 1,578,459 | 5,571,877 | 453 / 100 |
| far-history / long-6-short-18 | 2,228,224 | 1,578,311 | 5,581,834 | 366 / 88 |
| skewed / level-1 | 32,768 | 3,149 | 602,041 | 117 / 20 |
| skewed / level-3 | 32,768 | 2,682 | 1,297,619 | 116 / 13 |
| skewed / long-18-short-6 | 32,768 | 2,688 | 1,035,947 | 116 / 13 |
| skewed / long-6-short-18 | 32,768 | 2,682 | 749,267 | 116 / 13 |
| history / level-1 | 393,216 | 262,172 | 1,482,103 | 44 / 6 |
| history / level-3 | 393,216 | 262,172 | 2,923,895 | 45 / 6 |
| history / long-18-short-6 | 393,216 | 262,172 | 3,448,695 | 45 / 6 |
| history / long-6-short-18 | 393,216 | 262,172 | 3,448,695 | 45 / 6 |
| mixed / level-1 | 524,288 | 135,514 | 1,987,978 | 224 / 40 |
| mixed / level-3 | 524,288 | 135,144 | 3,350,442 | 224 / 40 |
| mixed / long-18-short-6 | 524,288 | 135,151 | 3,911,291 | 224 / 40 |
| mixed / long-6-short-18 | 524,288 | 134,286 | 3,705,036 | 137 / 20 |

See the [independent reconciliation](evidence/2026-10-12-double-fast-measurements.json)
and [reconstruction archive](evidence/2026-10-12-double-fast-measurements.tar.gz).
The archive preserves both raw campaigns, every frame, source/build identities,
environment, ordered commands, sample counters, controls and CPU-barrier record.
All 1,559 archive members and reconstructed file identities were verified.
Compiled binaries/targets are omitted with their identities retained. The
standalone TSV is a direct projection of the archived per-backend medians,
including the safe/all repeat, without discarding any configuration or case.
