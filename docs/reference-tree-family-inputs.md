# Broader direct tree input design

This preparation retains the immutable ZTREE01 helper, request/observation
contracts and tiny recipes. All inputs stay within the existing bounds; no
larger source, table, schedule, dictionary or new operation type is introduced.
`FamilyCase` owns an immutable request and copied classification metadata. Names,
hypotheses and predecessor identities remain outside the C packet. They describe
input construction, not predicted matches or C branch observations.

The planned 103 cases contain all 30 exact tiny packets and these additions:

- Forty-five offset-cost inputs combine minimum matches 3..7 with near distances
  28/29/61 and far distances 255/511/1023. The intended near match is eight bytes
  and the far match nine bytes, with deliberately different terminating bytes.
  Their copied arithmetic records compare gain four to the high-bit difference
  `highbit(far+1) - highbit(near+3)`, yielding below/equal/above hypotheses. They
  only describe the decision if near is the incumbent and far is compared next;
  actual observations must establish the outcome separately.
- Twelve ring-boundary inputs use chain logs 6/7/8 and candidate distances at
  node mask minus one, equal, plus one and twice node count. Source comparison
  and child-node eligibility have different boundaries. Both candidate extents
  remain inside the source and the explicit window.
- Six window inputs place one exact 16-byte candidate at distance W-1/W/W+1
  for window logs 10/11. This distinguishes strict moving-floor exclusion from
  source-byte availability without caller-supplied floor mutation.
- Five inclusive-tail inputs end an eight-byte match exactly at source end,
  one for each admitted minimum-match value.
- Five long periodic sources use every early query, intermediate positions and
  the final eight-byte query. They expose retained frontiers and skipped calls
  through observations, without directly supplying a frontier.

For candidate constructions, deterministic bounded retries vary only source
noise. A source-only check proves each named candidate matches exactly the stated
number of bytes and then differs, and proves the latest pre-query hash bucket
position is the intended nearest candidate. This uses the pinned scalar hash
formula, not a search algorithm or C execution. No candidate table contents or
expected final maps are emitted. Separate classification fields remain clearly
labelled as hypotheses until an independently audited capture exists.

Tiny controls must retain exact source, schedule, parameter and packet identities.
The upcoming controller/auditor will use new names and pin these additive files;
existing scripts and captured bytes will not be rewritten to accept new cases.
The initial full C capture and its archive remain the independent predecessor.
