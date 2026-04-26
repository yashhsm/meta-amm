use meta_amm_math::ReferenceQuoteParams;
use meta_amm_math::{CpmmReserves, Q64x64};
use meta_amm_sim::{
    simulate_generated_cpmm, simulate_generated_reference_quote, AggregateReport,
    GeneratedCpmmScenario, GeneratedReferenceQuoteScenario, ReferenceQuoteAggregateReport,
    ScenarioAssumptions,
};

fn main() {
    let assumptions = ScenarioAssumptions {
        name: "generated-smoke",
        flow_model: "seeded random walk fair price with Bernoulli flow",
        landing_model: "instant deterministic landing",
        path_count: 32,
    };
    let seed = 0x6d657461_616d6d5f_736d6f6b_65000001;
    let slots_per_path = 1_000;
    let initial_fair_price = Q64x64::from_int(30_000);
    let volatility_bps_per_slot = 5;
    let drift_bps_per_slot = 0;
    let trade_probability_bps = 1_500;
    let max_trade_base_atoms = 1_000;

    let cpmm = GeneratedCpmmScenario {
        assumptions,
        seed,
        slots_per_path,
        initial_reserves: CpmmReserves {
            base: 1_000_000,
            quote: 30_000_000_000,
        },
        initial_fair_price,
        fee_bps: 30,
        volatility_bps_per_slot,
        drift_bps_per_slot,
        trade_probability_bps,
        max_trade_base_atoms,
    };
    let reference = GeneratedReferenceQuoteScenario {
        assumptions,
        seed,
        slots_per_path,
        initial_base_inventory: 1_000_000,
        initial_quote_inventory: 30_000_000_000,
        target_base_inventory: 1_000_000,
        initial_fair_price,
        maker_update_period_slots: 6,
        params: ReferenceQuoteParams {
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
        volatility_bps_per_slot,
        drift_bps_per_slot,
        trade_probability_bps,
        max_trade_base_atoms,
    };

    let cpmm_report = simulate_generated_cpmm(cpmm).expect("CPMM smoke scenario should simulate");
    let reference_report = simulate_generated_reference_quote(reference)
        .expect("ReferenceQuote smoke should simulate");

    println!("scenario: {}", assumptions.name);
    println!("flow_model: {}", assumptions.flow_model);
    println!("landing_model: {}", assumptions.landing_model);
    println!("paths: {}", assumptions.path_count);
    println!();
    print_cpmm_report(&cpmm_report);
    println!();
    print_reference_report(&reference_report);
    println!();
    println!("warning: generated smoke output is not market replay or maker edge");
}

fn print_cpmm_report(report: &AggregateReport) {
    println!("engine: CPMM");
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
}

fn print_reference_report(report: &ReferenceQuoteAggregateReport) {
    println!("engine: ReferenceQuote");
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
        "rejected_stale: min={} mean={} max={}",
        report.rejected_stale.min, report.rejected_stale.mean, report.rejected_stale.max
    );
    println!(
        "rejected_protected: min={} mean={} max={}",
        report.rejected_protected.min,
        report.rejected_protected.mean,
        report.rejected_protected.max
    );
    println!(
        "rejected_inventory: min={} mean={} max={}",
        report.rejected_inventory.min,
        report.rejected_inventory.mean,
        report.rejected_inventory.max
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
    println!(
        "max_abs_inventory_imbalance_bps: min={} mean={} max={}",
        report.max_abs_inventory_imbalance_bps.min,
        report.max_abs_inventory_imbalance_bps.mean,
        report.max_abs_inventory_imbalance_bps.max
    );
}
