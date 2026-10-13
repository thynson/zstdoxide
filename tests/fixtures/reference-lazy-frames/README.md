# Checked ordinary lazy-family Rust trajectories

`cases.bin` is a test-only projection of the immutable ordinary C collection in
`reference-lazy-frame-capture`. It contains 510 cases / 1,377 positive blocks and
549 complete frames, with 22,499,784 source bytes. It executes no codec and does
not infer match selection from Rust. `provenance.json` binds the generator,
independent collection checker and exact source pack.

The little-endian envelope is eight-byte `ZLZFR01\0`, then u32 case count and
u32 positive-block count. Each case is a u32-length-delimited body beginning with
u8 depth (0..2) and three zero reserved bytes. The remaining body is exactly the
Greedy ordinary schema documented in `reference-greedy-frames/README.md`, with a
matching `d0_`/`d1_`/`d2_` name prefix. Every block, sequence, sparse owner delta,
complete fingerprint, repeat tuple, source/literal/frame extent and schedule is
preserved. The full C collection separately retains zero-call and entropy-owner
metadata omitted by the common Rust projection.

Collection/case caps are 256 MiB / 32 MiB. Nested source, frame, schedule, block,
sequence and table caps retain their checked predecessor limits. Partial-header
observations are deliberately excluded. The Rust wrapper uses bounded fallible
metadata vectors and borrows all byte spans. Shared semantic replay checks the
observations without invoking the production parser.

Regenerate exact bytes with `python3 scripts/pack_lazy_frame_trajectories.py
--check`. Existing destinations are immutable; `--write` requires a fresh output
directory and rejects symlinks or unexpected output names.
