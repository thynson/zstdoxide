//! Direct caller-owned output with checked, frame-local LZ history.

use crate::history::SequenceOutput;
use crate::{Error, Result};

pub(crate) struct SliceHistory<'a> {
    output: &'a mut [u8],
    position: usize,
    frame_start: usize,
    window: usize,
    limit: usize,
}

impl<'a> SliceHistory<'a> {
    pub(crate) fn new(output: &'a mut [u8], limit: usize) -> Self {
        let limit = limit.min(output.len());
        Self {
            output,
            position: 0,
            frame_start: 0,
            window: 0,
            limit,
        }
    }

    pub(crate) fn start_frame(&mut self, window: usize) {
        self.frame_start = self.position;
        self.window = window;
    }

    pub(crate) fn frame_bytes(&self) -> &[u8] {
        &self.output[self.frame_start..self.position]
    }

    fn checked_end(&self, count: usize) -> Result<usize> {
        if count > self.limit - self.position {
            return Err(Error::ResourceLimit);
        }
        Ok(self.position + count)
    }

    pub(crate) fn repeat(&mut self, byte: u8, count: usize) -> Result<()> {
        let end = self.checked_end(count)?;
        self.output[self.position..end].fill(byte);
        self.position = end;
        Ok(())
    }
}

impl SequenceOutput for SliceHistory<'_> {
    fn len(&self) -> usize {
        self.position
    }

    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        let end = self.checked_end(bytes.len())?;
        self.output[self.position..end].copy_from_slice(bytes);
        self.position = end;
        Ok(())
    }

    /// Match expansion corresponding to upstream `ZSTD_execSequence`.
    fn copy_match(&mut self, offset: usize, length: usize) -> Result<()> {
        if offset == 0 || offset > self.position - self.frame_start || offset > self.window {
            return Err(Error::CorruptData);
        }
        let end = self.checked_end(length)?;
        #[cfg(not(feature = "unsafe-slice-history"))]
        for destination in self.position..end {
            self.output[destination] = self.output[destination - offset];
        }
        #[cfg(feature = "unsafe-slice-history")]
        {
            let pointer = self.output.as_mut_ptr();
            // SAFETY: the exclusive slice borrow keeps its initialized u8
            // allocation alive and prevents competing access for this loop.
            // checked_end proves position <= end <= limit <= output.len(),
            // without overflow. The validated nonzero offset is at most the
            // current frame prefix, so each destination-offset is in bounds,
            // at least frame_start, and strictly before its destination. It
            // reads the original prefix or an earlier write in this loop.
            // Both addresses have u8 alignment (one); read and write copy
            // initialized bytes without creating overlapping references.
            // No allocation, ownership or slice-length change occurs. Forward
            // ordering preserves overlapping LZ matches. An empty loop never
            // dereferences the pointer, including at the slice end.
            unsafe {
                for destination in self.position..end {
                    let byte = pointer.add(destination - offset).read();
                    pointer.add(destination).write(byte);
                }
            }
        }
        self.position = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_small_offset_and_length_expands_forward_and_preserves_guards() {
        for prefix in 1..=32 {
            for offset in 1..=prefix {
                for length in [0, 1, offset - 1, offset, offset + 1, 137] {
                    let mut output = [0xa5; 192];
                    let mut history = SliceHistory::new(&mut output[1..191], usize::MAX);
                    history.start_frame(prefix);
                    for byte in 0..prefix {
                        history.append(&[byte as u8]).unwrap();
                    }
                    history.copy_match(offset, length).unwrap();
                    assert_eq!(history.len(), prefix + length);
                    for index in 0..prefix + length {
                        let expected = if index < prefix {
                            index
                        } else {
                            prefix - offset + (index - prefix) % offset
                        };
                        assert_eq!(history.frame_bytes()[index], expected as u8);
                    }
                    assert!(output[1 + prefix + length..].iter().all(|&b| b == 0xa5));
                    assert_eq!(output[0], 0xa5);
                }
            }
        }
    }

    #[test]
    fn rejected_operations_preserve_bytes_position_and_frame_state() {
        for operation in 0..7 {
            let mut output = [0xa5; 20];
            let mut history = SliceHistory::new(&mut output, 8);
            history.append(b"old").unwrap();
            history.start_frame(2);
            history.append(b"abc").unwrap();
            let before = history.output.to_vec();
            let result = match operation {
                0 => history.append(b"xyz"),
                1 => history.repeat(0, usize::MAX),
                2 => history.copy_match(1, usize::MAX),
                3 => history.copy_match(0, 0),
                4 => history.copy_match(3, 1),
                5 => history.copy_match(4, 0),
                _ => history.copy_match(usize::MAX, usize::MAX),
            };
            assert_eq!(
                result,
                Err(if operation < 3 {
                    Error::ResourceLimit
                } else {
                    Error::CorruptData
                })
            );
            assert_eq!(history.output, before.as_slice());
            assert_eq!(
                (
                    history.position,
                    history.frame_start,
                    history.window,
                    history.limit
                ),
                (6, 3, 2, 8)
            );
        }
    }

    #[test]
    fn empty_and_exact_capacity_operations_respect_the_effective_limit() {
        for capacity in 0..=12 {
            for requested in [0, 3, 12, usize::MAX] {
                let mut output = [0xa5; 14];
                let mut history = SliceHistory::new(&mut output[1..1 + capacity], requested);
                let limit = capacity.min(requested);
                history.start_frame(usize::MAX);
                history.append(&[]).unwrap();
                history.repeat(0, 0).unwrap();
                assert_eq!(history.copy_match(1, 0), Err(Error::CorruptData));
                history.repeat(7, limit).unwrap();
                assert_eq!(history.append(&[0]), Err(Error::ResourceLimit));
                assert_eq!(history.repeat(0, 1), Err(Error::ResourceLimit));
                if limit > 0 {
                    history.copy_match(1, 0).unwrap();
                    assert_eq!(history.copy_match(1, 1), Err(Error::ResourceLimit));
                }
                assert_eq!(history.frame_bytes(), &[7; 12][..limit]);
                assert_eq!(output[0], 0xa5);
                assert!(output[1 + limit..].iter().all(|&b| b == 0xa5));
            }
        }
    }

    #[test]
    fn concatenated_frames_exclude_previous_bytes_and_apply_each_window() {
        let mut output = [0xa5; 32];
        let mut history = SliceHistory::new(&mut output, 32);
        history.start_frame(8);
        history.append(b"previous").unwrap();
        history.start_frame(1);
        assert!(history.frame_bytes().is_empty());
        assert_eq!(history.copy_match(1, 1), Err(Error::CorruptData));
        history.append(b"ab").unwrap();
        assert_eq!(history.copy_match(2, 1), Err(Error::CorruptData));
        history.copy_match(1, 8).unwrap();
        assert_eq!(history.frame_bytes(), b"abbbbbbbbb");
        history.start_frame(0);
        history.append(b"x").unwrap();
        assert_eq!(history.copy_match(1, 0), Err(Error::CorruptData));
        assert_eq!(history.frame_bytes(), b"x");
        assert_eq!(&output[..8], b"previous");
        assert!(output[19..].iter().all(|&b| b == 0xa5));
    }
}
