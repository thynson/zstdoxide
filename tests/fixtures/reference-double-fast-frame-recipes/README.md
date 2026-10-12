# Ordinary DoubleFast source hypotheses

This manifest pins51 independent source/schedule recipes before an ordinary
DoubleFast frame oracle runs. The total is6,348,944 source bytes,164 scheduled
calls (160 positive), seven aligned whole-buffer comparison requests, and13
separately incomplete header-only probes. No expected compressed bytes, block
kinds, sequences, repeats, tables or entropy states are generated here.

    python3 scripts/check_double_fast_frame_recipes.py --check

The checker verifies the generator identity before importing it, checks the
independently fixed manifest hash and reconstructs every source. Recipes contain
complete explicit W/C/H/S/M/T/strategy tuples, source hashes and ordered call
sizes. Their scope is ordinary internal-begin frame mode, not public parameter
resolution or the sequence collector. Names and hypotheses do not assert that
the intended raw/RLE, retention or discarded-repeat witness has been observed.
The original Fast and constructed direct DoubleFast captures remain unchanged.

The protocol and allocation/observation review requirements are in
`docs/reference-double-fast-frame-oracle.md`. The new generator's noise and
small-period recipes reuse the attributed deterministic xorshift32 construction
of the frozen Fast test scaffold; it never imports a production Rust codec.

Attribution: facebook/zstd v1.5.7, revision
`f8745da6ff1ad1e7bab384bd1f9d742439278e99`, copyright Meta Platforms, Inc. and
affiliates. This project selects BSD-3-Clause, provided in `../../../LICENSE.zstd`.
