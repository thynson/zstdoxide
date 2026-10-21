# Bounded retained Ultra independent model

Module 21 currently provides a source-only retained policy transport and replay.
It does not implement or admit retained Ultra in production Rust. Module 20's
public retained Optimal implementation and existing fresh Ultra/Ultra2 behavior
remain unchanged. The proposed retained strategy-8 envelope and ownership
requirements are recorded in `data-structures.md`.

The additive `ZURTS01` packet consists of its eight-byte magic, a little-endian
32-bit policy (exactly 0 or 2), and the complete unchanged `ZOPRS03` packet.
Nested source/schedule/geometry validation preserves the independently qualified
128..32768-byte private domain. Boolean policies and mutable nested inputs are
rejected. This protocol does not expose a new public Rust API.

The replay combines the retained-wide initializer and consumed-prefix search
with the qualified Ultra frequency and path rules. It deliberately keeps the
retained initializer instead of inheriting the fresh-only constructor. Frequency,
path and search owners and sparse baselines persist across blocks. Publication
kinds remain explicit hypotheses; only compressed publication commits candidate
repeats. The model does not predict entropy decisions.

Ordinary and optimized Python each pass the same qualification:

- All 19 immutable Module 20 native predecessor cases replay under policy 0.
  Removing only the declared policy/alternative tags and restoring the metadata
  protocol reproduces 134642 events across both padding passes exactly.
- All 38 corresponding normal/sanitized transcripts are bound to identical
  complete hashes in the pinned predecessor archive. Every archive member is
  independently hashed, including records not replayed a second time.
- 104 malformed transport, type, publication, sparse-state and JSON controls
  are rejected. Each forged trace is checked to differ from the valid bytes.
- Ten source-only boundary replays cover policies 0/2 at 128, 16385, 24576,
  24577 and 32768 bytes, with retained block initialization and compressed/RLE
  publication. A separate raw rollback hypothesis is checked. These policy-2
  outputs are not native-qualified observations.
- A child-execution audit hook permits no compiler, codec or subprocess call.
  The 27 loaded source identities and all outcomes agree between modes.

Commands were `python3 -B scripts/check_ultra_retained_model.py` and the same
command with `-O`. Exact stdout/stderr, the three initial preexecution scripts,
all loaded sources and a dependency receipt are retained in
`evidence/2026-10-21-ultra-retained-model.{json,tar.gz}`. The archive reconstructs
and verifies every bundled file. Its SHA-256 is
`a2b5d81817d503d7b59245d823c86aaf22d58271973010f7e77a7c6c7acbf51c`.
The existing predecessor capture archive is explicitly reused at SHA-256
`750e3a3624c78b9d189ab93249085f96736667d25f857156ae1e065e05e4e985`;
it is preserved in repository history rather than duplicated in the new bundle.

An additive native observer campaign and independent capture audit remain separate
gates before fixture or production admission.

## Frozen retained source hypotheses (2026-10-21)

The 42-case manifest now preserves all 19 predecessor source packets under each
policy and adds two four-block alternative sources under each policy. Ordinary
generation and optimized read-only regeneration produce the exact same file,
SHA-256 `888f364acc26b310d82b397dd5508a35afb2d000a311858377fc99ce5a715636`.
All 19 policy-zero complete state hashes are unchanged. The model predicts 207635
logical events per padding pass and 348 policy-two alternative events after the
first block. Publication kinds remain explicit source-only hypotheses.

Ordinary/optimized controls each verify all 38 nested old packets and reject 17
effective later-block alternative forgeries. Changing only future backing bytes
leaves the complete first block unchanged: 1484 records for policy zero and 2048
for policy two. The changed suffix is subsequently exercised, so these are not
vacuous equal-input checks. Both added sources exhibit later-block alternatives.

Every request has a conservative stdout bound checked before native execution.
The largest is 90603078 bytes and the total for two builds is 1230723400 bytes.
The independently checked source-only evidence, selected-source exploration,
executed scripts and immutable manifest are retained in
`evidence/2026-10-21-ultra-retained-recipes.{json,tar.gz}`; archive SHA-256
`53c95e78f4978be8d32461110017883e1d666fca8799a0b80b6382c4eb5a579a`.
These checks still make no native retained-Ultra or production support claim.
