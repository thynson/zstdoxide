#!/bin/sh
set -eu

# Keep the official build outside the project. It supplies fixtures and oracle
# answers only; Cargo never links it into zstdoxide.
: "${ZSTD_UPSTREAM:?Set ZSTD_UPSTREAM to the external v1.5.7 source checkout}"
export ZSTD_BIN="${ZSTD_BIN:-$ZSTD_UPSTREAM/programs/zstd}"
export ZSTD_DECODECORPUS="${ZSTD_DECODECORPUS:-$ZSTD_UPSTREAM/tests/decodecorpus}"

python3 scripts/audit_upstream_tests.py --upstream "$ZSTD_UPSTREAM"
python3 scripts/import_upstream_golden.py "$ZSTD_UPSTREAM" --check
python3 scripts/import_upstream_regressions.py "$ZSTD_UPSTREAM" --check
python3 scripts/import_upstream_testcard.py "$ZSTD_UPSTREAM" --check
python3 scripts/generate_upstream_decodecorpus.py "$ZSTD_UPSTREAM" --check
python3 scripts/generate_upstream_entropy.py "$ZSTD_UPSTREAM" --check
python3 scripts/generate_upstream_windows.py "$ZSTD_UPSTREAM" --check
python3 scripts/check_upstream_fixtures.py

# Select oracle tests explicitly: legacy success assertions remain blocked.
cargo test --release --all-features --test interop -- --ignored
cargo test --release --all-features --test upstream_golden -- --ignored
cargo test --release --all-features --test upstream_decodecorpus -- --ignored
cargo test --release --all-features --test upstream_fuzzer intensive_fuzzer_one_shot_cycles -- --ignored
cargo test --release --all-features --test upstream_streaming official_codec_accepts_streamed_frames_and_stream_decoder_accepts_official_frames -- --ignored
cargo test --release --all-features --test upstream_window -- --ignored
