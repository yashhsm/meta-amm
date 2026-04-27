//! Reference-price quote math with explicit quote-age states.

use crate::errors::MathError;
use crate::fixed::{mul_div_floor, Q64x64};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteAgeState {
    Fresh,
    Aging,
    Protected,
    Expired,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuoteState {
    pub base_inventory: u64,
    pub quote_inventory: u64,
    pub target_base_inventory: u64,
    pub mid_price: Q64x64,
    pub mid_publish_slot: u64,
    pub now_slot: u64,
    pub paused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuoteParams {
    pub fee_bps: u16,
    pub base_half_spread_bps: u16,
    pub aging_start_slots: u64,
    pub protected_start_slots: u64,
    pub expire_slots: u64,
    pub aging_surcharge_bps_per_slot: u16,
    pub max_aging_surcharge_bps: u16,
    pub max_trade_base_atoms: u64,
    pub protected_max_trade_base_atoms: u64,
    pub inventory_skew_bps_per_10k_imbalance: u16,
    pub max_inventory_skew_bps: u16,
    pub hard_inventory_band_bps: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuote {
    pub age_state: QuoteAgeState,
    pub amount_in_less_fee: u64,
    pub amount_out: u64,
    pub new_base_inventory: u64,
    pub new_quote_inventory: u64,
    pub effective_price: Q64x64,
    pub applied_spread_bps: u16,
    pub inventory_imbalance_bps: i32,
}

pub fn quote_age_state(
    state: ReferenceQuoteState,
    params: ReferenceQuoteParams,
) -> Result<QuoteAgeState, MathError> {
    validate_params(params)?;
    if state.paused {
        return Ok(QuoteAgeState::Paused);
    }

    let age = state.now_slot.saturating_sub(state.mid_publish_slot);
    if age >= params.expire_slots {
        Ok(QuoteAgeState::Expired)
    } else if age >= params.protected_start_slots {
        Ok(QuoteAgeState::Protected)
    } else if age >= params.aging_start_slots {
        Ok(QuoteAgeState::Aging)
    } else {
        Ok(QuoteAgeState::Fresh)
    }
}

pub fn quote_exact_in(
    state: ReferenceQuoteState,
    params: ReferenceQuoteParams,
    amount_in: u64,
    base_to_quote: bool,
) -> Result<ReferenceQuote, MathError> {
    validate_params(params)?;
    if amount_in == 0 {
        return Err(MathError::InvalidAmount);
    }
    if state.base_inventory == 0 || state.quote_inventory == 0 {
        return Err(MathError::EmptyLiquidity);
    }
    if state.target_base_inventory == 0 {
        return Err(MathError::InvalidConfig);
    }

    let age_state = quote_age_state(state, params)?;
    match age_state {
        QuoteAgeState::Paused => return Err(MathError::PoolPaused),
        QuoteAgeState::Expired => return Err(MathError::StaleQuote),
        QuoteAgeState::Fresh | QuoteAgeState::Aging | QuoteAgeState::Protected => {}
    }

    let base_equivalent = base_equivalent_amount(state.mid_price, amount_in, base_to_quote)?;
    let max_base_size = max_base_trade_for_age(state, params, age_state);
    if base_equivalent == 0 || base_equivalent > max_base_size {
        return Err(if age_state == QuoteAgeState::Protected {
            MathError::QuoteProtected
        } else {
            MathError::InvalidAmount
        });
    }

    let amount_in_less_fee = amount_less_fee(amount_in, params.fee_bps)?;
    let current_inventory_imbalance_bps =
        inventory_imbalance_bps(state.base_inventory, state.target_base_inventory);
    let inventory_skew_bps = inventory_skew_bps(current_inventory_imbalance_bps, params);
    let aging_surcharge_bps = aging_surcharge_bps(state, params);
    let applied_spread_bps = params
        .base_half_spread_bps
        .saturating_add(aging_surcharge_bps);

    let effective_price = effective_price(
        state.mid_price,
        applied_spread_bps,
        inventory_skew_bps,
        base_to_quote,
    )?;

    let (amount_out, new_base_inventory, new_quote_inventory) = if base_to_quote {
        let amount_out = effective_price.mul_amount_floor(amount_in_less_fee)?;
        if amount_out == 0 || amount_out >= state.quote_inventory {
            return Err(MathError::InvalidAmount);
        }
        (
            amount_out,
            state
                .base_inventory
                .checked_add(amount_in)
                .ok_or(MathError::Overflow)?,
            state
                .quote_inventory
                .checked_sub(amount_out)
                .ok_or(MathError::Underflow)?,
        )
    } else {
        let amount_out = effective_price.div_amount_floor(amount_in_less_fee)?;
        if amount_out == 0 || amount_out >= state.base_inventory {
            return Err(MathError::InvalidAmount);
        }
        (
            amount_out,
            state
                .base_inventory
                .checked_sub(amount_out)
                .ok_or(MathError::Underflow)?,
            state
                .quote_inventory
                .checked_add(amount_in)
                .ok_or(MathError::Overflow)?,
        )
    };

    let new_imbalance = inventory_imbalance_bps(new_base_inventory, state.target_base_inventory);
    if abs_i32(new_imbalance) > params.hard_inventory_band_bps as i32 {
        return Err(MathError::InventoryBand);
    }

    Ok(ReferenceQuote {
        age_state,
        amount_in_less_fee,
        amount_out,
        new_base_inventory,
        new_quote_inventory,
        effective_price,
        applied_spread_bps,
        inventory_imbalance_bps: current_inventory_imbalance_bps,
    })
}

fn validate_params(params: ReferenceQuoteParams) -> Result<(), MathError> {
    if params.fee_bps >= 10_000
        || params.base_half_spread_bps >= 10_000
        || params.max_aging_surcharge_bps >= 10_000
        || params.inventory_skew_bps_per_10k_imbalance >= 10_000
        || params.max_inventory_skew_bps >= 10_000
    {
        return Err(MathError::InvalidConfig);
    }
    if params.aging_start_slots > params.protected_start_slots
        || params.protected_start_slots >= params.expire_slots
        || params.expire_slots == 0
        || params.max_trade_base_atoms == 0
        || params.protected_max_trade_base_atoms == 0
        || params.protected_max_trade_base_atoms > params.max_trade_base_atoms
    {
        return Err(MathError::InvalidConfig);
    }
    Ok(())
}

fn amount_less_fee(amount: u64, fee_bps: u16) -> Result<u64, MathError> {
    if fee_bps >= 10_000 {
        return Err(MathError::InvalidFee);
    }
    let result = ((amount as u128) * ((10_000 - fee_bps) as u128) / 10_000) as u64;
    if result == 0 {
        return Err(MathError::InvalidAmount);
    }
    Ok(result)
}

fn base_equivalent_amount(
    mid_price: Q64x64,
    amount_in: u64,
    base_to_quote: bool,
) -> Result<u64, MathError> {
    if base_to_quote {
        Ok(amount_in)
    } else {
        mid_price.div_amount_floor(amount_in)
    }
}

fn max_base_trade_for_age(
    state: ReferenceQuoteState,
    params: ReferenceQuoteParams,
    age_state: QuoteAgeState,
) -> u64 {
    match age_state {
        QuoteAgeState::Fresh => params.max_trade_base_atoms,
        QuoteAgeState::Protected => params.protected_max_trade_base_atoms,
        QuoteAgeState::Aging => {
            let age = state.now_slot.saturating_sub(state.mid_publish_slot);
            let aging_span = params
                .protected_start_slots
                .saturating_sub(params.aging_start_slots)
                .max(1);
            let age_into_aging = age.saturating_sub(params.aging_start_slots).min(aging_span);
            let decay = params
                .max_trade_base_atoms
                .saturating_sub(params.protected_max_trade_base_atoms);
            params.max_trade_base_atoms.saturating_sub(
                ((decay as u128) * (age_into_aging as u128) / (aging_span as u128)) as u64,
            )
        }
        QuoteAgeState::Expired | QuoteAgeState::Paused => 0,
    }
}

fn aging_surcharge_bps(state: ReferenceQuoteState, params: ReferenceQuoteParams) -> u16 {
    let age = state.now_slot.saturating_sub(state.mid_publish_slot);
    let slots = age.saturating_sub(params.aging_start_slots);
    let surcharge = (slots as u128) * (params.aging_surcharge_bps_per_slot as u128);
    surcharge.min(params.max_aging_surcharge_bps as u128) as u16
}

fn inventory_imbalance_bps(base_inventory: u64, target_base_inventory: u64) -> i32 {
    if target_base_inventory == 0 {
        return 0;
    }
    let delta = (base_inventory as i128) - (target_base_inventory as i128);
    let bps = delta.saturating_mul(10_000) / (target_base_inventory as i128);
    bps.clamp(i32::MIN as i128, i32::MAX as i128) as i32
}

fn inventory_skew_bps(imbalance_bps: i32, params: ReferenceQuoteParams) -> i32 {
    let raw = (imbalance_bps as i128).saturating_mul(
        params.inventory_skew_bps_per_10k_imbalance as i128,
    ) / 10_000;
    let cap = params.max_inventory_skew_bps as i128;
    raw.clamp(-cap, cap) as i32
}

fn effective_price(
    mid_price: Q64x64,
    spread_bps: u16,
    inventory_skew_bps: i32,
    base_to_quote: bool,
) -> Result<Q64x64, MathError> {
    let factor_bps = if base_to_quote {
        10_000i32 - (spread_bps as i32) - inventory_skew_bps
    } else {
        10_000i32 + (spread_bps as i32) - inventory_skew_bps
    };
    if factor_bps <= 0 {
        return Err(MathError::InvalidConfig);
    }
    Ok(Q64x64(mul_div_floor(
        mid_price.0,
        factor_bps as u128,
        10_000,
    )?))
}

fn abs_i32(value: i32) -> i32 {
    if value < 0 {
        value.saturating_neg()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_state(now_slot: u64) -> ReferenceQuoteState {
        ReferenceQuoteState {
            base_inventory: 1_000_000,
            quote_inventory: 30_000_000_000,
            target_base_inventory: 1_000_000,
            mid_price: Q64x64::from_int(30_000),
            mid_publish_slot: 10,
            now_slot,
            paused: false,
        }
    }

    fn params() -> ReferenceQuoteParams {
        ReferenceQuoteParams {
            fee_bps: 30,
            base_half_spread_bps: 10,
            aging_start_slots: 5,
            protected_start_slots: 10,
            expire_slots: 15,
            aging_surcharge_bps_per_slot: 2,
            max_aging_surcharge_bps: 20,
            max_trade_base_atoms: 10_000,
            protected_max_trade_base_atoms: 1_000,
            inventory_skew_bps_per_10k_imbalance: 1_000,
            max_inventory_skew_bps: 500,
            hard_inventory_band_bps: 3_000,
        }
    }

    #[test]
    fn age_state_transitions_are_explicit() {
        assert_eq!(
            quote_age_state(base_state(10), params()).unwrap(),
            QuoteAgeState::Fresh
        );
        assert_eq!(
            quote_age_state(base_state(15), params()).unwrap(),
            QuoteAgeState::Aging
        );
        assert_eq!(
            quote_age_state(base_state(20), params()).unwrap(),
            QuoteAgeState::Protected
        );
        assert_eq!(
            quote_age_state(base_state(25), params()).unwrap(),
            QuoteAgeState::Expired
        );
    }

    #[test]
    fn fresh_quote_executes_and_updates_inventory() {
        let quote = quote_exact_in(base_state(10), params(), 1_000, true).unwrap();
        assert_eq!(quote.age_state, QuoteAgeState::Fresh);
        assert!(quote.amount_out > 0);
        assert!(quote.new_base_inventory > 1_000_000);
        assert!(quote.new_quote_inventory < 30_000_000_000);
    }

    #[test]
    fn aging_quote_surcharges_and_decays_size() {
        let fresh = quote_exact_in(base_state(10), params(), 1_000, true).unwrap();
        let aging = quote_exact_in(base_state(17), params(), 1_000, true).unwrap();
        assert_eq!(aging.age_state, QuoteAgeState::Aging);
        assert!(aging.applied_spread_bps > fresh.applied_spread_bps);
        assert!(aging.amount_out < fresh.amount_out);
    }

    #[test]
    fn protected_quote_rejects_large_trade() {
        assert_eq!(
            quote_exact_in(base_state(20), params(), 2_000, true),
            Err(MathError::QuoteProtected)
        );
        assert!(quote_exact_in(base_state(20), params(), 500, true).is_ok());
    }

    #[test]
    fn expired_and_paused_quotes_reject() {
        assert_eq!(
            quote_exact_in(base_state(25), params(), 500, true),
            Err(MathError::StaleQuote)
        );
        let mut paused = base_state(10);
        paused.paused = true;
        assert_eq!(
            quote_exact_in(paused, params(), 500, true),
            Err(MathError::PoolPaused)
        );
    }

    #[test]
    fn imbalance_saturates_instead_of_wrapping_for_tiny_target() {
        // Previously `((delta * 10_000) / target) as i32` truncated to i32
        // and could wrap into the hard-band window. With saturating math the
        // imbalance pins to i32::MAX and the post-trade band check rejects.
        let mut state = base_state(10);
        state.target_base_inventory = 1;
        state.base_inventory = u64::MAX / 2;
        state.quote_inventory = 1_000_000_000_000;

        assert!(matches!(
            quote_exact_in(state, params(), 1, true),
            Err(MathError::InvalidAmount) | Err(MathError::InventoryBand)
        ));

        // Direct check: the bps value saturates, not wraps.
        let bps = super::inventory_imbalance_bps(state.base_inventory, state.target_base_inventory);
        assert_eq!(bps, i32::MAX);
    }

    #[test]
    fn hard_inventory_band_blocks_one_sided_flow() {
        let mut state = base_state(10);
        state.base_inventory = 1_290_000;
        assert_eq!(
            quote_exact_in(state, params(), 20_000, true),
            Err(MathError::InvalidAmount)
        );

        let mut tight = params();
        tight.max_trade_base_atoms = 50_000;
        assert_eq!(
            quote_exact_in(state, tight, 20_000, true),
            Err(MathError::InventoryBand)
        );
    }
}
