# Frozen ordinary Greedy observations

These 138 source-only recipes were captured from unchanged facebook/zstd v1.5.7
through the reviewed `greedy_frame_v3.c` helper and versioned capture controller.
`capture.json` indexes consecutive, SHA256-checked extents in `artifacts.bin`:
normal NDJSON, complete scheduled frames, aligned whole-frame comparisons,
native metadata and qualified incomplete header probes. The instrumented build
produced exactly the same observations and frame bytes. The complete capture,
including every command, rejected-input diagnostic, input and binary, is retained
in the evidence archive named by the manifest. No Rust encoder generated these
expected outputs. Run `scripts/check_greedy_frames.py --check` to replay offline.

There are 395 positive blocks (181 raw, 38 RLE, 176 compressed), 367 fresh
candidate generations and 37 changed-repeat candidates discarded on fallback
(25 raw, 12 RLE). Each build decoded 151 complete frames / 13,587,614 bytes and
rejected 35 malformed commands. The 15 header probes are incomplete and were
never decoded. Source/sampling/state observations do not trace every parser
probe, and these internal explicit parameters do not establish public request
resolution or collector equivalence. See the Greedy frame validation report.
