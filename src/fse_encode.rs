//! Checked inverse FSE transitions for sequence encoding.
//!
//! The state/symbol relation corresponds to `FSE_buildCTable_wksp`,
//! `FSE_initCState2`, and `FSE_encodeSymbol` in facebook/zstd v1.5.7. This
//! implementation owns typed rows instead of adopting C's packed table layout.

use alloc::vec::Vec;

use crate::fse::{FseEntry, FseTable};
use crate::{Error, Result};

const UNAVAILABLE: u16 = u16::MAX;

/// A copied inverse relation: decoding `state` yields the requested symbol,
/// consumes `bits` bits with `value`, then reaches the requested successor.
/// This initialized scalar record needs no separate unsafe representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FseEncodingTransition {
    pub(crate) state: u16,
    pub(crate) value: u16,
    pub(crate) bits: u8,
}

impl FseEncodingTransition {
    const UNAVAILABLE: Self = Self {
        state: UNAVAILABLE,
        value: 0,
        bits: 0,
    };
}

/// An immutable owned inverse table, or one allocation-free inline RLE symbol.
///
/// Each present dense row covers all `state_count` successor states exactly
/// once. Absent rows and initial states are explicitly unavailable. Logical
/// links are indices into initialized owned storage; no source borrow escapes
/// construction. A normalized singleton retains its original nonzero log.
#[derive(Debug)]
pub(crate) struct FseEncodingTable {
    transitions: Vec<FseEncodingTransition>,
    initial: Vec<u16>,
    repeated: Option<u8>,
    state_count: usize,
    table_log: u8,
}

impl FseEncodingTable {
    /// Build an ordinary normalized table with the decoder's mass and alphabet
    /// validation. Single-symbol normalized dictionary tables enter through
    /// `from_decoding`; an inline RLE table enters through `rle`.
    pub(crate) fn new(counts: &[i16], table_log: u8) -> Result<Self> {
        Self::from_decoding(&FseTable::from_normalized(counts, table_log)?)
    }

    /// Invert an immutable validated decoding table without retaining a borrow.
    /// The first decoding state for each symbol is its terminal initialization,
    /// preserving the previous predefined encoder's exact initial selection.
    pub(crate) fn from_decoding(decoding: &FseTable) -> Result<Self> {
        Self::from_relation(decoding.table_log(), |state| decoding.entry(state))
    }

    /// Construct the wire's inline RLE mode, which has no state or payload bits.
    pub(crate) const fn rle(symbol: u8) -> Self {
        Self {
            transitions: Vec::new(),
            initial: Vec::new(),
            repeated: Some(symbol),
            state_count: 1,
            table_log: 0,
        }
    }

    pub(crate) fn table_log(&self) -> u8 {
        self.table_log
    }

    pub(crate) fn retained_capacity(&self) -> usize {
        self.transitions.capacity() * core::mem::size_of::<FseEncodingTransition>()
            + self.initial.capacity() * core::mem::size_of::<u16>()
    }

    pub(crate) fn initial(&self, symbol: usize) -> Result<u16> {
        if let Some(repeated) = self.repeated {
            return if symbol == usize::from(repeated) {
                Ok(0)
            } else {
                Err(Error::CorruptData)
            };
        }
        self.initial
            .get(symbol)
            .copied()
            .filter(|&state| state != UNAVAILABLE)
            .ok_or(Error::CorruptData)
    }

    pub(crate) fn transition(&self, symbol: usize, next: u16) -> Result<FseEncodingTransition> {
        // This also rejects absent rows, including zero-initialized holes in
        // the alphabet, before any flattened transition access is attempted.
        self.initial(symbol)?;
        if usize::from(next) >= self.state_count {
            return Err(Error::CorruptData);
        }
        if self.repeated.is_some() {
            return Ok(FseEncodingTransition {
                state: 0,
                value: 0,
                bits: 0,
            });
        }
        let index = symbol
            .checked_mul(self.state_count)
            .and_then(|row| row.checked_add(usize::from(next)))
            .ok_or(Error::CorruptData)?;
        self.transitions
            .get(index)
            .copied()
            .filter(|transition| transition.state != UNAVAILABLE)
            .ok_or(Error::CorruptData)
    }

    /// The relation callback reads the same immutable table on both passes.
    /// Keeping this checked setup separate permits malformed-relation tests
    /// without exposing unvalidated constructors on the decoding table.
    fn from_relation(table_log: u8, entry_at: impl Fn(usize) -> Result<FseEntry>) -> Result<Self> {
        if table_log == 0 {
            let entry = entry_at(0)?;
            if entry.bits != 0 || entry.base != 0 {
                return Err(Error::CorruptData);
            }
            return Ok(Self::rle(entry.symbol));
        }
        if !(5..=12).contains(&table_log) {
            return Err(Error::CorruptData);
        }
        let state_count = 1_usize
            .checked_shl(u32::from(table_log))
            .ok_or(Error::ResourceLimit)?;
        let mut first_states = [UNAVAILABLE; 256];
        let mut symbol_count = 0;
        for state in 0..state_count {
            let entry = entry_at(state)?;
            validate_range(entry, table_log, state_count)?;
            let symbol = usize::from(entry.symbol);
            symbol_count = symbol_count.max(symbol + 1);
            if first_states[symbol] == UNAVAILABLE {
                first_states[symbol] = u16::try_from(state).map_err(|_| Error::ResourceLimit)?;
            }
        }
        let transition_count = symbol_count
            .checked_mul(state_count)
            .ok_or(Error::ResourceLimit)?;
        transition_count
            .checked_mul(core::mem::size_of::<FseEncodingTransition>())
            .ok_or(Error::ResourceLimit)?;
        let mut transitions = Vec::new();
        transitions
            .try_reserve_exact(transition_count)
            .map_err(|_| Error::AllocationFailed)?;
        transitions.resize(transition_count, FseEncodingTransition::UNAVAILABLE);
        let mut initial = Vec::new();
        initial
            .try_reserve_exact(symbol_count)
            .map_err(|_| Error::AllocationFailed)?;
        initial.extend_from_slice(&first_states[..symbol_count]);

        for state in 0..state_count {
            let entry = entry_at(state)?;
            let width = validate_range(entry, table_log, state_count)?;
            let symbol = usize::from(entry.symbol);
            let row = symbol
                .checked_mul(state_count)
                .ok_or(Error::ResourceLimit)?;
            for value in 0..width {
                let next = usize::from(entry.base)
                    .checked_add(value)
                    .ok_or(Error::CorruptData)?;
                let index = row.checked_add(next).ok_or(Error::ResourceLimit)?;
                let slot = transitions.get_mut(index).ok_or(Error::CorruptData)?;
                if slot.state != UNAVAILABLE {
                    return Err(Error::CorruptData);
                }
                *slot = FseEncodingTransition {
                    state: u16::try_from(state).map_err(|_| Error::ResourceLimit)?,
                    value: u16::try_from(value).map_err(|_| Error::ResourceLimit)?,
                    bits: entry.bits,
                };
            }
        }
        for (symbol, &initial_state) in initial.iter().enumerate() {
            let row = &transitions[symbol * state_count..(symbol + 1) * state_count];
            if initial_state != UNAVAILABLE
                && row.iter().any(|transition| transition.state == UNAVAILABLE)
            {
                return Err(Error::CorruptData);
            }
        }
        Ok(Self {
            transitions,
            initial,
            repeated: None,
            state_count,
            table_log,
        })
    }
}

fn validate_range(entry: FseEntry, table_log: u8, state_count: usize) -> Result<usize> {
    if entry.bits > table_log {
        return Err(Error::CorruptData);
    }
    let width = 1_usize
        .checked_shl(u32::from(entry.bits))
        .ok_or(Error::CorruptData)?;
    let end = usize::from(entry.base)
        .checked_add(width)
        .ok_or(Error::CorruptData)?;
    if end > state_count {
        return Err(Error::CorruptData);
    }
    Ok(width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{LL_DEFAULT, ML_DEFAULT, OF_DEFAULT};
    use crate::entropy_encode::write_normalized_counts;

    fn verify_inverse(decoding: &FseTable) {
        let encoding = FseEncodingTable::from_decoding(decoding).unwrap();
        let state_count = 1_usize << decoding.table_log();
        assert_eq!(encoding.table_log(), decoding.table_log());
        for symbol in 0..=255 {
            let first = (0..state_count)
                .find(|&state| usize::from(decoding.entry(state).unwrap().symbol) == symbol);
            if let Some(first) = first {
                assert_eq!(encoding.initial(symbol).unwrap(), first as u16);
                for next in 0..state_count {
                    let inverse = encoding.transition(symbol, next as u16).unwrap();
                    let entry = decoding.entry(usize::from(inverse.state)).unwrap();
                    assert_eq!(usize::from(entry.symbol), symbol);
                    assert_eq!(entry.bits, inverse.bits);
                    assert!(usize::from(inverse.value) < 1 << inverse.bits);
                    assert_eq!(usize::from(entry.base + inverse.value), next);
                }
            } else {
                assert_eq!(encoding.initial(symbol), Err(Error::CorruptData));
                for next in 0..state_count {
                    assert_eq!(
                        encoding.transition(symbol, next as u16),
                        Err(Error::CorruptData)
                    );
                }
            }
            assert_eq!(
                encoding.transition(symbol, state_count as u16),
                Err(Error::CorruptData)
            );
        }
        assert_eq!(encoding.initial(usize::MAX), Err(Error::CorruptData));
        assert_eq!(encoding.transition(usize::MAX, 0), Err(Error::CorruptData));
    }

    #[test]
    fn predefined_tables_preserve_initial_states_and_every_inverse_relation() {
        for (counts, log) in [
            (&LL_DEFAULT[..], 6),
            (&ML_DEFAULT[..], 6),
            (&OF_DEFAULT[..], 5),
        ] {
            let decoding = FseTable::from_normalized(counts, log).unwrap();
            verify_inverse(&decoding);
            let encoding = FseEncodingTable::new(counts, log).unwrap();
            assert!(encoding.retained_capacity() > 0);
            for symbol in 0..counts.len() {
                let initial = encoding.initial(symbol).unwrap();
                assert_eq!(
                    decoding.entry(usize::from(initial)).unwrap().symbol as usize,
                    symbol
                );
            }
        }
    }

    #[test]
    fn support_holes_and_all_supported_logs_have_complete_inverse_rows() {
        for log in 5..=12 {
            let counts = [(1_i16 << log) - 2, 0, -1, 1];
            verify_inverse(&FseTable::from_normalized(&counts, log).unwrap());
        }
        verify_inverse(&FseTable::from_normalized(&[1; 256], 8).unwrap());
    }

    #[test]
    fn inline_rle_accepts_only_its_symbol_and_zero_successor_without_allocation() {
        for symbol in [0, 1, 255] {
            let table = FseEncodingTable::rle(symbol);
            assert_eq!(table.table_log(), 0);
            assert_eq!(table.retained_capacity(), 0);
            assert_eq!(table.initial(usize::from(symbol)), Ok(0));
            assert_eq!(
                table.transition(usize::from(symbol), 0),
                Ok(FseEncodingTransition {
                    state: 0,
                    value: 0,
                    bits: 0
                })
            );
            assert_eq!(
                table.transition(usize::from(symbol), 1),
                Err(Error::CorruptData)
            );
            assert_eq!(table.initial(256), Err(Error::CorruptData));
            verify_inverse(&FseTable::rle(symbol));
        }
    }

    #[test]
    fn dictionary_singletons_preserve_their_nonzero_initial_state_width() {
        for log in 5..=12 {
            let header = write_normalized_counts(&[0, 1_i16 << log], log).unwrap();
            let (decoding, consumed) = FseTable::read_dictionary(&header, 1, log).unwrap();
            assert_eq!(consumed, header.len());
            let encoding = FseEncodingTable::from_decoding(&decoding).unwrap();
            assert_eq!(encoding.table_log(), log);
            assert_eq!(encoding.initial(1), Ok(0));
            assert!(encoding.retained_capacity() > 0);
            for next in 0..1 << log {
                assert_eq!(
                    encoding.transition(1, next).unwrap(),
                    FseEncodingTransition {
                        state: next,
                        value: 0,
                        bits: 0
                    }
                );
            }
            assert_eq!(encoding.initial(0), Err(Error::CorruptData));
            assert_eq!(encoding.transition(0, 0), Err(Error::CorruptData));
        }
    }

    #[test]
    fn malformed_distributions_and_decoding_relations_are_rejected() {
        for (counts, log) in [
            (&[][..], 5),
            (&[32][..], 5),
            (&[-2, 34][..], 5),
            (&[16, 15][..], 5),
            (&[16, 17][..], 5),
            (&[16, 16][..], 4),
            (&[16, 16][..], 13),
            (&[1; 257][..], 8),
        ] {
            assert!(matches!(
                FseEncodingTable::new(counts, log),
                Err(Error::CorruptData)
            ));
        }
        for entry in [
            FseEntry {
                symbol: 0,
                base: 0,
                bits: 6,
            },
            FseEntry {
                symbol: 0,
                base: 32,
                bits: 0,
            },
            FseEntry {
                symbol: 0,
                base: u16::MAX,
                bits: 1,
            },
            // Each row assignment is individually bounded, but collisions
            // make the relation ambiguous and must fail during inversion.
            FseEntry {
                symbol: 0,
                base: 0,
                bits: 5,
            },
        ] {
            assert!(matches!(
                FseEncodingTable::from_relation(5, |_| Ok(entry)),
                Err(Error::CorruptData)
            ));
        }
        // A noncolliding row for symbol zero misses successor 31; symbol one
        // independently covers all states. Publication must reject the hole.
        assert!(matches!(
            FseEncodingTable::from_relation(5, |state| Ok(if state == 31 {
                FseEntry {
                    symbol: 1,
                    base: 0,
                    bits: 5,
                }
            } else {
                FseEntry {
                    symbol: 0,
                    base: state as u16,
                    bits: 0,
                }
            })),
            Err(Error::CorruptData)
        ));
        for entry in [
            FseEntry {
                symbol: 0,
                base: 1,
                bits: 0,
            },
            FseEntry {
                symbol: 0,
                base: 0,
                bits: 1,
            },
        ] {
            assert!(matches!(
                FseEncodingTable::from_relation(0, |_| Ok(entry)),
                Err(Error::CorruptData)
            ));
        }
    }
}
