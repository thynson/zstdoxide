# Retained observer v2 qualification corrections

The first retained capture completed every native command but failed independent
replay. Two distinct source-model assumptions were wrong. First, ZSTD_buildSeqStore
advances nextToUpdate before entering a block when current > nextToUpdate + 384,
using current - min(192, current - nextToUpdate - 384). The model had only handled
sorted-tree catch-up inside the parser. Second, the predicted raw middle block in
retained_raw_rollback compressed successfully; it contained a longer earlier-source
match and therefore committed candidate repeats. Its packet and failed evidence
remain unchanged in the predecessor campaign.

V2 applies the pinned inter-block formula before copying main frontier into the
block-local short frontier. The old compressed case is named retained_short_commit
with corrected publication hypotheses. Three new uniform-source cases use first
block lengths 385, 386 and 512 to distinguish the strict threshold (initial main
frontier is biased position 3). Their second-block entry frontiers must be 3, 387
and 387. A new independently specified raw-candidate hypothesis uses unrelated
bytes with an isolated four-byte nearby match and target zero; it is not presumed
correct until actual wire kinds and full replay agree.

The exact C helper, fourteen observation anchors, native API entry points, compiler
flags, sanitizer and pointer attributes remain unchanged. Sixteen source-only cases
predict 4,486 events, 1,523 queries and 41 sequences. The fresh execution plan has
140 commands and 23 malformed requests per build. A new capture directory retains
both builds/padding passes. V1 is archived as a failed audit rather than relabeled.
