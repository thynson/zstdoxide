# Private policy-bound frequency measurements

This harness measures frequency state lifecycles, not Ultra compression or frames.
Public Ultra was still unsupported at this private measurement checkpoint.
Current bounded public support is described in `reference-ultra-public-qualification.md`.
The native fixture, independent model and implementation qualification are
described in `reference-ultra-validation.md`.

## Fixed workloads and measured region

Twelve schedules pair Optimal and Ultra pricing for six source-only cases:
single-symbol initialization, a full cyclic source block, 8192 symbol updates,
disabled-literal symbol queries, grouped dirty updates and the last permitted
fractional literal sum. Every source byte, operation and parameter comes from the
unchanged audited fixture. All 298 fixture cases, 4065 complete state transitions
and 2848 query results are checked before any timed sample.

One measured iteration constructs a Global-owned frequency table, executes the
exact schedule and queries, copies the final snapshot into a preallocated result
buffer and drops the owner. All final snapshots are compared outside clocks with
the previously qualified complete state. Fixture decoding, intermediate state
comparisons, owner monitoring and output formatting remain outside timing.
These numbers include allocation and result-buffer copies; they do not isolate
the checked cell-access operations selected by the unsafe feature.

Each schedule has three untimed warmups and nine samples. Heavy 8192-update and
fractional-boundary schedules use eight iterations per sample; the others use 128.
The executable defaults to a clock-free run; only `--timed` enables timing.

## Clock-free qualification

Safe, isolated `unsafe-optimal-frequencies` and all-feature release executables
pass complete fixture replay and all twelve exact borrowed non-cloning owner
checks. Each owner makes one allocation requesting 1508 bytes, never grows or
shrinks, releases the allocation and drops exactly one allocator identity.
A separate Python model independently reconstructs every selected final state;
ordinary and optimized Python each reject twelve forged state/owner transcripts.

Strict all-target all-feature Clippy passes. The initial duplicated-attribute
failure and source are retained with the corrected source and final log.
`evidence/2026-10-16-ultra-frequency-benchmark-harness.tar.gz` preserves the three
executables, exact build commands, toolchain/environment, independent expectations,
complete transcripts and forged controls. Timing is gated on completion of the
source-identical feature matrix, Miri and no-atomics jobs. General host idle state
is not inferred from those known task-worker barriers.

## Timed campaigns and independent repeat

The complete feature matrix, 28 Miri executions and four no-atomics checks finished
before timing. The primary campaign runs safe, frequency-only unsafe and all
features sequentially; the independent repeat reverses the safe/all order.
All five measured runs use the corresponding clock-free executable hashes. Every final state and owner record matches the independent model.
Ordinary and optimized Python audit all 540 samples and each reject 16 forged
timing transcripts. The two campaigns retain 60 case/backend runs.

The equal-case geometric mean of median latency ratios is 0.981911 for isolated
frequency unsafe/safe, 0.992021 for all/safe, and 0.998121 for the independent
all/safe repeat. Differences are small and vary by workload. This does not support
a stable or universal unsafe speedup claim, and is not a compression throughput
measurement. Raw samples and source/binary/barrier identities are retained in
`evidence/2026-10-16-ultra-frequency-timed.tar.gz`.

| Workload | Policy | Safe median (microseconds/lifecycle) | Frequency unsafe/safe | All/safe | Repeat all/safe |
| --- | --- | ---: | ---: | ---: | ---: |
| init_single_9_m0 | 0 | 1.616 | 1.0312 | 1.0125 | 1.0000 |
| init_cycle_131072_m0 | 0 | 612.495 | 1.0108 | 1.0094 | 0.9956 |
| symbols_n9_m1_2 | 0 | 1.465 | 0.9829 | 1.0189 | 0.9070 |
| counts_8192_m0 | 0 | 453.406 | 0.9885 | 1.0171 | 1.0181 |
| grouped_dirty_cache | 0 | 227.044 | 0.9222 | 0.9568 | 1.0198 |
| fraction_boundary_30 | 0 | 7439.912 | 1.0268 | 1.0224 | 0.9857 |
| init_single_9_m0 | 2 | 1.643 | 1.0210 | 1.0046 | 0.9976 |
| init_cycle_131072_m0 | 2 | 615.192 | 1.0085 | 1.0321 | 0.9994 |
| symbols_n9_m1_2 | 2 | 1.555 | 0.9822 | 0.9535 | 0.9671 |
| counts_8192_m0 | 2 | 495.786 | 0.9263 | 0.9690 | 1.0199 |
| grouped_dirty_cache | 2 | 228.178 | 0.9466 | 0.9614 | 1.0162 |
| fraction_boundary_30 | 2 | 8006.193 | 0.9444 | 0.9520 | 1.0586 |
