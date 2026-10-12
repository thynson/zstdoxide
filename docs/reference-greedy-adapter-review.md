# Greedy direct adapter review before execution

The coordinator reviewed `scripts/oracles/greedy_direct.c` against the pinned
v1.5.7 chain, row and depth-zero parser source and the
[packet/arena contract](reference-greedy-oracle.md). At this checkpoint only
normal and sanitizer-configured **syntax checks** have run; no helper binary,
reference parser, fresh-context salt observation or Greedy codec has executed.
Packet/record validation, tiny recipe freezing and capture are separate items.

Reviewed helper SHA256:
`51bfb1d124fbdaf72612f8802f271218f30da478358cd1610b8961f6a1748828`.
Changes to this helper require another review before capture. The capture driver
must pin this source and the final protocol/validator/recipe identities before
compiling or importing executable infrastructure.

The inspection established these concrete properties:

- Input decoding reads the exact 152-byte header as scalar bytes, checks both
  reserved fields and the opened regular-file size, and checks exact EOF.
  Logs/counts are bounded before shifts or variable allocation. ID characters
  cannot inject JSON syntax or paths. Seed storage is a separate owner, not
  an input-sized stack array.
- Every guarded payload uses `max(bytes,1)+191` allocated bytes. A checked
  offset 64..127 places it at a 64-byte boundary with at least 64 trailing
  bytes. All actual leading/trailing allocation padding is initialized and
  checked. Typed storage checks alignment divisibility; main also checks
  the host's `max_align_t` requirement. The original malloc pointer remains
  with its owner until free.
- The selected chain or row family has two live owners and two complete
  before-images. Inactive branch storage is absent. The seed loop is strictly
  increasing and precedes U, with each selected key wholly within the prefix.
  Chain slots use biased `(p+2)&mask`, with the old hash head captured before
  replacement. Row cell zero remains reserved; tag and position insertion
  follow the same backwards nonzero-cell cycle.
- Thirteen simultaneous direct owners are preflighted, including the packet
  source and seed owners. On the declared U32/SeqDef/64-bit observation layouts
  (4/8/24 bytes), maximum chain and row totals are respectively 18,968,500 and
  17,919,924 bytes, including guards/alignment slack. Actual runtime `sizeof`
  values are reported and used in checked arithmetic; the 64-MiB cap applies
  independently of these source-derived layout calculations.
- Both initialized execution arenas contain source-2, all short-input negative
  tail pointers, and the final failed-probe step. Their complete physical bytes
  are compared after parsing. Logical source length excludes padding. Chain
  and row use their own eight/sixteen-byte search margins; row cached +8 hash
  reads and width-dependent row prefetches fit the reviewed bounds.
- Sequence/literal/code capacities are installed before reset and parse. The
  conservative sequence count exceeds the possible number of four-byte
  matches. Literal write slack lies before the guard. Replay expands long
  lengths, independently resolves full wire repeats, checks history/window/
  floor/remaining lengths, and copies overlapping matches byte by byte.
- Full before/delta output includes every nonzero cell and every changed cell,
  with canonical LE32/byte fingerprints. All cells are inspected afterward;
  predecessor cells are not mistakenly validated as hash buckets. Row head
  values, reserved positions and salted tag/bucket identities are checked.
  Final maps cannot establish intermediate store chronology or probe counts.
- The fresh-salt mode creates and releases two independent contexts sequentially
  through a separate 64-slot/8-MiB custom-allocation ledger. Returned payloads
  use distinct nonzero fills 0x3b and 0xd1. C initialization, rather than helper
  zero-fill, must produce the observed initial state and table zeros. Applied
  tuple/mode is inspected, salt is read before free, and all frees recover exact
  recorded owners. No reset, source compression or preassigned expected salt
  occurs in this mode.
- Compact integer widths and bounded arrays satisfy the 128-MiB stdout cap.
  Conservatively bounding each sequence at 256 bytes and all row cells as
  seven-digit U32 indices yields at most 119,559,680 bytes for both padding
  observations plus metadata. The actual sequence format is shorter. The
  controller must still enforce stream byte limits and a deadline, retain
  failure logs and reject missing completion.

Syntax checks passed with Apple clang 17.0.0 (clang-1700.4.4.1), target
arm64-apple-darwin24.6.0, `-std=c11 -Wall -Wextra -Werror`,
`-DZSTD_DISABLE_ASM -DZSTD_LEGACY_SUPPORT=0`, and the pinned external lib include
path. The normal check used `-O2`; the sanitizer-configured check used
`-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all
-fno-omit-frame-pointer`. Both used `-fsyntax-only` and returned zero with no
diagnostics. `ZSTD_MULTITHREAD` is left undefined, not defined to zero, because
upstream tests macro presence. Formatting uses clang-format.

These checks neither execute sanitizer instrumentation nor prove the unchanged
upstream parser free of undefined behavior. The source retains upstream
`no_sanitize("pointer-overflow")` annotations and unused dictionary expressions.
Later captures must instrument every linked C translation unit, disclose those
source exclusions and use no prebuilt support archive. Only successful recorded
normal/instrumented executions can complete the subsequent capture item.
