# Initial direct DoubleFast observations

Fifteen fixed, constructed inputs were captured on 2026-10-12 using unchanged
facebook/zstd v1.5.7 DoubleFast. Each file contains metadata, two complete
observations with different physical padding, and a completion marker. The
normal and scoped ASan/UBSan builds produce identical complete stdout. This is
60 C case executions: 15 recipes times 2 padding patterns times 2 builds.

Inputs are reconstructed from `scripts/oracles/double_fast_direct.py`; the
pre-execution recipe manifest records their exact source/packet identities.
Every table has a complete zero-initialized before-image reconstructed from
ordered seeds, sorted changed cells, and before/after LE32 fingerprints.
These are final table observations, not a chronological store trace. An
independent Python replay checks literals, long markers, offsets, overlap,
complete source reproduction, wire repeats and both table maps.

Observed boundaries include no sequence or table change for fresh7/8/9;
fresh10 writes one cell in each table; prefix9/10 match their entire blocks.
Current long/short candidates at floor0 produce lengths16/4 respectively.
The isolated later-at-floor case retains a four-byte short match, while its
above-floor counterpart uses a fifteen-byte match after one literal. Backward
extension reaches floor0 with a thirty-two-byte match. The minMatch3 and
ignored-search/target controls agree with period4 after removing only their
explicitly different input scalar fields. These findings apply to these
constructed states and do not assert a whole-context insertion trajectory.

Run `python3 scripts/check_double_fast_tiny.py --check` without any C toolchain.
For a fresh capture, use `python3 scripts/capture_double_fast_tiny.py --upstream
/path/to/pinned/zstd --output /tmp/new-double-fast-tiny`; the driver requires
the recorded support-archive identity and explicitly compiles unchanged
DoubleFast in both modes. See `docs/reference-double-fast-adapter-review.md`
for the pre-execution allocation review and `docs/reference-double-fast-validation.md`
for evidence and limitations. Ordinary frames, dictionaries, wider directed
families and public Rust DoubleFast compression are separate work items.
