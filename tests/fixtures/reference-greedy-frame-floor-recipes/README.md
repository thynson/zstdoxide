# Additive tiny window-floor inputs

These 12 source-only hypotheses supplement, without changing, the original
126 ordinary Greedy frame recipes. Six chain W10 and six row W15 cases place
lengths1..6 after two uniform window-sized blocks, then resume fresh parsing
with 33 noise bytes. They contain 405,942 source bytes and 48 positive calls.

The frame loop's nextToUpdate window-floor clamp precedes the tiny sequence
store bypass. The reviewed successor helper accounts for this ordering; see
`docs/reference-greedy-frame-helper-v2-review.md`. The original helper remains
unexecuted. No C observation or emitted block-kind witness is claimed yet.

Generator SHA256:
`e30cb2c60762b333d6e4b510e9c9a5384c43a20e93209e89d52716f099bae2c4`.
Manifest SHA256:
`8414ea9f5f9eee715b22f473023a299331df5f8abe11f974202bda03af58cfbd`.
The generator pins and reuses only the original source recipe/value checker;
neither a C codec nor the Rust parser participates in input construction.
