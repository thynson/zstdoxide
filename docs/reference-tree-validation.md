# Bounded reference BinaryTreeLazy2 preparation

BinaryTreeLazy2 remains unsupported. Module 15 production inputs are frozen
while its correctness and timing closure runs. This document records additive
preparation only; it does not change the scope of that immutable matrix.

## Permanent predecessor baseline

The [baseline evidence](evidence/2026-10-13-lazy-baseline.json) preserves 80
Lazy/Lazy2/Greedy/DoubleFast/Fast cases from the completed sixteen-configuration
clock-free campaign. Its 36 distinct complete frames occupy 22,819,999 bytes.
The separate offline checker reconstructs every frame and allocator record
from the pinned archive, without importing a codec or executing archived code.
Two Python modes each accept the original/restored fixture and reject 15
mutations with no child processes.

The new `reference_lazy_defaults` target passes all 21 tests separately in safe,
unsafe-reference-chain, unsafe-row-table and all-feature builds: 84 executions.
It checks deterministic source fingerprints, resolved tuples, complete frame
bytes, standard frame extents, decoder reconstruction, exact qualified allocator
records, unchanged request/source and final owner release. Exact layout/byte
accounting is qualified to the recorded aarch64 macOS compiler; other targets
still enforce output and owner/category invariants. Focused safe/all Clippy and
formatting pass. These additive tests were created after the Module 15 matrix
snapshot and are not retroactively counted in it. Earlier baselines are intact.
