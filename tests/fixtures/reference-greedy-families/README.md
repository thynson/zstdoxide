# Broader direct Greedy input hypotheses

These 137 source/initial-state recipes are frozen before broader C execution.
There are 63 chain cases and 74 row cases. No expected sequence, final table,
branch-hit assertion or Rust parser output is generated here. All inputs use
the unchanged reviewed `ZGRDY01` helper and its bounded construction grammar.

The source-only families cover:

- Effective widths 3/4/5/6/7, hash logs through 19, asymmetric hash/chain sizes,
  and explicitly forced chain/row branches at window logs 14/15/21. Forcing a
  branch does not test public automatic selection or known-source adjustment.
- Initial prefix lengths 63/64/65 and 127/128/129 with every complete prefix
  key seeded, preserving raw C index bias two in cyclic slots.
- Pending gaps 383/384/385 and 575/576/577. These exercise direct update state;
  they do not invoke the ordinary wrapper's distinct cross-block adjustment.
- Incompressible source tails around 2 KiB and a later periodic region, as
  hypotheses for entering and leaving lazy skipping. Actual final flags and
  sequence/table consequences must come from C, not the recipe names.
- Moving-window floor distances 1023/1024/1025, backward extension at two
  retained floors, periodic repeat distances and long match/literal records.
  Period-four sources include 65541..65544 bytes to bracket the expanded ML
  threshold when C emits the previously observed four initial literals; the
  actual literal/match split still has to be observed for each case.
- A head at the full 2-MiB window distance, deliberately older than the chain
  ring capacity. The first valid head is a distinct contract from following
  cyclic predecessor links.
- Row capacities 15/31/63 around reserved-head wrap, zero tags and sentinels,
  search logs 5/6/7/29, equal-length candidate hypotheses, distinct colliding
  four-byte keys and wrapping salt-entropy state.

Distinct-key collisions are found with bounded independent hash arithmetic;
that search selects inputs and does not implement compression. Initial tables
are still constructed only from strictly increasing seed positions. The fresh
salt `835892b4bb2cae74` is traced to the separately pinned tiny C observation;
explicit zero-salt cases are separate constructed states.

The fixture contains 6,164,630 logical source bytes and 6,192,130 packet bytes
across all recipes. The largest packet is 2,097,372 bytes, within the reviewed
4,325,528-byte envelope. Data is regenerated deterministically from the pinned
recipe source; `recipes.json` preserves every source and packet SHA256, scalar,
seed, repeat and salt. Repeated inputs across profiles remain identified by
full bytes, not merely similar labels.

Run `python3 scripts/check_greedy_family_recipes.py --check`. The offline checker
verifies source identities before import, recreates and round-trips every
packet, verifies the observed salt provenance and rejects 53 malformed inputs
under both normal Python and `-O`. Thirty-seven are the unchanged tiny packet
controls; sixteen additional controls address row state and seeded prefixes.
None executes C or a Rust codec.

Broader normal/instrumented capture, independent record replay and rejection
controls are the next work item. Preserve these inputs and first diagnostics
if a hypothesis fails to exercise its intended behavior. Additional witnesses
must be added explicitly rather than silently changing observed expectations.
