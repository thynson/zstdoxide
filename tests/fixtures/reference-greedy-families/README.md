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

The subsequent normal/ASan-UBSan capture preserves these exact recipes.
`capture.json` and `observations.ndjson` freeze 139 complete observation groups
(137 direct cases and two fresh-context groups), totaling 5,104,709 bytes. Every
direct output matches across C modes. All 26 linked C units plus the helper
are instrumented in the sanitized build, retaining upstream per-function
sanitizer exclusions; opaque fresh-context memory peaks remain mode-qualified.
The permanent checker is `python3 scripts/check_greedy_family_capture.py
--check-fixture`; a capture-directory argument independently audits all 444
commands, inputs, compile/link arguments, source identities and complete maps.

The [validation report](../../../docs/reference-greedy-validation.md) distinguishes
observed consequences from hypotheses. Both branches emit a 64-byte match at
2 MiB distance; distances 1023/1024 are accepted at W10 and 1025 is rejected.
Distinct colliding keys separate C6/C7 chain traversal. Row widths 16/32/64 show
reserved-head wrap and select the newest equal-length candidate. Row pending
gaps 383/384 produce 384/385 changed position cells, whereas gaps 385 and above
produce 129. The ordinary cross-block wrapper is not called by these inputs.

Period-four source sizes 65541..65544 bracket the long match-length marker:
expanded ML 65538 has no marker and ML 65539 does. All three long-literal recipes
produce LL above 65535 (65760/65795/65796); they do not bracket the exact LL
cutoff. Both branches finish with skipping enabled at 4096 noise bytes and
clear it on the later periodic match. These are final state consequences, not
a trace of every internal probe or proof of an exact attempt count. No input
was silently changed to improve its coverage. Invalid cyclic/future table cells
remain separate Rust leaf tests because C inputs allow valid construction only.

The observations are test oracles, not Rust parser output, ordinary frame
trajectories, public Greedy support or performance measurements. Preserve them
and add explicitly versioned witnesses when a new boundary needs coverage.
