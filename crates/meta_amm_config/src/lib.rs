#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use meta_amm_math::{
    DecimalScale, DecimalScalePreimage, MathError, PriceDomain, ReferenceQuoteParams,
    DECIMAL_SCALE_PREIMAGE_LEN,
};

pub const CONFIG_SCHEMA_VERSION: u16 = 1;
pub const BASIS_POINTS: u16 = 10_000;
pub const REFERENCE_QUOTE_MODE_ID: u8 = 1;
pub const REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS: u8 = 10;
pub const REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET: u8 = 12;
pub const REFERENCE_QUOTE_POOL_CONFIG_DISCRIMINATOR: [u8; 8] = *b"MAMMCFG1";
pub const REFERENCE_QUOTE_POOL_CONFIG_RESERVED_BYTES: usize = 64;
pub const REFERENCE_QUOTE_POOL_CONFIG_MAX_BYTES: usize = 512;
pub const REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN: usize =
    core::mem::size_of::<ReferenceQuotePoolConfigAccount>();
pub const REFERENCE_QUOTE_POOL_CONFIG_SAME_SLOT_ORDER_OFFSET: usize = 8 + 8 + 56 + 8 + 8 + 2;

/// Byte sum the manual `to_bytes` writer emits, computed independently of
/// `core::mem::size_of`. Held equal to `REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN`
/// by the const assertion below so reordering or adding a field that desyncs
/// the writer from the struct layout is a compile error rather than a
/// debug-only assert.
pub const REFERENCE_QUOTE_POOL_CONFIG_WRITER_LEN: usize = 8 // discriminator
    + 2 + 1 + 1 + 1 + 3 // PoolConfigHeaderLayout
    + 5 * 8 + 7 * 2 + 2 // ReferenceQuoteParamsLayout
    + 2 * 8 + 2 + 1 + 5 // QuoteUpdateEnvelopeLayout
    + 1 + 1 // AccountBudgetLayout
    + 4 * 32 // base/quote mints + base/quote token programs
    + DECIMAL_SCALE_PREIMAGE_LEN
    + REFERENCE_QUOTE_POOL_CONFIG_RESERVED_BYTES
    + 2; // _trailing_padding

const _: () = assert!(
    REFERENCE_QUOTE_POOL_CONFIG_WRITER_LEN == REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN,
    "manual to_bytes writer length must match ReferenceQuotePoolConfigAccount layout size"
);
const _: () = assert!(
    REFERENCE_QUOTE_POOL_CONFIG_SAME_SLOT_ORDER_OFFSET < REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN
);
pub const REFERENCE_QUOTE_SWAP_ACCOUNT_META_HEADROOM: u8 =
    REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET - REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS;

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

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolConfigHeaderLayout {
    pub schema_version: u16,
    pub mode: u8,
    pub paused: u8,
    pub authority_bump: u8,
    pub _padding: [u8; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuoteParamsLayout {
    pub aging_start_slots: u64,
    pub protected_start_slots: u64,
    pub expire_slots: u64,
    pub max_trade_base_atoms: u64,
    pub protected_max_trade_base_atoms: u64,
    pub fee_bps: u16,
    pub base_half_spread_bps: u16,
    pub aging_surcharge_bps_per_slot: u16,
    pub max_aging_surcharge_bps: u16,
    pub inventory_skew_bps_per_10k_imbalance: u16,
    pub max_inventory_skew_bps: u16,
    pub hard_inventory_band_bps: u16,
    pub _padding: [u8; 2],
}

impl ReferenceQuoteParamsLayout {
    pub const fn from_params(params: ReferenceQuoteParams) -> Self {
        Self {
            aging_start_slots: params.aging_start_slots,
            protected_start_slots: params.protected_start_slots,
            expire_slots: params.expire_slots,
            max_trade_base_atoms: params.max_trade_base_atoms,
            protected_max_trade_base_atoms: params.protected_max_trade_base_atoms,
            fee_bps: params.fee_bps,
            base_half_spread_bps: params.base_half_spread_bps,
            aging_surcharge_bps_per_slot: params.aging_surcharge_bps_per_slot,
            max_aging_surcharge_bps: params.max_aging_surcharge_bps,
            inventory_skew_bps_per_10k_imbalance: params.inventory_skew_bps_per_10k_imbalance,
            max_inventory_skew_bps: params.max_inventory_skew_bps,
            hard_inventory_band_bps: params.hard_inventory_band_bps,
            _padding: [0; 2],
        }
    }

    pub const fn to_params(self) -> ReferenceQuoteParams {
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

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuoteUpdateEnvelopeLayout {
    pub max_maker_update_period_slots: u64,
    pub max_landing_latency_slots: u64,
    pub min_update_success_probability_bps: u16,
    pub same_slot_order: u8,
    pub _padding: [u8; 5],
}

impl QuoteUpdateEnvelopeLayout {
    pub const fn from_envelope(envelope: QuoteUpdateEnvelope) -> Self {
        Self {
            max_maker_update_period_slots: envelope.max_maker_update_period_slots,
            max_landing_latency_slots: envelope.max_landing_latency_slots,
            min_update_success_probability_bps: envelope.min_update_success_probability_bps,
            same_slot_order: same_slot_update_order_to_u8(envelope.same_slot_order),
            _padding: [0; 5],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountBudgetLayout {
    pub required_swap_account_metas: u8,
    pub max_swap_account_metas: u8,
}

impl AccountBudgetLayout {
    pub const fn from_budget(account_budget: AccountBudget) -> Self {
        Self {
            required_swap_account_metas: account_budget.required_swap_account_metas,
            max_swap_account_metas: account_budget.max_swap_account_metas,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuotePoolConfigAccount {
    pub discriminator: [u8; 8],
    pub header: PoolConfigHeaderLayout,
    pub params: ReferenceQuoteParamsLayout,
    pub quote_update_envelope: QuoteUpdateEnvelopeLayout,
    pub account_budget: AccountBudgetLayout,
    pub base_mint: [u8; 32],
    pub quote_mint: [u8; 32],
    pub base_token_program: [u8; 32],
    pub quote_token_program: [u8; 32],
    pub decimal_scale_preimage: [u8; DECIMAL_SCALE_PREIMAGE_LEN],
    pub reserved: [u8; REFERENCE_QUOTE_POOL_CONFIG_RESERVED_BYTES],
    pub _trailing_padding: [u8; 2],
}

impl ReferenceQuotePoolConfigAccount {
    pub fn from_compiled(
        compiled: CompiledReferenceQuoteConfig,
        authority_bump: u8,
        paused: bool,
    ) -> Self {
        Self {
            discriminator: REFERENCE_QUOTE_POOL_CONFIG_DISCRIMINATOR,
            header: PoolConfigHeaderLayout {
                schema_version: compiled.schema_version,
                mode: compiled.mode.as_u8(),
                paused: u8::from(paused),
                authority_bump,
                _padding: [0; 3],
            },
            params: ReferenceQuoteParamsLayout::from_params(compiled.params),
            quote_update_envelope: QuoteUpdateEnvelopeLayout::from_envelope(
                compiled.quote_update_envelope,
            ),
            account_budget: AccountBudgetLayout::from_budget(compiled.account_budget),
            base_mint: compiled.token_pair.base_mint,
            quote_mint: compiled.token_pair.quote_mint,
            base_token_program: compiled.token_pair.base_token_program,
            quote_token_program: compiled.token_pair.quote_token_program,
            decimal_scale_preimage: compiled.decimal_scale_preimage.to_bytes(),
            reserved: [0; REFERENCE_QUOTE_POOL_CONFIG_RESERVED_BYTES],
            _trailing_padding: [0; 2],
        }
    }

    pub fn to_bytes(self) -> [u8; REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN] {
        let mut out = [0u8; REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN];
        let mut offset = 0usize;

        put_bytes(&mut out, &mut offset, &self.discriminator);
        put_u16(&mut out, &mut offset, self.header.schema_version);
        put_u8(&mut out, &mut offset, self.header.mode);
        put_u8(&mut out, &mut offset, self.header.paused);
        put_u8(&mut out, &mut offset, self.header.authority_bump);
        put_bytes(&mut out, &mut offset, &self.header._padding);
        put_u64(&mut out, &mut offset, self.params.aging_start_slots);
        put_u64(&mut out, &mut offset, self.params.protected_start_slots);
        put_u64(&mut out, &mut offset, self.params.expire_slots);
        put_u64(&mut out, &mut offset, self.params.max_trade_base_atoms);
        put_u64(
            &mut out,
            &mut offset,
            self.params.protected_max_trade_base_atoms,
        );
        put_u16(&mut out, &mut offset, self.params.fee_bps);
        put_u16(&mut out, &mut offset, self.params.base_half_spread_bps);
        put_u16(
            &mut out,
            &mut offset,
            self.params.aging_surcharge_bps_per_slot,
        );
        put_u16(&mut out, &mut offset, self.params.max_aging_surcharge_bps);
        put_u16(
            &mut out,
            &mut offset,
            self.params.inventory_skew_bps_per_10k_imbalance,
        );
        put_u16(&mut out, &mut offset, self.params.max_inventory_skew_bps);
        put_u16(&mut out, &mut offset, self.params.hard_inventory_band_bps);
        put_bytes(&mut out, &mut offset, &self.params._padding);
        put_u64(
            &mut out,
            &mut offset,
            self.quote_update_envelope.max_maker_update_period_slots,
        );
        put_u64(
            &mut out,
            &mut offset,
            self.quote_update_envelope.max_landing_latency_slots,
        );
        put_u16(
            &mut out,
            &mut offset,
            self.quote_update_envelope
                .min_update_success_probability_bps,
        );
        put_u8(
            &mut out,
            &mut offset,
            self.quote_update_envelope.same_slot_order,
        );
        put_bytes(&mut out, &mut offset, &self.quote_update_envelope._padding);
        put_u8(
            &mut out,
            &mut offset,
            self.account_budget.required_swap_account_metas,
        );
        put_u8(
            &mut out,
            &mut offset,
            self.account_budget.max_swap_account_metas,
        );
        put_bytes(&mut out, &mut offset, &self.base_mint);
        put_bytes(&mut out, &mut offset, &self.quote_mint);
        put_bytes(&mut out, &mut offset, &self.base_token_program);
        put_bytes(&mut out, &mut offset, &self.quote_token_program);
        put_bytes(&mut out, &mut offset, &self.decimal_scale_preimage);
        put_bytes(&mut out, &mut offset, &self.reserved);
        put_bytes(&mut out, &mut offset, &self._trailing_padding);
        // Writer length is locked to the struct layout by the
        // REFERENCE_QUOTE_POOL_CONFIG_WRITER_LEN const assertion above; this
        // assert catches the off-chance that a `put_*` helper drifts in size
        // without changing the const.
        debug_assert_eq!(offset, REFERENCE_QUOTE_POOL_CONFIG_WRITER_LEN);

        out
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportField {
    Version,
    Mode,
    FeeBps,
    BaseHalfSpreadBps,
    AgingStartSlots,
    ProtectedStartSlots,
    ExpireSlots,
    AgingSurchargeBpsPerSlot,
    MaxAgingSurchargeBps,
    MaxTradeBaseAtoms,
    ProtectedMaxTradeBaseAtoms,
    InventorySkewBpsPer10kImbalance,
    MaxInventorySkewBps,
    HardInventoryBandBps,
    MaxMakerUpdatePeriodSlots,
    MaxLandingLatencySlots,
    MinUpdateSuccessProbabilityBps,
    SameSlotOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigImportError {
    MissingField(ExportField),
    InvalidNumber(ExportField),
    InvalidString(ExportField),
    UnsupportedVersion(u64),
    UnsupportedMode,
    UnknownSameSlotOrder,
    Config(ConfigError),
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

pub fn parse_reference_quote_strategy_export(
    source: &str,
) -> Result<ReferenceQuoteStrategyConfig, ConfigImportError> {
    let version = parse_u64_field(source, ExportField::Version)?;
    if version != CONFIG_SCHEMA_VERSION as u64 {
        return Err(ConfigImportError::UnsupportedVersion(version));
    }
    if parse_string_field(source, ExportField::Mode)? != "ReferenceQuote" {
        return Err(ConfigImportError::UnsupportedMode);
    }

    let params = ReferenceQuoteParams {
        fee_bps: parse_u16_field(source, ExportField::FeeBps)?,
        base_half_spread_bps: parse_u16_field(source, ExportField::BaseHalfSpreadBps)?,
        aging_start_slots: parse_u64_field(source, ExportField::AgingStartSlots)?,
        protected_start_slots: parse_u64_field(source, ExportField::ProtectedStartSlots)?,
        expire_slots: parse_u64_field(source, ExportField::ExpireSlots)?,
        aging_surcharge_bps_per_slot: parse_u16_field(
            source,
            ExportField::AgingSurchargeBpsPerSlot,
        )?,
        max_aging_surcharge_bps: parse_u16_field(source, ExportField::MaxAgingSurchargeBps)?,
        max_trade_base_atoms: parse_u64_field(source, ExportField::MaxTradeBaseAtoms)?,
        protected_max_trade_base_atoms: parse_u64_field(
            source,
            ExportField::ProtectedMaxTradeBaseAtoms,
        )?,
        inventory_skew_bps_per_10k_imbalance: parse_u16_field(
            source,
            ExportField::InventorySkewBpsPer10kImbalance,
        )?,
        max_inventory_skew_bps: parse_u16_field(source, ExportField::MaxInventorySkewBps)?,
        hard_inventory_band_bps: parse_u16_field(source, ExportField::HardInventoryBandBps)?,
    };
    validate_reference_quote_params(params).map_err(ConfigImportError::Config)?;

    let same_slot_order = match parse_string_field(source, ExportField::SameSlotOrder)? {
        "update_before_swap" => SameSlotUpdateOrder::UpdateBeforeSwap,
        "swap_before_update" => SameSlotUpdateOrder::SwapBeforeUpdate,
        _ => return Err(ConfigImportError::UnknownSameSlotOrder),
    };
    let quote_update_envelope = QuoteUpdateEnvelope {
        max_maker_update_period_slots: parse_u64_field(
            source,
            ExportField::MaxMakerUpdatePeriodSlots,
        )?,
        max_landing_latency_slots: parse_u64_field(source, ExportField::MaxLandingLatencySlots)?,
        min_update_success_probability_bps: parse_u16_field(
            source,
            ExportField::MinUpdateSuccessProbabilityBps,
        )?,
        same_slot_order,
    };
    validate_quote_update_envelope(quote_update_envelope, params.expire_slots)
        .map_err(ConfigImportError::Config)?;

    Ok(ReferenceQuoteStrategyConfig {
        params,
        quote_update_envelope,
    })
}

pub fn reference_quote_input_from_export(
    source: &str,
    token_pair: TokenPairIdentity,
    account_budget: AccountBudget,
) -> Result<ReferenceQuoteConfigInput, ConfigImportError> {
    let strategy = parse_reference_quote_strategy_export(source)?;
    validate_account_budget(account_budget).map_err(ConfigImportError::Config)?;
    DecimalScale::new(token_pair.base_decimals, token_pair.quote_decimals)
        .map_err(ConfigError::DecimalScaleInvalid)
        .map_err(ConfigImportError::Config)?;

    Ok(ReferenceQuoteConfigInput {
        strategy,
        token_pair,
        account_budget,
    })
}

pub fn compile_reference_quote_export(
    source: &str,
    token_pair: TokenPairIdentity,
    account_budget: AccountBudget,
) -> Result<CompiledReferenceQuoteConfig, ConfigImportError> {
    compile_reference_quote_config(reference_quote_input_from_export(
        source,
        token_pair,
        account_budget,
    )?)
    .map_err(ConfigImportError::Config)
}

pub fn compile_reference_quote_pool_config_account(
    input: ReferenceQuoteConfigInput,
    authority_bump: u8,
    paused: bool,
) -> Result<ReferenceQuotePoolConfigAccount, ConfigError> {
    let compiled = compile_reference_quote_config(input)?;
    Ok(ReferenceQuotePoolConfigAccount::from_compiled(
        compiled,
        authority_bump,
        paused,
    ))
}

pub fn compile_reference_quote_export_account(
    source: &str,
    token_pair: TokenPairIdentity,
    account_budget: AccountBudget,
    authority_bump: u8,
    paused: bool,
) -> Result<ReferenceQuotePoolConfigAccount, ConfigImportError> {
    let compiled = compile_reference_quote_export(source, token_pair, account_budget)?;
    Ok(ReferenceQuotePoolConfigAccount::from_compiled(
        compiled,
        authority_bump,
        paused,
    ))
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

fn put_u8<const N: usize>(out: &mut [u8; N], offset: &mut usize, value: u8) {
    out[*offset] = value;
    *offset += 1;
}

fn put_u16<const N: usize>(out: &mut [u8; N], offset: &mut usize, value: u16) {
    put_bytes(out, offset, &value.to_le_bytes());
}

fn put_u64<const N: usize>(out: &mut [u8; N], offset: &mut usize, value: u64) {
    put_bytes(out, offset, &value.to_le_bytes());
}

fn put_bytes<const OUT: usize, const INPUT: usize>(
    out: &mut [u8; OUT],
    offset: &mut usize,
    value: &[u8; INPUT],
) {
    out[*offset..*offset + INPUT].copy_from_slice(value);
    *offset += INPUT;
}

const fn same_slot_update_order_to_u8(order: SameSlotUpdateOrder) -> u8 {
    match order {
        SameSlotUpdateOrder::UpdateBeforeSwap => 0,
        SameSlotUpdateOrder::SwapBeforeUpdate => 1,
    }
}

fn parse_u16_field(source: &str, field: ExportField) -> Result<u16, ConfigImportError> {
    let value = parse_u64_field(source, field)?;
    if value > u16::MAX as u64 {
        return Err(ConfigImportError::InvalidNumber(field));
    }
    Ok(value as u16)
}

fn parse_u64_field(source: &str, field: ExportField) -> Result<u64, ConfigImportError> {
    let value = field_value(source, field)?;
    let value = skip_ascii_whitespace(value);
    let digits_len = value
        .as_bytes()
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits_len == 0 {
        return Err(ConfigImportError::InvalidNumber(field));
    }
    // Reject trailing junk like `30abc`. Numbers in the export must be
    // terminated by whitespace, `,`, `}`, `]`, or end-of-input — anything
    // else means the input wasn't shaped like the simulator export.
    if let Some(next) = value.as_bytes().get(digits_len) {
        let ok = matches!(*next, b',' | b'}' | b']') || next.is_ascii_whitespace();
        if !ok {
            return Err(ConfigImportError::InvalidNumber(field));
        }
    }
    parse_u64_digits(&value[..digits_len]).ok_or(ConfigImportError::InvalidNumber(field))
}

fn parse_string_field(source: &str, field: ExportField) -> Result<&str, ConfigImportError> {
    let value = skip_ascii_whitespace(field_value(source, field)?);
    let rest = value
        .strip_prefix('"')
        .ok_or(ConfigImportError::InvalidString(field))?;
    let mut escaped = false;
    for (index, ch) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Ok(&rest[..index]),
            _ => {}
        }
    }
    Err(ConfigImportError::InvalidString(field))
}

fn field_value(source: &str, field: ExportField) -> Result<&str, ConfigImportError> {
    let needle = export_field_needle(field);
    let key_index = source
        .find(needle)
        .ok_or(ConfigImportError::MissingField(field))?;
    let after_key = &source[key_index + needle.len()..];
    let colon_index = after_key
        .find(':')
        .ok_or(ConfigImportError::MissingField(field))?;
    Ok(&after_key[colon_index + 1..])
}

fn skip_ascii_whitespace(value: &str) -> &str {
    value.trim_start_matches(|ch: char| ch.is_ascii_whitespace())
}

fn parse_u64_digits(value: &str) -> Option<u64> {
    let mut out = 0u64;
    for byte in value.as_bytes() {
        let digit = byte.checked_sub(b'0')?;
        if digit > 9 {
            return None;
        }
        out = out.checked_mul(10)?.checked_add(digit as u64)?;
    }
    Some(out)
}

fn export_field_needle(field: ExportField) -> &'static str {
    match field {
        ExportField::Version => "\"version\"",
        ExportField::Mode => "\"mode\"",
        ExportField::FeeBps => "\"fee_bps\"",
        ExportField::BaseHalfSpreadBps => "\"base_half_spread_bps\"",
        ExportField::AgingStartSlots => "\"aging_start_slots\"",
        ExportField::ProtectedStartSlots => "\"protected_start_slots\"",
        ExportField::ExpireSlots => "\"expire_slots\"",
        ExportField::AgingSurchargeBpsPerSlot => "\"aging_surcharge_bps_per_slot\"",
        ExportField::MaxAgingSurchargeBps => "\"max_aging_surcharge_bps\"",
        ExportField::MaxTradeBaseAtoms => "\"max_trade_base_atoms\"",
        ExportField::ProtectedMaxTradeBaseAtoms => "\"protected_max_trade_base_atoms\"",
        ExportField::InventorySkewBpsPer10kImbalance => "\"inventory_skew_bps_per_10k_imbalance\"",
        ExportField::MaxInventorySkewBps => "\"max_inventory_skew_bps\"",
        ExportField::HardInventoryBandBps => "\"hard_inventory_band_bps\"",
        ExportField::MaxMakerUpdatePeriodSlots => "\"max_maker_update_period_slots\"",
        ExportField::MaxLandingLatencySlots => "\"max_landing_latency_slots\"",
        ExportField::MinUpdateSuccessProbabilityBps => "\"min_update_success_probability_bps\"",
        ExportField::SameSlotOrder => "\"same_slot_order\"",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCE_QUOTE_EXPORT_FIXTURE: &str =
        include_str!("../../../tests/golden/reference-quote-config-export.json");

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
    fn parses_reference_quote_export_fixture_and_compiles_it() {
        let parsed = parse_reference_quote_strategy_export(REFERENCE_QUOTE_EXPORT_FIXTURE).unwrap();

        assert_eq!(parsed.params, params());
        assert_eq!(
            parsed.quote_update_envelope,
            strategy().quote_update_envelope
        );

        let input = reference_quote_input_from_export(
            REFERENCE_QUOTE_EXPORT_FIXTURE,
            token_pair(),
            AccountBudget::reference_quote_jupiter_default(),
        )
        .unwrap();
        let compiled = compile_reference_quote_export(
            REFERENCE_QUOTE_EXPORT_FIXTURE,
            token_pair(),
            AccountBudget::reference_quote_jupiter_default(),
        )
        .unwrap();

        assert_eq!(input.strategy, parsed);
        assert_eq!(compiled.params, params());
        assert_eq!(compiled.quote_update_envelope, parsed.quote_update_envelope);
        assert_eq!(
            compiled.decimal_scale_preimage,
            token_pair().decimal_scale_preimage()
        );
    }

    #[test]
    fn reference_quote_pool_config_account_layout_has_fixed_size_and_meta_headroom() {
        let layout_len = core::mem::size_of::<ReferenceQuotePoolConfigAccount>();
        assert_eq!(layout_len, REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN);
        assert_eq!(layout_len, 448);
        assert!(layout_len <= REFERENCE_QUOTE_POOL_CONFIG_MAX_BYTES);
        assert_eq!(layout_len % 8, 0);

        let budget = AccountBudget::reference_quote_jupiter_default();
        let headroom = budget
            .max_swap_account_metas
            .saturating_sub(budget.required_swap_account_metas);
        assert_eq!(headroom, REFERENCE_QUOTE_SWAP_ACCOUNT_META_HEADROOM);
        assert_eq!(headroom, 2);
        assert_eq!(
            budget.required_swap_account_metas,
            REFERENCE_QUOTE_REQUIRED_SWAP_ACCOUNT_METAS
        );
        assert_eq!(
            budget.max_swap_account_metas,
            REFERENCE_QUOTE_DEFAULT_SWAP_ACCOUNT_META_BUDGET
        );
        assert!(budget.required_swap_account_metas < budget.max_swap_account_metas);
    }

    #[test]
    fn compiles_reference_quote_export_into_pool_config_account_layout() {
        let account = compile_reference_quote_export_account(
            REFERENCE_QUOTE_EXPORT_FIXTURE,
            token_pair(),
            AccountBudget::reference_quote_jupiter_default(),
            254,
            false,
        )
        .unwrap();

        assert_eq!(
            account.discriminator,
            REFERENCE_QUOTE_POOL_CONFIG_DISCRIMINATOR
        );
        assert_eq!(account.header.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(account.header.mode, REFERENCE_QUOTE_MODE_ID);
        assert_eq!(account.header.paused, 0);
        assert_eq!(account.header.authority_bump, 254);
        assert_eq!(account.header._padding, [0; 3]);
        assert_eq!(account.params.to_params(), params());
        assert_eq!(
            account.quote_update_envelope.max_maker_update_period_slots,
            strategy()
                .quote_update_envelope
                .max_maker_update_period_slots
        );
        assert_eq!(
            account.quote_update_envelope.max_landing_latency_slots,
            strategy().quote_update_envelope.max_landing_latency_slots
        );
        assert_eq!(
            account
                .quote_update_envelope
                .min_update_success_probability_bps,
            strategy()
                .quote_update_envelope
                .min_update_success_probability_bps
        );
        assert_eq!(account.quote_update_envelope.same_slot_order, 1);
        assert_eq!(account.account_budget.required_swap_account_metas, 10);
        assert_eq!(account.account_budget.max_swap_account_metas, 12);
        assert_eq!(account.base_mint, token_pair().base_mint);
        assert_eq!(
            account.decimal_scale_preimage,
            token_pair().decimal_scale_preimage().to_bytes()
        );
        assert_eq!(
            account.reserved,
            [0; REFERENCE_QUOTE_POOL_CONFIG_RESERVED_BYTES]
        );
        assert_eq!(account._trailing_padding, [0; 2]);
        let bytes = account.to_bytes();
        assert_eq!(bytes.len(), REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN);
        assert_eq!(&bytes[..8], &REFERENCE_QUOTE_POOL_CONFIG_DISCRIMINATOR);
        assert_eq!(bytes[8..10], CONFIG_SCHEMA_VERSION.to_le_bytes());
        assert_eq!(bytes[10], REFERENCE_QUOTE_MODE_ID);
        assert_eq!(bytes[11], 0);
        assert_eq!(bytes[12], 254);
        assert_eq!(
            bytes[REFERENCE_QUOTE_POOL_CONFIG_SAME_SLOT_ORDER_OFFSET],
            account.quote_update_envelope.same_slot_order
        );
        assert_eq!(bytes[REFERENCE_QUOTE_POOL_CONFIG_ACCOUNT_LEN - 2..], [0, 0]);
    }

    #[test]
    fn parser_rejects_trailing_junk_after_numeric_field() {
        // The parser is closed-loop with the simulator's export writer; any
        // number that isn't terminated by whitespace, `,`, `}`, `]`, or EOI
        // is treated as malformed input.
        let bad_number =
            REFERENCE_QUOTE_EXPORT_FIXTURE.replacen("\"fee_bps\": 30", "\"fee_bps\": 30abc", 1);
        assert_eq!(
            parse_reference_quote_strategy_export(&bad_number),
            Err(ConfigImportError::InvalidNumber(ExportField::FeeBps))
        );
    }

    #[test]
    fn rejects_malformed_or_unsupported_reference_quote_exports() {
        assert_eq!(
            parse_reference_quote_strategy_export("{}"),
            Err(ConfigImportError::MissingField(ExportField::Version))
        );

        let bad_version =
            REFERENCE_QUOTE_EXPORT_FIXTURE.replacen("\"version\": 1", "\"version\": 2", 1);
        assert_eq!(
            parse_reference_quote_strategy_export(&bad_version),
            Err(ConfigImportError::UnsupportedVersion(2))
        );

        let bad_mode = REFERENCE_QUOTE_EXPORT_FIXTURE.replacen(
            "\"mode\": \"ReferenceQuote\"",
            "\"mode\": \"Other\"",
            1,
        );
        assert_eq!(
            parse_reference_quote_strategy_export(&bad_mode),
            Err(ConfigImportError::UnsupportedMode)
        );

        let bad_order = REFERENCE_QUOTE_EXPORT_FIXTURE.replacen(
            "\"same_slot_order\": \"swap_before_update\"",
            "\"same_slot_order\": \"maker_last\"",
            1,
        );
        assert_eq!(
            parse_reference_quote_strategy_export(&bad_order),
            Err(ConfigImportError::UnknownSameSlotOrder)
        );

        let bad_bps =
            REFERENCE_QUOTE_EXPORT_FIXTURE.replacen("\"fee_bps\": 30", "\"fee_bps\": 70000", 1);
        assert_eq!(
            parse_reference_quote_strategy_export(&bad_bps),
            Err(ConfigImportError::InvalidNumber(ExportField::FeeBps))
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
