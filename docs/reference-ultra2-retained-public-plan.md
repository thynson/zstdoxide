# Retained Ultra2 public hypotheses

The additive source-only manifest fixes 1672 requests: all 1593 retained Ultra
predecessors and 79 additions. It predicts 1486 successes and 186 resource
rejections. The additions contain 61 successes and 18 resource rejections.
These are hypotheses; public retained Ultra2 is not yet admitted.

Exactly two predecessor classifications change: index 1512,
`retained-resource-ultra2`, and index 1592, `retained-ultra-resource-ultra2`.
Both contain 32768 uniform bytes, source SHA-256
`b217b65e6f205f41b3fb8ef90cf7c44da93f630ca03965273485bbb21a5cccf5`.
Their respective requested levels are 11 and 13; both explicit strategy-9
requests resolve to W15/C12/H12/S6/minimum3/target4095/strategy9 and are proposed
to move from resource rejection to success. Every other predecessor source,
request, resolved tuple and classification remains identical. Public execution
must later preserve all 3182 safe/all responses for the 1591 unchanged cases.

The additions cover retained source recipes, balanced block-count boundaries
16385/24576/24577/32768, uniform/periodic/noise sources, default levels 15/16,
minimums 3..7, targets 0/1/4095, geometry endpoints and resource rejections.
Sources must fit one resolved window, W<=16, chain/hash<=12, search<=6 and
target<=4095. Default levels retain independently resolved strategies and bounds;
no explicit strategy is imposed on a default-level recipe. Window-14 requests
at 16385 and 32768 remain resource errors. Existing strategy families retain
all earlier admission rules.

Ordinary and optimized Python generate byte-identical manifests and matching
control reports. Both reject 15 malformed policy inputs and pass 32 source,
geometry, sibling-strategy and minimum/target controls with child execution
forbidden. All upstream admission inventory rows remain unchanged. Exact
input origins and source snapshots include the complete transitive predecessor
models, protocol documents and fixture identities. The two new helpers were
qualified in an external snapshot and then transferred byte-for-byte into the
repository; the archive includes the exact promotion receipt.

Manifest: `tests/fixtures/reference-retained-ultra2-public/recipes.json`, SHA-256
`8208d9871ce2ed9d508b704178b7df8ed5c9d29665c70c6765218d0d2548afaf`.
Evidence: `docs/evidence/2026-10-22-ultra2-retained-public-recipes.{json,tar.gz}`,
archive SHA-256 `3c8a59e98256732fc5cd702697deb82061f259194f7c39e119254e6fed9b6d90`.
Public execution must separately qualify all 63 new/transition successes with
pinned normal/sanitized native encoding and exact bidirectional decoding.
Safe/all Rust frames must agree; native/Rust entropy choices need not match.
Private integration/platform gates, final public regression and measurements
remain separate obligations.
