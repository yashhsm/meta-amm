#![allow(unexpected_cfgs)]
#![forbid(unsafe_code)]

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use meta_amm_config::{
    compile_reference_quote_pool_config_account, reference_quote_params_from_pool_config_bytes,
    AccountBudget, ReferenceQuoteConfigInput, ReferenceQuoteStrategyConfig, SameSlotUpdateOrder,
    TokenPairIdentity, REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN,
    REFERENCE_QUOTE_POOL_CONFIG_SAME_SLOT_ORDER_OFFSET,
};
use meta_amm_math::{
    reference_quote_exact_in, MathError, Q64x64, ReferenceQuoteParams,
    ReferenceQuoteState as MathReferenceQuoteState,
};

declare_id!("CzVBvCUvx8RWEsiRybEAtr6TwydEn9WByXG7WTezGsq1");

pub const POOL_CONFIG_SEED: &[u8] = b"pool-config";
pub const QUOTE_STATE_SEED: &[u8] = b"quote-state";
pub const VAULT_STATE_SEED: &[u8] = b"vault-state";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"vault-authority";
pub const BASE_VAULT_SEED: &[u8] = b"base-vault";
pub const QUOTE_VAULT_SEED: &[u8] = b"quote-vault";
pub const CUSTODY_MODEL_MAKER_OWNED: u8 = 0;

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
            base_decimals: ctx.accounts.base_mint.decimals,
            quote_decimals: ctx.accounts.quote_mint.decimals,
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

    pub fn initialize_reference_quote_state(
        ctx: Context<InitializeReferenceQuoteState>,
        args: InitializeReferenceQuoteStateArgs,
    ) -> Result<()> {
        require!(
            args.quote_authority != Pubkey::default(),
            MetaAmmError::InvalidQuoteAuthority
        );

        let same_slot_order = ctx.accounts.pool_config.reference_quote_same_slot_order()?;
        let quote_state = &mut ctx.accounts.quote_state;
        quote_state.pool_config = ctx.accounts.pool_config.key();
        quote_state.quote_authority = args.quote_authority;
        quote_state.bump = ctx.bumps.quote_state;
        quote_state.paused = false;
        quote_state.same_slot_order = same_slot_order;
        quote_state._padding = [0; 5];
        quote_state.mid_price_q64x64 = 0;
        quote_state.publish_slot = 0;
        quote_state.sequence = 0;
        quote_state.last_update_slot = 0;

        Ok(())
    }

    pub fn update_reference_quote(
        ctx: Context<UpdateReferenceQuote>,
        args: UpdateReferenceQuoteArgs,
    ) -> Result<()> {
        require_eq!(
            ctx.accounts.quote_state.same_slot_order,
            ctx.accounts.pool_config.reference_quote_same_slot_order()?,
            MetaAmmError::QuoteStateOrderMismatch
        );

        let current_slot = Clock::get()?.slot;
        ctx.accounts.quote_state.apply_update(
            ctx.accounts.pool_config.authority,
            ctx.accounts.quote_signer.key(),
            args,
            current_slot,
        )
    }

    pub fn initialize_maker_vaults(ctx: Context<InitializeMakerVaults>) -> Result<()> {
        let vault_state = &mut ctx.accounts.vault_state;
        vault_state.pool_config = ctx.accounts.pool_config.key();
        vault_state.maker_authority = ctx.accounts.pool_config.authority;
        vault_state.base_vault = ctx.accounts.base_vault.key();
        vault_state.quote_vault = ctx.accounts.quote_vault.key();
        vault_state.base_mint = ctx.accounts.base_mint.key();
        vault_state.quote_mint = ctx.accounts.quote_mint.key();
        vault_state.vault_authority_bump = ctx.bumps.vault_authority;
        vault_state.bump = ctx.bumps.vault_state;
        vault_state.custody_model = CUSTODY_MODEL_MAKER_OWNED;
        vault_state.paused = false;
        vault_state._padding = [0; 4];
        vault_state.target_base_inventory = 0;
        vault_state.reserved = [0; 32];

        Ok(())
    }

    pub fn fund_pool(ctx: Context<FundPool>, args: FundPoolArgs) -> Result<()> {
        args.validate()?;
        ctx.accounts.vault_state.assert_maker_owned_for_pool(
            ctx.accounts.pool_config.key(),
            ctx.accounts.pool_config.authority,
            ctx.accounts.base_vault.key(),
            ctx.accounts.quote_vault.key(),
        )?;

        if args.base_amount > 0 {
            transfer_checked(
                CpiContext::new(
                    ctx.accounts.base_token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.base_source.to_account_info(),
                        mint: ctx.accounts.base_mint.to_account_info(),
                        to: ctx.accounts.base_vault.to_account_info(),
                        authority: ctx.accounts.authority.to_account_info(),
                    },
                ),
                args.base_amount,
                ctx.accounts.base_mint.decimals,
            )?;
        }

        if args.quote_amount > 0 {
            transfer_checked(
                CpiContext::new(
                    ctx.accounts.quote_token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.quote_source.to_account_info(),
                        mint: ctx.accounts.quote_mint.to_account_info(),
                        to: ctx.accounts.quote_vault.to_account_info(),
                        authority: ctx.accounts.authority.to_account_info(),
                    },
                ),
                args.quote_amount,
                ctx.accounts.quote_mint.decimals,
            )?;
        }

        ctx.accounts
            .vault_state
            .apply_base_funding_to_target(ctx.accounts.base_vault.amount, args.base_amount)?;

        Ok(())
    }

    pub fn swap_exact_in(ctx: Context<SwapExactIn>, args: SwapExactInArgs) -> Result<()> {
        args.validate()?;
        ctx.accounts.quote_state.assert_usable_for_swap(
            ctx.accounts.pool_config.key(),
            ctx.accounts.pool_config.reference_quote_same_slot_order()?,
            args.expected_quote_sequence,
            Clock::get()?.slot,
        )?;
        ctx.accounts.vault_state.assert_maker_owned_for_pool(
            ctx.accounts.pool_config.key(),
            ctx.accounts.pool_config.authority,
            ctx.accounts.base_vault.key(),
            ctx.accounts.quote_vault.key(),
        )?;
        require!(
            !ctx.accounts.pool_config.paused,
            MetaAmmError::PoolConfigPaused
        );
        require!(
            ctx.accounts.vault_state.target_base_inventory > 0,
            MetaAmmError::UninitializedInventoryTarget
        );

        let now_slot = Clock::get()?.slot;
        let quote = reference_quote_exact_in(
            MathReferenceQuoteState {
                base_inventory: ctx.accounts.base_vault.amount,
                quote_inventory: ctx.accounts.quote_vault.amount,
                target_base_inventory: ctx.accounts.vault_state.target_base_inventory,
                mid_price: Q64x64(ctx.accounts.quote_state.mid_price_q64x64),
                mid_publish_slot: ctx.accounts.quote_state.publish_slot,
                now_slot,
                paused: ctx.accounts.pool_config.paused
                    || ctx.accounts.quote_state.paused
                    || ctx.accounts.vault_state.paused,
            },
            ctx.accounts.pool_config.reference_quote_params(),
            args.amount_in,
            args.base_to_quote,
        )
        .map_err(map_swap_math_error)?;

        require!(
            quote.amount_out >= args.minimum_amount_out,
            MetaAmmError::SlippageExceeded
        );

        let pool_config_key = ctx.accounts.pool_config.key();
        let vault_authority_bump = [ctx.accounts.vault_state.vault_authority_bump];
        let vault_authority_seeds: &[&[u8]] = &[
            VAULT_AUTHORITY_SEED,
            pool_config_key.as_ref(),
            &vault_authority_bump,
        ];
        let signer_seeds = &[vault_authority_seeds];

        if args.base_to_quote {
            transfer_checked(
                CpiContext::new(
                    ctx.accounts.base_token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.user_base_account.to_account_info(),
                        mint: ctx.accounts.base_mint.to_account_info(),
                        to: ctx.accounts.base_vault.to_account_info(),
                        authority: ctx.accounts.taker.to_account_info(),
                    },
                ),
                args.amount_in,
                ctx.accounts.base_mint.decimals,
            )?;
            transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.quote_token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.quote_vault.to_account_info(),
                        mint: ctx.accounts.quote_mint.to_account_info(),
                        to: ctx.accounts.user_quote_account.to_account_info(),
                        authority: ctx.accounts.vault_authority.to_account_info(),
                    },
                    signer_seeds,
                ),
                quote.amount_out,
                ctx.accounts.quote_mint.decimals,
            )?;
        } else {
            transfer_checked(
                CpiContext::new(
                    ctx.accounts.quote_token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.user_quote_account.to_account_info(),
                        mint: ctx.accounts.quote_mint.to_account_info(),
                        to: ctx.accounts.quote_vault.to_account_info(),
                        authority: ctx.accounts.taker.to_account_info(),
                    },
                ),
                args.amount_in,
                ctx.accounts.quote_mint.decimals,
            )?;
            transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.base_token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.base_vault.to_account_info(),
                        mint: ctx.accounts.base_mint.to_account_info(),
                        to: ctx.accounts.user_base_account.to_account_info(),
                        authority: ctx.accounts.vault_authority.to_account_info(),
                    },
                    signer_seeds,
                ),
                quote.amount_out,
                ctx.accounts.base_mint.decimals,
            )?;
        }

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(args: InitializeReferenceQuotePoolArgs)]
pub struct InitializeReferenceQuotePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    #[account(mint::token_program = base_token_program)]
    pub base_mint: InterfaceAccount<'info, Mint>,
    #[account(mint::token_program = quote_token_program)]
    pub quote_mint: InterfaceAccount<'info, Mint>,
    pub base_token_program: Interface<'info, TokenInterface>,
    pub quote_token_program: Interface<'info, TokenInterface>,
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
    pub pool_config: Box<Account<'info, ReferenceQuotePoolConfig>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeReferenceQuoteState<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        constraint = pool_config.authority == authority.key() @ MetaAmmError::UnauthorizedPoolAuthority
    )]
    pub authority: Signer<'info>,
    #[account(
        seeds = [
            POOL_CONFIG_SEED,
            pool_config.authority.as_ref(),
            pool_config.base_mint.as_ref(),
            pool_config.quote_mint.as_ref(),
        ],
        bump = pool_config.bump
    )]
    pub pool_config: Box<Account<'info, ReferenceQuotePoolConfig>>,
    #[account(
        init,
        payer = payer,
        space = 8 + ReferenceQuoteState::INIT_SPACE,
        seeds = [
            QUOTE_STATE_SEED,
            pool_config.key().as_ref(),
        ],
        bump
    )]
    pub quote_state: Account<'info, ReferenceQuoteState>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateReferenceQuote<'info> {
    pub quote_signer: Signer<'info>,
    #[account(
        seeds = [
            POOL_CONFIG_SEED,
            pool_config.authority.as_ref(),
            pool_config.base_mint.as_ref(),
            pool_config.quote_mint.as_ref(),
        ],
        bump = pool_config.bump
    )]
    pub pool_config: Account<'info, ReferenceQuotePoolConfig>,
    #[account(
        mut,
        seeds = [
            QUOTE_STATE_SEED,
            pool_config.key().as_ref(),
        ],
        bump = quote_state.bump,
        constraint = quote_state.pool_config == pool_config.key() @ MetaAmmError::QuoteStatePoolMismatch
    )]
    pub quote_state: Account<'info, ReferenceQuoteState>,
}

#[derive(Accounts)]
pub struct InitializeMakerVaults<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        constraint = pool_config.authority == authority.key() @ MetaAmmError::UnauthorizedPoolAuthority
    )]
    pub authority: Signer<'info>,
    #[account(
        seeds = [
            POOL_CONFIG_SEED,
            pool_config.authority.as_ref(),
            pool_config.base_mint.as_ref(),
            pool_config.quote_mint.as_ref(),
        ],
        bump = pool_config.bump
    )]
    pub pool_config: Box<Account<'info, ReferenceQuotePoolConfig>>,
    /// CHECK: PDA authority only; never stores data and only signs token CPIs through seeds.
    #[account(
        seeds = [
            VAULT_AUTHORITY_SEED,
            pool_config.key().as_ref(),
        ],
        bump
    )]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(
        mint::token_program = base_token_program,
        constraint = base_mint.key() == pool_config.base_mint @ MetaAmmError::VaultMintMismatch
    )]
    pub base_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mint::token_program = quote_token_program,
        constraint = quote_mint.key() == pool_config.quote_mint @ MetaAmmError::VaultMintMismatch
    )]
    pub quote_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        constraint = base_token_program.key() == pool_config.base_token_program @ MetaAmmError::VaultTokenProgramMismatch
    )]
    pub base_token_program: Interface<'info, TokenInterface>,
    #[account(
        constraint = quote_token_program.key() == pool_config.quote_token_program @ MetaAmmError::VaultTokenProgramMismatch
    )]
    pub quote_token_program: Interface<'info, TokenInterface>,
    #[account(
        init,
        payer = payer,
        space = 8 + VaultState::INIT_SPACE,
        seeds = [
            VAULT_STATE_SEED,
            pool_config.key().as_ref(),
        ],
        bump
    )]
    pub vault_state: Box<Account<'info, VaultState>>,
    #[account(
        init,
        payer = payer,
        token::mint = base_mint,
        token::authority = vault_authority,
        token::token_program = base_token_program,
        seeds = [
            BASE_VAULT_SEED,
            pool_config.key().as_ref(),
        ],
        bump
    )]
    pub base_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        init,
        payer = payer,
        token::mint = quote_mint,
        token::authority = vault_authority,
        token::token_program = quote_token_program,
        seeds = [
            QUOTE_VAULT_SEED,
            pool_config.key().as_ref(),
        ],
        bump
    )]
    pub quote_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FundPool<'info> {
    pub authority: Signer<'info>,
    #[account(
        seeds = [
            POOL_CONFIG_SEED,
            pool_config.authority.as_ref(),
            pool_config.base_mint.as_ref(),
            pool_config.quote_mint.as_ref(),
        ],
        bump = pool_config.bump,
        constraint = pool_config.authority == authority.key() @ MetaAmmError::UnauthorizedPoolAuthority
    )]
    pub pool_config: Box<Account<'info, ReferenceQuotePoolConfig>>,
    /// CHECK: PDA authority only; the token accounts below verify it owns both vaults.
    #[account(
        seeds = [
            VAULT_AUTHORITY_SEED,
            pool_config.key().as_ref(),
        ],
        bump = vault_state.vault_authority_bump
    )]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            VAULT_STATE_SEED,
            pool_config.key().as_ref(),
        ],
        bump = vault_state.bump,
        constraint = vault_state.pool_config == pool_config.key() @ MetaAmmError::VaultStatePoolMismatch,
        constraint = vault_state.maker_authority == authority.key() @ MetaAmmError::VaultStateAuthorityMismatch,
        constraint = vault_state.custody_model == CUSTODY_MODEL_MAKER_OWNED @ MetaAmmError::UnsupportedCustodyModel,
        constraint = !vault_state.paused @ MetaAmmError::VaultPaused
    )]
    pub vault_state: Box<Account<'info, VaultState>>,
    #[account(
        mint::token_program = base_token_program,
        constraint = base_mint.key() == pool_config.base_mint @ MetaAmmError::VaultMintMismatch
    )]
    pub base_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mint::token_program = quote_token_program,
        constraint = quote_mint.key() == pool_config.quote_mint @ MetaAmmError::VaultMintMismatch
    )]
    pub quote_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        constraint = base_token_program.key() == pool_config.base_token_program @ MetaAmmError::VaultTokenProgramMismatch
    )]
    pub base_token_program: Interface<'info, TokenInterface>,
    #[account(
        constraint = quote_token_program.key() == pool_config.quote_token_program @ MetaAmmError::VaultTokenProgramMismatch
    )]
    pub quote_token_program: Interface<'info, TokenInterface>,
    #[account(
        mut,
        token::mint = base_mint,
        token::authority = authority,
        token::token_program = base_token_program
    )]
    pub base_source: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = quote_mint,
        token::authority = authority,
        token::token_program = quote_token_program
    )]
    pub quote_source: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = base_mint,
        token::authority = vault_authority,
        token::token_program = base_token_program,
        constraint = base_vault.key() == vault_state.base_vault @ MetaAmmError::VaultAccountMismatch
    )]
    pub base_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = quote_mint,
        token::authority = vault_authority,
        token::token_program = quote_token_program,
        constraint = quote_vault.key() == vault_state.quote_vault @ MetaAmmError::VaultAccountMismatch
    )]
    pub quote_vault: Box<InterfaceAccount<'info, TokenAccount>>,
}

#[derive(Accounts)]
pub struct SwapExactIn<'info> {
    pub taker: Signer<'info>,
    #[account(
        seeds = [
            POOL_CONFIG_SEED,
            pool_config.authority.as_ref(),
            pool_config.base_mint.as_ref(),
            pool_config.quote_mint.as_ref(),
        ],
        bump = pool_config.bump
    )]
    pub pool_config: Box<Account<'info, ReferenceQuotePoolConfig>>,
    #[account(
        seeds = [
            QUOTE_STATE_SEED,
            pool_config.key().as_ref(),
        ],
        bump = quote_state.bump,
        constraint = quote_state.pool_config == pool_config.key() @ MetaAmmError::QuoteStatePoolMismatch
    )]
    pub quote_state: Box<Account<'info, ReferenceQuoteState>>,
    /// CHECK: PDA authority only; token accounts below verify it owns both vaults.
    #[account(
        seeds = [
            VAULT_AUTHORITY_SEED,
            pool_config.key().as_ref(),
        ],
        bump = vault_state.vault_authority_bump
    )]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(
        seeds = [
            VAULT_STATE_SEED,
            pool_config.key().as_ref(),
        ],
        bump = vault_state.bump,
        constraint = vault_state.pool_config == pool_config.key() @ MetaAmmError::VaultStatePoolMismatch,
        constraint = vault_state.maker_authority == pool_config.authority @ MetaAmmError::VaultStateAuthorityMismatch,
        constraint = vault_state.custody_model == CUSTODY_MODEL_MAKER_OWNED @ MetaAmmError::UnsupportedCustodyModel,
        constraint = !vault_state.paused @ MetaAmmError::VaultPaused
    )]
    pub vault_state: Box<Account<'info, VaultState>>,
    #[account(
        mint::token_program = base_token_program,
        constraint = base_mint.key() == pool_config.base_mint @ MetaAmmError::VaultMintMismatch
    )]
    pub base_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mint::token_program = quote_token_program,
        constraint = quote_mint.key() == pool_config.quote_mint @ MetaAmmError::VaultMintMismatch
    )]
    pub quote_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        constraint = base_token_program.key() == pool_config.base_token_program @ MetaAmmError::VaultTokenProgramMismatch
    )]
    pub base_token_program: Interface<'info, TokenInterface>,
    #[account(
        constraint = quote_token_program.key() == pool_config.quote_token_program @ MetaAmmError::VaultTokenProgramMismatch
    )]
    pub quote_token_program: Interface<'info, TokenInterface>,
    #[account(
        mut,
        token::mint = base_mint,
        token::authority = taker,
        token::token_program = base_token_program
    )]
    pub user_base_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = quote_mint,
        token::authority = taker,
        token::token_program = quote_token_program
    )]
    pub user_quote_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = base_mint,
        token::authority = vault_authority,
        token::token_program = base_token_program,
        constraint = base_vault.key() == vault_state.base_vault @ MetaAmmError::VaultAccountMismatch
    )]
    pub base_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        token::mint = quote_mint,
        token::authority = vault_authority,
        token::token_program = quote_token_program,
        constraint = quote_vault.key() == vault_state.quote_vault @ MetaAmmError::VaultAccountMismatch
    )]
    pub quote_vault: Box<InterfaceAccount<'info, TokenAccount>>,
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

impl ReferenceQuotePoolConfig {
    fn reference_quote_same_slot_order(&self) -> Result<u8> {
        match self.layout[REFERENCE_QUOTE_POOL_CONFIG_SAME_SLOT_ORDER_OFFSET] {
            0 => Ok(0),
            1 => Ok(1),
            _ => err!(MetaAmmError::InvalidSameSlotUpdateOrder),
        }
    }

    fn reference_quote_params(&self) -> ReferenceQuoteParams {
        reference_quote_params_from_pool_config_bytes(&self.layout)
    }
}

#[account]
#[derive(InitSpace)]
pub struct ReferenceQuoteState {
    pub pool_config: Pubkey,
    pub quote_authority: Pubkey,
    pub bump: u8,
    pub paused: bool,
    pub same_slot_order: u8,
    pub _padding: [u8; 5],
    pub mid_price_q64x64: u128,
    pub publish_slot: u64,
    pub sequence: u64,
    pub last_update_slot: u64,
}

impl ReferenceQuoteState {
    fn apply_update(
        &mut self,
        pool_authority: Pubkey,
        signer: Pubkey,
        args: UpdateReferenceQuoteArgs,
        current_slot: u64,
    ) -> Result<()> {
        require!(
            signer == pool_authority || signer == self.quote_authority,
            MetaAmmError::UnauthorizedQuoteUpdate
        );
        require!(
            args.mid_price_q64x64 > 0,
            MetaAmmError::InvalidReferenceQuotePrice
        );
        require!(
            args.sequence > self.sequence,
            MetaAmmError::NonMonotonicQuoteSequence
        );
        require!(
            args.publish_slot >= self.publish_slot,
            MetaAmmError::StaleQuotePublishSlot
        );
        require!(
            args.publish_slot <= current_slot,
            MetaAmmError::FutureQuotePublishSlot
        );

        self.mid_price_q64x64 = args.mid_price_q64x64;
        self.publish_slot = args.publish_slot;
        self.sequence = args.sequence;
        self.last_update_slot = current_slot;

        Ok(())
    }

    fn assert_usable_for_swap(
        &self,
        pool_config: Pubkey,
        expected_same_slot_order: u8,
        expected_sequence: u64,
        current_slot: u64,
    ) -> Result<()> {
        require_eq!(
            self.pool_config,
            pool_config,
            MetaAmmError::QuoteStatePoolMismatch
        );
        require!(!self.paused, MetaAmmError::QuoteStatePaused);
        require_eq!(
            self.same_slot_order,
            expected_same_slot_order,
            MetaAmmError::QuoteStateOrderMismatch
        );
        require!(
            self.mid_price_q64x64 > 0 && self.sequence > 0,
            MetaAmmError::QuoteNotInitialized
        );
        require_eq!(
            self.sequence,
            expected_sequence,
            MetaAmmError::QuoteSequenceMismatch
        );
        require!(
            self.publish_slot <= current_slot,
            MetaAmmError::FutureQuotePublishSlot
        );

        Ok(())
    }
}

#[account]
#[derive(InitSpace)]
pub struct VaultState {
    pub pool_config: Pubkey,
    pub maker_authority: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub vault_authority_bump: u8,
    pub bump: u8,
    pub custody_model: u8,
    pub paused: bool,
    pub _padding: [u8; 4],
    pub target_base_inventory: u64,
    pub reserved: [u8; 32],
}

impl VaultState {
    fn assert_maker_owned_for_pool(
        &self,
        pool_config: Pubkey,
        maker_authority: Pubkey,
        base_vault: Pubkey,
        quote_vault: Pubkey,
    ) -> Result<()> {
        require_eq!(
            self.pool_config,
            pool_config,
            MetaAmmError::VaultStatePoolMismatch
        );
        require_eq!(
            self.maker_authority,
            maker_authority,
            MetaAmmError::VaultStateAuthorityMismatch
        );
        require_eq!(
            self.base_vault,
            base_vault,
            MetaAmmError::VaultAccountMismatch
        );
        require_eq!(
            self.quote_vault,
            quote_vault,
            MetaAmmError::VaultAccountMismatch
        );
        require_eq!(
            self.custody_model,
            CUSTODY_MODEL_MAKER_OWNED,
            MetaAmmError::UnsupportedCustodyModel
        );
        require!(!self.paused, MetaAmmError::VaultPaused);

        Ok(())
    }

    fn apply_base_funding_to_target(
        &mut self,
        pre_base_vault_amount: u64,
        base_amount: u64,
    ) -> Result<()> {
        if base_amount == 0 {
            return Ok(());
        }
        if self.target_base_inventory == 0 {
            self.target_base_inventory = pre_base_vault_amount
                .checked_add(base_amount)
                .ok_or(MetaAmmError::VaultInventoryOverflow)?;
        } else {
            self.target_base_inventory = self
                .target_base_inventory
                .checked_add(base_amount)
                .ok_or(MetaAmmError::VaultInventoryOverflow)?;
        }

        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitializeReferenceQuotePoolArgs {
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct InitializeReferenceQuoteStateArgs {
    pub quote_authority: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateReferenceQuoteArgs {
    pub mid_price_q64x64: u128,
    pub publish_slot: u64,
    pub sequence: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FundPoolArgs {
    pub base_amount: u64,
    pub quote_amount: u64,
}

impl FundPoolArgs {
    fn validate(self) -> Result<()> {
        require!(
            self.base_amount > 0 || self.quote_amount > 0,
            MetaAmmError::EmptyFundingAmount
        );
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwapExactInArgs {
    pub amount_in: u64,
    pub minimum_amount_out: u64,
    pub expected_quote_sequence: u64,
    pub base_to_quote: bool,
}

impl SwapExactInArgs {
    fn validate(self) -> Result<()> {
        require!(self.amount_in > 0, MetaAmmError::InvalidSwapAmount);
        require!(
            self.minimum_amount_out > 0,
            MetaAmmError::InvalidMinimumAmountOut
        );
        require!(
            self.expected_quote_sequence > 0,
            MetaAmmError::QuoteNotInitialized
        );
        Ok(())
    }
}

fn map_swap_math_error(error: MathError) -> anchor_lang::error::Error {
    match error {
        MathError::InvalidAmount => error!(MetaAmmError::InvalidSwapAmount),
        MathError::EmptyLiquidity => error!(MetaAmmError::EmptyLiquidity),
        MathError::PoolPaused => error!(MetaAmmError::PoolConfigPaused),
        MathError::StaleQuote => error!(MetaAmmError::StaleReferenceQuote),
        MathError::QuoteProtected => error!(MetaAmmError::ProtectedQuoteTradeLimit),
        MathError::InventoryBand => error!(MetaAmmError::InventoryBandExceeded),
        MathError::InvalidConfig => error!(MetaAmmError::InvalidReferenceQuoteConfig),
        MathError::InvalidFee
        | MathError::Overflow
        | MathError::Underflow
        | MathError::DivByZero
        | MathError::DecimalsOutOfRange => error!(MetaAmmError::SwapMathFailed),
    }
}

#[error_code]
pub enum MetaAmmError {
    #[msg("ReferenceQuote config failed bounded compiler validation")]
    InvalidReferenceQuoteConfig,
    #[msg("same_slot_order must be 0 update_before_swap or 1 swap_before_update")]
    InvalidSameSlotUpdateOrder,
    #[msg("authority does not control this pool config")]
    UnauthorizedPoolAuthority,
    #[msg("quote_authority must be non-default")]
    InvalidQuoteAuthority,
    #[msg("quote signer is not authorized for this pool")]
    UnauthorizedQuoteUpdate,
    #[msg("quote mid price must be positive")]
    InvalidReferenceQuotePrice,
    #[msg("quote sequence must increase monotonically")]
    NonMonotonicQuoteSequence,
    #[msg("quote publish slot cannot move backwards")]
    StaleQuotePublishSlot,
    #[msg("quote publish slot cannot be in the future")]
    FutureQuotePublishSlot,
    #[msg("quote state belongs to a different pool config")]
    QuoteStatePoolMismatch,
    #[msg("quote state ordering metadata does not match pool config")]
    QuoteStateOrderMismatch,
    #[msg("vault mint does not match pool config")]
    VaultMintMismatch,
    #[msg("vault token program does not match pool config")]
    VaultTokenProgramMismatch,
    #[msg("vault state belongs to a different pool config")]
    VaultStatePoolMismatch,
    #[msg("vault state authority does not match pool authority")]
    VaultStateAuthorityMismatch,
    #[msg("vault token account does not match vault state")]
    VaultAccountMismatch,
    #[msg("vault custody model is not supported by this instruction")]
    UnsupportedCustodyModel,
    #[msg("vault is paused")]
    VaultPaused,
    #[msg("funding amount must include at least one positive side")]
    EmptyFundingAmount,
    #[msg("pool config is paused")]
    PoolConfigPaused,
    #[msg("quote state is paused")]
    QuoteStatePaused,
    #[msg("quote state is not initialized")]
    QuoteNotInitialized,
    #[msg("quote sequence does not match the swap request")]
    QuoteSequenceMismatch,
    #[msg("swap amount must be positive")]
    InvalidSwapAmount,
    #[msg("pool has empty liquidity")]
    EmptyLiquidity,
    #[msg("minimum amount out must be positive")]
    InvalidMinimumAmountOut,
    #[msg("swap output is below minimum amount out")]
    SlippageExceeded,
    #[msg("vault inventory target is not initialized")]
    UninitializedInventoryTarget,
    #[msg("vault inventory overflow")]
    VaultInventoryOverflow,
    #[msg("reference quote is stale")]
    StaleReferenceQuote,
    #[msg("trade exceeds protected quote size")]
    ProtectedQuoteTradeLimit,
    #[msg("inventory band exceeded")]
    InventoryBandExceeded,
    #[msg("swap math failed")]
    SwapMathFailed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use meta_amm_config::{
        REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET,
        REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS,
    };

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn default_args() -> InitializeReferenceQuotePoolArgs {
        InitializeReferenceQuotePoolArgs {
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

    fn pool_config(authority: Pubkey, same_slot_order: u8) -> ReferenceQuotePoolConfig {
        let mut args = default_args();
        args.quote_update_envelope.same_slot_order = same_slot_order;
        let token_pair = TokenPairIdentity {
            base_mint: key(2).to_bytes(),
            quote_mint: key(3).to_bytes(),
            base_token_program: key(4).to_bytes(),
            quote_token_program: key(5).to_bytes(),
            base_decimals: 8,
            quote_decimals: 6,
        };
        let input = ReferenceQuoteConfigInput {
            strategy: args.strategy_config().unwrap(),
            token_pair,
            account_budget: args.account_budget.to_account_budget(),
        };
        let layout = compile_reference_quote_pool_config_account(input, 254, false)
            .unwrap()
            .to_bytes();

        ReferenceQuotePoolConfig {
            authority,
            base_mint: key(2),
            quote_mint: key(3),
            base_token_program: key(4),
            quote_token_program: key(5),
            bump: 254,
            paused: false,
            layout,
        }
    }

    fn quote_state(pool_config: Pubkey, quote_authority: Pubkey) -> ReferenceQuoteState {
        ReferenceQuoteState {
            pool_config,
            quote_authority,
            bump: 253,
            paused: false,
            same_slot_order: 1,
            _padding: [0; 5],
            mid_price_q64x64: 0,
            publish_slot: 0,
            sequence: 0,
            last_update_slot: 0,
        }
    }

    fn vault_state(
        pool_config: Pubkey,
        maker_authority: Pubkey,
        base_vault: Pubkey,
        quote_vault: Pubkey,
    ) -> VaultState {
        VaultState {
            pool_config,
            maker_authority,
            base_vault,
            quote_vault,
            base_mint: key(2),
            quote_mint: key(3),
            vault_authority_bump: 252,
            bump: 251,
            custody_model: CUSTODY_MODEL_MAKER_OWNED,
            paused: false,
            _padding: [0; 4],
            target_base_inventory: 0,
            reserved: [0; 32],
        }
    }

    fn update_args(sequence: u64, publish_slot: u64) -> UpdateReferenceQuoteArgs {
        UpdateReferenceQuoteArgs {
            mid_price_q64x64: 30_000u128 << 64,
            publish_slot,
            sequence,
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

    #[test]
    fn quote_state_account_space_is_stable() {
        assert_eq!(
            ReferenceQuoteState::INIT_SPACE,
            32 + 32 + 1 + 1 + 1 + 5 + 16 + 8 + 8 + 8
        );
    }

    #[test]
    fn vault_state_account_space_is_stable() {
        assert_eq!(
            VaultState::INIT_SPACE,
            32 + 32 + 32 + 32 + 32 + 32 + 1 + 1 + 1 + 1 + 4 + 8 + 32
        );
    }

    #[test]
    fn vault_pda_seeds_are_pool_scoped_and_distinct() {
        let pool = key(9);
        let (vault_authority, _) =
            Pubkey::find_program_address(&[VAULT_AUTHORITY_SEED, pool.as_ref()], &crate::ID);
        let (vault_state, _) =
            Pubkey::find_program_address(&[VAULT_STATE_SEED, pool.as_ref()], &crate::ID);
        let (base_vault, _) =
            Pubkey::find_program_address(&[BASE_VAULT_SEED, pool.as_ref()], &crate::ID);
        let (quote_vault, _) =
            Pubkey::find_program_address(&[QUOTE_VAULT_SEED, pool.as_ref()], &crate::ID);

        assert_ne!(vault_authority, vault_state);
        assert_ne!(base_vault, quote_vault);
        assert_ne!(vault_authority, base_vault);
        assert_ne!(vault_state, quote_vault);
    }

    #[test]
    fn maker_owned_vault_state_keeps_future_custody_bytes_reserved() {
        let state = vault_state(key(1), key(2), key(3), key(4));

        assert_eq!(state.custody_model, CUSTODY_MODEL_MAKER_OWNED);
        assert!(!state.paused);
        assert_eq!(state.target_base_inventory, 0);
        assert_eq!(state.reserved, [0; 32]);
    }

    #[test]
    fn fund_pool_args_reject_zero_sided_noop() {
        assert!(FundPoolArgs {
            base_amount: 0,
            quote_amount: 0
        }
        .validate()
        .is_err());
        assert!(FundPoolArgs {
            base_amount: 1,
            quote_amount: 0
        }
        .validate()
        .is_ok());
        assert!(FundPoolArgs {
            base_amount: 0,
            quote_amount: 1
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn vault_state_validates_maker_owned_funding_binding() {
        let pool = key(1);
        let maker = key(2);
        let base_vault = key(3);
        let quote_vault = key(4);
        let state = vault_state(pool, maker, base_vault, quote_vault);

        assert!(state
            .assert_maker_owned_for_pool(pool, maker, base_vault, quote_vault)
            .is_ok());
        assert!(state
            .assert_maker_owned_for_pool(key(9), maker, base_vault, quote_vault)
            .is_err());
        assert!(state
            .assert_maker_owned_for_pool(pool, key(9), base_vault, quote_vault)
            .is_err());
        assert!(state
            .assert_maker_owned_for_pool(pool, maker, key(9), quote_vault)
            .is_err());
    }

    #[test]
    fn vault_state_rejects_paused_or_non_maker_owned_funding() {
        let pool = key(1);
        let maker = key(2);
        let base_vault = key(3);
        let quote_vault = key(4);
        let mut state = vault_state(pool, maker, base_vault, quote_vault);

        state.paused = true;
        assert!(state
            .assert_maker_owned_for_pool(pool, maker, base_vault, quote_vault)
            .is_err());

        state.paused = false;
        state.custody_model = 9;
        assert!(state
            .assert_maker_owned_for_pool(pool, maker, base_vault, quote_vault)
            .is_err());
    }

    #[test]
    fn vault_state_tracks_base_funding_target() {
        let mut state = vault_state(key(1), key(2), key(3), key(4));

        state.apply_base_funding_to_target(0, 0).unwrap();
        assert_eq!(state.target_base_inventory, 0);

        state.apply_base_funding_to_target(10, 90).unwrap();
        assert_eq!(state.target_base_inventory, 100);

        state.apply_base_funding_to_target(100, 25).unwrap();
        assert_eq!(state.target_base_inventory, 125);

        state.target_base_inventory = u64::MAX;
        assert!(state.apply_base_funding_to_target(0, 1).is_err());
    }

    #[test]
    fn swap_args_require_amount_slippage_and_quote_binding() {
        assert!(SwapExactInArgs {
            amount_in: 0,
            minimum_amount_out: 1,
            expected_quote_sequence: 1,
            base_to_quote: true
        }
        .validate()
        .is_err());
        assert!(SwapExactInArgs {
            amount_in: 1,
            minimum_amount_out: 0,
            expected_quote_sequence: 1,
            base_to_quote: true
        }
        .validate()
        .is_err());
        assert!(SwapExactInArgs {
            amount_in: 1,
            minimum_amount_out: 1,
            expected_quote_sequence: 0,
            base_to_quote: true
        }
        .validate()
        .is_err());
        assert!(SwapExactInArgs {
            amount_in: 1,
            minimum_amount_out: 1,
            expected_quote_sequence: 1,
            base_to_quote: false
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn quote_state_validates_swap_sequence_binding() {
        let pool = key(1);
        let quote_authority = key(2);
        let mut state = quote_state(pool, quote_authority);

        assert!(state.assert_usable_for_swap(pool, 1, 1, 10).is_err());

        state.mid_price_q64x64 = 1u128 << 64;
        state.publish_slot = 8;
        state.sequence = 2;
        assert!(state.assert_usable_for_swap(pool, 1, 2, 10).is_ok());
        assert!(state.assert_usable_for_swap(pool, 1, 1, 10).is_err());
        assert!(state.assert_usable_for_swap(pool, 0, 2, 10).is_err());
        assert!(state.assert_usable_for_swap(pool, 1, 2, 7).is_err());

        state.paused = true;
        assert!(state.assert_usable_for_swap(pool, 1, 2, 10).is_err());
    }

    #[test]
    fn pool_config_reads_same_slot_order_from_compiled_layout() {
        let mut pool_config = pool_config(key(1), 1);

        assert_eq!(pool_config.reference_quote_same_slot_order().unwrap(), 1);
        assert_eq!(
            pool_config.layout[REFERENCE_QUOTE_POOL_CONFIG_SAME_SLOT_ORDER_OFFSET],
            1
        );

        pool_config.layout[REFERENCE_QUOTE_POOL_CONFIG_SAME_SLOT_ORDER_OFFSET] = 3;
        assert!(pool_config.reference_quote_same_slot_order().is_err());
    }

    #[test]
    fn quote_update_accepts_pool_authority_and_tracks_landing_slot() {
        let pool_authority = key(1);
        let quote_authority = key(2);
        let mut state = quote_state(key(3), quote_authority);

        state
            .apply_update(pool_authority, pool_authority, update_args(1, 10), 12)
            .unwrap();

        assert_eq!(state.mid_price_q64x64, 30_000u128 << 64);
        assert_eq!(state.publish_slot, 10);
        assert_eq!(state.sequence, 1);
        assert_eq!(state.last_update_slot, 12);
    }

    #[test]
    fn quote_update_accepts_quote_authority() {
        let pool_authority = key(1);
        let quote_authority = key(2);
        let mut state = quote_state(key(3), quote_authority);

        assert!(state
            .apply_update(pool_authority, quote_authority, update_args(1, 10), 12)
            .is_ok());
    }

    #[test]
    fn quote_update_allows_same_publish_slot_with_higher_sequence() {
        let pool_authority = key(1);
        let quote_authority = key(2);
        let mut state = quote_state(key(3), quote_authority);

        state
            .apply_update(pool_authority, quote_authority, update_args(1, 10), 12)
            .unwrap();
        state
            .apply_update(pool_authority, quote_authority, update_args(2, 10), 12)
            .unwrap();

        assert_eq!(state.publish_slot, 10);
        assert_eq!(state.sequence, 2);
    }

    #[test]
    fn quote_update_rejects_bad_authority() {
        let pool_authority = key(1);
        let quote_authority = key(2);
        let mut state = quote_state(key(3), quote_authority);

        assert!(state
            .apply_update(pool_authority, key(9), update_args(1, 10), 12)
            .is_err());
    }

    #[test]
    fn quote_update_rejects_replayed_or_lower_sequence() {
        let pool_authority = key(1);
        let quote_authority = key(2);
        let mut state = quote_state(key(3), quote_authority);

        state
            .apply_update(pool_authority, quote_authority, update_args(3, 10), 12)
            .unwrap();

        assert!(state
            .apply_update(pool_authority, quote_authority, update_args(3, 11), 12)
            .is_err());
        assert!(state
            .apply_update(pool_authority, quote_authority, update_args(2, 11), 12)
            .is_err());
    }

    #[test]
    fn quote_update_rejects_backdated_publish_slot() {
        let pool_authority = key(1);
        let quote_authority = key(2);
        let mut state = quote_state(key(3), quote_authority);

        state
            .apply_update(pool_authority, quote_authority, update_args(1, 10), 12)
            .unwrap();

        assert!(state
            .apply_update(pool_authority, quote_authority, update_args(2, 9), 12)
            .is_err());
    }

    #[test]
    fn quote_update_rejects_future_publish_slot() {
        let pool_authority = key(1);
        let quote_authority = key(2);
        let mut state = quote_state(key(3), quote_authority);

        assert!(state
            .apply_update(pool_authority, quote_authority, update_args(1, 13), 12)
            .is_err());
    }

    #[test]
    fn quote_update_rejects_zero_price() {
        let pool_authority = key(1);
        let quote_authority = key(2);
        let mut state = quote_state(key(3), quote_authority);
        let mut args = update_args(1, 10);
        args.mid_price_q64x64 = 0;

        assert!(state
            .apply_update(pool_authority, quote_authority, args, 12)
            .is_err());
    }
}
