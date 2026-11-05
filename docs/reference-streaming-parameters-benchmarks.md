# Reference-stream parameter lifecycle measurements

This bounded campaign measures complete frame work under four lifecycle
operations using identical requests, source, options and caller buffers. It
does not measure native zstd or isolate the nanosecond cost of a setter.

## Protocol

Twelve cases combine six starting reference families and two repeated-text
sizes (65 and 4,200 bytes). Successive samples alternate between the starting
and next family, with requested window logs 17 and 16, hash/chain logs 14,
an exact pledge and checksum. The resolved window can shrink for these small
sources. Each mode sees the same alternating requests and source.

Three warmed contexts use session reset, parameter-plus-session reset, or
reset-all followed by options/request replacement. The comparator constructs
and drops a fresh context each sample. Timers include those operations plus
whole-frame pumping into preallocated output, including fresh destruction.
Construction of reused contexts, warm decoding, output equality and digests
are outside timers. Every output equals the fresh warmed frame for that
request; all variants decode to the source. Safe/all and both repeats have
byte-identical frames and ledgers.

Two rounds of forty samples rotate four-mode order after a 250 ms CPU barrier
per case/round. Runs execute safe/all/all/safe after the regression matrix:
15,360 samples total. Each case/variant/mode median contains forty samples.
The optimized test profile uses opt-level 3, debug 0, incremental disabled
and test assertions. Host is Darwin arm64 with pinned nightly-2026-10-08.
No CPU affinity/frequency lock or background host isolation was applied.
Qualification smoke timing is excluded.

## Observations

Ratios are geometric means over 24 case/variant ratios of medians. Greater
than one means the numerator took longer. These are point estimates, without
confidence bounds or universal performance implications.

| Reused/fresh mode | Safe 1 | Safe 2 | All 1 | All 2 |
| --- | ---: | ---: | ---: | ---: |
| Session reset + replace | 0.4939 | 0.4907 | 0.4676 | 0.4747 |
| Parameter reset + session reset + replace | 0.4968 | 0.4893 | 0.4894 | 0.4698 |
| Reset all + replace | 0.4916 | 0.4885 | 0.4775 | 0.4669 |

| All/safe mode | Repeat 1 | Repeat 2 |
| --- | ---: | ---: |
| Session reset + replace | 0.8840 | 0.9178 |
| Parameter reset + session reset + replace | 0.9197 | 0.9109 |
| Reset all + replace | 0.9068 | 0.9067 |
| Fresh construction/drop | 0.9336 | 0.9487 |

These measurements include parser and frame work and allocator reuse. They
cannot attribute an aggregate difference to a particular reset method or an
unsafe operation. No claim extends to prefixes, large/unknown input, other
allocators/platforms, LDM, workers or unsupported strategies.

The matching JSON receipt retains every median and source/build/output hash.
The archive contains the harness, controllers, build/validation logs, raw
samples, frames and ledgers. Module 41 full isolated-feature suites remain
a separate open qualification gate.
