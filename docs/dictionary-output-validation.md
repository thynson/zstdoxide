# Dictionary-backed caller-owned output qualification

`decompress_into_with_dictionary` and `decompress_into_with_dictionary_in`
write into a caller-owned slice using a borrowed shared dictionary or view.
Raw content, prepared entropy and initial repeat offsets are independent from
scratch allocation. The shared frame loop applies IDs before window/output
limits and restores dictionary state at every standard frame. Skippable frames
do not consume a persistent selection. Prefix behavior is outside this module.

The output slice supplies all current-frame history. A checked match can start
in the dictionary and cross into already produced frame bytes with forward
copy semantics. The source dictionary may exceed the frame window before the
frame has displaced it. Once frame position exceeds the declared window,
dictionary-origin matches are rejected according to the existing Rust decoder
policy. Earlier concatenated-frame output is never match history. The complete
match destination range is checked before any write; capacity failures preserve
bytes outside the allowed output. Successful unused suffixes remain untouched.

## Staged source and ownership checks

The storage, engine and public-api evidence archives preserve distinct source
snapshots and their focused checks. Storage and private integration each pass
17 commands; public admission passes 24. Safe, isolated slice history, isolated
dictionary, their combination and all features exercise the actual storage
implementations. Strict Clippy, formatting and true no_std checks accompany
these gates. Private integration preserves all old no-dictionary contracts.

Five public tests cover raw/prepared fixtures, exact/short/zero destinations,
independent window and output limits, every truncated prefix and byte mutation,
concatenated frames and guard bytes. Borrowed nonclone dictionary and scratch
allocators retain independent identities. Every observed scratch allocation
ordinal is failed separately; all tracked payloads are released. Raw, RLE,
empty and skippable decoding makes no codec scratch payload requests, even when
a prepared dictionary is selected. Neither entrypoint clones its dictionary
handle or its allocation owner. Plain adapters share a safe representation;
unsafe-slice-history and unsafe-dictionary remain independently selectable.

The archives preserve the original ambiguous test-integer compile failure and
the later strict-Clippy failure for an unnecessary test feature attribute.
Successful reruns bind the corrected source. Neither failed attempt is counted
as a passed command, and neither required a production algorithm change.

## Pinned native output checks

`evidence/2026-10-26-dictionary-output-native.json` records 119 descriptors through
native usingDict and prepared usingDDict, each in normal and ASan/UBSan builds:
476 calls. Rust shared and borrowed-view APIs run every descriptor under five
storage selections, totaling 1,190 calls. Independent raw/prepared originals,
real prefix/suffix guards, successful unused suffixes and source hashes are
retained alongside helper sources and native executables.

The descriptors include known/omitted/mismatched IDs, prepared treeless literals
and repeat sequences, duplicate initial repeat offsets, raw dictionaries,
dictionary-to-frame crossing, persistent concatenation, exact/short output,
truncation and checksums. The p>window fixture is deliberately native-success
and Rust-CorruptData under the established format-based rule; this is a recorded
difference, not a falsely claimed equality. Other malformed error precedence
and provisional bytes are not required to match C. The independent oracle is
never linked into the Rust implementation.

## Platform qualification

`evidence/2026-10-26-dictionary-output-platform.json` records safe/all strict-
provenance Miri on i686 and s390x: four actual interpreter executions. Raw far
dictionary offsets, dictionary-to-frame crossing, prepared four-stream treeless
literals, concatenation, exact/short output, expiry and invalid source ranges
exercise real output guards. These are foreign-width/endian interpretation,
not native foreign-hardware runs.

Two thumbv6m core+alloc checks and two concrete release builds pass. Retained
LLVM IR defines `dictionary_output_codegen_probe`; retained object headers
identify little-endian 32-bit ARM ELF. The probe uses a raw dictionary view and
the real allocator-aware public direct-output entrypoint. The production files
are hash-identical to the candidate; only the concrete probe is added.

## Corpus, full-matrix and performance gates

The changed dictionary_decompress adapter preserves its tail-selected source,
training ranges, attachment mode and destination capacity. Prepared/loaded
persistent branches use guarded direct output and compare success/bytes with
the prior buffered decoder; prefix branches remain buffered. Training uses the
independent Rust algorithm and does not claim native fastCover byte equality.
The five storage configurations, strict Clippy and formatting have passed.

`evidence/2026-10-26-dictionary-output-corpus.json` records the complete 12,063-file
rolling corpus (353,411,091 bytes) under safe/all release with zero skipped files.
The archive pin is SHA-256
`4165df80490877b9b8f32dc0460d49896a362e192369cdc77ad1a0fd54e263e8`.
Every member hash and per-input branch/size/capacity/training/outcome ledger is
retained. Safe/all ledgers agree: 10,490 prepared selections, 440 loaded and
1,133 prefixes; 10,930 branches therefore execute actual direct destinations.
There are 11,768 trained dictionaries and 295 explicit empty fallbacks. Twenty
bounded decodes succeed and 12,043 reject malformed/limited inputs; accepted
fuzz errors are not valid-frame claims. The original source selectors and prior
buffered success counts remain unchanged. Corpus payload stays external and is
not asserted to belong to the v1.5.7 source tag.

The complete final feature matrix, measurements and closure are separately
recorded below.
The final matrix includes the native/platform tests and changed corpus adapter
before its source freeze. Original native contexts, workspace controls, complete
upstream dictionary/training APIs and complete test migration remain partial.

## Complete regression, measurements and reconciliation

`evidence/2026-10-26-dictionary-output-feature-matrix.json`
records the complete suites: 1,598 passed and 59 explicitly ignored
tests across 84 result groups in each configuration. All twenty isolated
unsafe features and safe/all configurations have complete regression, true
no_std and strict Clippy qualification. Formatting and complete all-feature
release pass; safe release is also complete.
Ignored external or unsupported assertions are not counted as passed tests.

`dictionary-output-benchmarks.md` reports all 22 primary backends and independently rebuilt
safe/all repeats on identical deterministic workloads and allocation ledgers.
Source, fixtures, executables, CPU barriers and raw samples are retained. These
fixed-order host measurements do not establish a universal unsafe speedup.
`evidence/2026-10-26-dictionary-output-closure.json` rehashes this module's evidence and binds
the integrated production source to final matrix, native/platform, corpus where
applicable, and measured executable source identities. Previous milestones are
linked through their closure receipts; historical checks are not relabelled as
fresh executions. Complete upstream migration remains open.
