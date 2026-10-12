# Reference admission policy, version 2

This source-only successor layer preserves the Module12 C selector observations,
C preset table, original92 interop recipes and historical admission results.
It records the intended bounded Fast/DoubleFast admission before changing public
execution. It does not run a codec or count missing C executions as successes.

The frozen81 selector cases retain every source byte, source-prefix decision,
capacity observation and advanced-branch nonconsumption. Historical policy is
57 successes/20 unsupported/4 advanced; version2 predicts61/16/4. The selected
C presets establish strategy identity; their recorded values are before known
source adjustment, and are not presented as applied scalar observations. All
supported presets in this fixed corpus already fit their strategy's resource
caps before size reduction.

The original92 interop inputs retain their historical Rust-resolved tuples,
source hashes and expected outcomes. Historical79 successes/11 unsupported/
2 resource errors become85/4/3. Six formerly rejected cases need new positive C
execution: three level2 size-gap cases, levels0/3 on65 bytes and explicit
strategy2 on4096 bytes. The oversized strategy2/hash19 case changes from
unsupported to a resource error. Its request never becomes supported merely
because DoubleFast exists. Historical C records explicitly say these rejected
cases were not run; that fact remains unchanged.

The original fuzzer loops retain all levels and source sizes: empty -1..22
moves from3 to5 admitted profiles, and the30-byte initial-match source at
levels1..19 moves from2 to3. Context reuse, destination-capacity failures,
mutable decoder settings, advanced parameters and in-place behavior remain
separate obligations. These expectations do not complete any official source
unit or establish CLI equivalence.

`fuzz.tsv`, `interop.tsv` and `loops.tsv` have fixed SHA256 identities and keep
historical and successor columns together. The offline checker verifies input
identities before importing the historical validators, reconstructs original
sources, preserves the historical counts and regenerates the successor policy.
The policy checks actual strategy before operational resource caps. Fast uses
window<=20/hash<=18 and no chain table; DoubleFast uses window<=21/hash<=18/
chain<=18. Other strategies remain unsupported.

    python3 scripts/check_reference_admission_v2.py --check

Future live tests must compare their fresh resolution and result with this
separate policy, and new positive C interoperability must be recorded additively.
The original Module12 archives and fixture manifests remain historical evidence.
