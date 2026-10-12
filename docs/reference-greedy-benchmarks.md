# Bounded reference Greedy measurements

## Frozen workloads and contract

The separate `greedy_benchmark` example and `benchmark_greedy.py` driver retain
the previous benchmark sources unchanged. Four sources reproduce the exact
DoubleFast inputs using xorshift64 seed `0x5eedfa5720261009`: far-history
(2,228,224 bytes), skewed (32,768), history (393,216), and mixed (524,288).
Their independent SHA256 identities are, in that order:

```
71ba4eaf6ed525768048f439ba3341e20f8a17c52b1503e540aa508d5a98a78c
613ad2caf8e6b161d48f0ce4aee8f29eaa1c45b9978876a4f9577e149ef607b2
9cc84f81533a4a4d2e16b59f04213ff5da07e716391b0d8d6314d32345937c98
b2153d35b896544d8086e4bb03600fdc07d3e84723a26545a878a45c7a275ce3
```

Each source uses eight profiles in this fixed order. The six explicit Greedy
requests start from level 3, override strategy to Greedy and minimum match to 4,
and set every listed dimension. Target zero retains the level-3 default zero.
Known-source adjustment follows the overrides; there is no dictionary.

| Profile | Window log | Hash log | Chain log | Search log | Target |
| --- | ---: | ---: | ---: | ---: | ---: |
| level-1 | reference level 1 | preset | preset | preset | preset |
| level-3 | reference level 3 | preset | preset | preset | preset |
| chain-4 | 14 | 15 | 14 | 4 | 0 |
| chain-8 | 14 | 15 | 14 | 8 | 0 |
| row-16 | 21 | 12 | 6 | 1 | 0 |
| row-32 | 21 | 12 | 6 | 5 | 0 |
| row-64 | 21 | 19 | 6 | 6 | 0 |
| row-capped | 21 | 19 | 30 | 30 | 131072 |

These are 32 cases, including 24 Greedy cases. Chain search visits at most 16 or
256 candidates. Row widths are 16, 32 or 64, with search budgets 2, 32 or 64.
The last row profile uses the same effective layout and search as `row-64`:
its unused chain dimension, capped visits and ignored target must leave complete
frame bytes and allocation accounting identical. The 32 KiB source adjusts row
window/hash maxima to 15/16, history and mixed to window 19, and far-history to
window 21. Chain dimensions remain 14/15/14 on every source. Source identities
and resolved tuples are checked independently before accepting observations.

The eight Fast/DoubleFast controls must retain complete frames from the frozen
pre-Greedy baseline. Allocator counters must also match when target and compiler
match the historical qualification. The three older Fast baseline frames remain
an additional independent control. Baseline generators and archives are not
rewritten to accept current outputs.

All source, frame and allocator-survey records in this std-only harness own
initialized Vec storage. Chain owns independent usize head/link tables; row owns
usize positions and u8 tags. Existing opt-in leaf backends alone change access.
No new unsafe storage or codec API is introduced. The separate TrackingAllocator
survey checks exact output bytes, one returned allocation, no violations and
complete owner release. Peaks cover allocator-visible requested/returned bytes,
excluding allocator metadata and transient old/new overlap during growth.

Default execution is clock-free. Timed execution measures only Global one-shot
request resolution, allocation, compression and output destruction. Two untimed
calls first establish deterministic complete output. Each case has five samples
of at least 100 ms, with a separate complete-output comparison after each.
Source construction, decoding, hashing, saved files and allocator surveys remain
outside clocks. All requested executables are built before sampling.

Validate safe, each of fourteen individual unsafe features and all together
before timing those sixteen configurations. A separate safe/all repeat retains
every sample. The coordinator must record completion of task-owned correctness,
Miri and build jobs before each timed campaign; the driver must require that
barrier evidence. Unrelated desktop work, fixed order, frequency and thermal
changes remain limitations. A backend unused by a profile cannot be assigned a
causal benefit from that profile's timing. No C throughput, C ratio equivalence,
universal unsafe speedup or foreign-native performance is implied.

## Verified harness

The separate example and Python driver pass all 32 clock-free cases in safe/all
builds. Complete frames and all allocation counters agree between backends.
Eight pre-Greedy Fast/DoubleFast frames and qualified owner records remain exact;
the three earlier Fast controls also pass. Every source's row-64 and row-capped
frames and owner counters agree despite their different resolved requests.
Formatting and strict example Clippy pass in safe/all builds.

Normal and optimized Python independently replay the complete evidence. Each
also accepts two positive captures and rejects 43 changed captures with child
creation blocked and zero attempts. Controls cover commands, environment,
features, source identities, exact parameters, allocation leaks/owners, saved
frames, summaries and inappropriate smoke clocks/barriers. Coherently changing
both backends' Fast/DoubleFast bytes or allocator peaks still fails the historical
baselines; coherently changing only capped-row output or owners fails its paired
equivalence. The original capture passes again after the controls.

The [harness evidence](evidence/2026-10-12-greedy-benchmark-harness.json) and
[archive](evidence/2026-10-12-greedy-benchmark-harness.tar.gz) preserve all nine
commands, complete frames, surveys, source identities, independent controls and
their exact modified inputs. The 30,121,924-byte archive uses 4-KiB deduplication
for 7,640 files representing 3,101,167,916 bytes. Every member and reconstructed
file was verified. Compiled outputs are omitted with executable hashes retained;
archived reconstruction code is not run automatically. The design document in
the archive predates this checkpoint, and its measurement contract is unchanged.

The default run has no clocks. Timed runs additionally require a recorded CPU
barrier containing passed, unchanged platform, full-matrix and all-backend
clock-free evidence. This records the coordinator's task-owned job observation;
it does not prove that unrelated desktop work is idle.

```sh
python3 scripts/benchmark_greedy.py /tmp/new-greedy-smoke --configs safe,all
python3 scripts/benchmark_greedy.py /tmp/new-greedy-smoke --validate-only
python3 -O scripts/benchmark_greedy.py /tmp/new-greedy-smoke --validate-only
```

All-backend clock-free validation and measured campaigns remain pending.
