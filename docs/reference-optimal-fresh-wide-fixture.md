# Complete fresh sparse fixture

`ZOFFW03\0` stores at most twenty complete fresh cases in a 16-MiB bounded
pack. Every case uses the twelve-word ZORFW02 header, two allocator ledgers,
ASCII name, exact source, complete frame, then exactly one block with its
publication owners and all event records. Source and block lengths are
64..16,384, frame <=32,768, events <=65,536, path cells exactly 4,099 and
complete search cells <=73,730. The source fits its selected W10..16 window;
H/C6..12, S1..6, minimum 3..7 and target <=4095 remain explicit.

Projection hashes every archive member and immutable capture input, reruns the
independent complete-state model, and requires identical normal/sanitized and
padding frames. Canonical sparse native path/search deltas are projected
without resampling; frequency deltas use a complete prior-state baseline.
Native candidates remain observations, never match-provider inputs.

The independent Rust reader borrows source/name/frame bytes, owns fallibly
reserved bounded event/delta vectors, and reconstructs complete initialized
path/search/frequency state. It checks increasing changed indices, source-match
bounds, forward/reverse roles, exact sequence publication and repeat owners,
all frame structure, complete final-block termination and EOF before exposing
cases. Shared pure path invariant helpers do not import production search or
parser choices. Malformed counts, horizon indices, publication state, strict
frame prefixes and changed termination are checked independently. Complete C
frames decode through Rust before fresh production admission expands.
