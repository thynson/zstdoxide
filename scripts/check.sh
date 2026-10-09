#!/bin/sh
set -eu

python3 scripts/audit_upstream_tests.py
python3 scripts/check_upstream_fixtures.py

# Each backend alone must compile without std and pass the same test suite.
cargo fmt --all -- --check
cargo test
cargo build
rustdoc --edition 2024 --test docs/dictionary-usage.md --extern zstdoxide=target/debug/libzstdoxide.rlib -L dependency=target/debug/deps
cargo clippy --all-targets -- -D warnings
cargo check --no-default-features
for feature in unsafe-byte-cursor unsafe-bitstream unsafe-history unsafe-ring-history unsafe-fse unsafe-huffman unsafe-match-table unsafe-dictionary unsafe-training; do
    cargo test --features "$feature"
    cargo check --no-default-features --features "$feature"
done
cargo test --all-features
cargo test --release --all-features
cargo check --no-default-features --features unsafe-byte-cursor,unsafe-bitstream,unsafe-history,unsafe-ring-history,unsafe-fse,unsafe-huffman,unsafe-match-table,unsafe-dictionary,unsafe-training
cargo clippy --all-targets --all-features -- -D warnings
