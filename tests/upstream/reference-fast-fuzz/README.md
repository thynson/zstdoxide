# Reference Fast fuzz selector observations

This collection records only the selector prefix of facebook/zstd v1.5.7
`tests/fuzz/simple_round_trip.c`, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. Derived test code is used under
upstream's BSD-3-Clause option; see root `LICENSE.zstd`. The unmodified C
`fuzz_data_producer.c` and `fuzz_helpers.c` stay in the external reference tree.
No codec binding, compressed frames, or duplicate large input corpus is stored.

The target fixes result capacity and `ZSTD_compressBound` using the original
input length before reserving a prefix (:145–159). It then selects the branch
(:61), simple-branch level in −3..19 (:79; `zstd_helpers.c:23–24`), and decoder
setting toggle (:93). The helper calls the original producer functions in this
order. Advanced cases stop immediately after the branch; their remaining two
bytes are not interpreted as level or decoder selectors.

There are 81 deterministic packets: 55 combinations of eleven source sizes and
levels −3, −2, −1, 1, 2; eighteen other levels at 30 bytes; four exhausted
inputs; and four advanced branches. The expected **Rust** classification is
57 admitted, 20 unsupported, four advanced, including the two level-2 cases
where the 128–256 KiB tier selects DoubleFast. These counts describe the later
Rust assertion port, not compression executed by this C helper.

For controlled packets, source byte i is `(17*i+123) mod 256`. Append bytes
`[decoder_toggle, level+3, branch, capacity_reduction]` and a little-endian tail
size of four, choosing its width consistently with the complete input length.
The exact packet SHA-256 and suffix are recorded per row. Sizes 250/251 and
65529/65530 cross the one/two/three-byte prefix selector boundaries. Exhausted
packets are exactly empty, `00`, `01`, and `ff`. The importer reconstructs every
packet without a C compiler. Rows retain every remaining-byte boundary. A dash
means not selected, never zero. `compression_bound` is the actual C query for
original N, not the selected source prefix.

Normal Cargo integration may assert that a returned frame fits the captured
bound minus the selected byte reduction. That inequality does not establish
caller-owned destination capacity checks, atomic output failure, reusable C
contexts, mutable decoder maximum-block settings, overlapping in-place decode,
third-party sequence producers, or the advanced random-parameter branch.
The existing named-policy corpus remains a separate qualified adapter.

## Reproduction

```sh
python3 scripts/import_reference_fast_fuzz.py --check
python3 -O scripts/import_reference_fast_fuzz.py --check
python3 scripts/import_reference_fast_fuzz.py --upstream /path/to/zstd-v1.5.7 --output /tmp/new-selector-run --sanitize
```

Default and `--check` are offline and never launch a process. Regeneration is
explicit, requires a clean pinned checkout and its prebuilt `lib/libzstd.a`,
and writes only to a new directory disjoint from the repository and reference.
It compares complete deterministic records without rewriting captured metadata.
`--capture` is restricted to initial creation when both fixture data and manifest
are absent. Compiler/archive hashes, actual C scalar widths, source/header
hashes, exact commands, stdout/stderr and failures are recorded. Input is one
bounded binary batch with exactly 81 records and EOF: one MiB per input,
16 MiB total. All helper-owned source allocations have 64-byte guards.

The sanitizer run instruments the helper, unchanged producer, unchanged fuzz
helpers, and their included header code with ASan/UBSan. The linked archive is
not instrumented; only its version and compression-bound functions are called.
Leak sanitizer is disabled. No full-codec sanitizer or unexecuted native-platform
claim is made. Source recipes and observation bytes are architecture independent
within the checked 32-bit-int / 32-or-64-bit-size_t host domain.

The importer independently pins the original TSV SHA-256, including every C
compression-bound observation. Rehashing a changed capacity in the manifest
does not authorize a new capture. The initial successful capture used the same
C helper and recipes; the later importer change only adds this offline identity
anchor and is covered by fresh normal/sanitized reproduction.
