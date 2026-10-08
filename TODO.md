# Tasks

## Module 0: project foundation

- [x] Establish project instructions and incremental delivery workflow.
- [ ] Analyze upstream data structures and document the architecture.
- [ ] Create a dependency-free Rust crate with no_std and feature boundaries.

## Roadmap

1. Checked byte/bit access and entropy decoding (FSE and Huffman).
2. Frame/block decoding, history, checksums, and bounded allocation.
3. Interoperable frame encoding and initial compression strategies.
4. Official-codec differential tests and safe/unsafe benchmarks.
5. Streaming, dictionaries, advanced compression, and allocator support.

Each module must be decomposed before implementation. Completion of an initial
codec milestone does not mean parity with the upstream library or CLI.
