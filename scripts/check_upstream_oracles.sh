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
python3 scripts/generate_upstream_dictionaries.py "$ZSTD_UPSTREAM" --check
python3 scripts/check_upstream_fixtures.py

# Select oracle tests explicitly: legacy success assertions remain blocked.
cargo test --release --all-features --test interop -- --ignored
cargo test --release --all-features --test upstream_golden -- --ignored
cargo test --release --all-features --test upstream_decodecorpus -- --ignored
cargo test --release --all-features --test upstream_fuzzer intensive_fuzzer_one_shot_cycles -- --ignored
cargo test --release --all-features --test upstream_streaming official_codec_accepts_streamed_frames_and_stream_decoder_accepts_official_frames -- --ignored
cargo test --release --all-features --test upstream_window -- --ignored
cargo test --release --all-features --test upstream_dictionary_encode -- --ignored
cargo test --release --all-features --test upstream_training -- --ignored
cargo test --release --all-features --test upstream_formatted_training -- --ignored
cargo test --release --all-features --lib official_fse_and_huffman_readers_accept_generated_headers -- --ignored
cargo test --release --all-features --test upstream_trained_dictionary_fuzz official_training_sample_loop_matches_every_selected_range -- --ignored

# Module 8 entropy streams, adaptive sequence state and independent strategies.
# These targets contain explicit C checks only; external corpus entry points
# remain separately parameterized by their pinned cache paths.
cargo test --release --all-features --lib upstream_huffman_encode_tests -- --ignored
cargo test --release --all-features --lib entropy_output::tests::oracle -- --ignored
cargo test --release --all-features --lib upstream_huffman_round_trip_tests::oracle::official_c_producer_matches_huffman_parameters_for_both_bmi_layouts -- --ignored --exact
cargo test --release --all-features --lib upstream_huffman_round_trip_tests::oracle::official_c_depth_selection_matches_adjustment_and_cheap_policy -- --ignored --exact
cargo test --release --all-features --lib upstream_sequence_generator_tests::oracle -- --ignored
cargo test --release --all-features --lib sequence_encode::tests::official_codec_decodes_every_supplied_table_mode_and_repeat_combination -- --ignored --exact
cargo test --release --all-features --lib fse_select::tests::official_codec_decodes_automatically_selected_channel_tables -- --ignored --exact
cargo test --release --all-features --lib sequence_context::tests::official_codec_decodes_transactional_and_dictionary_frames -- --ignored --exact
cargo test --release --all-features --test entropy_encoding -- --ignored
cargo test --release --all-features --test sequence_encoding -- --ignored
cargo test --release --all-features --test compression_strategies -- --ignored
cargo test --release --all-features --test upstream_entropy_lifetimes -- --ignored
