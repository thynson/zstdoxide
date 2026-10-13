# Generic adapter reuse for public tree requests

The existing `examples/reference_greedy_oracle.rs`,
`scripts/oracles/reference_greedy_frames.c` and
`scripts/oracles/greedy_interop_protocol.py` are reused byte-for-byte. Their
reviewed `ZGOI0001` transport carries strategies 1..9 and forwards public
requests. They contain no tree implementation or policy prediction. Their
39 malformed packets remain unchanged; admission errors are successful transport
responses, distinct from malformed-input exits.

New source-only recipes and v5 mappings predict bounded strategy 6 after
independent known-size adjustment. The successor controller uses schema 5 and
retains all 886 historical requests. Its auditor compares every reported tuple
to the frozen independently resolved/historical tuple, preserves earlier source
and scalar identities, and applies explicit tree W21/H19/C19/S8 admission.
Three- and four-digit case identifiers are required by the 1,197-case inventory.
All 26 C codec units are compiled separately under both normal and ASan/UBSan
flags; no cached support archive supplies uninstrumented units. The exact generic
adapter sources and dependency identities are pinned before execution.

This is a protocol/data-flow review. It makes no claim that the new public
entry point has run, that sanitizer checks have passed, or that Rust emits the
same entropy bytes as C. Capture, offline forgery checks and final configuration
checks remain separate work items.
