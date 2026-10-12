# Ordinary DoubleFast frame publication observations

These immutable observations come from unchanged facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, through ordinary internal-begin frame
compression. They are test oracles, not production C bindings. The separately
frozen source manifest contains51 cases/6,348,944 bytes/164 scheduled calls.
Normal and ASan/UBSan captures agree on every observation and complete frame.

The160 positive blocks contain88 raw,6 RLE and66 compressed blocks. There are148
fresh candidates;12 tiny blocks preserve stale scratch. Thirteen raw and three
RLE blocks discard meaningfully changed first-two candidate repeats while the
sampled tables survive. All12 directed raw-candidate recipes provide such a
witness; the additional raw witness is the nine-byte prefix case. First uniform
input is compressed, not converted to RLE. Window20 loses the final copy in the
A/1MiB-noise/A recipe, while window21 retains it; A/2MiB-noise/A expires even at
window21. These are observed input consequences, not intermediate probe traces.

Four complete observation/frame pairs remain equal after removing only their
specified input ID/parameter differences: target0/4, target0/131072, search1/29,
and minimum-match3/4. Positive DoubleFast target lengths do not impose Fast's
raw-literal policy. The full unequal-table observations retain long hashLog and
short chainLog as independent dimensions, including low/maximum combinations.

Each build fully decodes51 scheduled frames and seven aligned whole-buffer
comparisons:58 frames and12,382,352 decoded bytes. Thirteen additional header-only
probes are expressly incomplete and undecoded. Both builds compile all26 used
common/compress/decompress C translation units from pinned source; the sanitizer
instruments all those units and the helper. Assembly/legacy support and
LeakSanitizer are disabled. No support archive is linked. All192 capture commands
pass without runtime/sanitizer diagnostics. This is native arm64 macOS evidence,
not native32-bit execution.

The123 data artifacts preserve51 complete NDJSON state records,51 scheduled
frames, seven whole frames,13 incomplete-header record files and one native
metadata file. An independently pinned catalog binds every file's size/hash.
Source recipes regenerate separately and are verified before their generator is
imported. The helper review occurred before the first execution; original
source/command/binary/header identities and bounded logs are retained in the
project evidence archive. No direct or Fast fixture was rewritten.

    python3 scripts/check_double_fast_frames.py --check

The offline checker uses no compiler or subprocess. It verifies exact fields,
JSON scalar types, source/schedule identities, frame/block boundaries, expanded
sequences and literal bytes, overlap-aware source replay, raw/full wire repeats,
initialized owner continuity, publication/rollback and both complete table maps.
It also verifies raw/RLE payloads and aligned whole-buffer payload equality.
Full compressed payload decoding is recorded C capture evidence; the Python
checker does not contain a compressed-block decoder.

The explicit internal parameters remain distinct from public query/applied
parameter-resolution observations. The deprecated public sequence collector is
not called and its unconditional candidate publication is not substituted for
ordinary frame behavior. C and Rust entropy can choose different block kinds;
future persistent Rust tests must replay the specified incoming repeats and
publication schedule rather than assume independent frames share trajectories.
