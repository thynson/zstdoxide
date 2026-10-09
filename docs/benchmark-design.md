# Backend benchmark design

## Work decomposition

1. Independently audit entropy bounds, normalized counts, and terminal-state
   behavior on malformed inputs, and exercise deterministic malformed inputs.
2. Add deterministic measurements of each selectable data-structure backend
   and public whole-codec operations; verify the same output across builds.
3. Run the default build, each unsafe feature separately, and all features;
   record compiler, host, seeds, median timings, and correctness fingerprints.

The coordinating agent tracks and commits these items through `TODO.md`.

The Module 7 extension is decomposed before changing benchmark code:

1. Add checked dictionary content reads, reusable dictionary encoder/decoder
   workloads, and bounded raw training over exact pinned sample bytes.
2. Report full input/output hashes, dictionary identities and sizes, training
   resource caps, and context retained capacity; compare these values across
   safe, each of nine independent unsafe features, and the combined build.
3. Add an explicit smoke mode which verifies deterministic operations and
   metadata without collecting or reporting timings. Compile and smoke-check
   this checkpoint while codec work continues; run timed comparisons only after
   the coordinating agent freezes the measured sources.

New workload records are plain sizes, owned byte buffers, `Dictionary` shared
handles and borrowed sample slices. Dictionary construction occurs outside
timing; contexts retain their configured owned handle across reset. The trainer
owns its scratch through its normal public call, returns an owned raw dictionary,
and borrows samples only during that call. The benchmark introduces no raw
pointers or additional unsafe operations.

## Structures and workloads

The benchmark owns immutable deterministic input buffers and borrows them for
operations. Each sample owns only timing counters and scalar fingerprints;
codec-owned buffers retain their normal allocation boundaries. No raw pointers
or unsafe operations are introduced by the benchmark. Detailed codec ownership
invariants are in `data-structures.md` and the linked module design documents.

| Workload | Operation measured | Work unit |
| --- | --- | --- |
| byte_cursor | Variable-width little-endian reads through a checked cursor | Input byte |
| reverse_bits | Exact variable-width reverse reads across byte boundaries | Input byte |
| fse_lookup | Checked state lookups in a sparse normalized table | Transition |
| huffman_decode | Decode mixed-length codes into an allocated output | Output byte |
| history_overlap | Allocate history and expand offset-7 overlapping matches | Output byte |
| ring_overlap | Reuse a 4 KiB ring and expand offset-7 matches across wraparound | Output byte |
| match_table | Reset and replace candidates in the four-byte hash table | Four-byte candidate |
| compress | Compress deterministic mixed text and byte patterns | Original byte |
| decompress | Decode an official zstd fixture when present | Decoded byte |
| stream_compress | Reuse an encoder with 4 KiB input and 1 KiB output slices | Original byte |
| stream_decompress | Reuse a decoder with 257-byte input and 4 KiB output slices | Decoded byte |
| dictionary_content | Checked indexed reads from prepared immutable dictionary content | Content byte |
| dictionary_compress | Reuse an encoder with a formatted dictionary and exact dictionary-derived input | Original byte |
| dictionary_decompress | Decode 64 official HTTP frames with one persistent prepared dictionary | Decoded byte |
| raw_training | Select raw dictionary content from 256 pinned samples with explicit sample/output caps | Sample byte admitted by the budget |

Dictionary encode/decode output is verified in full before timing. Their
metadata identifies serialized/content dictionary sizes, IDs, full dictionary
checksums, frame counts, exact input/output checksums, and retained capacities
before/after reset. Raw training uses 256 independent 512-byte sample slices,
a 64-KiB sample budget, a 2-KiB output cap and 64-byte candidate segments.
The training operation includes its allocations and selection algorithm; it is
not a measurement of just one private frequency-table lookup. The returned raw
content is fingerprinted, hashed in full outside timing, and checked against its
cap. No formatted metadata or COVER/fastCover equivalence is inferred.

`ZSTDOXIDE_BENCH_SMOKE=1` runs each deterministic operation twice and emits
`CHECK` rows. It never starts a timing clock or creates throughput ratios.
The comparison script validates the same complete case and metadata set in
smoke and timed modes; all fixture files consumed by these workloads participate
in the source/input fingerprint that prevents comparisons across changing code.

A fixed xorshift generator and seed construct inputs once outside the timed
region. FSE and Huffman tables are constructed outside the timed region.
The history and decode operations include allocation because their current
public contracts return owned storage. Inputs and outputs cross compiler
optimization barriers. Each operation returns a deterministic fingerprint;
all invocations are checked against the initial value, and the script compares
fingerprints across feature configurations. Byte-output fingerprints sample
64 positions. Outside the timed region, whole-codec workloads also report their
original/decoded and compressed byte sizes and hash every output byte with
FNV-1a. The comparison script checks that sizes and full-output checksums agree
across all builds. The compression workload's complete output is also decoded
and checked against its input before timing. These noncryptographic checksums
supplement the complete byte comparisons in unit and interoperability tests;
correctness is not inferred from sampled fingerprints.

By default each case runs five samples of at least 150 ms each, then reports the
median throughput and minimum/maximum sample durations per work unit. This is
at least eleven seconds per complete feature configuration, excluding setup,
memory trials and compilation.
Environment overrides permit longer measurements. Wall-clock measurements on a
shared workstation include noise and should not imply universal speedups.

Streaming checks outside timing compare every decoded byte, exact encoded
fingerprints, and context-owned retained capacity. A decoder is measured after
four and 64 concatenated frames and within one growing frame. An encoder is
measured after four and 64 blocks of a single frame. Their fixed caller buffers
and transient per-call allocations are excluded from `retained_capacity()`;
bounded retained capacity is not a measurement of allocator-call count or peak
process memory. Each streaming operation resets logical state while retaining
its allocated primary buffers. All eleven feature configurations must report
identical workload/output/capacity metadata before throughput is compared.

## Entropy safety review

The independent review checked normalized-count sum and symbol limits, the
power-of-two spread permutation, low-probability tail placement (including an
all-low-probability table), initialized transition ranges, exact reverse-cursor
bounds, maximum shift widths, and both complete initial FSE states. Terminal
weight decoding follows the specification's overread rule without performing
an actual out-of-bounds read and rejects output beyond 255 weights. The review
found no additional entropy defect.

A standalone harness then exercised 266,240 deterministic byte strings: lengths
0 through 64, with 4,096 strings at each length, generated by xorshift64 from
seed `0x0000d1fe20265eed`. Each string was passed to normalized-count parsing,
Huffman-weight FSE decoding, and (when its marker was valid) complete reverse
bit reads in chunks of at most 17 bits. Both the safe build and the combined
`unsafe-bitstream,unsafe-fse` build completed without panics. This bounded
stress check supplements the committed semantic tests; it does not constitute
an exhaustive parser proof or replace a memory-model checker.
