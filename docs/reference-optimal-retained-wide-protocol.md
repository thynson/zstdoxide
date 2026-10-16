# Wider retained sparse Optimal protocol

`ZOPRW02\0` is additive to the immutable small `ZOPRT01` transport. It uses the
same nine u32 header words, two-to-four u32 block lengths, and exact source bytes.
All words are little endian. The maximum packet is 16,444 bytes. Source length
is 128..16,384; blocks are 64..8,192; W10..16/H6..12/C6..12/S1..6; minimum match
is 3..7, target is 0..4095, literal mode is 0 or 1. Source must fit the window.
No dictionary, worker, LDM or window-expiry interpretation is admitted.

Every block retains all 4,099 path cells. Path and search observations contain
`{length, changes}` objects, whose changed indices are strictly increasing and
whose values differ from the prior complete snapshot. Path values are seven-word
records. Search values flatten heads, both edges of every node, optional ordinary
short heads, main frontier and local short frontier. Their maximum length is
73,730. Baselines start at zero per padding pass and persist across blocks.
The `search_before` record for queries is measured before advancing the baseline;
it must have no changes from the preceding complete search state. The normal
post-query snapshot then advances the baseline. Other scalar events, frequencies,
sequences, frame publication, owner identity and guarded lifetime observations
retain their small-protocol meaning.

The output limit is 256 MiB per command, each line <=2 MiB, and 32,768 events per
block. Independent replay preserves global source coordinates but only compares
bytes through the current block end. Public Optimal strategy admission remains
unsupported until separately qualified. This protocol supplies no C match inputs.

Before wider C execution, normal and optimized Python each reconcile all 8,972
events from both padding passes of sixteen immutable small C captures after
lossless conversion to this representation. Both modes reject fifty malformed
transport/state controls and launch no children. Wider source-only branch
predictions and native capture remain subsequent gates.

The 2026-10-16 native capture passed all 124 planned commands. Seven recipes
produce 37,977 events, 12,510 queries and 31 sequences per build/padding pass.
All 28 complete frames and 28 allocator ledgers agree across normal/sanitized
builds and both padding values; 24 malformed packets are rejected per build.
The independent artifact gate checks the exact 110-artifact inventory. Normal
and optimized offline controls each accept two unmodified captures and reject
70 forgeries without launching children. Initial control-adapter attempts used
an obsolete executable filename and failed before the artifact mutation; their
logs remain beside the corrected passing runs in the evidence archive. Capture
inputs were unchanged. The archive contains 463 individually hashed members,
including all native logs and immutable source snapshots. Wider production
admission and fixture projection remain separate subsequent work items.
