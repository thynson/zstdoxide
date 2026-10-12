# Source-only DoubleFast recipe identities

These 168 inputs in fourteen families were fixed before running their wider C
capture. They contain no expected parser output. `recipes.json` identifies the
source generator, frozen ZDFST01 helper/recipe layer, all parameters and ordered
seeds, and exact source/packet hashes. The inputs total 5,242,885 bytes; the
largest packet has 2,228,372 bytes, within the reviewed helper cap.

The independent offline checker reconstructs all identities and checks the
binary envelope separately. See `docs/reference-double-fast-families.md` for
hypotheses and limitations. The original tiny observations remain unchanged;
this directory is an input plan, not an upstream output fixture collection.

`refinement-long-literal.json` was fixed separately after observing that the
original long-literal candidate produced no sequence. It records an additional
896 matching bytes under a new case name, giving a 1024-byte tail. The original
168-case manifest and all its inputs remain unchanged. The combined family
checker verifies this separate identity and the resulting C observations.
