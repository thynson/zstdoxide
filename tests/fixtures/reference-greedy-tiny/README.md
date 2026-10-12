# Frozen direct Greedy input hypotheses

The input recipes were frozen before any Greedy C capture or Rust
reference-Greedy implementation. Complete C observations have now been added
in a separate pinned capture manifest and NDJSON pack. Thirty-six source recipes
produce 6,821 packet bytes and 1,313 logical source bytes. They cover both
private chain/row entry points, fresh 7/8/9/10 and 15/16/17/18-byte blocks,
prefixed 9/10/16/17-byte blocks, period-one/four sources, minMatch 3/4/7,
unavailable initial repeats and two explicit nonzero row-salt cases.

The packet grammar and bounded owners are documented in
`docs/reference-greedy-oracle.md`; the helper's pre-execution source review is
in `docs/reference-greedy-adapter-review.md`. Automatic public branch selection
is not tested by forcing a private branch in these packets. Nor do these
recipes close the later row-width, collision, gap, skipping or ordinary-frame
capture tasks. Hash-cache observations and final tables are not chronological
traces of every probe.

`recipes.json` pins source/packet identities and all input scalars, seeds,
initial repeats and salts. It also pins the source-only generator/validator,
reviewed helper and protocol documents. The manifest itself is pinned by
`scripts/check_greedy_tiny_recipes.py`. The checker verifies identities before
importing the generator, recreates every input, round-trips its binary packet
and rejects 37 malformed packets. Thirty-one closed-form hash checks exercise
zero words, salt high-bit selection and little-endian extraction. Normal Python
and `-O` both pass these checks; neither executes a process or a C parser.

The independent record validator reconstructs complete initial/final maps,
checks canonical fields and bounded deltas, expands lengths and replays full
wire repeats/overlapping matches. Its runtime use against normal/instrumented C observations and forged-record
rejection controls now pass. It does
not implement the Greedy search to generate expected C output. The source-only
fresh-salt bitmix calculation agrees with the separate actual fresh CCtx
observations; the observed salt is `835892b4bb2cae74`.

Run `python3 scripts/check_greedy_tiny_recipes.py --check`. Keep this input set
unchanged when adding wider families; preserve any first diagnostics before
changing the helper or admitting observations.

`capture.json` indexes 147,120 bytes of complete observations: 36 direct
recipes and separate normal/sanitized fresh-context groups. Direct observations
are byte-identical between C builds. Only the opaque CCtx peak differs
(149,198 versus 151,502 guarded bytes), because upstream adds ASan workspace
redzones. Both peaks and the first failed overbroad comparison remain preserved.

Run `python3 scripts/check_greedy_tiny_capture.py --check-fixture` for the
permanent offline replay. To audit all 210 commands and raw artifacts of a live
capture, pass its directory instead. Normal Python and `-O` each reject 30
forged captures with child creation disabled. See
`docs/reference-greedy-validation.md` for scope, source and archive identities.
