# Public Ultra recipe hypotheses and preserved predecessor frames

The immutable manifest preserves all 1269 previous public source/request pairs
and adds 76 Ultra cases. Independent source resolution predicts 1222 successful,
108 resource-limited and 15 unsupported requests, with 16 exact request-equivalence
groups. These are source-only hypotheses; no native execution is implied.

Normal and optimized recipe controls each reject 15 forgeries without children.
Semantic mutations bypass the manifest digest and must fail full independent
regeneration. The 4636 predecessor encoded-response identities retain all four
original successful encode responses for each of 1159 original cases, including
complete frames and response parameters. They are independently reconstructed
from the preserved lossless Optimal capture archive. Comparison against a future
Ultra native capture is a separate check.

Use `scripts/check_ultra_interop_predecessor_v2.py --controls` for the predecessor
projection. The initial checker is retained: its 32 MiB archive-index read bound
rejected the actual 55635394-byte pinned index before comparing any data. The v2
checker verifies that exact index length and the unchanged full archive hash.
