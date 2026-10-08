//! Bounded, resumable LZ history independent of the frame's logical length.
//!
//! Corresponds to prefix/window retention in upstream `ZSTD_DCtx` and the
//! match-copy part of `ZSTD_execSequence`. No caller buffer is retained.

use alloc::vec::Vec;

use crate::{Error, Result};

/// Owns the active frame's last window bytes, with reusable allocation.
///
/// Before the ring fills, `head == bytes.len()`. Once full, `head < window`
/// identifies the oldest byte and the next slot to overwrite. The initialized
/// length never exceeds `window`, which never exceeds `maximum_window`.
/// `produced` is independent of the physical index and never wraps.
pub(crate) struct RingHistory {
    bytes: Vec<u8>,
    head: usize,
    window: usize,
    maximum_window: usize,
    produced: u64,
}

impl RingHistory {
    /// Create an empty context without allocating the maximum window eagerly.
    pub(crate) fn new(maximum_window: usize) -> Self {
        Self {
            bytes: Vec::new(),
            head: 0,
            window: 0,
            maximum_window,
            produced: 0,
        }
    }

    /// Start an independent frame, retaining the allocation from earlier ones.
    ///
    /// Validates and fallibly reserves before clearing old state. Bytes are
    /// initialized as they are emitted, not when the advertised window arrives.
    /// Corresponds to resetting the active prefix in `ZSTD_decompressBegin`.
    pub(crate) fn start_frame(&mut self, window: usize) -> Result<()> {
        if window > self.maximum_window {
            return Err(Error::ResourceLimit);
        }
        if self.bytes.capacity() < window {
            // len <= capacity < window, so subtraction cannot underflow. The
            // requested final capacity is window regardless of the old length.
            self.bytes
                .try_reserve_exact(window - self.bytes.len())
                .map_err(|_| Error::AllocationFailed)?;
        }
        self.clear();
        self.window = window;
        Ok(())
    }

    /// Clear frame-local state while retaining allocated storage for reuse.
    ///
    /// A new frame must be started before producing more bytes. Retained
    /// capacity is the high-water allocation of previous successful windows;
    /// selecting a smaller next window does not shrink that allocation.
    pub(crate) fn clear(&mut self) {
        self.bytes.clear();
        self.head = 0;
        self.window = 0;
        self.produced = 0;
    }

    pub(crate) fn frame_len(&self) -> u64 {
        self.produced
    }

    pub(crate) fn available(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn window_size(&self) -> usize {
        self.window
    }

    pub(crate) fn retained_capacity(&self) -> usize {
        self.bytes.capacity()
    }

    /// Emit the literal prefix that fits in `output`, returning its length.
    ///
    /// The caller resumes with `input[written..]`. Errors leave both buffers
    /// and logical state unchanged. Corresponds to literal copying within
    /// upstream `ZSTD_execSequence` and raw-block decoding.
    pub(crate) fn append(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
        let count = input.len().min(output.len());
        let produced = self.checked_advance(count)?;
        for (destination, &byte) in output[..count].iter_mut().zip(&input[..count]) {
            self.store(byte);
            *destination = byte;
        }
        self.produced = produced;
        Ok(count)
    }

    /// Emit an RLE prefix, returning the count to subtract from `remaining`.
    ///
    /// Corresponds to upstream `ZSTD_setRleBlock`, with caller-owned output
    /// and history retained independently across partial writes.
    pub(crate) fn repeat(
        &mut self,
        byte: u8,
        remaining: usize,
        output: &mut [u8],
    ) -> Result<usize> {
        let count = remaining.min(output.len());
        let produced = self.checked_advance(count)?;
        for destination in &mut output[..count] {
            self.store(byte);
            *destination = byte;
        }
        self.produced = produced;
        Ok(count)
    }

    /// Emit a match prefix, retaining overlap semantics across resumptions.
    ///
    /// Distance must be nonzero and within available active-frame history,
    /// even for an empty output slice. Repeating this call with the same
    /// distance and a reduced `remaining` resumes the same LZ match.
    /// Corresponds to the match-copy part of upstream `ZSTD_execSequence`.
    pub(crate) fn copy_match(
        &mut self,
        distance: usize,
        remaining: usize,
        output: &mut [u8],
    ) -> Result<usize> {
        if distance == 0 || distance > self.bytes.len() {
            return Err(Error::CorruptData);
        }
        let count = remaining.min(output.len());
        let produced = self.checked_advance(count)?;
        for destination in &mut output[..count] {
            let source = if distance <= self.head {
                self.head - distance
            } else {
                self.window - (distance - self.head)
            };
            // Read before overwriting: at distance == window the source and
            // destination ring slots coincide, and the old byte is required.
            let byte = self.bytes[source];
            self.store(byte);
            *destination = byte;
        }
        self.produced = produced;
        Ok(count)
    }

    /// Validate the complete count before mutating output or history.
    fn checked_advance(&self, count: usize) -> Result<u64> {
        if count != 0 && self.window == 0 {
            return Err(Error::CorruptData);
        }
        let count = u64::try_from(count).map_err(|_| Error::ResourceLimit)?;
        self.produced.checked_add(count).ok_or(Error::ResourceLimit)
    }

    /// Store one byte after an emitting method has validated a nonzero window.
    fn store(&mut self, byte: u8) {
        if self.bytes.len() < self.window {
            // start_frame reserved the complete active window; push cannot
            // reallocate, and the initialized prefix grows by exactly one.
            self.bytes.push(byte);
        } else {
            self.bytes[self.head] = byte;
        }
        // head < window before this step, so head + 1 <= window cannot overflow.
        self.head += 1;
        if self.head == self.window {
            self.head = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn seed(history: &mut RingHistory, bytes: &[u8]) {
        let mut output = vec![0; bytes.len()];
        assert_eq!(history.append(bytes, &mut output).unwrap(), bytes.len());
        assert_eq!(output, bytes);
    }

    #[test]
    fn literals_rle_and_zero_output_resume_without_retaining_caller_buffers() {
        let mut history = RingHistory::new(5);
        history.start_frame(5).unwrap();
        assert_eq!(history.append(b"abcdefg", &mut []).unwrap(), 0);
        let mut output = [99; 4];
        assert_eq!(history.append(b"abcdefg", &mut output).unwrap(), 4);
        assert_eq!(&output, b"abcd");
        assert_eq!(history.append(b"efg", &mut output).unwrap(), 3);
        assert_eq!(&output, b"efgd");
        assert_eq!(history.repeat(b'x', 9, &mut output).unwrap(), 4);
        assert_eq!(&output, b"xxxx");
        assert_eq!(history.repeat(b'y', 2, &mut output).unwrap(), 2);
        assert_eq!(&output, b"yyxx");
        assert_eq!(history.frame_len(), 13);
        assert_eq!(history.available(), 5);
        assert_eq!(history.window_size(), 5);
    }

    #[test]
    fn matches_wrap_at_every_distance_and_preserve_whole_window_sources() {
        for window in 1..=19 {
            for distance in 1..=window {
                let mut history = RingHistory::new(window);
                history.start_frame(window).unwrap();
                let prefix: Vec<u8> = (0..window * 3 + 5).map(|n| n as u8).collect();
                seed(&mut history, &prefix);
                let length = window * 4 + 11;
                let mut expected = prefix.clone();
                for _ in 0..length {
                    expected.push(expected[expected.len() - distance]);
                }
                let mut actual = vec![0; length];
                assert_eq!(
                    history.copy_match(distance, length, &mut actual).unwrap(),
                    length
                );
                assert_eq!(actual, expected[prefix.len()..]);
                assert_eq!(history.available(), window);
            }
        }
    }

    #[test]
    fn partially_filled_match_can_overlap_and_resume_with_irregular_outputs() {
        for distance in 1..=17 {
            let mut history = RingHistory::new(17);
            history.start_frame(17).unwrap();
            let prefix: Vec<u8> = (0..distance as u8).collect();
            seed(&mut history, &prefix);
            let mut actual = Vec::new();
            let mut calls = 0;
            while actual.len() < 211 {
                let capacity = [0, 1, 2, 5, 31][calls % 5];
                calls += 1;
                let mut output = vec![0; capacity];
                let count = history
                    .copy_match(distance, 211 - actual.len(), &mut output)
                    .unwrap();
                actual.extend_from_slice(&output[..count]);
            }
            let expected: Vec<u8> = prefix.iter().copied().cycle().take(211).collect();
            assert_eq!(actual, expected);
            assert_eq!(history.frame_len(), (distance + 211) as u64);
        }
    }

    #[test]
    fn resets_reuse_capacity_and_forbid_cross_frame_matches() {
        let mut history = RingHistory::new(32);
        history.start_frame(32).unwrap();
        let capacity = history.retained_capacity();
        seed(&mut history, b"previous frame");
        history.start_frame(3).unwrap();
        assert_eq!(history.available(), 0);
        assert_eq!(history.frame_len(), 0);
        assert_eq!(history.retained_capacity(), capacity);
        assert_eq!(history.copy_match(1, 1, &mut [0]), Err(Error::CorruptData));
        seed(&mut history, b"new frame");
        history.clear();
        assert_eq!(history.available(), 0);
        assert_eq!(history.window_size(), 0);
        assert_eq!(history.frame_len(), 0);
        assert_eq!(history.retained_capacity(), capacity);
    }

    #[test]
    fn rejects_invalid_distances_and_windows_without_mutation() {
        let mut history = RingHistory::new(3);
        history.start_frame(3).unwrap();
        seed(&mut history, b"abcd");
        let bytes = history.bytes.clone();
        let head = history.head;
        let mut output = [99; 4];
        for distance in [0, 4, usize::MAX] {
            assert_eq!(
                history.copy_match(distance, 4, &mut output),
                Err(Error::CorruptData)
            );
            assert_eq!(
                history.copy_match(distance, 0, &mut []),
                Err(Error::CorruptData)
            );
        }
        assert_eq!(history.start_frame(4), Err(Error::ResourceLimit));
        assert_eq!(history.bytes, bytes);
        assert_eq!(history.head, head);
        assert_eq!(history.frame_len(), 4);
        assert_eq!(output, [99; 4]);
    }

    #[test]
    fn zero_window_allows_empty_progress_and_rejects_produced_bytes() {
        let mut history = RingHistory::new(0);
        history.start_frame(0).unwrap();
        let mut output = [99];
        assert_eq!(history.append(&[], &mut output).unwrap(), 0);
        assert_eq!(history.append(b"x", &mut []).unwrap(), 0);
        assert_eq!(history.repeat(0, 0, &mut output).unwrap(), 0);
        assert_eq!(history.append(b"x", &mut output), Err(Error::CorruptData));
        assert_eq!(history.repeat(0, 1, &mut output), Err(Error::CorruptData));
        assert_eq!(history.copy_match(1, 0, &mut []), Err(Error::CorruptData));
        assert_eq!(output, [99]);
        assert_eq!(history.retained_capacity(), 0);
    }

    #[test]
    fn allocation_failure_preserves_the_previous_frame() {
        let mut history = RingHistory::new(usize::MAX);
        history.start_frame(3).unwrap();
        seed(&mut history, b"abc");
        assert_eq!(
            history.start_frame(usize::MAX),
            Err(Error::AllocationFailed)
        );
        let mut output = [0; 6];
        history.copy_match(3, 6, &mut output).unwrap();
        assert_eq!(&output, b"abcabc");
    }

    #[test]
    fn global_count_above_u32_does_not_determine_ring_indices() {
        let mut history = RingHistory::new(7);
        history.start_frame(7).unwrap();
        seed(&mut history, b"123456789abcdef");
        history.produced = u64::from(u32::MAX) + 91;
        let before = history.frame_len();
        let mut output = [0; 15];
        history.copy_match(3, 15, &mut output).unwrap();
        assert_eq!(&output, b"defdefdefdefdef");
        assert_eq!(history.frame_len(), before + 15);
    }

    #[test]
    fn counter_overflow_is_atomic_for_all_emission_modes() {
        let mut history = RingHistory::new(3);
        history.start_frame(3).unwrap();
        seed(&mut history, b"abc");
        history.produced = u64::MAX - 2;
        let bytes = history.bytes.clone();
        let head = history.head;
        let mut output = [99; 3];
        assert_eq!(
            history.append(b"xyz", &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            history.repeat(b'x', 3, &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            history.copy_match(1, 3, &mut output),
            Err(Error::ResourceLimit)
        );
        assert_eq!(history.bytes, bytes);
        assert_eq!(history.head, head);
        assert_eq!(history.frame_len(), u64::MAX - 2);
        assert_eq!(output, [99; 3]);
        assert_eq!(
            history.copy_match(1, usize::MAX, &mut output[..2]).unwrap(),
            2
        );
        assert_eq!(history.frame_len(), u64::MAX);
        assert_eq!(history.copy_match(1, usize::MAX, &mut []).unwrap(), 0);
        assert_eq!(history.repeat(0, 1, &mut output), Err(Error::ResourceLimit));
    }

    #[test]
    fn retained_history_stays_bounded_over_many_output_chunks() {
        let mut history = RingHistory::new(13);
        history.start_frame(13).unwrap();
        let capacity = history.retained_capacity();
        let mut output = [0; 37];
        for index in 0..4096 {
            history
                .repeat(index as u8, output.len(), &mut output)
                .unwrap();
            assert_eq!(history.available(), 13);
            assert_eq!(history.retained_capacity(), capacity);
        }
        assert_eq!(history.frame_len(), 4096 * 37);
    }
}
