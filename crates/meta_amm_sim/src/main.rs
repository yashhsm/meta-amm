use meta_amm_math::{CpmmReserves, Q64x64};
use meta_amm_sim::{simulate_cpmm, CpmmScenario, FlowEvent, ScenarioAssumptions, Side};

fn main() {
    let scenario = CpmmScenario {
        assumptions: ScenarioAssumptions {
            name: "cpmm-smoke",
            flow_model: "deterministic fixture",
            landing_model: "instant deterministic landing",
            path_count: 1,
        },
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

    let report = simulate_cpmm(scenario, &events).expect("smoke scenario should simulate");

    println!("scenario: {}", report.assumptions.name);
    println!("flow_model: {}", report.assumptions.flow_model);
    println!("landing_model: {}", report.assumptions.landing_model);
    println!("path_count: {}", report.assumptions.path_count);
    println!("trades_attempted: {}", report.trades_attempted);
    println!("trades_filled: {}", report.trades_filled);
    println!("fill_rate_bps: {}", report.fill_rate_bps());
    println!("fees_quote_atoms: {}", report.fees_quote_atoms);
    println!("taker_edge_quote_atoms: {}", report.taker_edge_quote_atoms);
    println!(
        "final_reserves: base={} quote={}",
        report.final_reserves.base, report.final_reserves.quote
    );
    if report.is_single_path() {
        println!("warning: single-path smoke output is not expected maker edge");
    }
}
