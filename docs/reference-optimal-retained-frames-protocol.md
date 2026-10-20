# Larger retained sparse Optimal protocol

`ZOPRS03\0` adds a 32768-byte source ceiling to immutable `ZOPRW02`.
It retains nine little-endian u32 header words, two-to-four u32 block lengths
and exact source bytes. The packet maximum is 32828 bytes. Sources are
128..32768 bytes; blocks 64..8192; W10..16/H6..12/C6..12/S1..6; minimum-match
3..7, target 0..4095, literal mode 0/1. Source must fit its window and schedule.
The proposed public extension uses balanced three/four-block schedules only;
the observation transport also retains old schedules for exact reconciliation.
No dictionary, prefix, window expiry, LDM or workers are admitted.

The existing 4099 path cells, 73730 maximum flattened search cells, canonical
sparse deltas, full frequencies, candidate sequences, guarded owner lifetimes,
two padding passes and complete publication observations remain unchanged.
There are at most 32768 events per block, 2 MiB per output line and 256 MiB
per positive command. All capture stdout/stderr together remain bounded to
512 MiB and the campaign to 1800 seconds, with 120 seconds per command.
Every frame artifact is at most 32832 bytes and each allocator ledger at most
2048 bytes. Compiler objects/binaries retain their separately checked limits.

The source-only manifest contains seven unchanged predecessor hypotheses and
twelve larger-source hypotheses, predicting 67321 events per padding pass in
total. Actual native records must reproduce all scalar and sparse states.
Publication kinds are independently read from complete emitted frames before
being accepted; source-only expectations never drive the C entropy decision.
Both normal and fully instrumented builds must agree, including both padding
passes and all complete frame bytes. Each complete frame is decoded by pinned
native code; independent Rust decoding is a subsequent gate.

Before this new native execution, both Python modes exactly replayed 75954
events of the seven previous wide native captures, rejected 64 controls and
checked all 16384 proposed source-length schedules. The larger model remains
an independent hypothesis until the new capture and its adversarial audit pass.
Production Rust admission is unchanged by this observation protocol.
