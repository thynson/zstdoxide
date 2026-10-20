//! A safe-first, independent implementation of the Zstandard format.
//!
//! The core supports `no_std` with `alloc`. Unsafe optimization features are
//! independent opt-ins and do not change the format or error contracts.
//!
//! ```
//! use zstdoxide::{compress, decompress};
//!
//! let input = b"safe Rust, safe Rust, safe Rust";
//! let encoded = compress(input)?;
//! assert_eq!(decompress(&encoded, 1024)?, input);
//! # Ok::<(), zstdoxide::Error>(())
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(
    not(any(
        feature = "unsafe-byte-cursor",
        feature = "unsafe-bitstream",
        feature = "unsafe-history",
        feature = "unsafe-ring-history",
        feature = "unsafe-fse",
        feature = "unsafe-fse-encoder",
        feature = "unsafe-huffman",
        feature = "unsafe-huffman-codebook",
        feature = "unsafe-match-table",
        feature = "unsafe-match-chain",
        feature = "unsafe-reference-chain",
        feature = "unsafe-reference-tree",
        feature = "unsafe-row-table",
        feature = "unsafe-dictionary",
        feature = "unsafe-training",
        feature = "unsafe-optimal-frequencies",
        feature = "unsafe-optimal-short-key",
        feature = "unsafe-optimal-matches",
        feature = "unsafe-optimal-paths"
    )),
    forbid(unsafe_code)
)]

extern crate alloc;

mod bitstream;
mod bitwriter;
mod block_entropy;
mod constants;
mod cursor;
mod decode;
mod dictionary;
mod dictionary_train;
mod double_fast_block;
#[cfg(test)]
#[path = "../tests/support/double_fast_frame_oracle.rs"]
mod double_fast_frame_oracle_fixture;
#[cfg(test)]
#[path = "../tests/support/double_fast_oracle.rs"]
mod double_fast_oracle_fixture;
mod double_fast_parse;
mod double_fast_plan;
mod double_fast_state;
mod double_fast_tables;
mod encode;
mod entropy_encode;
#[cfg(test)]
mod entropy_output;
mod error;
#[cfg(test)]
#[path = "../tests/support/fast_frame_oracle.rs"]
mod fast_frame_oracle_fixture;
#[cfg(test)]
#[path = "../tests/support/fast_oracle.rs"]
mod fast_oracle_fixture;
mod fast_parse;
mod fast_plan;
mod fast_state;
mod frame;
mod fse;
mod fse_encode;
mod fse_select;
mod greedy_block;
#[cfg(test)]
#[path = "../tests/support/greedy_frame_oracle.rs"]
mod greedy_frame_oracle_fixture;
mod greedy_key;
#[cfg(test)]
#[path = "../tests/support/greedy_oracle.rs"]
mod greedy_oracle_fixture;
mod greedy_parse;
mod greedy_plan;
mod greedy_search;
mod greedy_state;
mod heap;
mod history;
mod huffman;
mod huffman_codebook;
mod huffman_encode;
#[cfg(test)]
#[path = "../tests/support/lazy_frame_oracle.rs"]
mod lazy_frame_oracle_fixture;
#[cfg(test)]
#[path = "../tests/support/lazy_oracle.rs"]
mod lazy_oracle_fixture;
mod lazy_select;
mod literal_encode;
mod literal_header;
mod match_key;
mod match_parse;
mod match_search;
mod matchchain;
mod matchfinder;
mod optimal_frequencies;
mod optimal_matches;
mod optimal_parse;
#[cfg(test)]
#[path = "../tests/support/optimal_path_fixture.rs"]
mod optimal_path_fixture;
mod optimal_path_layout;
mod optimal_paths;
#[allow(
    dead_code,
    reason = "Private optimal scalar pricing precedes parser integration"
)]
mod optimal_price;
mod optimal_retained;
#[cfg(test)]
#[path = "../tests/support/optimal_retained_fixture.rs"]
mod optimal_retained_fixture;
mod optimal_search;
mod optimal_search_layout;
mod optimal_short_key;
#[cfg(test)]
#[path = "../tests/support/optimal_startup_fixture.rs"]
mod optimal_startup_fixture;
mod parameters;
mod reference_chain;
mod reference_encode;
mod reference_frame;
mod reference_frame_encode;
mod reference_policy;
mod reference_search;
mod reference_tree;
mod ring_history;
mod row_table;
mod sequence_context;
mod sequence_encode;
mod sequence_plan;
mod sequences;
mod strategy;
mod stream;
mod stream_decode;
mod stream_encode;
mod training;
#[cfg(test)]
#[path = "../tests/support/tree_frame_oracle.rs"]
mod tree_frame_oracle_fixture;
mod tree_layout;
#[cfg(test)]
#[path = "../tests/support/tree_oracle.rs"]
mod tree_oracle_fixture;
mod tree_search;
mod tree_sort;
mod tree_source;
#[cfg(test)]
#[path = "../tests/support/ultra_path_fixture.rs"]
mod ultra_path_fixture;
mod xxhash;

#[cfg(test)]
#[path = "../tests/upstream/entropy.rs"]
mod upstream_entropy_tests;

#[cfg(test)]
#[path = "../tests/upstream/huffman_encode.rs"]
mod upstream_huffman_encode_tests;

#[cfg(test)]
#[path = "../tests/upstream/huffman_round_trip.rs"]
mod upstream_huffman_round_trip_tests;

#[cfg(test)]
#[path = "../tests/upstream/sequence_generator.rs"]
mod upstream_sequence_generator_tests;

#[cfg(test)]
#[path = "../tests/upstream/sequence_sweep.rs"]
mod upstream_sequence_sweep_tests;

pub use decode::{
    DecodeOptions, decompress, decompress_in, decompress_with_dictionary,
    decompress_with_dictionary_in, decompress_with_options, decompress_with_options_in,
};
pub use dictionary::{
    Dictionary, DictionaryHandle, DictionaryMode, DictionaryView, SharedDictionary,
};
pub use dictionary_train::{
    DictionaryTrainingOptions, finalize_dictionary, finalize_dictionary_in, train_dictionary,
    train_dictionary_in,
};
pub use encode::{compress, compress_in, compress_with_dictionary, compress_with_dictionary_in};
pub use error::{Error, Result};
pub use frame::FrameHeader;
pub use match_search::MatchSearchOptions;
pub use parameters::{
    CompressionParameterRequest, RawCompressionParameters, ReferenceFrameParameters,
    ReferenceStrategy, ResolvedCompressionParameters, SourceSize, inspect_compression_level,
};
pub use reference_encode::{
    compress_at_level, compress_at_level_in, compress_with_parameters, compress_with_parameters_in,
};
pub use strategy::CompressionStrategy;
pub use stream::{EndDirective, Progress, StreamStatus};
pub use stream_decode::Decoder;
pub use stream_encode::{Encoder, EncoderOptions};
pub use training::{RawTrainingOptions, train_raw_dictionary, train_raw_dictionary_in};

#[cfg(test)]
#[path = "../tests/support/optimal_retained_wide_fixture.rs"]
mod optimal_retained_wide_fixture;

#[cfg(test)]
#[path = "../tests/support/optimal_retained_frames_fixture.rs"]
mod optimal_retained_frames_fixture;

#[cfg(test)]
#[path = "../tests/support/optimal_fresh_wide_fixture.rs"]
mod optimal_fresh_wide_fixture;

mod optimal_plan;

mod optimal_state;

#[cfg(test)]
#[path = "../tests/support/ultra_fresh_fixture.rs"]
mod ultra_fresh_fixture;

mod ultra2_plan;

#[cfg(test)]
#[path = "../tests/support/ultra2_fixture.rs"]
mod ultra2_fixture;

mod ultra2_state;

mod ultra2_frame_plan;

mod optimal_retained_plan;
