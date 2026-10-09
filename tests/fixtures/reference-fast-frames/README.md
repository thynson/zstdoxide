# Rust-readable ordinary Fast candidate trajectories

This is a derived view of the immutable ordinary-frame observations in
`tests/upstream/fast-frame-publication`, captured from unchanged facebook/zstd
v1.5.7 revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`. It contains 23
ordered recipes, 64 nonempty block observations and no partial-header captures.
The original source attribution, selected license (`LICENSE.zstd`), reviewed C
setup, actual frame decoding and sanitizer qualifications remain in that source
collection. No new C execution or Rust parser result is implied by packing it.

Run `python3 scripts/pack_fast_frame_trajectories.py --check` to validate source
hashes and semantics and reproduce the complete derived bytes without writing
anything or starting C. `--write` explicitly writes this derived binary and its
provenance; it never changes the source collection. The provenance binds the
original gzip records, complete frame pack, manifest, C helper, offline importer,
pure inspection support, packer and safe Rust reader by SHA-256.

`cases.bin` uses magic `ZFTRJ01\0`, followed by little-endian u32 exact case and
nonempty-block counts. Every case and block body has a u32 byte length. A case
stores its u16-length ASCII name, window/hash/minimum-match bytes and a zero
reserved byte, target u32, source-length u32, C source FNV64 u64, counted u32 call
lengths, and counted block bodies. Call lengths preserve empty Continue calls;
blocks represent only nonempty calls. Source bytes are not duplicated in the
binary: the Rust reader independently reconstructs each ordered fixed recipe.

A block stores four u32 fields (call, source start, source length, normalized
retained prefix floor), selected kind and fresh-candidate u8 fields, a zero u16,
and four u32 triples (wire before/after and raw C parser before/after). Counted
table changes contain bucket/old/new u32 triples, followed by the before/after
u64 FNV64 values over complete little-endian C u32 table entries. Raw C zero
means invalid; a valid C source index has bias two. A fresh candidate appends raw
parser and full semantic repeat triples, trailing-literal count, a u32-length
literal blob including its trailing suffix, and counted LL/ML/distance/offBase
u32 records. A tiny stale candidate appends nothing, preserving its absence
rather than inventing a new empty sequence list.

`tests/support/fast_frame_oracle.rs` is a safe `no_std`+`alloc` test reader. It
checks limits before reservations, complete body/file exhaustion, exact names
and counts, generated source length/FNV64, source schedule, block-start retained
floors, literal/match byte reconstruction, repeat selection and publication,
ordered hash deltas, logical key bounds and independent buckets/table hashes.
The raw third C parser cell is deliberately distinct from the third semantic
wire repeat. Source and metadata allocations are bounded and fallible.

Consumers may replay persistent parsing under the captured C publication policy
to compare later candidate trajectories. A production Rust encoder must instead
publish only its own selected compressed blocks; its independent entropy choices
need not reproduce every C block selection. This adapter contains no compressed
frame decoder. The separate normal Cargo tests decode all 27 saved completed C
frames, including four aligned whole-buffer comparisons.
