# Frozen ordinary binary-tree observations

245 source-only recipes, normal/instrumented builds and two padding passes.
`artifacts.bin.gz` losslessly stores the canonical pack described by `capture.json`;
its decompressed length and SHA-256 are unchanged. `scripts/check_tree_frames.py`
bounds decompression and independently replays every observation without children.
Fifteen partial headers are not complete frames. Public admission is separate.

The original archive identity in `capture.json` and the initial evidence report
identifies the capture container. Repository storage uses
`docs/evidence/2026-10-13-tree-frames-storage-v2.tar.gz`: content-addressed chunks
retain all 4,117 original members, including compiled binaries, both padding
passes, both builds, the rejected v1 audits and accepted v2 audits. Extract that
archive and run its `reconstruct.py` with a fresh destination to restore every
member byte. The original gzip container metadata is not reproduced.
