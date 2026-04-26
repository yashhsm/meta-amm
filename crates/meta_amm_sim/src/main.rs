use meta_amm_math::{CpmmReserves, Q64x64};
use meta_amm_sim::{simulate_generated_cpmm, GeneratedCpmmScenario, ScenarioAssumptions};

fn main() {
    let scenario = GeneratedCpmmScenario {
        assumptions: ScenarioAssumptions {
            name: "cpmm-generated-smoke",
            flow_model: "seeded random walk fair price with Bernoulli flow",
            landing_model: "instant deterministic landing",
            path_count: 32,
        },
        seed: 0x6d657461_616d6d5f_736d6f6b_65000001,
        slots_per_path: 1_000,
        initial_reserves: CpmmReserves {
            base: 1_000_000,
            quote: 30_000_000_000,
        },
        initial_fair_price: Q64x64::from_int(30_000),
        fee_bps: 30,
        volatility_bps_per_slot: 5,
        drift_bps_per_slot: 0,
        trade_probability_bps: 1_500,
        max_trade_base_atoms: 1_000,
    };

    let report = simulate_generated_cpmm(scenario).expect("smoke scenario should simulate");

    println!("scenario: {}", report.assumptions.name);
    println!("flow_model: {}", report.assumptions.flow_model);
    println!("landing_model: {}", report.assumptions.landing_model);
    println!("paths: {}", report.paths);
    println!(
        "trades_attempted: min={} mean={} max={}",
        report.trades_attempted.min, report.trades_attempted.mean, report.trades_attempted.max
    );
    println!(
        "trades_filled: min={} mean={} max={}",
        report.trades_filled.min, report.trades_filled.mean, report.trades_filled.max
    );
    println!(
        "fill_rate_bps: min={} mean={} max={}",
        report.fill_rate_bps.min, report.fill_rate_bps.mean, report.fill_rate_bps.max
    );
    println!(
        "fees_quote_atoms: min={} mean={} max={}",
        report.fees_quote_atoms.min, report.fees_quote_atoms.mean, report.fees_quote_atoms.max
    );
    println!(
        "taker_edge_quote_atoms: min={} mean={} max={}",
        report.taker_edge_quote_atoms.min,
        report.taker_edge_quote_atoms.mean,
        report.taker_edge_quote_atoms.max
    );
    if report.is_single_path() {
        println!("warning: single-path output is not expected maker edge");
    } else {
        println!("warning: generated smoke output is not market replay or maker edge");
    }
}
