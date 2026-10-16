//! Coupled scalar policy conformance against complete independent/native rows.
#![forbid(unsafe_code)]
#[allow(dead_code)]
#[path = "../src/constants.rs"]
mod constants;
#[path = "support/ultra_scalar_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../src/optimal_price.rs"]
mod price;

use price::{PriceError, PricePolicy, SymbolPrice};

fn symbol(policy: PricePolicy, sum: u32, frequency: u32) -> SymbolPrice {
    SymbolPrice::new_with_policy(policy, policy.weight(sum).unwrap(), frequency).unwrap()
}

fn ll(policy: PricePolicy, length: u32, sum: u32, frequency: u32, predefined: u32) -> u32 {
    price::literal_length_price_with_policy(
        policy,
        length,
        (predefined == 0).then(|| symbol(policy, sum, frequency)),
    )
    .unwrap()
}

#[test]
fn every_observed_whole_and_fractional_scalar_matches() {
    let rows = fixture::Fixture::read(fixture::PACK).unwrap();
    let mut count = 0;
    for row in rows.rows() {
        let row = row.unwrap();
        let [mode, tag, a, b, c, d, e, f, g, _] = row.words;
        let policy = if mode == 0 {
            PricePolicy::Optimal
        } else {
            PricePolicy::Ultra
        };
        let actual = match tag {
            0 => i64::from(policy.weight(a).unwrap()),
            1 => i64::from(ll(policy, a, b, c, d)),
            2 => i64::from(
                price::match_price_with_policy(
                    policy,
                    a,
                    b,
                    (g == 0).then(|| (symbol(policy, c, d), symbol(policy, e, f))),
                )
                .unwrap(),
            ),
            3 => {
                let value = if e == 1 {
                    price::uncompressed_literals_cost(a as usize)
                } else if f == 1 {
                    price::predefined_literals_cost(a as usize)
                } else {
                    let source: Vec<_> = (0..a).map(|i| (i % 2) as u8).collect();
                    let mut frequencies = [0; 256];
                    frequencies[0] = c;
                    frequencies[1] = d;
                    price::dynamic_literals_cost_with_policy(
                        policy,
                        &source,
                        policy.weight(b).unwrap(),
                        &frequencies,
                    )
                };
                i64::from(value.unwrap())
            }
            4 => i64::from(ll(policy, a, b, d, e)) - i64::from(ll(policy, a - 1, b, c, e)),
            _ => panic!("validated fixture tag"),
        };
        assert_eq!(actual, row.value, "row {count}: {:?}", row.words);
        count += 1;
    }
    assert_eq!(count, 2470);
}

#[test]
fn fractional_weight_matches_integer_interpolation_over_its_entire_domain() {
    // Independent wide multiplication/division within each power interval,
    // rather than production's bounded u32 shift sequence.
    for exponent in 0..23u32 {
        let unit = 1u64 << exponent;
        for adjusted in unit..2 * unit {
            let expected = exponent * 256 + (adjusted * 256 / unit) as u32;
            assert_eq!(
                price::fractional_weight((adjusted - 1) as u32),
                Ok(expected)
            );
        }
    }
    for invalid in [8_388_607, 8_388_608, u32::MAX - 1, u32::MAX] {
        assert_eq!(
            price::fractional_weight(invalid),
            Err(PriceError::InvalidStatistics)
        );
    }
}

#[test]
fn cached_fractional_bases_require_an_actual_integer_weight_witness() {
    for base in 0..=8193u32 {
        let witnessed = if (256..=6143).contains(&base) {
            let exponent = base / 256 - 1;
            let unit = 1u64 << exponent;
            let fraction = u64::from(base % 256 + 256);
            let adjusted = (fraction * unit).div_ceil(256);
            adjusted <= 8_388_607
                && u64::from(exponent) * 256 + adjusted * 256 / unit == u64::from(base)
        } else {
            false
        };
        assert_eq!(
            SymbolPrice::new_with_policy(PricePolicy::Ultra, base, 0).is_ok(),
            witnessed,
            "base {base}"
        );
    }
    assert_eq!(
        SymbolPrice::new_with_policy(PricePolicy::Ultra, 256, 1),
        Err(PriceError::InvalidStatistics)
    );
    assert_eq!(
        SymbolPrice::new_with_policy(PricePolicy::Ultra, 6143, 8_388_607),
        Err(PriceError::InvalidStatistics)
    );
    assert_eq!(
        SymbolPrice::new_with_policy(PricePolicy::Ultra, u32::MAX, 0),
        Err(PriceError::InvalidStatistics)
    );
}

#[test]
fn dynamic_queries_reject_policy_mixing_before_price_arithmetic() {
    let whole = SymbolPrice::new(256, 0).unwrap();
    let fractional = SymbolPrice::new_with_policy(PricePolicy::Ultra, 256, 0).unwrap();
    for (policy, wrong) in [
        (PricePolicy::Optimal, fractional),
        (PricePolicy::Ultra, whole),
    ] {
        assert_eq!(
            price::literal_length_price_with_policy(policy, 0, Some(wrong)),
            Err(PriceError::InvalidStatistics)
        );
        for pair in [(whole, fractional), (fractional, whole), (wrong, wrong)] {
            assert_eq!(
                price::match_price_with_policy(policy, 1, 3, Some(pair)),
                Err(PriceError::InvalidStatistics)
            );
        }
    }
    assert_eq!(
        price::literal_length_price(0, Some(fractional)),
        Err(PriceError::InvalidStatistics)
    );
    assert_eq!(
        price::match_price(1, 3, Some((fractional, fractional))),
        Err(PriceError::InvalidStatistics)
    );
}

#[test]
fn fractional_literals_keep_zero_read_clamp_and_length_contracts() {
    let mut frequencies = [u32::MAX; 256];
    assert_eq!(
        price::dynamic_literals_cost_with_policy(PricePolicy::Ultra, &[], 0, &frequencies),
        Ok(0)
    );
    frequencies[7] = 17;
    let base = PricePolicy::Ultra.weight(17).unwrap();
    assert_eq!(
        price::dynamic_literals_cost_with_policy(PricePolicy::Ultra, &[7, 7], base, &frequencies),
        Ok(512)
    );
    assert_eq!(
        price::dynamic_literals_cost_with_policy(PricePolicy::Ultra, &[8], base, &frequencies),
        Err(PriceError::InvalidStatistics)
    );
    assert_eq!(
        price::dynamic_literals_cost_with_policy(PricePolicy::Ultra, &[7], 257, &frequencies),
        Err(PriceError::InvalidStatistics)
    );
    assert_eq!(
        price::dynamic_literals_cost_with_policy(
            PricePolicy::Ultra,
            &vec![7; 131_073],
            base,
            &frequencies
        ),
        Err(PriceError::InvalidLength)
    );
    assert_eq!(
        price::literal_length_price_with_policy(PricePolicy::Ultra, 131_073, None),
        Err(PriceError::InvalidLength)
    );
    for length in [0, 1, 2, 131_073, u32::MAX] {
        assert_eq!(
            price::match_price_with_policy(PricePolicy::Ultra, 1, length, None),
            Err(PriceError::InvalidLength)
        );
    }
    assert_eq!(
        price::match_price_with_policy(PricePolicy::Ultra, 0, 3, None),
        Err(PriceError::InvalidOffset)
    );
}

#[test]
fn ultra_removes_only_the_dynamic_large_offset_handicap() {
    for code in 0..32u32 {
        let whole = symbol(PricePolicy::Optimal, 31, 7);
        let fractional = symbol(PricePolicy::Ultra, 31, 7);
        let a = price::match_price(1 << code, 3, Some((whole, whole))).unwrap();
        let b = price::match_price_with_policy(
            PricePolicy::Ultra,
            1 << code,
            3,
            Some((fractional, fractional)),
        )
        .unwrap();
        assert_eq!(a - b, code.saturating_sub(19) * 512);
        assert_eq!(b % 256, 51);
    }
}
