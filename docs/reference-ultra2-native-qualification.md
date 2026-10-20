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

Native capture and copied-evidence controls are a separate pending gate. The
preflight results above do not claim that a native helper has passed.
