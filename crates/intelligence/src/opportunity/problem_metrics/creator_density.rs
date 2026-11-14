//! Creator concentration from the same eligible comments and mentions as problem metrics.
use super::{MetricCalculationError, MetricDefinition, ProtocolVersion, inputs::MetricInputs};
use crate::opportunity::wilson_interval;
use serde::Serialize;
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(super) type Cohorts<'a> = BTreeMap<(ProtocolVersion, &'a str), u64>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorDensity {
    pub creator_id: String,
    pub k: u64,
    pub n: u64,
    pub ci_low: f64,
    pub ci_high: f64,
    pub sufficient: bool,
}

/// A group is one problem, definition, taxonomy and protocol. Its rankings are never mixed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorDensityGroup {
    pub problem_ref: Uuid,
    pub taxonomy_version: i64,
    pub metric_def_version: i32,
    pub window_days: u32,
    pub protocol_version: ProtocolVersion,
    pub ranked: Vec<CreatorDensity>,
    pub insufficient: Vec<CreatorDensity>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorWindowExclusions {
    pub metric_def_version: i32,
    pub missing_work_published_at: usize,
    pub outside_window: usize,
}

pub(super) fn cohorts(inputs: &MetricInputs) -> Cohorts<'_> {
    let mut result = Cohorts::new();
    for comment in inputs.eligible.values() {
        if comment.creator_in_window == Some(true) {
            *result
                .entry((comment.protocol, &comment.creator_id))
                .or_default() += 1;
        }
    }
    result
}

pub(super) fn window_exclusions(
    inputs: &MetricInputs,
    metric_def_version: i32,
) -> CreatorWindowExclusions {
    CreatorWindowExclusions {
        metric_def_version,
        missing_work_published_at: inputs
            .eligible
            .values()
            .filter(|comment| comment.creator_in_window.is_none())
            .count(),
        outside_window: inputs
            .eligible
            .values()
            .filter(|comment| comment.creator_in_window == Some(false))
            .count(),
    }
}

pub(super) fn calculate(
    problem: Uuid,
    taxonomy_version: i64,
    definition: &MetricDefinition,
    inputs: &MetricInputs,
    cohorts: &Cohorts<'_>,
) -> Result<Vec<CreatorDensityGroup>, MetricCalculationError> {
    let mut people: BTreeMap<(ProtocolVersion, &str), BTreeSet<&str>> = BTreeMap::new();
    if let Some(mentions) = inputs.mentions.get(&problem) {
        for key in mentions {
            if let Some(comment) = inputs.eligible.get(key)
                && comment.creator_in_window == Some(true)
                && let Some(hash) = &comment.hash
            {
                people
                    .entry((comment.protocol, &comment.creator_id))
                    .or_default()
                    .insert(hash);
            }
        }
    }
    let mut groups: BTreeMap<ProtocolVersion, CreatorDensityGroup> = BTreeMap::new();
    for (&(protocol, creator_id), &n) in cohorts {
        let k = people
            .get(&(protocol, creator_id))
            .map_or(0, |p| p.len() as u64);
        let interval = wilson_interval(k, n, definition.params.z)
            .map_err(|_| MetricCalculationError::InvalidCounts)?;
        let density = CreatorDensity {
            creator_id: creator_id.to_owned(),
            k,
            n,
            ci_low: interval.low,
            ci_high: interval.high,
            sufficient: n >= definition.params.creator_min_n.into(),
        };
        let group = groups
            .entry(protocol)
            .or_insert_with(|| CreatorDensityGroup {
                problem_ref: problem,
                taxonomy_version,
                metric_def_version: definition.version,
                window_days: definition.params.creator_window_days,
                protocol_version: protocol,
                ranked: Vec::new(),
                insufficient: Vec::new(),
            });
        if density.sufficient {
            group.ranked.push(density);
        } else {
            group.insufficient.push(density);
        }
    }
    let mut result: Vec<_> = groups.into_values().collect();
    for group in &mut result {
        sort_creator_density_group(group);
    }
    Ok(result)
}

/// Apply the shared ordering to one protocol group, without comparing different protocols.
pub fn sort_creator_density_group(group: &mut CreatorDensityGroup) {
    group.ranked.sort_by(|a, b| {
        b.ci_low
            .total_cmp(&a.ci_low)
            .then_with(|| b.n.cmp(&a.n))
            .then_with(|| a.creator_id.cmp(&b.creator_id))
    });
    group
        .insufficient
        .sort_by(|a, b| b.n.cmp(&a.n).then_with(|| a.creator_id.cmp(&b.creator_id)));
}

pub(super) async fn unchanged(
    tx: &mut Transaction<'_, Postgres>,
    problem: Uuid,
    taxonomy_version: i64,
    metric_version: i32,
    as_of: &str,
    fingerprint: &str,
    expected_rows: usize,
) -> Result<bool, MetricCalculationError> {
    let previous: (i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT count(*), min(input_fingerprint), max(input_fingerprint)
         FROM linggan_opportunity_creator_problem_density
         WHERE problem_ref=$1 AND taxonomy_version=$2 AND metric_def_version=$3",
    )
    .bind(problem)
    .bind(taxonomy_version)
    .bind(metric_version)
    .fetch_one(&mut **tx)
    .await?;
    let same_count = usize::try_from(previous.0).ok() == Some(expected_rows);
    let same_fingerprint = if expected_rows == 0 {
        previous.1.is_none() && previous.2.is_none()
    } else {
        previous.1.as_deref() == Some(fingerprint) && previous.2.as_deref() == Some(fingerprint)
    };
    if !same_count || !same_fingerprint {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE linggan_opportunity_creator_problem_density SET computed_as_of=$4::timestamptz
         WHERE problem_ref=$1 AND taxonomy_version=$2 AND metric_def_version=$3",
    )
    .bind(problem)
    .bind(taxonomy_version)
    .bind(metric_version)
    .bind(as_of)
    .execute(&mut **tx)
    .await?;
    Ok(true)
}

pub(super) async fn save(
    tx: &mut Transaction<'_, Postgres>,
    problem: Uuid,
    taxonomy_version: i64,
    metric_version: i32,
    as_of: &str,
    fingerprint: &str,
    groups: &[CreatorDensityGroup],
) -> Result<usize, MetricCalculationError> {
    sqlx::query(
        "DELETE FROM linggan_opportunity_creator_problem_density
         WHERE problem_ref=$1 AND taxonomy_version=$2 AND metric_def_version=$3",
    )
    .bind(problem)
    .bind(taxonomy_version)
    .bind(metric_version)
    .execute(&mut **tx)
    .await?;
    let mut written = 0;
    for group in groups {
        for density in group.ranked.iter().chain(&group.insufficient) {
            sqlx::query(
                "INSERT INTO linggan_opportunity_creator_problem_density(
                 creator_id,problem_ref,window_days,taxonomy_version,metric_def_version,protocol_version,
                 k,n,ci_low,ci_high,sufficient,input_fingerprint,computed_as_of)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13::timestamptz)",
            )
            .bind(&density.creator_id)
            .bind(problem)
            .bind(i32::try_from(group.window_days).map_err(|_| MetricCalculationError::CountOverflow)?)
            .bind(taxonomy_version)
            .bind(metric_version)
            .bind(group.protocol_version)
            .bind(i32::try_from(density.k).map_err(|_| MetricCalculationError::CountOverflow)?)
            .bind(i32::try_from(density.n).map_err(|_| MetricCalculationError::CountOverflow)?)
            .bind(density.ci_low)
            .bind(density.ci_high)
            .bind(density.sufficient)
            .bind(fingerprint)
            .bind(as_of)
            .execute(&mut **tx)
            .await?;
            written += 1;
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn density(id: &str, k: u64, n: u64, sufficient: bool) -> CreatorDensity {
        let interval = wilson_interval(k, n, 1.959963984540054).unwrap();
        CreatorDensity {
            creator_id: id.to_owned(),
            k,
            n,
            ci_low: interval.low,
            ci_high: interval.high,
            sufficient,
        }
    }

    #[test]
    fn playbook_creator_order_and_ties_are_stable_within_a_protocol() {
        let mut group = CreatorDensityGroup {
            problem_ref: Uuid::nil(),
            taxonomy_version: 1,
            metric_def_version: 1,
            window_days: 90,
            protocol_version: Some(1),
            ranked: vec![
                density("C", 6, 1850, true),
                density("B", 5, 96, true),
                density("A", 7, 120, true),
            ],
            insufficient: vec![
                density("F", 0, 30, false),
                density("D", 3, 40, false),
                density("E", 0, 30, false),
            ],
        };
        sort_creator_density_group(&mut group);
        assert_eq!(
            group
                .ranked
                .iter()
                .map(|v| v.creator_id.as_str())
                .collect::<Vec<_>>(),
            ["A", "B", "C"]
        );
        assert_eq!(
            group
                .insufficient
                .iter()
                .map(|v| v.creator_id.as_str())
                .collect::<Vec<_>>(),
            ["D", "E", "F"]
        );
        group.ranked = vec![
            density("B", 0, 100, true),
            density("A", 0, 100, true),
            density("C", 0, 120, true),
        ];
        // Zero mentions yield the same exact lower bound for each sample size.
        sort_creator_density_group(&mut group);
        assert_eq!(
            group
                .ranked
                .iter()
                .map(|v| v.creator_id.as_str())
                .collect::<Vec<_>>(),
            ["C", "A", "B"]
        );
    }
}
