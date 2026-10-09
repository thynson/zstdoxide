# Parameter and encoder lifecycle validation

Module 10 is in progress. Each checkpoint below identifies its implementation
and evidence boundaries; the complete feature/width matrix and official test
migration update remain pending.

## Frozen reference observations

Commit `c30ede1` imports 934 observations from pinned facebook/zstd v1.5.7.
The [fixture README](../tests/upstream/parameters/README.md) distinguishes
public and private adapters, signed clamping, modular-size domains, 64-bit
execution, macro-derived 32-bit bounds, and different context protocols.
[Oracle evidence](evidence/2026-10-09-parameter-oracle.json) records exact
regeneration and provenance checks. This imported data is not itself a Rust
behavioral test or a completed upstream migration.

## Value records and scalar validation

The first Rust checkpoint implements six plain safe value records, concrete
validation, reference strategy conversion, request getters and checked setters,
and reference frame defaults. It introduces no allocations or unsafe operations.
It does not yet implement preset selection, adjustment, or query resolution.

Thirteen focused tests pass under safe, all-feature, no_std safe and no_std with
all twelve unsafe flags: 52 passing executions with no ignores. Both no_std
library checks, strict safe/all-target and all-feature Clippy, owned-file
formatting and diff checks pass. Two standalone usage examples pass under safe
and all-feature builds. The frozen owned source and all ten focused command
logs are preserved in [record evidence](evidence/2026-10-09-parameter-records.json).

The replay checks all 225 recorded raw inputs against C validation results,
without claiming their adjustment outputs. The request trace covers 119 scalar
state rows: 72 actual typed setter calls, 25 unrepresentable-input rejections,
and 22 initialization/reset rows. Thirty C frame-flag rows and one unsupported
parameter ID remain outside this request surface and are explicitly counted.
All 266 query rows supply reference frame-default assertions only at this stage.
Target-width bounds are checked against the labeled fixture records; no foreign
target execution is claimed by this checkpoint.
