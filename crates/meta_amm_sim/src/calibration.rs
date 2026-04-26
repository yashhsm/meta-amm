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
        reports.push(CalibrationCandidateReport {
            candidate_name: candidate.name,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GateSeverity;

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
}
