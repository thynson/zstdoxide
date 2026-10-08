//! Slice-backed input access; all positions are indices.

use crate::{Error, Result};

pub(crate) struct ByteCursor<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> ByteCursor<'a> {
    pub(crate) fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0 }
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }

    pub(crate) fn remaining(&self) -> &'a [u8] {
        &self.input[self.position..]
    }

    pub(crate) fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        if count > self.input.len() - self.position {
            return Err(Error::UnexpectedEof);
        }
        let end = self.position + count;
        #[cfg(not(feature = "unsafe-byte-cursor"))]
        let bytes = &self.input[self.position..end];
        #[cfg(feature = "unsafe-byte-cursor")]
        // SAFETY: the private position is initially zero and only updated to a
        // checked end <= input.len(). The subtraction check above proves the
        // ordered range position..end lies in the initialized, live shared
        // slice without overflow. get_unchecked preserves that slice's lifetime
        // and shared aliasing; no mutation or alignment-sensitive cast occurs.
        let bytes = unsafe { self.input.get_unchecked(self.position..end) };
        self.position = end;
        Ok(bytes)
    }

    pub(crate) fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    /// The checked equivalent of upstream `MEM_readLE16/24/32/64`.
    pub(crate) fn little_endian(&mut self, count: usize) -> Result<u64> {
        if count > 8 {
            return Err(Error::CorruptData);
        }
        let mut word = [0; 8];
        word[..count].copy_from_slice(self.take(count)?);
        Ok(u64::from_le_bytes(word))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_read_preserves_position_and_handles_overflow() {
        let mut cursor = ByteCursor::new(&[1, 2, 3]);
        assert_eq!(cursor.little_endian(2), Ok(513));
        assert_eq!(cursor.take(usize::MAX), Err(Error::UnexpectedEof));
        assert_eq!(cursor.little_endian(9), Err(Error::CorruptData));
        assert_eq!(cursor.byte(), Ok(3));
        assert_eq!(cursor.take(0), Ok(&[][..]));
        assert_eq!(cursor.byte(), Err(Error::UnexpectedEof));
        assert_eq!(cursor.position(), 3);
        assert!(cursor.remaining().is_empty());
    }
}
