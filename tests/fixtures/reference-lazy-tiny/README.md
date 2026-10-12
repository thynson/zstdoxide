# Frozen tiny lazy-family inputs

The 138 source-only hypotheses contain 4,449 source bytes and 25,581 packet
bytes. Each depth 0/1/2 has 46 cases: the same 36 original Greedy sources,
parameters and initial states, plus ten repeated-byte tail cases. There are no
new C observations, expected Lazy/Lazy2 outputs or Rust codec executions here.

Depth zero retains an explicit 36-case mapping to the original Greedy capture;
case IDs gain a `d0-` prefix and packets use the new protocol. Future capture
must compare the common semantic observations, not silently replace or relabel
the old artifact. Depth-one/two packet inputs use the same predecessor recipes
with their separate selectors.

The additive cases cover both chain and row tails. Fresh repeated inputs put
the initial query exactly at the strict main limit, one below it, or two below
it. Prefixed variants use eight bytes of retained history and a seed at zero.
Their recorded first-query and main-limit coordinates describe source geometry;
they do not assert that a particular speculative search actually happened.
The capture must establish each intended boundary before claiming coverage.

`scripts/check_lazy_tiny_recipes.py --check` checks the frozen helper/transport,
source generator, contract and predecessor identities before importing inputs.
It reconstructs exact metadata, packets, seed maps, predecessor mappings and
tail geometry; it rejects 39 malformed packets and checks 31 closed-form hash
boundaries without starting a child process or calling a codec. Normal and
optimized Python use the same explicit checks. The helper and all 26 C units
have separately passed syntax review; execution remains a later work item.
