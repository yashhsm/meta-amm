//! Constant-product quote math.

use crate::errors::MathError;
use crate::fixed::Q64x64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpmmReserves {
    pub base: u64,
    pub quote: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpmmQuote {
    pub amount_in_less_fee: u64,
    pub amount_out: u64,
    pub new_base: u64,
    pub new_quote: u64,
    pub effective_price: Q64x64,
}

pub fn quote_exact_in(
    reserves: CpmmReserves,
    amount_in: u64,
    base_to_quote: bool,
    fee_bps: u16,
) -> Result<CpmmQuote, MathError> {
    if reserves.base == 0 || reserves.quote == 0 {
        return Err(MathError::EmptyLiquidity);
    }
    if amount_in == 0 {
        return Err(MathError::InvalidAmount);
    }
    if fee_bps >= 10_000 {
        return Err(MathError::InvalidFee);
    }

    let amount_in_less_fee = ((amount_in as u128) * ((10_000 - fee_bps) as u128) / 10_000) as u64;
    if amount_in_less_fee == 0 {
        return Err(MathError::InvalidAmount);
    }

    let (in_reserve, out_reserve) = if base_to_quote {
        (reserves.base, reserves.quote)
    } else {
        (reserves.quote, reserves.base)
    };

    let denominator = (in_reserve as u128)
        .checked_add(amount_in_less_fee as u128)
        .ok_or(MathError::Overflow)?;
    let amount_out = ((out_reserve as u128) * (amount_in_less_fee as u128) / denominator) as u64;
    if amount_out == 0 || amount_out >= out_reserve {
        return Err(MathError::InvalidAmount);
    }

    let (new_base, new_quote, effective_price) = if base_to_quote {
        let new_base = reserves
            .base
            .checked_add(amount_in)
            .ok_or(MathError::Overflow)?;
        let new_quote = reserves
            .quote
            .checked_sub(amount_out)
            .ok_or(MathError::Underflow)?;
        let price = Q64x64::from_ratio(amount_out as u128, amount_in as u128)?;
        (new_base, new_quote, price)
    } else {
        let new_quote = reserves
            .quote
            .checked_add(amount_in)
            .ok_or(MathError::Overflow)?;
        let new_base = reserves
            .base
            .checked_sub(amount_out)
            .ok_or(MathError::Underflow)?;
        let price = Q64x64::from_ratio(amount_in as u128, amount_out as u128)?;
        (new_base, new_quote, price)
    };

    Ok(CpmmQuote {
        amount_in_less_fee,
        amount_out,
        new_base,
        new_quote,
        effective_price,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_base_to_quote_preserves_or_increases_k_after_fee() {
        let reserves = CpmmReserves {
            base: 1_000_000,
            quote: 30_000_000_000,
        };
        let quote = quote_exact_in(reserves, 1_000, true, 30).unwrap();
        let k_before = (reserves.base as u128) * (reserves.quote as u128);
        let k_after = (quote.new_base as u128) * (quote.new_quote as u128);
        assert!(k_after >= k_before);
        assert!(quote.amount_out > 0);
    }

    #[test]
    fn quote_quote_to_base_has_quote_per_base_effective_price() {
        let reserves = CpmmReserves {
            base: 1_000_000,
            quote: 30_000_000_000,
        };
        let quote = quote_exact_in(reserves, 30_000_000, false, 30).unwrap();
        assert!(quote.amount_out > 0);
        assert!(quote.effective_price.to_int_floor() > 29_000);
    }

    #[test]
    fn rejects_invalid_inputs() {
        let reserves = CpmmReserves { base: 1, quote: 1 };
        assert_eq!(
            quote_exact_in(reserves, 0, true, 30),
            Err(MathError::InvalidAmount)
        );
        assert_eq!(
            quote_exact_in(reserves, 1, true, 10_000),
            Err(MathError::InvalidFee)
        );
        assert_eq!(
            quote_exact_in(CpmmReserves { base: 0, quote: 1 }, 1, true, 30),
            Err(MathError::EmptyLiquidity)
        );
    }
}
