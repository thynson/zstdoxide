//! Bounded indexed candidates for chain and lazy match searches.
//!
//! This storage owns no source bytes or pointers. Positions are opaque logical
//! `u64` coordinates; the caller still verifies bytes, source regions, retained
//! history and advertised window bounds before accepting any candidate.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

use crate::{Error, Result};

const HASH_BITS: u32 = 16;
const HEAD_COUNT: usize = 1 << HASH_BITS;
const MAX_NODES: usize = 1 << 18;
const MAX_VISITS: usize = 16;

/// Plain indexed values shared by all future access backends. Tag zero is empty;
/// every live insertion receives a unique positive tag until a complete reset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Link {
    slot: usize,
    tag: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Node {
    position: u64,
    tag: u64,
    older: Link,
}

/// Owned fixed-size heads and initialized cyclic node storage.
///
/// Corresponds to the hash/chain-table roles in `ZSTD_HcFindBestMatch`; the
/// bounded search and storage policies are independent Rust implementation
/// choices. Newest means insertion order, not numeric source-position order.
/// Each vector retains its allocation owner; no pointer survives a mutation.
/// Moving the chain moves both independent allocator values with their storage.
#[derive(Debug)]
pub(crate) struct MatchChain<A: Allocator = Global> {
    heads: Vec<Link, A>,
    nodes: Vec<Node, A>,
    next_slot: usize,
    last_tag: u64,
}

impl MatchChain<Global> {
    /// Reserve at most 262,144 nodes, independently of total frame length.
    /// Dictionary seed storage may use a 65,536-node limit and be copied into
    /// a larger working chain. Both initialized allocations are fallible.
    pub(crate) fn new(node_limit: usize) -> Result<Self> {
        Self::new_in(node_limit, Global)
    }
}

impl<A: Allocator> MatchChain<A> {
    /// Create two independent typed owners. The heads allocate through a fresh
    /// clone, and nodes retain the supplied allocator. Clones need not share an
    /// allocation domain: each Vec keeps the exact value owning its storage.
    pub(crate) fn new_in(node_limit: usize, allocator: A) -> Result<Self>
    where
        A: Clone,
    {
        if !(1..=MAX_NODES).contains(&node_limit) {
            return Err(Error::InvalidParameter);
        }
        let mut heads = Vec::new_in(allocator.clone());
        heads
            .try_reserve_exact(HEAD_COUNT)
            .map_err(|_| Error::AllocationFailed)?;
        while heads.len() < HEAD_COUNT {
            heads.push(Link::default());
        }
        let mut nodes = Vec::new_in(allocator);
        nodes
            .try_reserve_exact(node_limit)
            .map_err(|_| Error::AllocationFailed)?;
        while nodes.len() < node_limit {
            nodes.push(Node::default());
        }
        Ok(Self {
            heads,
            nodes,
            next_slot: 0,
            last_tag: 0,
        })
    }

    /// Discard every link and tag while retaining both owned allocations.
    /// The exclusive borrow rules out a surviving candidate iterator, so fresh
    /// tag reuse cannot make a pre-reset link accessible through this API.
    pub(crate) fn reset(&mut self) {
        self.heads.fill(Link::default());
        self.nodes.fill(Node::default());
        self.next_slot = 0;
        self.last_tag = 0;
    }

    /// Copy a bounded seed into independent initialized working storage.
    ///
    /// A larger destination first uses the slots beyond the seed's allocation;
    /// equal sizes preserve the seed's cyclic replacement point. Copying partial
    /// or wrapped seeds preserves all index/tag pairs exactly. An incompatible
    /// size is rejected before changing the destination; this call allocates
    /// nothing and stores no reference to the seed.
    pub(crate) fn reset_from<B: Allocator>(&mut self, seed: &MatchChain<B>) -> Result<()> {
        if seed.nodes.len() > self.nodes.len() {
            return Err(Error::InvalidParameter);
        }
        self.heads.copy_from_slice(&seed.heads);
        self.nodes[..seed.nodes.len()].copy_from_slice(&seed.nodes);
        self.nodes[seed.nodes.len()..].fill(Node::default());
        self.next_slot = if seed.nodes.len() == self.nodes.len() {
            seed.next_slot
        } else {
            seed.nodes.len()
        };
        self.last_tag = seed.last_tag;
        Ok(())
    }

    /// Record one key/coordinate pair without allocating.
    ///
    /// Positions may be sparse, duplicated, or unordered; lookup filters out
    /// positions at or beyond its explicit upper bound. Replacing a cyclic slot
    /// invalidates old links through their mismatched tags. Tag exhaustion is
    /// rejected before mutation and requires a complete reset before insertion
    /// can resume. No source coordinate is converted into a physical index.
    pub(crate) fn insert(&mut self, word: [u8; 4], position: u64) -> Result<()> {
        let tag = self.last_tag.checked_add(1).ok_or(Error::ResourceLimit)?;
        let bucket = hash(word);
        let older = self.head(bucket)?;
        let slot = self.next_slot;
        if slot >= self.nodes.len() {
            return Err(Error::CorruptData);
        }
        let next_slot = if slot + 1 == self.nodes.len() {
            0
        } else {
            slot + 1
        };
        // Both indices have already been checked, and the exclusive borrow
        // prevents either initialized vector length from changing between these
        // writes. Their checked accessors therefore cannot fail after mutation.
        self.set_node(
            slot,
            Node {
                position,
                tag,
                older,
            },
        )?;
        self.set_head(bucket, Link { slot, tag })?;
        self.last_tag = tag;
        self.next_slot = next_slot;
        Ok(())
    }

    /// Borrow a newest-first candidate traversal with an explicit visit budget.
    ///
    /// The budget is at most sixteen (eight for the planned chain strategy).
    /// Every examined link consumes one visit, including a future coordinate or
    /// stale node. Stale, out-of-range, or non-descending tags terminate safely;
    /// they do not identify the newer occupant of a recycled slot. Hash equality
    /// only selects candidates and does not establish equality of source bytes.
    pub(crate) fn candidates(
        &self,
        word: [u8; 4],
        before: u64,
        visit_budget: usize,
    ) -> Result<Candidates<'_, A>> {
        if visit_budget > MAX_VISITS {
            return Err(Error::InvalidParameter);
        }
        let next = self.head(hash(word))?;
        Ok(Candidates {
            chain: self,
            next,
            before,
            remaining: visit_budget,
            visited: 0,
        })
    }

    /// Heap capacity in bytes, excluding this value's inline vector handles.
    pub(crate) fn retained_capacity(&self) -> usize {
        self.heads.capacity() * core::mem::size_of::<Link>()
            + self.nodes.capacity() * core::mem::size_of::<Node>()
    }

    fn head(&self, index: usize) -> Result<Link> {
        if index >= self.heads.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-match-chain"))]
        let head = self.heads[index];
        #[cfg(feature = "unsafe-match-chain")]
        // SAFETY: The runtime check proves index is in the initialized prefix
        // of the owned Vec<Link>, not merely within its capacity. Its allocation
        // is live and properly aligned. The shared self borrow prevents mutation
        // or reallocation for the access, so get_unchecked returns a valid shared
        // reference with no mutable alias. Link is an initialized Copy value;
        // the dereference copies it without moving ownership. No reference or
        // pointer escapes the block, and the link's tag/index is still validated
        // before a candidate node can be accepted.
        let head = unsafe { *self.heads.get_unchecked(index) };
        Ok(head)
    }

    fn node(&self, index: usize) -> Result<Node> {
        if index >= self.nodes.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-match-chain"))]
        let node = self.nodes[index];
        #[cfg(feature = "unsafe-match-chain")]
        // SAFETY: The checked index lies within the fully initialized owned
        // Vec<Node>. Construction, reset and replacement keep every element a
        // valid aligned Node. The shared self borrow keeps storage alive and
        // excludes mutation/reallocation; the candidate iterator holds that
        // borrow throughout traversal. get_unchecked therefore returns a valid
        // shared reference, and dereferencing copies the plain initialized Node
        // without ownership transfer or escaping references. Tag equality and
        // descending-link validation follow identically in both backends before
        // its source coordinate is returned; neither is assumed by this read.
        let node = unsafe { *self.nodes.get_unchecked(index) };
        Ok(node)
    }

    fn set_head(&mut self, index: usize, head: Link) -> Result<()> {
        if index >= self.heads.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-match-chain"))]
        {
            self.heads[index] = head;
        }
        #[cfg(feature = "unsafe-match-chain")]
        // SAFETY: The runtime bound places index within the initialized owned
        // Vec<Link>, whose elements and allocation are valid and aligned. The
        // exclusive self borrow provides unique access, with no iterator or
        // other element reference alive. get_unchecked_mut creates only this
        // temporary mutable reference; assignment replaces one initialized Copy
        // record with another, without allocation, drop effects or overlapping
        // access. No pointer/reference escapes, and vector length is unchanged.
        unsafe {
            *self.heads.get_unchecked_mut(index) = head;
        }
        Ok(())
    }

    fn set_node(&mut self, index: usize, node: Node) -> Result<()> {
        if index >= self.nodes.len() {
            return Err(Error::CorruptData);
        }
        #[cfg(not(feature = "unsafe-match-chain"))]
        {
            self.nodes[index] = node;
        }
        #[cfg(feature = "unsafe-match-chain")]
        // SAFETY: The explicit runtime guard proves index addresses an existing
        // initialized Node in the live, aligned owned vector. The exclusive self
        // borrow excludes candidate iterators, mutable aliases, and concurrent
        // reallocation. get_unchecked_mut's reference is confined to this block;
        // assignment replaces exactly one valid Copy record with the supplied
        // initialized value. It changes neither allocation nor length and has
        // no ownership/drop effects. No other head/node reference overlaps it.
        unsafe {
            *self.nodes.get_unchecked_mut(index) = node;
        }
        Ok(())
    }
}

fn hash(word: [u8; 4]) -> usize {
    let mixed = u32::from_le_bytes(word).wrapping_mul(0x9e37_79b1);
    (mixed >> (32 - HASH_BITS)) as usize
}

/// A non-allocating shared traversal. Its borrow prevents insertion, reset, or
/// slot replacement until traversal ends. This cursor owns only value fields.
pub(crate) struct Candidates<'a, A: Allocator = Global> {
    chain: &'a MatchChain<A>,
    next: Link,
    before: u64,
    remaining: usize,
    visited: usize,
}

impl<A: Allocator> Candidates<'_, A> {
    #[cfg(test)]
    pub(crate) fn visited(&self) -> usize {
        self.visited
    }
}

impl<A: Allocator> Iterator for Candidates<'_, A> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        while self.remaining != 0 && self.next.tag != 0 {
            let link = self.next;
            // Clearing first makes every failed validation terminate future
            // next() calls too, including malformed links introduced by tests.
            self.next = Link::default();
            self.remaining -= 1;
            self.visited += 1;
            if link.tag > self.chain.last_tag {
                return None;
            }
            let node = self.chain.node(link.slot).ok()?;
            if node.tag != link.tag || (node.older.tag != 0 && node.older.tag >= node.tag) {
                return None;
            }
            // Every nonempty successor has a strictly smaller positive tag.
            // A cycle therefore fails before revisiting a returned node, even
            // independently of the explicit finite visit budget.
            self.next = node.older;
            if node.position < self.before {
                return Some(node.position);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn collect<A: Allocator>(chain: &MatchChain<A>, key: [u8; 4], before: u64) -> Vec<u64> {
        chain.candidates(key, before, MAX_VISITS).unwrap().collect()
    }

    #[test]
    fn construction_bounds_allocation_and_accepts_the_documented_maximum() {
        for limit in [0, MAX_NODES + 1, usize::MAX] {
            assert_eq!(MatchChain::new(limit).unwrap_err(), Error::InvalidParameter);
        }
        let chain = MatchChain::new(MAX_NODES).unwrap();
        assert_eq!(chain.heads.len(), HEAD_COUNT);
        assert_eq!(chain.nodes.len(), MAX_NODES);
        assert!(
            chain.retained_capacity()
                >= HEAD_COUNT * core::mem::size_of::<Link>()
                    + MAX_NODES * core::mem::size_of::<Node>()
        );
    }

    #[test]
    fn checked_accessors_cover_extreme_indices_and_reject_invalid_writes() {
        let mut chain = MatchChain::new(2).unwrap();
        let head = Link {
            slot: 1,
            tag: u64::MAX,
        };
        let node = Node {
            position: u64::MAX,
            tag: u64::MAX,
            older: Link::default(),
        };
        for index in [0, HEAD_COUNT - 1] {
            chain.set_head(index, head).unwrap();
            assert_eq!(chain.head(index), Ok(head));
        }
        for index in [0, 1] {
            chain.set_node(index, node).unwrap();
            assert_eq!(chain.node(index), Ok(node));
        }
        for index in [HEAD_COUNT, usize::MAX] {
            assert_eq!(chain.head(index), Err(Error::CorruptData));
            assert_eq!(
                chain.set_head(index, Link::default()),
                Err(Error::CorruptData)
            );
        }
        for index in [2, usize::MAX] {
            assert_eq!(chain.node(index), Err(Error::CorruptData));
            assert_eq!(
                chain.set_node(index, Node::default()),
                Err(Error::CorruptData)
            );
        }
        assert_eq!(chain.head(0), Ok(head));
        assert_eq!(chain.head(HEAD_COUNT - 1), Ok(head));
        assert!(chain.nodes.iter().all(|&value| value == node));
    }

    #[test]
    fn deliberate_hash_collisions_remain_newest_first_unverified_candidates() {
        let first = 0_u32.to_le_bytes();
        let second = 0x0e8b_2f51_u32.to_le_bytes();
        assert_ne!(first, second);
        assert_eq!(hash(first), hash(second));
        let mut chain = MatchChain::new(8).unwrap();
        chain.insert(first, 2).unwrap();
        chain.insert(second, 7).unwrap();
        chain.insert(first, 5).unwrap();
        assert_eq!(collect(&chain, first, 100), [5, 7, 2]);
        assert_eq!(collect(&chain, second, 100), [5, 7, 2]);
        assert_eq!(collect(&chain, first, 6), [5, 2]);
    }

    #[test]
    fn future_positions_consume_visits_instead_of_bypassing_the_budget() {
        let mut chain = MatchChain::new(32).unwrap();
        for position in 0..32 {
            chain.insert(*b"same", position).unwrap();
        }
        for budget in [0, 1, 8, 16] {
            let mut candidates = chain.candidates(*b"same", 24, budget).unwrap();
            let actual: Vec<_> = candidates.by_ref().collect();
            let expected: Vec<_> = (0..32).rev().take(budget).filter(|&p| p < 24).collect();
            assert_eq!(actual, expected);
            assert_eq!(candidates.visited(), budget);
            assert_eq!(candidates.next(), None);
            assert_eq!(candidates.visited(), budget);
        }
        for budget in [17, usize::MAX] {
            assert!(matches!(
                chain.candidates(*b"same", 24, budget),
                Err(Error::InvalidParameter)
            ));
        }
    }

    #[test]
    fn cyclic_slot_reuse_rejects_stale_heads_and_older_links() {
        let mut chain = MatchChain::new(3).unwrap();
        for position in 0..8 {
            chain.insert(*b"same", position).unwrap();
        }
        assert_eq!(collect(&chain, *b"same", 100), [7, 6, 5]);
        let mut chain = MatchChain::new(2).unwrap();
        assert_ne!(hash(*b"aaaa"), hash(*b"bbbb"));
        chain.insert(*b"aaaa", 0).unwrap();
        chain.insert(*b"bbbb", 1).unwrap();
        chain.insert(*b"bbbb", 2).unwrap();
        let mut stale = chain.candidates(*b"aaaa", 3, 16).unwrap();
        assert_eq!(stale.next(), None);
        assert_eq!(stale.visited(), 1);
        assert_eq!(stale.next(), None);
        assert_eq!(collect(&chain, *b"bbbb", 3), [2, 1]);
    }

    #[test]
    fn invalid_indices_tags_and_cycles_terminate_without_returning_reused_nodes() {
        let mut chain = MatchChain::new(3).unwrap();
        let bucket = hash(*b"same");
        for malformed in [
            Link {
                slot: usize::MAX,
                tag: 1,
            },
            Link { slot: 0, tag: 2 },
        ] {
            chain.reset();
            chain.insert(*b"same", 10).unwrap();
            chain.heads[bucket] = malformed;
            let mut cursor = chain.candidates(*b"same", 100, 16).unwrap();
            assert_eq!(cursor.next(), None);
            assert_eq!(cursor.next(), None);
            assert_eq!(cursor.visited(), 1);
        }
        chain.reset();
        chain.insert(*b"same", 10).unwrap();
        chain.nodes[0].older = chain.heads[bucket];
        assert!(collect(&chain, *b"same", 100).is_empty());
        chain.reset();
        chain.insert(*b"same", 10).unwrap();
        chain.insert(*b"same", 20).unwrap();
        chain.nodes[0].older = chain.heads[bucket];
        assert_eq!(collect(&chain, *b"same", 100), [20]);
        chain.nodes[1].tag = 0;
        assert!(collect(&chain, *b"same", 100).is_empty());
    }

    #[test]
    fn reset_clears_every_link_and_tag_and_retains_both_allocations() {
        let mut chain = MatchChain::new(5).unwrap();
        for position in 0..10 {
            chain.insert(*b"same", position).unwrap();
        }
        let pointers = (chain.heads.as_ptr(), chain.nodes.as_ptr());
        let capacity = chain.retained_capacity();
        chain.reset();
        assert_eq!((chain.heads.as_ptr(), chain.nodes.as_ptr()), pointers);
        assert_eq!(chain.retained_capacity(), capacity);
        assert!(chain.heads.iter().all(|&head| head == Link::default()));
        assert!(chain.nodes.iter().all(|&node| node == Node::default()));
        assert!(collect(&chain, *b"same", 100).is_empty());
        chain.insert(*b"same", 0).unwrap();
        assert_eq!(chain.last_tag, 1);
        assert_eq!(collect(&chain, *b"same", 1), [0]);
    }

    #[test]
    fn copying_partial_seeds_preserves_independence_and_reuses_working_storage() {
        let mut seed = MatchChain::new(4).unwrap();
        seed.insert(*b"same", 100).unwrap();
        seed.insert(*b"same", 101).unwrap();
        let mut working = MatchChain::new(8).unwrap();
        working.insert(*b"same", 1).unwrap();
        let capacity = working.retained_capacity();
        working.reset_from(&seed).unwrap();
        assert_eq!(working.next_slot, 4);
        working.insert(*b"same", 102).unwrap();
        assert_eq!(collect(&working, *b"same", 103), [102, 101, 100]);
        assert_eq!(collect(&seed, *b"same", 103), [101, 100]);
        working.reset_from(&seed).unwrap();
        assert_eq!(collect(&working, *b"same", 103), [101, 100]);
        assert_eq!(working.retained_capacity(), capacity);
        seed.reset();
        working.reset_from(&seed).unwrap();
        assert!(collect(&working, *b"same", 103).is_empty());
    }

    #[test]
    fn copying_wrapped_seeds_preserves_tags_and_both_replacement_policies() {
        let mut seed = MatchChain::new(3).unwrap();
        for position in 0..9 {
            seed.insert(*b"same", position).unwrap();
        }
        let mut larger = MatchChain::new(8).unwrap();
        larger.reset_from(&seed).unwrap();
        larger.insert(*b"same", 9).unwrap();
        larger.insert(*b"same", 10).unwrap();
        assert_eq!(collect(&larger, *b"same", 11), [10, 9, 8, 7, 6]);
        let mut equal = MatchChain::new(3).unwrap();
        equal.reset_from(&seed).unwrap();
        equal.insert(*b"same", 9).unwrap();
        assert_eq!(collect(&equal, *b"same", 10), [9, 8, 7]);
        assert_eq!(collect(&seed, *b"same", 10), [8, 7, 6]);
        let mut smaller = MatchChain::new(2).unwrap();
        smaller.insert(*b"same", 99).unwrap();
        assert_eq!(smaller.reset_from(&seed), Err(Error::InvalidParameter));
        assert_eq!(collect(&smaller, *b"same", 100), [99]);
    }

    #[test]
    fn full_width_coordinates_are_never_truncated_into_storage_indices() {
        let mut chain = MatchChain::new(4).unwrap();
        let positions = [u64::MAX, usize::MAX as u64, u64::MAX - 1, 0];
        for position in positions {
            chain.insert(*b"same", position).unwrap();
        }
        let expected: Vec<_> = positions
            .into_iter()
            .rev()
            .filter(|&p| p < u64::MAX)
            .collect();
        assert_eq!(collect(&chain, *b"same", u64::MAX), expected);
        assert!(collect(&chain, *b"same", 0).is_empty());
        assert_eq!(collect(&chain, *b"same", 1), [0]);
    }

    #[test]
    fn tag_exhaustion_fails_before_mutation_and_reset_permits_fresh_tags() {
        let mut chain = MatchChain::new(1).unwrap();
        chain.last_tag = u64::MAX - 1;
        chain.insert(*b"same", 7).unwrap();
        assert_eq!(collect(&chain, *b"same", 8), [7]);
        let before = (chain.heads[hash(*b"same")], chain.nodes[0], chain.next_slot);
        assert_eq!(chain.insert(*b"same", 8), Err(Error::ResourceLimit));
        assert_eq!(
            (chain.heads[hash(*b"same")], chain.nodes[0], chain.next_slot),
            before
        );
        assert_eq!(chain.last_tag, u64::MAX);
        chain.reset();
        chain.insert(*b"same", u64::MAX - 1).unwrap();
        assert_eq!(chain.last_tag, 1);
        assert_eq!(collect(&chain, *b"same", u64::MAX), vec![u64::MAX - 1]);
    }
    #[test]
    fn custom_setup_failures_drop_each_partial_owner_and_validation_allocates_nothing() {
        use core::num::NonZeroU64;
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        for ordinal in 1..=2 {
            let allocator = TrackingAllocator::new(AllocatorConfig {
                max_live_allocations: 4,
                fail_at: NonZeroU64::new(ordinal),
            })
            .unwrap();
            let monitor = allocator.monitor();
            assert!(matches!(
                MatchChain::new_in(8, allocator),
                Err(Error::AllocationFailed)
            ));
            let stats = monitor.snapshot();
            assert_eq!(stats.attempts, ordinal);
            assert_eq!(stats.successes, ordinal - 1);
            assert_eq!(stats.deallocate_calls, ordinal - 1);
            assert_eq!(stats.identities_created, 2);
            assert_eq!(stats.owners_dropped, 2);
            monitor.assert_released();
        }
        for limit in [0, MAX_NODES + 1, usize::MAX] {
            let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
            let monitor = allocator.monitor();
            assert!(matches!(
                MatchChain::new_in(limit, allocator),
                Err(Error::InvalidParameter)
            ));
            assert_eq!(monitor.snapshot().attempts, 0);
            assert_eq!(monitor.snapshot().identities_created, 1);
            monitor.assert_released();
        }
    }

    #[test]
    fn custom_distinct_domains_preserve_candidates_and_move_with_both_vectors() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let identity = allocator.identity();
        let monitor = allocator.monitor();
        let mut custom = MatchChain::new_in(8, allocator).unwrap();
        let mut global = MatchChain::new(8).unwrap();
        assert_eq!(custom.nodes.allocator().identity(), identity);
        assert_ne!(custom.heads.allocator().identity(), identity);
        let pointers = (custom.heads.as_ptr(), custom.nodes.as_ptr());
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        for index in 0..64_u64 {
            let key = (index % 3).to_le_bytes()[..4].try_into().unwrap();
            let position = match index % 5 {
                0 => u64::MAX - index,
                1 => u64::from(u32::MAX) + index,
                _ => index,
            };
            custom.insert(key, position).unwrap();
            global.insert(key, position).unwrap();
            for limit in [0, 1, u64::from(u32::MAX), u64::MAX] {
                for budget in [0, 1, 8, 16] {
                    let actual: Vec<_> = custom.candidates(key, limit, budget).unwrap().collect();
                    let expected: Vec<_> = global.candidates(key, limit, budget).unwrap().collect();
                    assert_eq!(actual, expected);
                }
            }
        }
        assert_eq!(monitor.snapshot(), before);
        let mut moved = custom;
        assert_eq!((moved.heads.as_ptr(), moved.nodes.as_ptr()), pointers);
        moved.reset();
        assert_eq!(moved.last_tag, 0);
        assert_eq!(moved.next_slot, 0);
        assert!(collect(&moved, [0; 4], u64::MAX).is_empty());
        assert_eq!(monitor.snapshot().attempts, before.attempts);
        drop(moved);
        monitor.assert_released();
    }

    #[test]
    fn custom_seed_copy_borrows_non_clone_allocator_and_never_adopts_source_storage() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default())
            .unwrap()
            .into_non_clone();
        let monitor = allocator.monitor();
        let mut working = MatchChain::new_in(8, &allocator).unwrap();
        let mut seed = MatchChain::new(3).unwrap();
        let larger = MatchChain::new(9).unwrap();
        for position in [7, 11, 17, 23] {
            seed.insert(*b"seed", position).unwrap();
        }
        let pointers = (working.heads.as_ptr(), working.nodes.as_ptr());
        let before = monitor.snapshot();
        assert!(monitor.fail_next());
        working.reset_from(&seed).unwrap();
        assert_eq!(collect(&working, *b"seed", u64::MAX), [23, 17, 11]);
        working.insert(*b"seed", 29).unwrap();
        seed.insert(*b"seed", 31).unwrap();
        let expected = collect(&working, *b"seed", u64::MAX);
        assert_eq!(expected, [29, 23, 17, 11]);
        assert_eq!(working.reset_from(&larger), Err(Error::InvalidParameter));
        assert_eq!(collect(&working, *b"seed", u64::MAX), expected);
        assert_eq!((working.heads.as_ptr(), working.nodes.as_ptr()), pointers);
        assert_eq!(working.heads.allocator().identity(), allocator.identity());
        assert_eq!(working.nodes.allocator().identity(), allocator.identity());
        working.reset_from(&seed).unwrap();
        assert_eq!(collect(&working, *b"seed", u64::MAX), [31, 23, 17]);
        assert_eq!(monitor.snapshot(), before);
        assert!(matches!(
            MatchChain::new_in(1, &allocator),
            Err(Error::AllocationFailed)
        ));
        drop(working);
        drop(allocator);
        monitor.assert_released();
    }

    #[test]
    fn custom_chain_swaps_keep_all_four_allocator_owners_paired() {
        use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};
        let allocator = TrackingAllocator::new(AllocatorConfig::default()).unwrap();
        let monitor = allocator.monitor();
        let mut first = MatchChain::new_in(3, allocator.clone()).unwrap();
        let mut second = MatchChain::new_in(5, allocator.clone()).unwrap();
        let first_ids = (
            first.heads.allocator().identity(),
            first.nodes.allocator().identity(),
        );
        let second_ids = (
            second.heads.allocator().identity(),
            second.nodes.allocator().identity(),
        );
        assert_ne!(first_ids, second_ids);
        first.insert(*b"swap", 7).unwrap();
        second.insert(*b"swap", 11).unwrap();
        drop(allocator);
        let before = monitor.snapshot();
        core::mem::swap(&mut first, &mut second);
        assert_eq!(
            (
                first.heads.allocator().identity(),
                first.nodes.allocator().identity()
            ),
            second_ids
        );
        assert_eq!(
            (
                second.heads.allocator().identity(),
                second.nodes.allocator().identity()
            ),
            first_ids
        );
        assert_eq!(collect(&first, *b"swap", u64::MAX), [11]);
        assert_eq!(collect(&second, *b"swap", u64::MAX), [7]);
        assert_eq!(monitor.snapshot(), before);
        drop(first);
        assert_eq!(monitor.snapshot().live_blocks, 2);
        drop(second);
        monitor.assert_released();
    }
}
