# Direct small-log eight-byte hashes

512 fixed C observations cover 64 existing keys, logs6..9, and aligned/shift1
loads from an explicitly eight-byte-aligned array. Inputs reuse the immutable
`../match-search/patterns.bin` and their original recorded generation. Both
normal and ASan/UBSan builds emit exactly the same TSV. No archived library is
linked: the observed operation is the unchanged pinned inline hash.

The compiler-free checker pins the complete TSV, helper and existing inputs,
verifies every tuple and compares independent wrapping arithmetic plus the
original log10 observations. Run `python3 scripts/check_double_fast_hashes.py
--check`. Regeneration uses the capture script and full commands retained in
`docs/evidence/2026-10-12-double-fast-hashes.tar.gz`; choose a fresh external
output directory. See `docs/reference-double-fast-hashes.md` for the reviewed
buffer boundaries and validation scope. These values do not demonstrate a
DoubleFast parser or frame encoder.
