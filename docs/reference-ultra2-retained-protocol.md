# Retained Ultra2 phase/block model

`ZU2RT01` is an additive private observation protocol. Its active independent
model is `scripts/oracles/ultra2_retained_validation_v4.py`; it does not call C,
Rust or a codec executable. Pinned algorithm attribution and ownership rules
are in `reference-ultra2-retained-design.md` and `data-structures.md`.

## Packet and private scope

An eight-byte `ZU2RT01\0` magic is followed by eleven little-endian `u32` values:
source length, block count, bootstrap flag, price policy, window log, hash log,
chain log, search log, minimum match, target and literal mode. The block lengths
follow, then exactly the source bytes. The fixed header is 52 bytes; the maximum
packet extent is `52 + 4*4 + 32768 = 32836` bytes. No trailing bytes, implicit
padding or mutable source objects are admitted.

The model accepts one to four complete blocks. One block is exclusively a
fresh-predecessor reconciliation control, bounded to 7–16384 bytes. Two to four
blocks are bounded individually to 7–8192 bytes and in total to 32768 bytes.
All source bytes must fit the resolved window. Window logs are 10–16, hash/chain
logs 6–12, search logs 1–6, minimum match 3–7, targets 0–4095 and literal mode
0/1. Integer fields reject booleans. Policy is 0 or 2; bootstrap requires policy 2.

Bootstrap-disabled packets express an opaque first-entry sequence-prefix
hypothesis for compatibility with prior observations. They are not another
public mode. The planned new native frame observer accepts only bootstrap-enabled
policy-2 requests with two to four blocks. It will execute the actual Ultra2
wrapper, without fabricating a gate blocker or adding an opaque prefix to a
wire frame. Existing captured single-block and ordinary retained transcripts
serve as independent predecessor evidence for the broader model controls.

## Complete observations

Each record identifies its actual block and algorithm phase independently of
its padding-pass number. The transport contains metadata, block begin,
eligibility, pass begin, complete sparse parser/search events, pass end,
optional first-block reset, actual block publication and a final trailer.
Eligibility is observed after `ZSTD_buildSeqStore` applies its insertion
catch-up, matching the wrapper's actual entry point.

Event coordinates and sequence anchors are physical offsets in the complete
source. Table cells, insertion frontiers and lifecycle origins retain native
logical indices, including the constant two-byte bias. Rebase adds exactly the
first block length. Sparse path/search baselines continue through the discarded
pass, rebase and later blocks; omitting dormant cells or clearing the baselines
would change the observation contract.

The model borrows only the currently consumed source prefix through checked
integer translation. Stale logical indices below the origin and positive reads
past that prefix reject. Future source mutations cannot affect earlier records.
Candidate repeats from the discarded pass do not become caller repeats.
Search/price history survives actual raw/RLE blocks, while wire-state ownership
changes only on compressed publication. Publication kinds remain explicit
hypotheses; this model does not predict entropy coder size selection.

The one-block compatibility emitter retains its established 65536-event cap.
Each actual retained algorithm pass retains the established 32768-event cap.
Every output line is bounded by 2 MiB and a complete observation by 256 MiB.
The checked native capture plan must additionally freeze source-derived bounds
and total campaign output before execution.

## Qualification status

The active v4 model passes ordinary and optimized Python qualification. Each
mode rejects 83 malformed/forged controls with no child execution and reconciles
all 53 fresh Ultra2 and 42 retained policy-0/policy-2 native cases. All
1,204,582 observed event records agree under the declared tag-only projections;
190 identical normal/sanitized transcripts are anchored to their committed
native archives and the completed Module 21 closure. These are existing native
observations replayed by the new model, not new native codec executions. The
initial complete-command run reached its 1800-second bound after 78 fully
compared ordinary cases and 77 optimized cases. An additive continuation preserves
those exact source-bound success prefixes and completes the remaining 17/18 cases in
individually bounded workers. Inherited cases are not reported as rerun. Three earlier
executed model versions remain immutable: the first two rejected a large fresh
predecessor under the smaller retained event cap; the third attempted an invalid
cross-class `super()` emitter call. The active emitter explicitly implements
both bounded geometries. These failures are model-adaptation evidence, not
failures of the production Rust codec. No retained Ultra2 public admission or
successor native parity is claimed at this stage.


The complete source-bound record, both Python modes and all earlier failed model
stages are in `evidence/2026-10-22-ultra2-retained-model.{json,tar.gz}`. The archive
was independently reconstructed and all original files rehashed. Archive SHA-256:
`a7f05fac7377b7372d2bef893986328d50cc6400ad6ae0c809fe77bf6694f69e`.
