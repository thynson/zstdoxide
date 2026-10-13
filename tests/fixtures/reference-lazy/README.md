# Direct Lazy-family Rust fixture projection

This test-only format projects frozen v1.5.7 direct C observations, never Rust
encoder output. `ZLZST01\0` (eight bytes) and a little-endian u32 count of 645
start the file. Each record has a u32 body byte length, then a u8 depth (0..2),
three zero reserved bytes, and the exact Greedy body schema documented in
`tests/fixtures/reference-greedy/README.md`. Names start with `d0-`, `d1-` or
`d2-` matching the explicit depth. Strategy is derived as 3 + depth; it is not
stored as a second potentially inconsistent selector. Depth is not a public
Rust encoder admission decision.

The common body retains every input scalar, full source, seeds, initial/final
state, C cache metadata, literals, raw/expanded sequences and complete sparse
owner maps with fingerprints. C positions retain bias two and zero sentinel.
Bodies are capped at 32 MiB and the collection at 64 MiB; unused/reserved/trailing
bytes reject. Borrowed spans and fallible metadata vectors avoid hidden pointer
ownership. The old Greedy fixture and its strict parser/search bounds remain
unchanged. This projection cannot be sent to the separate C packet protocol.

`provenance.json` binds the generator, independently pinned source checker,
immutable capture and every record extent. `--write` only initializes missing
outputs after checking all existing outputs; it never refreshes expectations.
Use `python3 scripts/pack_lazy_parser_fixtures.py --check` for offline exact
reconstruction. Fresh-context salt groups remain in the original collection.
Final maps and published sequences are not chronological probe traces. A Rust
adapter and parser equivalence checks follow as separate work items.


The frozen pack contains 645 records and 24,567,619 bytes, SHA256
`be38aa8c6e4439bf999a6168ee81347b9e7af38391925b6fc548ffaf8c60aea7`.
It represents 1,290 padding observations and 2,580 direct observations across
both C builds. An independent reader compares every field and extent with the
original immutable observations. Normal/optimized Python each accept two
positive reads and reject 28 mutations with child creation blocked; separate
publication controls each accept two positives and reject four overwrite,
partial-output and symlink cases. Their source-validation scope is separate.


The checked Rust adapter is `tests/support/lazy_oracle.rs`. Structural decoding
checks selectors and bounded bodies; dedicated tests independently replay all
sequences and complete maps before parser-equivalence tests use the evidence.
Five tests pass in safe, isolated chain, isolated row and all-feature builds;
all 173 depth-zero predecessor records compare exactly after stripping the name
prefix. This validates reading and seeded storage, not Rust Lazy parsing.
