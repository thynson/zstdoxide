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
checkpoint contains no timing measurements. The full fourteen-configuration
measurement campaign is the next separately validated step.

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
