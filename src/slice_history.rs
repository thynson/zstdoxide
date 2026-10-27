//! Direct caller-owned output with checked, frame-local LZ history.

use crate::history::{FrameOutput, SequenceOutput};
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

    /// Reborrow initialized caller storage using validated frame-local positions.
    /// The coordinator supplies only previously written output as the prefix;
    /// this validates coordinates, not the semantic origin of those bytes.
    #[cfg(test)]
    pub(crate) fn resume(
        output: &'a mut [u8],
        position: usize,
        frame_start: usize,
        window: usize,
        limit: usize,
    ) -> Result<Self> {
        if frame_start > position || position > limit || limit > output.len() {
            return Err(Error::InvalidState);
        }
        Ok(Self {
            output,
            position,
            frame_start,
            window,
            limit,
        })
    }

    pub(crate) fn start_frame(&mut self, window: usize) {
        self.frame_start = self.position;
        self.window = window;
    }

    pub(crate) fn frame_bytes(&self) -> &[u8] {
        &self.output[self.frame_start..self.position]
    }

    /// All written output, including completed frames, borrowed through this sink.
    pub(crate) fn written(&self) -> &[u8] {
        &self.output[..self.position]
    }

    /// Restart the same output borrow without clearing bytes or changing limits.
    pub(crate) fn reset(&mut self) {
        self.position = 0;
        self.frame_start = 0;
        self.window = 0;
    }

    /// Expand one match from immutable dictionary content into current-frame output.
    /// Initial dictionary distances may exceed the advertised frame window.
    pub(crate) fn copy_dictionary_match(
        &mut self,
        dictionary: crate::DictionaryView<'_>,
        distance: usize,
        length: usize,
    ) -> Result<()> {
        let position = self.position - self.frame_start;
        if distance <= position {
            return self.copy_match(distance, length);
        }
        if position > self.window {
            return Err(Error::CorruptData);
        }
        let before_frame = distance - position;
        let source = dictionary
            .content()
            .len()
            .checked_sub(before_frame)
            .ok_or(Error::CorruptData)?;
        let end = self.checked_end(length)?;
        #[cfg(not(feature = "unsafe-slice-history"))]
        for index in 0..length {
            let byte = if index < before_frame {
                dictionary.content_byte(source + index)?
            } else {
                self.output[self.frame_start + index - before_frame]
            };
            self.output[self.position + index] = byte;
        }
        #[cfg(feature = "unsafe-slice-history")]
        {
            let pointer = self.output.as_mut_ptr();
            // SAFETY: checked_end proves position + length = end <= limit <=
            // output.len() without overflow. The exclusive initialized u8 slice
            // remains alive, aligned to one byte and unaliased throughout this
            // loop; the separately shared dictionary cannot alias it in safe
            // callers. The dictionary range was validated before any writes:
            // source + index < content.len() for index < before_frame, and its
            // accessor independently checks that bound. For the frame portion,
            // frame_start + index - before_frame is strictly below destination
            // position + index by distance = position - frame_start +
            // before_frame > 0. It is at least frame_start and therefore reads
            // an initialized old prefix byte or an earlier write in this loop.
            // Each destination is below end. No reference, allocation or length
            // change occurs; forward iteration preserves overlap. An empty loop
            // never dereferences the pointer, even for a zero-capacity slice.
            unsafe {
                for index in 0..length {
                    let byte = if index < before_frame {
                        dictionary.content_byte(source + index)?
                    } else {
                        pointer.add(self.frame_start + index - before_frame).read()
                    };
                    pointer.add(self.position + index).write(byte);
                }
            }
        }
        self.position = end;
        Ok(())
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

impl FrameOutput for SliceHistory<'_> {
    fn start_frame(&mut self, window: usize) {
        SliceHistory::start_frame(self, window);
    }
    fn frame_bytes(&self) -> &[u8] {
        SliceHistory::frame_bytes(self)
    }
    fn repeat(&mut self, byte: u8, count: usize) -> Result<()> {
        SliceHistory::repeat(self, byte, count)
    }
}

/// Borrowed dictionary metadata adds no storage family or unsafe representation.
pub(crate) struct DictionarySliceOutput<'output, 'dictionary> {
    history: SliceHistory<'output>,
    dictionary: crate::DictionaryView<'dictionary>,
}

impl<'output, 'dictionary> DictionarySliceOutput<'output, 'dictionary> {
    pub(crate) fn new(
        output: &'output mut [u8],
        limit: usize,
        dictionary: crate::DictionaryView<'dictionary>,
    ) -> Self {
        Self {
            history: SliceHistory::new(output, limit),
            dictionary,
        }
    }
}

impl SequenceOutput for DictionarySliceOutput<'_, '_> {
    fn len(&self) -> usize {
        self.history.len()
    }
    fn append(&mut self, bytes: &[u8]) -> Result<()> {
        self.history.append(bytes)
    }
    fn copy_match(&mut self, offset: usize, length: usize) -> Result<()> {
        self.history
            .copy_dictionary_match(self.dictionary, offset, length)
    }
}

impl FrameOutput for DictionarySliceOutput<'_, '_> {
    fn start_frame(&mut self, window: usize) {
        self.history.start_frame(window);
    }
    fn frame_bytes(&self) -> &[u8] {
        self.history.frame_bytes()
    }
    fn repeat(&mut self, byte: u8, count: usize) -> Result<()> {
        self.history.repeat(byte, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_checks_all_coordinates_before_admitting_frame_history() {
        for capacity in 0..=8 {
            for position in 0..=9 {
                for frame_start in 0..=9 {
                    for limit in 0..=9 {
                        let mut output = [0xa5; 10];
                        let result = SliceHistory::resume(
                            &mut output[1..1 + capacity],
                            position,
                            frame_start,
                            3,
                            limit,
                        );
                        let valid =
                            frame_start <= position && position <= limit && limit <= capacity;
                        assert_eq!(result.is_ok(), valid);
                        if let Ok(mut history) = result {
                            assert_eq!(history.len(), position);
                            assert_eq!(history.frame_bytes().len(), position - frame_start);
                            let before = history.output.to_vec();
                            assert_eq!(history.copy_match(0, 0), Err(Error::CorruptData));
                            assert_eq!(history.repeat(0, usize::MAX), Err(Error::ResourceLimit));
                            assert_eq!(history.output, before);
                        }
                        assert_eq!(output, [0xa5; 10]);
                    }
                }
            }
        }
        for (position, start, limit) in [
            (usize::MAX, 0, usize::MAX),
            (0, usize::MAX, 0),
            (0, 0, usize::MAX),
        ] {
            let mut empty = [];
            assert!(matches!(
                SliceHistory::resume(&mut empty, position, start, 0, limit),
                Err(Error::InvalidState)
            ));
        }
    }

    #[test]
    fn resumed_history_matches_continuous_writes_and_excludes_prior_frames() {
        for prefix in 1..=16 {
            for offset in 1..=prefix {
                for length in [0, 1, offset, offset + 1, 33] {
                    let mut continuous = [0xa5; 80];
                    let mut resumed = continuous;
                    let prefix_bytes: alloc::vec::Vec<_> = (0..prefix as u8).collect();
                    let mut history = SliceHistory::new(&mut continuous, 70);
                    history.append(b"old").unwrap();
                    history.start_frame(prefix);
                    history.append(&prefix_bytes).unwrap();
                    history.copy_match(offset, length).unwrap();
                    resumed[..3].copy_from_slice(b"old");
                    resumed[3..3 + prefix].copy_from_slice(&prefix_bytes);
                    let mut history =
                        SliceHistory::resume(&mut resumed, 3 + prefix, 3, prefix, 70).unwrap();
                    assert_eq!(history.copy_match(prefix + 1, 0), Err(Error::CorruptData));
                    history.copy_match(offset, length).unwrap();
                    assert_eq!(continuous, resumed);
                }
            }
        }
    }

    #[test]
    fn session_reset_preserves_bytes_borrow_and_limit_but_discards_old_history() {
        let mut output = [0xa5; 16];
        let mut history = SliceHistory::new(&mut output, 7);
        let pointer = history.output.as_ptr();
        history.start_frame(7);
        history.append(b"abc").unwrap();
        history.start_frame(4);
        history.append(b"defg").unwrap();
        assert_eq!(history.written(), b"abcdefg");
        assert_eq!(history.frame_bytes(), b"defg");
        history.reset();
        assert_eq!(history.output.as_ptr(), pointer);
        assert_eq!(
            history.output,
            b"abcdefg\xa5\xa5\xa5\xa5\xa5\xa5\xa5\xa5\xa5"
        );
        assert!(history.written().is_empty());
        assert!(history.frame_bytes().is_empty());
        assert_eq!(history.copy_match(1, 1), Err(Error::CorruptData));
        history.start_frame(1);
        history.append(b"z").unwrap();
        history.copy_match(1, 6).unwrap();
        assert_eq!(history.written(), b"zzzzzzz");
        assert_eq!(history.append(b"!"), Err(Error::ResourceLimit));
        history.reset();
        history.reset();
        assert!(history.written().is_empty());
        assert_eq!(&output[..7], b"zzzzzzz");
        assert_eq!(&output[7..], &[0xa5; 9]);
    }

    #[test]
    fn dictionary_matches_cover_every_small_crossing_and_frame_start() {
        for dictionary_length in 0..=16 {
            let content: alloc::vec::Vec<_> = (0..dictionary_length as u8).collect();
            let dictionary = crate::Dictionary::raw(&content).unwrap();
            for prefix in 0_usize..=16 {
                for distance in 1..=dictionary_length + prefix {
                    for length in [0, 1, distance - 1, distance, distance + 1, 65] {
                        for window in [0, prefix.saturating_sub(1), prefix, 32] {
                            let mut output = [0xa5; 128];
                            let mut history = SliceHistory::new(&mut output[1..127], 100);
                            history.append(b"old").unwrap();
                            history.start_frame(window);
                            let frame: alloc::vec::Vec<_> =
                                (0..prefix).map(|i| 32 + i as u8).collect();
                            history.append(&frame).unwrap();
                            let before = history.output.to_vec();
                            let result =
                                history.copy_dictionary_match(dictionary.view(), distance, length);
                            let permitted = if distance <= prefix {
                                distance <= window
                            } else {
                                prefix <= window
                            };
                            if permitted {
                                result.unwrap();
                                let mut logical = content.clone();
                                logical.extend_from_slice(&frame);
                                for _ in 0..length {
                                    logical.push(logical[logical.len() - distance]);
                                }
                                assert_eq!(history.frame_bytes(), &logical[dictionary_length..]);
                                assert_eq!(&history.output[..3], b"old");
                            } else {
                                assert_eq!(result, Err(Error::CorruptData));
                                assert_eq!(history.output, before.as_slice());
                            }
                            let written = history.len();
                            assert!(output[1 + written..].iter().all(|&b| b == 0xa5));
                            assert_eq!(output[0], 0xa5);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_dictionary_matches_and_capacity_fail_before_any_write() {
        let dictionary = crate::Dictionary::raw(b"dict").unwrap();
        for (distance, length, window, expected) in [
            (0, 0, 8, Error::CorruptData),
            (0, usize::MAX, 8, Error::CorruptData),
            (7, 0, 8, Error::CorruptData),
            (usize::MAX, 1, 8, Error::CorruptData),
            (3, 0, 1, Error::CorruptData),
            (3, 1, 1, Error::CorruptData),
            (2, 0, 1, Error::CorruptData),
            (6, usize::MAX, 2, Error::ResourceLimit),
            (3, 7, 2, Error::ResourceLimit),
        ] {
            let mut output = [0xa5; 16];
            let mut history = SliceHistory::new(&mut output, 10);
            history.append(b"old").unwrap();
            history.start_frame(window);
            history.append(b"ab").unwrap();
            let before = history.output.to_vec();
            assert_eq!(
                history.copy_dictionary_match(dictionary.view(), distance, length),
                Err(expected)
            );
            assert_eq!(history.output, before.as_slice());
            assert_eq!(
                (
                    history.position,
                    history.frame_start,
                    history.window,
                    history.limit
                ),
                (5, 3, window, 10)
            );
        }
        let mut empty = [];
        let mut history = SliceHistory::new(&mut empty, 0);
        history.start_frame(0);
        history
            .copy_dictionary_match(dictionary.view(), 4, 0)
            .unwrap();
        assert_eq!(
            history.copy_dictionary_match(dictionary.view(), 4, 1),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            history.copy_dictionary_match(dictionary.view(), 5, 0),
            Err(Error::CorruptData)
        );
    }

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
