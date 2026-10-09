# Configurable match-search benchmark design

The module checklist separates clock-free harness validation from the final
timed comparison. Complete the harness before the immutable final matrix, then
measure only after source and tests are frozen and concurrent work has stopped.

## Scope

Compare identical deterministic public Encoder workloads across safe, each of
12 individual unsafe features, and all features. Measure Global execution only;
collect owned TrackingAllocator allocation surveys separately outside timing.
TrackingAllocator is a Global-backed bookkeeping wrapper, not another heap.
Do not infer C throughput or universal setting/backend improvements. Individual
features may affect only untimed decoding or no selected encoder path. A timing
difference in those builds does not establish an encoder-access improvement.

## Cases

Use three fixed input/schedule workloads, three complete search profiles, and
Greedy/Chain/Lazy: 27 rows per backend. Every row records all option scalars.

- block: one 32 KiB mixed text/repetition source, one Finish.
- history: two distinct 128 KiB sources in an A/B/A three-block schedule,
  Flush/Flush/Finish, with a 1 MiB wire window. Generate A/B with an explicit
  fixed xorshift recipe.
  This includes repeated history reseeding and larger-horizon retention cost.
- dictionary: 32 KiB deterministic source made from contiguous 512-byte runs at
  fixed offsets in the existing attributed prepared dictionary fixture; borrow a
  dictionary view prepared outside timing. Preserve runs to exercise actual
  dictionary matches rather than scattering individual source bytes.

These complete measurement profiles describe the comparison, rather than
recommended user presets:

| Profile | Hash log | Chain log | Key bytes | Chain visits | Lazy visits | History bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| default | 16 | 18 | 4 | 8 | 16 | 131,072 |
| compact | 10 | 10 | 4 | 1 | 1 | 8,192 |
| wide | 18 | 19 | 8 | 32 | 64 | 1,048,576 |

Use exact same sources, schedules and profile/strategy pairs for all backends.
All contexts use window log 20 and checksums, identical caller chunking. Caller
input/output storage, dictionary parsing, verification and fingerprints are
outside timing. Reused timing resets the context (preserving options), encodes
one complete frame, and black-boxes the result. State explicitly that this
includes context reset, dictionary seeding and per-block history rebuilding.
Ordinary API-driver progress and output-bound checks remain inside measured
work; complete byte comparisons, decoding and fingerprints remain outside.
Compare fresh and reset output bytes before timing. Full Rust decode verifies
input for every row; compare all backend lengths/fingerprints/options/metrics.
Preserve default compatibility tests separately; do not regenerate expectations.

## Evidence and metrics

For each row emit source/dictionary/frame lengths and FNV64 fingerprints,
compressed-size/input-size ratio, exact search_retained_capacity and total
retained_capacity, plus allocation survey snapshots before/cold/reused/drop.
Check zero owner violations and zero final live allocations; accounting must
exclude the separately Global-owned dictionary from tracker totals while public
total retained includes its reachable payload as documented. Sample at least five
independent elapsed batches with minimum duration; emit raw nanoseconds and
iterations, min/median/max ns/input-byte, and throughput. Verify the final output
of each sample after its clock ends. Never time the instrumented survey or
attribute its ledger allocations to codec payload.

Clock-free smoke default and explicit --timed mode. Pin source and input manifest
before the campaign and compare before/after every build/run. Driver must require
the complete 27-row matrix, declared feature set, exact unique row identities, finite
positive timings only in timed mode, complete sample counts and equal non-timing
records across backends. Archive raw JSONL/logs/environment/source manifest and
reproduction code hashes. Independently compute generated input fingerprints
before executing the Rust harness. Bind each parsed result to its canonical,
hashed run artifact, and record or constrain inherited compiler settings.
Negative fixture edits should be rejected by the
parser under normal and optimized Python. No timing while other agents build,
test or run Miri. Report actual host/toolchain and sample bands.
