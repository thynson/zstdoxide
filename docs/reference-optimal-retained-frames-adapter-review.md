# Larger retained observer adaptation review

The additive observer differs from the immutable wide helper only in protocol
identity and fixed source/frame buffer bounds. The generated adapter diff is
against `optimal_retained_wide.c`; the separate pinned upstream transformation
retains the same fourteen unique observation substitutions. No match provider,
parser branch, pricing transition or entropy decision is replaced. Ordinary
Begin/Continue/End calls observe both live compressed-state owners.

Source storage admits 32768 bytes plus existing guarded padding, decoded storage
32768, packet storage 32828, and frame storage 32832. The 64-byte frame allowance
exceeds the single-segment header, four nonfinal block headers and empty final
block. Worst-case raw data fits without changing native output decisions.
Each admitted scalar is checked before shifts/copies and complete source must
fit the selected window. Existing 4099 path cells, 73730 flattened search cells,
2731 per-block sequences and 32768 per-block events remain bounded and unchanged.
All prefix positions refer to the contiguous immutable guarded source.

The zero-initializing allocator retains alignment, 64-byte guards, 32 allocation
identities, an 8 MiB live-byte limit and exact release checks. Upstream sanitizer
workspace redzones remain unchanged. Fourteen workspace redzone pairs explain
the normal/instrumented allocation delta; larger observer-owned static buffers
do not become allocator ledger entries. Every event checks source/guard identity.
No future source bytes are supplied to independent search replay.

The command plan builds all 26 used C translation units under normal O2 and
ASan/UBSan O1, DEBUGLEVEL=2, assembly disabled and multithreading undefined.
Exact dependency/predefine gates run before compilation/execution; all source,
generated hooks, commands, environments, packets, objects, executables, logs,
frames and ledgers have closed inventories. Bounds remain 256 MiB per positive
stdout, 512 MiB aggregate logs, 120 seconds per command and 1800 seconds per
campaign. Pure preflight must reject forged plans without executing children.
Normal and optimized offline controls include digest-reconciled larger-source,
balanced-boundary and late-block publication forgeries in addition to inherited
state, frame, allocator and provenance mutations. Native and Rust correctness
claims wait for their corresponding completed gates; capture duration is not a
codec benchmark.
