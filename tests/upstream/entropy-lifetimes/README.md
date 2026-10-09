# Original dictionary entropy lifetime input

This fixture preserves the default dictionary used by facebook/zstd v1.5.7
`tests/zstreamtest.c:1831-1881`, at revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. The test and data generator are
attributed under the BSD-3-Clause option in root `LICENSE.zstd`.
`provenance.json` records source, generator, dictionary, input and frame hashes.
Only the 49,152-byte dictionary is stored; source buffers and reference frames
are regenerated outside the repository.

## Exact dictionary and source construction

The original `basicUnitTests` receives seed zero from `main`. Before any change
to that seed, it generates a 10 MiB buffer with `RDG_genBuffer` at match
probability 0.50 and literal probability zero. `FUZ_createDictionary` trains
from the first 3,495,253 bytes: 213 complete samples of 16,384 bytes and one
sample of 5,461 bytes. `ZDICT_trainFromBuffer` receives a dictionary capacity
of 49,152 bytes. Its result has ID 2,004,100,296 and a 147-byte entropy header.
This pins the original default training profile; upstream's other `-P` values
are not claimed by this fixture.

The earlier offset-at-window-boundary test at line 1652 changes `seed` twice
for each of 9,999 sequences and each window log from ten through the chosen
maximum. Other earlier randomized operations use the distinct `coreSeed`.
The two later `RDG_genBuffer` calls receive the same resulting seed by value:

| Preceding test mode | Maximum window log | RNG draws | Seed at line 1831 |
| --- | --- | --- | --- |
| `--no-big-tests` | 26 | 339,966 | 437,141,494 |
| `--big-tests`, the default on 64-bit hosts | 29 | 399,960 | 3,173,672,436 |

For each mode, the source is exactly:

1. 131,072 bytes from `RDG_genBuffer(size, 0.0, 0.0, seed)`.
2. 130,816 bytes from a fresh `RDG_genBuffer(size, 0.05, 0.0, seed)`.
3. Serialized dictionary bytes `[256..384]`.
4. Serialized dictionary bytes `[128..256]`.

The final size is 262,144 bytes. The ranges address the serialized dictionary,
not its content suffix; the latter range overlaps the entropy header. Rust
reuses the independently verified `tests/upstream/support/mod.rs` RDG and
FUZ generators. Normal tests check the exact scalar state and independent C
source hashes. The explicit oracle test compares every generated source byte.

## Setup and coverage limits

The C helper uses the original full dictionary, by-reference CDict preparation,
`ZSTD_getCParams(3, 0, dictionary_size)`, `ZSTD_compress_usingCDict`, and
`ZSTD_decompress_usingDict`, and compares the complete decoded output. The
initial C parameters report window log 17. The resulting frames are single
segment with an actual 262,144-byte window. Rust streaming tests therefore use
window log 18; copying the initial log 17 would expire dictionary references
before the final source ranges. The one-shot Rust frame has the same actual
window naturally.

Rust prepares an owned shared formatted dictionary and exercises its independent
Greedy, Chain and Lazy policies. It does not expose numeric level 3 or borrowed
by-reference CDict ownership. Passing the C baseline does not implement those
missing Rust API contracts. The Rust tests verify exactly two blocks, a raw
first block, a compressed second block, complete output equality, and actual
dictionary-content dependence by changing only dictionary content while keeping
its ID, entropy tables and repeat offsets intact.

The optional C oracle regenerates and checks the pinned dictionary, executes
both original C campaigns, decodes those C frames with Rust, and independently
decodes six Rust streaming frames plus two one-shot frames. This checkpoint
does not include the separate six-byte flush campaign at line 1884.

## Reproduction

With a clean checkout of the pinned reference and its static library built:

```sh
export ZSTD_UPSTREAM=/path/to/zstd-v1.5.7
cargo test --release --test upstream_entropy_lifetimes -- --ignored --nocapture
```

That test compiles the helper and original `programs/datagen.c` into an external
temporary executable. It links C only into that test process, never into the
Rust codec, and removes its own temporary files. The complete compile and
generation argument lists are preserved in `provenance.json`.
