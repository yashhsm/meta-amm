#![allow(unexpected_cfgs)]
#![forbid(unsafe_code)]

use anchor_lang::prelude::*;
use meta_amm_config::{
    compile_reference_quote_pool_config_account, AccountBudget, ReferenceQuoteConfigInput,
    ReferenceQuoteStrategyConfig, SameSlotUpdateOrder, TokenPairIdentity,
    REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN,
};
use meta_amm_math::ReferenceQuoteParams;

declare_id!("CzVBvCUvx8RWEsiRybEAtr6TwydEn9WByXG7WTezGsq1");

pub const POOL_CONFIG_SEED: &[u8] = b"pool-config";

#[program]
pub mod meta_amm {
    use super::*;

    pub fn initialize_reference_quote_pool(
        ctx: Context<InitializeReferenceQuotePool>,
        args: InitializeReferenceQuotePoolArgs,
    ) -> Result<()> {
        let pool_config = &mut ctx.accounts.pool_config;
        let token_pair = TokenPairIdentity {
            base_mint: ctx.accounts.base_mint.key().to_bytes(),
            quote_mint: ctx.accounts.quote_mint.key().to_bytes(),
            base_token_program: ctx.accounts.base_token_program.key().to_bytes(),
            quote_token_program: ctx.accounts.quote_token_program.key().to_bytes(),
            base_decimals: args.base_decimals,
            quote_decimals: args.quote_decimals,
        };
        let account_budget = args.account_budget.to_account_budget();
        let input = ReferenceQuoteConfigInput {
            strategy: args.strategy_config()?,
            token_pair,
            account_budget,
        };
        let layout =
            compile_reference_quote_pool_config_account(input, ctx.bumps.pool_config, false)
                .map_err(|_| error!(MetaAmmError::InvalidReferenceQuoteConfig))?;

        pool_config.authority = ctx.accounts.authority.key();
        pool_config.base_mint = ctx.accounts.base_mint.key();
        pool_config.quote_mint = ctx.accounts.quote_mint.key();
        pool_config.base_token_program = ctx.accounts.base_token_program.key();
        pool_config.quote_token_program = ctx.accounts.quote_token_program.key();
        pool_config.bump = ctx.bumps.pool_config;
        pool_config.paused = false;
        pool_config.layout = layout.to_bytes();

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(args: InitializeReferenceQuotePoolArgs)]
pub struct InitializeReferenceQuotePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    /// CHECK: The first program slice stores mint identity only; mint account parsing lands with vault custody.
    pub base_mint: UncheckedAccount<'info>,
    /// CHECK: The first program slice stores mint identity only; mint account parsing lands with vault custody.
    pub quote_mint: UncheckedAccount<'info>,
    /// CHECK: Stored as part of canonical token-pair identity for decimal-scale preimage.
    pub base_token_program: UncheckedAccount<'info>,
    /// CHECK: Stored as part of canonical token-pair identity for decimal-scale preimage.
    pub quote_token_program: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        space = 8 + ReferenceQuotePoolConfig::INIT_SPACE,
        seeds = [
            POOL_CONFIG_SEED,
            authority.key().as_ref(),
            base_mint.key().as_ref(),
            quote_mint.key().as_ref(),
        ],
        bump
    )]
    pub pool_config: Account<'info, ReferenceQuotePoolConfig>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct ReferenceQuotePoolConfig {
    pub authority: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub base_token_program: Pubkey,
    pub quote_token_program: Pubkey,
    pub bump: u8,
    pub paused: bool,
    pub layout: [u8; REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitializeReferenceQuotePoolArgs {
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub params: ReferenceQuoteParamsArgs,
    pub quote_update_envelope: QuoteUpdateEnvelopeArgs,
    pub account_budget: AccountBudgetArgs,
}

impl InitializeReferenceQuotePoolArgs {
    fn strategy_config(self) -> Result<ReferenceQuoteStrategyConfig> {
        Ok(ReferenceQuoteStrategyConfig {
            params: self.params.to_params(),
            quote_update_envelope: self.quote_update_envelope.to_envelope()?,
        })
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceQuoteParamsArgs {
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

impl ReferenceQuoteParamsArgs {
    fn to_params(self) -> ReferenceQuoteParams {
        ReferenceQuoteParams {
            fee_bps: self.fee_bps,
            base_half_spread_bps: self.base_half_spread_bps,
            aging_start_slots: self.aging_start_slots,
            protected_start_slots: self.protected_start_slots,
            expire_slots: self.expire_slots,
            aging_surcharge_bps_per_slot: self.aging_surcharge_bps_per_slot,
            max_aging_surcharge_bps: self.max_aging_surcharge_bps,
            max_trade_base_atoms: self.max_trade_base_atoms,
            protected_max_trade_base_atoms: self.protected_max_trade_base_atoms,
            inventory_skew_bps_per_10k_imbalance: self.inventory_skew_bps_per_10k_imbalance,
            max_inventory_skew_bps: self.max_inventory_skew_bps,
            hard_inventory_band_bps: self.hard_inventory_band_bps,
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteUpdateEnvelopeArgs {
    pub max_maker_update_period_slots: u64,
    pub max_landing_latency_slots: u64,
    pub min_update_success_probability_bps: u16,
    pub same_slot_order: u8,
}

impl QuoteUpdateEnvelopeArgs {
    fn to_envelope(self) -> Result<meta_amm_config::QuoteUpdateEnvelope> {
        let same_slot_order = match self.same_slot_order {
            0 => SameSlotUpdateOrder::UpdateBeforeSwap,
            1 => SameSlotUpdateOrder::SwapBeforeUpdate,
            _ => return err!(MetaAmmError::InvalidSameSlotUpdateOrder),
        };
        Ok(meta_amm_config::QuoteUpdateEnvelope {
            max_maker_update_period_slots: self.max_maker_update_period_slots,
            max_landing_latency_slots: self.max_landing_latency_slots,
            min_update_success_probability_bps: self.min_update_success_probability_bps,
            same_slot_order,
        })
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountBudgetArgs {
    pub required_swap_account_metas: u8,
    pub max_swap_account_metas: u8,
}

impl AccountBudgetArgs {
    fn to_account_budget(self) -> AccountBudget {
        AccountBudget {
            required_swap_account_metas: self.required_swap_account_metas,
            max_swap_account_metas: self.max_swap_account_metas,
        }
    }
}

#[error_code]
pub enum MetaAmmError {
    #[msg("ReferenceQuote config failed bounded compiler validation")]
    InvalidReferenceQuoteConfig,
    #[msg("same_slot_order must be 0 update_before_swap or 1 swap_before_update")]
    InvalidSameSlotUpdateOrder,
}

#[cfg(test)]
mod tests {
    use super::*;
    use meta_amm_config::{
        REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET,
        REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS,
    };

    fn default_args() -> InitializeReferenceQuotePoolArgs {
        InitializeReferenceQuotePoolArgs {
            base_decimals: 8,
            quote_decimals: 6,
            params: ReferenceQuoteParamsArgs {
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
            },
            quote_update_envelope: QuoteUpdateEnvelopeArgs {
                max_maker_update_period_slots: 4,
                max_landing_latency_slots: 3,
                min_update_success_probability_bps: 7_400,
                same_slot_order: 1,
            },
            account_budget: AccountBudgetArgs {
                required_swap_account_metas: REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS,
                max_swap_account_metas: REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET,
            },
        }
    }

    #[test]
    fn init_args_compile_to_reference_quote_strategy_config() {
        let strategy = default_args().strategy_config().unwrap();

        assert_eq!(strategy.params.fee_bps, 30);
        assert_eq!(
            strategy.quote_update_envelope.same_slot_order,
            SameSlotUpdateOrder::SwapBeforeUpdate
        );
    }

    #[test]
    fn rejects_unknown_same_slot_order_discriminant() {
        let mut args = default_args();
        args.quote_update_envelope.same_slot_order = 2;

        assert!(args.strategy_config().is_err());
    }

    #[test]
    fn anchor_account_space_preserves_compiled_layout_bytes() {
        assert_eq!(
            ReferenceQuotePoolConfig::INIT_SPACE,
            32 * 5 + 1 + 1 + REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN
        );
    }
}
