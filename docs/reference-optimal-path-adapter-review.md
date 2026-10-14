# Scripted Optimal path adapter review

The executable includes one generated external copy of pinned `zstd_opt.c` in
place of its ordinary object. Twenty-five other pinned codec C units link with
it; all 26 used units receive the same sanitizer switches in the instrumented
build. Assembly and multithreading are disabled. Reference revision and every
used C/header digest are checked before preparation and audited after capture.

Fifteen unique source anchors are reviewed in
`oracles/optimal_path_instrumentation.py`. Fourteen add observation calls while
preserving every original statement. The remaining anchor selects the bounded
scripted callback instead of the ordinary match finder; a non-executing reference
to the original selector avoids an unused-function warning. The output includes
the original copy, generated copy and complete substitution manifest. Missing or
duplicate anchors fail before compilation. No parser, price or repeat statement
is replaced. This explicitly qualifies a synthetic match-provider boundary,
not ordinary CCtx search or fresh-repeat startup.

The 5,148-byte maximum packet admits 9..512 logical source bytes, 0..64 ordered
script positions and 1..8 increasing candidates per scripted position. Every
header and candidate byte extent is checked before access. Direct matches are
validated against source before any output. Dynamic repeat candidates are checked
against actual query history before being copied. Independent replay must accept
every positive script before C execution. Twenty malformed transport/source
packets must fail before metadata output.

Source resides 64 bytes into a fully initialized arena with at least 64 trailing
bytes. Thus even the upstream literal-copy boundary `litLimit - 32` stays within
the arena for the minimum nine-byte source. Window base is source minus two,
within the same allocation. Ordinary match discovery is never called, so the
synthetic callback does not form out-of-prefix repeat pointers. Queries require
at least eight remaining logical bytes; candidate comparisons stay within the
logical source. Both source and all surrounding/inactive bytes are rechecked.

All 4,099 path cells, candidate cells and frequency arrays are initialized before
execution. Only the first source_length+3 path cells may change; the rest must
remain zero at every event. Independent guards surround every owner. The
sequence owner has 171 cells, exceeding floor(512/3), and the literal owner has
512 logical bytes plus 64 initialized bytes for the pinned copy allowance.
Each stored upstream sequence and copied literal span is compared against the
observer's source coordinates before the event is emitted. All observer-owned
static storage, including output buffer and additional margin, is below 4 MiB.

Events are capped at 1,024 per padding pass. The independent plan records exact
expected event counts; the helper enforces the limit before event output. Per
command stdout is capped at 64 MiB for positive traces, stderr at 1 MiB, command
time at 120 seconds and aggregate campaign output at 256 MiB. The maximum
source-only trace is checked in both independent model modes before execution.
Every positive request runs with two distinct initialized arena padding bytes;
normal and ASan/UBSan traces must agree byte-for-byte. The offline auditor reruns
only independent Python replay and never executes a captured binary.

The first protocol does not reach the 4,096-position horizon or implement the
production Rust path owner. Its completion cannot enable public Optimal. Capture
failures are retained; corrections after capture use additive versions or an
explicit new capture rather than rewriting frozen evidence.
