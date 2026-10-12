//! Concrete resource controls for the existing named match-search policies.
//!
//! These plain values own no memory and share their safe representation across
//! all backends. They do not admit reference compression levels or algorithms.

use core::alloc::Layout;

use crate::{CompressionStrategy, Error, Result};

const MIN_HASH_LOG: u8 = 10;
const MAX_HASH_LOG: u8 = 18;
const MIN_CHAIN_LOG: u8 = 10;
const MAX_CHAIN_LOG: u8 = 19;
const MIN_KEY_BYTES: u8 = 4;
const MAX_KEY_BYTES: u8 = 8;
const MAX_VISITS: u8 = 64;
pub(crate) const MAX_HISTORY_BYTES: u32 = 1 << 20;
const MAX_SEED_NODES: usize = 1 << 16;
const MAX_REQUESTED_STORAGE_BYTES: usize = 32 * 1024 * 1024;

/// Concrete storage and effort settings for the independent named Rust policies.
///
/// Zero is invalid for every field. These controls are separate from reference
/// compression-parameter inspection: they neither select numeric levels nor
/// reproduce C's position-masked chain representation. Greedy always examines
/// one candidate; the Chain and Lazy visit limits apply to their named policies.
///
/// The default preserves 65,536 hash buckets, 262,144 working chain nodes,
/// four-byte keys, eight Chain visits, sixteen visits per Lazy probe, and
/// 128 KiB retained output history. Dictionary chain seeds use at most 65,536
/// nodes. Actual retention is also bounded by the frame's advertised window.
///
/// This record owns no allocation. Setter errors preserve every field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchSearchOptions {
    hash_log: u8,
    chain_log: u8,
    key_bytes: u8,
    chain_visits: u8,
    lazy_visits: u8,
    history_bytes: u32,
}

impl MatchSearchOptions {
    /// Construct the existing named-policy defaults without allocating.
    pub const fn new() -> Self {
        Self {
            hash_log: 16,
            chain_log: 18,
            key_bytes: 4,
            chain_visits: 8,
            lazy_visits: 16,
            history_bytes: 128 * 1024,
        }
    }

    /// Set the working and dictionary-seed hash bucket log, in 10..=18.
    pub fn set_hash_log(&mut self, value: u8) -> Result<()> {
        self.hash_log = checked_byte(value, MIN_HASH_LOG, MAX_HASH_LOG)?;
        Ok(())
    }

    /// Set the Chain/Lazy working node log, in 10..=19.
    /// Seed nodes are the smaller of the working count and 65,536.
    pub fn set_chain_log(&mut self, value: u8) -> Result<()> {
        self.chain_log = checked_byte(value, MIN_CHAIN_LOG, MAX_CHAIN_LOG)?;
        Ok(())
    }

    /// Set the exact initialized search-key width, in 4..=8 bytes.
    /// This is not a minimum emitted-match length: collisions still need
    /// verification and may produce valid matches as short as four bytes.
    pub fn set_key_bytes(&mut self, value: u8) -> Result<()> {
        self.key_bytes = checked_byte(value, MIN_KEY_BYTES, MAX_KEY_BYTES)?;
        Ok(())
    }

    /// Set Chain's examined-link limit per probe, in 1..=64.
    /// Future, stale and rejected candidates consume effort too.
    pub fn set_chain_visits(&mut self, value: u8) -> Result<()> {
        self.chain_visits = checked_byte(value, 1, MAX_VISITS)?;
        Ok(())
    }

    /// Set the examined-link limit for each Lazy probe, in 1..=64.
    /// The same limit applies at the current and next positions.
    pub fn set_lazy_visits(&mut self, value: u8) -> Result<()> {
        self.lazy_visits = checked_byte(value, 1, MAX_VISITS)?;
        Ok(())
    }

    /// Set retained previous output, in 1..=1,048,576 bytes.
    /// The wire window can further reduce it. Earlier bytes within the current
    /// input block and dictionary eligibility have their own matching rules.
    pub fn set_history_bytes(&mut self, value: u32) -> Result<()> {
        if !(1..=MAX_HISTORY_BYTES).contains(&value) {
            return Err(Error::InvalidParameter);
        }
        self.history_bytes = value;
        Ok(())
    }

    /// Configured base-two bucket count.
    pub const fn hash_log(&self) -> u8 {
        self.hash_log
    }

    /// Configured base-two working Chain/Lazy node count.
    pub const fn chain_log(&self) -> u8 {
        self.chain_log
    }

    /// Configured initialized search-key width.
    pub const fn key_bytes(&self) -> u8 {
        self.key_bytes
    }

    /// Configured examined-link limit for Chain.
    pub const fn chain_visits(&self) -> u8 {
        self.chain_visits
    }

    /// Configured examined-link limit for each Lazy probe.
    pub const fn lazy_visits(&self) -> u8 {
        self.lazy_visits
    }

    /// Configured output-history horizon before applying the frame window.
    pub const fn history_bytes(&self) -> u32 {
        self.history_bytes
    }
}

impl Default for MatchSearchOptions {
    fn default() -> Self {
        Self::new()
    }
}

fn checked_byte(value: u8, minimum: u8, maximum: u8) -> Result<u8> {
    if (minimum..=maximum).contains(&value) {
        Ok(value)
    } else {
        Err(Error::InvalidParameter)
    }
}

/// A checked copied descriptor for compatible hash keys and bucket storage.
///
/// The hash layer must additionally check a key's width against this descriptor;
/// leaf accesses must check the computed bucket against their live initialized
/// length. This scalar value alone does not prove storage is allocated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SearchKeyLayout {
    hash_log: u8,
    key_bytes: u8,
    bucket_count: usize,
}

impl SearchKeyLayout {
    pub(crate) fn new(hash_log: u8, key_bytes: u8) -> Result<Self> {
        Self::with_bounds(hash_log, key_bytes, MIN_HASH_LOG, MAX_KEY_BYTES)
    }

    /// Fast's private hash domain includes logs 6..=9. Its dispatch accepts
    /// four-through-seven-byte keys; minMatch three resolves to width four
    /// before this constructor. Named public settings retain their old bounds.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "Fast parser integration follows")
    )]
    pub(crate) fn for_fast(hash_log: u8, key_bytes: u8) -> Result<Self> {
        Self::with_bounds(hash_log, key_bytes, 6, 7)
    }

    /// DoubleFast's independent long table always hashes eight initialized
    /// bytes, including at private logs 6..=9. Its short table uses the existing
    /// four-through-seven-byte layout. Neither domain expands named settings
    /// or Fast's accepted widths. This copied descriptor owns no storage.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "DoubleFast plan and table integration follows")
    )]
    pub(crate) fn for_double_fast_long(hash_log: u8) -> Result<Self> {
        Self::with_bounds(hash_log, 8, 6, 8)
    }

    fn with_bounds(
        hash_log: u8,
        key_bytes: u8,
        minimum_log: u8,
        maximum_width: u8,
    ) -> Result<Self> {
        checked_byte(hash_log, minimum_log, MAX_HASH_LOG)?;
        checked_byte(key_bytes, MIN_KEY_BYTES, maximum_width)?;
        let bucket_count = checked_power_of_two(hash_log)?;
        Ok(Self {
            hash_log,
            key_bytes,
            bucket_count,
        })
    }

    pub(crate) const fn hash_log(&self) -> u8 {
        self.hash_log
    }

    pub(crate) const fn key_bytes(&self) -> u8 {
        self.key_bytes
    }

    pub(crate) const fn bucket_count(&self) -> usize {
        self.bucket_count
    }
}

fn checked_power_of_two(log: u8) -> Result<usize> {
    1_usize
        .checked_shl(u32::from(log))
        .ok_or(Error::ResourceLimit)
}

/// Immutable dimensions and requested payloads before any owner is allocated.
///
/// The all-cache total includes two Greedy vectors, both Chain heads/node pairs,
/// and history, even if the selected policy uses only one family. These are
/// requested initialized bytes, not retained capacity or peak/process memory:
/// vector over-allocation, grow overlap, other codec scratch, dictionary storage
/// and allocator metadata are excluded. Old configurations may retain larger
/// capacities; each individual admitted shape obeys the same hard dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SearchShape {
    layout: SearchKeyLayout,
    chain_nodes: usize,
    seed_nodes: usize,
    visits: usize,
    history_bytes: usize,
    greedy_table_bytes: usize,
    chain_working_bytes: usize,
    chain_seed_bytes: usize,
    all_cached_bytes: usize,
}

impl SearchShape {
    /// Resolve a concrete shape using the actual internal frame window.
    ///
    /// A one-shot empty frame passes zero; its effective history is zero and
    /// integration must not allocate a ring. Other one-shot windows need not be
    /// powers of two. Public streaming windows retain their separate validation.
    /// This routine validates layouts and totals without cloning any allocator.
    pub(crate) fn new(
        options: MatchSearchOptions,
        strategy: CompressionStrategy,
        window_size: u64,
    ) -> Result<Self> {
        let layout = SearchKeyLayout::new(options.hash_log, options.key_bytes)?;
        let chain_nodes = checked_power_of_two(options.chain_log)?;
        let seed_nodes = chain_nodes.min(MAX_SEED_NODES);
        let visits = match strategy {
            CompressionStrategy::Greedy => 1,
            CompressionStrategy::Chain => usize::from(options.chain_visits),
            CompressionStrategy::Lazy => usize::from(options.lazy_visits),
        };
        let history_bytes = usize::try_from(window_size.min(u64::from(options.history_bytes)))
            .map_err(|_| Error::ResourceLimit)?;
        // Layout::array checks both multiplication and allocation alignment/
        // isize limits, rather than only whether the count fits usize.
        let greedy_table_bytes = Layout::array::<usize>(layout.bucket_count)
            .map_err(|_| Error::ResourceLimit)?
            .size();
        let chain_working_bytes =
            crate::matchchain::requested_storage_bytes(layout.bucket_count, chain_nodes)?;
        let chain_seed_bytes =
            crate::matchchain::requested_storage_bytes(layout.bucket_count, seed_nodes)?;
        Layout::array::<u8>(history_bytes).map_err(|_| Error::ResourceLimit)?;
        let all_cached_bytes = checked_cached_bytes(
            greedy_table_bytes,
            chain_working_bytes,
            chain_seed_bytes,
            history_bytes,
        )?;
        Ok(Self {
            layout,
            chain_nodes,
            seed_nodes,
            visits,
            history_bytes,
            greedy_table_bytes,
            chain_working_bytes,
            chain_seed_bytes,
            all_cached_bytes,
        })
    }

    pub(crate) const fn layout(&self) -> SearchKeyLayout {
        self.layout
    }

    pub(crate) const fn chain_nodes(&self) -> usize {
        self.chain_nodes
    }

    pub(crate) const fn seed_nodes(&self) -> usize {
        self.seed_nodes
    }

    pub(crate) const fn visits(&self) -> usize {
        self.visits
    }

    pub(crate) const fn history_bytes(&self) -> usize {
        self.history_bytes
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "validated payload diagnostic used by resource-bound tests"
        )
    )]
    pub(crate) const fn greedy_table_bytes(&self) -> usize {
        self.greedy_table_bytes
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "validated payload diagnostic used by resource-bound tests"
        )
    )]
    pub(crate) const fn chain_working_bytes(&self) -> usize {
        self.chain_working_bytes
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "validated payload diagnostic used by resource-bound tests"
        )
    )]
    pub(crate) const fn chain_seed_bytes(&self) -> usize {
        self.chain_seed_bytes
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "validated payload diagnostic used by resource-bound tests"
        )
    )]
    pub(crate) const fn all_cached_bytes(&self) -> usize {
        self.all_cached_bytes
    }
}

fn checked_cached_bytes(
    greedy_table: usize,
    chain_working: usize,
    chain_seed: usize,
    history: usize,
) -> Result<usize> {
    let total = greedy_table
        .checked_mul(2)
        .and_then(|value| value.checked_add(chain_working))
        .and_then(|value| value.checked_add(chain_seed))
        .and_then(|value| value.checked_add(history))
        .ok_or(Error::ResourceLimit)?;
    if total > MAX_REQUESTED_STORAGE_BYTES {
        return Err(Error::ResourceLimit);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    const DEFAULTS: MatchSearchOptions = MatchSearchOptions::new();

    #[test]
    fn defaults_preserve_every_existing_dimension_and_policy_effort() {
        assert_eq!(DEFAULTS, MatchSearchOptions::default());
        assert_eq!(
            (
                DEFAULTS.hash_log(),
                DEFAULTS.chain_log(),
                DEFAULTS.key_bytes(),
                DEFAULTS.chain_visits(),
                DEFAULTS.lazy_visits(),
                DEFAULTS.history_bytes()
            ),
            (16, 18, 4, 8, 16, 131_072)
        );
        for (strategy, visits) in [
            (CompressionStrategy::Greedy, 1),
            (CompressionStrategy::Chain, 8),
            (CompressionStrategy::Lazy, 16),
        ] {
            let shape = SearchShape::new(DEFAULTS, strategy, 1 << 17).unwrap();
            assert_eq!(shape.layout().bucket_count(), 65_536);
            assert_eq!(shape.chain_nodes(), 262_144);
            assert_eq!(shape.seed_nodes(), 65_536);
            assert_eq!(shape.history_bytes(), 131_072);
            assert_eq!(shape.visits(), visits);
        }
    }

    #[test]
    fn all_invalid_scalar_inputs_preserve_the_complete_nondefault_record() {
        type Setter = fn(&mut MatchSearchOptions, u8) -> Result<()>;
        type Getter = fn(&MatchSearchOptions) -> u8;
        let setters: [(Setter, Getter, u8, u8); 5] = [
            (
                MatchSearchOptions::set_hash_log,
                MatchSearchOptions::hash_log,
                10,
                18,
            ),
            (
                MatchSearchOptions::set_chain_log,
                MatchSearchOptions::chain_log,
                10,
                19,
            ),
            (
                MatchSearchOptions::set_key_bytes,
                MatchSearchOptions::key_bytes,
                4,
                8,
            ),
            (
                MatchSearchOptions::set_chain_visits,
                MatchSearchOptions::chain_visits,
                1,
                64,
            ),
            (
                MatchSearchOptions::set_lazy_visits,
                MatchSearchOptions::lazy_visits,
                1,
                64,
            ),
        ];
        let mut options = MatchSearchOptions::new();
        options.set_hash_log(17).unwrap();
        options.set_chain_log(19).unwrap();
        options.set_key_bytes(7).unwrap();
        options.set_chain_visits(63).unwrap();
        options.set_lazy_visits(3).unwrap();
        options.set_history_bytes(999_999).unwrap();
        for (setter, getter, minimum, maximum) in setters {
            for value in u8::MIN..=u8::MAX {
                let mut candidate = options;
                let result = setter(&mut candidate, value);
                if (minimum..=maximum).contains(&value) {
                    assert_eq!(result, Ok(()));
                    assert_eq!(getter(&candidate), value);
                } else {
                    assert_eq!(result, Err(Error::InvalidParameter));
                    assert_eq!(candidate, options);
                }
            }
        }
        for value in [0, MAX_HISTORY_BYTES + 1, i32::MAX as u32, u32::MAX] {
            let mut candidate = options;
            assert_eq!(
                candidate.set_history_bytes(value),
                Err(Error::InvalidParameter)
            );
            assert_eq!(candidate, options);
        }
    }

    #[test]
    fn layout_checks_both_fields_and_target_width_before_any_shift() {
        for hash_log in 0..=u8::MAX {
            for key_bytes in [0, 3, 4, 5, 6, 7, 8, 9, u8::MAX] {
                let result = SearchKeyLayout::new(hash_log, key_bytes);
                if (10..=18).contains(&hash_log) && (4..=8).contains(&key_bytes) {
                    let layout = result.unwrap();
                    assert_eq!(layout.hash_log(), hash_log);
                    assert_eq!(layout.key_bytes(), key_bytes);
                    assert_eq!(layout.bucket_count(), 1_usize << hash_log);
                } else {
                    assert_eq!(result, Err(Error::InvalidParameter));
                }
            }
        }
        for log in [usize::BITS as u8, u8::MAX] {
            assert_eq!(checked_power_of_two(log), Err(Error::ResourceLimit));
        }
        assert_eq!(checked_power_of_two(0), Ok(1));
    }

    #[test]
    fn fast_layout_admission_does_not_expand_named_public_bounds() {
        for log in 0..=u8::MAX {
            for width in [0, 3, 4, 5, 6, 7, 8, 9, u8::MAX] {
                let result = SearchKeyLayout::for_fast(log, width);
                if (6..=18).contains(&log) && (4..=7).contains(&width) {
                    let layout = result.unwrap();
                    assert_eq!(layout.bucket_count(), 1_usize << log);
                    assert_eq!((layout.hash_log(), layout.key_bytes()), (log, width));
                } else {
                    assert_eq!(result, Err(Error::InvalidParameter));
                }
                if log < 10 {
                    assert_eq!(
                        SearchKeyLayout::new(log, width),
                        Err(Error::InvalidParameter)
                    );
                    let mut options = DEFAULTS;
                    assert_eq!(options.set_hash_log(log), Err(Error::InvalidParameter));
                    assert_eq!(options, DEFAULTS);
                }
            }
        }
    }

    #[test]
    fn double_fast_long_layout_keeps_fast_and_named_domains_separate() {
        for log in 0..=u8::MAX {
            let result = SearchKeyLayout::for_double_fast_long(log);
            if (6..=18).contains(&log) {
                let layout = result.unwrap();
                assert_eq!((layout.hash_log(), layout.key_bytes()), (log, 8));
                assert_eq!(layout.bucket_count(), 1_usize << log);
                assert!(layout.read_key(b"1234567").is_none());
                assert!(layout.read_key(b"12345678").is_some());
                if log >= 10 {
                    assert_eq!(SearchKeyLayout::new(log, 8), Ok(layout));
                }
            } else {
                assert_eq!(result, Err(Error::InvalidParameter));
            }
            assert_eq!(
                SearchKeyLayout::for_fast(log, 8),
                Err(Error::InvalidParameter)
            );
            if log < 10 {
                assert_eq!(SearchKeyLayout::new(log, 8), Err(Error::InvalidParameter));
            }
        }
    }

    #[test]
    fn all_public_dimensions_have_checked_layout_costs_including_inactive_caches() {
        for hash_log in 10..=18 {
            for chain_log in 10..=19 {
                for key_bytes in 4..=8 {
                    let mut options = DEFAULTS;
                    options.set_hash_log(hash_log).unwrap();
                    options.set_chain_log(chain_log).unwrap();
                    options.set_key_bytes(key_bytes).unwrap();
                    options.set_history_bytes(MAX_HISTORY_BYTES).unwrap();
                    let shape =
                        SearchShape::new(options, CompressionStrategy::Greedy, u64::MAX).unwrap();
                    let heads = 1_usize << hash_log;
                    let nodes = 1_usize << chain_log;
                    let seed = nodes.min(65_536);
                    assert_eq!(shape.layout().bucket_count(), heads);
                    assert_eq!(shape.layout().key_bytes(), key_bytes);
                    assert_eq!(shape.chain_nodes(), nodes);
                    assert_eq!(shape.seed_nodes(), seed);
                    assert_eq!(shape.greedy_table_bytes(), heads * size_of::<usize>());
                    assert_eq!(
                        shape.chain_working_bytes(),
                        crate::matchchain::requested_storage_bytes(heads, nodes).unwrap()
                    );
                    assert_eq!(
                        shape.chain_seed_bytes(),
                        crate::matchchain::requested_storage_bytes(heads, seed).unwrap()
                    );
                    assert_eq!(
                        shape.all_cached_bytes(),
                        2 * shape.greedy_table_bytes()
                            + shape.chain_working_bytes()
                            + shape.chain_seed_bytes()
                            + shape.history_bytes()
                    );
                    assert!(shape.all_cached_bytes() <= MAX_REQUESTED_STORAGE_BYTES);
                    assert!(shape.all_cached_bytes() > 2 * shape.greedy_table_bytes());
                }
            }
        }
        // Derive record costs through the actual private types, not assumptions
        // about u64 alignment on a particular 32-bit ABI.
        let link = crate::matchchain::requested_storage_bytes(1, 0).unwrap();
        let node = crate::matchchain::requested_storage_bytes(0, 1).unwrap();
        let expected = 2 * 65_536 * size_of::<usize>()
            + 2 * 65_536 * link
            + (262_144 + 65_536) * node
            + 131_072;
        assert_eq!(
            SearchShape::new(DEFAULTS, CompressionStrategy::Chain, 1 << 17)
                .unwrap()
                .all_cached_bytes(),
            expected
        );
        if (size_of::<usize>(), link, node) == (8, 16, 32) {
            assert_eq!(expected, 13_762_560);
            let mut largest = DEFAULTS;
            largest.set_hash_log(18).unwrap();
            largest.set_chain_log(19).unwrap();
            largest.set_history_bytes(MAX_HISTORY_BYTES).unwrap();
            assert_eq!(
                SearchShape::new(largest, CompressionStrategy::Lazy, u64::MAX)
                    .unwrap()
                    .all_cached_bytes(),
                32_505_856
            );
        }
    }

    #[test]
    fn effective_history_admits_empty_one_shot_and_clamps_every_other_window() {
        for requested in [1, 2, 1024, 131_072, MAX_HISTORY_BYTES] {
            let mut options = DEFAULTS;
            options.set_history_bytes(requested).unwrap();
            for window in [
                0,
                1,
                2,
                3,
                1024,
                131_071,
                131_072,
                1 << 20,
                1 << 31,
                u64::MAX,
            ] {
                let shape = SearchShape::new(options, CompressionStrategy::Chain, window).unwrap();
                assert_eq!(
                    shape.history_bytes() as u64,
                    window.min(u64::from(requested))
                );
            }
        }
    }

    #[test]
    fn policy_effort_uses_separate_concrete_limits_without_changing_storage() {
        for chain in [1, 8, 16, 64] {
            for lazy in [1, 8, 16, 64] {
                let mut options = DEFAULTS;
                options.set_chain_visits(chain).unwrap();
                options.set_lazy_visits(lazy).unwrap();
                let greedy = SearchShape::new(options, CompressionStrategy::Greedy, 1024).unwrap();
                let chain_shape =
                    SearchShape::new(options, CompressionStrategy::Chain, 1024).unwrap();
                let lazy_shape =
                    SearchShape::new(options, CompressionStrategy::Lazy, 1024).unwrap();
                assert_eq!(greedy.visits(), 1);
                assert_eq!(chain_shape.visits(), usize::from(chain));
                assert_eq!(lazy_shape.visits(), usize::from(lazy));
                assert_eq!(greedy.all_cached_bytes(), chain_shape.all_cached_bytes());
                assert_eq!(greedy.all_cached_bytes(), lazy_shape.all_cached_bytes());
            }
        }
    }

    #[test]
    fn cost_arithmetic_rejects_layout_overflow_and_the_aggregate_ceiling() {
        assert_eq!(crate::matchchain::requested_storage_bytes(0, 0), Ok(0));
        assert_eq!(
            crate::matchchain::requested_storage_bytes(usize::MAX, 1),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            crate::matchchain::requested_storage_bytes(1, usize::MAX),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            checked_cached_bytes(usize::MAX, 0, 0, 0),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            checked_cached_bytes(0, usize::MAX, 1, 0),
            Err(Error::ResourceLimit)
        );
        assert_eq!(
            checked_cached_bytes(0, 0, 0, MAX_REQUESTED_STORAGE_BYTES),
            Ok(MAX_REQUESTED_STORAGE_BYTES)
        );
        assert_eq!(
            checked_cached_bytes(0, 0, 0, MAX_REQUESTED_STORAGE_BYTES + 1),
            Err(Error::ResourceLimit)
        );
    }
}
