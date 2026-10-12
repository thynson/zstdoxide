# Ordinary Greedy C frame capture

The corrected ordinary helper observes unchanged facebook/zstd v1.5.7
(`f8745da6ff1ad1e7bab384bd1f9d742439278e99`) through explicit-parameter internal
begin/Continue/End calls. It runs outside the Rust library; it is not a C codec
binding or a source of production parsing. Source recipes were committed before
execution. The original 126 recipes remain unchanged, with 12 additive
window-floor/tiny-call recipes. The first v2-helper attempt failed on the empty
chain case because the adapter reset an already-resolved switch to `auto`.
That failed report and all logs are preserved separately. The reviewed v3
helper retains the library initializer's resolved selection; schema 2 is unchanged.

The successful successor completed 440 bounded commands. Each normal/O2 and
ASan+UBSan/O1 build compiled all 26 used common/compression/decompression units
and the helper, without an archive, assembly, legacy codecs or workers. Every
input source/header hash stayed unchanged before and after commands. Normal and
instrumented NDJSON, frame bytes, whole-frame comparisons and header observations
agree byte for byte. Upstream function-level sanitizer annotations are retained;
LeakSanitizer is disabled. These qualifications preclude an unqualified claim
that all upstream C operations were checked by every sanitizer.

The observed corpus contains:

- 138 inputs, 7,365,784 source bytes and 403 scheduled calls, including eight zero
  calls. There are 395 positive blocks and 367 fresh candidate generations.
- 181 raw, 38 RLE and 176 compressed blocks. Candidate repeat changes are
  discarded on 25 raw and 12 RLE choices. Complete selected head/link or
  position/tag maps are retained across all calls, separately from publication.
- 151 complete C frame decodes per build (138 scheduled and 13 aligned whole
  frames), totaling 13,587,614 decoded bytes per build. Whole-frame comparisons
  ignore only the final-bit/explicit terminal-empty-block distinction.
- 15 incomplete header probes per build, explicitly not decoded, including
  small content-size transitions, window transitions and the 32-bit size edge.
- 35 malformed commands rejected in each build, with exit 1, a helper diagnostic,
  no sanitizer diagnostic and no output frame.

Both branches' mixed fallback cases select compressed/raw/RLE/compressed.
All 24 constructed short candidate cases preserve changed-repeat raw fallback.
Row window17 expires a remote match which window19 retains; window20 expires
the larger witness which window21 retains; the later window21 expiry case is
raw throughout. These are measured family outcomes, not universal behavior
claims for the parameter values.

All twelve additive tiny-floor cases select compressed/RLE/raw/raw. With a
one-byte tiny call, the chain frontier changes from raw index834 to1026 and the
row frontier from32578 to32770, exactly their block-start floors. Both tables
are unchanged during that call; the following fresh noise block resumes table
updates. Other sampling fields remain unchanged. The fresh 384/192 adjustment
is distinct and occurs only after the floor clamp. Zero calls do neither.

For both branches, target0/4/131072 produce identical observations after
normalizing only the case ID and applied target field, and identical complete
frames. Long-match recipe names denote source lengths: source65543 yields
match65539/raw-base0 with marker type2. They do not claim exact match-length
cutoff bracketing. The long-literal witness is literal length65796 with raw260
and marker type1. Earlier direct fixtures retain their distinct boundary scope.

The independent offline auditor checks all exact compile/link/helper commands,
source and generated-input closure, dependencies, effective controls, runtime
environments, logs and their hashes, frame bounds, table deltas/fingerprints,
source hash membership, repeat replay, owner publication and normal/sanitized
identity. Normal and optimized Python runs each reject 47 forged evidence or
actual-record cases with child creation blocked and zero child attempts.
The immutable packed collection and full capture archive allow replay without
rerunning C. Source/hash and artifact identities are recorded in their manifests
and the accompanying machine-readable evidence.

The subsequent checked Rust projection validates all 151 C frames and retains
full table/sequence trajectories. Retained Greedy state reproduces all 395
positive blocks without reseeding tables. Complete private frame tests decode
274 Rust frames over the same 138 inputs. Scheduled block choices and committed
wire repeats match C, including all 25 raw and 12 RLE discarded repeat changes.
The encoder uses its existing entropy implementation; complete compressed bytes
are not claimed identical to C.

All 6,184 allocation requests observed across the 138 whole-frame encodes are
failed individually. Every failure returns the allocation error and releases
every exact owner. Separate chain/row append failures preserve previously
published output and repeats, discard pending Huffman state and poison the
workspace. Empty/tiny allocation counts, invalid schedules, borrowed non-clone
allocators, target and match-width equivalences, and retained-window effects
are checked. All six frame tests pass for safe, each isolated new unsafe leaf,
and all features. Strict safe/all Clippy, formatting and safe/combined-leaf
no_std compilation pass.

Public Greedy admission still rejects at this checkpoint. Public parameter
resolution, broader public C interoperability, collector behavior, intermediate
per-probe chronology and complete upstream test migration are separate remaining
obligations. Admission follows the frozen v3 mapping.
