# Ordinary coupled-policy Ultra observation protocol

Reference: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`; source attribution is retained in
`LICENSE.zstd` and `NOTICE`. The upstream codec is an observation oracle only.

A request is `ZUFRS01\0`, a little-endian u32 policy (0 or 2), followed at byte 12
by the exact original `ZOPFW03\0` packet. Total bounds are 124..16444 bytes.
The nested source size, block count, window/hash/chain/search logs, minimum match,
target and literal mode start at byte 20. A single block length is at byte 56;
source bytes start at byte 60. Every original request constraint remains in force.
See `reference-ultra-fresh-model.md` for immutable request ownership.

NDJSON metadata uses protocol `ZUFRS01`. Every record includes `price_policy`.
Each event has an `alternative` member: null normally, or twelve signed integers
for a saved-match observation. Its order is saved price/offset/match/literal/reps,
signed LL increment, match-plus-one-literal price, longer-literal-run price,
existing next-cell price and match predecessor. Existing stage names stay intact;
`alternative` is the one added stage. Full canonical search/path deltas,
frequencies/caches, sequences, repeats, guards, block owners and final counts
retain their original meanings. Both padding passes must agree with source-only
replay, and normal/sanitized transcripts and complete frames must be identical.

The frozen 44 recipes retain twenty original sources under each policy plus two
additional branch sources under both policies. The additive `capture-plan.json`
records complete source-only row hashes, counts and coverage before execution.
Expected block kinds are explicit publication hypotheses, checked against native
wire headers and owner transitions. They are not predictions of entropy encoding.

`ultra_fresh_capture_plan.py` specifies exact argv, order, diagnostics, environment,
timeouts and output bounds. Malformed packet names describe the changed field.
Filesystem controls live separately under `special/`; the symlink points only to
a valid local request, the FIFO has no writer, and the missing path remains absent.
These controls must exit before output or upstream allocation. The auditor uses
strict JSON decoding and exact complete-state replay; forged transport, command,
source, state, frame and allocator records must be rejected without child execution.

This campaign qualifies only fresh ordinary source-driven observations. Retained
Ultra, public strategy 8 admission, execution planning, shared entropy publication,
complete interoperability, benchmarks and Ultra2 lifecycle remain separate gates.
