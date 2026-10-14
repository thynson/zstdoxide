# Fresh ordinary Optimal startup prediction protocol

This additive source-only protocol combines the previously reconciled integer
search and path rules. It is not C observation evidence. Existing direct-search
and scripted-path captures and their guards remain immutable. The next CCtx
observer must preserve original provider selection and pass its own review and
preflight before these predictions can qualify production startup.

`ZOPST01` followed by zero identifies a packet. Eight little-endian u32 words
follow: source length, window log, primary hash log, chain log, search log,
minimum-match parameter, target and literal mode. The exact source bytes end the
packet; there are no scripts, client repeat inputs or trailing fields.

Admission is source 64..512, window 10..12, hash/chain 6..9, search 1..4, minimum
3..7, target 0..4095, literal mode 0/1. Strategy is fixed btopt/optLevel0, no
external dictionary, LDM or threads. Effective minimum is three for parameter
three, otherwise four. The short-key log is min(window,17) for minimum three and
absent otherwise. Fresh repeats are [1,4,8]; source positions begin at one and
search maps retain the upstream index bias of two. Invalid fresh distances are
ineligible, not source locations. No ordinary Rust query guard is relaxed here.

The predictive stream has metadata, two equivalent source-padding event passes
and a completion record. Each path event retains the previous scripted protocol's
complete path, frequency, sequence and repeat state, plus a complete search map
(heads, nodes, short heads and both frontiers). Query events additionally retain
the complete pre-query map; other events carry null in that field. This makes
unchanged search state between queries explicit. Streams are bounded to 1,024
logical events, 1 MiB per line and 128 MiB overall. Original raw arrays are retained;
there is no unchecked sparse reconstruction at this preliminary boundary.

The twelve recipes cover initial unavailable fresh repeats, no-match input,
ordinary tree/short-key/repeat discovery, skipped queries, literal-zero queries,
multiple sequences, all-literal path replacement, minimum parameters three through
seven and enabled/disabled literal pricing. `scripts/plan_optimal_startup.py`
checks frozen packet/source/event hashes and branch predictions. The model's
candidate production comes from integer sorted-search replay rather than script
inputs. It shares neither production Rust decisions nor output data.

Future CCtx capture must additionally bind applied parameters, zero-initializing
custom allocator records and an ordinary complete frame to this state stream.
Use the unchanged advanced initializer with explicit parameters and reject any
unexpected adjustment. The observer review must retain upstream pointer-overflow
attributes and document unused noDict pointer expressions; sanitizer success is
not an ISO C provenance proof. Full-frame decoding and offline forged-evidence
controls remain mandatory. Retained multi-block state is a separate extension.
