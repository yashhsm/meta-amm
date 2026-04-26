use std::{env, fs};

use meta_amm_math::ReferenceQuoteParams;
use meta_amm_math::{CpmmReserves, Q64x64};
use meta_amm_sim::{
    calibrate_reference_quote, default_reference_quote_scenario_pack, evaluate_scenario_pack,
    parse_replay_csv, quote_update_policy_from_replay, simulate_generated_cpmm,
    simulate_generated_reference_quote, simulate_reference_quote_replay,
    simulate_reference_quote_scenario_pack, AggregateReport, CalibrationCandidateReport,
    FlowDistributionReport, GateFinding, GeneratedCpmmScenario, GeneratedReferenceQuoteScenario,
    LandingDistributionReport, QuoteUpdatePolicy, ReferenceQuoteAggregateReport,
    ReferenceQuoteCalibrationReport, ReferenceQuoteReport, ReferenceQuoteScenario,
    SameSlotUpdateOrder, ScenarioAssumptions, ScenarioGateThresholds, ScenarioPackEvaluation,
    ScenarioPackReport, SummaryI128, SummaryU128, SummaryU16, SummaryU64,
};

fn main() {
    if calibrate_reference_requested() || export_best_config_requested() {
        run_reference_calibration();
        return;
    }
    if scenario_pack_requested() {
        run_scenario_pack();
        return;
    }
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

fn run_scenario_pack() {
    let scenarios = default_reference_quote_scenario_pack(0x6d657461_616d6d5f_7061636b);
    let report =
        simulate_reference_quote_scenario_pack(&scenarios).expect("scenario pack should simulate");
    let evaluation =
        evaluate_scenario_pack(&report, ScenarioGateThresholds::reference_quote_default());

    println!("scenario_pack: default-reference-quote");
    println!("scenarios: {}", report.len());
    println!("evaluation: {}", evaluation.severity().as_str());
    println!();
    print_scenario_pack_report(&report, &evaluation);
    println!("warning: scenario-pack output is generated, not market replay");
}

fn run_reference_calibration() {
    let report = calibrate_reference_quote(
        0x006d_6574_6161_6d6d_5f63_616c,
        ScenarioGateThresholds::reference_quote_default(),
    )
    .expect("reference calibration should simulate");

    println!("calibration: reference-quote-generated");
    println!("candidates: {}", report.candidates.len());
    if let Some(best) = report.best() {
        println!("best_candidate: {}", best.candidate_name);
        println!("best_evaluation: {}", best.evaluation.severity().as_str());
    }
    println!();
    print_calibration_report(&report);
    println!("warning: calibration output is generated search plumbing, not market proof");
    if export_best_config_requested() {
        if let Some(config) = report.export_best_config() {
            println!();
            println!("config_export:");
            print!("{config}");
        }
    }
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

fn scenario_pack_requested() -> bool {
    env::args().skip(1).any(|arg| arg == "--scenario-pack")
}

fn calibrate_reference_requested() -> bool {
    env::args()
        .skip(1)
        .any(|arg| arg == "--calibrate-reference")
}

fn export_best_config_requested() -> bool {
    env::args().skip(1).any(|arg| arg == "--export-best-config")
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
    print_summary_u64("trades_attempted", &report.trades_attempted);
    print_summary_u64("trades_filled", &report.trades_filled);
    print_summary_u16("fill_rate_bps", &report.fill_rate_bps);
    print_summary_u128("fees_quote_atoms", &report.fees_quote_atoms);
    print_summary_i128("taker_edge_quote_atoms", &report.taker_edge_quote_atoms);
}

fn print_reference_report(report: &ReferenceQuoteAggregateReport) {
    println!("engine: ReferenceQuote");
    print_summary_u64("trades_attempted", &report.trades_attempted);
    print_summary_u64("trades_filled", &report.trades_filled);
    print_summary_u16("fill_rate_bps", &report.fill_rate_bps);
    print_summary_u64("rejected_stale", &report.rejected_stale);
    print_summary_u64("rejected_protected", &report.rejected_protected);
    print_summary_u64("rejected_inventory", &report.rejected_inventory);
    print_summary_u64("quote_updates_sent", &report.quote_updates_sent);
    print_summary_u64("quote_updates_landed", &report.quote_updates_landed);
    print_summary_u64("quote_updates_dropped", &report.quote_updates_dropped);
    print_summary_u64("max_quote_age_slots", &report.max_quote_age_slots);
    print_summary_u128("fees_quote_atoms", &report.fees_quote_atoms);
    print_summary_i128("taker_edge_quote_atoms", &report.taker_edge_quote_atoms);
    print_summary_u16(
        "max_abs_inventory_imbalance_bps",
        &report.max_abs_inventory_imbalance_bps,
    );
}

fn print_scenario_pack_report(report: &ScenarioPackReport, evaluation: &ScenarioPackEvaluation) {
    for (scenario, evaluation) in report.reports.iter().zip(evaluation.evaluations.iter()) {
        println!("scenario: {}", scenario.assumptions.name);
        println!("flow_model: {}", scenario.assumptions.flow_model);
        println!("landing_model: {}", scenario.assumptions.landing_model);
        println!("paths: {}", scenario.paths);
        println!("evaluation: {}", evaluation.severity.as_str());
        if evaluation.findings.is_empty() {
            println!("gate_findings: none");
        } else {
            for finding in &evaluation.findings {
                print_gate_finding(finding);
            }
        }
        print_reference_report(scenario);
        println!();
    }
}

fn print_calibration_report(report: &ReferenceQuoteCalibrationReport) {
    for candidate in &report.candidates {
        print_calibration_candidate(candidate);
    }
}

fn print_calibration_candidate(candidate: &CalibrationCandidateReport) {
    println!("candidate: {}", candidate.candidate_name);
    println!("evaluation: {}", candidate.evaluation.severity().as_str());
    println!(
        "score: mean_p05_fill_rate_bps={} mean_fill_rate_bps={} mean_update_drop_rate_bps={} rough_maker_score_quote_atoms={}",
        candidate.score.mean_p05_fill_rate_bps,
        candidate.score.mean_fill_rate_bps,
        candidate.score.mean_update_drop_rate_bps,
        candidate.score.rough_maker_score_quote_atoms
    );
    for evaluation in &candidate.evaluation.evaluations {
        if evaluation.findings.is_empty() {
            println!(
                "scenario_gate: scenario={} severity={} findings=none",
                evaluation.scenario_name,
                evaluation.severity.as_str()
            );
        } else {
            for finding in &evaluation.findings {
                println!(
                    "scenario_gate: scenario={} severity={} metric={} observed={} threshold={}",
                    evaluation.scenario_name,
                    finding.severity.as_str(),
                    finding.metric,
                    finding.observed,
                    finding.threshold
                );
            }
        }
    }
    println!();
}

fn print_gate_finding(finding: &GateFinding) {
    println!(
        "gate_finding: severity={} metric={} observed={} threshold={}",
        finding.severity.as_str(),
        finding.metric,
        finding.observed,
        finding.threshold
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
    print_summary_u64("base_equivalent_amount", &report.base_equivalent_amount);
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
    print_summary_u64("latency_slots", &report.latency_slots);
}

fn print_summary_u16(label: &str, summary: &SummaryU16) {
    println!(
        "{}: min={} p05={} mean={} p50={} p95={} max={}",
        label, summary.min, summary.p05, summary.mean, summary.p50, summary.p95, summary.max
    );
}

fn print_summary_u64(label: &str, summary: &SummaryU64) {
    println!(
        "{}: min={} p05={} mean={} p50={} p95={} max={}",
        label, summary.min, summary.p05, summary.mean, summary.p50, summary.p95, summary.max
    );
}

fn print_summary_u128(label: &str, summary: &SummaryU128) {
    println!(
        "{}: min={} p05={} mean={} p50={} p95={} max={}",
        label, summary.min, summary.p05, summary.mean, summary.p50, summary.p95, summary.max
    );
}

fn print_summary_i128(label: &str, summary: &SummaryI128) {
    println!(
        "{}: min={} p05={} mean={} p50={} p95={} max={}",
        label, summary.min, summary.p05, summary.mean, summary.p50, summary.p95, summary.max
    );
}
