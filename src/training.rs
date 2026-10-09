//! Deterministic, bounded raw dictionary selection with indexed sample ranges.
//!
//! See `docs/dictionary-training.md`. This independent greedy coverage strategy
//! serves the content-selection role of upstream dictionary builders. It does
//! not reproduce COVER/fastCover parameters or byte-identical trained output.

use alloc::alloc::Global;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::cmp::Ordering;

use crate::heap::MaxHeap;
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
    train_raw_dictionary_in(samples, options, Global)
}

/// Select raw dictionary content using the supplied allocation family.
///
/// The returned dictionary retains the original allocator and owns its bytes
/// directly. Window records, frequency cells, candidates and selected ranges
/// each retain a fresh allocator clone; clones need not share an allocation
/// domain. Every failed collection allocation returns [`Error::AllocationFailed`]
/// and releases completed temporary owners. Allocator cloning itself may have
/// ordinary user-defined allocation or panic behavior. Passing a reference to
/// a non-Clone allocator avoids cloning its underlying state; that allocator
/// must outlive the returned dictionary. No sample borrow escapes this call.
/// Selection, quotas and tie rules match [`train_raw_dictionary`].
pub fn train_raw_dictionary_in<A: Allocator + Clone>(
    samples: &[&[u8]],
    options: RawTrainingOptions,
    allocator: A,
) -> Result<Dictionary<A>> {
    let options = options.validate()?;
    let windows = select_sample_windows_in(samples, options.max_sample_bytes, allocator.clone())?;
    let mut frequencies = TrainingFrequencies::new_in(allocator.clone())?;
    for window in &windows {
        frequencies.count(&samples[window.sample][..window.length])?;
    }
    let mut candidates = MaxHeap::new_in(allocator.clone());
    for window in &windows {
        let source = &samples[window.sample][..window.length];
        let length = options.segment_size.min(source.len());
        let mut start = 0;
        loop {
            let end = start + length;
            candidates.try_push(Candidate {
                sample: window.sample,
                start,
                end,
                score: frequencies.score(&source[start..end]),
            })?;
            if end == source.len() {
                break;
            }
            start = (start + options.segment_size / 2).min(source.len() - length);
        }
    }
    let mut chosen = Vec::new_in(allocator.clone());
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
            candidates.try_push(candidate)?;
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
    let mut content = Vec::new_in(allocator);
    content
        .try_reserve_exact(length)
        .map_err(|_| Error::AllocationFailed)?;
    for candidate in chosen.iter().rev() {
        // The complete sum of selected range lengths was reserved above.
        // Individual pushes stay within that proven capacity without relying
        // on bulk insertion's allocation behavior.
        for &byte in &samples[candidate.sample][candidate.start..candidate.end] {
            content.push(byte);
        }
    }
    Ok(Dictionary::from_raw_owned(content))
}

/// A plain range descriptor; the caller's sample collection owns the bytes.
#[derive(Clone, Copy, Debug)]
struct SampleWindow {
    sample: usize,
    length: usize,
}

#[cfg(test)]
fn select_sample_windows(samples: &[&[u8]], budget: usize) -> Result<Vec<SampleWindow>> {
    select_sample_windows_in(samples, budget, Global)
}

fn select_sample_windows_in<A: Allocator>(
    samples: &[&[u8]],
    budget: usize,
    allocator: A,
) -> Result<Vec<SampleWindow, A>> {
    let mut windows = Vec::new_in(allocator);
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
struct TrainingFrequencies<A: Allocator = Global> {
    counts: Vec<u32, A>,
}

#[cfg(test)]
impl TrainingFrequencies<Global> {
    fn new() -> Result<Self> {
        Self::new_in(Global)
    }
}

impl<A: Allocator> TrainingFrequencies<A> {
    fn new_in(allocator: A) -> Result<Self> {
        let mut counts = Vec::new_in(allocator);
        counts
            .try_reserve_exact(FREQUENCY_CELLS)
            .map_err(|_| Error::AllocationFailed)?;
        // Reserve the complete fixed-size owner before initializing entries.
        while counts.len() < FREQUENCY_CELLS {
            counts.push(0);
        }
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
    use core::num::NonZeroU64;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn allocator() -> TrackingAllocator {
        TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 8,
            fail_at: None,
        })
        .unwrap()
    }

    #[test]
    fn allocator_leaf_owners_accept_non_clone_values_and_preserve_frequency_cells() {
        let owner = allocator().into_non_clone();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let samples = [&b"0123456789abcdef"[..], &b"abcdefghijklmnopqrstuvwx"[..]];
        let windows = select_sample_windows_in(&samples, 23, owner).unwrap();
        assert_eq!(windows.allocator().identity(), identity);
        assert_eq!((windows[0].length, windows[1].length), (9, 13));
        drop(windows);
        monitor.assert_released();

        let owner = allocator().into_non_clone();
        let identity = owner.identity();
        let monitor = owner.monitor();
        let mut frequencies = TrainingFrequencies::new_in(owner).unwrap();
        assert_eq!(frequencies.counts.allocator().identity(), identity);
        assert_eq!(frequencies.counts.len(), FREQUENCY_CELLS);
        assert!(frequencies.counts.iter().all(|&count| count == 0));
        frequencies.count(b"xxxxxxxx").unwrap();
        assert_eq!(frequencies.score(b"xxxx"), 5);
        frequencies.cover(b"xxxx");
        assert_eq!(frequencies.score(b"xxxxxxxx"), 0);
        *frequencies.cell_mut(b"abcd") = u32::MAX;
        assert_eq!(frequencies.count(b"abcd"), Err(Error::ResourceLimit));
        assert_eq!(frequencies.score(b"abcd"), u64::from(u32::MAX));
        drop(frequencies);
        assert_eq!(monitor.snapshot().identities_created, 1);
        assert_eq!(monitor.snapshot().attempts, 1);
        monitor.assert_released();
    }

    #[test]
    fn allocator_borrowed_training_matches_global_across_quotas_and_capacities() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        let binary: Vec<_> = (0..512).map(|index| (index * 19) as u8).collect();
        let samples = [&binary[..256], &binary[13..]];
        for capacity in 8..=99 {
            for budget in [128, 1024] {
                let options = RawTrainingOptions {
                    max_sample_bytes: budget,
                    ..options(capacity)
                };
                let expected = train_raw_dictionary(&samples, options).unwrap();
                let actual = train_raw_dictionary_in(&samples, options, &owner).unwrap();
                assert_eq!(actual.as_bytes(), expected.as_bytes());
                assert_eq!(actual.id(), 0);
                assert!(!actual.is_formatted());
                assert_eq!(monitor.snapshot().live_blocks, 1);
                drop(actual);
                monitor.assert_released();
            }
        }
        assert_eq!(monitor.snapshot().identities_created, 1);
    }

    #[test]
    fn allocator_independent_scratch_domains_release_before_returning_owned_content() {
        let samples = [
            b"common prefix, first distinct suffix".to_vec(),
            b"common prefix, second distinct suffix".to_vec(),
        ];
        let views: Vec<_> = samples.iter().map(Vec::as_slice).collect();
        let expected = train_raw_dictionary(&views, options(64)).unwrap();
        let owner = allocator();
        let monitor = owner.monitor();
        let dictionary = train_raw_dictionary_in(&views, options(64), owner).unwrap();
        assert_eq!(dictionary.as_bytes(), expected.as_bytes());
        // Four fresh scratch owners have dropped; the original content owner
        // alone survives inside the returned dictionary, without a second copy.
        assert_eq!(monitor.snapshot().identities_created, 5);
        assert_eq!(monitor.snapshot().owners_dropped, 4);
        assert_eq!(monitor.snapshot().live_blocks, 1);
        drop(views);
        drop(samples);
        let pointer = dictionary.as_bytes().as_ptr();
        let moved = dictionary;
        assert_eq!(moved.as_bytes().as_ptr(), pointer);
        assert_eq!(moved.as_bytes(), expected.as_bytes());
        drop(moved);
        assert_eq!(monitor.snapshot().owners_dropped, 5);
        monitor.assert_released();
    }

    #[test]
    fn allocator_invalid_limits_and_empty_quotas_do_not_request_storage() {
        let owner = allocator().into_non_clone();
        let monitor = owner.monitor();
        assert!(matches!(
            train_raw_dictionary_in(&[b"12345678"], options(7), &owner),
            Err(Error::InvalidParameter)
        ));
        for samples in [&[][..], &[&b"short"[..]][..], &[&b"12345678"[..]; 3][..]] {
            assert!(matches!(
                train_raw_dictionary_in(
                    samples,
                    RawTrainingOptions {
                        max_sample_bytes: 8,
                        ..options(8)
                    },
                    &owner
                ),
                Err(Error::TrainingFailed)
            ));
        }
        assert_eq!(monitor.snapshot().attempts, 0);
        monitor.assert_released();
    }

    #[test]
    fn allocator_every_training_allocation_failure_releases_all_partial_owners() {
        let sources: Vec<Vec<_>> = (0..17)
            .map(|sample| {
                (0..64)
                    .map(|index| (sample * 53 + index * 29 + index * index) as u8)
                    .collect()
            })
            .collect();
        let views: Vec<_> = sources.iter().map(Vec::as_slice).collect();
        let options = RawTrainingOptions {
            max_sample_bytes: 2048,
            ..options(512)
        };
        let expected = train_raw_dictionary(&views, options).unwrap();
        assert!(expected.content().len() > 64);
        let owner = allocator();
        let monitor = owner.monitor();
        let baseline = train_raw_dictionary_in(&views, options, owner).unwrap();
        let attempts = monitor.snapshot().attempts;
        assert!(monitor.snapshot().grow_calls >= 3);
        assert_eq!(baseline.as_bytes(), expected.as_bytes());
        drop(baseline);
        monitor.assert_released();
        for ordinal in 1..=attempts {
            let owner = allocator();
            let monitor = owner.monitor();
            monitor.set_fail_at(NonZeroU64::new(ordinal));
            assert!(
                matches!(
                    train_raw_dictionary_in(&views, options, owner),
                    Err(Error::AllocationFailed)
                ),
                "allocation {ordinal}"
            );
            assert_eq!(monitor.snapshot().failures, 1);
            monitor.assert_released();
            assert_eq!(
                monitor.snapshot().owners_dropped,
                monitor.snapshot().identities_created
            );
        }
        assert_eq!(
            train_raw_dictionary(&views, options).unwrap().as_bytes(),
            expected.as_bytes()
        );
    }

    #[test]
    fn allocator_initial_window_and_frequency_failures_release_prior_storage() {
        for ordinal in [1, 2] {
            let owner = allocator();
            let monitor = owner.monitor();
            monitor.set_fail_at(NonZeroU64::new(ordinal));
            assert!(matches!(
                train_raw_dictionary_in(&[b"0123456789abcdef"], options(16), owner),
                Err(Error::AllocationFailed)
            ));
            assert_eq!(monitor.snapshot().failures, 1);
            monitor.assert_released();
        }
    }

    #[test]
    fn allocator_final_content_failure_releases_every_completed_owner() {
        let samples = [&b"0123456789abcdef"[..]];
        let owner = allocator();
        let monitor = owner.monitor();
        let baseline = train_raw_dictionary_in(&samples, options(16), owner).unwrap();
        let final_request = monitor.snapshot().attempts;
        assert_eq!(baseline.content(), samples[0]);
        drop(baseline);
        monitor.assert_released();
        let owner = allocator();
        let monitor = owner.monitor();
        monitor.set_fail_at(NonZeroU64::new(final_request));
        assert!(matches!(
            train_raw_dictionary_in(&samples, options(16), owner),
            Err(Error::AllocationFailed)
        ));
        assert_eq!(monitor.snapshot().failures, 1);
        assert_eq!(monitor.snapshot().identities_created, 5);
        assert_eq!(monitor.snapshot().owners_dropped, 5);
        monitor.assert_released();
    }

    fn options(capacity: usize) -> RawTrainingOptions {
        RawTrainingOptions {
            dictionary_size: capacity,
            segment_size: 16,
            max_sample_bytes: 1024,
        }
    }

    #[test]
    fn trained_content_preserves_the_pre_allocator_queue_baseline() {
        let messages = [
            &b"common prefix, category one, region east, end!"[..],
            &b"common prefix, category two, region west, end!"[..],
            &b"common prefix, category three, region east, end!"[..],
        ];
        let binary: Vec<_> = (0..512).map(|index| (index * 19) as u8).collect();
        let binary_samples = [&binary[..256], &binary[13..]];
        let repeated = [b'x'; 256];
        let repeated_samples = [&repeated[..], &repeated[..128]];
        let mut fingerprint = 0xcbf29ce484222325_u64;
        let mut cases = 0_u32;
        for samples in [&messages[..], &binary_samples[..], &repeated_samples[..]] {
            for capacity in [8, 9, 16, 17, 31, 64, 99, 128] {
                for budget in [128, 1024] {
                    for segment_size in [8, 16, 64] {
                        let dictionary = train_raw_dictionary(
                            samples,
                            RawTrainingOptions {
                                dictionary_size: capacity,
                                max_sample_bytes: budget,
                                segment_size,
                            },
                        )
                        .unwrap();
                        for byte in (dictionary.content().len() as u32)
                            .to_le_bytes()
                            .iter()
                            .chain(dictionary.content())
                        {
                            fingerprint =
                                (fingerprint ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
                        }
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 144);
        // Captured from the Global std BinaryHeap implementation before the
        // allocator-compatible queue replaced it. Includes quotas, clipped
        // capacities, coverage exhaustion, binary bytes and equal-score ties.
        assert_eq!(fingerprint, 11_733_226_933_974_065_203);
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
        let mut heap = MaxHeap::new_in(Global);
        for (sample, start, end) in [(2, 0, 8), (1, 4, 12), (0, 4, 12), (0, 0, 9), (0, 0, 8)] {
            heap.try_push(Candidate {
                sample,
                start,
                end,
                score: 100,
            })
            .unwrap();
        }
        let order: Vec<_> = core::iter::from_fn(|| heap.pop())
            .map(|item| (item.sample, item.start, item.end))
            .collect();
        assert_eq!(
            order,
            [(0, 0, 8), (0, 0, 9), (0, 4, 12), (1, 4, 12), (2, 0, 8)]
        );
    }

    #[test]
    fn stale_score_reinsertions_match_the_standard_candidate_queue() {
        use alloc::collections::BinaryHeap;

        let mut actual = MaxHeap::new_in(Global);
        let mut expected = BinaryHeap::new();
        for index in 0..257 {
            let value = Candidate {
                sample: index % 7,
                start: index % 11,
                end: index % 11 + 8 + index % 5,
                score: 1 + (index * 19 % 13) as u64,
            };
            actual.try_push(value).unwrap();
            expected.push(value);
        }
        let mut reinsertions = 0;
        let mut accepted = 0;
        while let Some(mut candidate) = expected.pop() {
            assert_eq!(actual.pop(), Some(candidate));
            assert_eq!(actual.peek(), expected.peek());
            // Coverage removes positive contributions; clipping can also alter
            // the final tie component. Each recomputation strictly reduces the
            // bound until it can be selected, so this campaign terminates.
            candidate.score = candidate.score.saturating_sub(1);
            candidate.end = candidate.end.min(candidate.start + 9);
            if expected.peek().is_some_and(|next| candidate < *next) {
                expected.push(candidate);
                actual.try_push(candidate).unwrap();
                reinsertions += 1;
            } else {
                accepted += 1;
            }
            assert_eq!(actual.peek(), expected.peek());
            assert_eq!(actual.len(), expected.len());
        }
        assert_eq!(accepted, 257);
        assert!(reinsertions > accepted);
        assert_eq!(actual.pop(), None);
    }

    #[test]
    fn frequency_cells_count_cover_and_detect_overflow() {
        let mut frequencies = TrainingFrequencies::new().unwrap();
        frequencies.count(b"xxxxxxxx").unwrap();
        assert_eq!(frequencies.score(b"xxxx"), 5);
        frequencies.cover(b"xxxx");
        assert_eq!(frequencies.score(b"xxxxxxxx"), 0);
        let index = TrainingFrequencies::<Global>::index(b"abcd");
        frequencies.counts[index] = u32::MAX;
        assert_eq!(frequencies.count(b"abcd"), Err(Error::ResourceLimit));
        assert_eq!(frequencies.counts[index], u32::MAX);
    }
}
