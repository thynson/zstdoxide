# Ordinary Greedy input recipes

These 126 source-only recipes precede ordinary C execution. The selected
explicit-parameter branches contain 55 chain and 71 row cases, totaling
6,959,842 source bytes, 355 scheduled calls (347 positive), 13 aligned
whole-buffer comparisons and 15 incomplete-header probes. Hypotheses are not
observed block-kind, discard, timing or public-admission results.

The generator is `scripts/oracles/greedy_frames.py`; the independent identity
checker is `scripts/check_greedy_frame_recipes.py`. The generator SHA256 is
`360bf5139b7e2a886e9d2359c1ea258317238bd1906cfe9d123be33df7a0ce44` and the
manifest SHA256 is
`f833a7015aba52bf9a43fc941f1e766ab0bec5ea8c8e285eb970de1f2d702f13`.
The xorshift32 source primitive preserves the previous Fast/DoubleFast recipe;
no codec participates in recipe construction. Source identity uses SHA256.

The complete lifecycle, ownership, alignment and capture requirements are in
`docs/reference-greedy-frame-oracle.md`. Separate helper review, offline
validation and normal/full-instrumented capture are pending. Existing direct
Greedy and historical Fast/DoubleFast fixtures remain immutable.
