//! A safe-first, independent implementation of the Zstandard format.
//!
//! The core supports `no_std` with `alloc`. Unsafe optimization features are
//! independent opt-ins and do not change the format or error contracts.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(
    not(any(
        feature = "unsafe-byte-cursor",
        feature = "unsafe-bitstream",
        feature = "unsafe-history",
        feature = "unsafe-fse",
        feature = "unsafe-huffman",
        feature = "unsafe-match-table"
    )),
    forbid(unsafe_code)
)]

extern crate alloc;

mod error;
mod bitstream;
mod cursor;
mod frame;
mod history;
mod fse;
mod huffman;
mod xxhash;

pub use error::{Error, Result};
pub use frame::FrameHeader;
