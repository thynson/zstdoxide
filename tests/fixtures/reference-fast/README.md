# Rust-readable direct Fast observations

`cases.bin` is a deterministic test-only projection of the 142 frozen recipes
in `tests/upstream/fast-parser-families`. The source collection validates both
padding records as equal before the converter stores one representative. There
are no new C executions or newly inferred parser decisions in this projection.

    python3 scripts/pack_fast_parser_fixtures.py --check

`--write` intentionally regenerates this projection from the already frozen,
independently checked observations. The pinned source collection and its
catalog cannot be replaced by changing this binary's provenance. Ordinary
Cargo tests read the binary directly; no Python, C or JSON/gzip crate is needed.

The little-endian stream begins with eight magic bytes `ZFAST03\0` and a u32
recipe count. Each record begins with a u32 byte length, then a u16 name length
and ASCII identifier; u8 window/hash/minMatch/reserved fields; u32 target,
prefix, prefix floor, initial/parser/wire repeat triples and trailing-literal
length; u8 long-length kind and u32 position; a length-prefixed source;
u32-counted seed positions; a length-prefixed literal buffer; u32-counted
LL/ML/distance/offBase quadruples; u32-counted nonzero bucket/index pairs and
bucket/old/new triples; and two u64 canonical table fingerprints. Blob lengths
and vector counts use u32. The reserved byte is zero. No native structure
padding or pointer addresses occur. Table indices preserve C's invalid zero
and valid source position plus two, so tests can distinguish them explicitly.

The checked reader in `tests/support/fast_oracle.rs` validates structural bounds
before slicing or reserving and rejects non-exhausted records/streams. Separate
tests replay all logical bytes and repeat transitions, expand long lengths,
reconstruct seeds/table deltas and compare hashes with the existing Rust key
primitive. Those checks distinguish structural parsing from semantic integrity.
The future Fast parser will consume these same records for actual comparisons.

Attribution: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, copyright Meta Platforms, Inc. and
affiliates. Observations and hash formulas derive from its Fast implementation;
this project uses the BSD-3-Clause option in `../../../LICENSE.zstd`. See the
original collection's NOTICE and provenance for unchanged-C capture details.
