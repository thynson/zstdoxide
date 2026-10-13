# Bounded reference Lazy-family measurements

## Frozen workload and measurement contract

The separate `lazy_benchmark` example and `benchmark_lazy.py` driver preserve the
four complete predecessor workloads, xorshift64 seed `0x5eedfa5720261009`, and all
eight original profiles. They add the six explicit chain/row profiles at each
of depths one and two, for 80 total cases: 32 unchanged controls and 48 new Lazy/
Lazy2 cases. These are synthetic deterministic workloads, not a corpus-wide
compression or upstream performance claim.

| Source | Bytes | SHA256 |
| --- | ---: | --- |
| far-history | 2,228,224 | `71ba4eaf6ed525768048f439ba3341e20f8a17c52b1503e540aa508d5a98a78c` |
| skewed | 32,768 | `613ad2caf8e6b161d48f0ce4aee8f29eaa1c45b9978876a4f9577e149ef607b2` |
| history | 393,216 | `9cc84f81533a4a4d2e16b59f04213ff5da07e716391b0d8d6314d32345937c98` |
| mixed | 524,288 | `b2153d35b896544d8086e4bb03600fdc07d3e84723a26545a878a45c7a275ce3` |

Each source uses `level-1`, `level-3` and the following six Greedy profiles,
followed by the same six with `lazy1-` and `lazy2-` prefixes. Explicit requests
start from level 3, override the strategy and minimum match to four, and set all
listed dimensions before actual known-size adjustment.

| Base profile | Window log | Hash log | Chain log | Search log | Target |
| --- | ---: | ---: | ---: | ---: | ---: |
| chain-4 | 14 | 15 | 14 | 4 | 0 |
| chain-8 | 14 | 15 | 14 | 8 | 0 |
| row-16 | 21 | 12 | 6 | 1 | 0 |
| row-32 | 21 | 12 | 6 | 5 | 0 |
| row-64 | 21 | 19 | 6 | 6 | 0 |
| row-capped | 21 | 19 | 30 | 30 | 131072 |

At each depth, row-64 and row-capped must retain identical complete frames and
allocator counters despite the unused chain dimension, capped search and ignored
target. Independent Python reconstruction checks every actual resolved tuple.
Every predecessor Greedy-era frame is compared against the immutable 32-case
baseline, with allocation fields also exact under the original compiler/target
qualification. Earlier Fast and DoubleFast baseline checks remain separate.

The harness uses std-only owned source/output vectors and an independent
TrackingAllocator survey; it adds no codec storage or unsafe implementation.
All sixteen configurations receive identical source/request bytes: safe, each
of fourteen individual unsafe leaves, then all. Complete frames and allocation
accounting must match before any performance conclusion. Old benchmark drivers,
fixtures and archived results remain unchanged.

Default codec execution is clock-free. Timed execution measures Global one-shot
request resolution, allocation, compression and output destruction. Generation,
decoding, hashing, files and the allocator survey remain outside clocks. Five
samples per case each run for at least 100 ms; complete output is checked outside
those samples. All requested executables are built before sampling. The JSON
metadata explicitly requires 80 cases per configuration; inherited human-facing
console labels from the shared Greedy harness do not define the case count.

Timing requires completed unchanged matrix, platform and all-backend clock-free
evidence, plus a recorded barrier showing no task-owned CPU work remains. A
separate safe/all repeat is required. Unrelated desktop work, fixed order, thermal
and frequency changes remain limitations. Unused unsafe leaves cannot be assigned
causal speedups, and no C throughput/ratio or foreign-native timing is measured.

## Clock-free verification

All 80 cases pass on all sixteen configurations, with exact complete output and
allocator equality. All 32 predecessor frames and qualified owner records remain
unchanged. A separate safe/all smoke supplies bounded adversarial harness
controls: normal and optimized Python each pass two valid captures and reject
49 mutations, with zero child execution. The [harness archive](evidence/2026-10-13-lazy-benchmark-harness.json)
preserves every mutated record, frame and validation log. Timed campaigns and their interpretation remain pending until the
correctness/platform barrier is complete.

The [all-backend clock-free report](evidence/2026-10-13-lazy-clock-free.json)
records 1,280 cases across sixteen configurations and 37 successful commands.
Normal controller validation and a separate optimized offline replay agree;
all 80 complete frames and owner records match the separately frozen safe/all
smoke. Every archive member and reconstructed file was verified. Packaging
initially rejected an obsolete human-readable replay label before writing an
archive; the retry checks the actual Lazy-family label. This packaging correction
does not change the campaign, validator, example or any codec source.
