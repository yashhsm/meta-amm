use meta_amm_math::{cpmm_quote_exact_in, CpmmReserves, MathError, Q64x64};

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
pub struct AggregateReport {
    pub assumptions: ScenarioAssumptions,
    pub paths: u32,
    pub trades_attempted: SummaryU64,
    pub trades_filled: SummaryU64,
    pub fill_rate_bps: SummaryU16,
    pub fees_quote_atoms: SummaryU128,
    pub taker_edge_quote_atoms: SummaryI128,
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

    let count = count.max(count_hint).max(1);
    SummaryI128 {
        min: if count == 0 { 0 } else { min },
        mean: sum / (count as i128),
        max: if max == i128::MIN { 0 } else { max },
    }
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
}
