# Direct Greedy Rust fixture projection

This test-only projection consumes the immutable tiny and broader C captures.
It does not execute C or a Rust codec, regenerate parser expectations, change
source recipes, or expose Greedy publicly. The source captures retain every
metadata field and both physical-padding observations in each instrumented mode.
The projection retains the padding-independent logical inputs, sequence records,
full sparse owner maps, and parser state needed by the checked Rust test adapter.

All integers are little-endian and unsigned. `ZGRST01\0` and a u32 case count
begin the file. Each case is a u32 byte length and an exactly bounded body:

1. u16 name length, then ASCII name bytes (1..63).
2. Eight u8 values: branch (chain 0 / row 1), window log, hash log, chain log,
   search log, minMatch, effective key width, row log (zero for chain).
3. Twenty u32 values: target, prefix length, retained floor, initial logical
   next-to-update, initial repeats[3], parser repeats[3], wire repeats[3], trailing
   literals, initial current, entry repeat floor, final **raw C** next-to-update,
   final skipping flag, initial entropy, final entropy.
4. Initial/final salt as two u64 values, then eight u32 C hash-cache cells.
   The cache is retained C metadata; a scalar Rust implementation need not own
   the same cache, but must preserve every logical insertion and search margin.
5. u8 long-length kind, u32 long-length sequence position.
6. u32 source length, complete source bytes, u64 source FNV-1a fingerprint.
7. u32 seed count and that many u32 logical source positions.
8. u32 stored-literal length and stored literal bytes (excluding trailing input).
9. u32 sequence count and six u32 values per sequence: raw LL, raw ML base,
   expanded LL, expanded ML, resolved distance, and offBase.
10. Two owner maps, positions/links for chain and positions/tags for row. Each
    starts with u8 element width (4 or 1), u8 log, and zero u16 reserved bits;
    then a u32 count of sorted nonzero `(slot, value)` u32 pairs, a u32 count
    of sorted `(slot, old, new)` u32 triples, and two u64 FNV fingerprints for
    complete before/after owners. Fingerprints encode u32 owners as four-byte
    little-endian cells and tag owners as single bytes.

No unused bytes or implicit padding are allowed. C position/link values preserve
its raw index bias of two and zero sentinel; row reserved-head tag bytes remain
ordinary u8 values. A parser comparison must translate Rust's invalid sentinel
and logical source indices explicitly. Source, name and literal slices can borrow
the fixture, while metadata vectors validate counts before allocation. A reader
must reject malformed dimensions, spans, ordering, values, sequence replay and
fingerprints; it must not trust a count merely because the pack is generated.

`provenance.json` pins the generator, independent checker identities, both source
collections and every case extent. Run `python3
scripts/pack_greedy_parser_fixtures.py --check` for an offline reconstruction and
exact byte comparison. `--write` initializes missing outputs only and refuses to
overwrite changed expectations. The C packet transport `ZGRDY01` is a separate
format; this projection cannot be sent to that executable.

The frozen projection has 173 records and 7,985,140 bytes, SHA256
`5bc0e95cd395d8be489638b2f03ead1494172d92c7c1b0ede49c97cf3e4a0064`.
It represents 346 padding observations and 692 direct observations across the
two C builds; fresh-context-only groups remain in their original collections.
An independent binary reader compared every input/state/sequence/map field with
the original C captures. Normal Python and `-O` each pass two positive reads and
reject 24 mutations without subprocess creation. Separate publication controls
reject changed case bytes, changed provenance, partial publication and symlink
outputs. [Evidence and replay drivers](../../../docs/evidence/2026-10-12-greedy-projection.json)
retain exact checks and their scope. The Rust reader/parser is the next work item.
