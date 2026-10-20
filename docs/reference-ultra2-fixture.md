# Private Ultra2 fixture layout

This specification precedes projection of the independently audited native
capture. It does not itself claim that native capture or the Rust reader passed.
No existing fixture is replaced. The new fixture contains parser lifecycle
observations only; it contains no entropy-coded block or complete frame.

## Ownership and bounds

The immutable little-endian `ZU2FX01\0` packet starts with a u32 case count.
Exactly 53 source-only recipes are projected in their sealed order. Names and
source bytes are borrowed by the Rust reader; phase/event/delta vectors use
bounded fallible allocation. The complete packet is capped at 128 MiB, cases at
53, source bytes at 16384, phases at two, events per phase at 65536, path cells at
4099, search words at 73730 and sequence records at 5462. All extents and domains
are checked before allocation/indexing. Full packet exhaustion is required.

Each case begins with twelve u32 words: ASCII name length, source length,
window/hash/chain/search logs, minimum match, target length, literal mode, requested
seeding flag, actual phase count and path-cell count. Two three-word native
allocation records follow (normal, then sanitized): CCtx bytes, workspace bytes,
peak bytes. Names and source bytes follow without padding. These allocation sizes
are opaque qualified native observations, not source-model predictions.

The eligibility record contains fifteen u32 words: five Boolean predicates,
actual seeding Boolean, current position, dictionary limit, low limit, update
frontier, source length, three original repeats and opaque sequence-prefix count.
The predicates must equal the independently derived fresh/skip/threshold rule.
The opaque prefix is never presented as encoded output.

## Phases and retained physical state

Each phase header has six u32 words: phase (zero for seeding, one for encoding),
integer source origin, three original repeats and event count. A full 386-word
frequency snapshot follows. Its layout is 256 literal, 36 literal-length,
53 match-length and 32 offset frequencies, then four sums, four cached base
prices (literal, literal length, match length, offset order), then a Boolean
predefined-price flag. The first phase starts with zero state; the second starts
with exactly the state retained at the preceding reset.

Each event uses the established Ultra event fields: stage, reverse-phase flag,
ip, anchor, optional cur/last/tail represented by u32 MAX, three repeats; then
presence-tagged stretch (seven words), reverse span (three), query (six), optional
query match count and pairs; an optional twelve-i64 alternative-price decision;
canonical path deltas, frequency deltas, cumulative and appended sequence counts
with appended four-word records, and canonical search deltas. Stages retain the
existing thirteen-value order. Delta entries are strictly increasing, in bounds,
nonredundant and complete. Their baselines persist across both phases. The two
search frontier words are included, so the first event after reset explicitly
records coordinate invalidation while physical hash/tree/short cells remain.
Query pre-state must equal the preceding complete search snapshot.

The phase footer contains three candidate repeats, sequence count and tail,
followed by the full 386-word frequency snapshot. Footer sequences and frequencies
must equal the final event's state; source reconstruction must consume the exact
input with the tail. The seeding phase has no publication effect.

A Boolean reset-presence word follows each footer. A seeding reset has fifteen
words: discarded sequence count, literal count, tail, three unchanged caller
repeats, three discarded candidate repeats, origin before, origin after, low
limit, dictionary limit, update frontier and a true table-preservation flag.
Its full frequency snapshot follows. All discarded counts must equal the first
phase; all new coordinates equal old origin plus source length. No frequency or
physical path/search cell is cleared by this record. Encoding has no reset.

## Independent verification

Projection accepts only a fully audited capture, verifies both native builds and
both padding observations, and serializes the first padding run. A separate
source-only reconstruction derives every eligibility/phase/event/reset byte,
retaining only audited native allocation sizes as opaque inputs. Equality covers
the complete packet; coherent forgeries are checked with the outer digest gate
bypassed. The Rust reader separately checks structural domains, lifecycle order,
frequency invariants, source-relative matches, repeat publication and stale
history exclusion. Neither projector nor reader is a production codec dependency.
