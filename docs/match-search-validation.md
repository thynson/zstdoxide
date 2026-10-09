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

## Validated options and storage shape

`MatchSearchOptions` now validates each concrete scalar setting without
allocation; unsuccessful setters preserve the complete record. The private
`SearchKeyLayout` validates shifts and key dimensions, while `SearchShape`
computes effective history, policy effort and all cached requested bytes using
the actual target layouts. The aggregate admission ceiling is 32 MiB of requested
payload. It is not a peak-memory, retained-capacity or allocator-overhead limit.
An empty one-shot frame resolves to zero history. No live setting is exported
or applied at this foundation checkpoint.

Seven focused tests pass in safe, all-feature, no_std safe and no_std with all
twelve unsafe features. Both no_std library checks, strict library/all-target
Clippy with all features, and owned-file formatting pass. All seven tests also
pass under strict-provenance Miri on i686 and big-endian s390x using cached
sysroots. These fourteen interpreted executions validate shared safe scalar
arithmetic and target layouts, not native foreign-platform or unsafe-backend
execution. The checkpoint adds no unsafe operations or allocations.

[Record evidence](evidence/2026-10-09-match-search-records.json) embeds sixteen
command logs and the frozen source hashes. It retains the initial all-target
Clippy failure caused by concurrent baseline-test compilation, and the final
passing run after that test owner repaired the errors. Search owner resizing,
hash primitives and actual parser/context consumers remain later items.

## Offline default-frame replay and archive audit

Five regression/schema tests and the two existing RDG helper tests now replay
every captured default frame. Each of safe, all-feature, no_std safe and no_std
with all twelve unsafe features passes all seven tests: 304 complete current
frames and 608 comparisons with the unchanged original backend records. Inputs
and dictionaries are verified before encoder calls. Output fingerprints, frame
and block metadata, process-call counts, bounded progress and output canaries
match; every current frame decodes completely in Rust. Fresh/reset frames and
the first/final Greedy strategy-cycle frames also match byte for byte. Portable
capacity assertions check reset preservation and reuse of both cache families.
Focused strict Clippy, no_std library compilation and owned formatting pass.

`scripts/check_match_search_baseline.py`, registered in both check runners,
verifies the original archive and every member, reproduction-source attribution,
exact TSV bytes and current RDG/dictionary inputs. It neither extracts the
archive nor regenerates codec expectations. Normal Python and `python3 -O` each
pass one positive isolated audit and twenty expected negative rejections,
including altered TSVs with locally updated hashes and invalid archive members.
The script uses explicit errors, so optimization cannot disable its checks.

[Replay evidence](evidence/2026-10-09-match-search-defaults.json) links all
commands, logs and the isolated mutation reproducer. This checkpoint does not
rerun C: the original archive retains the 152 independent complete C decodes.
Future implementation changes must continue satisfying these saved inputs.

## Exact-width copied keys and hashes

`MatchKey` carries an initialized little-endian word and explicit width, with
no source reference or allocation. Extraction copies only four through eight
selected bytes and returns `None` for a shorter slice. Hashing rejects a width
mismatch before applying the pinned wrapping arithmetic. Four-byte array
conversion preserves existing private callers; keys may be reused at another
hash log only when their width remains compatible. This implementation is
entirely safe and shared by all access backends.

Six tests pass in each of four safe/feature/no_std configurations. Each replays
all 2,880 C vectors, 2,304 unused-byte pairs with full-width controls and 64
historical compatibility guards, with complete tuple/count checks. Strict
library Clippy, no_std compilation and owned formatting pass. Three focused
tests pass under strict-provenance Miri on the host, i686 and s390x: nine
interpreted executions covering exact five-/six-/seven-byte allocations,
unaligned sources, short tails, width identity and frozen endian/wrapping
witnesses. No physical eight-byte source read is required for shorter keys.

[Key evidence](evidence/2026-10-09-match-search-keys.json) embeds fourteen command
logs. The initial i686 automatic-sysroot cache permission failure ran no tests;
the recorded successful foreign-target runs use existing sysroots and an
isolated `f08fceb` foundation with the identical key source. No target installation
or source change was required. These tests establish primitive correctness;
dynamic leaf storage and real policy consumers remain separate checkpoints.

## Dynamic Greedy candidate table

`MatchTable` now owns a checked active layout alongside its positions. Default
constructors retain the old dimensions. Reconfiguration validates and reserves
through the exact owner before clearing entries or publishing metadata;
in-capacity shrink/regrow makes no allocation request. Seed copying checks both
layouts and initialized lengths before mutation. Every safe or unsafe replacement
checks key width and its computed index against live length, including malformed
shortened internal test states with retained spare capacity. Existing block
callers now propagate these checked errors; benchmark inputs remain unchanged.

All 22 recorded commands pass: fourteen leaf tests in six configurations,
three no_std checks, strict all-target Clippy safe/all, two exact release bounds
checks, safe/all default-frame replay and six exact strict-provenance Miri
checks. This totals 106 passing test executions. The new cases cover all 45
hash-log/key-width combinations, seed/key mismatch, grow failures, shrink/regrow,
borrowed non-Clone owners, independent allocation domains and every request in
the selected constructor/two-growth campaign. Default replay still verifies all
76 logical frames per backend against the original records.

[Table evidence](evidence/2026-10-09-match-table-layouts.json) embeds command logs,
the frozen owned source and the integration source hashes shared with the
candidate-chain validation. Concurrent root commits are recorded accurately;
this focused checkpoint does not claim an immutable whole-repository matrix.
