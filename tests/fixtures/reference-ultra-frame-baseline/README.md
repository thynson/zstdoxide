# Complete Ultra frame baseline

This immutable pack contains 26 sources, requests, resolved tuples and complete
Rust frames selected before public native capture. Default levels 13/14 cover
64/2048-byte uniform, periodic and noise inputs; minimum settings 3..7 cross
targets 1/4095; four ordinary cases cover the wide horizon, window 16 and both
alternative-path seed inputs. No timing influenced corpus selection.

`pack_ultra_frame_baseline.py` first replays the full original native auditor,
then verifies safe/all complete bytes and normal/sanitized upstream C decoding
for every selected frame. The upstream reference is Zstandard v1.5.7, commit
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Rust compression bytes are not required
to equal C compression bytes. Sources and requests retain their source-only
recipe identities; provenance binds the complete captured campaign.

The read-only checker passes in normal and optimized Python. The pack SHA-256 is
`dce7dd5dc40283759314baf18daad00a385f739a157b02130b4d4cdaa0b33c75`.
Native capture evidence SHA-256 is
`acd9c2c7de77f426f84537aada3fb9ef9a02cb38b0f7f7ba8a587e3d5791a1ee`.
The complete native archive is recorded in
`docs/evidence/2026-10-19-ultra-public-interop.json`.
