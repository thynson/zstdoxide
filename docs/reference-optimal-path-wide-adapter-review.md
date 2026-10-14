# Wider scripted path adapter review

The original small scripted observer stays frozen. The wider adapter retains its
source digest and an exact generated unified diff. Changes are limited to the
protocol identifier, source/packet/event/sequence capacities, the separate
initialized previous-path array, the declared path extent, and canonical delta
serialization. The same fifteen instrumentation substitutions preserve the same
upstream parser statements. No match/path/price/repeat algorithm is changed.

The larger capacities are 8,192 source bytes, 12,828 packet bytes, 16,384 events
per padding pass, and 2,731 sequence cells. The last exceeds floor(8192/3).
Both current and previous path owners have 4,099 fully initialized cells.
The current visible extent is min(source_length+3,4099); inactive current cells
remain zero. The prior snapshot resets to zero for each padding pass and changes
only when an emitted event records a different complete cell. Serialization
compares initialized complete structs and stores each changed cell once, in
increasing index order. All owner guards and source/literal checks are retained.
The compile-time total-storage assertion includes the new prior-state owner and
still requires less than 4 MiB.

Positive stdout is bounded to 128 MiB per command, stderr to 1 MiB, command time
to 120 seconds and total capture logs to 512 MiB. The offline line parser permits
at most 512 KiB for the initial seeded path delta. Source-only model traces prove
actual queries and exits on either side of the 4096 horizon before C execution.
All seventeen small recipes reconstruct every prior complete state exactly.

The wider auditor caches only immutable canonical expected strings keyed by an
immutable request. It never caches mutable path cells, trusts self-reported
prices, or runs an observed executable. It recomputes source identities and the
exact generated parser/adapter diff. Normal and fully instrumented traces must
match across both padding passes. New evidence does not replace the small capture
and does not qualify ordinary CCtx startup or public Optimal admission.
