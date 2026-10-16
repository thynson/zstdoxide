# Source-only ordinary Ultra requests

`recipes.json` freezes 44 policy-tagged requests: all twenty original complete
fresh requests under policies 0 and 2, plus two deterministic branch sources
under both policies. It records exact packet/source hashes, copied parameters,
publication hypotheses and independent predictions for the four added cases.

Reproduce with `scripts/check_ultra_fresh_recipes.py`. The generator in
`scripts/oracles/ultra_fresh_recipes.py` uses explicit xorshift32 arithmetic and
fixed seeds 0 and 2. Inputs were selected solely by independent source-driven
replay, before any new native Ultra execution. See
`../../../docs/reference-ultra-fresh-model.md` and
`../../../docs/reference-ultra-validation.md` for ownership, original native
reconciliation, complete source search and forged-input controls.

This is a request manifest, not a native observation fixture. Its block kinds
are explicit publication hypotheses; they do not predict entropy selection.
