//! Checked coupled optLevel=0/2 scalar prices from zstd_opt.c v1.5.7.
//!
//! This preparatory module owns no frequency table or allocation. Its focused
//! harness qualifies captured queries before retained-state integration.
use crate::constants::{LL_BASE, LL_BITS, ML_BASE, ML_BITS};

pub(crate) const BLOCK_LIMIT: u32 = 131_072;
const SCALE: u32 = 256;
const UNREACHABLE_PRICE: u32 = 1 << 30;

/// Copied whole-parser policy; scalar qualification does not admit Ultra frames.
/// This value owns no memory and has no alternate unsafe representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PricePolicy {
    Optimal,
    #[allow(
        dead_code,
        reason = "Ultra scalar qualification precedes parser admission"
    )]
    Ultra,
}

impl PricePolicy {
    pub(crate) fn weight(self, statistic: u32) -> Result<u32, PriceError> {
        match self {
            Self::Optimal => bit_weight(statistic),
            Self::Ultra => fractional_weight(statistic),
        }
    }

    fn valid_base(self, base: u32) -> bool {
        match self {
            Self::Optimal => base <= 31 * SCALE && base.is_multiple_of(SCALE),
            Self::Ultra => {
                if !(SCALE..=6143).contains(&base) {
                    return false;
                }
                // For exponent k < 8, integer statistics quantize the fractional
                // remainder in steps of 2^(8-k). Larger exponents fill every step.
                let exponent = base / SCALE - 1;
                let step = 1 << 8u32.saturating_sub(exponent);
                (base % SCALE).is_multiple_of(step)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PriceError {
    InvalidLength,
    InvalidOffset,
    InvalidStatistics,
    PriceOverflow,
}

/// Copied policy and finite weights, with a nonnegative symbol cost.
/// No owner, borrowed table or alternate unchecked representation is needed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SymbolPrice {
    policy: PricePolicy,
    base: u32,
    frequency: u32,
}

impl SymbolPrice {
    /// The retained model supplies an explicitly refreshed cached base price.
    /// Validate the consumed frequency without changing any model statistics.
    pub(crate) fn new(base_price: u32, frequency: u32) -> Result<Self, PriceError> {
        Self::new_with_policy(PricePolicy::Optimal, base_price, frequency)
    }

    pub(crate) fn new_with_policy(
        policy: PricePolicy,
        base_price: u32,
        frequency: u32,
    ) -> Result<Self, PriceError> {
        let frequency = policy.weight(frequency)?;
        if !policy.valid_base(base_price) || frequency > base_price {
            return Err(PriceError::InvalidStatistics);
        }
        Ok(Self {
            policy,
            base: base_price,
            frequency,
        })
    }

    pub(crate) fn cost(self) -> u32 {
        // Construction establishes frequency <= base, including in release.
        self.base - self.frequency
    }

    fn literal_cost(self) -> Result<u32, PriceError> {
        if self.base < SCALE {
            return Err(PriceError::InvalidStatistics);
        }
        Ok(self.cost().max(SCALE))
    }
}

/// Whole-bit `ZSTD_bitWeight`, rejecting overflow of the upstream stat + 1.
pub(crate) fn bit_weight(stat: u32) -> Result<u32, PriceError> {
    let adjusted = stat.checked_add(1).ok_or(PriceError::InvalidStatistics)?;
    Ok(adjusted.ilog2() * SCALE)
}

/// `ZSTD_fracWeight`, including the pinned debug contract before shifting.
pub(crate) fn fractional_weight(stat: u32) -> Result<u32, PriceError> {
    // highbit(stat + 1) + 8 < 31 permits at most exponent 22. Checking
    // this stricter bound also proves increment and multiplication fit u32.
    if stat > 8_388_606 {
        return Err(PriceError::InvalidStatistics);
    }
    let adjusted = stat + 1;
    let exponent = adjusted.ilog2();
    Ok(exponent * SCALE + ((adjusted << 8) >> exponent))
}

/// A finite cost never reaches the upstream unreachable-path sentinel.
pub(crate) fn add_price(left: u32, right: u32) -> Result<u32, PriceError> {
    left.checked_add(right)
        .filter(|&price| price < UNREACHABLE_PRICE)
        .ok_or(PriceError::PriceOverflow)
}

pub(crate) fn literal_length_code(length: u32) -> Result<usize, PriceError> {
    if length > BLOCK_LIMIT {
        return Err(PriceError::InvalidLength);
    }
    // A full-block literal run uses the preceding symbol plus one extra bit.
    let represented = length.min(BLOCK_LIMIT - 1);
    LL_BASE
        .partition_point(|&base| base <= represented)
        .checked_sub(1)
        .ok_or(PriceError::InvalidLength)
}

pub(crate) fn match_length_code(length: u32) -> Result<usize, PriceError> {
    if !(3..=BLOCK_LIMIT).contains(&length) {
        return Err(PriceError::InvalidLength);
    }
    ML_BASE
        .partition_point(|&base| base <= length)
        .checked_sub(1)
        .ok_or(PriceError::InvalidLength)
}

pub(crate) fn offset_code(encoded_offset: u32) -> Result<usize, PriceError> {
    if encoded_offset == 0 {
        return Err(PriceError::InvalidOffset);
    }
    Ok(encoded_offset.ilog2() as usize)
}

pub(crate) fn predefined_literals_cost(length: usize) -> Result<u32, PriceError> {
    fixed_literals_cost(length, 6 * SCALE)
}

pub(crate) fn uncompressed_literals_cost(length: usize) -> Result<u32, PriceError> {
    fixed_literals_cost(length, 8 * SCALE)
}

fn fixed_literals_cost(length: usize, unit: u32) -> Result<u32, PriceError> {
    if length > BLOCK_LIMIT as usize {
        return Err(PriceError::InvalidLength);
    }
    let length = u32::try_from(length).map_err(|_| PriceError::InvalidLength)?;
    add_price(
        0,
        length.checked_mul(unit).ok_or(PriceError::PriceOverflow)?,
    )
}

/// Borrow counts for this call only; validate every consumed scalar statistic.
/// Unused entries are not inspected and no source or table reference is retained.
pub(crate) fn dynamic_literals_cost(
    literals: &[u8],
    base_price: u32,
    frequencies: &[u32; 256],
) -> Result<u32, PriceError> {
    dynamic_literals_cost_with_policy(PricePolicy::Optimal, literals, base_price, frequencies)
}

pub(crate) fn dynamic_literals_cost_with_policy(
    policy: PricePolicy,
    literals: &[u8],
    base_price: u32,
    frequencies: &[u32; 256],
) -> Result<u32, PriceError> {
    if literals.len() > BLOCK_LIMIT as usize {
        return Err(PriceError::InvalidLength);
    }
    let mut cost = 0;
    for &literal in literals {
        // A byte converts to an index within the typed 256-entry array.
        let price =
            SymbolPrice::new_with_policy(policy, base_price, frequencies[usize::from(literal)])?;
        cost = add_price(cost, price.literal_cost()?)?;
    }
    Ok(cost)
}

/// `None` selects predefined prices; dynamic prices use the selected LL symbol.
pub(crate) fn literal_length_price(
    length: u32,
    dynamic_symbol: Option<SymbolPrice>,
) -> Result<u32, PriceError> {
    literal_length_price_with_policy(PricePolicy::Optimal, length, dynamic_symbol)
}

pub(crate) fn literal_length_price_with_policy(
    policy: PricePolicy,
    length: u32,
    dynamic_symbol: Option<SymbolPrice>,
) -> Result<u32, PriceError> {
    let code = literal_length_code(length)?;
    match dynamic_symbol {
        None => policy.weight(length),
        Some(symbol) => {
            if symbol.policy != policy {
                return Err(PriceError::InvalidStatistics);
            }
            let extra = u32::from(*LL_BITS.get(code).ok_or(PriceError::InvalidLength)?) * SCALE;
            let price = add_price(extra, symbol.cost())?;
            add_price(price, if length == BLOCK_LIMIT { SCALE } else { 0 })
        }
    }
}

/// Dynamic costs contain offset and ML symbol prices, in that order.
/// Encoded offsets are sequence intents, not physical source distances.
pub(crate) fn match_price(
    encoded_offset: u32,
    length: u32,
    dynamic_symbols: Option<(SymbolPrice, SymbolPrice)>,
) -> Result<u32, PriceError> {
    match_price_with_policy(
        PricePolicy::Optimal,
        encoded_offset,
        length,
        dynamic_symbols,
    )
}

pub(crate) fn match_price_with_policy(
    policy: PricePolicy,
    encoded_offset: u32,
    length: u32,
    dynamic_symbols: Option<(SymbolPrice, SymbolPrice)>,
) -> Result<u32, PriceError> {
    let offset = offset_code(encoded_offset)?;
    let code = match_length_code(length)?;
    let offset_bits = u32::try_from(offset).map_err(|_| PriceError::InvalidOffset)?;
    match dynamic_symbols {
        None => add_price(policy.weight(length - 3)?, (16 + offset_bits) * SCALE),
        Some((off_symbol, ml_symbol)) => {
            if off_symbol.policy != policy || ml_symbol.policy != policy {
                return Err(PriceError::InvalidStatistics);
            }
            let price = add_price(offset_bits * SCALE, off_symbol.cost())?;
            let penalty = if policy == PricePolicy::Optimal {
                offset_bits.saturating_sub(19) * 2 * SCALE
            } else {
                0
            };
            let price = add_price(price, penalty)?;
            let extra = u32::from(*ML_BITS.get(code).ok_or(PriceError::InvalidLength)?) * SCALE;
            let price = add_price(price, add_price(extra, ml_symbol.cost())?)?;
            // Both coupled policies retain the integer heuristic: 256 / 5 = 51.
            add_price(price, SCALE / 5)
        }
    }
}
