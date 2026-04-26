use std::cmp::Ordering;

use meta_amm_math::{
    cpmm_quote_exact_in, mul_div_floor, CpmmReserves, DecimalScalePreimage, MathError, PriceDomain,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct U256([u64; 4]);

impl U256 {
    fn from_u128_shift_128(value: u128) -> Self {
        Self([0, 0, value as u64, (value >> 64) as u64])
    }

    fn mul_u128(left: u128, right: u128) -> Self {
        let left_limbs = [left as u64, (left >> 64) as u64];
        let right_limbs = [right as u64, (right >> 64) as u64];
        let mut out = [0u64; 4];

        for (i, left_limb) in left_limbs.iter().enumerate() {
            let mut carry = 0u128;
            for (j, right_limb) in right_limbs.iter().enumerate() {
                let index = i + j;
                let term =
                    out[index] as u128 + (*left_limb as u128) * (*right_limb as u128) + carry;
                out[index] = term as u64;
                carry = term >> 64;
            }

            let mut index = i + right_limbs.len();
            while carry != 0 {
                let term = out[index] as u128 + carry;
                out[index] = term as u64;
                carry = term >> 64;
                index += 1;
            }
        }

        Self(out)
    }
}

impl Ord for U256 {
    fn cmp(&self, other: &Self) -> Ordering {
        for index in (0..4).rev() {
            match self.0[index].cmp(&other.0[index]) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn next_u128(seed: &mut u128) -> u128 {
    *seed = seed
        .wrapping_mul(0xda942042e4dd58b5d1b54a32d192ed03)
        .wrapping_add(0x9e3779b97f4a7c15f39cc0605cedc835);
    *seed ^= *seed >> 29;
    *seed ^= *seed << 37;
    *seed ^= *seed >> 43;
    *seed
}

fn assert_mul_div_matches_wide_invariant(a: u128, b: u128, denominator: u128) {
    let product = U256::mul_u128(a, b);
    let overflow_threshold = U256::from_u128_shift_128(denominator);

    match mul_div_floor(a, b, denominator) {
        Ok(quotient) => {
            let lower_bound = U256::mul_u128(quotient, denominator);
            assert!(
                lower_bound <= product,
                "lower bound exceeds product: a={a} b={b} d={denominator} q={quotient}"
            );

            let upper_bound = if quotient == u128::MAX {
                overflow_threshold
            } else {
                U256::mul_u128(quotient + 1, denominator)
            };
            assert!(
                product < upper_bound,
                "product exceeds upper bound: a={a} b={b} d={denominator} q={quotient}"
            );
        }
        Err(MathError::Overflow) => {
            assert!(
                product >= overflow_threshold,
                "unexpected overflow: a={a} b={b} d={denominator}"
            );
        }
        Err(error) => panic!("unexpected error {error:?}: a={a} b={b} d={denominator}"),
    }
}

#[test]
fn mul_div_matches_independent_wide_invariant_on_edge_cases() {
    let cases = [
        (0, u128::MAX, 1),
        (1, u128::MAX, 1),
        (u128::MAX, 1, 1),
        (u128::MAX, u128::MAX, u128::MAX),
        (u128::MAX, u128::MAX, 1),
        (1u128 << 127, 2, 3),
        (
            (1u128 << 96) + 123,
            (1u128 << 95) + 456,
            (1u128 << 80) + 789,
        ),
        (u128::MAX / 7, u128::MAX / 11, u128::MAX / 13),
    ];

    for (a, b, denominator) in cases {
        assert_mul_div_matches_wide_invariant(a, b, denominator);
    }
}

#[test]
fn mul_div_matches_independent_wide_invariant_on_random_inputs() {
    let mut seed = 0x6501_f00d_dead_beef_cafe_babe_1234_5678u128;
    for _ in 0..50_000 {
        let a = next_u128(&mut seed);
        let b = next_u128(&mut seed);
        let denominator = next_u128(&mut seed).max(1);
        assert_mul_div_matches_wide_invariant(a, b, denominator);
    }
}

#[test]
fn mul_div_zero_denominator_is_rejected() {
    assert_eq!(mul_div_floor(1, 2, 0), Err(MathError::DivByZero));
}

#[test]
fn decimal_preimage_matches_golden_hex_fixture() {
    let preimage = DecimalScalePreimage {
        base_mint: [1; 32],
        quote_mint: [2; 32],
        base_token_program: [3; 32],
        quote_token_program: [4; 32],
        base_decimals: 8,
        quote_decimals: 6,
        price_domain: PriceDomain::RawQuoteAtomsPerRawBaseAtom,
    };

    let expected = decode_hex(include_str!(
        "../../../tests/golden/decimal-scale-preimage.hex"
    ));
    assert_eq!(preimage.to_bytes().as_slice(), expected.as_slice());
}

fn decode_hex(input: &str) -> Vec<u8> {
    let hex = input
        .chars()
        .filter(|character| character.is_ascii_hexdigit())
        .collect::<String>();
    assert_eq!(hex.len() % 2, 0, "hex fixture has odd length");

    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("valid hex fixture"))
        .collect()
}

#[test]
fn randomized_cpmm_quotes_preserve_constant_product_after_fee() {
    let mut seed = 0xfeed_face_c001_d00d_0123_4567_89ab_cdefu128;

    for _ in 0..20_000 {
        let base = ((next_u128(&mut seed) % 1_000_000_000_000) as u64).max(1_000);
        let quote = ((next_u128(&mut seed) % 1_000_000_000_000) as u64).max(1_000);
        let amount = ((next_u128(&mut seed) % 10_000_000) as u64).max(1);
        let fee_bps = (next_u128(&mut seed) % 1_000) as u16;
        let base_to_quote = next_u128(&mut seed) & 1 == 0;

        let reserves = CpmmReserves { base, quote };
        let Ok(result) = cpmm_quote_exact_in(reserves, amount, base_to_quote, fee_bps) else {
            continue;
        };

        let k_before = (base as u128) * (quote as u128);
        let k_after = (result.new_base as u128) * (result.new_quote as u128);
        assert!(
            k_after >= k_before,
            "k decreased: reserves={reserves:?} amount={amount} fee={fee_bps} side={base_to_quote}"
        );
        assert!(result.amount_in_less_fee <= amount);
    }
}
