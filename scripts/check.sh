#!/bin/sh
set -eu

# Each backend alone must compile without std and pass the same test suite.
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo check --no-default-features
for feature in unsafe-byte-cursor unsafe-bitstream unsafe-history unsafe-fse unsafe-huffman unsafe-match-table; do
    cargo test --features "$feature"
    cargo check --no-default-features --features "$feature"
done
cargo test --all-features
cargo test --release --all-features
cargo check --no-default-features --features unsafe-byte-cursor,unsafe-bitstream,unsafe-history,unsafe-fse,unsafe-huffman,unsafe-match-table
cargo clippy --all-targets --all-features -- -D warnings
