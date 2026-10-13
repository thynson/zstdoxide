//! Scalar price conformance before retained frequency ownership integration.
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "../src/constants.rs"]
mod constants;
#[path = "support/optimal_price_fixture.rs"]
mod fixture;
#[path = "../src/optimal_price.rs"]
mod price;

use price::{PriceError, SymbolPrice};

#[test]
fn every_captured_query_matches_checked_scalar_prices() {
    let cases = fixture::read(fixture::PACK).unwrap();
    let mut queries = 0;
    for case in cases {
        for observation in case.observations {
            let Some(expected) = observation.query else {
                continue;
            };
            let [
                _,
                offset,
                length,
                literal_length,
                encoded_offset,
                match_length,
            ] = observation.operation;
            let literals = &case.source[offset as usize..(offset + length) as usize];
            let raw = if case.disabled_literals {
                price::uncompressed_literals_cost(literals.len())
            } else if observation.predefined {
                price::predefined_literals_cost(literals.len())
            } else {
                price::dynamic_literals_cost(
                    literals,
                    observation.base_prices[0],
                    observation.frequencies[..256].try_into().unwrap(),
                )
            }
            .unwrap();
            let ll = price::literal_length_code(literal_length).unwrap();
            let ml = price::match_length_code(match_length).unwrap();
            let off = price::offset_code(encoded_offset).unwrap();
            let ll_symbol = SymbolPrice::new(
                observation.base_prices[1],
                observation.frequencies[256 + ll],
            )
            .unwrap();
            let ml_symbol = SymbolPrice::new(
                observation.base_prices[2],
                observation.frequencies[292 + ml],
            )
            .unwrap();
            let off_symbol = SymbolPrice::new(
                observation.base_prices[3],
                observation.frequencies[345 + off],
            )
            .unwrap();
            let ll_price = price::literal_length_price(
                literal_length,
                (!observation.predefined).then_some(ll_symbol),
            )
            .unwrap();
            let match_price = price::match_price(
                encoded_offset,
                match_length,
                (!observation.predefined).then_some((off_symbol, ml_symbol)),
            )
            .unwrap();
            assert_eq!(
                [raw, ll_price, match_price],
                expected,
                "{} {:?}",
                case.name,
                observation.operation
            );
            queries += 1;
        }
    }
    assert_eq!(queries, 1420);
}

#[test]
fn invalid_lengths_offsets_statistics_and_sentinels_are_rejected() {
    assert_eq!(
        price::bit_weight(u32::MAX),
        Err(PriceError::InvalidStatistics)
    );
    assert_eq!(
        price::literal_length_code(131_073),
        Err(PriceError::InvalidLength)
    );
    for length in [0, 1, 2, 131_073, u32::MAX] {
        assert_eq!(
            price::match_length_code(length),
            Err(PriceError::InvalidLength)
        );
        assert_eq!(
            price::match_price(1, length, None),
            Err(PriceError::InvalidLength)
        );
    }
    assert_eq!(price::offset_code(0), Err(PriceError::InvalidOffset));
    assert_eq!(
        price::match_price(0, 3, None),
        Err(PriceError::InvalidOffset)
    );
    for (base, frequency) in [(1, 0), (8192, 0), (0, 1), (256, 3), (256, u32::MAX)] {
        assert_eq!(
            SymbolPrice::new(base, frequency),
            Err(PriceError::InvalidStatistics)
        );
    }
    assert_eq!(
        price::add_price((1 << 30) - 1, 1),
        Err(PriceError::PriceOverflow)
    );
    assert_eq!(
        price::add_price(u32::MAX, 1),
        Err(PriceError::PriceOverflow)
    );
    assert_eq!(
        price::predefined_literals_cost(131_073),
        Err(PriceError::InvalidLength)
    );
    assert_eq!(
        price::uncompressed_literals_cost(usize::MAX),
        Err(PriceError::InvalidLength)
    );
    assert_eq!(
        price::dynamic_literals_cost(&vec![0; 131_073], 256, &[0; 256]),
        Err(PriceError::InvalidLength)
    );
    assert_eq!(
        price::dynamic_literals_cost(&[0], 0, &[0; 256]),
        Err(PriceError::InvalidStatistics)
    );
    assert_eq!(
        price::dynamic_literals_cost(&[0], 256, &[u32::MAX; 256]),
        Err(PriceError::InvalidStatistics)
    );
}

#[test]
fn empty_literals_and_largest_finite_scalars_keep_their_contracts() {
    assert_eq!(
        price::dynamic_literals_cost(&[], 0, &[u32::MAX; 256]),
        Ok(0)
    );
    assert_eq!(price::bit_weight(u32::MAX - 1), Ok(31 * 256));
    assert_eq!(price::offset_code(u32::MAX), Ok(31));
    assert_eq!(price::literal_length_code(131_072), Ok(35));
    assert_eq!(price::match_length_code(131_072), Ok(52));
    assert_eq!(price::dynamic_literals_cost(&[7], 256, &[1; 256]), Ok(256));
    assert_eq!(price::uncompressed_literals_cost(131_072), Ok(268_435_456));
    assert_eq!(price::add_price((1 << 30) - 1, 0), Ok((1 << 30) - 1));
}
