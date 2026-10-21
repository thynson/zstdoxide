# Retained Optimal complete-frame controller review

The active controller is `scripts/benchmark_retained_optimal_frames_v2.py`.
Its Rust example and immutable `ZORB0001` pack are the exact frozen baseline
promoted in the preceding checkpoint. All 21 configurations (safe, each of 19
independent unsafe storage features, and all features) pass the clock-free run.
Each produces all 26 complete expected frames and the same allocation ledger.
There are 546 verified case rows, each with an empty sample array. No performance
measurement or speedup is established by this checkpoint.

## Input and command boundaries

The independent baseline checker verifies bounded regular files, complete pack
consumption, 26 preselected source-only requests, resolved tuples, source SHA-256,
complete frame SHA-256, standard frame headers and balanced block counts. The
pack and provenance hashes bind these observations to the earlier native decode
campaign. FNV values are secondary transport fingerprints, not independent oracles.

The controller inventories every production Rust file, allocator support, Cargo
inputs, its Rust example, the pack/provenance/source recipe and the exact Python
runtime dependency closure. The environment is explicitly whitelisted; Cargo
configuration files, compiler identity, host, build/run arguments, exit status
and both output streams are checked. Every requested executable is built before
any run begins. Each executable hash is checked before and after its invocation.
Complete saved frames must match both the independent baseline and the safe
backend. Allocation counters must have exact integer types, consistent totals,
balanced owners, zero final live allocations and no tracking violations.

Allocation survey, complete-frame comparison, decompression, filesystem output
and post-sample checks occur outside clocks. A timed sample covers global
one-shot compression, output destruction and loop/timer overhead. Five samples
per case each run for at least 100 ms. Summary arithmetic is recomputed from the
raw sample arrays; it cannot substitute for retaining those arrays.

Timed operation requires source-bound completed platform, final public matrix,
native interoperability and complete clock-free receipts. The matrix gate requires
22 full products, 19 isolated features and 21 no_std configurations. The platform
gate requires four interpreted public-entry executions and two concrete codegen
builds. The native gate requires all 1513 requests. A timestamped coordinator
barrier must report no remaining task-owned CPU jobs. This records completion of
our work; it does not prove the absence of unrelated desktop activity.

## Adversarial controls and preserved correction

The original 21-backend campaign passed. Its first control suite had 42 passing
checks out of 43: `wrong-budget` assigned the value 6 to a search-log field already
equal to 6. The resulting unchanged evidence was correctly accepted, so the
negative test itself was ineffective. The entire failed suite, original source,
logs, frames and binaries are retained in
`evidence/2026-10-21-optimal-retained-clockfree-initial.{json,tar.gz}`; SHA-256
`21080d43e2b09764f9a49b1966427eccc8bfd754b428a78d0a2184977789b251`.

The additive v2 runner changes that field from 6 to 7. It also separates the
controller's exact runtime dependencies from unrelated audit scripts. Both
ordinary and optimized Python now pass all 43 checks: 41 rejected forgeries and
two restored positive controls, without subprocess execution. Controls cover
coherent frame/FNV/log/summary tampering, parameter identity, command order and
environments, compiler/executable hashes, boolean counters, allocation leakage,
missing/extra frames and forbidden smoke samples/barriers. Complete frame pins
reject even coherent changes across every backend.

The v2 campaign again passes all 21 configurations. Every metadata/case output
byte and every complete frame agrees with the original campaign; all codec and
Rust example inputs are unchanged. Both full 43-case control suites, 21 actual
executables and complete evidence are retained in
`evidence/2026-10-21-optimal-retained-clockfree.{json,tar.gz}`; SHA-256
`b676cdae098fbb7d4242e3e08efbd298f326e8fce34606682d67f4530b2bcdf0`.
The archive reconstructs 55894 regular files, totaling 351297728 logical bytes.
Only individually hashed redundant build intermediates are omitted. Timing and
its timing-specific controls remain subsequent gates.
