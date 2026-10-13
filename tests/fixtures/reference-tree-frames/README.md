# Ordinary binary-tree trajectories and complete C frames

`cases.bin` is a bounded test-only projection of the separately audited ordinary
C tree capture: 245 cases, 531 nonempty blocks, 365 complete frames and 7,640,096
source bytes. Its `ZBTFR01` envelope uses a checked strategy-6 byte and three
reserved zero bytes before each ordinary observation body. No Rust encoder or
match finder generated expectations. Fifteen partial headers remain in the
original collection and are excluded from this complete-frame fixture.

Each length-prefixed case contains source/frame hashes, schedules, explicit tree
geometry, before/after repeat state, complete table deltas and candidate literals
and sequences. A table edge marker of raw value 1 is allowed only in the larger
(odd) edge cell. Raw zero means empty; positions use the original +2 bias.

Reproduce with `python3 scripts/pack_tree_frame_trajectories.py --check` and
optimized Python. The generator pins the independent collection checker, which
replays all original records with no child or codec execution. `provenance.json`
binds source, generator, checker, manifest and pack identities. The original
collection retains full observations and raw evidence not needed by this binary
projection. C and Rust may encode different valid entropy bytes.
