# Tiny Optimal match adapter review

Pinned upstream: facebook/zstd v1.5.7,
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. The observer includes unchanged
`lib/compress/zstd_opt.c` in place of its separate object and links the remaining
25 codec translation units. No production implementation binds to this helper.

## Request and storage review before execution

The packet is read only after a regular-file extent check (76..17,964 bytes).
Header words are decoded bytewise; bounded source/query counts precede all size
arithmetic. Exact size, geometry and every query are validated before any output
or upstream call. Query positions strictly increase, leave eight source bytes,
and repeat values cannot cause an out-of-arena discarded pointer expression.
Length-to-beat is at most remaining source, preserving the sorted-branch end exit.

The source arena owns two leading initialized bytes, the immutable logical source
and trailing initialized storage. `base` and `dictBase` refer to the same live
array; prefix lowLimit/dictLimit are two. The no-dictionary routine receives only
pointers into this arena. `nextSrc`/end are the exact logical end. Every hash load
has its required physical footprint. Main/short-key frontiers start at two.

Heads, paired tree cells and optional short-key cells have 4,096-cell capacities.
Active cells are initialized to zero, inactive cells to a checked sentinel. Each
owner has 16-word leading/trailing guards. A 4,099-cell candidate owner is filled
with sentinels before every call; unused cells and guards must remain unchanged.
At most three repeat candidates, one short-key candidate and 256 tree comparisons
can append a candidate under the admitted search log, below this capacity.
No dictionary or external match-state branch is selected.

A static assertion caps all named observer storage plus 64 KiB stack allowance
below 4 MiB. The observer performs no explicit dynamic allocation; the reported
zero allocation count concerns observer-owned codec storage, not libc I/O or
sanitizer internals. Two source-padding passes reconstruct fresh zero state and
must produce identical complete observations. Before/after heads, paired edges,
short-key heads, both frontiers and every returned candidate are emitted.

## Capture and audit limits

Normal and ASan/UBSan builds use the same 26 codec units, DEBUGLEVEL=2, disabled
assembly/legacy/multithreading and pinned dependencies. Instrumented compilation
uses nonrecovering sanitizers and frame pointers for every object. The reviewed
plan has 164 commands: four identity checks plus two builds and 51 packets per
build (29 positive, 22 negative). No helper executes during plan preflight.

Each command has a 120-second timeout, at most 1 MiB stderr and 2 MiB ordinary
stdout; positive observation stdout is capped at 64 MiB. The entire campaign is
capped at 256 MiB of output and 1,800 seconds. The controller kills timed-out or
oversized children and retains failed records. The offline model caps lines at
512 KiB, requires exact EOF, reconstructs all state from immutable source, and
compares all maps/frontiers/candidates and both padding passes. Normal and
instrumented transcripts must be byte-identical. Every packet, object, binary,
source copy, command and stream is hash-bound and retained.

This tiny direct geometry does not qualify ordinary CCtx hash3 logs, the strict
2^18 short-key distance boundary, dictionary modes, path parsing or public
Optimal admission. Those remain explicit later work.
