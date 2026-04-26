use std::fmt::Write;

use meta_amm_config::{QuoteUpdateEnvelope, ReferenceQuoteStrategyConfig, SameSlotUpdateOrder};
use meta_amm_math::{MathError, ReferenceQuoteParams};

use crate::{
    default_reference_quote_scenario_pack, evaluate_scenario_pack,
    simulate_reference_quote_scenario_pack, GeneratedReferenceQuoteScenario, QuoteUpdatePolicy,
    ScenarioGateThresholds, ScenarioPackEvaluation, ScenarioPackReport,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceQuoteCalibrationCandidate {
    pub name: &'static str,
    pub scenarios: [GeneratedReferenceQuoteScenario; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalibrationScore {
    pub mean_p05_fill_rate_bps: u16,
    pub mean_fill_rate_bps: u16,
    pub mean_update_drop_rate_bps: u16,
    pub rough_maker_score_quote_atoms: i128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationCandidateReport {
    pub candidate_name: &'static str,
    pub strategy_config: ReferenceQuoteStrategyConfig,
    pub score: CalibrationScore,
    pub scenario_pack: ScenarioPackReport,
    pub evaluation: ScenarioPackEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceQuoteCalibrationReport {
    pub candidates: Vec<CalibrationCandidateReport>,
}

impl ReferenceQuoteCalibrationReport {
    pub fn best(&self) -> Option<&CalibrationCandidateReport> {
        self.candidates.first()
    }

    pub fn export_best_config(&self) -> Option<String> {
        self.best().map(export_reference_quote_config)
    }
}

pub fn calibrate_reference_quote(
    seed: u128,
    thresholds: ScenarioGateThresholds,
) -> Result<ReferenceQuoteCalibrationReport, MathError> {
    let mut reports = Vec::new();
    for candidate in default_reference_quote_calibration_candidates(seed) {
        let scenario_pack = simulate_reference_quote_scenario_pack(&candidate.scenarios)?;
        let evaluation = evaluate_scenario_pack(&scenario_pack, thresholds);
        let score = score_candidate(&scenario_pack);
        let strategy_config = strategy_config_from_scenarios(&candidate.scenarios)?;
        reports.push(CalibrationCandidateReport {
            candidate_name: candidate.name,
            strategy_config,
            score,
            scenario_pack,
            evaluation,
        });
    }

    reports.sort_by(compare_candidates);
    Ok(ReferenceQuoteCalibrationReport {
        candidates: reports,
    })
}

pub fn export_reference_quote_config(candidate: &CalibrationCandidateReport) -> String {
    let mut out = String::new();
    let config = candidate.strategy_config;
    writeln!(&mut out, "{{").unwrap();
    writeln!(&mut out, "  \"version\": 1,").unwrap();
    writeln!(&mut out, "  \"mode\": \"ReferenceQuote\",").unwrap();
    write!(&mut out, "  \"candidate_name\": ").unwrap();
    write_json_string(&mut out, candidate.candidate_name);
    writeln!(&mut out, ",").unwrap();
    writeln!(
        &mut out,
        "  \"warning\": \"generated calibration output; not market replay or maker edge\","
    )
    .unwrap();
    writeln!(&mut out, "  \"params\": {{").unwrap();
    writeln!(&mut out, "    \"fee_bps\": {},", config.params.fee_bps).unwrap();
    writeln!(
        &mut out,
        "    \"base_half_spread_bps\": {},",
        config.params.base_half_spread_bps
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"aging_start_slots\": {},",
        config.params.aging_start_slots
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"protected_start_slots\": {},",
        config.params.protected_start_slots
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"expire_slots\": {},",
        config.params.expire_slots
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"aging_surcharge_bps_per_slot\": {},",
        config.params.aging_surcharge_bps_per_slot
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"max_aging_surcharge_bps\": {},",
        config.params.max_aging_surcharge_bps
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"max_trade_base_atoms\": {},",
        config.params.max_trade_base_atoms
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"protected_max_trade_base_atoms\": {},",
        config.params.protected_max_trade_base_atoms
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"inventory_skew_bps_per_10k_imbalance\": {},",
        config.params.inventory_skew_bps_per_10k_imbalance
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"max_inventory_skew_bps\": {},",
        config.params.max_inventory_skew_bps
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"hard_inventory_band_bps\": {}",
        config.params.hard_inventory_band_bps
    )
    .unwrap();
    writeln!(&mut out, "  }},").unwrap();
    writeln!(&mut out, "  \"quote_update_envelope\": {{").unwrap();
    writeln!(
        &mut out,
        "    \"max_maker_update_period_slots\": {},",
        config.quote_update_envelope.max_maker_update_period_slots
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"max_landing_latency_slots\": {},",
        config.quote_update_envelope.max_landing_latency_slots
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"min_update_success_probability_bps\": {},",
        config
            .quote_update_envelope
            .min_update_success_probability_bps
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"same_slot_order\": \"{}\"",
        same_slot_order_str(config.quote_update_envelope.same_slot_order)
    )
    .unwrap();
    writeln!(&mut out, "  }},").unwrap();
    writeln!(&mut out, "  \"score\": {{").unwrap();
    writeln!(
        &mut out,
        "    \"mean_p05_fill_rate_bps\": {},",
        candidate.score.mean_p05_fill_rate_bps
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"mean_fill_rate_bps\": {},",
        candidate.score.mean_fill_rate_bps
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"mean_update_drop_rate_bps\": {},",
        candidate.score.mean_update_drop_rate_bps
    )
    .unwrap();
    writeln!(
        &mut out,
        "    \"rough_maker_score_quote_atoms\": {}",
        candidate.score.rough_maker_score_quote_atoms
    )
    .unwrap();
    writeln!(&mut out, "  }},").unwrap();
    writeln!(&mut out, "  \"evaluation\": {{").unwrap();
    writeln!(
        &mut out,
        "    \"severity\": \"{}\",",
        candidate.evaluation.severity().as_str()
    )
    .unwrap();
    writeln!(&mut out, "    \"scenario_findings\": [").unwrap();
    let mut first_finding = true;
    for evaluation in &candidate.evaluation.evaluations {
        if evaluation.findings.is_empty() {
            if !first_finding {
                writeln!(&mut out, ",").unwrap();
            }
            write!(&mut out, "      {{\"scenario\": ").unwrap();
            write_json_string(&mut out, evaluation.scenario_name);
            write!(&mut out, ", \"severity\": ").unwrap();
            write_json_string(&mut out, evaluation.severity.as_str());
            write!(
                &mut out,
                ", \"metric\": \"none\", \"observed\": 0, \"threshold\": 0}}"
            )
            .unwrap();
            first_finding = false;
            continue;
        }
        for finding in &evaluation.findings {
            if !first_finding {
                writeln!(&mut out, ",").unwrap();
            }
            write!(&mut out, "      {{\"scenario\": ").unwrap();
            write_json_string(&mut out, evaluation.scenario_name);
            write!(&mut out, ", \"severity\": ").unwrap();
            write_json_string(&mut out, finding.severity.as_str());
            write!(&mut out, ", \"metric\": ").unwrap();
            write_json_string(&mut out, finding.metric);
            write!(
                &mut out,
                ", \"observed\": {}, \"threshold\": {}}}",
                finding.observed, finding.threshold
            )
            .unwrap();
            first_finding = false;
        }
    }
    if !first_finding {
        writeln!(&mut out).unwrap();
    }
    writeln!(&mut out, "    ]").unwrap();
    writeln!(&mut out, "  }},").unwrap();
    writeln!(&mut out, "  \"scenario_assumptions\": [").unwrap();
    for (index, report) in candidate.scenario_pack.reports.iter().enumerate() {
        let comma = if index + 1 == candidate.scenario_pack.reports.len() {
            ""
        } else {
            ","
        };
        write!(&mut out, "    {{\"name\": ").unwrap();
        write_json_string(&mut out, report.assumptions.name);
        write!(&mut out, ", \"flow_model\": ").unwrap();
        write_json_string(&mut out, report.assumptions.flow_model);
        write!(&mut out, ", \"landing_model\": ").unwrap();
        write_json_string(&mut out, report.assumptions.landing_model);
        writeln!(&mut out, ", \"paths\": {}}}{}", report.paths, comma).unwrap();
    }
    writeln!(&mut out, "  ]").unwrap();
    writeln!(&mut out, "}}").unwrap();
    out
}

pub fn default_reference_quote_calibration_candidates(
    seed: u128,
) -> Vec<ReferenceQuoteCalibrationCandidate> {
    let baseline = default_reference_quote_scenario_pack(seed);
    vec![
        ReferenceQuoteCalibrationCandidate {
            name: "baseline",
            scenarios: baseline,
        },
        ReferenceQuoteCalibrationCandidate {
            name: "wider-spread",
            scenarios: map_params(baseline, |params| ReferenceQuoteParams {
                base_half_spread_bps: 20,
                aging_surcharge_bps_per_slot: 4,
                max_aging_surcharge_bps: 40,
                protected_max_trade_base_atoms: 400,
                ..params
            }),
        },
        ReferenceQuoteCalibrationCandidate {
            name: "faster-updates",
            scenarios: map_policy(baseline, faster_policy),
        },
        ReferenceQuoteCalibrationCandidate {
            name: "wide-and-fast",
            scenarios: map_policy(
                map_params(baseline, |params| ReferenceQuoteParams {
                    base_half_spread_bps: 18,
                    aging_surcharge_bps_per_slot: 4,
                    max_aging_surcharge_bps: 40,
                    protected_max_trade_base_atoms: 400,
                    ..params
                }),
                faster_policy,
            ),
        },
        ReferenceQuoteCalibrationCandidate {
            name: "tight-freshness",
            scenarios: map_policy(
                map_params(baseline, |params| ReferenceQuoteParams {
                    aging_start_slots: 3,
                    protected_start_slots: 7,
                    expire_slots: 11,
                    aging_surcharge_bps_per_slot: 5,
                    max_aging_surcharge_bps: 50,
                    protected_max_trade_base_atoms: 300,
                    ..params
                }),
                faster_policy,
            ),
        },
    ]
}

fn compare_candidates(
    left: &CalibrationCandidateReport,
    right: &CalibrationCandidateReport,
) -> std::cmp::Ordering {
    left.evaluation
        .severity()
        .cmp(&right.evaluation.severity())
        .then_with(|| {
            right
                .score
                .mean_p05_fill_rate_bps
                .cmp(&left.score.mean_p05_fill_rate_bps)
        })
        .then_with(|| {
            right
                .score
                .rough_maker_score_quote_atoms
                .cmp(&left.score.rough_maker_score_quote_atoms)
        })
        .then_with(|| left.candidate_name.cmp(right.candidate_name))
}

fn score_candidate(report: &ScenarioPackReport) -> CalibrationScore {
    let count = report.reports.len().max(1) as u128;
    let mean_p05_fill_rate_bps = avg_u16(
        report.reports.iter().map(|report| report.fill_rate_bps.p05),
        count,
    );
    let mean_fill_rate_bps = avg_u16(
        report
            .reports
            .iter()
            .map(|report| report.fill_rate_bps.mean),
        count,
    );
    let mean_update_drop_rate_bps = avg_u16(
        report.reports.iter().map(|report| {
            rate_bps(
                report.quote_updates_dropped.mean,
                report.quote_updates_sent.mean,
            )
        }),
        count,
    );
    let rough_maker_score_quote_atoms = report
        .reports
        .iter()
        .map(|report| {
            (report.fees_quote_atoms.mean as i128)
                .saturating_sub(report.taker_edge_quote_atoms.mean)
        })
        .sum::<i128>()
        / (count as i128);

    CalibrationScore {
        mean_p05_fill_rate_bps,
        mean_fill_rate_bps,
        mean_update_drop_rate_bps,
        rough_maker_score_quote_atoms,
    }
}

fn avg_u16(values: impl Iterator<Item = u16>, count: u128) -> u16 {
    let sum = values.map(|value| value as u128).sum::<u128>();
    (sum / count) as u16
}

fn rate_bps(numerator: u64, denominator: u64) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((numerator.saturating_mul(10_000)) / denominator).min(u16::MAX as u64) as u16
}

fn map_params(
    mut scenarios: [GeneratedReferenceQuoteScenario; 3],
    mut f: impl FnMut(ReferenceQuoteParams) -> ReferenceQuoteParams,
) -> [GeneratedReferenceQuoteScenario; 3] {
    for scenario in &mut scenarios {
        scenario.params = f(scenario.params);
    }
    scenarios
}

fn map_policy(
    mut scenarios: [GeneratedReferenceQuoteScenario; 3],
    mut f: impl FnMut(QuoteUpdatePolicy) -> QuoteUpdatePolicy,
) -> [GeneratedReferenceQuoteScenario; 3] {
    for scenario in &mut scenarios {
        scenario.quote_update_policy = f(scenario.quote_update_policy);
    }
    scenarios
}

fn faster_policy(policy: QuoteUpdatePolicy) -> QuoteUpdatePolicy {
    QuoteUpdatePolicy {
        maker_update_period_slots: (policy.maker_update_period_slots / 2).max(2),
        landing_latency_slots: (policy.landing_latency_slots / 2).max(1),
        update_success_probability_bps: policy
            .update_success_probability_bps
            .saturating_add(900)
            .min(9_950),
        ..policy
    }
}

fn strategy_config_from_scenarios(
    scenarios: &[GeneratedReferenceQuoteScenario; 3],
) -> Result<ReferenceQuoteStrategyConfig, MathError> {
    let params = scenarios[0].params;
    let same_slot_order = scenarios[0].quote_update_policy.same_slot_order;
    let mut max_maker_update_period_slots = 0;
    let mut max_landing_latency_slots = 0;
    let mut min_update_success_probability_bps = u16::MAX;

    for scenario in scenarios {
        if scenario.params != params
            || scenario.quote_update_policy.same_slot_order != same_slot_order
        {
            return Err(MathError::InvalidConfig);
        }
        max_maker_update_period_slots = max_maker_update_period_slots
            .max(scenario.quote_update_policy.maker_update_period_slots);
        max_landing_latency_slots =
            max_landing_latency_slots.max(scenario.quote_update_policy.landing_latency_slots);
        min_update_success_probability_bps = min_update_success_probability_bps
            .min(scenario.quote_update_policy.update_success_probability_bps);
    }

    Ok(ReferenceQuoteStrategyConfig {
        params,
        quote_update_envelope: QuoteUpdateEnvelope {
            max_maker_update_period_slots,
            max_landing_latency_slots,
            min_update_success_probability_bps,
            same_slot_order,
        },
    })
}

fn same_slot_order_str(order: SameSlotUpdateOrder) -> &'static str {
    match order {
        SameSlotUpdateOrder::UpdateBeforeSwap => "update_before_swap",
        SameSlotUpdateOrder::SwapBeforeUpdate => "swap_before_update",
    }
}

fn write_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => write!(out, "\\u{:04x}", ch as u32).unwrap(),
            ch => out.push(ch),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GateSeverity;
    use meta_amm_config::{
        compile_reference_quote_config, AccountBudget, ReferenceQuoteConfigInput,
        REFERENCE_QUOTE_MODE_ID,
    };

    #[test]
    fn calibration_candidates_are_sorted_by_gate_then_fill_tail() {
        let report =
            calibrate_reference_quote(77, ScenarioGateThresholds::reference_quote_default())
                .unwrap();

        assert_eq!(report.candidates.len(), 5);
        let best = report.best().unwrap();
        assert_ne!(best.evaluation.severity(), GateSeverity::Block);
        assert!(report
            .candidates
            .windows(2)
            .all(|pair| pair[0].evaluation.severity() <= pair[1].evaluation.severity()));
        assert!(report
            .candidates
            .iter()
            .any(|candidate| candidate.evaluation.severity() == GateSeverity::Block));
    }

    #[test]
    fn calibration_export_carries_config_gates_and_assumptions() {
        let report =
            calibrate_reference_quote(77, ScenarioGateThresholds::reference_quote_default())
                .unwrap();
        let best = report.best().unwrap();
        let export = export_reference_quote_config(best);

        assert_eq!(report.export_best_config().unwrap(), export);
        assert!(
            best.strategy_config
                .quote_update_envelope
                .max_maker_update_period_slots
                > 0
        );
        assert!(
            best.strategy_config
                .quote_update_envelope
                .min_update_success_probability_bps
                > 0
        );
        assert!(export.contains("\"version\": 1"));
        assert!(export.contains("\"mode\": \"ReferenceQuote\""));
        assert!(export.contains("\"candidate_name\": \""));
        assert!(export.contains("\"params\": {"));
        assert!(export.contains("\"quote_update_envelope\": {"));
        assert!(export.contains("\"scenario_findings\": ["));
        assert!(export.contains("\"scenario_assumptions\": ["));
        assert!(export.contains(
            "\"warning\": \"generated calibration output; not market replay or maker edge\""
        ));
    }

    #[test]
    fn best_calibration_candidate_compiles_to_bounded_config() {
        let report =
            calibrate_reference_quote(77, ScenarioGateThresholds::reference_quote_default())
                .unwrap();
        let best = report.best().unwrap();

        let compiled = compile_reference_quote_config(ReferenceQuoteConfigInput {
            strategy: best.strategy_config,
            token_pair: token_pair(),
            account_budget: AccountBudget::reference_quote_jupiter_default(),
        })
        .unwrap();

        assert_eq!(compiled.mode.as_u8(), REFERENCE_QUOTE_MODE_ID);
        assert_eq!(compiled.params, best.strategy_config.params);
        assert_eq!(
            compiled.quote_update_envelope,
            best.strategy_config.quote_update_envelope
        );
    }

    #[test]
    fn json_string_writer_escapes_control_characters() {
        let mut out = String::new();

        write_json_string(&mut out, "name \"quoted\" \\ path\nnext\tcell");

        assert_eq!(out, "\"name \\\"quoted\\\" \\\\ path\\nnext\\tcell\"");
    }

    fn token_pair() -> meta_amm_config::TokenPairIdentity {
        meta_amm_config::TokenPairIdentity {
            base_mint: [1; 32],
            quote_mint: [2; 32],
            base_token_program: [3; 32],
            quote_token_program: [4; 32],
            base_decimals: 8,
            quote_decimals: 6,
        }
    }
}
