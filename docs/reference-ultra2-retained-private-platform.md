# Private retained Ultra2 platform qualification

The complete private integration matrix supplies the frozen production and test
source inventory. A separate platform campaign preserves that inventory and adds
its benchmark-environment manifest helper and this protocol as explicit inputs.
It does not change source between feature products or interpret an earlier draft.

Strict-provenance Miri runs three named tests for i686 and s390x, each in safe and
all-feature configurations. The tests exercise checked phase/source entry and
first-block rebase, complete compressed/raw/RLE publication and reset, and all
nine exact child allocator identities across retained reuse. The planned 12
test executions target 32-bit little-endian and 64-bit big-endian interpretation.
The full native-state fixture remains separately qualified by the scoped matrix.

Safe/all thumbv6m core-plus-alloc checks exclude atomics. Two separate release
probes instantiate the retained Ultra2 plan, nine-owner state, runtime source
parsing, shared entropy, complete frame publication and destruction. Concrete
LLVM IR must define exactly one named probe; object headers must identify ARM
ELF32 little-endian. Preserve the probe, exact source hashes, command outputs and
all IR/object artifacts. The expected campaign contains eight commands: four
Miri invocations and four no-atomics checks/code-generation invocations.

These gates interpret or generate foreign code; they do not run native programs
on those targets. Public strategy admission, complete regression and performance
measurements remain separate obligations.
