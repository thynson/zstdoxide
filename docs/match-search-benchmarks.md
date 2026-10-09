# Configurable match-search measurements

Measured on 2026-10-09, macOS-15.7.7-arm64-arm-64bit-Mach-O, target `aarch64-apple-darwin`,
Rust 1.101.0-nightly (`1d81eb4ad`), toolchain `nightly-2026-10-08`.
The source footprint is `296a5e13d6415520d2daa99581e8a9f03965ddaeaf9bedb468a2e913591b9c13`;
all codec/build/test inputs match the immutable final correctness snapshot
`9537371`. Design and evidence-only commits precede capture at
`bb41ac072fd912da50a99c24dc81bfb28c82582b`. Fourteen configurations, 27 rows each,
and five samples of at least 100 ms produce 1,890 elapsed samples.
The recorded run order is safe, each of twelve individual features, then all.
All task compilation, tests and Miri finished before elapsed measurements.

The experiment shows concrete tradeoffs. In the safe block workload, compact
Greedy retains 16,384 search bytes instead of default 655,360, but emits 6,843
bytes instead of 3,810 and costs 15.417 ns/input-byte instead of 11.891. The
wide history profile reaches the repeated A block: roughly 262,173–262,174 frame
bytes versus 393,235 for default/compact. Its extra retained memory and repeated
history rebuilding remain part of the measured cost. Increasing every search
control is not guaranteed to improve either compressed size or speed.

The all-feature/safe comparison varies by row: safe/selected median ratios span
0.918–1.099. Some rows favor safe and others favor all features. Individual
features can be inactive in timed encoding; the fixed order and visible sample
variation do not support attributing every difference to an accessor or claiming
a universal speedup. Unsafe backends remain opt-in.

[Measurement evidence](evidence/2026-10-09-match-search-measurements.json),
[complete archive](evidence/2026-10-09-match-search-measurements.tar.gz), and
[all backend comparisons](evidence/2026-10-09-match-search-backend-comparison.tsv)
preserve raw samples, options, source/frame fingerprints, full allocation
surveys, compiler/environment, exact commands and reproduction sources. The
unmodified driver and normal/optimized replay both validate the complete matrix.
The earlier [clock-free harness validation](match-search-validation.md#clock-free-benchmark-harness)
separately covers malformed evidence and independent input pins.

Reproduce with Python 3.11 or newer and the pinned Rust toolchain:

```sh
# No timing; validate all deterministic workloads and backend records.
python3 scripts/benchmark_search.py target/search-smoke
# Explicit Global timing, separate allocation survey, preserved raw evidence.
python3 scripts/benchmark_search.py target/search-timed --timed --samples 5 --minimum-ms 100
# Validate a completed campaign without rerunning its benchmark binaries.
python3 scripts/benchmark_search.py target/search-timed --timed --samples 5 --minimum-ms 100 --validate-only
```

## Detailed measured rows

All 27 rows agree on every non-timing record across 14 declared backend configurations.
Every row compares full fresh/reused Global output and independently decodes the complete source.
Timed rows use only Global, including reset, dictionary seeding, history rebuilding and process/drain control.
After each elapsed batch ends, its final output is compared with the verified reference.
Caller buffers, sources, parsed dictionaries, full decoding/byte comparisons, fingerprints and owned-tracker surveys stay outside clocks. API-driver bounds/progress assertions remain inside measured work.
Cross-process FNV64 fingerprints are noncryptographic supplemental evidence, not a C throughput comparison.
Individual unsafe-feature builds may affect only untimed decoding or paths absent from these encoder workloads.
Do not attribute every measured difference to an active optimized encoder accessor; existing backend microbench evidence is separate.

Profiles are complete reproducible experimental controls, not recommended user presets or universal speedups.
Configurations run in the recorded fixed order. Sample bands include host scheduling and thermal noise; they are not a significance test.
Cost and throughput medians are computed independently from their individual sample values, including even sample counts.
History uses two distinct 128-KiB sources as A/B/A under a 1-MiB wire window; dictionary input uses 64 contiguous 512-byte runs.
TrackingAllocator wraps Global. Its separate Arc/Mutex/ledger bookkeeping and caller buffers are outside codec payload counts.
Public total retained capacity includes the independently Global-owned dictionary; its measured clean-context contribution
is recorded separately and checked against tracked live bytes. Search capacity is the narrower subset of owned storage.
Peaks are observable live payload high-water marks, not Global's transient resize overlap or total process memory.

| Workload | Profile | Policy | Frame bytes | Ratio | Search capacity | Total capacity | Cold / reused attempts | Median ns/B [min,max] |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| block | default | greedy | 3810 | 0.116272 | 655360 | 1295290 | 145 / 251 | 11.891 [11.715,13.602] |
| block | default | chain | 2773 | 0.084625 | 9568256 | 10164400 | 136 / 241 | 14.742 [14.566,14.983] |
| block | default | lazy | 2778 | 0.084778 | 9568256 | 10095055 | 135 / 239 | 17.648 [17.215,18.083] |
| block | compact | greedy | 6843 | 0.208832 | 16384 | 832182 | 148 / 255 | 15.417 [14.998,15.592] |
| block | compact | chain | 14381 | 0.438873 | 57344 | 777862 | 145 / 247 | 26.595 [25.895,26.789] |
| block | compact | lazy | 14291 | 0.436127 | 57344 | 774902 | 144 / 245 | 27.009 [26.885,38.411] |
| block | wide | greedy | 2879 | 0.087860 | 3145728 | 3678267 | 132 / 234 | 11.088 [11.070,11.191] |
| block | wide | chain | 2901 | 0.088531 | 22020096 | 22552397 | 132 / 233 | 17.666 [17.019,18.255] |
| block | wide | lazy | 2901 | 0.088531 | 22020096 | 22552397 | 132 / 233 | 17.796 [17.720,17.924] |
| history | default | greedy | 393235 | 1.000048 | 655360 | 1748379 | 111 / 187 | 23.392 [23.290,23.551] |
| history | default | chain | 393235 | 1.000048 | 9568256 | 10670264 | 172 / 303 | 53.878 [53.213,57.743] |
| history | default | lazy | 393235 | 1.000048 | 9568256 | 10670264 | 172 / 303 | 56.302 [56.059,64.766] |
| history | compact | greedy | 393235 | 1.000048 | 16384 | 1064977 | 46 / 82 | 11.542 [11.512,11.763] |
| history | compact | chain | 393235 | 1.000048 | 57344 | 1105937 | 47 / 83 | 19.150 [19.088,19.347] |
| history | compact | lazy | 393235 | 1.000048 | 57344 | 1105937 | 47 / 83 | 18.978 [18.751,20.333] |
| history | wide | greedy | 262174 | 0.666743 | 3145728 | 4234374 | 53 / 80 | 29.388 [29.325,30.342] |
| history | wide | chain | 262173 | 0.666740 | 22020096 | 23108742 | 54 / 81 | 36.791 [36.099,37.049] |
| history | wide | lazy | 262173 | 0.666740 | 22020096 | 23108742 | 54 / 81 | 36.234 [36.142,41.662] |
| dictionary | default | greedy | 261 | 0.007965 | 1179648 | 1849004 | 103 / 176 | 9.311 [9.149,9.451] |
| dictionary | default | chain | 239 | 0.007294 | 12713984 | 13378424 | 78 / 124 | 13.521 [13.490,13.826] |
| dictionary | default | lazy | 226 | 0.006897 | 12713984 | 13377893 | 92 / 152 | 17.787 [17.397,18.870] |
| dictionary | compact | greedy | 781 | 0.023834 | 24576 | 701029 | 112 / 193 | 10.104 [9.950,10.268] |
| dictionary | compact | chain | 11193 | 0.341583 | 106496 | 784914 | 77 / 123 | 15.537 [15.479,18.939] |
| dictionary | compact | lazy | 11193 | 0.341583 | 106496 | 784914 | 77 / 123 | 15.153 [15.108,15.194] |
| dictionary | wide | greedy | 258 | 0.007874 | 5242880 | 5912036 | 93 / 156 | 9.690 [9.374,10.015] |
| dictionary | wide | chain | 224 | 0.006836 | 28311552 | 28974699 | 75 / 118 | 21.044 [20.079,22.038] |
| dictionary | wide | lazy | 214 | 0.006531 | 28311552 | 28965902 | 52 / 72 | 25.077 [24.511,25.713] |

The table shows safe-backend rows. All configurations, raw samples, complete surveys and fingerprints remain in JSONL/results.json.
backend-comparison.tsv preserves every backend's timing bands and row-specific safe/selected median ratio.
A ratio above 1 means lower measured median cost for that row only. No aggregate or universal speedup is inferred.
