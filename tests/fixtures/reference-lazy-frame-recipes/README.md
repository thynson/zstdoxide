# Ordinary lazy-family input hypotheses

This immutable source-only manifest freezes 510 recipes before the ordinary C
campaign: 414 depth-tagged predecessor inputs and 96 constructed source inputs.
It contains 22,499,784 source bytes, 1,401 scheduled calls (1,377 positive), 39
aligned whole-frame comparisons and 45 incomplete header requests. Each depth
has 170 cases; automatic dispatch selects 231 chain and 279 row cases overall.

Regenerate and verify without executing a codec:

```sh
python3 scripts/check_lazy_frame_recipes.py --check --controls
python3 -O scripts/check_lazy_frame_recipes.py --check --controls
```

The 138 depth-zero predecessor inputs retain all source, parameter and schedule
fields. Constructed sources inherit only bytes from the previously frozen direct
score families; their ordinary context starts with default repeats and naturally
sampled tables. Their names do not assert a score witness or output behavior.
The protocol is `docs/reference-lazy-frame-oracle.md`; generator and manifest
identities are checked by the dedicated validator. This fixture has no expected
codec output and does not establish ordinary/public Lazy support.
