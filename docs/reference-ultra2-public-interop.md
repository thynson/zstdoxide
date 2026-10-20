# Bounded Ultra2 public interoperability

The isolated public candidate completes 16,008 commands for 1,433 frozen
source-only recipes: 1,292 admitted, 141 resource-limited and zero unsupported.
Both Rust builds and both C builds use the pinned source and exact reviewed
command/environment inventories. This is qualification of an isolated candidate;
main admission was still gated at capture time. The later main admission is
recorded in `reference-ultra2-public-qualification.md`.

Normal and fully ASan/UBSan-instrumented C builds compile all 26 relevant codec
translation units. Safe/all Rust frames are decoded by both C builds; normal/
sanitized C frames are decoded by both Rust builds. Each direction verifies
5,168 complete frames, totaling 802,569,512 decoded bytes. The 156 malformed
adapter exits are expected exit 2, not crashes. Rust safe/all output bytes agree;
normal/sanitized C output bytes agree. Equal C/Rust compressed representations
are not required.

All 1,345 predecessor recipe/source pairs remain unchanged. The 1,222 previously
successful cases retain all 4,888 complete encoded responses exactly, including
resolved parameters and frame bytes. The predecessor model is independently
reconstructed from its immutable archive rather than learned from the successor.
Twenty-five equivalence groups preserve their complete expected outcomes.

Independent positive replay passes in normal and optimized Python. Each mode
also rejects all 59 forged capture controls with zero child-process attempts.
Controls cover exact commands, compiler/dependency/source identities, bounded
artifact paths, instrumented translation units, coherent changed frames and
independently resolved strategy/resource tuples, including three new Ultra2
forgeries. Controls operate on unlink-before-write copies; original capture
inputs are revalidated afterwards. Preflight also passes in both Python modes
without executing native commands.

The complete capture freezes 493 source paths before any native invocation.
Later benchmark helpers are outside that historical source closure; replay uses
the stored source snapshot. New tooling does not retroactively alter this
campaign's preflight or inputs. Complete native requests, responses, executables,
source snapshots, compiler outputs, predecessor/preflight logs, both audit/control
runs and the source-only benchmark preselection are retained in the lossless
archive. Every regular file is independently reconstructed and rehashed, followed
by a second unchanged-original verification.

Archive: `evidence/2026-10-20-ultra2-public-interop.tar.gz`.
SHA-256: `c3296e521cf6d7e7a564b19a0e1b3bcdd14c8910c651e174c01d3a9483a42397`.
The archive reconstructs 48,594 regular files and 2,587,742,073 logical bytes
from 54,819,230 compressed bytes; its adjacent JSON binds the complete chunk map.
Private lifecycle/ownership, foreign targets, full regression and performance
have separate checkpoints. This evidence does not claim complete upstream test
migration, retained/dictionary public Ultra2 or full zstd parity.
