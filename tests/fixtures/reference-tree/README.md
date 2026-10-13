# Bounded direct binary-tree observations

`cases.bin` preserves the first normal padding pass from 103 independently audited
Zstandard v1.5.7 direct search recipes. It includes all thirty prior tiny controls.
The pinned upstream revision and archive, packet and complete stdout hashes are
in `provenance.json`. The binary format and bounds are specified in
`docs/reference-tree-fixtures.md`. Run `python3 scripts/pack_tree_fixtures.py`
to reconstruct and compare every projected byte without running archived code.

Source recipes are deterministic and generated locally. Search observations are
derived from facebook/zstd v1.5.7, copyright Meta Platforms, Inc. and affiliates,
under its BSD-3-Clause or GPL-2.0 licensing; the retained BSD license is `LICENSE.zstd` and source
identities are in `tests/upstream/manifest.json`. Reference sources remain external.
No production binary-tree implementation or public strategy support is implied.
