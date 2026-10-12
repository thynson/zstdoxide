# Initial DoubleFast adapter implementation review

Coordinator review on 2026-10-12, before first execution. The C helper is test
setup and independent replay only; no production Rust parser is called or
implemented. Reviewed helper SHA-256: `e20d452423a3f913831ac04f6edd79be3d0d18fb0fe6609357ef4ea47816d9d4`.

The implementation matches the ZDFST01 packet extent and validates scalar,
seed, selected-key, shift and aggregate allocation bounds before execution.
All twelve live heap owners have initialized payloads and 64-byte guards:
1 recipe source + 2 source arenas + 4 table owners + 1 sequence + 1 literal +
1 code + 1 replay + 1 observation. The checked total includes every owner
exactly once, including its guards.

Each malloc result plus the guard offset satisfies the checked U32/SeqDef/
Observation alignment. Stack-declared state/store preserve their native
alignment. Source normalization keeps base=source-2, negative tail expressions,
search advances and prefetch pointer expressions within the same arena; the
physical suffix is 131584 bytes, as proven in the protocol. Complete prefix
seed keys are validated before C hash calls. Short physical eight-byte reads
fit the arena even when selected keys have width5..7. Every parser table write
stores an earlier complete source key; both entire tables are checked against
independent key ranges and C bucket computation after execution. The Python
validator separately computes the LE hash arithmetic and reconstructs both
complete before/after arrays from ordered seeds and canonical changed cells.

The conservative sequence capacity floor(B/3)+1 exceeds the maximum possible
number of matches of at least four bytes. B+32 literal storage accommodates
permitted wildcopy slack. The exact logical literal/sequence extents and long
length marker are checked before independent replay. Three positive initial
repeats are admitted; unchanged entry logic disables unavailable distances
before its bitwise repeat loads. Match replay enforces the repeat window/F
bound without incorrectly imposing the stronger ordinary block-end floor.

Two padding executions use fresh owners and state. Source plus padding must
remain byte-identical to its before-image. Full observations must agree after
removing only the padding scalar. Cleanup checks every guard and exact owner
accounting. The test helper terminates on setup/assertion failure; OS process
cleanup handles failed setup, and LeakSanitizer remains disabled explicitly.
No whole-archive sanitizer coverage is claimed.

The driver requires a new output directory disjoint from repository/reference,
checks all cataloged lib C/headers and the recorded support archive, recompiles
unchanged DoubleFast explicitly in both modes, retains dependency lists and
all effective command environments, and caps each stdout/stderr stream at
64MiB with a 120-second deadline. Failed commands retain partial bounded logs
and evidence. Clock time here controls capture limits; it is not a benchmark.
