# Bounded reference binary-tree measurements

The new harness uses 24 deterministic tree cases: four immutable historical
corpora crossed with six explicit W/H/C/S profiles. Profiles cover small tables,
maximum admitted owners, search log 8, a short window, a tiny wrapping node ring,
and an ignored target-length equivalence. `tree-wide` and `tree-target` must
produce identical complete frames and exact allocator records. Public tuples
are independently adjusted from explicit scalars and known source lengths.

The previous eighty Fast/DoubleFast/Greedy/Lazy/Lazy2 frame and qualified owner
baselines remain permanent full-matrix tests. Their historical timing campaigns
are unchanged; this campaign measures only tree strategy 6. No row-table timing
is relabeled as tree behavior.

Each executable's default mode is clock-free. Build every requested backend
before running any measurements. Record all 15 independent unsafe leaves plus
safe and all features. Complete output bytes, decompression, deterministic
source fingerprints and allocator surveys are checked outside clocks. Timing
covers global-allocation one-shot compression and output destruction, with five
samples of at least 100 ms per case. Run a separate safe/all repeat only after
completion barriers for all task-owned correctness, platform and clock-free jobs.
The barrier does not prove absence of unrelated desktop load.

No measurements have been qualified at this preparation checkpoint. Results,
raw samples, exact frames/owners, compiler/feature identities and limitations
will be recorded separately. An inactive feature's timing difference does not
establish a causal advantage, and no universal unsafe speedup is presumed.

## Harness qualification

The safe/all clock-free harness verifies 24 cases per configuration and exact
frame/allocator equality. Normal and optimized offline controls each accept two
unaltered captures and reject 42 modified records with zero child execution.
These include source/tuple changes, command/feature/environment changes, exact
owner accounting, target-equivalence frames/owners and bounds. They do not prove
an arbitrary coherently replaced tree frame/owner set against an external frozen
tree baseline; runtime decompression and deterministic replay remain distinct.

The initial driver rejected its independent expectation that requested target
zero meant applied target zero. The pinned level-9 preset instead supplies 8 for
the small benchmark source and 16 for larger sources. Versioned
`benchmark_tree_v2.py` preserves the original driver and changes only that tuple
expectation plus its own source identity. Production code and benchmark requests
are unchanged. All initial logs, frames and the failed record are retained.
Legacy progress messages inherited from the prior harness say Greedy/thirty-two;
structured metadata and the checked inventory describe exactly 24 tree cases.
Evidence is in `evidence/2026-10-13-tree-benchmark-harness.json` and its archive.

## Complete clock-free backend campaign

All seventeen configurations (safe, fifteen individual unsafe leaves, all)
reproduce every complete frame and exact qualified allocator record from the
initial v2 smoke campaign. This checks 408 tree cases and four target-equivalence
pairs per configuration with no sample clocks. Both normal and optimized offline
replays pass. Exact source, executable identities, outputs and records are in
`evidence/2026-10-13-tree-clock-free.json` and its read-back-verified archive.
Timing remains pending the final correctness and task-owned CPU barrier.

## Qualified measurements and independent repeat

The October 14 campaigns retain 2,280 samples over 456 case/configuration runs:
17 backends times24cases in the first campaign, then an independent safe/all
repeat. Each case has five samples of at least100ms. All456complete output frames
and exact allocator records match the earlier clock-free baseline. Every build
finishes before timing starts, and both campaigns have observed task-owned CPU
barriers after correctness/platform/clock-free completion. Optimized offline
replay passes; normal/optimized controls each accept two positives and reject
22 timing/barrier forgeries with no child execution.

Ratios below are all-features throughput divided by safe throughput. Each ratio
uses a case's median; geometric means weight cases equally.

| Scope | First geometric mean | First range | Repeat geometric mean | Repeat range |
| --- | ---: | ---: | ---: | ---: |
| All 24 tree cases | 0.762 | 0.560–1.109 | 1.224 | 0.777–1.762 |
| tree-deep | 0.751 | 0.560–1.024 | 1.172 | 1.059–1.266 |
| tree-short-window | 0.739 | 0.603–0.889 | 1.195 | 0.970–1.334 |
| tree-small | 0.905 | 0.759–1.109 | 1.335 | 0.898–1.762 |
| tree-target | 0.726 | 0.570–0.805 | 1.166 | 0.777–1.610 |
| tree-tiny-ring | 0.681 | 0.634–0.798 | 1.202 | 1.046–1.363 |
| tree-wide | 0.791 | 0.599–1.020 | 1.282 | 1.065–1.650 |

The aggregate direction reverses between runs (0.762 first,1.224 repeat). These
data do not establish a stable throughput benefit for the unsafe implementations.
Fixed configuration order, uncontrolled unrelated desktop activity, clocks and
thermal state limit interpretation; no specific cause for the difference was
isolated. Unused-feature differences are not causal evidence. No C throughput,
universal speedup or native foreign-platform performance is claimed.

The preserved timing checkout at4df8609 matches all95clock-free source identities.
Later additive/private Optimal code is excluded from these measurements. Exact
samples, commands, environments, source, frames and owners are retained in
`evidence/2026-10-14-tree-measurements.json` and its50,023,560-byte lossless archive;
all reconstructed files were checked. The companion CSV has all24case-level
ratios, medians, frame sizes and peak requested bytes. Compiled intermediates are
omitted with executable/build identities retained.
