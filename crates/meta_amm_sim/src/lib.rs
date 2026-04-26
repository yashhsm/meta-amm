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
}
