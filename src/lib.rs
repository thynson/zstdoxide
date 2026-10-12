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
        feature = "unsafe-row-table",
        feature = "unsafe-dictionary",
        feature = "unsafe-training"
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
mod greedy_key;
mod greedy_plan;
mod heap;
mod history;
mod huffman;
mod huffman_codebook;
mod huffman_encode;
mod literal_encode;
mod literal_header;
mod match_key;
mod match_parse;
mod match_search;
mod matchchain;
mod matchfinder;
mod parameters;
mod reference_chain;
mod reference_encode;
mod reference_frame;
mod reference_frame_encode;
mod reference_policy;
mod reference_search;
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
