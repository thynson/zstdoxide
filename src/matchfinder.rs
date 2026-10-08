//! Position-indexed storage for a simple greedy match finder.

use alloc::vec::Vec;

use crate::{Error, Result};

const HASH_BITS: u32 = 16;
const TABLE_SIZE: usize = 1 << HASH_BITS;
const EMPTY: usize = usize::MAX;

/// An owned table of candidate positions; input bytes are never owned or pointed
/// to by this table. Callers verify candidate bytes to resolve hash collisions.
///
/// Corresponds to the indexed hash-table role of `ZSTD_MatchState_t`.
pub(crate) struct MatchTable {
    positions: Vec<usize>,
}

impl MatchTable {
    pub(crate) fn new() -> Result<Self> {
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(TABLE_SIZE)
            .map_err(|_| Error::AllocationFailed)?;
        positions.resize(TABLE_SIZE, EMPTY);
        Ok(Self { positions })
    }

    /// Starts an independent block without retaining candidate positions.
    pub(crate) fn reset(&mut self) {
        self.positions.fill(EMPTY);
    }

    /// Owned allocation size in bytes, excluding the inline vector handle.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.positions.capacity() * core::mem::size_of::<usize>()
    }

    /// Records the current position and returns an earlier candidate, if any.
    ///
    /// A returned candidate only denotes a hash collision; the caller must
    /// verify source bytes and its own history/window bounds before matching.
    /// Corresponds to table lookup/update in `ZSTD_compressBlock_fast`.
    pub(crate) fn replace(&mut self, word: [u8; 4], position: usize) -> Option<usize> {
        let hash = u32::from_le_bytes(word).wrapping_mul(0x9e37_79b1);
        let index = (hash >> (32 - HASH_BITS)) as usize;
        #[cfg(not(feature = "unsafe-match-table"))]
        let previous = core::mem::replace(&mut self.positions[index], position);
        #[cfg(feature = "unsafe-match-table")]
        let previous = {
            // SAFETY: HASH_BITS is 16, so shifting the u32 hash right by 16 proves
            // index < 2^16 = TABLE_SIZE. Construction initializes TABLE_SIZE
            // properly aligned usize entries; no method changes the vector
            // length or allocation. The exclusive self borrow grants unique
            // access to the element for this call. The temporary reference
            // cannot escape; neither operation reallocates or reads
            // uninitialized storage.
            unsafe { core::mem::replace(self.positions.get_unchecked_mut(index), position) }
        };
        (previous != EMPTY && previous < position).then_some(previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_only_prior_candidates_and_reset_discards_them() {
        let mut table = MatchTable::new().unwrap();
        assert_eq!(table.replace(*b"abcd", 0), None);
        assert_eq!(table.replace(*b"abcd", 9), Some(0));
        assert_eq!(table.replace(*b"abcd", 9), None);
        assert_eq!(table.replace(*b"abcd", 8), None);
        assert_eq!(table.replace(*b"abcd", 10), Some(8));
        table.reset();
        assert_eq!(table.replace(*b"abcd", 11), None);
    }

    #[test]
    fn collisions_remain_candidates_that_need_byte_verification() {
        let mut table = MatchTable::new().unwrap();
        // Multiplication maps these two distinct words to hash values 0 and 1,
        // whose high sixteen bits are equal.
        assert_eq!(table.replace(0_u32.to_le_bytes(), 1), None);
        assert_eq!(table.replace(0x0e8b_2f51_u32.to_le_bytes(), 5), Some(1));
    }

    #[test]
    fn empty_sentinel_cannot_escape_as_a_candidate() {
        let mut table = MatchTable::new().unwrap();
        assert_eq!(table.replace([255; 4], usize::MAX), None);
        assert_eq!(table.replace([255; 4], usize::MAX), None);
    }
}
