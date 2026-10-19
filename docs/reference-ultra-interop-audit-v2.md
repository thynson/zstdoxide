# Additive public Ultra audit directory validation

The original schema-7 native capture and its full independent audit passed.
The original adversarial-control run was interrupted during an expensive repeated
replay; its incomplete result is not counted as passed. The last forged evidence,
changed artifact bytes, original scripts and interruption record are retained
under `preserved-controls-v1` and `audit-successors/v1` in the capture archive.
No native request, response, command, source snapshot or recorded outcome changed.

`check_ultra_interop_capture_v2.py` is an additive successor. It first requires
`requests` and `logs` to be canonical, direct, nonsymlink directories under the
resolved capture root. The existing label grammar permits only ASCII letters,
digits, underscore and hyphen. Every artifact path must still equal its exact
parent, generated label and fixed suffix. Those checks prove containment without
resolving the same parent path for every read. File symlinks remain rejected.
The source-snapshot and executable-path checks retain their original resolution.

Every artifact access still checks its regular-file type, exact bounded length
and complete SHA-256. Bytes are reread and rehashed; no artifact cache or skipped
validation is introduced. Source identities, reviewed production pins, exact
commands/environments/dependencies, scalar reconstruction, full frames, decoded
bytes and all 4636 predecessor encode responses retain the original checks.
Concurrent mutation of the input directory is outside this immutable-capture
workflow, as it was for the original sequential auditor.

`check_ultra_interop_capture_controls_v2.py` preserves all 50 original forgeries
and adds symlinked request/log directories and a symlinked response file. The
three new cases require their specific path-rejection diagnostic. The control
runner uses a fresh short temporary path; this changes no recorded native root
or artifact. Normal and optimized Python outcomes are recorded separately.
The original controller, adapter review, protocol and original auditor remain
unchanged, so existing capture hashes and pre-execution review are preserved.
