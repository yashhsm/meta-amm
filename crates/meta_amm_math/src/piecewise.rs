//! Bounded piecewise-price curve primitives.
//!
//! A side stores seven ascending price points over six equal base-quantity
//! segments. Ask-side fills walk low to high; bid-side fills walk high to low.

use crate::errors::MathError;
use crate::fixed::{mul_div_floor, Q64x64};

pub const PIECEWISE_PRICE_POINT_COUNT: usize = 7;
pub const PIECEWISE_SEGMENT_COUNT: usize = PIECEWISE_PRICE_POINT_COUNT - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostFillPolicy {
    None,
    HealThenAdd,
    ProportionalScale,
    DepthProportional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiecewiseBookSide {
    pub prices: [Q64x64; PIECEWISE_PRICE_POINT_COUNT],
    pub total_quantity: u64,
    pub consumed_quantity: u64,
}

impl PiecewiseBookSide {
    pub fn new(
        prices: [Q64x64; PIECEWISE_PRICE_POINT_COUNT],
        total_quantity: u64,
    ) -> Result<Self, MathError> {
        let side = Self {
            prices,
            total_quantity,
            consumed_quantity: 0,
        };
        side.validate()?;
        Ok(side)
    }

    pub fn validate(self) -> Result<(), MathError> {
        if self.total_quantity < PIECEWISE_SEGMENT_COUNT as u64
            || self.consumed_quantity > self.total_quantity
        {
            return Err(MathError::InvalidConfig);
        }
        if self.prices[0].0 == 0 {
            return Err(MathError::InvalidConfig);
        }
        for index in 1..PIECEWISE_PRICE_POINT_COUNT {
            if self.prices[index].0 <= self.prices[index - 1].0 {
                return Err(MathError::InvalidConfig);
            }
        }
        Ok(())
    }

    pub fn remaining_quantity(self) -> u64 {
        self.total_quantity.saturating_sub(self.consumed_quantity)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PiecewiseQuote {
    pub amount_in_used: u64,
    pub amount_out: u64,
    pub new_consumed_quantity: u64,
    pub segments_crossed: u8,
}

pub fn buy_base_with_quote(
    ask_side: &mut PiecewiseBookSide,
    quote_amount_in: u64,
) -> Result<PiecewiseQuote, MathError> {
    ask_side.validate()?;
    if quote_amount_in == 0 || ask_side.remaining_quantity() == 0 {
        return Err(MathError::InvalidAmount);
    }

    let mut quote_remaining = quote_amount_in;
    let mut quote_used = 0u64;
    let mut base_out = 0u64;
    let mut segments_crossed = 0u8;

    while quote_remaining > 0 && ask_side.remaining_quantity() > 0 {
        let segment = segment_from_consumed(*ask_side);
        let segment_start = segment_start(ask_side.total_quantity, segment);
        let segment_end = segment_end(ask_side.total_quantity, segment);
        let offset = ask_side.consumed_quantity.saturating_sub(segment_start);
        let remaining_in_segment = segment_end.saturating_sub(ask_side.consumed_quantity);
        if remaining_in_segment == 0 {
            return Err(MathError::InvalidConfig);
        }

        let start_price = ask_price_at_offset(*ask_side, segment, offset)?;
        let full_end_price = ask_price_at_offset(
            *ask_side,
            segment,
            offset.saturating_add(remaining_in_segment),
        )?;
        let full_cost = linear_quote_amount(start_price, full_end_price, remaining_in_segment)?;

        let (base_take, cost) = if full_cost > 0 && full_cost <= quote_remaining {
            (remaining_in_segment, full_cost)
        } else {
            let base_take = max_base_for_quote(
                *ask_side,
                segment,
                offset,
                remaining_in_segment,
                quote_remaining,
                true,
            )?;
            if base_take == 0 {
                break;
            }
            let end_price =
                ask_price_at_offset(*ask_side, segment, offset.saturating_add(base_take))?;
            (
                base_take,
                linear_quote_amount(start_price, end_price, base_take)?,
            )
        };

        if cost == 0 {
            break;
        }

        ask_side.consumed_quantity = ask_side
            .consumed_quantity
            .checked_add(base_take)
            .ok_or(MathError::Overflow)?;
        quote_remaining = quote_remaining
            .checked_sub(cost)
            .ok_or(MathError::Underflow)?;
        quote_used = quote_used.checked_add(cost).ok_or(MathError::Overflow)?;
        base_out = base_out.checked_add(base_take).ok_or(MathError::Overflow)?;
        segments_crossed = segments_crossed.saturating_add(1);
    }

    if base_out == 0 || quote_remaining > 0 {
        return Err(MathError::InvalidAmount);
    }

    Ok(PiecewiseQuote {
        amount_in_used: quote_used,
        amount_out: base_out,
        new_consumed_quantity: ask_side.consumed_quantity,
        segments_crossed,
    })
}

pub fn sell_base_for_quote(
    bid_side: &mut PiecewiseBookSide,
    base_amount_in: u64,
) -> Result<PiecewiseQuote, MathError> {
    bid_side.validate()?;
    if base_amount_in == 0
        || bid_side.remaining_quantity() == 0
        || base_amount_in > bid_side.remaining_quantity()
    {
        return Err(MathError::InvalidAmount);
    }

    let mut base_remaining = base_amount_in;
    let mut quote_out = 0u64;
    let mut segments_crossed = 0u8;

    while base_remaining > 0 {
        let segment_from_best = segment_from_consumed(*bid_side);
        let actual_segment = PIECEWISE_SEGMENT_COUNT - 1 - segment_from_best;
        let segment_start = segment_start(bid_side.total_quantity, segment_from_best);
        let segment_end = segment_end(bid_side.total_quantity, segment_from_best);
        let offset = bid_side.consumed_quantity.saturating_sub(segment_start);
        let remaining_in_segment = segment_end.saturating_sub(bid_side.consumed_quantity);
        if remaining_in_segment == 0 {
            return Err(MathError::InvalidConfig);
        }

        let base_take = base_remaining.min(remaining_in_segment);
        let start_price = bid_price_at_offset(*bid_side, actual_segment, offset)?;
        let end_price =
            bid_price_at_offset(*bid_side, actual_segment, offset.saturating_add(base_take))?;
        let segment_quote_out = linear_quote_amount(start_price, end_price, base_take)?;
        if segment_quote_out == 0 {
            return Err(MathError::InvalidAmount);
        }

        bid_side.consumed_quantity = bid_side
            .consumed_quantity
            .checked_add(base_take)
            .ok_or(MathError::Overflow)?;
        base_remaining = base_remaining
            .checked_sub(base_take)
            .ok_or(MathError::Underflow)?;
        quote_out = quote_out
            .checked_add(segment_quote_out)
            .ok_or(MathError::Overflow)?;
        segments_crossed = segments_crossed.saturating_add(1);
    }

    Ok(PiecewiseQuote {
        amount_in_used: base_amount_in,
        amount_out: quote_out,
        new_consumed_quantity: bid_side.consumed_quantity,
        segments_crossed,
    })
}

pub fn apply_post_fill_to_side(
    side: &mut PiecewiseBookSide,
    replenishment_quantity: u64,
    policy: PostFillPolicy,
    segments_crossed: u8,
) -> Result<(), MathError> {
    side.validate()?;
    if replenishment_quantity == 0 || policy == PostFillPolicy::None {
        return Ok(());
    }

    match policy {
        PostFillPolicy::None => Ok(()),
        PostFillPolicy::HealThenAdd => heal_then_add(side, replenishment_quantity),
        PostFillPolicy::ProportionalScale => {
            let old_total = side.total_quantity;
            let new_total = old_total
                .checked_add(replenishment_quantity)
                .ok_or(MathError::Overflow)?;
            let new_consumed = mul_div_floor(
                side.consumed_quantity as u128,
                new_total as u128,
                old_total as u128,
            )?;
            side.total_quantity = new_total;
            side.consumed_quantity =
                u64::try_from(new_consumed).map_err(|_| MathError::Overflow)?;
            side.validate()
        }
        PostFillPolicy::DepthProportional => {
            let depth = segments_crossed.min(PIECEWISE_SEGMENT_COUNT as u8) as u128;
            let add_quantity = ((replenishment_quantity as u128) * depth
                / (PIECEWISE_SEGMENT_COUNT as u128)) as u64;
            let heal_quantity = replenishment_quantity
                .checked_sub(add_quantity)
                .ok_or(MathError::Underflow)?;
            heal_then_add(side, heal_quantity)?;
            side.total_quantity = side
                .total_quantity
                .checked_add(add_quantity)
                .ok_or(MathError::Overflow)?;
            side.validate()
        }
    }
}

fn heal_then_add(side: &mut PiecewiseBookSide, quantity: u64) -> Result<(), MathError> {
    if quantity <= side.consumed_quantity {
        side.consumed_quantity -= quantity;
    } else {
        let add_quantity = quantity - side.consumed_quantity;
        side.consumed_quantity = 0;
        side.total_quantity = side
            .total_quantity
            .checked_add(add_quantity)
            .ok_or(MathError::Overflow)?;
    }
    side.validate()
}

fn segment_from_consumed(side: PiecewiseBookSide) -> usize {
    if side.consumed_quantity >= side.total_quantity {
        return PIECEWISE_SEGMENT_COUNT - 1;
    }
    for segment in 0..PIECEWISE_SEGMENT_COUNT {
        if side.consumed_quantity < segment_end(side.total_quantity, segment) {
            return segment;
        }
    }
    PIECEWISE_SEGMENT_COUNT - 1
}

fn segment_start(total_quantity: u64, segment: usize) -> u64 {
    ((total_quantity as u128) * (segment as u128) / (PIECEWISE_SEGMENT_COUNT as u128)) as u64
}

fn segment_end(total_quantity: u64, segment: usize) -> u64 {
    if segment == PIECEWISE_SEGMENT_COUNT - 1 {
        total_quantity
    } else {
        ((total_quantity as u128) * ((segment + 1) as u128) / (PIECEWISE_SEGMENT_COUNT as u128))
            as u64
    }
}

fn ask_price_at_offset(
    side: PiecewiseBookSide,
    segment: usize,
    offset: u64,
) -> Result<Q64x64, MathError> {
    interpolate_price(
        side.prices[segment],
        side.prices[segment + 1],
        offset,
        segment_end(side.total_quantity, segment) - segment_start(side.total_quantity, segment),
    )
}

fn bid_price_at_offset(
    side: PiecewiseBookSide,
    actual_segment: usize,
    offset: u64,
) -> Result<Q64x64, MathError> {
    interpolate_price(
        side.prices[actual_segment + 1],
        side.prices[actual_segment],
        offset,
        segment_end(
            side.total_quantity,
            PIECEWISE_SEGMENT_COUNT - 1 - actual_segment,
        ) - segment_start(
            side.total_quantity,
            PIECEWISE_SEGMENT_COUNT - 1 - actual_segment,
        ),
    )
}

fn interpolate_price(
    start: Q64x64,
    end: Q64x64,
    offset: u64,
    segment_quantity: u64,
) -> Result<Q64x64, MathError> {
    if segment_quantity == 0 {
        return Err(MathError::InvalidConfig);
    }
    if offset == 0 {
        return Ok(start);
    }
    if start <= end {
        let delta = end.0.checked_sub(start.0).ok_or(MathError::Underflow)?;
        Ok(Q64x64(
            start
                .0
                .checked_add(mul_div_floor(
                    delta,
                    offset as u128,
                    segment_quantity as u128,
                )?)
                .ok_or(MathError::Overflow)?,
        ))
    } else {
        let delta = start.0.checked_sub(end.0).ok_or(MathError::Underflow)?;
        Ok(Q64x64(
            start
                .0
                .checked_sub(mul_div_floor(
                    delta,
                    offset as u128,
                    segment_quantity as u128,
                )?)
                .ok_or(MathError::Underflow)?,
        ))
    }
}

fn linear_quote_amount(
    start_price: Q64x64,
    end_price: Q64x64,
    base_quantity: u64,
) -> Result<u64, MathError> {
    if base_quantity == 0 {
        return Ok(0);
    }
    let average_price = Q64x64(
        start_price
            .0
            .checked_add(end_price.0)
            .ok_or(MathError::Overflow)?
            / 2,
    );
    average_price.mul_amount_floor(base_quantity)
}

fn max_base_for_quote(
    side: PiecewiseBookSide,
    segment: usize,
    offset: u64,
    capacity: u64,
    quote_available: u64,
    ask_side: bool,
) -> Result<u64, MathError> {
    let mut low = 0u64;
    let mut high = capacity;
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        let start_price = if ask_side {
            ask_price_at_offset(side, segment, offset)?
        } else {
            bid_price_at_offset(side, segment, offset)?
        };
        let end_price = if ask_side {
            ask_price_at_offset(side, segment, offset.saturating_add(mid))?
        } else {
            bid_price_at_offset(side, segment, offset.saturating_add(mid))?
        };
        let cost = linear_quote_amount(start_price, end_price, mid)?;
        if cost > 0 && cost <= quote_available {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    Ok(low)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prices() -> [Q64x64; PIECEWISE_PRICE_POINT_COUNT] {
        [
            Q64x64::from_int(98),
            Q64x64::from_int(99),
            Q64x64::from_int(100),
            Q64x64::from_int(101),
            Q64x64::from_int(102),
            Q64x64::from_int(103),
            Q64x64::from_int(104),
        ]
    }

    fn side() -> PiecewiseBookSide {
        PiecewiseBookSide::new(prices(), 600).unwrap()
    }

    #[test]
    fn validates_strictly_ascending_price_points() {
        assert!(PiecewiseBookSide::new(prices(), 600).is_ok());

        let mut bad = prices();
        bad[3] = bad[2];
        assert_eq!(
            PiecewiseBookSide::new(bad, 600),
            Err(MathError::InvalidConfig)
        );
    }

    #[test]
    fn ask_fill_walks_low_to_high() {
        let mut ask = side();
        let quote = buy_base_with_quote(&mut ask, 9_850).unwrap();

        assert_eq!(quote.amount_out, 100);
        assert_eq!(quote.amount_in_used, 9_850);
        assert_eq!(quote.new_consumed_quantity, 100);
        assert_eq!(quote.segments_crossed, 1);
    }

    #[test]
    fn bid_fill_walks_high_to_low() {
        let mut bid = side();
        let quote = sell_base_for_quote(&mut bid, 100).unwrap();

        assert_eq!(quote.amount_in_used, 100);
        assert_eq!(quote.amount_out, 10_350);
        assert_eq!(quote.new_consumed_quantity, 100);
        assert_eq!(quote.segments_crossed, 1);
    }

    #[test]
    fn exact_in_rejects_when_book_cannot_consume_full_input() {
        let mut ask = side();

        assert_eq!(
            buy_base_with_quote(&mut ask, 1_000_000),
            Err(MathError::InvalidAmount)
        );
    }

    #[test]
    fn heal_then_add_restores_top_liquidity_before_growing_depth() {
        let mut ask = side();
        buy_base_with_quote(&mut ask, 9_850).unwrap();

        apply_post_fill_to_side(&mut ask, 40, PostFillPolicy::HealThenAdd, 1).unwrap();
        assert_eq!(ask.consumed_quantity, 60);
        assert_eq!(ask.total_quantity, 600);

        apply_post_fill_to_side(&mut ask, 100, PostFillPolicy::HealThenAdd, 1).unwrap();
        assert_eq!(ask.consumed_quantity, 0);
        assert_eq!(ask.total_quantity, 640);
    }

    #[test]
    fn proportional_scale_preserves_consumed_fraction() {
        let mut ask = side();
        buy_base_with_quote(&mut ask, 9_850).unwrap();

        apply_post_fill_to_side(&mut ask, 600, PostFillPolicy::ProportionalScale, 1).unwrap();
        assert_eq!(ask.total_quantity, 1_200);
        assert_eq!(ask.consumed_quantity, 200);
    }

    #[test]
    fn depth_proportional_replenishes_farther_after_deeper_fills() {
        let mut shallow = side();
        let mut deep = side();
        shallow.consumed_quantity = 120;
        deep.consumed_quantity = 120;

        apply_post_fill_to_side(&mut shallow, 60, PostFillPolicy::DepthProportional, 1).unwrap();
        apply_post_fill_to_side(&mut deep, 60, PostFillPolicy::DepthProportional, 6).unwrap();

        assert!(shallow.consumed_quantity < deep.consumed_quantity);
        assert!(deep.total_quantity > shallow.total_quantity);
    }
}
