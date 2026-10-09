//! Compile-only standard allocator API probe, independent of the codec.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::{alloc::Global, collections::TryReserveError, vec::Vec};
use core::alloc::Allocator;

pub fn owned<A: Allocator>(allocator: A) -> Result<Vec<u8, A>, TryReserveError> {
    let mut bytes = Vec::new_in(allocator);
    bytes.try_reserve_exact(9)?;
    for &byte in b"allocator" {
        bytes.push(byte);
    }
    Ok(bytes)
}

pub fn borrowed<A: Allocator>(allocator: &A) -> Result<Vec<u8, &A>, TryReserveError> {
    owned(allocator)
}

pub fn global() -> Result<Vec<u8, Global>, TryReserveError> {
    owned(Global)
}
