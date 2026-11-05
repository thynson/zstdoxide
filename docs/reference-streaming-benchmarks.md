# Request-backed reference streaming measurements

This bounded Module 40 campaign compares the request-backed streaming owner
with the existing named-policy encoder. It does not compare against native
zstd speed or claim equivalence between their match-search policies.

## Protocol and scope

Eighteen deterministic cases combine six explicit reference families with
repeated text, 16 KiB seeded noise and repeated multi-block input beyond the
128 KiB window. Fast/DoubleFast use named Greedy as the comparator; reference
Greedy uses named Chain; Lazy/Lazy2/BinaryTreeLazy2 use named Lazy. Each pair
uses the same resolved window, exact pledge, checksum and caller chunk sizes
(4,093-byte input / 8,192-byte output). Reference table logs are explicitly
14; named policies retain their own default table/effort settings.

Contexts are warmed and reused. Timed regions include reset and complete-frame
pumping into preallocated caller buffers; construction, decoding, byte equality
and digest checks are outside timers. Every measured output equals its warmed
frame exactly. Safe/all builds and both repeats have byte-identical ledgers
and frame artifacts. All warm frames decode to their exact source.

Two rounds of forty alternating-order samples follow a 500 ms CPU barrier per
case and round. Two independent repeats run for each backend, in safe/all/all/
safe order after the feature matrix completes: 11,520 samples total. Binaries
use the optimized test profile (opt-level 3, debug 0, incremental disabled,
default test assertions) on Darwin arm64 with pinned nightly-2026-10-08
(rustc 1.101.0-nightly, LLVM 23.1.3). No CPU affinity or frequency lock was
used; background host activity is not controlled.

## Aggregate observations

Ratios are geometric means of the eighteen per-case ratios of medians.
Reference/named greater than one means the reference coordinator took longer;
all/safe greater than one means the all-feature build took longer. These are
point estimates from this corpus, not confidence bounds or universal rankings.

| Comparison | Repeat 1 | Repeat 2 |
| --- | ---: | ---: |
| Reference/named, safe | 0.3174 | 0.3062 |
| Reference/named, all features | 0.3378 | 0.3675 |
| All/safe, reference | 1.0993 | 1.0860 |
| All/safe, named | 1.0329 | 0.9050 |

Policy work, encoded size, table geometry and allocation reuse differ between
comparators. No universal reference advantage or unsafe speedup is inferred
from these aggregate values. Small effects require more isolated measurements.

## Per-case observations

Each ratio uses the median of eighty samples in that independent run. Encoded
sizes are constant across both repeats and backend configurations.

| Family / corpus | Source bytes | Reference bytes | Named bytes | Safe R1 | Safe R2 | All R1 | All R2 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Fast / text | 8379 | 73 | 73 | 0.2711 | 0.3002 | 0.2709 | 0.2741 |
| Fast / noise | 16384 | 16399 | 16399 | 0.5171 | 0.5021 | 0.5095 | 0.5185 |
| Fast / multi-block | 182400 | 87 | 86 | 0.2540 | 0.2528 | 0.2583 | 0.2608 |
| DoubleFast / text | 8379 | 73 | 73 | 0.3685 | 0.3780 | 0.5610 | 0.4141 |
| DoubleFast / noise | 16384 | 16399 | 16399 | 0.7120 | 0.6918 | 0.5811 | 0.6263 |
| DoubleFast / multi-block | 182400 | 87 | 86 | 0.4418 | 0.4217 | 0.4488 | 0.4682 |
| Greedy / text | 8379 | 73 | 73 | 0.1675 | 0.0833 | 0.1640 | 0.1868 |
| Greedy / noise | 16384 | 16399 | 16399 | 0.4715 | 0.4737 | 0.5027 | 0.4921 |
| Greedy / multi-block | 182400 | 87 | 86 | 0.3836 | 0.3745 | 0.4136 | 0.4150 |
| Lazy / text | 8379 | 73 | 73 | 0.2898 | 0.1190 | 0.1890 | 0.3279 |
| Lazy / noise | 16384 | 16399 | 16399 | 0.4242 | 0.4553 | 0.4818 | 0.5068 |
| Lazy / multi-block | 182400 | 87 | 86 | 0.3522 | 0.3403 | 0.3806 | 0.3858 |
| Lazy2 / text | 8379 | 73 | 73 | 0.1256 | 0.1986 | 0.1640 | 0.2162 |
| Lazy2 / noise | 16384 | 16399 | 16399 | 0.4488 | 0.4406 | 0.5085 | 0.5134 |
| Lazy2 / multi-block | 182400 | 87 | 86 | 0.3685 | 0.3607 | 0.3978 | 0.3943 |
| BinaryTreeLazy2 / text | 8379 | 73 | 73 | 0.1232 | 0.2078 | 0.1957 | 0.3213 |
| BinaryTreeLazy2 / noise | 16384 | 16399 | 16399 | 1.1363 | 1.1656 | 1.1495 | 1.2342 |
| BinaryTreeLazy2 / multi-block | 182400 | 87 | 86 | 0.0678 | 0.0641 | 0.0712 | 0.0781 |

Raw samples, warmed frames, ledgers, harness, controller and build logs are
in `evidence/2026-11-05-reference-streaming-benchmarks.tar.gz`. The JSON receipt
retains source/build/output hashes, commands, medians and repeat summaries.
No timing claim extends to unknown-size streams, prefix-backed inputs, other
allocators or platforms, native zstd, LDM, workers or unsupported families.
