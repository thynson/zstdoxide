# Broader public DoubleFast frame interoperability

This is a source-only protocol and input plan, prepared after public admission
and the bounded successor92-case campaign. It does not replace the frozen
direct parser, ordinary internal-begin, or Module12 oracle inputs. Reference:
facebook/zstd v1.5.7 revision `f8745da6ff1ad1e7bab384bd1f9d742439278e99`.

## Public calls and source recipes

The new C helper uses a fresh `ZSTD_CCtx`, applies the requested level and seven
compression overrides in their documented order, then the source hint. It sets
content size on, checksum off, dictionary ID on, and pledges the exact input
length before `ZSTD_compress2`. It does not load a dictionary or enable workers,
LDM, a sequence producer, special block splitting or an internal begin call.
There is no applied-C-scalar claim. Rust calls the public request API with the
same input and reports its resolved tuple separately.

[The recipes](../tests/fixtures/reference-double-fast-interop-recipes/recipes.json)
freeze159 unique requests and38,855,519 total source bytes. Expected admission
is113 success,42 unsupported and4 resource errors, pending execution. They cover:

- All51 frozen ordinary source recipes through a single public call. Original
  schedules and explicitly applied internal parameters remain separate evidence.
- Ten source boundaries at levels0/2/3/4, including both DoubleFast level4 tiers.
- Unequal long/short table logs6/18 in both directions with actual input large
  enough to retain the requested dimensions; minimum-match3/4/7.
- Search/target controls, known-source hints, all independent resource caps,
  unsupported-strategy precedence, and a complete4MiB source.
- Both complete original43-entry fuzzer level loops and the exact four newly
  admitted C-selector source prefixes. Reusable-context, caller-output and
  advanced-branch contracts remain outside these calls.

Sixteen Rust frame-equivalence groups are hypotheses fixed before capture:
level0/3, minimum-match3/4, nonoperative DoubleFast search/target fields and
known-source hints. Preserve failed hypotheses rather than deleting inputs or
silently changing expectations. C and Rust compressed bytes need not match.

## Versioned binary protocol

Use new8-byte request magic `ZDOI0001`. Follow it with twelve little-endian u32
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

Responses use `ZDOR0001` for Rust and `ZDOC0001` for C, then status, seven
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

Before the main campaign, exercise invalid packet magic/mode/extent/scalars,
short headers/payloads, excess trailing bytes, single-frame constraints and
metadata limits in every adapter. Never obtain negative cases by fabricating
an out-of-allocation pointer. Future platform and timing campaigns remain
separate from this native bidirectional correctness run.
