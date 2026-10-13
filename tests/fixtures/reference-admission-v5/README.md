# Source-only bounded binary-tree admission successor

These tables preserve the original upstream selector, numeric-loop and public
source/request inputs while recording W21/H19/C19/S8 tree admission separately
from v4. All 886 previous public requests remain represented in `lazy.tsv`.
Every newly admitted positive-level request is independently resolved from the
pinned upstream preset table by the already frozen independent resolver.

`fuzz.tsv` and `loops.tsv` retain the upstream pre-adjustment preset identity;
the public request tables retain their historical or independently reconstructed
adjusted tuples, as identified by each column. A success classification here is
a proposed bounded policy, not evidence that C was executed or that the public
Rust entry point has been enabled. Those gates are separate.

Reproduce offline using `python3 scripts/check_reference_admission_v5.py --check`
and optimized Python. No codec or child process is required.
