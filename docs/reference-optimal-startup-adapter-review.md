# Fresh Optimal startup observer review

The additive helper uses facebook/zstd v1.5.7 revision
f8745da6ff1ad1e7bab384bd1f9d742439278e99 as an external oracle. It includes a generated
copy of `zstd_opt.c` instead of linking that unit separately. Fourteen unique
observation anchors preserve every original parser statement and the original
`ZSTD_selectBtGetAllMatches` assignment. Two pre-query snapshots are added; no
scripted provider is installed. Removing the exact observation substitutions
reconstructs the original file byte for byte, including pointer-overflow attributes.

The packet accepts only one 64..512-byte source and bounded explicit btopt geometry.
There are no caller-supplied pointers, repeat histories, filenames in packet data,
dictionaries or scripts. Packet size and every scalar bound are checked before
metadata. Artifact paths are separate arguments supplied by a future exact capture
plan; exclusive creation refuses to overwrite existing files. The helper emits
two source-padding passes and four artifacts: two complete frames and two allocator
ledgers. A capture driver still needs exact dependency, command, environment,
output-size, timeout and artifact auditing before any execution is admitted.

The custom allocator limits requests to 32 and aggregate live bytes to 8 MiB.
It checks size arithmetic, adds 64-byte guards on each side and zero-initializes
all requested bytes. The 64-byte displacement preserves max_align_t alignment,
verified at compile time. Free validates exact live pointer identity and guards;
duplicate/foreign frees reject. The ledger records request sizes, peak live bytes
and complete release. This is first-use memory initialization via the ordinary
CCtx custom-memory API, not a patch to path or search decisions. Allocation failure
stops the helper; full injected allocator-failure coverage is a later capture gate.

The unchanged advanced initializer receives explicit complete parameters. State
observations reject adjusted geometry and verify consumed search/path/frequency/
sequence/literal ranges lie within live custom allocations. Source positions use
integer address checks before converting to block-relative offsets; observer
copies are bounded by admitted array extents. The observer does not dereference
stored search indices. Sequence records are checked against upstream SeqDef and
copied literals, after validating live output bounds. Source/frame/decode buffers
have guards; all inactive path slots beyond source+3 must remain initialized zero.
These checks do not claim individual guards around upstream workspace subobjects.

The original noDict code can construct an unused `dictBase + repIndex` after
unsigned repeat subtraction. Fresh [1,4,8] at position one can take that branch.
This helper leaves original eligibility and `ZSTD_ALLOW_POINTER_OVERFLOW_ATTR`
unchanged and does not form that expression itself. Eligible source reads are
controlled by upstream checks, while the independent integer model rejects
unavailable distances before source access. ASan/UBSan is required for capture,
but does not establish ISO C provenance of unused upstream pointer expressions.
The old direct-helper repeat guard stays intact.

Observed path events are candidate parser state. A complete frame may choose raw
storage, so these events alone do not prove retained committed repeat/frequency
publication. Ordinary frame bytes are retained and decoded by the pinned decoder
inside the helper; a subsequent independent Rust decode and offline frame/state
binding are still required. No C helper execution or public Optimal admission is
claimed by source review or syntax-only preflight.
