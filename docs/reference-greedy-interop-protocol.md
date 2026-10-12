# Successor public Greedy frame interoperability

This protocol precedes the new public C execution campaign. It extends the
bounded public transport from Module 13 in separately named adapters and keeps
all prior C/Rust sources, protocols, recipes and archives immutable. Reference:
facebook/zstd v1.5.7 revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.

## Public calls and source recipes

The new C helper uses a fresh `ZSTD_CCtx`, applies the requested level and seven
compression overrides in their documented order, then the source hint. It sets
content size on, checksum off, dictionary ID on, and pledges the exact input
length before `ZSTD_compress2`. It does not load a dictionary or enable workers,
LDM, a sequence producer, special block splitting or an internal begin call.
There is no applied-C-scalar claim. Rust calls the public request API with the
same input and reports its resolved tuple separately.

The successor source-only manifest will preserve all original 92 Fast-derived
and 159 broader DoubleFast input/request pairs, with separately frozen v3
admission outcomes. Historical names receive explicit family prefixes where
needed to keep a unique combined catalog. Source bytes, levels, overrides,
hints and historical resolved tuples must remain unchanged. The nine newly
admitted historical records need positive C execution; their previous not-run
records remain historical facts.

Add all 138 ordinary Greedy source recipes through one public known-size call.
Their schedules and explicitly applied internal parameters remain separate
observations. Public resolution may downsize dimensions and even select a chain
for a formerly forced row fixture on small input. Recipe names record source
provenance, not the newly selected branch. Resource predictions use actual
known-size adjustment and selected-family caps; never relabel a rejected case
as a successful C observation. Include the exact newly admitted selector prefix
and level from `unsupported-57`.

Add explicit source-dependent window14/15 transitions, maximum admitted row
hash/window dimensions, minimum-match3/4 and6/7 equivalences, target0/4/131072,
row widths16/32/64, search-budget capping, unused row chain settings and conflicting
hints. Freeze both old and new Rust frame-equivalence hypotheses before running
codecs. Preserve failed hypotheses instead of dropping cases. Bound each source
to4MiB, each frame to5MiB and aggregate recipe source bytes to128MiB. The frozen
manifest will give exact counts, identities and expected classifications.

## Versioned binary protocol

Use new 8-byte request magic `ZGOI0001`. Follow it with twelve little-endian u32
fields: mode, source length, frame length, signed-level bits, window, chain,
hash, search, minimum-match, target, strategy and source hint. Request header
length is56. Scalar zero retains its public setter meaning. Decode the signed
level without an implementation-defined C unsigned-to-signed conversion.

Mode0 encodes, requires frame length0, and carries source bytes. Mode1 decodes,
requires a nonempty frame, and carries expected source bytes followed by the
frame. Source is at most4,194,304 bytes; frame at most5,242,880 bytes. Reject
unknown modes, mismatched lengths, invalid scalar domains, truncation and any
trailing byte. Maximum request length is9,437,240; the Rust bounded reader may
read one extra byte only to detect overflow/trailing input.

Responses use `ZGOR0001` for Rust and `ZGOC0001` for C, then status, seven
resolved scalar u32 values, payload length, and payload. Header length is44.
Rust status0 means success,1 unsupported strategy and2 operational resource
limit. A rejected encode returns its valid resolved tuple with empty payload.
C successful responses and all decode responses have zero scalar fields; this
is explicitly not a C parameter observation. Unexpected errors terminate with
exit2 and no successful response. Maximum response length is5,242,924.

Both adapters accept exactly one standard frame, with known content size equal
to the expected source, dictionary ID0, no checksum, and window at most2MiB.
Walk/check the exact frame extent before decoding, rejecting concatenated or
skippable tails. Decode success requires length and every byte to equal the
expected source; the Rust decoder receives matching output/window limits.

## Allocation and pointer review before C execution

Retain the existing bounded public-helper structure in a new separately named
adapter. It owns three initialized guarded buffers: source, optional compressed
input and output. Each has64 initialized leading and trailing guard bytes.
Zero logical input still receives a one-byte payload allocation. All capacities
are checked against5MiB and `SIZE_MAX - 128` before addition or `malloc`.
No pointer is formed before the start or beyond the end of its own allocation.
Every read/write length is checked against that buffer's logical capacity;
input headers are initialized completely before scalar reads.

Encoding allocates `ZSTD_compressBound(source_size)` only after checking its
error status and0 < bound <=5MiB. Decoding allocates `max(source_size,1)` while
passing the true logical destination size. At maximum sizes, the conservative
three-buffer payload/guard bound is13,631,872 bytes (2*4MiB +5MiB +384). The
compression bound makes encoding smaller than that bound. This excludes the
opaque C context/workspace, libc buffering and sanitizer overhead; record that
qualification rather than treating it as a total process memory bound.

Check all six guard regions after successful codec operations, validate output
extent before serialization, and free each complete owner through its original
base. The public API owns its internal allocations; no table seeding, pointer
introspection or workspace copying is needed. Review the actual finished C and
Rust adapters against these bounds before their first oracle execution.

## Build, execution and evidence

Compile every26 `.c` unit in `lib/common`, `lib/compress` and `lib/decompress`
from the external pinned checkout for each normal and ASan/UBSan build. Verify
all source/header identities and compiler dependencies; disable assembly,
legacy support and workers. Do not link the prebuilt static archive. Compile
the helper with the same instrumentation. Keep LeakSanitizer disabled and say
so explicitly. Sanitizer silence applies only to the exact executed campaign.

Build safe/all Rust adapters separately. For each admitted recipe, require
identical safe/all frames, normal/sanitized C frames, C decoding of both Rust
frames, and both Rust decoders accepting each C frame. Rejected Rust requests
must agree in both builds and have no C compression substituted for success.
Check all fixed Rust equivalence groups and exact source hashes. Preserve full
source/command/environment/compiler/executable identities, binary responses,
stderr, exit codes and failed hypotheses in a fresh external capture root.

The controller sends C encode requests only for the frozen admitted recipe set.
The wire envelope alone is not a general bound on arbitrary valid C strategy
workspace or runtime. Preserve this qualification alongside per-command timeouts
and stream caps. Reject the old transport magic explicitly.

Before the main campaign, exercise invalid packet magic/mode/extent/scalars,
short headers/payloads, excess trailing bytes, single-frame constraints and
metadata limits in every adapter. Never obtain negative cases by fabricating
an out-of-allocation pointer. Future platform and timing campaigns remain
separate from this native bidirectional correctness run.
