# Read-only Ultra offline integration

`scripts/check.sh` now includes additive v7 admission, public Ultra source-only
recipes, scalar prices, frequency lifecycles, scripted paths and the complete
26-frame baseline. Historical check commands and fixtures remain unchanged.
The path wrapper performs read-only reconstruction and equality validation;
it does not run the native capture or rewrite its projection.

The six added commands pass under normal and optimized Python, and the shell
driver passes syntax checking: 13 successful commands. This focused integration
check is separate from the full Rust feature matrix and does not claim a fresh
execution of every historical command in `check.sh`.

The archive retains complete command logs, the exact integration driver and
source snapshots, with every member independently rehashed. Evidence:
`evidence/2026-10-19-ultra-offline.tar.gz`, SHA-256
`e8013b3d122a4f69e95b6e524da7c1eb802bcfde02da7dccbefe47311dad61a2`.
The adjacent JSON contains all 388 member identities. Unrelated Python sources
included in the snapshot are provenance, not additional executed checks.
