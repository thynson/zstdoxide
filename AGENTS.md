# Project instructions

## Goal and language

Reimplement the Zstandard format and core algorithms in idiomatic Rust, using
facebook/zstd v1.5.7 as a pinned reference. All code, comments, documentation,
commit messages, and task tracking are written in English. Do not bind to C or
wrap another codec as the implementation. The upstream executable is a test
oracle only. State supported and unsupported format features accurately.

## Design before implementation

Before a major module, analyze its data structures and record their ownership,
invariants, and upstream counterparts in `docs/data-structures.md`. Decompose
the module into small independently verifiable work items in `TODO.md` before
writing its implementation. Use Rust naming and APIs, not transliterated C.
Where helpful, document the corresponding upstream C function.

## Memory and unsafe code

- Default builds must be entirely safe Rust. Represent non-owning positions and
  links with indices/ranges; own allocations with `Box`, `Vec`, or `Arc`.
- Each performance-sensitive data structure has a safe implementation and an
  equivalent unsafe implementation selected by its own opt-in Cargo feature.
  Keep the same public contract. Plain value records need no artificial unsafe
  operations; document why their representation is shared.
- Every unsafe function must have a complete `# Safety` contract, including
  validity, bounds, alignment, initialization, aliasing, and lifetime conditions
  that apply. Every unsafe block must prove all preconditions of every unsafe
  operation it contains using a nearby `// SAFETY:` comment.
- Never use unchecked indexing based solely on input validity or assertions
  that disappear in release builds. Reject malformed input before access.
- Enable `unsafe_op_in_unsafe_fn` and undocumented-unsafe-block lints. Check
  every feature separately and all features together; compare correctness and
  benchmark safe and unsafe builds on identical deterministic workloads.

## Portability and allocation

Keep codec modules compatible with `no_std` plus `alloc`. Isolate filesystem,
CLI, timing, and process APIs behind `std`. Prefer caller-owned slices and
fallible allocation. Keep allocation boundaries explicit for future allocator
parameterization; do not depend on an assumed Rust 1.100 `Allocator` API.

## Workflow

- Preserve unrelated user changes. Use small commits with one completed work
  item each, including the checked task in the same commit.
- Commit each item after its focused checks pass. Do not batch multiple
  unrelated items into one commit. Only the coordinating agent makes commits.
- On finishing a major module, clear its completed checklist from `TODO.md`,
  retain the next module's decomposition, and record completion in
  `docs/progress.md`. Git history is the detailed completed-task archive.
- Test meaningful format boundaries, malformed input, and official zstd
  interoperability. Never claim an unrun check passed or a partial codec is a
  complete replacement. Run formatting, relevant tests, Clippy, and no_std
  compilation before reporting a milestone.
- Reference source downloaded for study stays outside the repository. Preserve
  source attribution for derived tables, constants, or fixtures.
