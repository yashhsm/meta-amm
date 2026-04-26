//! Q64.64 fixed-point arithmetic for raw atom-ratio prices.
//!
//! Prices are quote atoms per base atom. Human decimal scaling is handled by
//! `decimal::DecimalScale` before values enter this domain.

use crate::errors::MathError;

pub const Q64: u128 = 1u128 << 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Q64x64(pub u128);

impl Q64x64 {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(Q64);

    pub fn from_int(value: u64) -> Self {
        Self((value as u128) << 64)
    }

    pub fn from_ratio(numerator: u128, denominator: u128) -> Result<Self, MathError> {
        Ok(Self(mul_div_floor(numerator, Q64, denominator)?))
    }

    pub fn to_int_floor(self) -> u64 {
        (self.0 >> 64) as u64
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, MathError> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or(MathError::Overflow)
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, MathError> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or(MathError::Underflow)
    }

    pub fn checked_mul(self, rhs: Self) -> Result<Self, MathError> {
        Ok(Self(mul_div_floor(self.0, rhs.0, Q64)?))
    }

    pub fn checked_div(self, rhs: Self) -> Result<Self, MathError> {
        Ok(Self(mul_div_floor(self.0, Q64, rhs.0)?))
    }

    pub fn mul_amount_floor(self, amount: u64) -> Result<u64, MathError> {
        let scaled = mul_div_floor(amount as u128, self.0, Q64)?;
        u64::try_from(scaled).map_err(|_| MathError::Overflow)
    }

    pub fn div_amount_floor(self, amount: u64) -> Result<u64, MathError> {
        let scaled = mul_div_floor(amount as u128, Q64, self.0)?;
        u64::try_from(scaled).map_err(|_| MathError::Overflow)
    }
}

/// Returns floor((a * b) / denominator), checked for a u128 quotient.
pub fn mul_div_floor(a: u128, b: u128, denominator: u128) -> Result<u128, MathError> {
    if denominator == 0 {
        return Err(MathError::DivByZero);
    }

    if let Some(product) = a.checked_mul(b) {
        return Ok(product / denominator);
    }

    let (high, low) = mul_128_to_256(a, b)?;
    div_256_by_128(high, low, denominator)
}

fn mul_128_to_256(a: u128, b: u128) -> Result<(u128, u128), MathError> {
    let mask = (1u128 << 64) - 1;
    let ah = a >> 64;
    let al = a & mask;
    let bh = b >> 64;
    let bl = b & mask;

    let ll = al * bl;
    let lh = al * bh;
    let hl = ah * bl;
    let hh = ah * bh;

    let (mid, carry_mid) = lh.overflowing_add(hl);
    let mid_lo = mid & mask;
    let mid_hi = mid >> 64;

    let (low, carry_low) = ll.overflowing_add(mid_lo << 64);

    let mut high = hh.checked_add(mid_hi).ok_or(MathError::Overflow)?;
    if carry_mid {
        high = high.checked_add(1u128 << 64).ok_or(MathError::Overflow)?;
    }
    if carry_low {
        high = high.checked_add(1).ok_or(MathError::Overflow)?;
    }

    Ok((high, low))
}

fn div_256_by_128(high: u128, low: u128, denominator: u128) -> Result<u128, MathError> {
    if denominator == 0 {
        return Err(MathError::DivByZero);
    }
    if high == 0 {
        return Ok(low / denominator);
    }
    if high >= denominator {
        return Err(MathError::Overflow);
    }

    let mut remainder = high;
    let mut quotient = 0u128;

    for bit in (0..128).rev() {
        let carry = remainder >> 127;
        remainder = (remainder << 1) | ((low >> bit) & 1);
        quotient <<= 1;

        if carry == 1 || remainder >= denominator {
            if carry == 1 {
                remainder = remainder.wrapping_add(denominator.wrapping_neg());
            } else {
                remainder -= denominator;
            }
            quotient |= 1;
        }
    }

    Ok(quotient)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_u128(seed: &mut u128) -> u128 {
        *seed = seed
            .wrapping_mul(0xda942042e4dd58b5d1b54a32d192ed03)
            .wrapping_add(0x9e3779b97f4a7c15f39cc0605cedc835);
        *seed
    }

    #[test]
    fn identity_values_are_exact() {
        assert_eq!(Q64x64::ONE.to_int_floor(), 1);
        assert_eq!(
            Q64x64::from_int(7).checked_mul(Q64x64::ONE),
            Ok(Q64x64::from_int(7))
        );
        assert_eq!(
            Q64x64::from_int(7).checked_div(Q64x64::from_int(7)),
            Ok(Q64x64::ONE)
        );
    }

    #[test]
    fn ratio_and_amount_roundtrip() {
        let half = Q64x64::from_ratio(1, 2).unwrap();
        assert_eq!(half.checked_add(half), Ok(Q64x64::ONE));

        let price = Q64x64::from_int(30_000);
        assert_eq!(price.mul_amount_floor(1_000).unwrap(), 30_000_000);
        assert_eq!(price.div_amount_floor(30_000_000).unwrap(), 1_000);
    }

    #[test]
    fn mul_div_matches_u128_fast_path_reference() {
        let mut seed = 0x1234_5678_9abc_def0_0102_0304_0506_0708u128;
        for _ in 0..10_000 {
            let a = next_u128(&mut seed) & ((1u128 << 62) - 1);
            let b = next_u128(&mut seed) & ((1u128 << 62) - 1);
            let d = (next_u128(&mut seed) & ((1u128 << 63) - 1)).max(1);
            assert_eq!(mul_div_floor(a, b, d).unwrap(), (a * b) / d);
        }
    }

    #[test]
    fn mul_div_slow_path_cancels_large_factors() {
        let cases = [
            ((1u128 << 96) + 123_456_789, (1u128 << 80) + 987_654_321),
            ((1u128 << 100) + 55, (1u128 << 92) + 77),
            (u128::MAX / 3, (1u128 << 90) + 9),
        ];

        for (a, b) in cases {
            assert_eq!(mul_div_floor(a, b, b).unwrap(), a);
            assert_eq!(mul_div_floor(a, b, a).unwrap(), b);
        }
    }

    #[test]
    fn mul_div_reports_quotient_overflow() {
        assert_eq!(
            mul_div_floor(u128::MAX, u128::MAX, 1),
            Err(MathError::Overflow)
        );
    }

    #[test]
    fn division_by_zero_errors() {
        assert_eq!(
            Q64x64::ONE.checked_div(Q64x64::ZERO),
            Err(MathError::DivByZero)
        );
        assert_eq!(mul_div_floor(1, 1, 0), Err(MathError::DivByZero));
    }
}
