# Dictionaries, prefixes, and training

Module 7 follows the completed bounded-streaming milestone. Reference source is
facebook/zstd v1.5.7 at `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.
No C codec or other Rust codec becomes a production dependency.

## Ownership and public configuration

`Dictionary` owns an immutable serialized byte vector and a validated content
start index. Raw dictionaries use the entire vector and ID zero. Formatted
objects additionally own immutable prepared Huffman/FSE tables and three repeat
offsets. No field borrows another field. `content()` resolves a checked range;
`as_bytes()` preserves the complete original serialization. Construction copies
untrusted bytes through fallible reservation and validates before publication.

`DictionaryMode` is a plain enum: Auto, Raw, or Formatted. Auto treats short or
nonmatching magic as raw, but matching magic commits to formatted parsing and
never falls back after an error. Forced raw mode never interprets a header.
Auto's short-buffer threshold is eight bytes, even when a shorter buffer begins
with the magic. Forced formatted mode requires the magic and complete validated metadata.
Short raw buffers, including empty content, remain accepted for compatibility
with the reference loader, despite the format prose's eight-byte recommendation.

The public constructors are `Dictionary::from_bytes(input)` (Auto),
`from_bytes_with_mode(input, mode)`, and `raw(input)`. Read-only accessors expose
`id`, `content`, `as_bytes`, and whether the object is formatted. An
`into_shared()` convenience produces `SharedDictionary`: `Arc<Dictionary>` on
targets with pointer atomics, and `Rc<Dictionary>` otherwise. Sharing allocates
only its fixed-size ownership handle; dictionary-sized buffers are prepared
fallibly. This handle remains an explicit allocator migration boundary.

Encoder/decoder contexts own optional shared persistent, pending-prefix, and
active-frame handles. Configuration borrows no caller input. A persistent
dictionary is available to each subsequent frame. A prefix replaces previous
configuration, is claimed once when a standard frame begins, and is not restored
by reset after that claim. Skippable frames do not consume the pending prefix.
Configuration during an active frame requires reset first. Reset retains
persistent dictionaries and an unclaimed prefix, cancels active-frame state,
and preserves the established options/counter reset contracts.

A nonzero frame ID requires a matching selected dictionary. A zero or absent
frame ID does not disable a configured dictionary. IDs in reserved distribution
ranges are accepted by the decoder; newly trained IDs should use the ordinary
public range. Missing versus mismatched dictionaries have distinct typed errors.

## Formatted layout and immutable entropy

The format is little-endian magic `0xEC30A437`, little-endian u32 ID, Huffman
literals table, FSE offsets table, FSE match-length table, FSE literal-length
table, three little-endian u32 repeat offsets, then content. Bounds are OF
symbol 31/log 8, ML symbol 52/log 9, and LL symbol 35/log 9. Every repeat offset
must be nonzero and no greater than content length; duplicates are legal.

Prepared entropy remains immutable. Working frame tables override dictionary
tables only when a block supplies replacements. Missing working tables fall
back to the selected dictionary for first-block treeless/repeat modes, through
ordinary borrowed parameters during that decode call. No self-referential
borrow or per-frame clone of prepared tables is required. New frames restore
initial dictionary entropy and repeat offsets. Raw dictionaries initialize no
entropy and retain the ordinary repeat offsets `[1, 4, 8]`.

## History and dictionary-to-frame matches

Dictionary content is a separate immutable history region. It does not count
towards frame output, checksums, content size, or output limits. Its length and
legal initial match distances can exceed the advertised output window.

Let P be produced frame bytes after a sequence's literals, D dictionary content
length, W advertised window, and d match distance. An ordinary match has d<=P
and requires d<=W. A dictionary match has d>P, k=d-P in 1..=D, and begins at
content[D-k]. It copies up to k dictionary bytes, then, if necessary, continues
at frame byte zero with ordinary overlap expansion. Eligibility is checked at
sequence start, not rechecked after every copied byte.

The chosen dictionary lifetime rule follows the pinned format prose literally:
dictionary references are allowed at P<=W and rejected at P>W. Production C is
more permissive, while its optional fuzz assertion rejects P==W. This discrepancy
is explicitly preserved in boundary fixtures rather than hidden by oracle-only
acceptance. Direct, prepared, and streaming C probes independently confirm it.

A valid P=900, d=1100, length=300 match with W=1024 copies 200 dictionary bytes
then frame[0..100]. Writing those dictionary bytes to the ring immediately
would evict frame[0] before the match reads it. Therefore compressed blocks read
from unchanged previous-block ring history plus their own pending output,
then commit the completed block to the ring. Raw/RLE blocks keep direct streaming.
The pending block and literal scratch remain bounded by 128 KiB. Ring positions
and frame counters remain separate; absolute prior-frame reads are checked
against the initialized retained interval before accessing a physical slot.

## Safe and unsafe families

Dictionary metadata, selection enums, ranges, and shared ownership are ordinary
safe records. Huffman/FSE/match/ring operations reuse their independent existing
backends. `unsafe-dictionary` is a new opt-in alternative only for validated
immutable content access. Its safe API must check content-relative indices and
ranges before either implementation; unsafe operations require local proofs of
bounds, initialization, ownership, lifetime, alignment and non-aliasing. It must
be tested and benchmarked independently, not enabled as an implicit dependency.

## Compression and training work boundaries

The first encoding checkpoint caches an indexed dictionary seed table and copies
it into the working match table for eligible blocks. `BlockEncoder` owns a shared
handle alongside cached indices, preventing indices from being paired with a
different same-length buffer. It tracks the produced frame position separately
from dictionary/current-block coordinates and advances for raw/RLE blocks too.
Reset drops the active handle while retaining fixed table allocations. Context
memory accounting includes shared dictionary storage only once.

The initial predefined offset distribution supports symbols through 28, so
search ignores distances above `(1 << 29) - 4`; larger dictionaries remain
accepted and their addressable suffix remains usable. Explicit wire offset
values are `distance + 3` and therefore greater than three, independent of
prepared repeat offsets. Their entropy symbol may still be two. First-block
dictionary matches may cross into frame output; later-block crossing initially
stops at the dictionary end until retained output history is implemented in
the next separately tracked checkpoint. Ordinary dictionary-free encoding must
retain its existing deterministic bytes through these foundational changes.


Dictionary compression extends indexed match search over an explicit retained
prefix plus the current block. Candidate indices are validated against source
bytes and the permitted dictionary/output history intervals. Frames advertise
the selected formatted ID unless omission is explicitly requested. Raw literals
and predefined sequence tables remain a correct first strategy; prepared entropy
reuse and advanced strategies are later optimizations, not prerequisites to
using dictionary content effectively. Prefix lifetime/reset semantics match the
decoder's explicit owned configuration.

Training is a separate checkpoint within Module 7. Before implementing it,
record bounded sample selection, candidate scoring, sample ownership, output
capacity, parameter validation and dictionary serialization. It must construct
real useful dictionary content and valid metadata, pass independent compression
and decoding checks, and port applicable official training failures/reuse tests.
It must not claim to reproduce COVER/fastCover optimization or byte-identical
C training output unless those algorithms and their assertions are implemented.
Any unmet training obligations stay open in the module checklist.

The small work items and their required commits are in `TODO.md`. Before closing
Module 7, port applicable dictionary/prefix/training tests, replay matching
external corpora, measure the added backend, and run the complete no_std/backend,
independent-oracle and targeted Miri validation.

## Retained encoder output history

After dictionary-seeded matching, `BlockEncoder` gains an owned optional
`RingHistory` using its already independent safe/unsafe family. Allocate it
lazily for nonempty input with horizon `min(advertised_window, 128 KiB)`. Every
successfully encoded block commits its original input, including raw and RLE
blocks. Reset/start clears logical positions and preserves reusable capacity.
The ring allocation is included in context capacity. This horizon is a
compression search limit; larger advertised windows remain accepted.

Temporary match coordinates concatenate dictionary `[0,D)`, retained history
`[D,D+H)`, and the current block `[D+H,D+H+B)`, using checked usize additions.
Absolute frame positions remain u64. Seed the working table from dictionary and
retained output, including keys across the history/current-block boundary.
Validate ordinary distances against advertised window and initialized retained
history before comparing source bytes. Dictionary eligibility remains P<=W and
predefined offset symbols still impose their documented distance bound. A
dictionary match may cross into frame byte zero only while that position remains
available in the immutable ring or current input; otherwise clip the match.

Previous-block matching intentionally changes some previously independent-block
encoded bytes. Verify exact decoded content and determinism under caller chunk
schedules, document the changed subset of the old 48-frame baseline, and test
ring wrap, horizon eviction, explicit reset isolation and retained allocation.
No additional unsafe structure is introduced.
