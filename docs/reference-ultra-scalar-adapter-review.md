# Ultra scalar adapter review

The adapter includes the pinned, unchanged `zstd_opt.c`; its static scalar calls
use explicit modes 0 and 2. It never replaces a weight, price, table or search
function in the reference. Each normal/sanitized executable links the other 25
common/compress/decompress units. Assembly and multithreading are disabled and
DEBUGLEVEL=2 assertions remain active. Every codec unit receives the sanitizer
flags in the instrumented build.

`open` uses no-follow and nonblocking flags, followed by `fstat` regular-file and
size checks. Exact read and EOF checks precede row decoding. All ten words of all
rows are validated before any upstream call or stdout. The adapter's own format
LL interval function is used during this gate; the later scalar calls use native
LL/ML lookup. Length bounds ensure every native table index is in range. U32 sums
and selected counts obey subtraction-based bounds, preventing intermediate count
wraparound. The strict fractional maximum rejects the upstream assertion
boundary before evaluation of `raw+1` or its left shift.

All storage is static and initialized. Four frequency arrays have checked guards;
source has guards and an independently regenerated alternating pattern check.
Each synthetic table's remaining mass goes in an unused symbol (at most two
indices excluded, with every table at least 32 cells). Scalar inputs cannot name
pointers. Tables, entropy-cost storage and post-refresh optState are copied and
compared after each query. This proves no observable write to that storage; it
does not prove arbitrary logical overread absence. No match/path workspace is
used. A compile-time storage bound includes a 64 KiB stack allowance.

Dynamic LL and match products fit below 2^30. The largest raw literal product is
131072 * (31 * 256), also below that bound. Signed LL differences are computed in
int64 after each valid nonnegative price, avoiding unsigned subtraction. The
C expression for fractional weight remains the original shift order; Python
uses an independent multiplication/division expression. Whole-bit request rows
also serve as regression comparisons and retain their original u32 domain.

One initial syntax-only check failed because POSIX feature selection hid
`O_NOFOLLOW` on macOS. The source and failure log are retained. Enabling Darwin
extensions makes that platform flag visible; nonblocking opening also avoids a
FIFO wait before rejecting a nonregular file. No upstream observation ran before
this correction. Final normal and instrumented syntax/dependency checks are
separate preflight evidence; capture and forged-evidence controls follow later.
