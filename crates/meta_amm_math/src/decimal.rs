//! Decimal scaling and canonical decimal-scale serialization.

use crate::errors::MathError;
use crate::fixed::Q64x64;

pub const DECIMAL_SCALE_DOMAIN: &[u8; 25] = b"meta-amm:decimal-scale:v1";
pub const DECIMAL_SCALE_PREIMAGE_LEN: usize = DECIMAL_SCALE_DOMAIN.len() + (32 * 4) + 3;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceDomain {
    RawQuoteAtomsPerRawBaseAtom = 0,
}

impl PriceDomain {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecimalScalePreimage {
    pub base_mint: [u8; 32],
    pub quote_mint: [u8; 32],
    pub base_token_program: [u8; 32],
    pub quote_token_program: [u8; 32],
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub price_domain: PriceDomain,
}

impl DecimalScalePreimage {
    pub fn to_bytes(self) -> [u8; DECIMAL_SCALE_PREIMAGE_LEN] {
        let mut out = [0u8; DECIMAL_SCALE_PREIMAGE_LEN];
        let mut offset = 0usize;

        copy_into(&mut out, &mut offset, DECIMAL_SCALE_DOMAIN);
        copy_into(&mut out, &mut offset, &self.base_mint);
        copy_into(&mut out, &mut offset, &self.quote_mint);
        copy_into(&mut out, &mut offset, &self.base_token_program);
        copy_into(&mut out, &mut offset, &self.quote_token_program);
        out[offset] = self.base_decimals;
        offset += 1;
        out[offset] = self.quote_decimals;
        offset += 1;
        out[offset] = self.price_domain.as_u8();

        out
    }
}

fn copy_into<const N: usize>(
    out: &mut [u8; DECIMAL_SCALE_PREIMAGE_LEN],
    offset: &mut usize,
    bytes: &[u8; N],
) {
    out[*offset..*offset + N].copy_from_slice(bytes);
    *offset += N;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecimalScale {
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub factor: Q64x64,
    pub quote_ge_base: bool,
}

impl DecimalScale {
    pub const MAX_DECIMALS: u8 = 12;

    pub fn new(base_decimals: u8, quote_decimals: u8) -> Result<Self, MathError> {
        if base_decimals > Self::MAX_DECIMALS || quote_decimals > Self::MAX_DECIMALS {
            return Err(MathError::DecimalsOutOfRange);
        }

        let (diff, quote_ge_base) = if quote_decimals >= base_decimals {
            (quote_decimals - base_decimals, true)
        } else {
            (base_decimals - quote_decimals, false)
        };

        let factor = Q64x64::from_ratio(pow10(diff)?, 1)?;
        Ok(Self {
            base_decimals,
            quote_decimals,
            factor,
            quote_ge_base,
        })
    }

    pub fn human_to_atom_price(self, human_price: Q64x64) -> Result<Q64x64, MathError> {
        if self.quote_ge_base {
            human_price.checked_mul(self.factor)
        } else {
            human_price.checked_div(self.factor)
        }
    }

    pub fn atom_to_human_price(self, atom_price: Q64x64) -> Result<Q64x64, MathError> {
        if self.quote_ge_base {
            atom_price.checked_div(self.factor)
        } else {
            atom_price.checked_mul(self.factor)
        }
    }
}

fn pow10(exponent: u8) -> Result<u128, MathError> {
    let mut value = 1u128;
    for _ in 0..exponent {
        value = value.checked_mul(10).ok_or(MathError::Overflow)?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed::Q64x64;

    #[test]
    fn canonical_preimage_layout_is_stable() {
        let preimage = DecimalScalePreimage {
            base_mint: [1; 32],
            quote_mint: [2; 32],
            base_token_program: [3; 32],
            quote_token_program: [4; 32],
            base_decimals: 8,
            quote_decimals: 6,
            price_domain: PriceDomain::RawQuoteAtomsPerRawBaseAtom,
        };

        let bytes = preimage.to_bytes();
        assert_eq!(bytes.len(), DECIMAL_SCALE_PREIMAGE_LEN);
        assert_eq!(&bytes[..DECIMAL_SCALE_DOMAIN.len()], DECIMAL_SCALE_DOMAIN);
        assert_eq!(bytes[DECIMAL_SCALE_DOMAIN.len()], 1);
        assert_eq!(bytes[DECIMAL_SCALE_DOMAIN.len() + 32], 2);
        assert_eq!(bytes[DECIMAL_SCALE_DOMAIN.len() + 64], 3);
        assert_eq!(bytes[DECIMAL_SCALE_DOMAIN.len() + 96], 4);
        assert_eq!(bytes[DECIMAL_SCALE_PREIMAGE_LEN - 3], 8);
        assert_eq!(bytes[DECIMAL_SCALE_PREIMAGE_LEN - 2], 6);
        assert_eq!(bytes[DECIMAL_SCALE_PREIMAGE_LEN - 1], 0);
    }

    #[test]
    fn canonical_preimage_changes_with_mints() {
        let mut left = DecimalScalePreimage {
            base_mint: [1; 32],
            quote_mint: [2; 32],
            base_token_program: [3; 32],
            quote_token_program: [4; 32],
            base_decimals: 8,
            quote_decimals: 6,
            price_domain: PriceDomain::RawQuoteAtomsPerRawBaseAtom,
        };
        let mut right = left;
        right.base_mint[31] = 9;
        assert_ne!(left.to_bytes(), right.to_bytes());

        left.base_mint[31] = 9;
        assert_eq!(left.to_bytes(), right.to_bytes());
    }

    #[test]
    fn btc_usdc_human_atom_roundtrip() {
        let scale = DecimalScale::new(8, 6).unwrap();
        let human = Q64x64::from_int(30_000);
        let atom = scale.human_to_atom_price(human).unwrap();
        assert_eq!(atom.to_int_floor(), 300);
        assert_eq!(scale.atom_to_human_price(atom).unwrap(), human);
    }

    #[test]
    fn sol_usdc_human_atom_roundtrip_tolerates_flooring() {
        let scale = DecimalScale::new(9, 6).unwrap();
        let human = Q64x64::from_int(150);
        let atom = scale.human_to_atom_price(human).unwrap();
        let back = scale.atom_to_human_price(atom).unwrap();
        assert!((149..=150).contains(&back.to_int_floor()));
    }

    #[test]
    fn equal_decimals_preserve_price() {
        let scale = DecimalScale::new(6, 6).unwrap();
        let human = Q64x64::from_int(42);
        assert_eq!(scale.human_to_atom_price(human).unwrap(), human);
    }

    #[test]
    fn rejects_out_of_range_decimals() {
        assert_eq!(DecimalScale::new(13, 6), Err(MathError::DecimalsOutOfRange));
    }
}
