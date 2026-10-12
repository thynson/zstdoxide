# Bounded Fast measurements

## Harness and timing contract

The benchmark uses the public one-shot request API on nine fixed cases:
32 KiB skewed literals with copies, 384 KiB A/B/A history, and four mixed
128 KiB blocks, each with level 1, level -7, and an explicit
windowLog17/hashLog14/minMatch4 Fast request. Known-size adjustment reduces the
small source's windowLog to 15. The independent Python generator fixes each
complete input's SHA-256 and FNV-1a identity before starting any Rust process.

The timed region includes Global-backed request resolution, workspace/output
allocation, compression and destruction of the returned frame. Two untimed
calls establish warmup and deterministic complete output. Each of five samples
runs for at least 100 ms, followed by a separate full-output equality check.
Decoding, source/frame hashing, file output, formatting and a separate owned
TrackingAllocator survey stay outside the clocks. The survey includes the
returned frame's residency, peak requested/returned bytes, positive allocation
and growth counts, distinct owner totals, and complete final release. These
reported peaks do not include allocator-internal overhead or transient old/new
storage overlap during resizing.

All configurations must use identical source/request rows and complete encoded
bytes. Saved frame files are compared directly, with additional SHA-256/length
bindings. Feature switches for structures absent from the encoder may affect
untimed verification or no exercised path; no individual speedup is inferred
just from enabling such a flag. No C speed or compression-ratio parity is claimed.

Build every selected configuration before starting sequential measurements.
Stop other task-owned builds/tests/Miri before timing. The evidence records
compiler and host, effective environment and Cargo configs, source/executable
hashes, ordered commands, raw durations/iterations, full frame identities and
allocation counters. Offline validation rejects changed sources, commands,
metadata, frames or recomputed summaries.

## Verified harness checkpoint

The final clock-free smoke passes all nine cases for safe and all-feature
builds. Complete encoded bytes and allocation surveys agree. Independent review
and 70 final normal/optimized-Python controls pass (two positive validations and
68 intentional rejections). Earlier diagnostics are preserved: the original
small-source expected parameter record used the wrong reference tier, and an
initial evidence validator accepted JSON false as exit code zero. Corrections
use the pinned C preset source and strict typed metadata; source recipes and
timing boundaries remain unchanged. Strict example Clippy and formatting pass.

See the [harness evidence](evidence/2026-10-09-fast-benchmark-harness.json) and
[55-member archive](evidence/2026-10-09-fast-benchmark-harness.tar.gz). This
checkpoint contains no timing measurements. The separately verified campaigns
below retain their later execution dates and results.

## Measured campaigns on 2026-10-12

The full campaign completed all fourteen builds before its sequential runs:
126 case records and 630 samples. A subsequent safe/all diagnostic used the
same sources, requests and **identical executable hashes** and added eighteen
records and ninety samples. All 144 complete frames and allocation surveys
match the permanent pre-DoubleFast baseline. Four normal/optimized-Python
campaign replays pass; summary medians were also recomputed separately from
the raw elapsed/iteration records. No output expectation was refreshed.

The host reports macOS 15.7.7, aarch64-apple-darwin. The pinned compiler is
nightly-2026-10-08, rustc 1.101.0-nightly (1d81eb4ad). Before each campaign,
process checks found no active project test/compiler; two unrelated streaming
test invocations had previously delayed the first campaign and exited without
intervention. No other work from this task ran during clocks. This desktop host
was not isolated from unrelated system activity, frequency changes or scheduler
placement, and configuration order was fixed rather than randomized.

**The timing evidence does not establish a repeatable unsafe speedup.** The
full campaign's safe/all median-cost ratio ranges from 1.215 to 1.850, while the
short repeat ranges from 0.789 to 1.028 and reverses most apparent gains. The
ratio is safe ns/byte divided by all-feature ns/byte; greater than one means
all features were faster in that observation. Individual flags also vary,
including flags whose structures are absent from this compression path.
These sequential observations cannot assign that variation causally to an
unsafe backend. Both campaigns are retained without selecting only favorable
samples; there is no basis here for changing the safe default.

| Workload / request | Full safe MiB/s | Full all MiB/s | Repeat safe MiB/s | Repeat all MiB/s |
| --- | ---: | ---: | ---: | ---: |
| skewed / level-1 | 93.87 | 128.08 | 115.68 | 107.82 |
| skewed / level-minus-7 | 140.71 | 202.99 | 187.87 | 175.02 |
| skewed / window-17 | 91.92 | 116.50 | 117.80 | 92.91 |
| history / level-1 | 260.88 | 317.05 | 357.01 | 290.15 |
| history / level-minus-7 | 447.84 | 665.88 | 702.57 | 644.49 |
| history / window-17 | 137.90 | 233.84 | 258.22 | 237.65 |
| mixed / level-1 | 195.64 | 361.96 | 343.35 | 338.76 |
| mixed / level-minus-7 | 302.61 | 531.39 | 482.76 | 487.00 |
| mixed / window-17 | 222.03 | 304.19 | 299.70 | 308.13 |

The deterministic size/allocation observations are stable across both campaigns
and every tested backend. Peak values below are allocator-visible returned
bytes, with the returned frame resident, under the survey scope above.

| Workload / request | Source bytes | Frame bytes | Peak bytes | Allocations / grows |
| --- | ---: | ---: | ---: | ---: |
| skewed / level-1 | 32,768 | 3,149 | 602,041 | 117 / 20 |
| skewed / level-minus-7 | 32,768 | 6,191 | 505,472 | 105 / 11 |
| skewed / window-17 | 32,768 | 3,149 | 667,577 | 117 / 20 |
| history / level-1 | 393,216 | 262,172 | 1,482,103 | 44 / 6 |
| history / level-minus-7 | 393,216 | 262,172 | 1,416,567 | 24 / 2 |
| history / window-17 | 393,216 | 393,235 | 1,447,965 | 37 / 6 |
| mixed / level-1 | 524,288 | 135,514 | 1,987,978 | 224 / 40 |
| mixed / level-minus-7 | 524,288 | 151,682 | 1,959,587 | 192 / 29 |
| mixed / window-17 | 524,288 | 138,221 | 1,944,060 | 224 / 37 |

See the [full raw evidence](evidence/2026-10-12-fast-measurements.json),
[repeat raw evidence](evidence/2026-10-12-fast-measurements-diagnostic.json), and
[separate reconciliation](evidence/2026-10-12-fast-measurements-review.json).
The [full archive](evidence/2026-10-12-fast-measurements.tar.gz) and
[repeat archive](evidence/2026-10-12-fast-measurements-diagnostic.tar.gz) retain
all measured source files, commands/environment, compiler/executable hashes,
raw sample logs, offline validations and process-barrier records. Each stores
seven unique complete frames plus a map for every original frame path;
every mapped byte extent was verified. Build artifacts and executable copies
are omitted, while their identities and reproduction commands remain.

## Reproduction

Each execution requires a fresh external output directory. The full default
configuration order is safe, each of the twelve individual unsafe features,
and all features; `--configs safe,all` selects only a short smoke comparison.

```sh
python3 scripts/benchmark_fast.py /tmp/new-fast-smoke --configs safe,all
python3 scripts/benchmark_fast.py /tmp/new-fast-measurements --timed
python3 scripts/benchmark_fast.py /tmp/new-fast-measurements --validate-only
python3 -O scripts/benchmark_fast.py /tmp/new-fast-measurements --validate-only
```

Raw results and complete frame files remain alongside `evidence.json`; target
executables may be omitted from an archive if their recorded hashes and build
commands are retained. Offline replay still verifies an executable's bytes
when its original path remains available. The default Rust example starts no
clock unless `ZSTDOXIDE_FAST_BENCH_TIMED=1` is explicitly supplied.
