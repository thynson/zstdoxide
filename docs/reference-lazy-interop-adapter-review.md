# Shared public adapter review for Lazy/Lazy2

This campaign reuses the exact three generic transport sources reviewed in
`reference-greedy-interop-adapter-review.md`. Their unchanged SHA256 identities
are recorded below; all prior evidence remains immutable. Normal and
ASan/UBSan-enabled `cc -std=c11 -Wall -Wextra -Werror -fsyntax-only` checks against
the external pinned v1.5.7 headers pass before new public C execution.

| Source | SHA256 |
| --- | --- |
| `scripts/oracles/reference_greedy_frames.c` | `e49b52366148ab6c1ee19f150cdade770c72318b391c7fce482b1dca09f7430d` |
| `examples/reference_greedy_oracle.rs` | `b982e84fe2df72dcb4ec0b5131d0fa1f88e6dd7c5c0fe29dc17559841da47f7c` |
| `scripts/oracles/greedy_interop_protocol.py` | `cbf5c0763f9c1fbd230f988f2b01c517f6af22f58c4645239303a4c75b66b344` |

The helper validates strategy values 1..9 as scalars and forwards them through
public setters. It neither restricts execution to Greedy nor implements any
search loop. Both Lazy values therefore use the same request fields, initialized
owners, guard checks and public-context lifetimes. The safe Rust adapter resolves
the request separately and invokes the public encoder. Admission is enforced by
the codec and independently checked by the new auditor; it is not inferred from
the historical adapter name. Existing diagnostic labels and transport magic stay
unchanged, preserving all 39 malformed controls and their predecessor identities.

The new recipe envelope contains 886 requests and 43 complete-frame equivalence
hypotheses, with individual source/frame bounds unchanged. A new controller and
auditor must retain exact command/source closure and instrument all 26 C units.
These source and syntax checks do not claim successful new C frames. Positive
public Rust/C exchanges and malformed runtime rejection remain the later capture
gate described in `reference-lazy-interop-protocol.md`.
