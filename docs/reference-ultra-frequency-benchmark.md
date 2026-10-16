# Private policy-bound frequency measurements

This harness measures frequency state lifecycles, not Ultra compression or frames.
The public Ultra strategy remains unsupported. The native fixture, independent
model and implementation qualification are described in `reference-ultra-validation.md`.

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
