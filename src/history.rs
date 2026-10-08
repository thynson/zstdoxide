//! Bounded output storage with checked, overlap-aware LZ match expansion.

use crate::{Error, Result};
use alloc::vec::Vec;

/// The checked operations needed by sequence execution, independent of storage.
///
/// A sink reports its current block/output position and validates capacity and
/// backward distances before writes. Streaming sinks retain bounded ring
/// history; one-shot history owns the complete result.
pub(crate) trait SequenceOutput {
    fn len(&self) -> usize;
    fn append(&mut self, bytes: &[u8]) -> Result<()>;
    fn copy_match(&mut self, offset: usize, length: usize) -> Result<()>;
}

pub(crate) struct History {
    output: Vec<u8>,
    limit: usize,
    frame_start: usize,
    window: usize,
}

impl History {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            output: Vec::new(),
            limit,
            frame_start: 0,
            window: 0,
        }
    }

    pub(crate) fn start_frame(&mut self, window: usize) {
        self.frame_start = self.output.len();
        self.window = window;
    }

    pub(crate) fn len(&self) -> usize {
        self.output.len()
    }

    pub(crate) fn frame_bytes(&self) -> &[u8] {
        &self.output[self.frame_start..]
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.output
    }

    fn reserve(&mut self, count: usize) -> Result<()> {
        if count > self.limit - self.output.len() {
            return Err(Error::ResourceLimit);
        }
        self.output
            .try_reserve(count)
            .map_err(|_| Error::AllocationFailed)
    }

    pub(crate) fn append(&mut self, bytes: &[u8]) -> Result<()> {
        self.reserve(bytes.len())?;
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    pub(crate) fn repeat(&mut self, byte: u8, count: usize) -> Result<()> {
        self.reserve(count)?;
        self.output.resize(self.output.len() + count, byte);
        Ok(())
    }

    /// Execute the match-copy part of upstream `ZSTD_execSequence`.
    pub(crate) fn copy_match(&mut self, offset: usize, length: usize) -> Result<()> {
        if offset == 0 || offset > self.frame_bytes().len() || offset > self.window {
            return Err(Error::CorruptData);
        }
        self.reserve(length)?;
        let start = self.output.len();
        #[cfg(not(feature = "unsafe-history"))]
        for index in 0..length {
            let byte = self.output[start + index - offset];
            self.output.push(byte);
        }
        #[cfg(feature = "unsafe-history")]
        {
            let pointer = self.output.as_mut_ptr();
            // SAFETY: offset is nonzero and <= the initialized frame prefix.
            // reserve proved start + length <= limit without overflow and
            // allocated capacity for that end. No reallocation occurs below.
            // On iteration i, start+i-offset is nonnegative and < start+i:
            // it is in the original initialized prefix or was written by an
            // earlier iteration. Source and destination are within this same
            // live u8 allocation (alignment one). read copies a u8, write fills
            // one slot; their temporary pointers create no aliased references.
            // The exclusive self borrow prevents other access. Sequential reads
            // preserve LZ overlap semantics. After the loop every byte up to the
            // new length is initialized, proving set_len's requirements.
            unsafe {
                for index in 0..length {
                    let byte = pointer.add(start + index - offset).read();
                    pointer.add(start + index).write(byte);
                }
                self.output.set_len(start + length);
            }
        }
        Ok(())
    }
}

impl SequenceOutput for History {
    fn len(&self) -> usize {
        History::len(self)
    }

    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        History::append(self, bytes)
    }

    fn copy_match(&mut self, offset: usize, length: usize) -> Result<()> {
        History::copy_match(self, offset, length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_overlapping_patterns_at_every_small_offset() {
        for offset in 1..=16 {
            let mut history = History::new(1000);
            history.start_frame(128);
            let prefix: Vec<_> = (0..offset as u8).collect();
            history.append(&prefix).unwrap();
            history.copy_match(offset, 301).unwrap();
            let expected: Vec<_> = prefix.iter().copied().cycle().take(offset + 301).collect();
            assert_eq!(history.into_bytes(), expected);
        }
    }

    #[test]
    fn rejects_out_of_window_cross_frame_and_oversized_matches() {
        let mut history = History::new(10);
        history.start_frame(2);
        history.append(b"abc").unwrap();
        assert_eq!(history.copy_match(0, 1), Err(Error::CorruptData));
        assert_eq!(history.copy_match(3, 1), Err(Error::CorruptData));
        assert_eq!(history.copy_match(1, usize::MAX), Err(Error::ResourceLimit));
        assert_eq!(history.frame_bytes(), b"abc");
        history.start_frame(10);
        assert_eq!(history.copy_match(1, 1), Err(Error::CorruptData));
        history.repeat(b'd', 2).unwrap();
        assert_eq!(history.len(), 5);
        assert_eq!(history.frame_bytes(), b"dd");
    }
}
