# Captured default match-search regression

These expectations were captured from codec commit
`ba4529c9e44eee2218209bb660038e42848d11e8`, before Module 11 changed search
storage or history configuration. They are fixed observations, not outputs
regenerated with the encoder under test. The original capture, reproduction
source, commands, source manifest and independent C results are preserved in
`docs/evidence/2026-10-09-match-search-baseline.tar.gz`; its SHA-256 is
`bc8ad73fd3ea95877dc74d8fe75b7617985656d8350e5d70eff4b2c46678ca6d`.

`expected.tsv` retains all **152 original records**: 76 from the default safe
build and 76 from the build with all twelve unsafe backend features. The two
sets agree in every field except their backend label. `cases.tsv` retains the
12 source recipes and their original fingerprints. `provenance.json` identifies
the source revision, original source hashes and fingerprint conventions. Both
TSV files are unchanged copies of the archived package:

| File | SHA-256 |
| --- | --- |
| `expected.tsv` | `c12ee881fd1333e29302a8f8f4b2f475cdf714785587bdd3b72b6510f629a25d` |
| `cases.tsv` | `54d96b25eec068fc116622bd4086000a8c9df5b67be9d6649ad160c86d03daf3` |

The original independent oracle decoded every one of the 152 saved frames and
compared its complete output to the saved input. That oracle used facebook/zstd
v1.5.7, revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Ordinary regression
replay uses only Rust and committed fixtures; it does not rerun C or require any
additional dependency.

## Offline replay

`tests/match_search_defaults.rs` regenerates the recipes using the existing
pinned `tests/upstream/support/mod.rs` RDG implementation. It reuses the existing
`tests/upstream/dictionaries/formatted.dict`, whose serialized SHA-256 is
`354d5499788e19c3b2bb4f0f85d1a733c12204c3333f0f22e7f4fb01067b3281`.
No source, dictionary or encoded binary is duplicated here. Upstream-derived
RDG support and dictionary fixtures retain their existing attribution and
`LICENSE.zstd` provenance.

The five regression tests plus the RDG helper's two tests cover:

- Twelve inputs under each of Greedy, Chain and Lazy, first in a fresh context
  and then after session reset: 72 frames. Eleven inputs exceed two 128 KiB
  blocks. The six history witnesses start their repeated suffix immediately
  before, at or after the retained-history boundary, using window logs 17 and
  18. Three raw dictionaries straddle the default 65,536-start seed boundary;
  another case uses a prepared dictionary.
- The two-byte `Flush` followed by a 500-byte `Finish` case, which exercises a
  dictionary-to-frame match across a short prior block.
- Four frames in one persistent dictionary context, with
  Greedy → Chain → Lazy → Greedy selected through `reset_with_options`.
  Together these are all **76 logical frames**. Every current frame is compared
  with both corresponding archived backend records, with no skipped rows.
- Before encoder use, regenerated input and dictionary lengths and FNV-1a-64
  fingerprints, dictionary IDs and parsed content lengths. The capture's
  checksum, unknown pledge, dictionary/content-size flags, window size and
  257-byte output schedule are preserved.
- Encoded length and FNV, exact block layout, process-call counts, input/output
  totals, bounded progress, output canaries and full Rust decoding to the
  original source. Fresh and reset-reused frames are compared byte for byte;
  the first and last Greedy frames in the strategy cycle are also exactly equal.

Any supported current backend configuration replays the same two equivalent
sets. This does not claim that every individual feature configuration was
captured originally. The new test complements the existing default-byte
regression tests rather than replacing them.

FNV uses the existing test-support convention: start at
`0xcbf29ce484222325`, XOR each raw byte, and multiply by `0x100000001b3` with
wrapping 64-bit arithmetic. No length prefix or other framing is hashed. Length
plus FNV is a regression fingerprint, not a proof of equality with an absent
saved frame. The TSV preserves SHA-256 for archival verification; normal Rust
replay validates its syntax and uses FNV, while the fixture auditor handles
SHA-256 provenance. Neither hash is recalculated into an expected output by
this test. Missing dictionaries use `none` and `-`, distinct from an empty raw
dictionary.

## Capacity scope

The absolute `retained_capacity_before`, `retained_capacity_after` and
`peak_reachable_capacity` fields describe the original 64-bit host and its
construction history. They include the selected dictionary's reachable payload
and prepared tables, plus all owned codec buffers. They exclude inline fields,
shared outer control blocks and allocator metadata. The tests validate these
archived fields' accounting relationships; they do not require their numeric
values to be identical on every target or future allocation implementation.

`selected_dictionary_reachable_bytes` was measured before processing in fresh
contexts, while no codec payload was allocated. Subtracting it gives
`owned_codec_retained_before/after`, which still includes staging and entropy
storage as well as search storage. `narrow_search_retained_capacity` is explicitly
`not-recorded`: no such public metric existed at capture time, and it must not
be inferred from these totals.

Current portable retention assertions check that reset preserves allocations,
options and selected dictionaries; changing from Greedy to Chain adds the
independent cache; Lazy reuses that warmed cache; and returning to Greedy keeps
it. They make no RSS or allocator-overhead claim. All original encoded bytes
remain represented by fixed fingerprints and independent verification evidence,
avoiding 20,707,536 bytes of duplicate encoded assets in the source tree.

To run the offline regression:

```sh
cargo test --test match_search_defaults
cargo test --test match_search_defaults --all-features
```
