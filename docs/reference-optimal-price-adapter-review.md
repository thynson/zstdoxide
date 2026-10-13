# Optimal price adapter review

The v2 helper includes the unchanged pinned `zstd_opt.c` to reach five static
functions: rescale, update, base-price refresh, raw-literal price and LL price;
match price is a template in that same translation unit. No match enumeration,
optimal path, CCtx, dictionary or frame encoder is invoked. Compile the other
25 common/compress/decompress C units separately and omit a separate zstd_opt
object. The instrumentation build instruments all 26 used codec source units
plus the adapter, retaining upstream sanitizer annotations.

All packets are read completely into a fixed 134,164-byte buffer after regular
file, size and exact EOF checks. Header and complete operation validation finish
before any upstream call or stdout record. Sizes use subtraction-based source
bounds; all decoded words are u32, table cells are compile-time checked 32-bit
unsigned values, and updated-literal accumulation is u64. Per-request limits
bound sums well below u32 wraparound and all valid price products below 2^30.
The source array, four separate frequency arrays, zeroed entropy-cost record and
optState are static owned storage. The static zero initialization establishes
all counts and disables dictionary-derived Huffman repeat prices. No match/path
workspace is allocated or referenced by the selected calls.

Both literal modes share the same initialized storage. Source ranges permit an
empty one-past slice, which the invoked zero-length paths do not read. Positive
dynamic literal queries require initialized nonzero literal mass; dirty caches
cannot be queried. Updates can intentionally retain stale caches until a refresh
or rescale operation. Repeated updates call upstream individually and preserve
the same complete final observation contract; they do not rewrite statistics
using a helper-specific shortcut.

Frequency arrays have checked prefix/suffix canaries. Source storage has checked
canaries, an unchanged original copy in the packet, and an unchanged unused tail.
These checks detect writes, not arbitrary logical overreads inside allocated
storage. ASan/UBSan qualify their own allocated-object/arithmetic boundaries;
independent complete-state replay additionally verifies observable behavior.
The helper has no dynamic allocations. A compile-time adapter-owned storage cap
includes all buffers, records and a conservative 64 KiB stack allowance; it is
not a process-RSS limit. Output buffering is fixed at 64 KiB; the controller must
bound captured stdout/stderr, runtime and total retained log bytes.

Every operation prints all 377 frequencies, sums, cached prices and optional
query results. The request and end records bind the source fingerprint and exact
operation count; the controller separately binds cryptographic hashes of packet,
source, complete stdout and executable. The independent Python model derives
symbol intervals, frequencies and prices without calling C or production Rust.
A matching model transcript generated before C execution is a self-consistency
control only, never labeled an upstream observation.

Normal/instrumented syntax and dependency preflight, malformed command controls,
complete capture, independent forged-evidence audits and bounded Rust fixture
projection remain separate gates. Strategy 7 stays unsupported throughout this
observation work.
