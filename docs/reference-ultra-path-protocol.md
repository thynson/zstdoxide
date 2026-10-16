# Coupled Ultra scripted path observation protocol

Pinned reference: facebook/zstd v1.5.7,
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`. This successor observes unchanged
`ZSTD_compressBlock_opt_generic` pricing/path statements using a scripted match
callback. It does not qualify ordinary match discovery, retained contexts or
complete Ultra frames. Reference code and generated translation units stay
outside the repository.

## Request ownership and bounds

`ZUPTH01\0` is followed by six little-endian u32 fields: source size, script
position count, minimum match parameter, sufficient-match target, literal mode,
and policy. Source bytes follow the 32-byte header. Each script entry contains
position/count words and that many (offset intent, length) pairs. The complete
file is at most 12832 bytes; no trailing bytes are permitted.

Source sizes are 9..8192, script positions 0..64, candidates per present position
1..8, minimum parameter 3..7, target 0..4095, literal mode 0/1, and policy 0/2.
Positions are strictly increasing and within 1..source_size-8. Candidate lengths
increase strictly, begin at the effective minimum (3 or 4), and fit the remaining
source. Offset intents are nonzero and at most position+3. Direct-distance byte
matches are checked during complete request validation. Repeat intents are
checked against the actual history and literal-zero state at every callback.

The adapter opens a non-symlink regular file without blocking, checks its size
through the open descriptor, reads exactly the bounded contents and verifies EOF.
All transport/header/direct-match checks finish before stdout or parser calls.
The 21 malformed requests have exact fixed diagnostics; internal guard failures
cannot count as successful malformed-input rejection.

The 58 positive requests wrap all 27 original small/horizon requests unchanged
under both policies and add two independently selected alternative-branch scripts
under each policy. The source-only model predicts 65672 events and 143 sequences
per padding pass across the corpus, including 72 saved-match alternatives.
These predictions are not successor native observations until capture succeeds.

## Observation state

Each execution runs twice with distinct nonlogical padding. Metadata binds
revision, source fingerprint, parameters, policy, script extent and path extent.
Every event includes policy, stage/phase, input and anchor indices, cursor and
frontier, optional stretch/reverse/query data, repeat history, canonical deltas
for complete seven-word path cells, all 377 frequencies, four sums, four cached
bases, price mode and the complete sequence prefix. A completion record closes
both passes with exact event and sequence counts. All logical output must agree
between padding passes and between normal and fully instrumented builds.

The new `alternative` stage occurs after the two candidate prices have been
computed and before their strict comparison. Its 12 integer values are the saved
seven-word match, signed LL(1)-LL(0), match-plus-one-literal price, competing
longer-literal-run price, existing next-cell price, and predecessor index. Other
stages carry `alternative: null`. The subsequent `literal` event observes the
accepted next-cell write, repeat publication and any frontier extension.

Path storage is the pinned 4099-cell allocation. Only min(source_size+3,4099)
logical cells may change; the tail remains zero. Guarded frequency, path,
candidate, sequence and literal output owners are separate. Complete source,
prefix/tail padding, table guards, sequence bytes and copied literals are checked
after every observation. Static observer storage is below 4 MiB. There are at
most 16384 events per padding pass, 128 MiB stdout per positive execution, and
512 MiB aggregate command logs. Each child has a 120-second limit and the whole
capture has a 30-minute limit.

## Independent gates

The policy-0 successor replay must reproduce the original 3,211,235-byte C fixture
exactly before execution. Source-only model checks cover strict alternative
comparisons, negative signed increments, predecessor history, frontier extension,
continued queries and descending relaxation after rejection. The native plan has
220 commands: reference/compiler identity, full dependencies/predefines, 52 object
compilations, two links, 116 positive executions and 42 malformed executions.
All 26 used codec units are ASan/UBSan-instrumented in the sanitized build.
No subprocess is permitted during offline capture audit or forged-plan checks.
