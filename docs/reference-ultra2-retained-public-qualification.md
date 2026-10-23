# Bounded retained Ultra2 public qualification

Resolved strategy 9 admits known sources through 32768 bytes within one window,
under the existing W<=16, H/C<=12, S<=6 and target<=4095 resource caps.
Fresh sources through 16384 bytes retain their original plan and frame bytes.
Larger sources use balanced three/four-block schedules. Only the first block
runs discarded learning and rebase; later blocks use the retained single-pass
Ultra policy. Window expiry, dictionaries, numeric streaming, LDM and workers
remain outside this reference-profile extension.

## Public admission and exact predecessor preservation

All 1672 frozen requests match the independent hypotheses: 1486 successes and
186 resource-limit responses. Exactly predecessor indices 1512 and 1592 change
from resource rejection to success. The other 3182 safe/all response byte strings
remain unchanged. All 4132 capture commands pass, including 504 bidirectional
exact native/Rust decodes and 156 malformed calls returning exit 2 without a
successful response. Safe/all Rust frames agree; normal/sanitized pinned v1.5.7
native frames agree. Native and Rust entropy decisions need not be identical.
Ordinary and optimized offline audits each reject 26 effective forgeries and
accept two positive audits with child execution forbidden; their reports agree.

Sixteen focused commands qualify formatting, 14 public tests in each backend,
14 factory tests and 13 private integration tests in each backend, strict
safe/all Clippy, all nine usage examples in each backend, safe/all true no_std,
13 allocator tests and the upstream inventory. The complete-migration audit
still reports the expected 188 unported source units; it is not a parity pass.

The first focused attempt exposed an obsolete sibling test that assumed retained
Ultra2 was always rejected. Its replacement keeps the early-resource-rejection
contract by selecting a window smaller than the source. A second controller
attempt stopped after all public tests passed because its expected count was
15 instead of 14. Both failed receipts, logs and drivers are preserved; the
successful third campaign reruns the complete focused inventory.

Evidence: `evidence/2026-10-23-ultra2-retained-public-native.{json,tar.gz}`.
Archive SHA-256: `a95b599c9b68f4a2013ba61d2bbbe8dad273e4911b9ad1f9850967bc0b94621f`.
The archive independently reconstructs every included file. Unchanged fixture
payloads are restored from exact Git blobs and SHA-256 identities. All four
interop executables are embedded. Focused correctness executables remain in
verified external lossless gzip storage, with exact paths and hashes retained;
the first archive attempt rejected one gzip exceeding its per-file bound.
Full regression, final source/platform reconciliation and benchmarks remain
separate milestone obligations.
