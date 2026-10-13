# Ordinary retained tree frame observation protocol

This protocol qualifies private BinaryTreeLazy2 before public strategy 6
admission. It uses unchanged facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, as an external test oracle only.
The helper derives observation scaffolding from the immutable
`lazy_frame_v1.c`; no C codec implementation is copied into Rust.

## Source-only inputs

`scripts/oracles/tree_frames.py` produces 245 deterministic source/schedule
recipes: 138 prior ordinary frame/floor sources, all 103 direct-tree sources,
and four explicit owner/search/minimum/target boundary hypotheses. Their names
preserve source origin; every selected algorithm is tree, including names with
historical `row` or `chain` substrings. Sources total 7,640,096 bytes. There are
539 scheduled calls, 531 positive calls, and 120 aligned whole-frame comparisons.
Fifteen header-only controls remain incomplete and are never decoded as frames.

No recipe predicts selected sequences, block kinds, match scores or frame bytes.
Direct-tree sources run with normal empty initial contexts and automatic parser
queries; their direct query schedules do not seed or steer the frame parser.
The independent direct-tree observations remain immutable predecessor evidence.

Each source is at most 4 MiB and has at most 64 scheduled calls. Each positive
call is at most min(128 KiB, window size, source size); lengths exactly cover the
source, including explicit zero calls. Whole-frame comparisons require ordinary
maximal block boundaries. Inputs are ordinary contiguous no-dictionary prefixes;
there are no arbitrary seeded tables, repeat overrides or forged C state.

The explicit private bounds are W10..21, H6..19, C6..19 edge bits, S1..8,
minimum match 3..7 and target length 0..131072. Tree always disables row search.
Minimum/target equivalences remain hypotheses until observed. The frame JSON
parameter order is **W,H,C,S,M,target,strategy**, following earlier frame
protocols; the direct-tree protocol instead uses W,C,H. Consumers must not
silently swap those formats. Strategy is fixed at 6, parser depth at two.

## Observation contract

The helper initializes resolved explicit parameters and checks that applied
context and match-state parameters retain them. It verifies disabled dictionary,
workers, long-distance search, custom sequence producers and block splitting.
Every source/output/scratch buffer has checked extents and guards. Initialized
head and paired-edge arenas are validated against the actual workspace extent,
alignment and non-overlap before copying them. Unsigned sentinel 1 is allowed
only in odd edge cells; it is never interpreted as a source position.

Every call records complete table state through bounded ordered deltas with
before/after fingerprints, retained sampling frontier, wrapper window floor,
parser sequences/literals, candidate repeats and entropy state, committed state,
physical publication ownership, and encoded block type/payload identity.
Whole frames are decoded by the C library and retained as exact output bytes.
Scheduled completion adds a final empty raw block; aligned whole compression
compares the preceding complete payloads independently of the last-bit change.

Tiny calls below seven bytes do not refresh parser scratch. Zero calls do not
advance any sampling or source state. Positive calls clamp the frontier to the
block-start window floor before applying the existing 384/192 gap adjustment
for fresh parser calls. Sampling writes persist through raw/RLE fallback, while
only compressed blocks publish candidate repeats and entropy owners.

## Pre-execution gates

Before running C: freeze recipes and source identities; validate malformed recipe
controls in normal/optimized Python; review the helper's changes; compile normal
and ASan/UBSan syntax; independently implement bounded observation replay and
forged-record controls; freeze all dependency hashes; and preflight the full
normal/instrumented command inventory and output caps. Build all participating
upstream units under the same instrumentation policy, not just the helper.

Capture two independently initialized source-padding passes per recipe and
require identical observations/frame bytes. Retain compiler/revision identities,
commands, exit statuses, stdout/stderr, frames, complete source recipes and
normal/instrumented equality. Offline audits must execute no children and reject
forged metadata, table deltas, source identities, sampling, sequences, repeats,
publication, frame extents and archive/source hashes without depending on Python
assertions. Do not mutate previous captured scripts or fixtures to accept new
results; version a correction and preserve its predecessor evidence instead.

## Rust qualification

After capture, a bounded independent fixture reader replays source and state
invariants before exposing borrowed observations. Rust retained state must match
all candidate sequences, literals, sampling frontiers and complete head/edge
maps after each call. Framing must match every complete scheduled/whole payload
and decode correctly. Allocation failures and late publication failures retain
exact owner release and transactional repeat/entropy behavior. Only then proceed
to independently resolved public tuples and bidirectional interoperability.
