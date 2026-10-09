//! Deterministic, bounded raw dictionary selection with indexed sample ranges.
//!
//! See `docs/dictionary-training.md`. This independent greedy coverage strategy
//! serves the content-selection role of upstream dictionary builders. It does
//! not reproduce COVER/fastCover parameters or byte-identical trained output.

use alloc::collections::BinaryHeap;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::{Dictionary, Error, Result};

const HASH_BITS: u32 = 16;
const FREQUENCY_CELLS: usize = 1 << HASH_BITS;

/// Explicit resource and segment limits for raw dictionary training.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RawTrainingOptions {
    /// Maximum selected dictionary bytes, at least eight (default 8 KiB).
    pub dictionary_size: usize,
    /// Combined sample-byte budget, no more than u32::MAX (default 8 MiB).
    /// Larger datasets receive proportional per-sample prefix quotas.
    pub max_sample_bytes: usize,
    /// Candidate segment length in 8..=4096 bytes (default 64).
    pub segment_size: usize,
}

impl Default for RawTrainingOptions {
    fn default() -> Self {
        Self {
            dictionary_size: 8192,
            max_sample_bytes: 8 * 1024 * 1024,
            segment_size: 64,
        }
    }
}

impl RawTrainingOptions {
    pub(crate) fn validate(self) -> Result<Self> {
        if self.dictionary_size < 8
            || self.dictionary_size > self.max_sample_bytes
            || self.max_sample_bytes as u128 > u128::from(u32::MAX)
            || !(8..=4096).contains(&self.segment_size)
        {
            return Err(Error::InvalidParameter);
        }
        Ok(self)
    }
}

/// Select useful raw dictionary content from independent borrowed samples.
///
/// Construction is deterministic and fallibly allocated. Output may be smaller
/// than the requested cap when no uncovered four-byte keys remain. Selected
/// bytes are owned by the returned dictionary; no sample borrow escapes. Samples
/// contributing fewer than eight bytes under the budget are ignored. If no
/// candidate remains, this returns [`Error::TrainingFailed`].
///
/// This is raw content selection: it does not create formatted entropy metadata
/// or implement the original COVER/fastCover strategy-specific contracts.
pub fn train_raw_dictionary(samples: &[&[u8]], options: RawTrainingOptions) -> Result<Dictionary> {
    let options = options.validate()?;
    let windows = select_sample_windows(samples, options.max_sample_bytes)?;
    let mut frequencies = TrainingFrequencies::new()?;
    for window in &windows {
        frequencies.count(&samples[window.sample][..window.length])?;
    }
    let mut candidates = BinaryHeap::new();
    for window in &windows {
        let source = &samples[window.sample][..window.length];
        let length = options.segment_size.min(source.len());
        let mut start = 0;
        loop {
            let end = start + length;
            candidates
                .try_reserve(1)
                .map_err(|_| Error::AllocationFailed)?;
            candidates.push(Candidate {
                sample: window.sample,
                start,
                end,
                score: frequencies.score(&source[start..end]),
            });
            if end == source.len() {
                break;
            }
            start = (start + options.segment_size / 2).min(source.len() - length);
        }
    }
    let mut chosen = Vec::new();
    let mut length = 0;
    while options.dictionary_size - length >= 8 {
        let Some(mut candidate) = candidates.pop() else {
            break;
        };
        candidate.end = candidate.start
            + (candidate.end - candidate.start).min(options.dictionary_size - length);
        let source = samples[candidate.sample];
        candidate.score = frequencies.score(&source[candidate.start..candidate.end]);
        if candidates.peek().is_some_and(|next| candidate < *next) {
            // Popping freed a slot, so reinserting a stale bound cannot allocate.
            candidates.push(candidate);
            continue;
        }
        if candidate.score == 0 {
            break;
        }
        chosen.try_reserve(1).map_err(|_| Error::AllocationFailed)?;
        frequencies.cover(&source[candidate.start..candidate.end]);
        length += candidate.end - candidate.start;
        chosen.push(candidate);
    }
    if length < 8 {
        return Err(Error::TrainingFailed);
    }
    let mut content = Vec::new();
    content
        .try_reserve_exact(length)
        .map_err(|_| Error::AllocationFailed)?;
    for candidate in chosen.iter().rev() {
        content.extend_from_slice(&samples[candidate.sample][candidate.start..candidate.end]);
    }
    Ok(Dictionary::from_raw_owned(content))
}

/// A plain range descriptor; the caller's sample collection owns the bytes.
#[derive(Clone, Copy, Debug)]
struct SampleWindow {
    sample: usize,
    length: usize,
}

fn select_sample_windows(samples: &[&[u8]], budget: usize) -> Result<Vec<SampleWindow>> {
    let mut windows = Vec::new();
    visit_sample_prefixes(samples, budget, |sample, bytes| {
        if bytes.len() >= 8 {
            windows
                .try_reserve(1)
                .map_err(|_| Error::AllocationFailed)?;
            windows.push(SampleWindow {
                sample,
                length: bytes.len(),
            });
        }
        Ok(())
    })?;
    if windows.is_empty() {
        return Err(Error::TrainingFailed);
    }
    Ok(windows)
}

/// Visit deterministic proportional prefixes without allocating sample records.
///
/// All sample lengths are summed before visiting any source. Each quota rounds
/// down, so the aggregate visited bytes never exceed budget. Empty and short
/// prefixes are included: content selection filters them, but literal entropy
/// observations must count every selected byte. No caller borrow is retained.
pub(crate) fn visit_sample_prefixes(
    samples: &[&[u8]],
    budget: usize,
    mut visit: impl FnMut(usize, &[u8]) -> Result<()>,
) -> Result<()> {
    let total = samples.iter().try_fold(0_usize, |total, sample| {
        total.checked_add(sample.len()).ok_or(Error::ResourceLimit)
    })?;
    for (sample, bytes) in samples.iter().enumerate() {
        let length = if total > budget {
            let quota = (bytes.len() as u128)
                .checked_mul(budget as u128)
                .ok_or(Error::ResourceLimit)?
                / total as u128;
            usize::try_from(quota).map_err(|_| Error::ResourceLimit)?
        } else {
            bytes.len()
        };
        visit(sample, &bytes[..length])?;
    }
    Ok(())
}

/// Plain heap values; source indices and integer scores need no unsafe variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Candidate {
    sample: usize,
    start: usize,
    end: usize,
    score: u64,
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .cmp(&other.score)
            .then_with(|| other.sample.cmp(&self.sample))
            .then_with(|| other.start.cmp(&self.start))
            .then_with(|| other.end.cmp(&self.end))
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Owned fixed-size frequency cells, independent of borrowed training samples.
struct TrainingFrequencies {
    counts: Vec<u32>,
}

impl TrainingFrequencies {
    fn new() -> Result<Self> {
        let mut counts = Vec::new();
        counts
            .try_reserve_exact(FREQUENCY_CELLS)
            .map_err(|_| Error::AllocationFailed)?;
        counts.resize(FREQUENCY_CELLS, 0);
        Ok(Self { counts })
    }

    fn index(key: &[u8]) -> usize {
        let key = u32::from_le_bytes([key[0], key[1], key[2], key[3]]);
        (key.wrapping_mul(0x9e37_79b1) >> (32 - HASH_BITS)) as usize
    }

    fn count(&mut self, sample: &[u8]) -> Result<()> {
        for key in sample.windows(4) {
            let count = self.cell_mut(key);
            *count = count.checked_add(1).ok_or(Error::ResourceLimit)?;
        }
        Ok(())
    }

    fn score(&self, segment: &[u8]) -> u64 {
        // Segments contain at most 4096 bytes, so 4093*u32::MAX fits u64.
        segment
            .windows(4)
            .map(|key| u64::from(*self.cell(key)))
            .sum()
    }

    fn cover(&mut self, segment: &[u8]) {
        for key in segment.windows(4) {
            *self.cell_mut(key) = 0;
        }
    }

    fn cell(&self, key: &[u8]) -> &u32 {
        let index = Self::index(key);
        #[cfg(not(feature = "unsafe-training"))]
        {
            &self.counts[index]
        }
        #[cfg(feature = "unsafe-training")]
        {
            // SAFETY: index is a u32 hash shifted by 32-HASH_BITS, hence less
            // than 2^HASH_BITS = FREQUENCY_CELLS. Construction initializes that
            // many properly aligned u32 cells and no method changes length or
            // allocation. The shared self borrow keeps the allocation live and
            // prevents mutation for the returned reference's entire lifetime.
            // The reference cannot outlive self and only grants shared access.
            unsafe { self.counts.get_unchecked(index) }
        }
    }

    fn cell_mut(&mut self, key: &[u8]) -> &mut u32 {
        let index = Self::index(key);
        #[cfg(not(feature = "unsafe-training"))]
        {
            &mut self.counts[index]
        }
        #[cfg(feature = "unsafe-training")]
        {
            // SAFETY: the fixed-width hash proves index < FREQUENCY_CELLS.
            // All FREQUENCY_CELLS u32 entries were initialized by construction,
            // have Vec-provided alignment, and remain in the same live allocation
            // with unchanged length. The exclusive self borrow prevents any
            // competing references or access during the returned unique borrow.
            // Its lifetime is tied to self; this call does not reallocate.
            unsafe { self.counts.get_unchecked_mut(index) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn options(capacity: usize) -> RawTrainingOptions {
        RawTrainingOptions {
            dictionary_size: capacity,
            segment_size: 16,
            max_sample_bytes: 1024,
        }
    }

    #[test]
    fn invalid_limits_are_rejected_before_reading_samples() {
        for options in [
            RawTrainingOptions {
                dictionary_size: 7,
                ..options(8)
            },
            RawTrainingOptions {
                dictionary_size: 1025,
                ..options(8)
            },
            RawTrainingOptions {
                max_sample_bytes: 0,
                ..options(8)
            },
            RawTrainingOptions {
                segment_size: 7,
                ..options(8)
            },
            RawTrainingOptions {
                segment_size: 4097,
                ..options(8)
            },
        ] {
            assert_eq!(
                train_raw_dictionary(&[], options).unwrap_err(),
                Error::InvalidParameter
            );
        }
    }

    #[test]
    fn insufficient_samples_and_budget_quotas_fail_explicitly() {
        for samples in [
            &[][..],
            &[&b""[..]][..],
            &[&b"1234567"[..]][..],
            &[&b"short"[..], &b"short"[..]][..],
        ] {
            assert_eq!(
                train_raw_dictionary(samples, options(8)).unwrap_err(),
                Error::TrainingFailed
            );
        }
        let samples = [&b"12345678"[..]; 3];
        assert_eq!(
            train_raw_dictionary(
                &samples,
                RawTrainingOptions {
                    max_sample_bytes: 8,
                    ..options(8)
                }
            )
            .unwrap_err(),
            Error::TrainingFailed
        );
    }

    #[test]
    fn proportional_windows_do_not_exceed_the_sample_budget() {
        let samples = [
            &b"0123456789abcdef"[..],
            &b"abcdefghijklmnopqrstuvwx"[..],
            &b""[..],
        ];
        let windows = select_sample_windows(&samples, 23).unwrap();
        assert_eq!(
            windows
                .iter()
                .map(|window| window.length)
                .collect::<Vec<_>>(),
            [9, 13]
        );
        assert_eq!(
            windows
                .iter()
                .map(|window| window.sample)
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[test]
    fn repeated_content_stops_when_all_features_are_covered() {
        let bytes = vec![b'x'; 256];
        let dictionary = train_raw_dictionary(&[&bytes, &bytes], options(128)).unwrap();
        assert_eq!(dictionary.content(), &[b'x'; 16]);
        assert!(!dictionary.is_formatted());
        assert_eq!(dictionary.id(), 0);
    }

    #[test]
    fn selected_content_is_owned_deterministic_and_respects_every_small_capacity() {
        let samples = [
            b"common prefix, category one, end!".to_vec(),
            b"common prefix, category two, end!".to_vec(),
        ];
        let slices: Vec<_> = samples.iter().map(Vec::as_slice).collect();
        for capacity in 8..=99 {
            let first = train_raw_dictionary(&slices, options(capacity)).unwrap();
            let second = train_raw_dictionary(&slices, options(capacity)).unwrap();
            assert_eq!(first.content(), second.content());
            assert!((8..=capacity).contains(&first.content().len()));
        }
        let dictionary = train_raw_dictionary(&slices, options(64)).unwrap();
        drop(slices);
        drop(samples);
        assert!(!dictionary.content().is_empty());
    }

    #[test]
    fn large_requested_budget_does_not_force_a_large_output_allocation() {
        let bytes = b"0123456789abcdef";
        let dictionary = train_raw_dictionary(
            &[bytes],
            RawTrainingOptions {
                dictionary_size: u32::MAX as usize,
                max_sample_bytes: u32::MAX as usize,
                segment_size: 8,
            },
        )
        .unwrap();
        assert!(dictionary.content().len() <= bytes.len() * 2);
    }

    #[test]
    fn heap_ties_prefer_earlier_sample_and_start_indices() {
        let mut heap = BinaryHeap::new();
        for (sample, start) in [(2, 0), (1, 4), (0, 4), (0, 0)] {
            heap.push(Candidate {
                sample,
                start,
                end: start + 8,
                score: 100,
            });
        }
        let order: Vec<_> = core::iter::from_fn(|| heap.pop())
            .map(|item| (item.sample, item.start))
            .collect();
        assert_eq!(order, [(0, 0), (0, 4), (1, 4), (2, 0)]);
    }

    #[test]
    fn frequency_cells_count_cover_and_detect_overflow() {
        let mut frequencies = TrainingFrequencies::new().unwrap();
        frequencies.count(b"xxxxxxxx").unwrap();
        assert_eq!(frequencies.score(b"xxxx"), 5);
        frequencies.cover(b"xxxx");
        assert_eq!(frequencies.score(b"xxxxxxxx"), 0);
        let index = TrainingFrequencies::index(b"abcd");
        frequencies.counts[index] = u32::MAX;
        assert_eq!(frequencies.count(b"abcd"), Err(Error::ResourceLimit));
        assert_eq!(frequencies.counts[index], u32::MAX);
    }
}
