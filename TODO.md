# Tasks

## Module 0: project foundation

- [x] Establish project instructions and incremental delivery workflow.
- [x] Analyze upstream data structures and document the architecture.
- [x] Create a dependency-free Rust crate with no_std and feature boundaries.

## Module 1: entropy primitives

- [x] Document entropy structure invariants and module work decomposition.
- [x] Implement checked reverse bitstreams and the unsafe-bitstream alternative.
- [ ] Implement FSE normalized-count parsing and decoding tables with unsafe-fse.
- [ ] Implement two-state FSE decoding of Huffman weights.
- [ ] Implement Huffman table construction and safe/unsafe-huffman decoding.
- [ ] Implement raw, RLE, compressed, and treeless literal sections.

## Module 2: frames and sequence execution

- [x] Document frame, cursor, history, and sequence invariants.
- [x] Implement checked frame headers and safe/unsafe-byte-cursor access.
- [x] Implement bounded history and safe/unsafe-history match expansion.
- [ ] Implement XXH64 content checksum with published vectors.
- [ ] Implement sequence decoding and repeat-offset updates.
- [ ] Integrate frame decoding, concatenation, and skippable frames.

## Module 3: initial compressor

- [ ] Decompose encoder structures and encoding work before implementation.

## Roadmap

1. Checked byte/bit access and entropy decoding (FSE and Huffman).
2. Frame/block decoding, history, checksums, and bounded allocation.
3. Interoperable frame encoding and initial compression strategies.
4. Official-codec differential tests and safe/unsafe benchmarks.
5. Streaming, dictionaries, advanced compression, and allocator support.

Each module must be decomposed before implementation. Completion of an initial
codec milestone does not mean parity with the upstream library or CLI.
