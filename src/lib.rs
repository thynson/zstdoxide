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
        feature = "unsafe-dictionary",
        feature = "unsafe-training"
    )),
    forbid(unsafe_code)
)]

extern crate alloc;

mod bitstream;
mod bitwriter;
mod constants;
mod cursor;
mod decode;
mod dictionary;
mod dictionary_train;
mod encode;
mod entropy_encode;
mod error;
mod frame;
mod fse;
mod fse_encode;
mod history;
mod huffman;
#[cfg(test)]
mod huffman_codebook;
mod huffman_encode;
#[cfg(test)]
mod literal_header;
mod matchfinder;
mod ring_history;
#[cfg(test)]
mod sequence_plan;
mod sequences;
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

pub use decode::{DecodeOptions, decompress, decompress_with_dictionary, decompress_with_options};
pub use dictionary::{Dictionary, DictionaryMode, SharedDictionary};
pub use dictionary_train::{DictionaryTrainingOptions, finalize_dictionary, train_dictionary};
pub use encode::{compress, compress_with_dictionary};
pub use error::{Error, Result};
pub use frame::FrameHeader;
pub use stream::{EndDirective, Progress, StreamStatus};
pub use stream_decode::Decoder;
pub use stream_encode::{Encoder, EncoderOptions};
pub use training::{RawTrainingOptions, train_raw_dictionary};
