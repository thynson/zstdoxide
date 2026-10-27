//! Checked coordinates for one exclusively borrowed suffix-input allocation.
//!
//! This independently tested component does not parse or decode Zstandard frames.
//! The frame coordinator must validate block semantics and content checksums.
use crate::{Error, Result};

/// One owner of source and output bytes; no interior pointers are retained.
///
/// Invariants: frame_start <= output_position <= output_limit <= bytes.len(),
/// output_position <= input_position <= bytes.len(). Consumed encoded bytes may
/// be overwritten; each operation protects the encoded suffix it has not consumed.
pub(crate) struct InPlaceBuffer<'a> {
    bytes: &'a mut [u8],
    input_position: usize,
    output_position: usize,
    output_limit: usize,
    frame_start: usize,
    window: usize,
}

impl<'a> InPlaceBuffer<'a> {
    pub(crate) fn new(
        bytes: &'a mut [u8],
        input_start: usize,
        output_limit: usize,
    ) -> Result<Self> {
        if input_start > bytes.len() {
            return Err(Error::ResourceLimit);
        }
        let output_limit = output_limit.min(bytes.len());
        Ok(Self {
            bytes,
            input_position: input_start,
            output_position: 0,
            output_limit,
            frame_start: 0,
            window: 0,
        })
    }

    pub(crate) fn positions(&self) -> (usize, usize, usize) {
        (self.input_position, self.output_position, self.output_limit)
    }

    pub(crate) fn remaining(&self) -> &[u8] {
        &self.bytes[self.input_position..]
    }

    pub(crate) fn written(&self) -> &[u8] {
        &self.bytes[..self.output_position]
    }

    pub(crate) fn start_frame(&mut self, window: usize) {
        self.frame_start = self.output_position;
        self.window = window;
    }

    pub(crate) fn frame_coordinates(&self) -> (usize, usize) {
        (self.frame_start, self.window)
    }

    pub(crate) fn frame_bytes(&self) -> &[u8] {
        &self.bytes[self.frame_start..self.output_position]
    }

    fn source_end(&self, count: usize) -> Result<usize> {
        let end = self
            .input_position
            .checked_add(count)
            .ok_or(Error::ResourceLimit)?;
        if end > self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        Ok(end)
    }

    fn output_end(&self, count: usize, fence: usize) -> Result<usize> {
        let end = self
            .output_position
            .checked_add(count)
            .ok_or(Error::ResourceLimit)?;
        if end > self.output_limit || end > fence {
            return Err(Error::ResourceLimit);
        }
        Ok(end)
    }

    /// Advance only after the caller has copied any needed metadata into values.
    pub(crate) fn consume(&mut self, count: usize) -> Result<()> {
        let end = self.source_end(count)?;
        self.input_position = end;
        Ok(())
    }

    /// Copy the current raw payload leftward, permitting actual byte overlap.
    /// Reject malformed ranges/capacity before any write or coordinate change.
    pub(crate) fn copy_raw(&mut self, count: usize) -> Result<()> {
        let source_end = self.source_end(count)?;
        let output_end = self.output_end(count, source_end)?;
        if count != 0 {
            #[cfg(not(feature = "unsafe-in-place-buffer"))]
            self.bytes
                .copy_within(self.input_position..source_end, self.output_position);
            #[cfg(feature = "unsafe-in-place-buffer")]
            {
                let pointer = self.bytes.as_mut_ptr();
                // SAFETY: count > 0 and source_end/output_end were checked without
                // overflow before pointer construction. Both complete ranges are
                // within this initialized u8 slice, whose exclusive borrow keeps
                // the allocation alive and excludes competing reads/writes. All
                // u8 addresses have alignment one. ptr::copy explicitly permits
                // overlap; no reference into either range survives this method.
                // The destination ends by the consumed payload end, preserving
                // every unread suffix byte. No pointer escapes, length changes,
                // deallocation or ownership transfer occurs during the move.
                unsafe {
                    core::ptr::copy(
                        pointer.add(self.input_position),
                        pointer.add(self.output_position),
                        count,
                    );
                }
            }
        }
        self.input_position = source_end;
        self.output_position = output_end;
        Ok(())
    }

    /// Read one RLE byte, protecting its payload-start fence conservatively.
    /// Even zero-length RLE consumes one source byte after all checks pass.
    pub(crate) fn repeat_rle(&mut self, count: usize) -> Result<()> {
        let source_end = self.source_end(1)?;
        let output_end = self.output_end(count, self.input_position)?;
        let byte = self.bytes[self.input_position];
        self.bytes[self.output_position..output_end].fill(byte);
        self.input_position = source_end;
        self.output_position = output_end;
        Ok(())
    }

    /// Disjoint output/history and immutable compressed payload borrows.
    /// The caller resumes checked history with the saved frame coordinates.
    /// No encoded cursor is retained across a write to consumed metadata bytes.
    pub(crate) fn compressed_parts(&mut self, encoded_size: usize) -> Result<(&mut [u8], &[u8])> {
        self.source_end(encoded_size)?;
        let allowance = self.output_limit.min(self.input_position);
        let (left, right) = self.bytes.split_at_mut(self.input_position);
        Ok((&mut left[..allowance], &right[..encoded_size]))
    }

    /// Publish a checked history endpoint after all block borrows end.
    /// The coordinator supplies the parsed encoded length and the history's
    /// returned position; this method does not establish semantic decoding.
    pub(crate) fn commit_compressed(
        &mut self,
        encoded_size: usize,
        output_end: usize,
    ) -> Result<()> {
        let source_end = self.source_end(encoded_size)?;
        if output_end < self.output_position {
            return Err(Error::InvalidState);
        }
        self.output_end(output_end - self.output_position, self.input_position)?;
        self.input_position = source_end;
        self.output_position = output_end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn every_small_raw_overlap_matches_original_bytes_and_preserves_outer_guards() {
        let maximum = if cfg!(miri) { 8 } else { 24 };
        for length in 0..=maximum {
            for start in 0..=length {
                for limit in 0..=length {
                    for count in 0..=length + 1 {
                        let mut bytes: Vec<_> =
                            (0..length + 2).map(|i| (i * 7 + 13) as u8).collect();
                        let before = bytes.clone();
                        let mut buffer =
                            InPlaceBuffer::new(&mut bytes[1..length + 1], start, limit).unwrap();
                        let old = buffer.positions();
                        let result = buffer.copy_raw(count);
                        if count <= length - start && count <= limit {
                            assert_eq!(result, Ok(()));
                            assert_eq!(buffer.positions(), (start + count, count, limit));
                            assert_eq!(buffer.written(), &before[1 + start..1 + start + count]);
                            assert_eq!(&bytes[1 + count..], &before[1 + count..]);
                            assert_eq!(bytes[0], before[0]);
                        } else {
                            assert!(result.is_err());
                            assert_eq!(buffer.positions(), old);
                            assert_eq!(bytes, before);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rle_fence_rejection_is_transactional_including_zero_lengths() {
        for start in 0..=16 {
            for limit in 0..=16 {
                for count in 0..=17 {
                    let mut bytes =
                        core::array::from_fn::<_, 18, _>(|index| (index * 3 + 17) as u8);
                    let before = bytes;
                    let mut buffer = InPlaceBuffer::new(&mut bytes[1..17], start, limit).unwrap();
                    let old = buffer.positions();
                    let result = buffer.repeat_rle(count);
                    if start < 16 && count <= start && count <= limit {
                        assert_eq!(result, Ok(()));
                        assert_eq!(buffer.positions(), (start + 1, count, limit));
                        assert!(
                            buffer
                                .written()
                                .iter()
                                .all(|&byte| byte == before[1 + start])
                        );
                        assert_eq!(&bytes[1 + count..], &before[1 + count..]);
                        assert_eq!(bytes[0], before[0]);
                    } else {
                        assert!(result.is_err());
                        assert_eq!(buffer.positions(), old);
                        assert_eq!(bytes, before);
                    }
                }
            }
        }
    }

    #[test]
    fn disjoint_compressed_views_and_frame_coordinates_survive_successive_blocks() {
        let mut bytes = *b"........abcXYZtail";
        let mut buffer = InPlaceBuffer::new(&mut bytes, 8, 12).unwrap();
        buffer.start_frame(1024);
        assert_eq!(buffer.frame_coordinates(), (0, 1024));
        {
            let (output, payload) = buffer.compressed_parts(3).unwrap();
            assert_eq!(output.len(), 8);
            assert_eq!(payload, b"abc");
            output[..4].copy_from_slice(b"done");
            assert_eq!(payload, b"abc");
        }
        buffer.commit_compressed(3, 4).unwrap();
        assert_eq!(buffer.written(), b"done");
        assert_eq!(buffer.frame_bytes(), b"done");
        buffer.start_frame(7);
        buffer.copy_raw(3).unwrap();
        assert_eq!(buffer.written(), b"doneXYZ");
        assert_eq!(buffer.frame_bytes(), b"XYZ");
        assert_eq!(buffer.frame_coordinates(), (4, 7));
        assert_eq!(buffer.remaining(), b"tail");
        buffer.consume(4).unwrap();
        assert!(buffer.remaining().is_empty());
        assert_eq!(&bytes[7..], b".abcXYZtail");
    }

    #[test]
    fn invalid_indices_overflow_and_publication_leave_bytes_and_state_unchanged() {
        let mut bytes = [0xa5; 16];
        assert!(InPlaceBuffer::new(&mut bytes, 17, 16).is_err());
        assert!(InPlaceBuffer::new(&mut bytes, usize::MAX, 16).is_err());
        let mut buffer = InPlaceBuffer::new(&mut bytes, 8, usize::MAX).unwrap();
        assert_eq!(buffer.positions(), (8, 0, 16));
        for count in [9, usize::MAX] {
            assert!(buffer.consume(count).is_err());
            assert!(buffer.copy_raw(count).is_err());
            assert!(buffer.compressed_parts(count).is_err());
            assert!(buffer.commit_compressed(count, 0).is_err());
            assert_eq!(buffer.positions(), (8, 0, 16));
        }
        assert!(buffer.repeat_rle(9).is_err());
        assert!(buffer.repeat_rle(usize::MAX).is_err());
        assert!(buffer.commit_compressed(0, 9).is_err());
        buffer.commit_compressed(0, 4).unwrap();
        assert_eq!(buffer.commit_compressed(0, 3), Err(Error::InvalidState));
        assert_eq!(buffer.positions(), (8, 4, 16));
        assert_eq!(bytes, [0xa5; 16]);
    }

    #[test]
    fn split_history_resumes_matches_and_preserves_unread_payload_across_blocks() {
        use crate::history::SequenceOutput;
        use crate::slice_history::SliceHistory;
        let mut bytes = *b"............abcPAYLOADtail";
        let mut buffer = InPlaceBuffer::new(&mut bytes, 12, 20).unwrap();
        buffer.start_frame(8);
        buffer.copy_raw(3).unwrap();
        let (_, position, _) = buffer.positions();
        let (start, window) = buffer.frame_coordinates();
        let end = {
            let (output, payload) = buffer.compressed_parts(7).unwrap();
            assert_eq!(payload, b"PAYLOAD");
            let limit = output.len();
            let mut history = SliceHistory::resume(output, position, start, window, limit).unwrap();
            history.copy_match(3, 9).unwrap();
            assert_eq!(history.written(), b"abcabcabcabc");
            assert_eq!(history.copy_match(3, 4), Err(Error::ResourceLimit));
            assert_eq!(payload, b"PAYLOAD");
            history.len()
        };
        buffer.commit_compressed(7, end).unwrap();
        assert_eq!(buffer.frame_bytes(), b"abcabcabcabc");
        assert_eq!(buffer.remaining(), b"tail");
        buffer.start_frame(8);
        let (_, position, _) = buffer.positions();
        let (start, window) = buffer.frame_coordinates();
        {
            let (output, payload) = buffer.compressed_parts(4).unwrap();
            let limit = output.len();
            let mut history = SliceHistory::resume(output, position, start, window, limit).unwrap();
            assert_eq!(history.copy_match(1, 1), Err(Error::CorruptData));
            history.append(b"new").unwrap();
            assert_eq!(payload, b"tail");
        }
        buffer.commit_compressed(4, 15).unwrap();
        assert_eq!(buffer.frame_bytes(), b"new");
        assert_eq!(buffer.written(), b"abcabcabcabcnew");
        assert_eq!(&bytes[15..], b"PAYLOADtail");
    }

    #[test]
    fn empty_slices_and_zero_payloads_never_require_a_pointer_access() {
        let mut bytes = [];
        let mut buffer = InPlaceBuffer::new(&mut bytes, 0, 0).unwrap();
        buffer.copy_raw(0).unwrap();
        buffer.consume(0).unwrap();
        let (left, right) = buffer.compressed_parts(0).unwrap();
        assert!(left.is_empty() && right.is_empty());
        buffer.commit_compressed(0, 0).unwrap();
        assert_eq!(buffer.repeat_rle(0), Err(Error::UnexpectedEof));
        assert_eq!(buffer.positions(), (0, 0, 0));
    }
}
