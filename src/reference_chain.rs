//! Independent initialized owners for reference Greedy's cyclic hash chain.
//!
//! Unlike the named MatchChain, slots belong to source positions (with the C
//! bias of two), not insertion generations. Links are indices into borrowed
//! source; this leaf never reads source bytes or treats a collision as a match.
#![cfg_attr(not(feature = "unsafe-reference-chain"), forbid(unsafe_code))]

use alloc::{alloc::Global, vec::Vec};
use core::alloc::{Allocator, Layout};

use crate::greedy_key::{GreedyKey, GreedyKeyLayout};
use crate::{Error, Result};

const EMPTY: usize = usize::MAX;

/// Complete independent head/link owners with exact allocator values. All
/// cells are initialized indices or EMPTY; logical source position zero is valid.
/// Corresponds to `hashTable` and `chainTable` in `ZSTD_HcFindBestMatch`.
pub(crate) struct ReferenceChain<H: Allocator = Global, L: Allocator = H> {
    heads: Vec<usize, H>,
    links: Vec<usize, L>,
    key: GreedyKeyLayout,
    chain_log: u8,
}

impl<H: Allocator, L: Allocator> ReferenceChain<H, L> {
    /// Check both layouts and their sum before requesting either allocation.
    /// Failure releases every allocated vector through its exact allocator;
    /// no cloning, allocator equivalence, or manually paired deallocation is used.
    pub(crate) fn new_in(key: GreedyKeyLayout, chain_log: u8, heads: H, links: L) -> Result<Self> {
        if !(6..=18).contains(&chain_log) {
            return Err(Error::InvalidParameter);
        }
        let link_count = 1_usize
            .checked_shl(u32::from(chain_log))
            .ok_or(Error::ResourceLimit)?;
        let head_layout = Layout::array::<usize>(key.cells()).map_err(|_| Error::ResourceLimit)?;
        let link_layout = Layout::array::<usize>(link_count).map_err(|_| Error::ResourceLimit)?;
        head_layout
            .size()
            .checked_add(link_layout.size())
            .ok_or(Error::ResourceLimit)?;
        fn initialized<A: Allocator>(count: usize, owner: A) -> Result<Vec<usize, A>> {
            let mut values = Vec::new_in(owner);
            values
                .try_reserve_exact(count)
                .map_err(|_| Error::AllocationFailed)?;
            // Each push fits the completed reservation; no clone or second
            // allocation is needed to initialize the sentinel values.
            while values.len() < count {
                values.push(EMPTY);
            }
            Ok(values)
        }
        let heads = initialized(key.cells(), heads)?;
        let links = initialized(link_count, links)?;
        Ok(Self {
            heads,
            links,
            key,
            chain_log,
        })
    }

    pub(crate) const fn key_layout(&self) -> GreedyKeyLayout {
        self.key
    }
    pub(crate) const fn chain_log(&self) -> u8 {
        self.chain_log
    }
    #[cfg(test)]
    pub(crate) fn retained_capacity(&self) -> usize {
        (self.heads.capacity() + self.links.capacity()) * core::mem::size_of::<usize>()
    }
    pub(crate) fn reset(&mut self) {
        self.heads.fill(EMPTY);
        self.links.fill(EMPTY);
    }

    fn chain_size(&self) -> usize {
        1 << self.chain_log
    }
    fn slot(&self, position: usize) -> usize {
        let mask = self.chain_size() - 1;
        // (logical + ZSTD_WINDOW_START_INDEX) modulo 2^chain_log. Masking
        // before addition preserves near-usize-limit indices without overflow.
        ((position & mask) + 2) & mask
    }

    fn head(&self, key: GreedyKey) -> Result<usize> {
        let bucket = self.key.bucket(key)?;
        if bucket >= self.heads.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-reference-chain"))]
        let value = self.heads[bucket];
        #[cfg(feature = "unsafe-reference-chain")]
        let value = {
            // SAFETY: The runtime check proves bucket is below initialized
            // length, not capacity. Vec owns valid, aligned, initialized usize
            // cells through its exact allocator. The shared self borrow excludes
            // mutation/reallocation for this access. The reference is immediately
            // copied and cannot escape or outlive the owner borrow.
            unsafe { *self.heads.get_unchecked(bucket) }
        };
        Ok(value)
    }

    fn predecessor(&self, position: usize) -> Result<usize> {
        let slot = self.slot(position);
        if slot >= self.links.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-reference-chain"))]
        let value = self.links[slot];
        #[cfg(feature = "unsafe-reference-chain")]
        let value = {
            // SAFETY: slot < links.len() is checked at runtime independently of
            // the position or stored links. Vec's exact allocator owns aligned,
            // valid initialized usize cells. Shared self keeps storage live and
            // excludes writes/reallocation; only a copied value escapes this
            // temporary reference. No source pointer is formed from that value.
            unsafe { *self.links.get_unchecked(slot) }
        };
        Ok(value)
    }

    /// Append one sampled source index, preserving the replaced head in the
    /// cyclic slot. Corresponds to one `ZSTD_insertAndFindFirstIndex_internal`
    /// update. Callers insert in increasing source order and prove key bytes.
    /// Reject an incompatible key, reserved sentinel, truncated initialized
    /// owner, or non-earlier predecessor before changing either table.
    pub(crate) fn insert(&mut self, key: GreedyKey, position: usize) -> Result<()> {
        if position == EMPTY {
            return Err(Error::InvalidParameter);
        }
        let bucket = self.key.bucket(key)?;
        let slot = self.slot(position);
        if bucket >= self.heads.len() || slot >= self.links.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-reference-chain"))]
        let previous = self.heads[bucket];
        #[cfg(feature = "unsafe-reference-chain")]
        let previous = {
            // SAFETY: Both live bounds were checked above before any access.
            // heads owns initialized, aligned usize storage, retained by self.
            // The exclusive self borrow excludes mutation or reallocation by
            // any other access. This read's temporary shared reference is copied
            // and ends before either table is mutated.
            unsafe { *self.heads.get_unchecked(bucket) }
        };
        if previous != EMPTY && previous >= position {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-reference-chain"))]
        {
            self.links[slot] = previous;
            self.heads[bucket] = position;
        }
        #[cfg(feature = "unsafe-reference-chain")]
        {
            // SAFETY: bucket < heads.len() and slot < links.len() were checked
            // before the read and remain true: no allocation, resizing or
            // callbacks intervened. The two Vec owners retain separate valid,
            // aligned allocations of initialized usize cells. Exclusive self
            // excludes aliases/concurrent accesses. Each mutable reference ends
            // at its assignment; none escapes. Copy writes preserve initialization
            // without drop, ownership transfer, or calling an allocator.
            unsafe {
                *self.links.get_unchecked_mut(slot) = previous;
                *self.heads.get_unchecked_mut(bucket) = position;
            }
        }
        Ok(())
    }

    /// Start at the hash head, with a separately checked per-probe source floor.
    /// A valid head may predate the cyclic capacity. Only following links is
    /// stopped at that capacity, after yielding the boundary candidate itself.
    /// The shared borrow prevents insertion/reset from changing this traversal.
    pub(crate) fn candidates(
        &self,
        key: GreedyKey,
        current: usize,
        floor: usize,
        attempts: usize,
    ) -> Result<ChainCandidates<'_, H, L>> {
        if floor > current || !(1..=256).contains(&attempts) {
            return Err(Error::InvalidParameter);
        }
        Ok(ChainCandidates {
            chain: self,
            head: self.head(key)?,
            previous: None,
            current,
            floor,
            remaining: attempts,
            done: false,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_maps(&self) -> (&[usize], &[usize]) {
        (&self.heads, &self.links)
    }
}

/// A bounded traversal holding one table borrow and copied source indices.
/// It yields collisions only. The parser must independently check each candidate
/// against the source extent before comparing bytes. No source reference escapes.
pub(crate) struct ChainCandidates<'a, H: Allocator, L: Allocator> {
    chain: &'a ReferenceChain<H, L>,
    head: usize,
    previous: Option<usize>,
    current: usize,
    floor: usize,
    remaining: usize,
    done: bool,
}

impl<H: Allocator, L: Allocator> Iterator for ChainCandidates<'_, H, L> {
    type Item = Result<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.remaining == 0 {
            return None;
        }
        // Set done before any fallible access so corruption emits one error and
        // cannot create an unbounded error stream. Only a valid yield reopens it.
        self.done = true;
        let candidate = if let Some(previous) = self.previous {
            // A previous yield established previous < current. This distance
            // test exactly translates C's raw-index minChain cutoff, including
            // logical zero when current is still smaller than the ring size.
            if self.current - previous >= self.chain.chain_size() {
                return None;
            }
            match self.chain.predecessor(previous) {
                Ok(EMPTY) => return None,
                Ok(next) if next >= previous => return Some(Err(Error::CorruptData)),
                Ok(next) => next,
                Err(error) => return Some(Err(error)),
            }
        } else {
            self.head
        };
        if candidate == EMPTY {
            return None;
        }
        if candidate >= self.current {
            return Some(Err(Error::CorruptData));
        }
        if candidate < self.floor {
            return None;
        }
        self.remaining -= 1;
        self.previous = Some(candidate);
        self.done = false;
        Some(Ok(candidate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    fn layout() -> GreedyKeyLayout {
        GreedyKeyLayout::new(6, 4).unwrap()
    }
    fn key() -> GreedyKey {
        layout().read_key(b"abcd").unwrap()
    }
    fn empty() -> ReferenceChain {
        ReferenceChain::new_in(layout(), 6, Global, Global).unwrap()
    }
    fn collect(
        chain: &ReferenceChain,
        current: usize,
        floor: usize,
        attempts: usize,
    ) -> Result<Vec<usize>> {
        chain.candidates(key(), current, floor, attempts)?.collect()
    }

    #[test]
    fn biased_slots_and_complete_maps_preserve_zero_and_ring_wrap() {
        let mut chain = empty();
        let bucket = layout().bucket(key()).unwrap();
        for position in 0..=64 {
            chain.insert(key(), position).unwrap();
        }
        let (heads, links) = chain.test_maps();
        assert_eq!(heads.len(), 64);
        assert_eq!(links.len(), 64);
        assert_eq!(heads[bucket], 64);
        assert!(
            heads
                .iter()
                .enumerate()
                .all(|(i, &p)| i == bucket || p == EMPTY)
        );
        // Position 64 overwrites position zero's biased slot 2. Every other
        // ring cell contains the predecessor of its latest absolute position.
        for position in 1..=64 {
            assert_eq!(links[(position + 2) % 64], position - 1);
        }
        assert_eq!(
            collect(&chain, 70, 0, 256).unwrap(),
            (6..=64).rev().collect::<Vec<_>>()
        );
        assert_eq!(collect(&chain, 70, 61, 256).unwrap(), [64, 63, 62, 61]);
        assert_eq!(collect(&chain, 70, 0, 2).unwrap(), [64, 63]);
        assert!(collect(&chain, 70, 65, 256).unwrap().is_empty());
        chain.reset();
        assert!(
            chain
                .test_maps()
                .0
                .iter()
                .chain(chain.test_maps().1)
                .all(|&p| p == EMPTY)
        );
        chain.insert(key(), 0).unwrap();
        assert_eq!(collect(&chain, 1, 0, 256).unwrap(), [0]);
        assert_eq!(collect(&chain, 2 * 1024 * 1024, 0, 256).unwrap(), [0]);
        assert_eq!(chain.key_layout(), layout());
        assert_eq!(chain.chain_log(), 6);
    }

    #[test]
    fn corrupt_heads_links_and_short_owners_fail_before_following_indices() {
        let mut chain = empty();
        let bucket = layout().bucket(key()).unwrap();
        chain.insert(key(), 7).unwrap();
        for bad in [7, 8, 99, usize::MAX - 1] {
            let slot = chain.slot(7);
            chain.links[slot] = bad;
            let mut candidates = chain.candidates(key(), 10, 0, 256).unwrap();
            assert_eq!(candidates.next(), Some(Ok(7)));
            assert_eq!(candidates.next(), Some(Err(Error::CorruptData)));
            assert_eq!(candidates.next(), None);
        }
        for bad in [10, usize::MAX - 1] {
            chain.heads[bucket] = bad;
            assert_eq!(collect(&chain, 10, 0, 256), Err(Error::CorruptData));
        }
        chain.heads[bucket] = 7;
        chain.links.clear();
        assert_eq!(collect(&chain, 10, 0, 256), Err(Error::CorruptData));
        let before = chain.heads.clone();
        assert_eq!(chain.insert(key(), 8), Err(Error::CorruptData));
        assert_eq!(chain.heads, before);
        chain.heads.clear();
        assert!(matches!(
            chain.candidates(key(), 10, 0, 2),
            Err(Error::CorruptData)
        ));
    }

    #[test]
    fn invalid_updates_preserve_both_maps_and_large_slots_do_not_overflow() {
        let mut chain = empty();
        chain.insert(key(), 0).unwrap();
        let before = (chain.heads.clone(), chain.links.clone());
        for p in [0, EMPTY] {
            assert!(chain.insert(key(), p).is_err());
        }
        let other = GreedyKeyLayout::new(6, 5)
            .unwrap()
            .read_key(b"abcde")
            .unwrap();
        assert_eq!(chain.insert(other, 1), Err(Error::InvalidParameter));
        for (current, floor, budget) in [(10, 11, 2), (10, 0, 0), (10, 0, 257)] {
            assert!(matches!(
                chain.candidates(key(), current, floor, budget),
                Err(Error::InvalidParameter)
            ));
        }
        assert_eq!(chain.test_maps(), (&before.0[..], &before.1[..]));
        chain.insert(key(), usize::MAX - 1).unwrap();
        assert_eq!(chain.slot(usize::MAX - 1), 0);
        assert_eq!(
            collect(&chain, usize::MAX, 0, 256).unwrap(),
            [usize::MAX - 1, 0]
        );
    }

    #[test]
    fn independent_nonclone_owners_release_on_either_failure_and_invalid_shapes() {
        for reject in 0..=2 {
            let h = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let l = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let hm = h.monitor();
            let lm = l.monitor();
            if reject == 1 {
                assert!(hm.fail_next());
            }
            if reject == 2 {
                assert!(lm.fail_next());
            }
            let result = ReferenceChain::new_in(GreedyKeyLayout::new(19, 6).unwrap(), 6, h, l);
            if reject == 0 {
                let mut chain = result.unwrap();
                let bytes = ((1 << 19) + 64) * core::mem::size_of::<usize>();
                assert_eq!(chain.retained_capacity(), bytes);
                assert_eq!(
                    hm.snapshot().live_requested_bytes + lm.snapshot().live_requested_bytes,
                    bytes as u128
                );
                let counts = (hm.snapshot().attempts, lm.snapshot().attempts);
                chain.reset();
                assert_eq!((hm.snapshot().attempts, lm.snapshot().attempts), counts);
                drop(chain);
            } else {
                assert!(matches!(result, Err(Error::AllocationFailed)));
            }
            assert_eq!(hm.snapshot().attempts, 1);
            assert_eq!(lm.snapshot().attempts, u64::from(reject != 1));
            for monitor in [hm, lm] {
                monitor.assert_released();
                assert_eq!(monitor.snapshot().owners_dropped, 1);
            }
        }
        for log in [0, 5, 19, u8::MAX] {
            let owner = TrackingAllocator::new(AllocatorConfig::default())
                .unwrap()
                .into_non_clone();
            let monitor = owner.monitor();
            assert!(matches!(
                ReferenceChain::new_in(layout(), log, &owner, &owner),
                Err(Error::InvalidParameter)
            ));
            assert_eq!(monitor.snapshot().attempts, 0);
            monitor.assert_released();
        }
    }

    #[test]
    fn sparse_cyclic_traversals_match_an_independent_source_history_model() {
        // The model retains every historical insertion, with no cyclic slots.
        // It filters actual source age only after yielding each candidate, so
        // it distinguishes this structure from an insertion-generation chain.
        let mut chain = empty();
        let mut history = Vec::new();
        for position in (0..385).step_by(7) {
            let bytes = ((position * 37 % 23) as u32).to_le_bytes();
            let key = layout().read_key(&bytes).unwrap();
            let bucket = layout().bucket(key).unwrap();
            history.push((bucket, position));
            chain.insert(key, position).unwrap();
        }
        for value in 0_u32..32 {
            let key = layout().read_key(&value.to_le_bytes()).unwrap();
            let bucket = layout().bucket(key).unwrap();
            for floor in [0, 300, 350, 385] {
                for budget in [1, 2, 7, 256] {
                    let mut expected = Vec::new();
                    for &(b, position) in history.iter().rev() {
                        if b != bucket {
                            continue;
                        }
                        if position < floor || expected.len() == budget {
                            break;
                        }
                        expected.push(position);
                        if 390 - position >= 64 {
                            break;
                        }
                    }
                    let observed: Result<Vec<_>> =
                        chain.candidates(key, 390, floor, budget).unwrap().collect();
                    assert_eq!(observed.unwrap(), expected);
                }
            }
        }
    }

    #[test]
    fn complete_moves_swaps_and_resets_preserve_each_exact_owner() {
        let a = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let b = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let am = a.monitor();
        let bm = b.monitor();
        let mut first = ReferenceChain::new_in(layout(), 18, &a, &b).unwrap();
        let mut second =
            ReferenceChain::new_in(GreedyKeyLayout::new(19, 6).unwrap(), 6, &b, &a).unwrap();
        first.insert(key(), 0).unwrap();
        core::mem::swap(&mut first, &mut second);
        assert_eq!(second.test_maps().0[layout().bucket(key()).unwrap()], 0);
        let before = (am.snapshot(), bm.snapshot());
        first.reset();
        second.reset();
        assert_eq!((am.snapshot(), bm.snapshot()), before);
        drop((first, second));
        am.assert_released();
        bm.assert_released();
        assert_eq!(
            (am.snapshot().owners_dropped, bm.snapshot().owners_dropped),
            (0, 0)
        );
        drop((a, b));
        assert_eq!(
            (am.snapshot().owners_dropped, bm.snapshot().owners_dropped),
            (1, 1)
        );
    }
}
