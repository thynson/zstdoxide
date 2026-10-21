# Private retained Ultra Rust wire qualification

All 18 automatic-literal policy-2 recipes now have source-bound Rust wire
qualification. Safe and all-feature captures are byte-for-byte identical across
the exact 37-file inventory: 18 sources, 18 complete frames and one observation
file. The actual wire histories contain 59 blocks: 3 raw, 12 RLE and 44 compressed.

The independent source-only replay derives 62546 parser events and all 270
published sequences under the actual Rust block kinds. It verifies complete
literal bytes, absolute positions, tails, offset intents, incoming/candidate/
committed repeat owners, frame headers, block extents, final flags, raw/RLE
payloads and complete exhaustion. All 18 block-kind histories happen to match
the earlier native hypotheses; that equality is an observed result, not an input
to the replay or a claim that entropy bytes agree.

The pinned v1.5.7 CLI decodes every safe/all frame, giving 36 exact source
round trips. The executable, its Zstandard library, external xz/lz4 provenance,
load metadata and all original/copy identities are retained. The copied CLI
loads its copied Zstandard library through its existing relative rpath; absolute
external dependencies are checked before and after. Rust decoding was performed
inside the captured test, before each artifact was written.

Both ordinary and optimized Python pass 34 effective negative controls and two
positive audits, with identical complete reports and no child execution. The
forgeries include first- and later-block repeats, literal/sequence/offset state,
coordinates, tails, kinds, boolean substitutions, case inventory, source/frame
truncation or alteration and unexpected artifacts. Repaired local serialization
does not bypass the independent source-derived comparison.

All 45 planned commands pass: four load inventories, one native version check,
two Rust captures, 36 native decodes and two controlled independent audits. The
complete current/snapshot source closure, configuration and captured artifacts
remain unchanged. The protocol is
`reference-ultra-retained-rust-wire-protocol.md`; full source, copied native
binaries/libraries, logs, captures and control reports are archived in
`evidence/2026-10-21-ultra-retained-rust-wire.{json,tar.gz}`.

These are private frame checks. Public strategy-8 admission, all isolated
storage/foreign-platform completion and final public regression are tracked
separately. Earlier failed recipe-generation helpers are unrelated to these
successful wire checks and retain their own historical evidence.

Archive SHA-256: `2732b1a1c225551fcbb6a75eb1ae6a6a623a54f6bc5474e4c9a4c34619fce0c7`.
