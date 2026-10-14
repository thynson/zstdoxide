# Tiny Optimal match inputs

The frozen manifest records 29 source-only recipes, 71,072 source bytes and
273 queries for pinned upstream v1.5.7. Reconstruct exact packets with
`scripts/oracles/optimal_match_recipes.py`; verify them with
`python3 scripts/check_optimal_match_recipes.py` and its optimized Python mode.
Each mode rejects 172 malformed transport or metadata variants without spawning
children. Inputs and branch hypotheses are not observed matches or maps.

See `docs/reference-optimal-match-protocol.md` for ownership and bounds. No C
helper has executed these requests. Ordinary CCtx hash3 geometry, the 2^18
short-key distance boundary, full repeat state and independent complete-map
replay remain required before capture and Rust search implementation.
