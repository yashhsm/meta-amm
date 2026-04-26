use meta_amm_math::{
    cpmm_quote_exact_in, reference_quote_exact_in, CpmmReserves, MathError, Q64x64, QuoteAgeState,
    ReferenceQuoteParams, ReferenceQuoteState,
};

pub use meta_amm_config::{QuoteUpdateEnvelope, ReferenceQuoteStrategyConfig, SameSlotUpdateOrder};

mod calibration;
mod replay;

pub use calibration::*;
pub use replay::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    BaseToQuote,
    QuoteToBase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowEvent {
    pub slot: u64,
    pub side: Side,
    pub amount_in: u64,
    pub fair_price: Q64x64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowOrder {
    pub side: Side,
    pub amount_in: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketSlot {
    pub slot: u64,
    pub fair_price: Q64x64,
    pub flow: Option<FlowOrder>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuoteUpdatePolicy {
    pub maker_update_period_slots: u64,
    pub landing_latency_slots: u64,
    pub update_success_probability_bps: u16,
    pub failure_seed: u128,
    pub same_slot_order: SameSlotUpdateOrder,
}

impl QuoteUpdatePolicy {
    pub const fn instant(maker_update_period_slots: u64) -> Self {
        Self {
            maker_update_period_slots,
            landing_latency_slots: 0,
            update_success_probability_bps: 10_000,
            failure_seed: 0,
            same_slot_order: SameSlotUpdateOrder::UpdateBeforeSwap,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScenarioAssumptions {
    pub name: &'static str,
    pub flow_model: &'static str,
    pub landing_model: &'static str,
    pub path_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpmmScenario {
    pub assumptions: ScenarioAssumptions,
    pub initial_reserves: CpmmReserves,
    pub fee_bps: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimReport {
    pub assumptions: ScenarioAssumptions,
    pub trades_attempted: u64,
    pub trades_filled: u64,
    pub trades_rejected: u64,
    pub fees_quote_atoms: u128,
    pub taker_edge_quote_atoms: i128,
    pub final_reserves: CpmmReserves,
}

impl SimReport {
    pub fn fill_rate_bps(&self) -> u16 {
        if self.trades_attempted == 0 {
            return 0;
        }
        ((self.trades_filled * 10_000) / self.trades_attempted) as u16
    }

    pub fn is_single_path(&self) -> bool {
        self.assumptions.path_count == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedCpmmScenario {
    pub assumptions: ScenarioAssumptions,
    pub seed: u128,
    pub slots_per_path: u64,
    pub initial_reserves: CpmmReserves,
    pub initial_fair_price: Q64x64,
    pub fee_bps: u16,
    pub volatility_bps_per_slot: u16,
    pub drift_bps_per_slot: i16,
    pub trade_probability_bps: u16,
    pub max_trade_base_atoms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuoteScenario {
    pub assumptions: ScenarioAssumptions,
    pub initial_base_inventory: u64,
    pub initial_quote_inventory: u64,
    pub target_base_inventory: u64,
    pub initial_mid_price: Q64x64,
    pub quote_update_policy: QuoteUpdatePolicy,
    pub params: ReferenceQuoteParams,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedReferenceQuoteScenario {
    pub assumptions: ScenarioAssumptions,
    pub seed: u128,
    pub slots_per_path: u64,
    pub initial_base_inventory: u64,
    pub initial_quote_inventory: u64,
    pub target_base_inventory: u64,
    pub initial_fair_price: Q64x64,
    pub quote_update_policy: QuoteUpdatePolicy,
    pub params: ReferenceQuoteParams,
    pub volatility_bps_per_slot: u16,
    pub drift_bps_per_slot: i16,
    pub trade_probability_bps: u16,
    pub max_trade_base_atoms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggregateReport {
    pub assumptions: ScenarioAssumptions,
    pub paths: u32,
    pub trades_attempted: SummaryU64,
    pub trades_filled: SummaryU64,
    pub fill_rate_bps: SummaryU16,
    pub fees_quote_atoms: SummaryU128,
    pub taker_edge_quote_atoms: SummaryI128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuoteReport {
    pub assumptions: ScenarioAssumptions,
    pub trades_attempted: u64,
    pub trades_filled: u64,
    pub rejected_stale: u64,
    pub rejected_protected: u64,
    pub rejected_inventory: u64,
    pub rejected_other: u64,
    pub quote_updates_sent: u64,
    pub quote_updates_landed: u64,
    pub quote_updates_dropped: u64,
    pub max_quote_age_slots: u64,
    pub fresh_fills: u64,
    pub aging_fills: u64,
    pub protected_fills: u64,
    pub fees_quote_atoms: u128,
    pub taker_edge_quote_atoms: i128,
    pub final_base_inventory: u64,
    pub final_quote_inventory: u64,
    pub max_abs_inventory_imbalance_bps: u16,
}

impl ReferenceQuoteReport {
    pub fn fill_rate_bps(&self) -> u16 {
        if self.trades_attempted == 0 {
            return 0;
        }
        ((self.trades_filled * 10_000) / self.trades_attempted) as u16
    }

    pub fn rejected_total(&self) -> u64 {
        self.rejected_stale
            .saturating_add(self.rejected_protected)
            .saturating_add(self.rejected_inventory)
            .saturating_add(self.rejected_other)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceQuoteAggregateReport {
    pub assumptions: ScenarioAssumptions,
    pub paths: u32,
    pub trades_attempted: SummaryU64,
    pub trades_filled: SummaryU64,
    pub fill_rate_bps: SummaryU16,
    pub rejected_stale: SummaryU64,
    pub rejected_protected: SummaryU64,
    pub rejected_inventory: SummaryU64,
    pub rejected_other: SummaryU64,
    pub quote_updates_sent: SummaryU64,
    pub quote_updates_landed: SummaryU64,
    pub quote_updates_dropped: SummaryU64,
    pub max_quote_age_slots: SummaryU64,
    pub fees_quote_atoms: SummaryU128,
    pub taker_edge_quote_atoms: SummaryI128,
    pub max_abs_inventory_imbalance_bps: SummaryU16,
}

impl ReferenceQuoteAggregateReport {
    pub fn is_single_path(&self) -> bool {
        self.paths == 1
    }
}

impl AggregateReport {
    pub fn is_single_path(&self) -> bool {
        self.paths == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryU16 {
    pub min: u16,
    pub p05: u16,
    pub mean: u16,
    pub p50: u16,
    pub p95: u16,
    pub max: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryU64 {
    pub min: u64,
    pub p05: u64,
    pub mean: u64,
    pub p50: u64,
    pub p95: u64,
    pub max: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryU128 {
    pub min: u128,
    pub p05: u128,
    pub mean: u128,
    pub p50: u128,
    pub p95: u128,
    pub max: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryI128 {
    pub min: i128,
    pub p05: i128,
    pub mean: i128,
    pub p50: i128,
    pub p95: i128,
    pub max: i128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioPackReport {
    pub reports: Vec<ReferenceQuoteAggregateReport>,
}

impl ScenarioPackReport {
    pub fn len(&self) -> usize {
        self.reports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GateSeverity {
    Pass,
    Warn,
    Block,
}

impl GateSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Block => "block",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScenarioGateThresholds {
    pub min_paths: u32,
    pub warn_min_p05_fill_rate_bps: u16,
    pub block_min_p05_fill_rate_bps: u16,
    pub warn_max_p95_stale_reject_rate_bps: u16,
    pub block_max_p95_stale_reject_rate_bps: u16,
    pub warn_max_p95_protected_reject_rate_bps: u16,
    pub block_max_p95_protected_reject_rate_bps: u16,
    pub warn_max_p95_update_drop_rate_bps: u16,
    pub block_max_p95_update_drop_rate_bps: u16,
    pub warn_max_p95_quote_age_slots: u64,
    pub block_max_p95_quote_age_slots: u64,
    pub warn_max_p95_inventory_imbalance_bps: u16,
    pub block_max_p95_inventory_imbalance_bps: u16,
}

impl ScenarioGateThresholds {
    pub const fn reference_quote_default() -> Self {
        Self {
            min_paths: 32,
            warn_min_p05_fill_rate_bps: 7_000,
            block_min_p05_fill_rate_bps: 5_000,
            warn_max_p95_stale_reject_rate_bps: 1_500,
            block_max_p95_stale_reject_rate_bps: 2_500,
            warn_max_p95_protected_reject_rate_bps: 2_000,
            block_max_p95_protected_reject_rate_bps: 3_500,
            warn_max_p95_update_drop_rate_bps: 2_500,
            block_max_p95_update_drop_rate_bps: 3_500,
            warn_max_p95_quote_age_slots: 32,
            block_max_p95_quote_age_slots: 40,
            warn_max_p95_inventory_imbalance_bps: 2_000,
            block_max_p95_inventory_imbalance_bps: 2_800,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateFinding {
    pub severity: GateSeverity,
    pub metric: &'static str,
    pub observed: u64,
    pub threshold: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioEvaluation {
    pub scenario_name: &'static str,
    pub severity: GateSeverity,
    pub findings: Vec<GateFinding>,
}

impl ScenarioEvaluation {
    pub fn passes(&self) -> bool {
        self.severity == GateSeverity::Pass
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioPackEvaluation {
    pub evaluations: Vec<ScenarioEvaluation>,
}

impl ScenarioPackEvaluation {
    pub fn severity(&self) -> GateSeverity {
        self.evaluations
            .iter()
            .map(|evaluation| evaluation.severity)
            .max()
            .unwrap_or(GateSeverity::Pass)
    }

    pub fn passes(&self) -> bool {
        self.severity() == GateSeverity::Pass
    }
}

pub fn simulate_cpmm(scenario: CpmmScenario, events: &[FlowEvent]) -> Result<SimReport, MathError> {
    let mut reserves = scenario.initial_reserves;
    let mut report = SimReport {
        assumptions: scenario.assumptions,
        trades_attempted: events.len() as u64,
        trades_filled: 0,
        trades_rejected: 0,
        fees_quote_atoms: 0,
        taker_edge_quote_atoms: 0,
        final_reserves: reserves,
    };

    for event in events {
        let base_to_quote = matches!(event.side, Side::BaseToQuote);
        match cpmm_quote_exact_in(reserves, event.amount_in, base_to_quote, scenario.fee_bps) {
            Ok(quote) => {
                report.trades_filled += 1;
                report.fees_quote_atoms = report
                    .fees_quote_atoms
                    .saturating_add(fee_in_quote_atoms(event, quote.amount_in_less_fee)?);
                report.taker_edge_quote_atoms = report
                    .taker_edge_quote_atoms
                    .saturating_add(taker_edge_quote_atoms(event, quote.amount_out)?);
                reserves = CpmmReserves {
                    base: quote.new_base,
                    quote: quote.new_quote,
                };
            }
            Err(MathError::InvalidAmount) | Err(MathError::EmptyLiquidity) => {
                report.trades_rejected += 1;
            }
            Err(error) => return Err(error),
        }
    }

    report.final_reserves = reserves;
    Ok(report)
}

pub fn simulate_generated_cpmm(
    scenario: GeneratedCpmmScenario,
) -> Result<AggregateReport, MathError> {
    let paths = scenario.assumptions.path_count.max(1);
    let mut reports = Vec::with_capacity(paths as usize);

    for path_index in 0..paths {
        let path_events = generate_flow_path(scenario, path_index)?;
        let path_scenario = CpmmScenario {
            assumptions: scenario.assumptions,
            initial_reserves: scenario.initial_reserves,
            fee_bps: scenario.fee_bps,
        };
        reports.push(simulate_cpmm(path_scenario, &path_events)?);
    }

    Ok(aggregate_reports(scenario.assumptions, &reports))
}

pub fn simulate_reference_quote(
    scenario: ReferenceQuoteScenario,
    events: &[FlowEvent],
) -> Result<ReferenceQuoteReport, MathError> {
    let slots: Vec<MarketSlot> = events
        .iter()
        .map(|event| MarketSlot {
            slot: event.slot,
            fair_price: event.fair_price,
            flow: Some(FlowOrder {
                side: event.side,
                amount_in: event.amount_in,
            }),
        })
        .collect();
    simulate_reference_quote_market_path(scenario, &slots)
}

pub fn simulate_reference_quote_market_path(
    scenario: ReferenceQuoteScenario,
    slots: &[MarketSlot],
) -> Result<ReferenceQuoteReport, MathError> {
    validate_quote_update_policy(scenario.quote_update_policy)?;
    let mut state = ReferenceQuoteState {
        base_inventory: scenario.initial_base_inventory,
        quote_inventory: scenario.initial_quote_inventory,
        target_base_inventory: scenario.target_base_inventory,
        mid_price: scenario.initial_mid_price,
        mid_publish_slot: 0,
        now_slot: 0,
        paused: false,
    };
    let mut report = ReferenceQuoteReport {
        assumptions: scenario.assumptions,
        trades_attempted: 0,
        trades_filled: 0,
        rejected_stale: 0,
        rejected_protected: 0,
        rejected_inventory: 0,
        rejected_other: 0,
        quote_updates_sent: 0,
        quote_updates_landed: 0,
        quote_updates_dropped: 0,
        max_quote_age_slots: 0,
        fresh_fills: 0,
        aging_fills: 0,
        protected_fills: 0,
        fees_quote_atoms: 0,
        taker_edge_quote_atoms: 0,
        final_base_inventory: state.base_inventory,
        final_quote_inventory: state.quote_inventory,
        max_abs_inventory_imbalance_bps: inventory_imbalance_abs_bps(
            state.base_inventory,
            state.target_base_inventory,
        ),
    };
    let mut pending_updates = Vec::new();
    let mut next_update_slot = scenario.quote_update_policy.maker_update_period_slots;
    let mut previous_slot = None;

    for slot in slots {
        if let Some(previous_slot) = previous_slot {
            if slot.slot < previous_slot {
                return Err(MathError::InvalidConfig);
            }
        }
        previous_slot = Some(slot.slot);

        while next_update_slot <= slot.slot {
            report.quote_updates_sent += 1;
            if quote_update_should_land(scenario.quote_update_policy, next_update_slot) {
                pending_updates.push(PendingQuoteUpdate {
                    publish_slot: next_update_slot,
                    landing_slot: next_update_slot
                        .saturating_add(scenario.quote_update_policy.landing_latency_slots),
                    mid_price: slot.fair_price,
                });
            } else {
                report.quote_updates_dropped += 1;
            }
            next_update_slot = next_update_slot
                .saturating_add(scenario.quote_update_policy.maker_update_period_slots);
        }

        let land_through_before_swap = match scenario.quote_update_policy.same_slot_order {
            SameSlotUpdateOrder::UpdateBeforeSwap => slot.slot,
            SameSlotUpdateOrder::SwapBeforeUpdate => slot.slot.saturating_sub(1),
        };
        report.quote_updates_landed +=
            apply_landed_quote_updates(&mut state, &mut pending_updates, land_through_before_swap);
        state.now_slot = slot.slot;
        report.max_quote_age_slots = report
            .max_quote_age_slots
            .max(state.now_slot.saturating_sub(state.mid_publish_slot));

        execute_reference_quote_slot(&mut report, &mut state, scenario.params, *slot)?;

        if scenario.quote_update_policy.same_slot_order == SameSlotUpdateOrder::SwapBeforeUpdate {
            report.quote_updates_landed +=
                apply_landed_quote_updates(&mut state, &mut pending_updates, slot.slot);
        }
    }

    report.final_base_inventory = state.base_inventory;
    report.final_quote_inventory = state.quote_inventory;
    Ok(report)
}

pub fn simulate_reference_quote_replay(
    scenario: ReferenceQuoteScenario,
    replay: &ReplayMarketPath,
) -> Result<ReferenceQuoteReport, MathError> {
    validate_quote_update_policy(scenario.quote_update_policy)?;
    validate_quote_updates(&replay.quote_updates)?;
    let mut state = ReferenceQuoteState {
        base_inventory: scenario.initial_base_inventory,
        quote_inventory: scenario.initial_quote_inventory,
        target_base_inventory: scenario.target_base_inventory,
        mid_price: scenario.initial_mid_price,
        mid_publish_slot: 0,
        now_slot: 0,
        paused: false,
    };
    let mut report = ReferenceQuoteReport {
        assumptions: scenario.assumptions,
        trades_attempted: 0,
        trades_filled: 0,
        rejected_stale: 0,
        rejected_protected: 0,
        rejected_inventory: 0,
        rejected_other: 0,
        quote_updates_sent: 0,
        quote_updates_landed: 0,
        quote_updates_dropped: 0,
        max_quote_age_slots: 0,
        fresh_fills: 0,
        aging_fills: 0,
        protected_fills: 0,
        fees_quote_atoms: 0,
        taker_edge_quote_atoms: 0,
        final_base_inventory: state.base_inventory,
        final_quote_inventory: state.quote_inventory,
        max_abs_inventory_imbalance_bps: inventory_imbalance_abs_bps(
            state.base_inventory,
            state.target_base_inventory,
        ),
    };
    let mut pending_updates = Vec::new();
    let mut update_index = 0usize;
    let mut previous_slot = None;

    for slot in &replay.slots {
        if let Some(previous_slot) = previous_slot {
            if slot.slot < previous_slot {
                return Err(MathError::InvalidConfig);
            }
        }
        previous_slot = Some(slot.slot);

        while let Some(update) = replay.quote_updates.get(update_index) {
            if update.publish_slot > slot.slot {
                break;
            }
            report.quote_updates_sent += 1;
            match update.landing_slot {
                Some(landing_slot) => pending_updates.push(PendingQuoteUpdate {
                    publish_slot: update.publish_slot,
                    landing_slot,
                    mid_price: update.mid_price,
                }),
                None => report.quote_updates_dropped += 1,
            }
            update_index += 1;
        }

        let land_through_before_swap = match scenario.quote_update_policy.same_slot_order {
            SameSlotUpdateOrder::UpdateBeforeSwap => slot.slot,
            SameSlotUpdateOrder::SwapBeforeUpdate => slot.slot.saturating_sub(1),
        };
        report.quote_updates_landed +=
            apply_landed_quote_updates(&mut state, &mut pending_updates, land_through_before_swap);
        state.now_slot = slot.slot;
        report.max_quote_age_slots = report
            .max_quote_age_slots
            .max(state.now_slot.saturating_sub(state.mid_publish_slot));
        execute_reference_quote_slot(&mut report, &mut state, scenario.params, *slot)?;

        if scenario.quote_update_policy.same_slot_order == SameSlotUpdateOrder::SwapBeforeUpdate {
            report.quote_updates_landed +=
                apply_landed_quote_updates(&mut state, &mut pending_updates, slot.slot);
        }
    }

    report.final_base_inventory = state.base_inventory;
    report.final_quote_inventory = state.quote_inventory;
    Ok(report)
}

pub fn simulate_generated_reference_quote(
    scenario: GeneratedReferenceQuoteScenario,
) -> Result<ReferenceQuoteAggregateReport, MathError> {
    let paths = scenario.assumptions.path_count.max(1);
    let mut reports = Vec::with_capacity(paths as usize);

    for path_index in 0..paths {
        let path_slots = generate_reference_market_path(scenario, path_index)?;
        let path_scenario = ReferenceQuoteScenario {
            assumptions: scenario.assumptions,
            initial_base_inventory: scenario.initial_base_inventory,
            initial_quote_inventory: scenario.initial_quote_inventory,
            target_base_inventory: scenario.target_base_inventory,
            initial_mid_price: scenario.initial_fair_price,
            quote_update_policy: scenario.quote_update_policy,
            params: scenario.params,
        };
        reports.push(simulate_reference_quote_market_path(
            path_scenario,
            &path_slots,
        )?);
    }

    Ok(aggregate_reference_reports(scenario.assumptions, &reports))
}

pub fn simulate_reference_quote_scenario_pack(
    scenarios: &[GeneratedReferenceQuoteScenario],
) -> Result<ScenarioPackReport, MathError> {
    let mut reports = Vec::with_capacity(scenarios.len());
    for scenario in scenarios {
        reports.push(simulate_generated_reference_quote(*scenario)?);
    }
    Ok(ScenarioPackReport { reports })
}

pub fn evaluate_reference_quote_report(
    report: &ReferenceQuoteAggregateReport,
    thresholds: ScenarioGateThresholds,
) -> ScenarioEvaluation {
    let mut findings = Vec::new();

    if report.paths < thresholds.min_paths {
        findings.push(GateFinding {
            severity: GateSeverity::Block,
            metric: "paths",
            observed: report.paths as u64,
            threshold: thresholds.min_paths as u64,
        });
    }
    push_min_gate(
        &mut findings,
        "fill_rate_p05_bps",
        report.fill_rate_bps.p05 as u64,
        thresholds.warn_min_p05_fill_rate_bps as u64,
        thresholds.block_min_p05_fill_rate_bps as u64,
    );
    push_max_gate(
        &mut findings,
        "stale_reject_rate_p95_bps",
        rate_bps(report.rejected_stale.p95, report.trades_attempted.p95) as u64,
        thresholds.warn_max_p95_stale_reject_rate_bps as u64,
        thresholds.block_max_p95_stale_reject_rate_bps as u64,
    );
    push_max_gate(
        &mut findings,
        "protected_reject_rate_p95_bps",
        rate_bps(report.rejected_protected.p95, report.trades_attempted.p95) as u64,
        thresholds.warn_max_p95_protected_reject_rate_bps as u64,
        thresholds.block_max_p95_protected_reject_rate_bps as u64,
    );
    push_max_gate(
        &mut findings,
        "update_drop_rate_p95_bps",
        rate_bps(
            report.quote_updates_dropped.p95,
            report.quote_updates_sent.p95,
        ) as u64,
        thresholds.warn_max_p95_update_drop_rate_bps as u64,
        thresholds.block_max_p95_update_drop_rate_bps as u64,
    );
    push_max_gate(
        &mut findings,
        "max_quote_age_p95_slots",
        report.max_quote_age_slots.p95,
        thresholds.warn_max_p95_quote_age_slots,
        thresholds.block_max_p95_quote_age_slots,
    );
    push_max_gate(
        &mut findings,
        "inventory_imbalance_p95_bps",
        report.max_abs_inventory_imbalance_bps.p95 as u64,
        thresholds.warn_max_p95_inventory_imbalance_bps as u64,
        thresholds.block_max_p95_inventory_imbalance_bps as u64,
    );

    let severity = findings
        .iter()
        .map(|finding| finding.severity)
        .max()
        .unwrap_or(GateSeverity::Pass);
    ScenarioEvaluation {
        scenario_name: report.assumptions.name,
        severity,
        findings,
    }
}

pub fn evaluate_scenario_pack(
    report: &ScenarioPackReport,
    thresholds: ScenarioGateThresholds,
) -> ScenarioPackEvaluation {
    ScenarioPackEvaluation {
        evaluations: report
            .reports
            .iter()
            .map(|report| evaluate_reference_quote_report(report, thresholds))
            .collect(),
    }
}

pub fn default_reference_quote_scenario_pack(seed: u128) -> [GeneratedReferenceQuoteScenario; 3] {
    let base_params = ReferenceQuoteParams {
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
    };
    let base = GeneratedReferenceQuoteScenario {
        assumptions: ScenarioAssumptions {
            name: "calm-fast-updates",
            flow_model: "seeded random walk fair price with Bernoulli flow",
            landing_model: "fast deterministic maker updates",
            path_count: 64,
        },
        seed,
        slots_per_path: 1_000,
        initial_base_inventory: 1_000_000,
        initial_quote_inventory: 30_000_000_000,
        target_base_inventory: 1_000_000,
        initial_fair_price: Q64x64::from_int(30_000),
        quote_update_policy: QuoteUpdatePolicy {
            maker_update_period_slots: 4,
            landing_latency_slots: 1,
            update_success_probability_bps: 9_900,
            failure_seed: seed ^ 0x616c6d5f66617374,
            same_slot_order: SameSlotUpdateOrder::SwapBeforeUpdate,
        },
        params: base_params,
        volatility_bps_per_slot: 3,
        drift_bps_per_slot: 0,
        trade_probability_bps: 1_500,
        max_trade_base_atoms: 1_000,
    };

    let mut trending = base;
    trending.assumptions = ScenarioAssumptions {
        name: "trending-slow-updates",
        flow_model: "positive drift fair price with Bernoulli flow",
        landing_model: "slow maker updates with same-slot swap precedence",
        path_count: 64,
    };
    trending.seed = seed ^ 0x7472656e64696e67;
    trending.quote_update_policy = QuoteUpdatePolicy {
        maker_update_period_slots: 9,
        landing_latency_slots: 4,
        update_success_probability_bps: 8_500,
        failure_seed: seed ^ 0x7472656e645f6c616e64,
        same_slot_order: SameSlotUpdateOrder::SwapBeforeUpdate,
    };
    trending.volatility_bps_per_slot = 7;
    trending.drift_bps_per_slot = 2;
    trending.trade_probability_bps = 2_000;

    let mut volatile = base;
    volatile.assumptions = ScenarioAssumptions {
        name: "volatile-dropped-updates",
        flow_model: "high-volatility random walk with Bernoulli flow",
        landing_model: "dropped quote updates and long landing latency",
        path_count: 64,
    };
    volatile.seed = seed ^ 0x766f6c6174696c65;
    volatile.quote_update_policy = QuoteUpdatePolicy {
        maker_update_period_slots: 8,
        landing_latency_slots: 6,
        update_success_probability_bps: 6_500,
        failure_seed: seed ^ 0x766f6c5f64726f70,
        same_slot_order: SameSlotUpdateOrder::SwapBeforeUpdate,
    };
    volatile.volatility_bps_per_slot = 18;
    volatile.trade_probability_bps = 2_500;
    volatile.max_trade_base_atoms = 1_500;

    [base, trending, volatile]
}

pub fn generate_flow_path(
    scenario: GeneratedCpmmScenario,
    path_index: u32,
) -> Result<Vec<FlowEvent>, MathError> {
    let mut rng = DeterministicRng::new(
        scenario
            .seed
            .wrapping_add((path_index as u128).wrapping_mul(0x9e3779b97f4a7c15)),
    );
    let mut fair_price = scenario.initial_fair_price.to_int_floor().max(1);
    let mut events = Vec::new();

    for slot in 0..scenario.slots_per_path {
        fair_price = update_integer_price(
            fair_price,
            scenario.drift_bps_per_slot,
            scenario.volatility_bps_per_slot,
            &mut rng,
        );

        if rng.next_bps() >= scenario.trade_probability_bps.min(10_000) {
            continue;
        }

        let side = if rng.next_bool() {
            Side::BaseToQuote
        } else {
            Side::QuoteToBase
        };
        let base_size = 1 + (rng.next_u64() % scenario.max_trade_base_atoms.max(1));
        let fair_q64 = Q64x64::from_int(fair_price);
        let amount_in = match side {
            Side::BaseToQuote => base_size,
            Side::QuoteToBase => fair_q64.mul_amount_floor(base_size)?,
        };

        events.push(FlowEvent {
            slot,
            side,
            amount_in,
            fair_price: fair_q64,
        });
    }

    Ok(events)
}

pub fn generate_reference_flow_path(
    scenario: GeneratedReferenceQuoteScenario,
    path_index: u32,
) -> Result<Vec<FlowEvent>, MathError> {
    Ok(generate_reference_market_path(scenario, path_index)?
        .into_iter()
        .filter_map(|slot| {
            slot.flow.map(|flow| FlowEvent {
                slot: slot.slot,
                side: flow.side,
                amount_in: flow.amount_in,
                fair_price: slot.fair_price,
            })
        })
        .collect())
}

pub fn generate_reference_market_path(
    scenario: GeneratedReferenceQuoteScenario,
    path_index: u32,
) -> Result<Vec<MarketSlot>, MathError> {
    let mut rng = DeterministicRng::new(
        scenario
            .seed
            .wrapping_add((path_index as u128).wrapping_mul(0x517cc1b727220a95)),
    );
    let mut fair_price = scenario.initial_fair_price.to_int_floor().max(1);
    let mut slots = Vec::with_capacity(scenario.slots_per_path as usize);

    for slot in 0..scenario.slots_per_path {
        fair_price = update_integer_price(
            fair_price,
            scenario.drift_bps_per_slot,
            scenario.volatility_bps_per_slot,
            &mut rng,
        );
        let fair_q64 = Q64x64::from_int(fair_price);
        let flow = if rng.next_bps() < scenario.trade_probability_bps.min(10_000) {
            let side = if rng.next_bool() {
                Side::BaseToQuote
            } else {
                Side::QuoteToBase
            };
            let base_size = 1 + (rng.next_u64() % scenario.max_trade_base_atoms.max(1));
            let amount_in = match side {
                Side::BaseToQuote => base_size,
                Side::QuoteToBase => fair_q64.mul_amount_floor(base_size)?,
            };
            Some(FlowOrder { side, amount_in })
        } else {
            None
        };

        slots.push(MarketSlot {
            slot,
            fair_price: fair_q64,
            flow,
        });
    }

    Ok(slots)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingQuoteUpdate {
    publish_slot: u64,
    landing_slot: u64,
    mid_price: Q64x64,
}

fn validate_quote_update_policy(policy: QuoteUpdatePolicy) -> Result<(), MathError> {
    if policy.maker_update_period_slots == 0 || policy.update_success_probability_bps > 10_000 {
        return Err(MathError::InvalidConfig);
    }
    Ok(())
}

fn quote_update_should_land(policy: QuoteUpdatePolicy, publish_slot: u64) -> bool {
    if policy.update_success_probability_bps == 10_000 {
        return true;
    }
    if policy.update_success_probability_bps == 0 {
        return false;
    }
    let mut rng = DeterministicRng::new(
        policy
            .failure_seed
            .wrapping_add((publish_slot as u128).wrapping_mul(0x9e3779b97f4a7c15)),
    );
    rng.next_bps() < policy.update_success_probability_bps
}

fn apply_landed_quote_updates(
    state: &mut ReferenceQuoteState,
    pending_updates: &mut Vec<PendingQuoteUpdate>,
    land_through_slot: u64,
) -> u64 {
    let mut landed = 0;
    let mut latest = None;
    pending_updates.retain(|update| {
        if update.landing_slot <= land_through_slot {
            landed += 1;
            latest = Some(*update);
            false
        } else {
            true
        }
    });

    if let Some(update) = latest {
        state.mid_price = update.mid_price;
        state.mid_publish_slot = update.publish_slot;
    }

    landed
}

fn execute_reference_quote_slot(
    report: &mut ReferenceQuoteReport,
    state: &mut ReferenceQuoteState,
    params: ReferenceQuoteParams,
    slot: MarketSlot,
) -> Result<(), MathError> {
    let Some(flow) = slot.flow else {
        return Ok(());
    };

    report.trades_attempted += 1;
    let event = FlowEvent {
        slot: slot.slot,
        side: flow.side,
        amount_in: flow.amount_in,
        fair_price: slot.fair_price,
    };
    let base_to_quote = matches!(flow.side, Side::BaseToQuote);
    match reference_quote_exact_in(*state, params, flow.amount_in, base_to_quote) {
        Ok(quote) => {
            report.trades_filled += 1;
            match quote.age_state {
                QuoteAgeState::Fresh => report.fresh_fills += 1,
                QuoteAgeState::Aging => report.aging_fills += 1,
                QuoteAgeState::Protected => report.protected_fills += 1,
                QuoteAgeState::Expired | QuoteAgeState::Paused => {}
            }
            report.fees_quote_atoms = report
                .fees_quote_atoms
                .saturating_add(fee_in_quote_atoms(&event, quote.amount_in_less_fee)?);
            report.taker_edge_quote_atoms = report
                .taker_edge_quote_atoms
                .saturating_add(taker_edge_quote_atoms(&event, quote.amount_out)?);
            state.base_inventory = quote.new_base_inventory;
            state.quote_inventory = quote.new_quote_inventory;
            report.max_abs_inventory_imbalance_bps =
                report
                    .max_abs_inventory_imbalance_bps
                    .max(inventory_imbalance_abs_bps(
                        state.base_inventory,
                        state.target_base_inventory,
                    ));
        }
        Err(MathError::StaleQuote) => report.rejected_stale += 1,
        Err(MathError::QuoteProtected) => report.rejected_protected += 1,
        Err(MathError::InventoryBand) => report.rejected_inventory += 1,
        Err(MathError::InvalidAmount) | Err(MathError::EmptyLiquidity) => {
            report.rejected_other += 1;
        }
        Err(error) => return Err(error),
    }

    Ok(())
}

fn validate_quote_updates(updates: &[QuoteUpdateEvent]) -> Result<(), MathError> {
    let mut previous_publish_slot = None;
    for update in updates {
        if let Some(previous_publish_slot) = previous_publish_slot {
            if update.publish_slot < previous_publish_slot {
                return Err(MathError::InvalidConfig);
            }
        }
        if update
            .landing_slot
            .is_some_and(|landing_slot| landing_slot < update.publish_slot)
        {
            return Err(MathError::InvalidConfig);
        }
        previous_publish_slot = Some(update.publish_slot);
    }
    Ok(())
}

fn update_integer_price(
    price: u64,
    drift_bps_per_slot: i16,
    volatility_bps_per_slot: u16,
    rng: &mut DeterministicRng,
) -> u64 {
    let vol = volatility_bps_per_slot as i32;
    let random_delta = if vol == 0 {
        0
    } else {
        (rng.next_u64() % ((vol * 2 + 1) as u64)) as i32 - vol
    };
    let total_bps = drift_bps_per_slot as i32 + random_delta;
    let multiplier = (10_000 + total_bps).max(1) as u128;
    let next_price = (((price as u128) * multiplier) / 10_000).max(1);
    next_price.min(u64::MAX as u128) as u64
}

fn aggregate_reports(assumptions: ScenarioAssumptions, reports: &[SimReport]) -> AggregateReport {
    let paths = reports.len().max(1) as u128;

    AggregateReport {
        assumptions,
        paths: reports.len() as u32,
        trades_attempted: summarize_u64(reports.iter().map(|report| report.trades_attempted)),
        trades_filled: summarize_u64(reports.iter().map(|report| report.trades_filled)),
        fill_rate_bps: summarize_u16(reports.iter().map(SimReport::fill_rate_bps)),
        fees_quote_atoms: summarize_u128(reports.iter().map(|report| report.fees_quote_atoms)),
        taker_edge_quote_atoms: summarize_i128(
            reports.iter().map(|report| report.taker_edge_quote_atoms),
            paths,
        ),
    }
}

fn aggregate_reference_reports(
    assumptions: ScenarioAssumptions,
    reports: &[ReferenceQuoteReport],
) -> ReferenceQuoteAggregateReport {
    let paths = reports.len().max(1) as u128;

    ReferenceQuoteAggregateReport {
        assumptions,
        paths: reports.len() as u32,
        trades_attempted: summarize_u64(reports.iter().map(|report| report.trades_attempted)),
        trades_filled: summarize_u64(reports.iter().map(|report| report.trades_filled)),
        fill_rate_bps: summarize_u16(reports.iter().map(ReferenceQuoteReport::fill_rate_bps)),
        rejected_stale: summarize_u64(reports.iter().map(|report| report.rejected_stale)),
        rejected_protected: summarize_u64(reports.iter().map(|report| report.rejected_protected)),
        rejected_inventory: summarize_u64(reports.iter().map(|report| report.rejected_inventory)),
        rejected_other: summarize_u64(reports.iter().map(|report| report.rejected_other)),
        quote_updates_sent: summarize_u64(reports.iter().map(|report| report.quote_updates_sent)),
        quote_updates_landed: summarize_u64(
            reports.iter().map(|report| report.quote_updates_landed),
        ),
        quote_updates_dropped: summarize_u64(
            reports.iter().map(|report| report.quote_updates_dropped),
        ),
        max_quote_age_slots: summarize_u64(reports.iter().map(|report| report.max_quote_age_slots)),
        fees_quote_atoms: summarize_u128(reports.iter().map(|report| report.fees_quote_atoms)),
        taker_edge_quote_atoms: summarize_i128(
            reports.iter().map(|report| report.taker_edge_quote_atoms),
            paths,
        ),
        max_abs_inventory_imbalance_bps: summarize_u16(
            reports
                .iter()
                .map(|report| report.max_abs_inventory_imbalance_bps),
        ),
    }
}

fn summarize_u16(values: impl Iterator<Item = u16>) -> SummaryU16 {
    let mut values: Vec<u16> = values.collect();
    if values.is_empty() {
        return SummaryU16 {
            min: 0,
            p05: 0,
            mean: 0,
            p50: 0,
            p95: 0,
            max: 0,
        };
    }
    values.sort_unstable();
    let mut sum = 0u128;
    for value in &values {
        sum += *value as u128;
    }
    let count = values.len() as u128;

    SummaryU16 {
        min: values[0],
        p05: percentile_u16(&values, 5),
        mean: (sum / count) as u16,
        p50: percentile_u16(&values, 50),
        p95: percentile_u16(&values, 95),
        max: values[values.len() - 1],
    }
}

fn summarize_u64(values: impl Iterator<Item = u64>) -> SummaryU64 {
    let mut values: Vec<u64> = values.collect();
    if values.is_empty() {
        return SummaryU64 {
            min: 0,
            p05: 0,
            mean: 0,
            p50: 0,
            p95: 0,
            max: 0,
        };
    }
    values.sort_unstable();
    let mut sum = 0u128;
    for value in &values {
        sum += *value as u128;
    }
    let count = values.len() as u128;

    SummaryU64 {
        min: values[0],
        p05: percentile_u64(&values, 5),
        mean: (sum / count) as u64,
        p50: percentile_u64(&values, 50),
        p95: percentile_u64(&values, 95),
        max: values[values.len() - 1],
    }
}

fn summarize_u128(values: impl Iterator<Item = u128>) -> SummaryU128 {
    let mut values: Vec<u128> = values.collect();
    if values.is_empty() {
        return SummaryU128 {
            min: 0,
            p05: 0,
            mean: 0,
            p50: 0,
            p95: 0,
            max: 0,
        };
    }
    values.sort_unstable();
    let mut sum = 0u128;
    for value in &values {
        sum = sum.saturating_add(*value);
    }
    let count = values.len() as u128;

    SummaryU128 {
        min: values[0],
        p05: percentile_u128(&values, 5),
        mean: sum / count,
        p50: percentile_u128(&values, 50),
        p95: percentile_u128(&values, 95),
        max: values[values.len() - 1],
    }
}

fn summarize_i128(values: impl Iterator<Item = i128>, count_hint: u128) -> SummaryI128 {
    let mut values: Vec<i128> = values.collect();
    if values.is_empty() {
        return SummaryI128 {
            min: 0,
            p05: 0,
            mean: 0,
            p50: 0,
            p95: 0,
            max: 0,
        };
    }
    values.sort_unstable();
    let mut sum = 0i128;
    for value in &values {
        sum = sum.saturating_add(*value);
    }
    let observed_count = values.len() as u128;
    let count = observed_count.max(count_hint).max(1);
    SummaryI128 {
        min: values[0],
        p05: percentile_i128(&values, 5),
        mean: sum / (count as i128),
        p50: percentile_i128(&values, 50),
        p95: percentile_i128(&values, 95),
        max: values[values.len() - 1],
    }
}

fn percentile_index(len: usize, percentile: u32) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len - 1;
    ((last as u128) * (percentile as u128) / 100) as usize
}

fn percentile_u16(values: &[u16], percentile: u32) -> u16 {
    values[percentile_index(values.len(), percentile)]
}

fn percentile_u64(values: &[u64], percentile: u32) -> u64 {
    values[percentile_index(values.len(), percentile)]
}

fn percentile_u128(values: &[u128], percentile: u32) -> u128 {
    values[percentile_index(values.len(), percentile)]
}

fn percentile_i128(values: &[i128], percentile: u32) -> i128 {
    values[percentile_index(values.len(), percentile)]
}

fn push_min_gate(
    findings: &mut Vec<GateFinding>,
    metric: &'static str,
    observed: u64,
    warn_threshold: u64,
    block_threshold: u64,
) {
    if observed < block_threshold {
        findings.push(GateFinding {
            severity: GateSeverity::Block,
            metric,
            observed,
            threshold: block_threshold,
        });
    } else if observed < warn_threshold {
        findings.push(GateFinding {
            severity: GateSeverity::Warn,
            metric,
            observed,
            threshold: warn_threshold,
        });
    }
}

fn push_max_gate(
    findings: &mut Vec<GateFinding>,
    metric: &'static str,
    observed: u64,
    warn_threshold: u64,
    block_threshold: u64,
) {
    if observed > block_threshold {
        findings.push(GateFinding {
            severity: GateSeverity::Block,
            metric,
            observed,
            threshold: block_threshold,
        });
    } else if observed > warn_threshold {
        findings.push(GateFinding {
            severity: GateSeverity::Warn,
            metric,
            observed,
            threshold: warn_threshold,
        });
    }
}

fn rate_bps(numerator: u64, denominator: u64) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((numerator.saturating_mul(10_000)) / denominator).min(u16::MAX as u64) as u16
}

fn inventory_imbalance_abs_bps(base_inventory: u64, target_base_inventory: u64) -> u16 {
    if target_base_inventory == 0 {
        return u16::MAX;
    }
    let delta = (base_inventory as i128) - (target_base_inventory as i128);
    let abs_delta = if delta < 0 {
        delta.saturating_neg()
    } else {
        delta
    };
    let bps = (abs_delta * 10_000) / (target_base_inventory as i128);
    bps.min(u16::MAX as i128) as u16
}

struct DeterministicRng {
    state: u128,
}

impl DeterministicRng {
    fn new(seed: u128) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(0xda942042e4dd58b5d1b54a32d192ed03)
            .wrapping_add(0x9e3779b97f4a7c15f39cc0605cedc835);
        self.state ^= self.state >> 29;
        self.state ^= self.state << 37;
        self.state ^= self.state >> 43;
        (self.state >> 64) as u64
    }

    fn next_bps(&mut self) -> u16 {
        (self.next_u64() % 10_000) as u16
    }

    fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }
}

fn fee_in_quote_atoms(event: &FlowEvent, amount_in_less_fee: u64) -> Result<u128, MathError> {
    let fee_atoms = event.amount_in.saturating_sub(amount_in_less_fee);
    match event.side {
        Side::BaseToQuote => Ok(event.fair_price.mul_amount_floor(fee_atoms)? as u128),
        Side::QuoteToBase => Ok(fee_atoms as u128),
    }
}

fn taker_edge_quote_atoms(event: &FlowEvent, amount_out: u64) -> Result<i128, MathError> {
    match event.side {
        Side::BaseToQuote => {
            let fair_quote_out = event.fair_price.mul_amount_floor(event.amount_in)?;
            Ok((amount_out as i128).saturating_sub(fair_quote_out as i128))
        }
        Side::QuoteToBase => {
            let fair_base_out = event.fair_price.div_amount_floor(event.amount_in)?;
            let base_edge = (amount_out as i128).saturating_sub(fair_base_out as i128);
            let edge_abs_quote = event
                .fair_price
                .mul_amount_floor(base_edge.unsigned_abs() as u64)?;
            if base_edge >= 0 {
                Ok(edge_abs_quote as i128)
            } else {
                Ok(-(edge_abs_quote as i128))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assumptions(path_count: u32) -> ScenarioAssumptions {
        ScenarioAssumptions {
            name: "unit",
            flow_model: "deterministic fixture",
            landing_model: "instant deterministic landing",
            path_count,
        }
    }

    #[test]
    fn cpmm_sim_reports_assumptions_and_fill_rate() {
        let scenario = CpmmScenario {
            assumptions: assumptions(1),
            initial_reserves: CpmmReserves {
                base: 1_000_000,
                quote: 30_000_000_000,
            },
            fee_bps: 30,
        };
        let events = [
            FlowEvent {
                slot: 1,
                side: Side::BaseToQuote,
                amount_in: 1_000,
                fair_price: Q64x64::from_int(30_000),
            },
            FlowEvent {
                slot: 2,
                side: Side::QuoteToBase,
                amount_in: 30_000_000,
                fair_price: Q64x64::from_int(30_000),
            },
        ];

        let report = simulate_cpmm(scenario, &events).unwrap();
        assert_eq!(report.trades_attempted, 2);
        assert_eq!(report.trades_filled, 2);
        assert_eq!(report.fill_rate_bps(), 10_000);
        assert!(report.is_single_path());
        assert!(report.fees_quote_atoms > 0);
    }

    #[test]
    fn cpmm_sim_rejects_dust_without_aborting_scenario() {
        let scenario = CpmmScenario {
            assumptions: assumptions(3),
            initial_reserves: CpmmReserves {
                base: 1_000_000,
                quote: 30_000_000_000,
            },
            fee_bps: 9_999,
        };
        let events = [FlowEvent {
            slot: 1,
            side: Side::BaseToQuote,
            amount_in: 1,
            fair_price: Q64x64::from_int(30_000),
        }];

        let report = simulate_cpmm(scenario, &events).unwrap();
        assert_eq!(report.trades_attempted, 1);
        assert_eq!(report.trades_filled, 0);
        assert_eq!(report.trades_rejected, 1);
        assert!(!report.is_single_path());
    }

    #[test]
    fn generated_paths_are_deterministic() {
        let scenario = GeneratedCpmmScenario {
            assumptions: assumptions(8),
            seed: 42,
            slots_per_path: 100,
            initial_reserves: CpmmReserves {
                base: 1_000_000,
                quote: 30_000_000_000,
            },
            initial_fair_price: Q64x64::from_int(30_000),
            fee_bps: 30,
            volatility_bps_per_slot: 5,
            drift_bps_per_slot: 0,
            trade_probability_bps: 2_000,
            max_trade_base_atoms: 1_000,
        };

        let left = simulate_generated_cpmm(scenario).unwrap();
        let right = simulate_generated_cpmm(scenario).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.paths, 8);
        assert!(!left.is_single_path());
        assert!(left.trades_attempted.mean > 0);
    }

    #[test]
    fn generated_flow_respects_zero_trade_probability() {
        let scenario = GeneratedCpmmScenario {
            assumptions: assumptions(4),
            seed: 7,
            slots_per_path: 50,
            initial_reserves: CpmmReserves {
                base: 1_000_000,
                quote: 30_000_000_000,
            },
            initial_fair_price: Q64x64::from_int(30_000),
            fee_bps: 30,
            volatility_bps_per_slot: 5,
            drift_bps_per_slot: 0,
            trade_probability_bps: 0,
            max_trade_base_atoms: 1_000,
        };

        let report = simulate_generated_cpmm(scenario).unwrap();
        assert_eq!(report.trades_attempted.max, 0);
        assert_eq!(report.trades_filled.max, 0);
        assert_eq!(report.fill_rate_bps.max, 0);
    }

    #[test]
    fn reference_quote_sim_tracks_age_rejections_and_inventory() {
        let scenario = ReferenceQuoteScenario {
            assumptions: assumptions(1),
            initial_base_inventory: 1_000_000,
            initial_quote_inventory: 30_000_000_000,
            target_base_inventory: 1_000_000,
            initial_mid_price: Q64x64::from_int(30_000),
            quote_update_policy: QuoteUpdatePolicy::instant(100),
            params: reference_params(),
        };
        let events = [
            FlowEvent {
                slot: 1,
                side: Side::BaseToQuote,
                amount_in: 500,
                fair_price: Q64x64::from_int(30_000),
            },
            FlowEvent {
                slot: 13,
                side: Side::BaseToQuote,
                amount_in: 2_000,
                fair_price: Q64x64::from_int(30_000),
            },
            FlowEvent {
                slot: 21,
                side: Side::BaseToQuote,
                amount_in: 500,
                fair_price: Q64x64::from_int(30_000),
            },
        ];

        let report = simulate_reference_quote(scenario, &events).unwrap();
        assert_eq!(report.trades_attempted, 3);
        assert_eq!(report.trades_filled, 1);
        assert_eq!(report.rejected_protected, 1);
        assert_eq!(report.rejected_stale, 1);
        assert_eq!(report.rejected_total(), 2);
        assert!(report.max_abs_inventory_imbalance_bps > 0);
    }

    #[test]
    fn generated_reference_quote_paths_are_deterministic() {
        let scenario = GeneratedReferenceQuoteScenario {
            assumptions: assumptions(8),
            seed: 99,
            slots_per_path: 100,
            initial_base_inventory: 1_000_000,
            initial_quote_inventory: 30_000_000_000,
            target_base_inventory: 1_000_000,
            initial_fair_price: Q64x64::from_int(30_000),
            quote_update_policy: QuoteUpdatePolicy::instant(6),
            params: reference_params(),
            volatility_bps_per_slot: 5,
            drift_bps_per_slot: 0,
            trade_probability_bps: 2_000,
            max_trade_base_atoms: 1_000,
        };

        let left = simulate_generated_reference_quote(scenario).unwrap();
        let right = simulate_generated_reference_quote(scenario).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.paths, 8);
        assert!(!left.is_single_path());
    }

    #[test]
    fn summary_reports_quantiles_without_hiding_extremes() {
        let summary = summarize_u64((1u64..=20).map(|value| value * 10));
        assert_eq!(summary.min, 10);
        assert_eq!(summary.p05, 10);
        assert_eq!(summary.mean, 105);
        assert_eq!(summary.p50, 100);
        assert_eq!(summary.p95, 190);
        assert_eq!(summary.max, 200);

        let signed = summarize_i128([-30, -10, 0, 10, 30].into_iter(), 0);
        assert_eq!(signed.min, -30);
        assert_eq!(signed.p50, 0);
        assert_eq!(signed.max, 30);
    }

    #[test]
    fn default_reference_quote_scenario_pack_is_deterministic() {
        let scenarios = default_reference_quote_scenario_pack(55);
        assert_eq!(scenarios.len(), 3);
        assert_eq!(scenarios[0].assumptions.name, "calm-fast-updates");
        assert_eq!(scenarios[1].assumptions.name, "trending-slow-updates");
        assert_eq!(scenarios[2].assumptions.name, "volatile-dropped-updates");

        let left = simulate_reference_quote_scenario_pack(&scenarios).unwrap();
        let right = simulate_reference_quote_scenario_pack(&scenarios).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.len(), 3);
        assert!(left.reports.iter().all(|report| report.paths == 64));
        assert!(
            left.reports[0].fill_rate_bps.mean > left.reports[2].fill_rate_bps.mean,
            "calm scenario should fill more reliably than volatile dropped-update scenario"
        );
    }

    #[test]
    fn scenario_pack_evaluation_surfaces_warnings_and_blockers() {
        let scenarios = default_reference_quote_scenario_pack(0x6d657461_616d6d5f_7061636b);
        let report = simulate_reference_quote_scenario_pack(&scenarios).unwrap();
        let evaluation =
            evaluate_scenario_pack(&report, ScenarioGateThresholds::reference_quote_default());

        assert_eq!(evaluation.severity(), GateSeverity::Block);
        assert!(evaluation.evaluations[0].passes());
        assert_eq!(evaluation.evaluations[1].severity, GateSeverity::Warn);
        assert_eq!(evaluation.evaluations[2].severity, GateSeverity::Block);
        assert!(evaluation.evaluations[2]
            .findings
            .iter()
            .any(|finding| finding.metric == "fill_rate_p05_bps"
                && finding.severity == GateSeverity::Block));
        assert!(evaluation.evaluations[2]
            .findings
            .iter()
            .any(|finding| finding.metric == "update_drop_rate_p95_bps"
                && finding.severity == GateSeverity::Block));
    }

    #[test]
    fn generated_reference_quote_paths_track_landing_failures() {
        let scenario = GeneratedReferenceQuoteScenario {
            assumptions: assumptions(4),
            seed: 123,
            slots_per_path: 80,
            initial_base_inventory: 1_000_000,
            initial_quote_inventory: 30_000_000_000,
            target_base_inventory: 1_000_000,
            initial_fair_price: Q64x64::from_int(30_000),
            quote_update_policy: QuoteUpdatePolicy {
                maker_update_period_slots: 4,
                landing_latency_slots: 3,
                update_success_probability_bps: 0,
                failure_seed: 77,
                same_slot_order: SameSlotUpdateOrder::UpdateBeforeSwap,
            },
            params: reference_params(),
            volatility_bps_per_slot: 5,
            drift_bps_per_slot: 0,
            trade_probability_bps: 10_000,
            max_trade_base_atoms: 500,
        };

        let report = simulate_generated_reference_quote(scenario).unwrap();
        assert!(report.quote_updates_sent.mean > 0);
        assert_eq!(report.quote_updates_dropped, report.quote_updates_sent);
        assert_eq!(report.quote_updates_landed.max, 0);
        assert!(report.rejected_stale.max > 0);
        assert!(report.max_quote_age_slots.max >= reference_params().expire_slots);
    }

    #[test]
    fn same_slot_update_order_changes_swap_observed_quote() {
        let slots = [MarketSlot {
            slot: 1,
            fair_price: Q64x64::from_int(31_000),
            flow: Some(FlowOrder {
                side: Side::BaseToQuote,
                amount_in: 500,
            }),
        }];
        let mut before_policy = QuoteUpdatePolicy::instant(1);
        before_policy.same_slot_order = SameSlotUpdateOrder::UpdateBeforeSwap;
        let mut after_policy = before_policy;
        after_policy.same_slot_order = SameSlotUpdateOrder::SwapBeforeUpdate;

        let before = simulate_reference_quote_market_path(
            ReferenceQuoteScenario {
                assumptions: assumptions(1),
                initial_base_inventory: 1_000_000,
                initial_quote_inventory: 30_000_000_000,
                target_base_inventory: 1_000_000,
                initial_mid_price: Q64x64::from_int(30_000),
                quote_update_policy: before_policy,
                params: reference_params(),
            },
            &slots,
        )
        .unwrap();
        let after = simulate_reference_quote_market_path(
            ReferenceQuoteScenario {
                assumptions: assumptions(1),
                initial_base_inventory: 1_000_000,
                initial_quote_inventory: 30_000_000_000,
                target_base_inventory: 1_000_000,
                initial_mid_price: Q64x64::from_int(30_000),
                quote_update_policy: after_policy,
                params: reference_params(),
            },
            &slots,
        )
        .unwrap();

        assert_eq!(before.quote_updates_landed, 1);
        assert_eq!(after.quote_updates_landed, 1);
        assert_eq!(before.max_quote_age_slots, 0);
        assert_eq!(after.max_quote_age_slots, 1);
        assert!(before.taker_edge_quote_atoms > after.taker_edge_quote_atoms);
    }

    #[test]
    fn reference_quote_market_path_rejects_unsorted_slots() {
        let scenario = ReferenceQuoteScenario {
            assumptions: assumptions(1),
            initial_base_inventory: 1_000_000,
            initial_quote_inventory: 30_000_000_000,
            target_base_inventory: 1_000_000,
            initial_mid_price: Q64x64::from_int(30_000),
            quote_update_policy: QuoteUpdatePolicy::instant(2),
            params: reference_params(),
        };
        let slots = [
            MarketSlot {
                slot: 3,
                fair_price: Q64x64::from_int(30_000),
                flow: None,
            },
            MarketSlot {
                slot: 2,
                fair_price: Q64x64::from_int(30_000),
                flow: None,
            },
        ];

        assert_eq!(
            simulate_reference_quote_market_path(scenario, &slots),
            Err(MathError::InvalidConfig)
        );
    }

    #[test]
    fn replay_csv_parses_distributions_and_exact_update_outcomes() {
        let replay =
            parse_replay_csv(include_str!("../../../tests/fixtures/reference-replay.csv")).unwrap();
        assert_eq!(replay.slots.len(), 10);
        assert_eq!(replay.quote_updates.len(), 3);

        let flow = replay.flow_distribution().unwrap();
        assert_eq!(flow.observations, 10);
        assert_eq!(flow.trades, 6);
        assert_eq!(flow.trade_probability_bps, 6_000);
        assert_eq!(flow.base_to_quote_trades, 5);
        assert_eq!(flow.quote_to_base_trades, 1);
        assert!(flow.base_equivalent_amount.mean > 0);

        let landing = replay.landing_distribution();
        assert_eq!(landing.quote_updates_sent, 3);
        assert_eq!(landing.quote_updates_landed, 2);
        assert_eq!(landing.quote_updates_dropped, 1);
        assert_eq!(landing.success_probability_bps, 6_666);
        assert_eq!(landing.latency_slots.min, 2);
        assert_eq!(landing.latency_slots.max, 2);

        let policy =
            quote_update_policy_from_replay(&replay, SameSlotUpdateOrder::SwapBeforeUpdate, 11)
                .unwrap();
        assert_eq!(policy.maker_update_period_slots, 3);
        assert_eq!(policy.landing_latency_slots, 2);
        assert_eq!(policy.update_success_probability_bps, 6_666);

        let report = simulate_reference_quote_replay(
            ReferenceQuoteScenario {
                assumptions: assumptions(1),
                initial_base_inventory: 1_000_000,
                initial_quote_inventory: 30_000_000_000,
                target_base_inventory: 1_000_000,
                initial_mid_price: replay.first_fair_price().unwrap(),
                quote_update_policy: policy,
                params: reference_params(),
            },
            &replay,
        )
        .unwrap();

        assert_eq!(report.trades_attempted, flow.trades);
        assert_eq!(report.quote_updates_sent, landing.quote_updates_sent);
        assert_eq!(report.quote_updates_landed, landing.quote_updates_landed);
        assert_eq!(report.quote_updates_dropped, landing.quote_updates_dropped);
        assert!(report.trades_filled > 0);
    }

    #[test]
    fn replay_rejects_unsorted_quote_updates() {
        let replay = ReplayMarketPath {
            slots: vec![MarketSlot {
                slot: 4,
                fair_price: Q64x64::from_int(30_000),
                flow: None,
            }],
            quote_updates: vec![
                QuoteUpdateEvent {
                    publish_slot: 3,
                    landing_slot: Some(4),
                    mid_price: Q64x64::from_int(30_000),
                },
                QuoteUpdateEvent {
                    publish_slot: 2,
                    landing_slot: Some(4),
                    mid_price: Q64x64::from_int(30_000),
                },
            ],
        };

        assert_eq!(
            simulate_reference_quote_replay(
                ReferenceQuoteScenario {
                    assumptions: assumptions(1),
                    initial_base_inventory: 1_000_000,
                    initial_quote_inventory: 30_000_000_000,
                    target_base_inventory: 1_000_000,
                    initial_mid_price: Q64x64::from_int(30_000),
                    quote_update_policy: QuoteUpdatePolicy::instant(1),
                    params: reference_params(),
                },
                &replay,
            ),
            Err(MathError::InvalidConfig)
        );
    }

    fn reference_params() -> ReferenceQuoteParams {
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
}
