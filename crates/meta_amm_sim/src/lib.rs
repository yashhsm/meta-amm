use meta_amm_math::{
    cpmm_quote_exact_in, reference_quote_exact_in, CpmmReserves, MathError, Q64x64, QuoteAgeState,
    ReferenceQuoteParams, ReferenceQuoteState,
};

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
pub enum SameSlotUpdateOrder {
    UpdateBeforeSwap,
    SwapBeforeUpdate,
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
    pub mean: u16,
    pub max: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryU64 {
    pub min: u64,
    pub mean: u64,
    pub max: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryU128 {
    pub min: u128,
    pub mean: u128,
    pub max: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryI128 {
    pub min: i128,
    pub mean: i128,
    pub max: i128,
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

        if let Some(flow) = slot.flow {
            report.trades_attempted += 1;
            let event = FlowEvent {
                slot: slot.slot,
                side: flow.side,
                amount_in: flow.amount_in,
                fair_price: slot.fair_price,
            };
            let base_to_quote = matches!(flow.side, Side::BaseToQuote);
            match reference_quote_exact_in(state, scenario.params, flow.amount_in, base_to_quote) {
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
                    report.max_abs_inventory_imbalance_bps = report
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
        }

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
    let mut min = u16::MAX;
    let mut max = 0u16;
    let mut sum = 0u128;
    let mut count = 0u128;

    for value in values {
        min = min.min(value);
        max = max.max(value);
        sum += value as u128;
        count += 1;
    }

    SummaryU16 {
        min: if count == 0 { 0 } else { min },
        mean: if count == 0 { 0 } else { (sum / count) as u16 },
        max,
    }
}

fn summarize_u64(values: impl Iterator<Item = u64>) -> SummaryU64 {
    let mut min = u64::MAX;
    let mut max = 0u64;
    let mut sum = 0u128;
    let mut count = 0u128;

    for value in values {
        min = min.min(value);
        max = max.max(value);
        sum += value as u128;
        count += 1;
    }

    SummaryU64 {
        min: if count == 0 { 0 } else { min },
        mean: if count == 0 { 0 } else { (sum / count) as u64 },
        max,
    }
}

fn summarize_u128(values: impl Iterator<Item = u128>) -> SummaryU128 {
    let mut min = u128::MAX;
    let mut max = 0u128;
    let mut sum = 0u128;
    let mut count = 0u128;

    for value in values {
        min = min.min(value);
        max = max.max(value);
        sum = sum.saturating_add(value);
        count += 1;
    }

    SummaryU128 {
        min: if count == 0 { 0 } else { min },
        mean: if count == 0 { 0 } else { sum / count },
        max,
    }
}

fn summarize_i128(values: impl Iterator<Item = i128>, count_hint: u128) -> SummaryI128 {
    let mut min = i128::MAX;
    let mut max = i128::MIN;
    let mut sum = 0i128;
    let mut count = 0u128;

    for value in values {
        min = min.min(value);
        max = max.max(value);
        sum = sum.saturating_add(value);
        count += 1;
    }

    if count == 0 && count_hint == 0 {
        return SummaryI128 {
            min: 0,
            mean: 0,
            max: 0,
        };
    }
    let count = count.max(count_hint).max(1);
    SummaryI128 {
        min,
        mean: sum / (count as i128),
        max: if max == i128::MIN { 0 } else { max },
    }
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
