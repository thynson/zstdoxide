//! Checked append-only access to initialized caller-owned encoded output.
use crate::{Error, Result};

/// Owns exclusive access, not allocation: position is always <= output.len().
/// Each append is atomic; earlier appends remain visible after a later error.
pub(crate) struct FrameSink<'a> {
    output: &'a mut [u8],
    position: usize,
}

/// Validate the full destination extent before indexing or pointer arithmetic.
fn checked_end(position: usize, count: usize, capacity: usize) -> Result<usize> {
    if position > capacity {
        return Err(Error::InvalidState);
    }
    let end = position.checked_add(count).ok_or(Error::ResourceLimit)?;
    if end > capacity {
        return Err(Error::ResourceLimit);
    }
    Ok(end)
}

impl<'a> FrameSink<'a> {
    pub(crate) fn new(output: &'a mut [u8]) -> Self {
        Self {
            output,
            position: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.position
    }

    /// Unwritten suffix length. `position <= output.len()` is invariant.
    pub(crate) fn remaining(&self) -> usize {
        self.output.len() - self.position
    }

    pub(crate) fn written(&self) -> &[u8] {
        &self.output[..self.position]
    }

    /// Append the complete independent initialized source or change nothing.
    pub(crate) fn append(&mut self, source: &[u8]) -> Result<()> {
        let end = checked_end(self.position, source.len(), self.output.len())?;
        #[cfg(not(feature = "unsafe-frame-sink"))]
        self.output[self.position..end].copy_from_slice(source);
        #[cfg(feature = "unsafe-frame-sink")]
        if !source.is_empty() {
            // SAFETY: checked_end validated non-overflowing position+len within
            // this initialized output slice before pointer construction. A
            // nonempty source therefore gives position < output.len(), with the
            // complete destination range valid for writes. The safe immutable
            // source slice is initialized and valid for reads of source.len().
            // Both pointers have u8 alignment one. The exclusive output borrow
            // and independent shared source borrow cannot overlap for nonzero
            // ranges in a valid safe call; no shared output reference survives
            // this mutable method call. Both allocations remain alive and fixed
            // throughout the copy; no pointer/reference escapes or ownership
            // changes. Empty input performs no raw-pointer operation at all.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    source.as_ptr(),
                    self.output.as_mut_ptr().add(self.position),
                    source.len(),
                );
            }
        }
        self.position = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn all_small_append_pairs_preserve_exact_prefix_suffix_and_guards() {
        let maximum = if cfg!(miri) { 8 } else { 32 };
        let source: Vec<_> = (0..maximum + 2).map(|n| (n * 7 + 13) as u8).collect();
        for capacity in 0..=maximum {
            for first in 0..=maximum + 1 {
                for second in 0..=maximum + 1 {
                    let mut guarded = alloc::vec![0xa5; capacity + 2];
                    let mut sink = FrameSink::new(&mut guarded[1..1 + capacity]);
                    let a = sink.append(&source[..first]);
                    let position = if first <= capacity { first } else { 0 };
                    assert_eq!(
                        a,
                        if first <= capacity {
                            Ok(())
                        } else {
                            Err(Error::ResourceLimit)
                        }
                    );
                    assert_eq!(sink.len(), position);
                    let b = sink.append(&source[..second]);
                    let fits = second <= capacity - position;
                    assert_eq!(
                        b,
                        if fits {
                            Ok(())
                        } else {
                            Err(Error::ResourceLimit)
                        }
                    );
                    let written = position + if fits { second } else { 0 };
                    assert_eq!(sink.len(), written);
                    assert_eq!(&sink.written()[..position], &source[..position]);
                    assert_eq!(
                        &sink.written()[position..],
                        &source[..if fits { second } else { 0 }]
                    );
                    assert!(
                        guarded[..1]
                            .iter()
                            .chain(&guarded[1 + written..])
                            .all(|&b| b == 0xa5)
                    );
                }
            }
        }
    }

    #[test]
    fn rejected_append_preserves_position_and_can_be_followed_by_a_smaller_one() {
        let mut output = [0xa5; 9];
        let mut sink = FrameSink::new(&mut output);
        sink.append(b"abc").unwrap();
        assert_eq!(sink.append(b"too-long"), Err(Error::ResourceLimit));
        assert_eq!(sink.len(), 3);
        assert_eq!(sink.written(), b"abc");
        sink.append(b"def").unwrap();
        sink.append(b"ghi").unwrap();
        assert_eq!(sink.append(b"x"), Err(Error::ResourceLimit));
        sink.append(b"").unwrap();
        assert_eq!(sink.written(), b"abcdefghi");
    }

    #[test]
    fn empty_buffers_and_appends_require_no_special_pointer_contract() {
        let mut empty = [];
        let mut sink = FrameSink::new(&mut empty);
        assert_eq!(sink.append(&[]), Ok(()));
        assert_eq!(sink.append(b"x"), Err(Error::ResourceLimit));
        assert_eq!(sink.len(), 0);
        assert!(sink.written().is_empty());
    }

    #[test]
    fn numeric_overflow_and_invalid_positions_are_rejected_before_access() {
        assert_eq!(checked_end(0, 0, 0), Ok(0));
        assert_eq!(checked_end(usize::MAX, 0, usize::MAX), Ok(usize::MAX));
        assert_eq!(checked_end(usize::MAX - 1, 1, usize::MAX), Ok(usize::MAX));
        assert_eq!(
            checked_end(usize::MAX, 1, usize::MAX),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            checked_end(1, usize::MAX, usize::MAX),
            Err(Error::ResourceLimit)
        );
        assert_eq!(checked_end(9, 0, 8), Err(Error::InvalidState));
        assert_eq!(checked_end(9, usize::MAX, 8), Err(Error::InvalidState));
        assert_eq!(checked_end(3, 6, 8), Err(Error::ResourceLimit));
    }
}
