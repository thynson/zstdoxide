# Private Ultra2 two-pass observation protocol

This protocol precedes native execution. It does not open public strategy 9.
The pinned reference is facebook/zstd v1.5.7 revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`.

`ZU2PS01\0` is followed by nine little-endian u32 fields: source size, seed mode,
window log, hash log, chain log, search log, minimum match, target and literal
mode. Source bytes begin at byte 44; total extent is exactly 44 plus source size.
Valid size is 7..16384, modes are 0/1, geometry is W10..16, H/C6..12, S1..6,
minimum 3..7 and target 0..4095, with the entire source inside its window.
Seven/eight/nine-byte requests probe the private threshold only. These bounds
are not public admission and do not alter any historical request or fixture.

Seed mode one creates fresh no-dictionary state, original repeats 1/4/8 and an
empty sequence store. The unchanged native wrapper decides whether to seed.
Seed mode zero creates the same state with one explicit opaque sentinel sequence
(zero literal length, match base zero, offset base one); the native empty-store
predicate then skips seeding. The sentinel is checked unchanged but never
published as a compressed frame. Other gate blockers remain arithmetic controls
at this stage, not claimed native dictionary/retained-state observations.

Metadata binds the source fingerprint, exact parameters, protocol, revision,
path extent and two padding fills. Each padding run starts with zero observation
baselines and a fresh real CCtx. Records are chronological: eligibility;
pass_begin before frequency scaling; every complete ordinary event; pass_end;
reset after the original discarded-pass sequence-store and coordinate changes;
then encoding pass_begin/events/pass_end. Small inputs or seed-mode zero omit
seeding and reset. The final record accounts for both algorithm passes and all
events across both padding fills, with exact EOF and no trailing records.

Path events retain the ordinary block-relative query/IP/anchor offsets and full
frequency/caches, sequences, repeats, signed alternatives and canonical sparse
path/search deltas. The additional pass_name is seeding or encoding. Physical
search indices stay absolute across the coordinate change; no stale cell is
normalized away. A reset record preserves discarded counts/tail/repeats, caller
repeats, frequency state and every new history limit/frontier. Source-view read
counts belong only to the Python model and are deliberately absent on the wire.

Every line is bounded by 2 MiB, each transcript by 256 MiB, and the campaign's
combined logs by 5 GiB. The helper caps events per parser invocation at 65536,
path cells at 4099, search words at 73730 and recorded sequences at 5462. Native
allocation requests and live bytes are capped at 8 MiB; every owner is guarded,
checked live and released. Allocation ledgers are separate small artifacts,
identical across padding fills. Both normal and fully ASan/UBSan builds cover
all 26 used translation units, with original workspace poisoning enabled.

Only the source-only model, immutable recipes, bounded transport and read-only
instrumentation may determine expected observations. Native output may falsify
them but may not be used to choose new expectations without a documented,
additive correction and preserved failed evidence.
