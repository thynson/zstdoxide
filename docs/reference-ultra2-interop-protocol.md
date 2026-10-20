# Bounded public Ultra2 interoperability protocol

This additive campaign retains all 1345 Ultra predecessor source/request pairs.
It changes only strategy-9 admission under the source-aware v8 policy, with
independent pinned-preset resolution of every old strategy-9 tuple and every new
request. New sources include all distinct observed seed requests above the public
64-byte floor, six stale-ring residues, default levels 16/17, explicit aliases,
minimum/target boundaries and source/geometry resource rejections. Private tiny
or skipped-seeding controls do not widen the public contract.

The unchanged greedy-frame transport and public C adapter are reused. The request
contains level, window, chain, hash, search, minimum, target, strategy and size
hint followed by source bytes. Caps remain 4 MiB source, 5 MiB frame and 256 MiB
aggregate regenerated source. All 39 malformed requests are replayed against
normal C, fully ASan/UBSan C, safe Rust and all-feature Rust. All 26 C translation
units use the selected build instrumentation; no hidden support library replaces
them. LeakSanitizer remains disabled on this host.

For admitted cases, normal and sanitized C decode both Rust frame variants, and
both Rust variants decode both C variants. Rust safe/all responses must be exact,
and normal/sanitized C responses must be exact within that pair. Independent
replay checks complete source bytes, resolved parameters, equivalence groups and
all predecessor successful Rust responses. No cross-implementation entropy-byte
identity or observation of C's internally applied scalar tuple is claimed.
Complete commands, dependencies, sources, environments and binary requests and
responses must be retained and independently audited. Source recipe checks are
hypotheses; public dispatch and native execution remain separate gates.
