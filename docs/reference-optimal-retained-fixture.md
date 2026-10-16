# Retained Optimal fixture encoding

The additive `ZORFX01\0` encoding projects only independently audited C output.
Every integer below is little-endian u32. Bytes and lengths are explicit; there
is no alignment padding. The pack is bounded by 16 MiB and contains at most 16
uniquely named cases. The reader borrows source/frame/name bytes and uses checked,
fallible allocations for bounded block/event/delta records.

After magic and case count, each case has twelve words: name bytes, source bytes,
window log, hash log, chain log, search log, minimum match, target length, literal
mode, block count, frame bytes, path cells. Path cells is exactly 515. Two triples
follow, ordinary then sanitizer: context allocation, workspace allocation, peak.
The name, source and complete frame bytes follow, then chronological blocks.

Each block begins with twenty words: global start, length, wire kind, previous
owner before, previous owner after, pre-call main frontier, event count, three
incoming repeats, three candidate repeats, both owner repeat triples after
publication, and trailing literal count. The reader derives initial owners
`[[1,4,8],[0,0,0]]` and previous owner zero; it checks the next block's incoming
state against the preceding published state. A compressed block swaps owners;
raw/RLE retain the previous owner. The pending owner receives candidate repeats
in either case. Frame block kinds and source extents independently bind this
publication record. An empty final raw block terminates the complete frame.

An event uses the existing path event vocabulary: stage, reverse-phase flag,
global ip, global anchor, optional cur/last/tail (`0xffffffff` means absent), and
three repeats. Optional stretch/reverse/query records have a 0/1 presence word
then 7/3/6 words. A query is followed by candidate count and offset/length pairs.
Canonical ordered path changes follow as count then (index, seven cell words);
frequency changes follow as count then (index,value), over 386 words. Sequence
count and appended count precede (literal start, literal length, match length,
offset intent) records. Search changes follow as count then (index,value), over
hash heads, flattened paired nodes, optional ordinary short heads and two biased
frontiers. Every changed cell must differ from its previous value.

Path, frequency and search delta baselines persist between blocks. The per-block
sequence count, anchor and wire repeat interpretation restart from the declared
block start and incoming committed repeats. Match validation borrows only the
source prefix ending at that block's end. The first event preserves search maps
and derives main frontier from the pre-call value using the strict 384/192 rule,
then initializes local short frontier from that main frontier. Initial fresh
maps are zero and frontiers are two. Only query events can subsequently change
search state. Complete C states are compared again by the independent Python
model before projection; the Rust reader checks representation and semantic
invariants independently, followed by full Rust frame decode.
