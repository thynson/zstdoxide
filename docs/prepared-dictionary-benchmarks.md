# Compiled-state and per-call staging measurements

Eighteen deterministic case/level pairs cover Fast, DoubleFast, chain,
BinaryTreeLazy2 and formatted-dictionary inputs. Both roles write the same
caller-owned destination with the same input, dictionary, request and output
capacity; compiled construction and dictionary preparation are outside every
clock. Each role runs two rounds of forty samples after an in-test CPU barrier,
and the safe and all-feature configurations are each run twice. Scratch
allocation is included in both roles.

Qualification ledgers are byte-identical across all four runs: eighteen cases,
one decode check each, and 2,880 timed samples per run. Fast, DoubleFast, chain,
row and the formatted entropy seed produce equal frames; the tree cases record
their measured differences as in the frame comparison.

Safe overall compiled/staging geometric-mean time ratio is
1.4373. Per-configuration ratios:
{'safe-1': 1.4373241346935601, 'safe-2': 1.3647998292176315, 'all-1': 1.4287413333198136, 'all-2': 1.4286452825116942}. A ratio below one means the compiled path was faster
on this workload. No universal backend, tree-shape, allocation or native
workspace benefit is claimed; compiled tables were built before timing and the
per-call staging path rebuilds its seeds inside each clock. See
`evidence/2026-11-03-compiled-dictionary-benchmarks.json` and the campaign archive for raw samples.
