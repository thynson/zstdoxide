# Configurable named match-search validation

Module 11 is in progress. The [design](match-search-design.md) defines the
operational controls and resource boundaries. These checkpoints do not add
numeric reference strategies or complete the upstream test migration.

## Independent default compatibility inputs

The pre-change codec snapshot `ba4529c` produced 76 frames under each of the
safe and all-feature backends. Twelve deterministic workloads cover all three
named policies, fresh and reset contexts, strategy cycling, retained-history
boundaries, dictionary seed thresholds and short Flush crossings. All 152
frames were independently decoded by pinned C with complete byte comparisons.
The original recipes, records and reproduction sources are retained in the
[baseline archive](evidence/2026-10-09-match-search-baseline.json).

The saved output lengths and FNV fingerprints are compatibility regression
inputs. They are distinct from the original complete C byte comparisons.
Historical capacities describe the capture host and include all codec storage
plus the selected dictionary; no search-only capacity is inferred from them.

## Frozen hash primitive observations

The [match-search collection](../tests/upstream/match-search/README.md) calls the
actual pinned C pointer wrappers for key widths four through eight and hash
logs ten through eighteen. It records 2,880 primary observations, 2,304
unused-byte invariance pairs with width-eight sensitivity controls, and 64
historical four-byte arithmetic guards. Every C input has eight initialized
readable bytes. Future Rust extraction must read only the selected key width.

Normal regeneration, two explicit reproduction configurations and ASan/UBSan
reproduction agree byte-for-byte. The importer verifies all 20 included pinned
headers and separates stable provenance from capture compiler/archive metadata.
It rejects an attempted report overwrite and a corrupted isolated fixture even
with Python assertions disabled. Independent root checks confirm that the
generic offline audit rejects fixture mutation, importer mutation and a missing
required collection. The current audit passes 324 files in ten collections;
the oracle runner's shell syntax also passes.

[Oracle evidence](evidence/2026-10-09-match-search-oracle.json) preserves all
sixteen checkpoint/review commands, full logs and reproduction reports. The
recorded C platform is little-endian arm64 macOS with 64-bit `size_t` and
32-bit `int`. This checkpoint runs no Rust hash implementation and promotes no
whole upstream test case. C headers and the linked oracle library remain
outside the repository and are not production dependencies.
