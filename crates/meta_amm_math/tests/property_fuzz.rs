use meta_amm_math::{
    apply_post_fill_to_side, buy_base_with_quote, cpmm_quote_exact_in, mul_div_floor,
    reference_quote_exact_in, sell_base_for_quote, CpmmReserves, MathError, PiecewiseBookSide,
    PostFillPolicy, Q64x64, ReferenceQuoteParams, ReferenceQuoteState,
};
use proptest::prelude::*;

fn reference_params() -> ReferenceQuoteParams {
    ReferenceQuoteParams {
        fee_bps: 30,
        base_half_spread_bps: 10,
        aging_start_slots: 5,
        protected_start_slots: 10,
        expire_slots: 15,
        aging_surcharge_bps_per_slot: 2,
        max_aging_surcharge_bps: 20,
        max_trade_base_atoms: 25_000,
        protected_max_trade_base_atoms: 2_500,
        inventory_skew_bps_per_10k_imbalance: 1_000,
        max_inventory_skew_bps: 500,
        hard_inventory_band_bps: 3_000,
    }
}

fn piecewise_prices(mid: u64) -> [Q64x64; 7] {
    [
        Q64x64::from_int(mid),
        Q64x64::from_int(mid + 1),
        Q64x64::from_int(mid + 2),
        Q64x64::from_int(mid + 3),
        Q64x64::from_int(mid + 4),
        Q64x64::from_int(mid + 5),
        Q64x64::from_int(mid + 6),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 20_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn mul_div_floor_matches_u128_fast_path(
        a in 0u128..(1u128 << 60),
        b in 0u128..(1u128 << 60),
        denominator in 1u128..(1u128 << 60),
    ) {
        prop_assert_eq!(mul_div_floor(a, b, denominator).unwrap(), (a * b) / denominator);
    }

    #[test]
    fn cpmm_quote_preserves_or_increases_k_after_fee(
        base in 1_000u64..1_000_000_000_000,
        quote in 1_000u64..1_000_000_000_000,
        amount in 1u64..100_000_000,
        fee_bps in 0u16..1_000,
        base_to_quote in any::<bool>(),
    ) {
        let reserves = CpmmReserves { base, quote };
        let result = cpmm_quote_exact_in(reserves, amount, base_to_quote, fee_bps);
        if let Ok(result) = result {
            let k_before = (base as u128) * (quote as u128);
            let k_after = (result.new_base as u128) * (result.new_quote as u128);
            prop_assert!(k_after >= k_before);
            prop_assert!(result.amount_in_less_fee <= amount);
            prop_assert!(result.amount_out > 0);
            if base_to_quote {
                prop_assert_eq!(result.new_base, base.checked_add(amount).unwrap());
                prop_assert!(result.new_quote < quote);
            } else {
                prop_assert_eq!(result.new_quote, quote.checked_add(amount).unwrap());
                prop_assert!(result.new_base < base);
            }
        }
    }

    #[test]
    fn reference_quote_success_respects_accounting_and_hard_band(
        base_inventory in 900_000u64..1_100_000,
        quote_inventory in 25_000_000_000u64..35_000_000_000,
        target_base_inventory in 950_000u64..1_050_000,
        amount in 1u64..20_000,
        now_slot in 10u64..24,
        base_to_quote in any::<bool>(),
    ) {
        let params = reference_params();
        let state = ReferenceQuoteState {
            base_inventory,
            quote_inventory,
            target_base_inventory,
            mid_price: Q64x64::from_int(30_000),
            mid_publish_slot: 10,
            now_slot,
            paused: false,
        };
        let amount_in = if base_to_quote {
            amount
        } else {
            Q64x64::from_int(30_000).mul_amount_floor(amount).unwrap()
        };

        match reference_quote_exact_in(state, params, amount_in, base_to_quote) {
            Ok(quote) => {
                prop_assert!(quote.amount_in_less_fee <= amount_in);
                prop_assert!(quote.amount_out > 0);
                prop_assert!(quote.applied_spread_bps >= params.base_half_spread_bps);
                let imbalance = ((quote.new_base_inventory as i128)
                    - (target_base_inventory as i128))
                    .abs()
                    * 10_000
                    / (target_base_inventory as i128);
                prop_assert!(imbalance <= params.hard_inventory_band_bps as i128);
                if base_to_quote {
                    prop_assert_eq!(quote.new_base_inventory, base_inventory + amount_in);
                    prop_assert_eq!(quote.new_quote_inventory, quote_inventory - quote.amount_out);
                } else {
                    prop_assert_eq!(quote.new_base_inventory, base_inventory - quote.amount_out);
                    prop_assert_eq!(quote.new_quote_inventory, quote_inventory + amount_in);
                }
            }
            Err(MathError::StaleQuote | MathError::QuoteProtected | MathError::InventoryBand | MathError::InvalidAmount) => {}
            Err(error) => return Err(TestCaseError::fail(format!("unexpected error: {error:?}"))),
        }
    }

    #[test]
    fn piecewise_fills_and_post_fill_keep_side_valid(
        mid in 2u64..1_000,
        total_quantity in 600u64..60_000,
        fill_quantity in 1u64..10_000,
        policy_selector in 0u8..4,
        base_to_quote in any::<bool>(),
    ) {
        let mut side = PiecewiseBookSide::new(piecewise_prices(mid), total_quantity).unwrap();
        let policy = match policy_selector {
            0 => PostFillPolicy::None,
            1 => PostFillPolicy::HealThenAdd,
            2 => PostFillPolicy::ProportionalScale,
            _ => PostFillPolicy::DepthProportional,
        };

        let quote_result = if base_to_quote {
            sell_base_for_quote(&mut side, fill_quantity.min(total_quantity))
        } else {
            let quote_budget = Q64x64::from_int(mid + 6)
                .mul_amount_floor(fill_quantity.min(total_quantity))
                .unwrap()
                .saturating_add(1);
            buy_base_with_quote(&mut side, quote_budget)
        };

        match quote_result {
            Ok(quote) => {
                prop_assert!(quote.amount_out > 0);
                prop_assert!(quote.segments_crossed > 0);
                prop_assert!(side.consumed_quantity <= side.total_quantity);
                apply_post_fill_to_side(
                    &mut side,
                    quote.amount_out.min(total_quantity),
                    policy,
                    quote.segments_crossed,
                ).unwrap();
                prop_assert!(side.consumed_quantity <= side.total_quantity);
                side.validate().unwrap();
            }
            Err(MathError::InvalidAmount) => {}
            Err(error) => return Err(TestCaseError::fail(format!("unexpected error: {error:?}"))),
        }
    }
}
