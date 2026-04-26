use std::{env, fs};

use meta_amm_math::ReferenceQuoteParams;
use meta_amm_math::{CpmmReserves, Q64x64};
use meta_amm_sim::{
    parse_replay_csv, quote_update_policy_from_replay, simulate_generated_cpmm,
    simulate_generated_reference_quote, simulate_reference_quote_replay, AggregateReport,
    FlowDistributionReport, GeneratedCpmmScenario, GeneratedReferenceQuoteScenario,
    LandingDistributionReport, QuoteUpdatePolicy, ReferenceQuoteAggregateReport,
    ReferenceQuoteReport, ReferenceQuoteScenario, SameSlotUpdateOrder, ScenarioAssumptions,
};

fn main() {
    if let Some(path) = replay_csv_path_arg() {
        run_replay_csv(&path);
        return;
    }
    run_generated_smoke();
}

fn run_generated_smoke() {
    let assumptions = ScenarioAssumptions {
        name: "generated-smoke",
        flow_model: "seeded random walk fair price with Bernoulli flow",
        landing_model: "deterministic quote latency with seeded update drops",
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
        quote_update_policy: QuoteUpdatePolicy {
            maker_update_period_slots: 8,
            landing_latency_slots: 5,
            update_success_probability_bps: 7_500,
            failure_seed: seed ^ 0x71756f74655f757064617465,
            same_slot_order: SameSlotUpdateOrder::SwapBeforeUpdate,
        },
        params: reference_params(),
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

fn run_replay_csv(path: &str) {
    let input = fs::read_to_string(path).expect("replay CSV should be readable");
    let replay = parse_replay_csv(&input).expect("replay CSV should parse");
    let flow = replay
        .flow_distribution()
        .expect("replay flow distribution should calculate");
    let landing = replay.landing_distribution();
    let policy = quote_update_policy_from_replay(
        &replay,
        SameSlotUpdateOrder::SwapBeforeUpdate,
        0x7265706c61795f71756f7465,
    )
    .expect("replay should contain at least two quote updates to infer period");
    let assumptions = ScenarioAssumptions {
        name: "replay-csv",
        flow_model: "CSV replay slot/flow path",
        landing_model: "CSV quote update landing outcomes",
        path_count: 1,
    };
    let scenario = ReferenceQuoteScenario {
        assumptions,
        initial_base_inventory: 1_000_000,
        initial_quote_inventory: 30_000_000_000,
        target_base_inventory: 1_000_000,
        initial_mid_price: replay
            .first_fair_price()
            .expect("replay CSV should include at least one market row"),
        quote_update_policy: policy,
        params: reference_params(),
    };
    let report =
        simulate_reference_quote_replay(scenario, &replay).expect("replay scenario should run");

    println!("scenario: {}", assumptions.name);
    println!("flow_model: {}", assumptions.flow_model);
    println!("landing_model: {}", assumptions.landing_model);
    println!("paths: {}", assumptions.path_count);
    println!("source: {}", path);
    println!();
    print_flow_distribution(&flow);
    println!();
    print_landing_distribution(&landing);
    println!();
    print_reference_single_report(&report);
    println!();
    println!("warning: replay output is only as good as the supplied data schema");
}

fn replay_csv_path_arg() -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--replay-csv" {
            return Some(
                args.next()
                    .expect("--replay-csv requires a file path argument"),
            );
        }
    }
    None
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
        protected_max_trade_base_atoms: 500,
        inventory_skew_bps_per_10k_imbalance: 1_000,
        max_inventory_skew_bps: 500,
        hard_inventory_band_bps: 3_000,
    }
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
        "quote_updates_sent: min={} mean={} max={}",
        report.quote_updates_sent.min,
        report.quote_updates_sent.mean,
        report.quote_updates_sent.max
    );
    println!(
        "quote_updates_landed: min={} mean={} max={}",
        report.quote_updates_landed.min,
        report.quote_updates_landed.mean,
        report.quote_updates_landed.max
    );
    println!(
        "quote_updates_dropped: min={} mean={} max={}",
        report.quote_updates_dropped.min,
        report.quote_updates_dropped.mean,
        report.quote_updates_dropped.max
    );
    println!(
        "max_quote_age_slots: min={} mean={} max={}",
        report.max_quote_age_slots.min,
        report.max_quote_age_slots.mean,
        report.max_quote_age_slots.max
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

fn print_reference_single_report(report: &ReferenceQuoteReport) {
    println!("engine: ReferenceQuote");
    println!("trades_attempted: {}", report.trades_attempted);
    println!("trades_filled: {}", report.trades_filled);
    println!("fill_rate_bps: {}", report.fill_rate_bps());
    println!("rejected_stale: {}", report.rejected_stale);
    println!("rejected_protected: {}", report.rejected_protected);
    println!("rejected_inventory: {}", report.rejected_inventory);
    println!("quote_updates_sent: {}", report.quote_updates_sent);
    println!("quote_updates_landed: {}", report.quote_updates_landed);
    println!("quote_updates_dropped: {}", report.quote_updates_dropped);
    println!("max_quote_age_slots: {}", report.max_quote_age_slots);
    println!("fees_quote_atoms: {}", report.fees_quote_atoms);
    println!("taker_edge_quote_atoms: {}", report.taker_edge_quote_atoms);
    println!(
        "max_abs_inventory_imbalance_bps: {}",
        report.max_abs_inventory_imbalance_bps
    );
}

fn print_flow_distribution(report: &FlowDistributionReport) {
    println!("replay_flow_distribution");
    println!("observations: {}", report.observations);
    println!("trades: {}", report.trades);
    println!("trade_probability_bps: {}", report.trade_probability_bps);
    println!("base_to_quote_trades: {}", report.base_to_quote_trades);
    println!("quote_to_base_trades: {}", report.quote_to_base_trades);
    println!(
        "base_equivalent_amount: min={} mean={} max={}",
        report.base_equivalent_amount.min,
        report.base_equivalent_amount.mean,
        report.base_equivalent_amount.max
    );
}

fn print_landing_distribution(report: &LandingDistributionReport) {
    println!("replay_landing_distribution");
    println!("quote_updates_sent: {}", report.quote_updates_sent);
    println!("quote_updates_landed: {}", report.quote_updates_landed);
    println!("quote_updates_dropped: {}", report.quote_updates_dropped);
    println!(
        "success_probability_bps: {}",
        report.success_probability_bps
    );
    println!(
        "latency_slots: min={} mean={} max={}",
        report.latency_slots.min, report.latency_slots.mean, report.latency_slots.max
    );
}
