# Audited Optimal price fixtures

The 13,036,992-byte pack contains 147 owned-source recipes and 2,018 complete C
observations. It is projected only from the immutable audited normal/instrumented
capture; model replay validates observations before projection. The pack is not
produced by production Rust or by synthesizing expected prices.

All integers are little-endian u32. Header: eight-byte `ZOPFX01\0` magic and case
count. Each case: name length, source length, operation count, literal mode, ASCII
name and source bytes. Each operation has six request words followed by 390
observation words: price mode, 377 frequencies (256 literal, 36 LL, 53 ML, 32 OF),
four sums, four cached base prices, query-present flag and three prices (zero
when absent). Case and stream extents are exact, with no trailing bytes.

Run `python3 scripts/pack_optimal_price_fixtures.py` to reproduce and compare the
complete projection from the pinned archive without C execution. The independent
Rust fixture reader checks bounds, update budgets, cache phases, table sums,
source ranges, modes and unused values. Strategy 7 remains unsupported.
