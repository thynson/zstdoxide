# Direct small-log eight-byte hash observations

## Pre-execution design and review

This work item isolates DoubleFast's private long-key hashes at logs6..9.
Its inputs are the existing 64 distinct eight-byte records in
`tests/upstream/match-search/patterns.bin`, whose original deterministic
generation and upstream attribution remain unchanged. For each log, run all
keys at stack-array shifts0 and1, giving 4*2*64=512 primitive observations.
Expected values must come from unchanged pinned `ZSTD_hashPtr`, not Rust.

The new C test helper reads exactly 512 bytes from a checked regular file and
rejects incomplete reads, trailing bytes and I/O errors. Its only byte owners
are the declared 512-byte pattern array and initialized 16-byte input array.
The latter has explicit eight-byte alignment. Copying8 bytes to shift0 or1
and reading8 bytes from there remains within that array. Pattern indices0..63
likewise select complete keys within the input array. Logs6..9 make both the
hash's high-bit shift and the bucket-bound shift valid. No heap allocation,
input-controlled integer domain or dynamically constructed source pointer is
involved. These implementation bounds were reviewed before its first execution.

Compile normal and scoped ASan/UBSan helper builds against unchanged pinned
headers. The hash is inline, so no archived parser object supplies the expected
operation. Record source/compiler/binary identities and complete command logs.
Both builds must emit byte-identical512-row TSV. Independently verify every
row's identity, input bytes, range, wrapped LE64 arithmetic and agreement with
the high bits of the original pinned log10 width8 observations. This separately
qualifies native unaligned C reads and Rust's alignment-independent byte copies.

Freeze an exact TSV identity and provenance before adding its Rust consumer.
Then test the private layout and real MatchTable read/write/collision behavior
against these direct buckets under safe, unsafe-match-table, all-features,
no_std and no_std-plus-all configurations. Existing named/Fast layout domains
and fixtures remain unchanged. This primitive checkpoint does not claim a
DoubleFast parser, frame output, performance or foreign-platform execution.

## Completed capture and Rust comparison

On 2026-10-12 all nine capture commands pass. Normal and scoped ASan/UBSan
builds each emit the same 512-row TSV, SHA-256
`16ee71dfb607ef5e6dfbbdf7ed3fd65599b1e5e24bfe701c2cecd76672c43cad`.
All rows agree with both independent wrapping arithmetic and the prior pinned
log10 high bits. The original source files, patterns and old fixture bytes
remain unchanged. No library archive is linked; LeakSanitizer is explicitly
disabled. The complete hash/helper inline code is compiled with sanitizers.

The new Rust test checks all 512 bucket values, collision replacement, index0
as a valid candidate, and the entire initialized table after each insertion.
It runs with the other ten DoubleFast foundation tests in five configurations:
safe, unsafe-match-table, all features, no-default, and no-default plus all
twelve unsafe features. All 55 focused executions pass. Twenty-two existing
Fast/named baseline executions also pass. Strict all-target Clippy safe/all,
no_std safe/all and Rust formatting pass: thirteen recorded commands and
77 passing tests, with no ignored tests in those selected suites.

The compiler-free checker and complete fixture registry pass under normal and
optimized Python. Each mode additionally passes one positive and 28 rejection
controls: altered/missing/extra/reordered rows, noncanonical scalars, forged
provenance, repaired fixture hashes, duplicate/nonfinite JSON and malformed
C input lengths in both builds. Registry totals are 546 files in 18 collections.
C formatting and diff-whitespace checks also pass. The only Rust change is a
test; this checkpoint makes no new production algorithm or unsafe operation.

See [evidence](evidence/2026-10-12-double-fast-hashes.json) and the
[capture, source and control archive](evidence/2026-10-12-double-fast-hashes.tar.gz).
The retained capture script can be run from the repository root with
`--upstream /path/to/pinned/zstd --output /tmp/fresh-hash-capture`.
