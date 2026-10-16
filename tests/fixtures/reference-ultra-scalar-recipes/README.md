# Source-only Ultra scalar recipes

`recipes.json` freezes synthetic scalar inputs for pinned zstd v1.5.7 at
revision f8745da6ff1ad1e7bab384bd1f9d742439278e99. It contains no C observations.
The deterministic recipe generator and independent interpolation model live in
`scripts/oracles/ultra_scalar_recipes.py` and `ultra_scalar.py`.
Extra-bit widths derive from the pinned format tables; see NOTICE/LICENSE.zstd.
Whole-bit and fractional policies have explicit tags and bounds. These inputs
do not claim to represent ordinary CCtx frequency states or public Ultra frames.
