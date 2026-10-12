#!/bin/sh
set -eu

python3 scripts/audit_upstream_tests.py
python3 scripts/check_upstream_fixtures.py
python3 scripts/import_fast_parser_smoke.py --check
python3 scripts/import_fast_collector.py --check
python3 scripts/import_negative_level_fixture.py --check
python3 scripts/import_reference_fast_fuzz.py --check
python3 scripts/import_fast_frame_publication.py --check
python3 scripts/import_fast_parser_families.py --check
python3 scripts/check_double_fast_tiny.py --check
python3 scripts/pack_fast_parser_fixtures.py --check
python3 scripts/pack_fast_frame_trajectories.py --check
python3 scripts/check_reference_fast_interop.py --check-plan
python3 scripts/check_match_search_baseline.py
python3 scripts/check_reference_fast_baseline.py

# Each backend alone must compile without std and pass the same test suite.
cargo fmt --all -- --check
cargo fmt --manifest-path test-support/allocator/Cargo.toml -- --check
cargo test --manifest-path test-support/allocator/Cargo.toml
cargo clippy --manifest-path test-support/allocator/Cargo.toml --all-targets -- -D warnings
cargo test
python3 scripts/check_doc_examples.py
cargo clippy --all-targets -- -D warnings
cargo check --no-default-features
for feature in unsafe-byte-cursor unsafe-bitstream unsafe-history unsafe-ring-history unsafe-fse unsafe-fse-encoder unsafe-huffman unsafe-huffman-codebook unsafe-match-table unsafe-match-chain unsafe-dictionary unsafe-training; do
    cargo test --features "$feature"
    cargo check --no-default-features --features "$feature"
done
cargo test --all-features
cargo test --release --all-features
cargo check --no-default-features --features unsafe-byte-cursor,unsafe-bitstream,unsafe-history,unsafe-ring-history,unsafe-fse,unsafe-fse-encoder,unsafe-huffman,unsafe-huffman-codebook,unsafe-match-table,unsafe-match-chain,unsafe-dictionary,unsafe-training
cargo clippy --all-targets --all-features -- -D warnings
