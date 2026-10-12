# Ordinary DoubleFast trajectory projection

This test-only projection preserves all51 independently frozen source recipes,
160 positive blocks,148 fresh candidates and both complete sampled table maps
from the pinned zstd1.5.7 ordinary-frame collection. Source bytes and literal
bytes are borrowed from `cases.bin`; schedules, sequences and sorted deltas use
bounded owned Rust vectors. Raw C repeat triples remain separate from full wire
triples and selected publication state. The164-call schedule retains empty
continuations and the12 stale-scratch tiny blocks.

`frames.rs` references the original51 scheduled and seven whole-buffer C frames;
frame bytes are not regenerated or duplicated here. The Rust adapter checks
frame fingerprints, header/block boundaries and raw/RLE payloads, then separately
tests full compressed-frame decoding against every original source. Independent
candidate replay and table reconstruction never call the production parser.
The13 incomplete header probes are excluded. Complete entropy/owner traces
remain available in the original NDJSON collection.

The converter verifies the fixed checker identity before importing it, validates
the complete original collection and regenerates the source recipes before
packing any record. Normal and optimized Python use the same explicit checks.
No C compiler, executable, JSON dependency or subprocess is used by the Rust
reader, and no production dependency is added.

    python3 scripts/pack_double_fast_frame_trajectories.py --check

The `ZDFTR01\0` binary starts with51 case and160 positive-block counts. Each case
and block has a checked u32 byte extent. All integers are little endian. A case
contains its ASCII name, eight parameter/reserved bytes, target length, source
blob, source/frame/whole-frame FNV64 fingerprints, schedule and block records.
Each block records its call/start/length/floor, kind/fresh flags, four repeat
triples, two separate ordered table delta/hash records, and optional candidate
raw/wire repeats, trailing length, literal blob and expanded LL/ML/distance/
offBase sequences. C table indices retain bias2 with zero denoting empty.
Bounds, ordered identities, complete record ends and source coverage are checked
before use. FNV fingerprints supplement semantic replay; provenance uses SHA256.

These observations derive from facebook/zstd v1.5.7 under its BSD3/GPLv2 dual
license. Attribution and source identities remain in
`tests/upstream/double-fast-frames/NOTICE` and its immutable manifest. This is an
adapter checkpoint, not public DoubleFast encoding or complete upstream parity.
