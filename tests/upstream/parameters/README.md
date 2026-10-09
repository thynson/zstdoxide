# Pinned compression-parameter observations

This collection freezes facebook/zstd **v1.5.7**, commit
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, before the Rust scalar parameter
implementation. The C executable is an external test oracle. Cargo does not
compile or link it, and the reference checkout stays outside this repository.
Derived tables and the explicitly identified clamp adapter retain the upstream
BSD-3-Clause attribution in the repository's `LICENSE.zstd`.

Regenerate or verify using an already-built external reference archive:

```sh
python3 scripts/import_parameter_fixtures.py /path/to/zstd-v1.5.7
python3 scripts/import_parameter_fixtures.py /path/to/zstd-v1.5.7 --check \
  --report /tmp/zstdoxide-parameter-regeneration.json
```

The importer verifies the pinned checkout revision and rejects tracked edits
under `lib/` or `tests/`. It compiles `oracle.c` against that checkout's headers
and `lib/libzstd.a`, then checks every generated TSV byte. The manifest hashes
all fixture data, the generator, importer, relevant upstream sources, and the
existing formatted dictionary used by lifecycle probes. Its original `capture`
object records compiler, flags, archive hash, and host. Those execution details
are **not** stable fixture bytes: `--check` compares stable provenance and data,
and optionally records current execution details in a separate report. A
changed compiler path or a rebuilt archive does not by itself invalidate the
fixtures; a changed result does.

The C probe requires 64-bit `size_t`, 32-bit `int`/`unsigned`/`ZSTD_strategy`,
seven four-byte compression fields, and the captured signed conversion of
`0x80000000` and `0xffffffff`. The two 32-bit maximum-log rows come directly from
pinned header macros. They are **not a native 32-bit C execution**. This dataset
must not be used to claim arbitrary-host ABI parity.

## Schema

Every TSV has a header. Numeric columns use decimal text; enum/error numbers
are the pinned C values. Compression fields are always ordered
`window_log, chain_log, hash_log, search_log, min_match, target_length, strategy`.
The wire names distinguish hash and chain even where C's setter enumeration
orders them differently. A physical row's ordinal is its identity: boundary
values can deliberately coincide, so case labels and values are not unique
keys. Error code zero means success. Other observed classes are parameter unsupported
(40), parameter out of bounds (42), wrong stage (60), and wrong source size (72). `return_value` stores a successful public
return value and is zero on errors; raw size_t error encodings are not imported.

| File | Rows | Meaning |
| --- | ---: | --- |
| `presets.tsv` | 92 | Exact `ZSTD_defaultCParameters[4][23]` entries included from `lib/compress/clevels.h`, before adjustment. Tier 0 is largest/default, 1 is at most 256 KiB, 2 at most 128 KiB, and 3 at most 16 KiB. Row 0 is the negative-level baseline, not level-zero normalization. |
| `bounds.tsv` | 13 | `ZSTD_cParam_getBounds` for level, seven compression fields, source hint, three frame flags, and an unsupported parameter. |
| `width-bounds.tsv` | 2 | Host 64-bit bounds and separately labeled macro-derived 32-bit window/chain/hash/search maxima. |
| `queries.tsv` | 266 | Public `getCParams` and `getParams` agreement, reference frame defaults, and explicitly labeled private no-attached-dictionary resolution. |
| `adjustments.tsv` | 225 | All seven raw input fields, concrete validation error, adjusted fields, and resulting validation error. Public and typed-empty adapter cases remain separate. |
| `requests.tsv` | 150 | Ordered initialization/set/reset operations, returned classification/value, all requested getters, and resolved empty/unknown/known-16385-byte profiles. |
| `lifecycle.tsv` | 186 | Tiny bounded context operations, requested/applied values, stage/pledge state, dictionary attachment observations, and available frame-header fields. |

### Queries and source-size domains

Public level lookup covers `i32::MIN`, below-minimum `-131073`, minimum
`-131072`, other negative levels, zero, every level 1 through 22, 23, and
`i32::MAX` at all four representative tiers. Separate rows cover minus/equal/
plus one around 16/128/256 KiB, dictionary-adjusted tier boundaries, the
unknown-source dictionary-plus-499 expression, small-source adjustments, and
window-resize boundaries. Known empty and unknown input are also compared with
nonempty dictionaries.

`api=public` invokes both `ZSTD_getCParams` and `ZSTD_getParams` and asserts all
seven compression fields agree. Public input source zero is tagged
`public_zero_unknown`, matching C's conversion of zero to its unknown sentinel.
`api=typed_no_attach` calls `ZSTD_getCParamsFromCCtxParams` from the static
library with pinned private headers and `ZSTD_cpm_noAttachDict`; zero means
known empty there. It is an internal reference observation, not a public C API.

The `domain` column separates ordinary inputs from reserved-known and modular-C
extremes. C's known source/dictionary sum, unknown dictionary-plus-499, and
intermediate dictionary-plus-window expressions can wrap. Their observed
results are recorded so Rust can deliberately reject out-of-domain arithmetic,
not accidentally reproduce wrapping in one build mode. A typed `Known(u64::MAX)`
is reserved and should be rejected; `Unknown` legitimately maps to that same C
sentinel. `source_kind` preserves that distinction.

### Validation and typed adjustment adapter

`input_validation_error` is from the actual public `ZSTD_checkCParams` for the
raw seven-field record. `api=public` uses the actual `ZSTD_adjustCParams`, which
first clamps each field through a signed-32 comparison and then adjusts. Thus
`u32::MAX` does not simply clamp to the unsigned maximum. Each field includes
zero, its bounds and outside bounds, `i32::MAX`, `0x80000000`, and `u32::MAX`.
Rows across all nine strategies cover row-match hash limits and the binary-tree
chain-cycle distinction; additional rows cover source/dictionary adjustments.

`ZSTD_adjustCParams_internal` is private static C code. For
`api=typed_no_attach_adapter`, the probe uses a **source-derived test adapter**
for `ZSTD_clampCParams` (`zstd_compress.c:1401-1421`), installs the seven clamped
fields as overrides in a level-1 `ZSTD_CCtx_params`, then calls the actual
`ZSTD_getCParamsFromCCtxParams`. Level 1 has target length zero in all four
presets, so the override rule's special zero value preserves zero exactly;
all other clamped fields are nonzero. This exercises the actual final internal
adjustment with known-empty semantics. The public rows independently exercise
the library's clamp implementation. This adapter is not described as a public
C adjustment call.

### Requested values and lifecycle

Requested records contain all seven overrides (zero means automatic), requested
level, source hint, and the three public frame flags. Separate sequences cover
hash/level setter ordering, every override surviving level changes, clearing
all overrides to automatic, source-hint bounds and precedence, failed setter
atomicity, and resetting defaults. Resolved columns distinguish known zero,
unknown, and known 16385 bytes; source hints affect only unknown resolution.
The positive 16384-byte hint changes the unknown-size preset tier while leaving
both known-size columns unchanged, so ignoring a valid hint fails this witness.
Frame flag setters use C's nonzero truth conversion even when a supplied signed
value lies outside the advertised 0/1 bounds. This is recorded without implying
that a Rust boolean setter accepts arbitrary integers.

Lifecycle scenarios use fresh contexts and at most 32 source bytes, 4096 output
bytes per call, and a bounded existing formatted dictionary. Numeric level and
strategy settings in these probes control **the C oracle only**. The probe
observes private fields before the conditional MT/tracing members of the pinned
context layout; public getters provide requested values. Attachment columns
are booleans or sizes, never pointer addresses. Applied fields before first
initialization can be zero and are not presented as validated profiles.
`stored_dictionary_id` records the context's cached `dictID`, which may retain a
previous frame's ID after its dictionary attachment has been cleared; it does
not prove an active dictionary. `attached_cdict` can also refer to an internally
prepared loaded dictionary, not only a caller-supplied CDict.

The scenarios distinguish fresh, empty Continue with zero output, a partially
emitted header, active input, pending Flush, pending End, completed frames, and
a pledge error. Partial-header setup flushes 16 source bytes into a one-byte
output buffer and asserts that exactly the first magic byte was emitted; a
complete frame header is not claimed. The actual pledge-error setup first
continues one byte under a one-byte pledge, then ends with one additional byte
and asserts `srcSize_wrong` (error 72). A separate first-call End observation
records that C replaces the earlier pledge with that call's actual input size. All three C reset directives are observed. Dictionary cases
cover loaded dictionaries, externally prepared CDicts, and one-use formatted
prefixes, both before and after completing a frame. A header-present flag guards
all recorded header fields; absent-header numeric zero placeholders are not an
observation of an empty frame. Next-frame probes explicitly set their own
pledge after reset where needed; the immediately preceding reset row preserves
what the reset itself did to the old pledge.

Important observations remain qualified:

- C accepts another frame automatically after completion. Rust's explicit
  `Finished` phase still requires its own session-reset protocol.
- Bulk parameter application rejects non-initial C stream state and an attached
  CDict, despite broader header prose about live MT updates. Auto-detected
  formatted loaded dictionaries and formatted prefixes have separate observations.
- A fresh C parameter-only reset retains its previous pledge (the fixture keeps
  `pledged_plus_one=65`). Rust's parameter reset restores default options,
  including no pledge; this is a deliberate lifecycle boundary difference.
- Individual authorized effort setters can change requested values after start;
  applied values remain separately visible. A successful getter is not evidence
  that the current single-threaded frame changed its algorithm.
- An invalid active hash setter leaves requested values unchanged but can set
  C's `cParamsChanged` flag before failing bounds validation. This is not full
  context-state atomicity and does not weaken Rust's planned checked mutation
  contract.
- Shared/prefix ownership, poisoned state, allocation reuse, and retained history
  in Rust need independent Rust tests; pointer observations from C do not prove
  them.

This collection does not claim live numeric compression-level support,
prepared-CDict profile inheritance in Rust, algorithm/ratio parity, worker
semantics, or completion of the upstream test migration.
