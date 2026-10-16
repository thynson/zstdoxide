# Bounded retained ordinary Optimal protocol

This additive source-only domain studies ordinary CCtx Continue/End behavior at
facebook/zstd v1.5.7, revision f8745da6ff1ad1e7bab384bd1f9d742439278e99. It does not
change fresh admission or imply public strategy 7 support.

Packets begin with `ZOPRT01` and zero, followed by nine little-endian u32 words:
source length, block count, window log, hash log, chain log, search log, minimum
parameter, target and literal mode. Exactly block-count u32 lengths follow, then
exactly the source bytes. No script candidates, repeat arrays, output paths or
publication decisions are carried in a packet. Bounds are 2..4 blocks, each
64..512 bytes, sum equal to source length and at most 1,024; W10..12, H/C6..9,
S1..4, minimum 3..7, target 0..4095, literal mode 0/1. The full source fits the
window, so expiry/rebase, dictionaries and external history remain excluded.

The helper uses ordinary custom-memory CCtx begin, one Continue per scheduled
block, and End with zero source. Requested/applied parameters must agree. Post
splitting, LDM, workers and external producers are absent. Each fresh padding
pass owns one zero-initialized allocation arena; it never clears search, path or
frequency buffers between Continue calls. Both passes produce complete frames
and allocation ledgers, and pinned decoding must reproduce all original bytes.

Each block has begin and publication records enclosing the complete observation
stream. Events retain all 515 path cells, all frequency tables/cache, all search
heads/nodes/short heads, local and main frontiers, repeat values, candidate lists,
and source-relative sequence positions. Source coordinates are global within the
owned input; cur/last/reconstruction positions remain path-relative. Short-key
frontier is initialized from main frontier on every block, but short heads remain.
The last-stretch value starts at zero each block; path and frequency owners persist.
Only the first block increments its starting source position to one.

Publication records bind consumed/written extents, wire kind, prev/next owner
indices, committed repeats before/after, candidate repeats and live candidate
sequences. The independent model freezes expected wire kinds as hypotheses; C
never receives them. The auditor requires actual frame block kinds to agree and
then applies ordinary publication: compressed swaps the two state owners and
commits candidate repeats; raw/RLE retain the prior committed owner/repeats.
Candidate search/path/frequency changes remain even after raw/RLE output.

Events are limited to 1,024 per block, lines to 1 MiB, request output to 256 MiB.
Frame and allocator paths are capture-plan arguments with exclusive creation.
Two padding passes and both normal/ASan+UBSan builds must agree on state/frame
bytes. Allocation sizes compare within builds; across builds the workspace retains
the pinned fourteen pairs of 128-byte redzones. The runner must validate exact
commands, dependencies, predefines, environment, timeout and output limits before
execution. Independent Rust decode and forged-state/publication/frame controls
remain required before retained production integration.
