//! Engineering gates over frozen paired shadow executions. These metrics measure transport and
//! contract behavior only; they neither claim semantic accuracy nor compare v5-only dimensions
//! with v4 labels.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplayThresholds {
    pub version: String,
    pub minimum_pairs: usize,
    pub minimum_works: usize,
    pub minimum_pair_coverage: f64,
    pub maximum_schema_failure: f64,
    pub maximum_schema_increase: f64,
    pub maximum_no_signal_shift: f64,
    pub maximum_token_ratio: f64,
    pub maximum_latency_ratio: f64,
    /// Strict normalized atom/evidence overlap for a requested repeat set. This never treats
    /// different strings as proof of semantic equivalence.
    #[serde(default = "default_repeat_consistency")]
    pub minimum_repeat_consistency: f64,
}

const fn default_repeat_consistency() -> f64 {
    0.90
}

impl Default for ReplayThresholds {
    fn default() -> Self {
        Self {
            version: "comment-replay.engineering.v1".into(),
            minimum_pairs: 30,
            minimum_works: 5,
            minimum_pair_coverage: 0.90,
            maximum_schema_failure: 0.05,
            maximum_schema_increase: 0.02,
            maximum_no_signal_shift: 0.15,
            maximum_token_ratio: 1.5,
            maximum_latency_ratio: 2.0,
            minimum_repeat_consistency: default_repeat_consistency(),
        }
    }
}

impl ReplayThresholds {
    pub fn valid(&self) -> bool {
        self.version == "comment-replay.engineering.v1"
            && self.minimum_pairs <= 120
            && self.minimum_works <= self.minimum_pairs
            && (0.0..=1.0).contains(&self.minimum_pair_coverage)
            && (0.0..=1.0).contains(&self.maximum_schema_failure)
            && (0.0..=1.0).contains(&self.maximum_schema_increase)
            && (0.0..=1.0).contains(&self.maximum_no_signal_shift)
            && self.maximum_token_ratio >= 1.0
            && self.maximum_latency_ratio >= 1.0
            && (0.0..=1.0).contains(&self.minimum_repeat_consistency)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplayObservation {
    pub structure_accepted: bool,
    pub no_signal: bool,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub elapsed_ms: Option<u64>,
    /// A violated invariant is a failed gate, even if another metric improves.
    pub invariant_violations: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ReplayPair {
    pub work_ref: Uuid,
    pub eligible: bool,
    pub baseline: Option<ReplayObservation>,
    pub candidate: Option<ReplayObservation>,
}

/// Candidate-only repeat receipts. Values are hashes of normalized kind+evidence-position
/// records, so no raw model output is persisted or read by the engineering comparison.
#[derive(Clone, Debug)]
pub struct ReplayRepeat {
    pub work_ref: Uuid,
    pub eligible: bool,
    pub initial: Option<ReplayObservation>,
    pub repeated: Option<ReplayObservation>,
    pub initial_atom_evidence_hashes: Option<BTreeSet<String>>,
    pub repeated_atom_evidence_hashes: Option<BTreeSet<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayComparison {
    pub state: &'static str,
    pub thresholds: ReplayThresholds,
    pub frozen_count: usize,
    pub eligible_count: usize,
    pub excluded_count: usize,
    pub completed_pairs: usize,
    pub unpaired_eligible_count: usize,
    pub work_count: usize,
    pub pair_coverage: Option<f64>,
    pub baseline_schema_failure: Option<f64>,
    pub candidate_schema_failure: Option<f64>,
    pub baseline_no_signal: Option<f64>,
    pub candidate_no_signal: Option<f64>,
    pub token_ratio: Option<f64>,
    pub latency_ratio: Option<f64>,
    pub unknown_usage_pairs: usize,
    pub unknown_latency_pairs: usize,
    pub repeat_requested_count: usize,
    pub repeat_completed_count: usize,
    pub repeat_consistency: Option<f64>,
    pub repeat_consistency_method: &'static str,
    pub new_dimensions_not_directly_compared: bool,
    pub reasons: Vec<&'static str>,
}

fn ratio(numerator: f64, denominator: f64) -> Option<f64> {
    (denominator > 0.0).then_some(numerator / denominator)
}

fn mean_ratio(values: &[(Option<u64>, Option<u64>)]) -> Option<f64> {
    if values.is_empty()
        || values
            .iter()
            .any(|(left, right)| left.is_none() || right.is_none())
    {
        return None;
    }
    let baseline = values
        .iter()
        .map(|(left, _)| left.expect("checked") as f64)
        .sum();
    let candidate = values
        .iter()
        .map(|(_, right)| right.expect("checked") as f64)
        .sum();
    ratio(candidate, baseline)
}

fn tokens(observation: &ReplayObservation) -> Option<u64> {
    observation
        .input_tokens?
        .checked_add(observation.output_tokens?)
}

pub fn compare(pairs: &[ReplayPair]) -> ReplayComparison {
    compare_with_thresholds(pairs, &[], ReplayThresholds::default())
}

pub fn compare_with_thresholds(
    pairs: &[ReplayPair],
    repeats: &[ReplayRepeat],
    thresholds: ReplayThresholds,
) -> ReplayComparison {
    compare_with_schema_mode(pairs, repeats, thresholds, true)
}

/// Both versions are always evaluated through their own builder and validator. For a v4→v5
/// comparison, only the shared engineering fields below are comparable; v5-only atomic
/// dimensions are deliberately excluded. A v5→v5 revision comparison has no such exclusion.
pub fn compare_with_schema_mode(
    pairs: &[ReplayPair],
    repeats: &[ReplayRepeat],
    thresholds: ReplayThresholds,
    cross_schema: bool,
) -> ReplayComparison {
    let eligible_count = pairs.iter().filter(|pair| pair.eligible).count();
    let unpaired_eligible_count = pairs
        .iter()
        .filter(|pair| pair.eligible && (pair.baseline.is_some() != pair.candidate.is_some()))
        .count();
    let completed: Vec<_> = pairs
        .iter()
        .filter_map(|pair| {
            pair.eligible.then_some(())?;
            Some((
                pair.work_ref,
                pair.baseline.as_ref()?,
                pair.candidate.as_ref()?,
            ))
        })
        .collect();
    let completed_pairs = completed.len();
    let work_count = completed
        .iter()
        .map(|(work_ref, _, _)| *work_ref)
        .collect::<BTreeSet<_>>()
        .len();
    let schema = |candidate: bool| {
        ratio(
            completed
                .iter()
                .filter(|(_, baseline, candidate_result)| {
                    !(if candidate {
                        candidate_result
                    } else {
                        baseline
                    })
                    .structure_accepted
                })
                .count() as f64,
            completed_pairs as f64,
        )
    };
    let usage: Vec<_> = completed
        .iter()
        .map(|(_, baseline, candidate)| (tokens(baseline), tokens(candidate)))
        .collect();
    let latency: Vec<_> = completed
        .iter()
        .map(|(_, baseline, candidate)| (baseline.elapsed_ms, candidate.elapsed_ms))
        .collect();
    let (repeat_requested_count, repeat_completed_count, repeat_consistency) =
        repeat_summary(repeats);
    let mut report = ReplayComparison {
        state: "insufficient_evidence",
        thresholds,
        frozen_count: pairs.len(),
        eligible_count,
        excluded_count: pairs.len().saturating_sub(eligible_count),
        completed_pairs,
        unpaired_eligible_count,
        work_count,
        pair_coverage: ratio(completed_pairs as f64, eligible_count as f64),
        baseline_schema_failure: schema(false),
        candidate_schema_failure: schema(true),
        baseline_no_signal: no_signal_rate(&completed, false),
        candidate_no_signal: no_signal_rate(&completed, true),
        token_ratio: mean_ratio(&usage),
        latency_ratio: mean_ratio(&latency),
        unknown_usage_pairs: usage
            .iter()
            .filter(|(baseline, candidate)| baseline.is_none() || candidate.is_none())
            .count(),
        unknown_latency_pairs: latency
            .iter()
            .filter(|(baseline, candidate)| baseline.is_none() || candidate.is_none())
            .count(),
        repeat_requested_count,
        repeat_completed_count,
        repeat_consistency,
        repeat_consistency_method: "normalized_atom_evidence_jaccard.v1",
        new_dimensions_not_directly_compared: cross_schema,
        reasons: Vec::new(),
    };
    qualify(&mut report, &completed);
    report
}

fn no_signal_rate(
    completed: &[(Uuid, &ReplayObservation, &ReplayObservation)],
    candidate: bool,
) -> Option<f64> {
    let accepted: Vec<_> = completed
        .iter()
        .map(|(_, baseline, candidate_result)| {
            if candidate {
                *candidate_result
            } else {
                *baseline
            }
        })
        .filter(|observation| observation.structure_accepted)
        .collect();
    ratio(
        accepted
            .iter()
            .filter(|observation| observation.no_signal)
            .count() as f64,
        accepted.len() as f64,
    )
}

fn repeat_summary(repeats: &[ReplayRepeat]) -> (usize, usize, Option<f64>) {
    let completed: Vec<_> = repeats
        .iter()
        .filter_map(|repeat| {
            repeat.eligible.then_some(())?;
            Some((
                repeat.initial.as_ref()?,
                repeat.repeated.as_ref()?,
                repeat.initial_atom_evidence_hashes.as_ref()?,
                repeat.repeated_atom_evidence_hashes.as_ref()?,
            ))
        })
        .collect();
    let requested = repeats.iter().filter(|repeat| repeat.eligible).count();
    let consistency = (!completed.is_empty()).then(|| {
        completed
            .iter()
            .map(|(initial, repeated, left, right)| {
                if !initial.structure_accepted || !repeated.structure_accepted {
                    0.0
                } else if left.is_empty() && right.is_empty() {
                    1.0
                } else {
                    left.intersection(right).count() as f64 / left.union(right).count() as f64
                }
            })
            .sum::<f64>()
            / completed.len() as f64
    });
    (requested, completed.len(), consistency)
}

fn qualify(
    report: &mut ReplayComparison,
    completed: &[(Uuid, &ReplayObservation, &ReplayObservation)],
) {
    let thresholds = &report.thresholds;
    if !thresholds.valid() {
        report.reasons.push("thresholds_invalid");
        report.state = "degraded";
        return;
    }
    let invariant_failed = completed.iter().any(|(_, baseline, candidate)| {
        !baseline.invariant_violations.is_empty()
            || !candidate.invariant_violations.is_empty()
            || (candidate.no_signal && !candidate.structure_accepted)
    });
    if invariant_failed {
        report.reasons.push("invariant_violation");
    }
    if report.unpaired_eligible_count > 0 {
        report.reasons.push("unpaired_execution");
    }
    if report.completed_pairs < thresholds.minimum_pairs {
        report.reasons.push("paired_sample_insufficient");
    }
    if report.work_count < thresholds.minimum_works {
        report.reasons.push("work_coverage_insufficient");
    }
    if report
        .pair_coverage
        .is_none_or(|value| value < thresholds.minimum_pair_coverage)
    {
        report.reasons.push("pair_coverage_insufficient");
    }
    if report.token_ratio.is_none() {
        report.reasons.push("token_comparison_unknown");
    }
    if report.latency_ratio.is_none() {
        report.reasons.push("latency_comparison_unknown");
    }
    if report.repeat_requested_count > 0 {
        if report.repeat_completed_count < report.repeat_requested_count {
            report.reasons.push("repeat_consistency_insufficient");
        } else if report
            .repeat_consistency
            .is_none_or(|value| value < thresholds.minimum_repeat_consistency)
        {
            report.reasons.push("repeat_consistency_regression");
        }
    }
    if invariant_failed || report.unpaired_eligible_count > 0 {
        report.state = "degraded";
        return;
    }
    if !report.reasons.is_empty() {
        return;
    }
    if report.candidate_schema_failure.is_some_and(|candidate| {
        candidate > thresholds.maximum_schema_failure
            || report
                .baseline_schema_failure
                .is_some_and(|baseline| candidate > baseline + thresholds.maximum_schema_increase)
    }) {
        report.reasons.push("schema_failure_regression");
    }
    if report
        .baseline_no_signal
        .zip(report.candidate_no_signal)
        .is_some_and(|(baseline, candidate)| {
            (candidate - baseline).abs() > thresholds.maximum_no_signal_shift
        })
    {
        report.reasons.push("no_signal_distribution_shift");
    }
    if report
        .token_ratio
        .is_some_and(|value| value > thresholds.maximum_token_ratio)
    {
        report.reasons.push("token_regression");
    }
    if report
        .latency_ratio
        .is_some_and(|value| value > thresholds.maximum_latency_ratio)
    {
        report.reasons.push("latency_regression");
    }
    report.state = if report.reasons.is_empty() {
        "pass"
    } else {
        "degraded"
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs() -> Vec<ReplayPair> {
        (0..30)
            .map(|index| {
                let result = ReplayObservation {
                    structure_accepted: true,
                    no_signal: false,
                    input_tokens: Some(100),
                    output_tokens: Some(20),
                    elapsed_ms: Some(1000),
                    invariant_violations: vec![],
                };
                ReplayPair {
                    work_ref: Uuid::from_u128(index % 5),
                    eligible: true,
                    baseline: Some(result.clone()),
                    candidate: Some(result),
                }
            })
            .collect()
    }

    #[test]
    fn complete_pairs_pass_engineering_gates() {
        let report = compare(&pairs());
        assert_eq!(report.state, "pass");
        assert_eq!(report.completed_pairs, 30);
        assert!(report.new_dimensions_not_directly_compared);
    }

    #[test]
    fn unknown_usage_cannot_pass_or_become_zero_cost() {
        let mut values = pairs();
        values[0]
            .candidate
            .as_mut()
            .expect("candidate")
            .input_tokens = None;
        let report = compare(&values);
        assert_eq!(report.state, "insufficient_evidence");
        assert_eq!(report.token_ratio, None);
        assert_eq!(report.unknown_usage_pairs, 1);
    }

    #[test]
    fn source_exclusion_removes_both_sides_and_keeps_frozen_denominator() {
        let mut values = pairs();
        values[0].eligible = false;
        let report = compare(&values);
        assert_eq!(report.frozen_count, 30);
        assert_eq!(report.completed_pairs, 29);
        assert_eq!(report.excluded_count, 1);
        assert_eq!(report.state, "insufficient_evidence");
    }

    #[test]
    fn unpaired_eligible_execution_is_a_hard_violation() {
        let mut values = pairs();
        values[0].candidate = None;
        let report = compare(&values);
        assert_eq!(report.state, "degraded");
        assert_eq!(report.unpaired_eligible_count, 1);
        assert!(report.reasons.contains(&"unpaired_execution"));
    }

    #[test]
    fn high_schema_failure_cannot_be_offset_by_lower_cost() {
        let mut values = pairs();
        for item in values.iter_mut().take(3) {
            let candidate = item.candidate.as_mut().expect("candidate");
            candidate.structure_accepted = false;
            candidate.output_tokens = Some(1);
        }
        let report = compare(&values);
        assert_eq!(report.state, "degraded");
        assert!(report.reasons.contains(&"schema_failure_regression"));
    }

    #[test]
    fn repeat_consistency_has_its_own_denominator_and_never_compares_new_dimensions() {
        let observation = ReplayObservation {
            structure_accepted: true,
            no_signal: false,
            input_tokens: Some(10),
            output_tokens: Some(2),
            elapsed_ms: Some(10),
            invariant_violations: vec![],
        };
        let repeat = ReplayRepeat {
            work_ref: Uuid::nil(),
            eligible: true,
            initial: Some(observation.clone()),
            repeated: Some(observation),
            initial_atom_evidence_hashes: Some(["a".to_owned(), "b".to_owned()].into()),
            repeated_atom_evidence_hashes: Some(["a".to_owned(), "b".to_owned()].into()),
        };
        let report = compare_with_thresholds(&pairs(), &[repeat], ReplayThresholds::default());
        assert_eq!(report.repeat_requested_count, 1);
        assert_eq!(report.repeat_completed_count, 1);
        assert_eq!(report.repeat_consistency, Some(1.0));
        assert_eq!(
            report.repeat_consistency_method,
            "normalized_atom_evidence_jaccard.v1"
        );
        assert!(report.new_dimensions_not_directly_compared);
    }

    #[test]
    fn v5_to_v5_compares_the_same_schema_without_a_cross_schema_marker() {
        let report = compare_with_schema_mode(&pairs(), &[], ReplayThresholds::default(), false);
        assert_eq!(report.state, "pass");
        assert!(!report.new_dimensions_not_directly_compared);
    }
}
