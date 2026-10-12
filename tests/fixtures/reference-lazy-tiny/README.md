# Frozen tiny lazy-family inputs

The 138 source-only hypotheses contain 4,449 source bytes and 25,581 packet
bytes. Each depth 0/1/2 has 46 cases: the same 36 original Greedy sources,
parameters and initial states, plus ten repeated-byte tail cases. They were frozen before C execution. A separately pinned capture now stores
real C observations; no Rust Lazy/Lazy2 codec generated these expectations.

Depth zero retains an explicit 36-case mapping to the original Greedy capture;
case IDs gain a `d0-` prefix and packets use the new protocol. The completed capture
compares every common observation and qualified owner byte with the unchanged
old artifact. Depth-one/two packet inputs use the same predecessor recipes
with their separate selectors.

The additive cases cover both chain and row tails. Fresh repeated inputs put
the initial query exactly at the strict main limit, one below it, or two below
it. Prefixed variants use eight bytes of retained history and a seed at zero.
Their recorded first-query and main-limit coordinates describe source geometry;
they do not assert that a particular speculative search actually happened.
The captured final frontiers now distinguish first/second lookahead boundaries;
they are not a chronological trace of every probe.

`scripts/check_lazy_tiny_recipes.py --check` checks the frozen helper/transport,
source generator, contract and predecessor identities before importing inputs.
It reconstructs exact metadata, packets, seed maps, predecessor mappings and
tail geometry; it rejects 39 malformed packets and checks 31 closed-form hash
boundaries without starting a child process or calling a codec. Normal and
optimized Python use the same explicit checks. The helper and all 26 C units
passed syntax review before the completed normal/instrumented execution.

`capture.json` indexes 576,102 bytes of real observations: 138 direct cases and
six strategy/build-specific fresh-context groups. All direct bytes agree across
builds. The 36 depth-zero predecessor controls retain complete maps, sequences
and exact qualified owners. Ordinary Lazy frame and public interoperability
coverage remain open.

Run `python3 scripts/check_lazy_tiny_fixture.py --check` for permanent offline
replay. Run `python3 scripts/check_lazy_tiny_capture.py CAPTURE_DIRECTORY` to
verify all 422 commands, packets, logs, objects and executables in the retained
campaign. Both commands also pass under `python3 -O`. Full evidence, raw
artifacts and the observed tail frontiers are described in
`docs/reference-lazy-validation.md`.
