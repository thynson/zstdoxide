# Private Ultra2 native qualification

Public Ultra2 remains unsupported. This report records qualification of the
private initial discarded pass and exact history invalidation, separately from
any Rust implementation, entropy/frame integration or public admission.

## Frozen preparation

The source-only recipe manifest was sealed before native execution, as described
in `reference-ultra2-model.md`. The observation contract and adapter review are
in `reference-ultra2-protocol.md` and `reference-ultra2-adapter-review.md`.
Normal and optimized pure preflight each pass 34 forged-plan/source/environment/
ledger rejection controls with zero child execution. Both check 220 commands,
70 local source inputs, 89 pinned upstream C/header files, 26 compiled codec
units, 19 removable observation hooks, 22 malformed packets and four filesystem
controls. Removing every hook recovers the original pinned source exactly.

The reconstructible preflight archive is
`evidence/2026-10-20-ultra2-preflight.tar.gz`, SHA-256
`7725da414870beb96ba1a76ac70277e51dc30f6b16196558f782395792b6c632`.
It retains both command logs, exact source snapshots, generated source/diff,
manifest, full command plan and qualification driver (82 regular files).
The source-bound normal/optimized protocol campaign separately rejects 57
malformed or coherently forged observations in each mode without children;
its archive/report is `evidence/2026-10-20-ultra2-protocol`.

## Complete native observations

The captured unchanged wrapper passes all 53 recipes in both normal and fully
ASan/UBSan-instrumented builds. Each build records 789312 events across two
padding runs (394656 logical events per padding). Every parsed canonical state
matches the independently frozen pre-execution prediction. Normal and sanitized
stdout are byte-identical. All 212 allocation ledgers agree across padding and
satisfy the exact sanitizer workspace-redzone relationship. The 220 planned
commands all reach their specified exits, including 26 malformed-packet/filesystem
rejections per build before observations or artifacts are produced.

Normal and optimized copied-evidence controls each pass 33 cases: two complete
positive audits and 31 rejected forgeries, with zero child execution inside the
controls. Mutations include phase identity, origin/frontier, discarded output,
original and copied repeats, retained caches/tree cells, coherent frequencies in
both builds, source/generated identities, command order/flags, allocation state
and a forged saved audit. Every mutation and diagnostic is retained. An initial
optimized wrapper supplied an unsupported `--output` option and stopped in
argument parsing before auditing. Its driver/log remain preserved alongside the
successful positional-argument rerun; it is not counted as a passed control run.

The native archive is `evidence/2026-10-20-ultra2-native.tar.gz`, SHA-256
`b1f9b0f828fa6bbc6d7981f443eb436c00c0d8847eea3cc893bd268ab9f9fbb5`.
It losslessly retains 1091 regular bundle files representing 2390584005 bytes,
including all raw observations, objects/executables, exact local and pinned
upstream sources, allocation ledgers, control mutations and command drivers.
The two restored duplicate control working copies are omitted only after
file-by-file equality with retained capture files; explicit omission maps record
every identity. Filesystem control nodes are represented by their type/target
manifest and actual command logs. Independent archive replay reconstructs and
hashes every retained file, followed by a second unchanged-source read.

This qualifies the private parser lifecycle and physical state transitions.
No entropy encoder or frame publication ran in this oracle. Rust implementation,
fixture-reader qualification and public strategy-9 admission remain later gates.
