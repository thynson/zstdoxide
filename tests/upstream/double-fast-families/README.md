# Wider constructed DoubleFast observations

The original 168 source-only recipes cover fourteen families and remain fixed
at their pre-execution identities. One separately frozen tail-extension recipe
adds an observed long-literal marker. Every one of the 169 files contains native
metadata, two complete padding observations and a completion marker. Normal and
scoped ASan/UBSan outputs match exactly: 676 C padding-case executions in total.

Every sequence/literal/repeat state and both complete table maps are checked
independently. The manifest retains isolated witnesses for shorter/tied/longer
lookahead, current-long priority, adjacent long-key collisions, the step3/4
conditional final bucket, repeated zero-literal sequences, floor equality,
repeat availability and both long-length marker kinds. These are final states
and isolated input consequences, not a chronological trace or ordinary frame
publication. The remaining support archive is uninstrumented and LeakSanitizer
is disabled; helper, unchanged DoubleFast and inline headers are instrumented.

The original `long-literal-candidate` produced no sequence and is preserved.
Its largest final sampled position is block-relative 65298, before the matching
tail at 65536. The separately named `long-literal-extended-tail` appends 896 bytes
of the same value, extending that tail from 128 to 1024 bytes. It produces a
65554-byte literal run with raw LL18 and long-literal marker position0, followed
by two independently replayed sequences. This refinement does not replace or
relabel the original result.

Run `python3 scripts/check_double_fast_families.py --check` without a C toolchain.
The checker pins fixture/tool/input identities before importing recipe code,
reconstructs the original and refined sources, rejects catalog changes, and
replays all observations. Original capture commands and the separate refinement
capture are retained in the evidence archive. `capture_double_fast_families.py`
regenerates the original 168 cases; the retained refinement script reuses those
exact compiled helper binaries and verifies their identities before invocation.
See `docs/reference-double-fast-families.md` for the inputs and observed scope.
