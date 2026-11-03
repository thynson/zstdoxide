# Streaming compiled-state measurements

Eighteen deterministic case/level pairs cover Fast, DoubleFast, chain,
BinaryTreeLazy2 and formatted-dictionary inputs. Both roles stream in 4 KiB
input fragments through the same caller-owned output buffer and the same
`CompiledStreamOptions`; each sample emits exactly one frame.

- The **compiled** role builds the compiled state and one stream once per case,
  then calls `reset` before every timed frame. Reused tables and prepared
  entropy are inside the clock; construction is outside it.
- The **per-frame staging** role constructs the same compiled state inside
  every timed sample and then streams one frame with it. Table seeding,
  prepared entropy and stream workspace allocation are all inside the clock.

Each role runs two rounds of forty samples after a 500 ms in-test CPU barrier.
Safe and all-feature configurations are each run twice. All four qualification
ledgers are byte-identical across 11,520 timed samples; every case also checks
equal frame bytes between roles and decodes the frame with the dictionary.

Primary safe staging/compiled geometric-mean ratio is **2.0064**. Per
configuration: safe-1 2.0064, safe-2 1.9700, all-1 2.0633, all-2 2.0901. A
ratio above one means the staging role took longer. The gain comes from not
rebuilding the compiled tables, so it is largest for short prefix cases
(3.0–5.4×) and smaller for multi-block or noise cases, where streaming work
dominates.

Safe-1 per-case ratios (staging ns / compiled ns):

| Case | Level | Input bytes | Frame bytes | Staging ns | Compiled ns | Ratio |
| --- | --- | --- | --- | --- | --- | --- |
| chain-interleaved | 4 | 3072 | 1049 | 130181 | 76531 | 1.701 |
| chain-multiblock | 4 | 81920 | 22 | 494067 | 266171 | 1.856 |
| chain-noise | 4 | 4096 | 4107 | 137876 | 94092 | 1.465 |
| chain-prefix | 4 | 2048 | 19 | 69449 | 22861 | 3.038 |
| double-fast-interleaved | 3 | 3072 | 1049 | 125972 | 72053 | 1.748 |
| double-fast-multiblock | 3 | 81920 | 22 | 234797 | 151583 | 1.549 |
| double-fast-noise | 3 | 4096 | 4107 | 128760 | 82658 | 1.558 |
| double-fast-prefix | 3 | 2048 | 19 | 79905 | 17124 | 4.666 |
| fast-interleaved | 1 | 3072 | 1049 | 91715 | 45503 | 2.016 |
| fast-multiblock | 1 | 81920 | 22 | 114234 | 121386 | 0.941 |
| fast-noise | 1 | 4096 | 4107 | 91105 | 55995 | 1.627 |
| fast-prefix | 1 | 2048 | 19 | 48307 | 10672 | 4.526 |
| prepared-json | 1 | 768 | 183 | 130637 | 102156 | 1.279 |
| prepared-json-repeat | 1 | 1536 | 209 | 134837 | 106016 | 1.272 |
| tree-interleaved | 9 | 3072 | 1049 | 219054 | 99524 | 2.201 |
| tree-multiblock | 9 | 81920 | 22 | 416076 | 209235 | 1.989 |
| tree-noise | 9 | 4096 | 4107 | 261250 | 138969 | 1.880 |
| tree-prefix | 9 | 2048 | 19 | 132761 | 24729 | 5.369 |

`fast-multiblock` records 0.941: the streaming work dominates and the compiled
role was slightly slower in this run, so no universal benefit is claimed. The
tree cases include its eager compiled tree and lazy per-call back-fill. These
are single-host measurements with two repetitions, not throughput guarantees.

Raw samples, ledgers, logs and the summarizer are in
`evidence/2026-11-03-compiled-streaming-benchmarks.json` and its campaign
archive.
