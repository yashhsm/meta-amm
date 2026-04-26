#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use meta_amm_math::{
    DecimalScale, DecimalScalePreimage, MathError, PriceDomain, ReferenceQuoteParams,
};

pub const CONFIG_SCHEMA_VERSION: u16 = 1;
pub const BASIS_POINTS: u16 = 10_000;
pub const REFERENCE_QUOTE_MODE_ID: u8 = 1;
pub const REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS: u8 = 10;
pub const REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET: u8 = 12;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyMode {
    ReferenceQuote = REFERENCE_QUOTE_MODE_ID,
}

impl StrategyMode {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSlotUpdateOrder {
    UpdateBeforeSwap,
    SwapBeforeUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuoteUpdateEnvelope {
    pub max_maker_update_period_slots: u64,
    pub max_landing_latency_slots: u64,
    pub min_update_success_probability_bps: u16,
    pub same_slot_order: SameSlotUpdateOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuoteStrategyConfig {
    pub params: ReferenceQuoteParams,
    pub quote_update_envelope: QuoteUpdateEnvelope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountBudget {
    pub required_swap_account_metas: u8,
    pub max_swap_account_metas: u8,
}

impl AccountBudget {
    pub const fn reference_quote_jupiter_default() -> Self {
        Self {
            required_swap_account_metas: REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS,
            max_swap_account_metas: REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenPairIdentity {
    pub base_mint: [u8; 32],
    pub quote_mint: [u8; 32],
    pub base_token_program: [u8; 32],
    pub quote_token_program: [u8; 32],
    pub base_decimals: u8,
    pub quote_decimals: u8,
}

impl TokenPairIdentity {
    pub const fn decimal_scale_preimage(self) -> DecimalScalePreimage {
        DecimalScalePreimage {
            base_mint: self.base_mint,
            quote_mint: self.quote_mint,
            base_token_program: self.base_token_program,
            quote_token_program: self.quote_token_program,
            base_decimals: self.base_decimals,
            quote_decimals: self.quote_decimals,
            price_domain: PriceDomain::RawQuoteAtomsPerRawBaseAtom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuoteConfigInput {
    pub strategy: ReferenceQuoteStrategyConfig,
    pub token_pair: TokenPairIdentity,
    pub account_budget: AccountBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompiledReferenceQuoteConfig {
    pub schema_version: u16,
    pub mode: StrategyMode,
    pub params: ReferenceQuoteParams,
    pub quote_update_envelope: QuoteUpdateEnvelope,
    pub token_pair: TokenPairIdentity,
    pub decimal_scale_preimage: DecimalScalePreimage,
    pub account_budget: AccountBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    BpsOutOfRange(BpsField),
    SlotWindowInvalid,
    TradeLimitInvalid,
    WorstCaseSpreadInvalid,
    InventoryBandInvalid,
    QuoteUpdateEnvelopeInvalid,
    AccountBudgetInvalid,
    DecimalScaleInvalid(MathError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BpsField {
    Fee,
    BaseHalfSpread,
    AgingSurchargePerSlot,
    MaxAgingSurcharge,
    InventorySkewPer10kImbalance,
    MaxInventorySkew,
}

pub fn compile_reference_quote_config(
    input: ReferenceQuoteConfigInput,
) -> Result<CompiledReferenceQuoteConfig, ConfigError> {
    validate_reference_quote_params(input.strategy.params)?;
    validate_quote_update_envelope(
        input.strategy.quote_update_envelope,
        input.strategy.params.expire_slots,
    )?;
    validate_account_budget(input.account_budget)?;
    DecimalScale::new(
        input.token_pair.base_decimals,
        input.token_pair.quote_decimals,
    )
    .map_err(ConfigError::DecimalScaleInvalid)?;

    Ok(CompiledReferenceQuoteConfig {
        schema_version: CONFIG_SCHEMA_VERSION,
        mode: StrategyMode::ReferenceQuote,
        params: input.strategy.params,
        quote_update_envelope: input.strategy.quote_update_envelope,
        token_pair: input.token_pair,
        decimal_scale_preimage: input.token_pair.decimal_scale_preimage(),
        account_budget: input.account_budget,
    })
}

pub fn validate_reference_quote_params(params: ReferenceQuoteParams) -> Result<(), ConfigError> {
    validate_bps(BpsField::Fee, params.fee_bps)?;
    validate_bps(BpsField::BaseHalfSpread, params.base_half_spread_bps)?;
    validate_bps(
        BpsField::AgingSurchargePerSlot,
        params.aging_surcharge_bps_per_slot,
    )?;
    validate_bps(BpsField::MaxAgingSurcharge, params.max_aging_surcharge_bps)?;
    validate_bps(
        BpsField::InventorySkewPer10kImbalance,
        params.inventory_skew_bps_per_10k_imbalance,
    )?;
    validate_bps(BpsField::MaxInventorySkew, params.max_inventory_skew_bps)?;

    if params.expire_slots == 0
        || params.aging_start_slots > params.protected_start_slots
        || params.protected_start_slots >= params.expire_slots
    {
        return Err(ConfigError::SlotWindowInvalid);
    }
    if params.max_trade_base_atoms == 0
        || params.protected_max_trade_base_atoms == 0
        || params.protected_max_trade_base_atoms > params.max_trade_base_atoms
    {
        return Err(ConfigError::TradeLimitInvalid);
    }
    if params.hard_inventory_band_bps == 0 || params.hard_inventory_band_bps > BASIS_POINTS {
        return Err(ConfigError::InventoryBandInvalid);
    }

    let worst_case_spread = (params.base_half_spread_bps as u32)
        .saturating_add(params.max_aging_surcharge_bps as u32)
        .saturating_add(params.max_inventory_skew_bps as u32);
    if worst_case_spread >= BASIS_POINTS as u32 {
        return Err(ConfigError::WorstCaseSpreadInvalid);
    }

    Ok(())
}

pub fn validate_quote_update_envelope(
    envelope: QuoteUpdateEnvelope,
    expire_slots: u64,
) -> Result<(), ConfigError> {
    if envelope.max_maker_update_period_slots == 0
        || envelope.min_update_success_probability_bps == 0
        || envelope.min_update_success_probability_bps > BASIS_POINTS
        || envelope.max_landing_latency_slots >= expire_slots
    {
        return Err(ConfigError::QuoteUpdateEnvelopeInvalid);
    }

    let no_drop_refresh_age = envelope
        .max_maker_update_period_slots
        .checked_add(envelope.max_landing_latency_slots)
        .ok_or(ConfigError::QuoteUpdateEnvelopeInvalid)?;
    if no_drop_refresh_age >= expire_slots {
        return Err(ConfigError::QuoteUpdateEnvelopeInvalid);
    }

    Ok(())
}

pub fn validate_account_budget(account_budget: AccountBudget) -> Result<(), ConfigError> {
    if account_budget.required_swap_account_metas == 0
        || account_budget.max_swap_account_metas == 0
        || account_budget.required_swap_account_metas < REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS
        || account_budget.required_swap_account_metas > account_budget.max_swap_account_metas
    {
        return Err(ConfigError::AccountBudgetInvalid);
    }
    Ok(())
}

fn validate_bps(field: BpsField, value: u16) -> Result<(), ConfigError> {
    if value >= BASIS_POINTS {
        return Err(ConfigError::BpsOutOfRange(field));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
            protected_max_trade_base_atoms: 500,
            inventory_skew_bps_per_10k_imbalance: 1_000,
            max_inventory_skew_bps: 500,
            hard_inventory_band_bps: 3_000,
        }
    }

    fn token_pair() -> TokenPairIdentity {
        TokenPairIdentity {
            base_mint: [1; 32],
            quote_mint: [2; 32],
            base_token_program: [3; 32],
            quote_token_program: [4; 32],
            base_decimals: 8,
            quote_decimals: 6,
        }
    }

    fn strategy() -> ReferenceQuoteStrategyConfig {
        ReferenceQuoteStrategyConfig {
            params: params(),
            quote_update_envelope: QuoteUpdateEnvelope {
                max_maker_update_period_slots: 4,
                max_landing_latency_slots: 3,
                min_update_success_probability_bps: 7_400,
                same_slot_order: SameSlotUpdateOrder::SwapBeforeUpdate,
            },
        }
    }

    #[test]
    fn compiles_reference_quote_config_with_canonical_decimal_preimage() {
        let input = ReferenceQuoteConfigInput {
            strategy: strategy(),
            token_pair: token_pair(),
            account_budget: AccountBudget::reference_quote_jupiter_default(),
        };

        let compiled = compile_reference_quote_config(input).unwrap();

        assert_eq!(compiled.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(compiled.mode.as_u8(), REFERENCE_QUOTE_MODE_ID);
        assert_eq!(compiled.params, params());
        assert_eq!(
            compiled.decimal_scale_preimage,
            token_pair().decimal_scale_preimage()
        );
        assert_eq!(
            compiled.account_budget.max_swap_account_metas,
            REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET
        );
    }

    #[test]
    fn rejects_config_that_cannot_refresh_before_expiry_without_drops() {
        let mut config = strategy();
        config.quote_update_envelope.max_maker_update_period_slots = 12;
        config.quote_update_envelope.max_landing_latency_slots = 3;

        let err = compile_reference_quote_config(ReferenceQuoteConfigInput {
            strategy: config,
            token_pair: token_pair(),
            account_budget: AccountBudget::reference_quote_jupiter_default(),
        })
        .unwrap_err();

        assert_eq!(err, ConfigError::QuoteUpdateEnvelopeInvalid);
    }

    #[test]
    fn rejects_spread_stack_that_can_cross_zero_price() {
        let mut config = strategy();
        config.params.base_half_spread_bps = 9_400;
        config.params.max_inventory_skew_bps = 590;

        let err = compile_reference_quote_config(ReferenceQuoteConfigInput {
            strategy: config,
            token_pair: token_pair(),
            account_budget: AccountBudget::reference_quote_jupiter_default(),
        })
        .unwrap_err();

        assert_eq!(err, ConfigError::WorstCaseSpreadInvalid);
    }

    #[test]
    fn rejects_out_of_range_bps_with_stable_field_id() {
        let mut config = strategy();
        config.params.aging_surcharge_bps_per_slot = BASIS_POINTS;

        let err = compile_reference_quote_config(ReferenceQuoteConfigInput {
            strategy: config,
            token_pair: token_pair(),
            account_budget: AccountBudget::reference_quote_jupiter_default(),
        })
        .unwrap_err();

        assert_eq!(
            err,
            ConfigError::BpsOutOfRange(BpsField::AgingSurchargePerSlot)
        );
    }

    #[test]
    fn rejects_invalid_decimal_scale_and_account_budget() {
        let mut bad_pair = token_pair();
        bad_pair.base_decimals = 13;
        let decimal_err = compile_reference_quote_config(ReferenceQuoteConfigInput {
            strategy: strategy(),
            token_pair: bad_pair,
            account_budget: AccountBudget::reference_quote_jupiter_default(),
        })
        .unwrap_err();
        assert_eq!(
            decimal_err,
            ConfigError::DecimalScaleInvalid(MathError::DecimalsOutOfRange)
        );

        let budget_err = compile_reference_quote_config(ReferenceQuoteConfigInput {
            strategy: strategy(),
            token_pair: token_pair(),
            account_budget: AccountBudget {
                required_swap_account_metas: 9,
                max_swap_account_metas: 12,
            },
        })
        .unwrap_err();
        assert_eq!(budget_err, ConfigError::AccountBudgetInvalid);

        let no_headroom_err = compile_reference_quote_config(ReferenceQuoteConfigInput {
            strategy: strategy(),
            token_pair: token_pair(),
            account_budget: AccountBudget {
                required_swap_account_metas: 13,
                max_swap_account_metas: 12,
            },
        })
        .unwrap_err();
        assert_eq!(no_headroom_err, ConfigError::AccountBudgetInvalid);
    }
}
