# Direct lazy-family oracle protocol and allocation review

This contract precedes implementation and execution. It extends the reviewed
[direct Greedy contract](reference-greedy-oracle.md) without modifying that
protocol, helper, fixtures or captures. The reference remains facebook/zstd
v1.5.7 at `f8745da6ff1ad1e7bab384bd1f9d742439278e99`. No new C result is claimed.

## Versioned packet and dispatch

Use the same bounded 152-byte little-endian scalar header, seed grammar and
source payload, with these two changes:

| Offset | Bytes | New field |
| --- | ---: | --- |
| 0 | 8 | Magic `ZLAZY01` followed by one zero byte |
| 144 | 4 | Lookahead depth: exactly 0, 1 or 2 |

Offset 148 remains a reserved zero u32. Offset 72 remains the separate branch
selector 0=chain, 1=row. All other fields, ranges, exact-EOF rules, ID grammar,
regular-file requirements and checked allocation arithmetic remain those in the
Greedy contract. Reject the old magic and any unknown depth before allocation.
Depth zero is a compatibility control; it does not silently reinterpret an old
packet. Set the actual cParams strategy to Greedy/Lazy/Lazy2 for depth 0/1/2 and
run the unchanged parameter validator before constructing a match state.

Dispatch only to these original entry points:

| Depth | Chain | Row |
| --- | --- | --- |
| 0 | `ZSTD_compressBlock_greedy` | `ZSTD_compressBlock_greedy_row` |
| 1 | `ZSTD_compressBlock_lazy` | `ZSTD_compressBlock_lazy_row` |
| 2 | `ZSTD_compressBlock_lazy2` | `ZSTD_compressBlock_lazy2_row` |

The helper supplies state and observes output; it must not implement selection,
scoring or expected sequence decisions. Both branches may be forced for a
direct recipe independently of the public automatic window-based choice.
Every packet remains at most 4,325,528 bytes. Source size is at most 2,228,224;
prefix at most 2 MiB; block length is 7..131,072 and at most the explicit window.
Chain search uses at most 256 visits. Row width/visit limits remain 64, with
the existing direct oracle's deliberately narrower unused chain-log bound 18.

## Owners and additional lookahead proof

The exact independent owners, 64-byte alignment, guard arithmetic, before-images,
source padding, replay allocation and 64-MiB aggregate live cap are unchanged.
The complete ledger is specified in the Greedy contract. No lookahead array,
extra speculative owner or packet-supplied pointer is introduced. Both selected
table maps are retained; no inactive family is allocated. Match/sequence records
start zero-initialized and naturally aligned. Each free uses the allocation's
exact original base; failure produces no complete successful observation.

The earlier leading-64/suffix-2048 proof still covers initial source/base/tail
pointer formation, final miss increments and literal wildcopy slack. The depth
extension additionally requires these facts to be reviewed in the actual code:

- The main search position is strictly below `N-8` for chains or `N-16` for
  rows. Each speculative increment is conditional on the old position being
  below that limit; therefore the new position is at most the limit. A repeated
  improvement can extend the number of iterations, not their admitted extent.
- At equality, a chain's physical eight-byte hash read ends at logical N. A row
  hash-cache read at current+8 also ends at N after its physical eight-byte load.
  Selected keys need at most six bytes. Four-byte repeat/improvement checks fit
  within the same logical block. No source bytes from a later block are required.
- Candidate indices still precede their query. Row candidates are collected
  before inserting the current position; chain updates exclude the query.
  Cyclic-chain stopping rules, moving floors and sorted seeds remain unchanged.
  Cache and table writes from rejected speculative matches remain observations.
- A selected candidate has at least four bytes and its start is no later than
  the last speculative query. A failed depth-one/two comparison advances at
  most two bytes beyond the current selected start before storage. An accepted
  explicit candidate resets that start and continues. Stored ranges therefore
  advance the literal anchor beyond sampling already performed; they do not
  make the next ordinary parser call move backward through source history.
- Repeated match extension and backward extension retain the existing admitted
  distance/source-floor conditions. Match counting ends at logical N. Neither
  padding pattern may influence a selected literal, match or table transition.
- Lookahead creates no additional stored sequences until selection finishes.
  Every stored match consumes at least four bytes; the existing conservative
  `Q=floor(B/3)+1` sequence/code capacities remain sufficient, including zero-
  literal cleanup. Literal capacity B plus initialized wildcopy slack is unchanged.

Run every recipe twice with source padding 0x6a and 0xc3 and compare all semantic
results. Source and every padding/guard byte remain unchanged. Retain upstream
sanitizer annotations and disclose their exclusions; instrumented execution is
not an assertion of ISO-C validity of unused upstream null-pointer expressions.
The Rust implementation will use checked integer indices and signed scores.

## Observations and fresh salt

Direct stdout remains exactly four newline-terminated JSON records: metadata,
two padding observations, and completion. Add an exact integer depth to metadata,
each case and completion; identify the actual strategy and entry point. Preserve
all original source, seed, scalar, full table-map/delta, literal/sequence, raw
parser-repeat, semantic wire-repeat, salt/entropy, cache and owner observations.
The revised protocol identity must appear in metadata. Final maps describe final
state, not a chronological trace or inferred probe count.

Use separately named `--fresh-salt-0`, `--fresh-salt-1` and `--fresh-salt-2`
modes. Each creates two fresh CCtx values sequentially with fills 0x3b/0xd1,
the existing guarded 8-MiB ledger and W15/C6/H6/S1/M4/T0, selecting the actual
strategy 3+depth. Enable rows explicitly, disable workers/LDM/dictionaries and
checksums, initialize once for source size 32768, verify both applied parameter
views and observe salt before/after. Free each context before the next. Do not
compress, reset or reuse a context in this mode. Metadata, salt records and
completion include the exact depth; the observed resolved tuple ends in 3+depth.
Source-derived common initialization and earlier Greedy captures are cross-checks,
not substitutes for these separately identified Lazy/Lazy2 observations.

Independent replay must check exact schema/types, names, source/packet hashes,
seed-built initial tables, every full-cell fingerprint and delta, valid row heads,
positive past indices, expanded long lengths and complete overlap-safe sequence
reconstruction. Raw C repeat three remains separate from semantic wire repeat
three. No codec, compiler, subprocess or archived program may run in offline
replay. Add malformed depth, old magic, incoherent strategy/entry-point and
depth disagreement to the existing structural and semantic rejection controls.

The constant extra depth fields do not change the previous 128-MiB stdout,
1-MiB stderr and 120-second per-process bounds. Recheck compact formatting bounds
against the implemented schema before first execution. Preserve timeout/kill,
return-code and incomplete-output failures instead of admitting partial evidence.

## Source and execution gates

Freeze the actual helper/Python transport and source-only recipes separately.
Review the complete pointer/owner proof and pass normal/instrumented syntax
checks before any C execution. Compile every used pinned C unit for normal and
ASan/UBSan builds, with no prebuilt support archive. Bind compiler identity,
dependency closure, controls, object/executable hashes and all command logs.
Reject overlapping/protected/symlinked destinations before spawning or writing.

Initial recipes cover both depths and branches, fresh/prefixed tiny tails,
inclusive equality queries, minimum-key clamps and unavailable repeats. Broader
recipes must distinguish initial repeat/table ties, strict gain equality and
improvements, repeat versus explicit lookahead, multiple improvements, row
widths, long gaps, cyclic wrap, skipping, floor expiry and entropy wrapping.
Depth-zero packets with otherwise unchanged inputs provide predecessor controls.
Freeze inputs before C capture; no Rust encoder may manufacture expectations.

## Implemented helper review before execution

`scripts/oracles/lazy_direct.c` now implements this separate packet and all six
unchanged upstream entry points. The added depth is validated before variable
allocation, maps through statically asserted strategy values 3/4/5, and appears
in metadata, case, salt and completion records. The old reserved word at 144 is
read only under the new magic; word 148 remains zero. The exact salt-option
string checks establish the digit's index and range before reading it.

Source review confirms unchanged owners, aggregate costs, pointer setup, map
validation and independent sequence replay. Twenty-four ownership/hash/replay
function bodies are token-equivalent to the reviewed Greedy predecessor after
discarding whitespace/comments and normalizing adjacent string concatenation.
The intentional differences are depth/strategy dispatch, the packet/schema and
fresh-context strategy selection. Direct calls use the existing bounded source
block; the helper performs no lookahead or match selection itself. The additional
lookahead-at-equality proof above applies to the original selected C functions.

Normal and ASan/UBSan syntax modes pass for the helper and all 26 used pinned C
units. The helper also passes `-Wall -Wextra -Werror`. This is syntax validation,
not an instrumented execution or a new output observation. All six compiler
identity/target/syntax commands and unchanged inputs are preserved in the
[review evidence](evidence/2026-10-12-lazy-helper-review.json) and
[archive](evidence/2026-10-12-lazy-helper-review.tar.gz). No helper executable has
run at this checkpoint. Python transport, frozen recipes and capture validation
are the next separate gates.

## Checked transport before new observations

`scripts/oracles/lazy_direct.py` now validates the new packet and all selectors
before reusing the unchanged common table/sequence/owner replay. It projects
only independently checked protocol, depth, strategy and entry-point fields;
every source, delta, repeat, literal and allocation observation still reaches
the complete predecessor validator. The frozen Greedy module remains unchanged.

Normal and optimized Python each accept 259 controls and reject 52 malformed
or changed inputs, with child creation blocked and zero attempts. Positives
include 108 packet round trips, 108 synthetic selector metadata records, 36
translated depth-zero Greedy observations, six synthetic salt-schema records
and one restored positive. The synthetic depth-one/two records test the schema;
they are not new C observations or expected Lazy/Lazy2 results. Rejections cover
old magic, malformed depth and reserved fields, bool/float selectors, wrong
strategy/entry point/completion, common table/sequence/owner corruption, JSON
ambiguity and incomplete records.

The [transport evidence](evidence/2026-10-12-lazy-transport.json) and
[archive](evidence/2026-10-12-lazy-transport.tar.gz) preserve both complete runs,
the reviewed source closure and control driver. Frozen new recipes and actual
C capture remain separate gates; no helper executable has run here.
