# Captured scripted Optimal paths

Regenerate with `python3 scripts/pack_optimal_path_fixtures.py`. The projector
checks every member of the immutable small and wider capture archives, both
C builds, both arena padding passes and independent replay. One equivalent copy
contains 27 cases, 22,024 events, 966 queries and 42 emitted sequences. These are
scripted-provider trajectories; ordinary CCtx startup remains unqualified.

All integers are little-endian u32. The header is `ZOPFX01` plus a zero byte and
case count. Each case has nine words: name length, source length, minimum-match
parameter, target, literal mode, path-cell count, event count, script count and
kind (0 small, 1 wider). ASCII name/source bytes precede scripts: position,
candidate count and offset-base/length pairs.

Each event starts with stage, phase, input position, anchor, current position,
last position, tail and three repeats. Optional scalar positions use u32::MAX.
Stages are begin, query, origin, seed, literal, relax, select, reverse_start,
reverse, sequence, refresh, complete (0 through 11). Phase 1 means reconstruction.
Three optional arrays follow, each with a 0/1 presence flag: seven-word stretch,
three-word reverse coordinates, six-word query. Present queries include candidate
count and offset-base/length pairs. Next are path-delta count and entries (index,
price, offset base, match length, literal length, three repeats), then frequency
count and index/value pairs. Last are sequence live count, append count and
four-word appended records (anchor, literal length, match length, offset base).

Deltas are strictly increasing changed indices against zeroed per-case baselines.
The 386-word frequency view contains 256 literal, 36 literal-length, 53 match-length
and 32 offset counts, then four sums, four cached prices and predefined mode.
The independent Rust reader validates extents before allocation, canonical deltas,
source matches, repeat publication, frequency sums/cache and reconstruction
coordinates. `provenance.json` binds every case to its archived request and trace.
Upstream reference: facebook/zstd v1.5.7, revision
f8745da6ff1ad1e7bab384bd1f9d742439278e99, BSD-3-Clause.
