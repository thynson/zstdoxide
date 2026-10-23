# Retained Ultra2 complete-frame benchmark baseline

The 26 inputs were selected before clocks from the independent retained Ultra2
public manifest. They cover all four balanced source boundaries, raw/RLE/
compressed histories, short keys, every minimum-match setting, target requests
and table/search endpoints. The exact selection is embedded in provenance.json.

The bounded ZU2B0001 pack stores each name, nine requested parameter values,
seven independently resolved values, complete input and complete Rust frame.
The safe/all Rust frames are identical and each was decoded exactly by both
normal and sanitizer-instrumented facebook/zstd v1.5.7 adapters, pinned to
f8745da6ff1ad1e7bab384bd1f9d742439278e99. The upstream implementation is used
only as a test oracle. No upstream codec is linked into the Rust implementation.

Ordinary and optimized source-only projections produce identical bytes and
provenance with child execution forbidden. The checker separately pins all
pack, provenance, source-manifest and native-archive digests and validates each
bounded field against its original independent recipe. The benchmark's borrowed
reader consumes this trusted immutable test pack outside all timing regions.

The public capture archive is
`docs/evidence/2026-10-23-ultra2-retained-public-native.tar.gz`, SHA-256
`a95b599c9b68f4a2013ba61d2bbbe8dad273e4911b9ad1f9850967bc0b94621f`.
Pack SHA-256: `918067789e5216b6aa050f9164eb0a7f43582ad09c0f734527f03140ca93e203`.
The fixture contains 688138 source bytes and 132267 complete frame bytes;
those counts do not establish allocator or throughput results.
