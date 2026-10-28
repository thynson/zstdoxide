# Checked frame-sink measurements

This benchmark measures the isolated append leaf, not a complete compression API.
Both primary and independent repeat rebuilds retain safe/unsafe executables from
identical leaf bytes. Eleven deterministic cases include zero/one/header/page/
maximum-block copies, nonzero written prefixes, full-buffer empty append and two
short-destination rejections. Every executable has55 retained samples, five per
case, with deterministic8..1024 iterations.

Caller allocation/fill and prefix setup are outside clocks. Only checked append
is timed, including validation, copying when admitted and position publication.
Full bytes, guards and unchanged failed appends are checked outside clocks. Core-
only component proof establishes no allocator dependency; qualification rows are
result identities, not dynamic allocation counters. Two fresh process inventories
precede each sequential campaign after all recorded task CPU gates finish.
Whole-desktop idleness, statistical significance or general unsafe benefit is not
claimed. Tiny and rejected operations can be dominated by clock overhead.

| Case | Primary unsafe / safe | Repeat unsafe / safe |
| --- | ---: | ---: |
| after-block | 0.9758 | 0.7293 |
| after-header | 0.9843 | 0.4926 |
| block | 0.9900 | 0.5209 |
| empty | 0.9648 | 0.4313 |
| empty-after-full | 1.0178 | 0.8972 |
| header | 0.9702 | 0.4359 |
| one | 1.0095 | 0.4517 |
| page | 0.9166 | 0.5175 |
| short | 0.9973 | 0.5045 |
| short-after-prefix | 1.1476 | 0.9644 |
| small | 0.9978 | 0.4414 |

Equal-weight geometric means across nine nonempty requested-source cases are 0.9972 primary and 0.5429 repeat. This aggregate includes two rejected appends; the two zero-length cases remain separate. Lower ratios mean lower observed median latency on this host.

The archive contains exact source, all four executables, eleven identical result/guard rows per build, all raw samples, process observations and the initial benchmark-only cfg(test) preparation rejection. Production leaf bytes never changed. Evidence: `evidence/2026-10-27-frame-sink-benchmarks.json`.
