//! Allocator-owned, checked no-dictionary Optimal frequency lifecycle.
#![cfg_attr(not(feature = "unsafe-optimal-frequencies"), forbid(unsafe_code))]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "Private optimal state precedes parser integration"
    )
)]

use crate::optimal_price::{self as price, SymbolPrice};
use crate::{Error, Result};
use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

pub(crate) const CELLS: usize = 377;
const START: [usize; 4] = [0, 256, 292, 345];
const LENGTH: [usize; 4] = [256, 36, 53, 32];
// Pinned ZSTD_rescaleFreqs distributions, v1.5.7; NOTICE/LICENSE.zstd.
const OFFSET_BASE: [u32; 32] = [
    6, 2, 1, 1, 2, 3, 4, 4, 4, 3, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrequencySnapshot {
    pub(crate) counts: [u32; CELLS],
    pub(crate) sums: [u32; 4],
    pub(crate) base_prices: [u32; 4],
    pub(crate) predefined: bool,
    pub(crate) initialized: bool,
    pub(crate) dirty: bool,
}

pub(crate) struct OptimalFrequencies<A: Allocator = Global> {
    counts: Vec<u32, A>,
    sums: [u32; 4],
    base_prices: [u32; 4],
    predefined: bool,
    initialized: bool,
    dirty: bool,
    disabled_literals: bool,
}

fn read_cell(values: &[u32], index: usize) -> Result<u32> {
    if index >= values.len() {
        return Err(Error::CorruptData);
    }
    #[cfg(not(feature = "unsafe-optimal-frequencies"))]
    let value = values[index];
    #[cfg(feature = "unsafe-optimal-frequencies")]
    let value = {
        // SAFETY: The runtime check proves index is below initialized slice
        // length, not capacity. The live Vec owner supplies aligned initialized
        // u32 cells. This shared borrow excludes writes/reallocation for the
        // access; only a copied scalar escapes and no reference outlives it.
        unsafe { *values.get_unchecked(index) }
    };
    Ok(value)
}
fn write_cell(values: &mut [u32], index: usize, value: u32) -> Result<()> {
    if index >= values.len() {
        return Err(Error::CorruptData);
    }
    #[cfg(not(feature = "unsafe-optimal-frequencies"))]
    {
        values[index] = value;
    }
    #[cfg(feature = "unsafe-optimal-frequencies")]
    {
        // SAFETY: The runtime check establishes index < initialized length.
        // The live slice is valid, aligned and exclusively borrowed, excluding
        // every competing reference, mutation and reallocation. Assigning a
        // copied u32 preserves initialization; the temporary reference ends
        // here and cannot escape or outlive its owning Vec.
        unsafe {
            *values.get_unchecked_mut(index) = value;
        }
    }
    Ok(())
}
fn price_error(error: price::PriceError) -> Error {
    match error {
        price::PriceError::InvalidLength | price::PriceError::InvalidOffset => {
            Error::InvalidParameter
        }
        price::PriceError::InvalidStatistics => Error::CorruptData,
        price::PriceError::PriceOverflow => Error::ResourceLimit,
    }
}
fn counter_add(left: u32, right: u32) -> Result<u32> {
    left.checked_add(right)
        .filter(|&v| v < u32::MAX)
        .ok_or(Error::ResourceLimit)
}
fn refreshed(sums: [u32; 4], disabled_literals: bool) -> Result<[u32; 4]> {
    let mut result = [0; 4];
    for table in 0..4 {
        if table != 0 || !disabled_literals {
            result[table] = price::bit_weight(sums[table]).map_err(|_| Error::CorruptData)?;
        }
    }
    Ok(result)
}

impl<A: Allocator> OptimalFrequencies<A> {
    pub(crate) const fn disabled_literals(&self) -> bool {
        self.disabled_literals
    }
    pub(crate) fn new_in(disabled_literals: bool, owner: A) -> Result<Self> {
        let mut counts = Vec::new_in(owner);
        counts
            .try_reserve_exact(CELLS)
            .map_err(|_| Error::AllocationFailed)?;
        counts.resize(CELLS, 0);
        Ok(Self {
            counts,
            sums: [0; 4],
            base_prices: [0; 4],
            predefined: false,
            initialized: false,
            dirty: false,
            disabled_literals,
        })
    }
    fn check_layout(&self) -> Result<()> {
        if self.counts.len() != CELLS {
            Err(Error::CorruptData)
        } else {
            Ok(())
        }
    }
    pub(crate) fn snapshot(&self) -> Result<FrequencySnapshot> {
        self.check_layout()?;
        let mut counts = [0; CELLS];
        for (index, cell) in counts.iter_mut().enumerate() {
            *cell = read_cell(&self.counts, index)?;
        }
        Ok(FrequencySnapshot {
            counts,
            sums: self.sums,
            base_prices: self.base_prices,
            predefined: self.predefined,
            initialized: self.initialized,
            dirty: self.dirty,
        })
    }
    fn publish(&mut self, next: FrequencySnapshot) -> Result<()> {
        self.check_layout()?;
        // All indices are within the runtime-checked exact live length, and
        // this exclusive borrow cannot change that length during publication.
        for (index, value) in next.counts.into_iter().enumerate() {
            write_cell(&mut self.counts, index, value)?;
        }
        self.sums = next.sums;
        self.base_prices = next.base_prices;
        self.predefined = next.predefined;
        self.initialized = next.initialized;
        self.dirty = next.dirty;
        Ok(())
    }
    pub(crate) fn reset(&mut self) -> Result<()> {
        self.publish(FrequencySnapshot {
            counts: [0; CELLS],
            sums: [0; 4],
            base_prices: [0; 4],
            predefined: false,
            initialized: false,
            dirty: false,
        })
    }
    pub(crate) fn begin_block(&mut self, source: &[u8]) -> Result<()> {
        if source.len() > price::BLOCK_LIMIT as usize {
            return Err(Error::ResourceLimit);
        }
        let mut next = self.snapshot()?;
        next.predefined = false;
        if !next.initialized {
            next.predefined = source.len() <= 8;
            if !self.disabled_literals {
                let mut histogram = [0u32; 256];
                for &byte in source {
                    histogram[usize::from(byte)] += 1;
                }
                for (index, count) in histogram.into_iter().enumerate() {
                    next.counts[index] = u32::from(count != 0) + (count >> 8);
                }
            }
            next.counts[256..292].fill(1);
            next.counts[256] = 4;
            next.counts[257] = 2;
            next.counts[292..345].fill(1);
            next.counts[345..377].copy_from_slice(&OFFSET_BASE);
        } else {
            for table in 0..4 {
                if table == 0 && self.disabled_literals {
                    continue;
                }
                let cells = &mut next.counts[START[table]..START[table] + LENGTH[table]];
                let sum = cells
                    .iter()
                    .try_fold(0, |sum, &count| counter_add(sum, count))?;
                let factor = sum >> if table == 0 { 12 } else { 11 };
                if factor > 1 {
                    let shift = factor.ilog2();
                    for count in cells {
                        *count = counter_add(1, *count >> shift)?;
                    }
                }
            }
        }
        for table in 0..4 {
            next.sums[table] = next.counts[START[table]..START[table] + LENGTH[table]]
                .iter()
                .try_fold(0, |sum, &count| counter_add(sum, count))?;
        }
        next.base_prices = refreshed(next.sums, self.disabled_literals)?;
        next.initialized = true;
        next.dirty = false;
        self.publish(next)
    }
    pub(crate) fn update(
        &mut self,
        literals: &[u8],
        encoded_offset: u32,
        match_length: u32,
    ) -> Result<()> {
        if !self.initialized {
            return Err(Error::InvalidState);
        }
        if literals.len() >= price::BLOCK_LIMIT as usize {
            return Err(Error::InvalidParameter);
        }
        let ll = price::literal_length_code(literals.len() as u32).map_err(price_error)?;
        let ml = price::match_length_code(match_length).map_err(price_error)?;
        let off = price::offset_code(encoded_offset).map_err(price_error)?;
        let mut next = self.snapshot()?;
        if !self.disabled_literals {
            for &byte in literals {
                let cell = &mut next.counts[usize::from(byte)];
                *cell = counter_add(*cell, 2)?;
            }
            next.sums[0] = counter_add(next.sums[0], (literals.len() as u32) * 2)?;
        }
        for (table, code) in [(1, ll), (2, ml), (3, off)] {
            let index = START[table].checked_add(code).ok_or(Error::CorruptData)?;
            if code >= LENGTH[table] {
                return Err(Error::CorruptData);
            }
            next.counts[index] = counter_add(next.counts[index], 1)?;
            next.sums[table] = counter_add(next.sums[table], 1)?;
        }
        next.dirty = true;
        self.publish(next)
    }
    pub(crate) fn refresh(&mut self) -> Result<()> {
        self.check_layout()?;
        if !self.initialized {
            return Err(Error::InvalidState);
        }
        let prices = refreshed(self.sums, self.disabled_literals)?;
        self.base_prices = prices;
        self.dirty = false;
        Ok(())
    }
    pub(crate) fn prices(
        &self,
        literals: &[u8],
        literal_length: u32,
        encoded_offset: u32,
        match_length: u32,
    ) -> Result<[u32; 3]> {
        self.check_layout()?;
        if !self.initialized || self.dirty {
            return Err(Error::InvalidState);
        }
        let ll = price::literal_length_code(literal_length).map_err(price_error)?;
        let ml = price::match_length_code(match_length).map_err(price_error)?;
        let off = price::offset_code(encoded_offset).map_err(price_error)?;
        let raw = if self.disabled_literals {
            price::uncompressed_literals_cost(literals.len())
        } else if self.predefined {
            price::predefined_literals_cost(literals.len())
        } else {
            price::dynamic_literals_cost(
                literals,
                self.base_prices[0],
                self.counts[..256]
                    .try_into()
                    .map_err(|_| Error::CorruptData)?,
            )
        }
        .map_err(price_error)?;
        let symbol = |table: usize, code: usize| -> Result<SymbolPrice> {
            SymbolPrice::new(
                self.base_prices[table],
                read_cell(&self.counts, START[table] + code)?,
            )
            .map_err(|_| Error::CorruptData)
        };
        let ll_price = price::literal_length_price(
            literal_length,
            if self.predefined {
                None
            } else {
                Some(symbol(1, ll)?)
            },
        )
        .map_err(|_| Error::CorruptData)?;
        let match_price = price::match_price(
            encoded_offset,
            match_length,
            if self.predefined {
                None
            } else {
                Some((symbol(3, off)?, symbol(2, ml)?))
            },
        )
        .map_err(|_| Error::CorruptData)?;
        Ok([raw, ll_price, match_price])
    }
}

#[cfg(test)]
#[path = "../tests/support/optimal_price_fixture.rs"]
mod fixture;

#[cfg(test)]
mod tests {
    use super::*;
    use zstdoxide_test_allocator::{AllocatorConfig, TrackingAllocator};

    #[test]
    fn every_captured_frequency_transition_and_query_matches() {
        let cases = fixture::read(fixture::PACK).unwrap();
        let mut transitions = 0;
        let mut queries = 0;
        for case in cases {
            let mut model = OptimalFrequencies::new_in(case.disabled_literals, Global).unwrap();
            for expected in case.observations {
                let [tag, a, b, c, d, e] = expected.operation;
                match tag {
                    0 | 3 => model
                        .begin_block(&case.source[a as usize..(a + b) as usize])
                        .unwrap(),
                    1 | 5 => {
                        for _ in 0..if tag == 5 { e } else { 1 } {
                            model
                                .update(&case.source[a as usize..(a + b) as usize], c, d)
                                .unwrap();
                        }
                    }
                    2 => model.refresh().unwrap(),
                    4 => {
                        assert_eq!(
                            model
                                .prices(&case.source[a as usize..(a + b) as usize], c, d, e)
                                .unwrap(),
                            expected.query.unwrap(),
                            "{}",
                            case.name
                        );
                        queries += 1;
                    }
                    _ => unreachable!(),
                }
                let actual = model.snapshot().unwrap();
                assert_eq!(
                    actual.counts, expected.frequencies,
                    "{} {:?}",
                    case.name, expected.operation
                );
                assert_eq!(actual.sums, expected.sums, "{}", case.name);
                assert_eq!(actual.base_prices, expected.base_prices, "{}", case.name);
                assert_eq!(actual.predefined, expected.predefined, "{}", case.name);
                assert!(actual.initialized);
                transitions += 1;
            }
        }
        assert_eq!(transitions, 2018);
        assert_eq!(queries, 1420);
    }

    #[test]
    fn phase_and_invalid_requests_preserve_the_complete_model() {
        let mut model = OptimalFrequencies::new_in(false, Global).unwrap();
        let fresh = model.snapshot().unwrap();
        assert_eq!(model.update(b"", 1, 3), Err(Error::InvalidState));
        assert_eq!(model.refresh(), Err(Error::InvalidState));
        assert_eq!(model.prices(b"", 0, 1, 3), Err(Error::InvalidState));
        assert_eq!(model.snapshot().unwrap(), fresh);
        model.begin_block(b"aaaaaaaaa").unwrap();
        let before = model.snapshot().unwrap();
        for (offset, length) in [(0, 3), (1, 0), (1, 2), (1, 131073)] {
            assert_eq!(
                model.update(b"a", offset, length),
                Err(Error::InvalidParameter)
            );
            assert_eq!(model.snapshot().unwrap(), before);
        }
        assert_eq!(
            model.update(&alloc::vec![0;131072], 1, 3),
            Err(Error::InvalidParameter)
        );
        assert_eq!(
            model.begin_block(&alloc::vec![0;131073]),
            Err(Error::ResourceLimit)
        );
        assert_eq!(model.snapshot().unwrap(), before);
        model.update(b"a", 1, 3).unwrap();
        let dirty = model.snapshot().unwrap();
        assert!(dirty.dirty);
        assert_eq!(model.prices(b"a", 0, 1, 3), Err(Error::InvalidState));
        assert_eq!(model.snapshot().unwrap(), dirty);
        model.refresh().unwrap();
        assert!(!model.snapshot().unwrap().dirty);
        assert!(model.prices(b"a", 0, 1, 3).is_ok());
    }

    #[test]
    fn counter_overflow_rejects_without_partial_literal_or_symbol_updates() {
        for literal_overflow in [false, true] {
            let mut model = OptimalFrequencies::new_in(false, Global).unwrap();
            model.begin_block(b"aaaaaaaaa").unwrap();
            if literal_overflow {
                model.counts[97] = u32::MAX - 1;
                model.sums[0] = u32::MAX - 1;
            } else {
                model.counts[256] = u32::MAX - 37;
                model.sums[1] = u32::MAX - 1;
            }
            model.refresh().unwrap();
            let before = model.snapshot().unwrap();
            assert_eq!(
                model.update(if literal_overflow { b"a" } else { b"" }, 1, 3),
                Err(Error::ResourceLimit)
            );
            assert_eq!(model.snapshot().unwrap(), before);
        }
    }

    #[test]
    fn malformed_live_length_and_scalar_indices_reject_before_access() {
        let mut values = [7, 9];
        assert_eq!(read_cell(&values, 2), Err(Error::CorruptData));
        assert_eq!(read_cell(&values, usize::MAX), Err(Error::CorruptData));
        assert_eq!(write_cell(&mut values, 2, 99), Err(Error::CorruptData));
        assert_eq!(
            write_cell(&mut values, usize::MAX, 99),
            Err(Error::CorruptData)
        );
        assert_eq!(values, [7, 9]);
        let mut model = OptimalFrequencies::new_in(false, Global).unwrap();
        model.begin_block(b"abcdefghi").unwrap();
        model.counts.pop();
        let before = model.counts.clone();
        assert_eq!(model.snapshot(), Err(Error::CorruptData));
        assert_eq!(model.begin_block(b"x"), Err(Error::CorruptData));
        assert_eq!(model.update(b"x", 1, 3), Err(Error::CorruptData));
        assert_eq!(model.refresh(), Err(Error::CorruptData));
        assert_eq!(model.reset(), Err(Error::CorruptData));
        assert_eq!(model.prices(b"x", 1, 1, 3), Err(Error::CorruptData));
        assert_eq!(model.counts, before);
    }

    #[test]
    fn borrowed_non_clone_owner_outlives_source_and_reset_reuses_storage() {
        let owner = TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 2,
            fail_at: None,
        })
        .unwrap()
        .into_non_clone();
        let monitor = owner.monitor();
        {
            let mut model = OptimalFrequencies::new_in(false, &owner).unwrap();
            {
                let source = alloc::vec![b'a';513];
                model.begin_block(&source).unwrap();
            }
            let attempts = monitor.snapshot().attempts;
            assert_eq!(attempts, 1);
            model.update(b"aaa", 1, 3).unwrap();
            model.refresh().unwrap();
            assert!(model.prices(b"a", 1, 1, 3).is_ok());
            model.reset().unwrap();
            assert_eq!(model.snapshot().unwrap().counts, [0; CELLS]);
            assert_eq!(model.prices(b"", 0, 1, 3), Err(Error::InvalidState));
            model.begin_block(b"bbbbbbbbb").unwrap();
            assert_eq!(monitor.snapshot().attempts, attempts);
        }
        monitor.assert_released();
        assert!(!monitor.snapshot().violations.any());
        drop(owner);
        let final_state = monitor.snapshot();
        assert_eq!(final_state.identities_created, final_state.owners_dropped);
    }

    #[test]
    fn initial_allocation_failure_drops_the_exact_moved_owner() {
        let owner = TrackingAllocator::new(AllocatorConfig {
            max_live_allocations: 2,
            fail_at: core::num::NonZeroU64::new(1),
        })
        .unwrap()
        .into_non_clone();
        let monitor = owner.monitor();
        assert!(matches!(
            OptimalFrequencies::new_in(false, owner),
            Err(Error::AllocationFailed)
        ));
        monitor.assert_released();
        let final_state = monitor.snapshot();
        assert_eq!(final_state.failures, 1);
        assert_eq!(final_state.identities_created, final_state.owners_dropped);
    }

    #[test]
    fn malformed_consumed_statistics_reject_without_mutation() {
        let mut model = OptimalFrequencies::new_in(false, Global).unwrap();
        model.begin_block(b"aaaaaaaaa").unwrap();
        model.counts[97] = u32::MAX;
        let before = model.snapshot().unwrap();
        assert_eq!(model.prices(b"a", 0, 1, 3), Err(Error::CorruptData));
        assert_eq!(model.snapshot().unwrap(), before);
    }

    #[test]
    fn disabled_literals_and_predefined_transition_keep_separate_statistics() {
        let mut model = OptimalFrequencies::new_in(true, Global).unwrap();
        model.begin_block(b"abcdefgh").unwrap();
        assert!(model.snapshot().unwrap().predefined);
        model.update(b"abcdef", 4, 35).unwrap();
        model.refresh().unwrap();
        assert_eq!(model.snapshot().unwrap().sums[0], 0);
        assert_eq!(model.prices(b"a", 0, 1, 3).unwrap()[0], 2048);
        model.begin_block(b"").unwrap();
        assert!(!model.snapshot().unwrap().predefined);
        assert_eq!(model.snapshot().unwrap().base_prices[0], 0);
    }
}
