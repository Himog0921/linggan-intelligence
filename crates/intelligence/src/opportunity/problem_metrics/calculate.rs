//! Period aggregates, Wilson intervals and baseline decisions.
use super::{MetricCalculationError, MetricDefinition, ProtocolVersion, inputs::MetricInputs};
use crate::opportunity::{EvidenceTier, TierInput, TierParams, evidence_tier, wilson_interval};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricPeriod {
    pub problem_ref: Uuid,
    pub taxonomy_version: i64,
    pub metric_def_version: i32,
    pub period_start: String,
    pub protocol_version: Option<i32>,
    pub k: u64,
    pub n: u64,
    pub ci_low: Option<f64>,
    pub ci_high: Option<f64>,
    pub empty_reason: Option<&'static str>,
    pub baseline_k: u64,
    pub baseline_n: u64,
    pub no_baseline: bool,
    pub mentioners_total: u64,
    pub tier: &'static str,
}

pub(super) struct ProblemMetrics {
    pub(super) problem: Uuid,
    pub(super) fingerprint: String,
    pub(super) rows: Vec<MetricPeriod>,
}

type PeriodPeople<'a> = BTreeMap<ProtocolVersion, BTreeMap<&'a str, BTreeSet<&'a str>>>;
type AllPeople<'a> = BTreeMap<ProtocolVersion, BTreeSet<&'a str>>;

fn collect_people(problem: Uuid, inputs: &MetricInputs) -> (PeriodPeople<'_>, AllPeople<'_>) {
    let mut people = PeriodPeople::new();
    let mut all_people = AllPeople::new();
    if let Some(keys) = inputs.mentions.get(&problem) {
        for key in keys {
            if let Some(comment) = inputs.eligible.get(key)
                && let Some(hash) = &comment.hash
            {
                people
                    .entry(comment.protocol)
                    .or_default()
                    .entry(&comment.period)
                    .or_default()
                    .insert(hash);
                all_people.entry(comment.protocol).or_default().insert(hash);
            }
        }
    }
    (people, all_people)
}

pub(super) fn calculate(
    problem: Uuid,
    taxonomy_version: i64,
    fingerprint: String,
    definition: &MetricDefinition,
    periods: &[String],
    inputs: &MetricInputs,
) -> Result<ProblemMetrics, MetricCalculationError> {
    let (people, all_people) = collect_people(problem, inputs);
    let count = definition
        .params
        .series_periods
        .checked_add(definition.params.baseline_periods)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(MetricCalculationError::CountOverflow)?;
    let baseline_count = usize::try_from(definition.params.baseline_periods)
        .map_err(|_| MetricCalculationError::CountOverflow)?;
    let mut rows = Vec::new();
    for (protocol, counts) in &inputs.denominators {
        for (index, period) in periods.iter().take(count).enumerate() {
            let n = counts.get(period).copied().unwrap_or(0);
            let k = people
                .get(protocol)
                .and_then(|p| p.get(period.as_str()))
                .map_or(0, |p| p.len() as u64);
            let baseline = &periods[index + 1..index + 1 + baseline_count];
            let baseline_n: u64 = baseline
                .iter()
                .map(|p| counts.get(p).copied().unwrap_or(0))
                .sum();
            let baseline_k: u64 = baseline
                .iter()
                .map(|p| {
                    people
                        .get(protocol)
                        .and_then(|group| group.get(p.as_str()))
                        .map_or(0, |p| p.len() as u64)
                })
                .sum();
            let no_baseline = baseline
                .iter()
                .any(|p| counts.get(p).copied().unwrap_or(0) == 0)
                || baseline_n < definition.params.min_n_period.into();
            let total = all_people
                .get(protocol)
                .map_or(0, |people| people.len() as u64);
            let tier = evidence_tier(
                &TierInput {
                    mentioners_total: total,
                    k_cur: k,
                    n_cur: n,
                    k_base: baseline_k,
                    n_base: if no_baseline { 0 } else { baseline_n },
                },
                &TierParams {
                    z: definition.params.z,
                    min_k_significant: definition.params.min_k_significant.into(),
                    min_n_period: definition.params.min_n_period.into(),
                    min_total_for_significant: definition.params.min_total_for_significant.into(),
                },
            );
            let interval = if n == 0 {
                None
            } else {
                Some(
                    wilson_interval(k, n, definition.params.z)
                        .map_err(|_| MetricCalculationError::InvalidCounts)?,
                )
            };
            rows.push(MetricPeriod {
                problem_ref: problem,
                taxonomy_version,
                metric_def_version: definition.version,
                period_start: period.clone(),
                protocol_version: *protocol,
                k,
                n,
                ci_low: interval.map(|v| v.low),
                ci_high: interval.map(|v| v.high),
                empty_reason: (n == 0).then_some("no_eligible_comments"),
                baseline_k,
                baseline_n,
                no_baseline,
                mentioners_total: total,
                tier: match tier {
                    EvidenceTier::Candidate => "candidate",
                    EvidenceTier::Established => "established",
                    EvidenceTier::Significant => "significant",
                },
            });
        }
    }
    Ok(ProblemMetrics {
        problem,
        fingerprint,
        rows,
    })
}

pub(super) fn fingerprint(
    problem: Uuid,
    taxonomy_version: i64,
    definition: &MetricDefinition,
    periods: &[String],
    inputs: &MetricInputs,
) -> Result<String, MetricCalculationError> {
    let eligible: Vec<_> = inputs
        .eligible
        .iter()
        .map(|((content, comment), value)| (content, comment, value))
        .collect();
    let fingerprint_input = serde_json::to_vec(&(
        taxonomy_version,
        definition.version,
        &definition.params_source,
        periods,
        eligible,
        inputs.mentions.get(&problem),
    ))
    .map_err(|_| MetricCalculationError::Fingerprint)?;
    Ok(Sha256::digest(fingerprint_input)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
