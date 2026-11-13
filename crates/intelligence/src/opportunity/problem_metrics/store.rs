//! Atomically persist one versioned result set and skip unchanged input sets.
use super::{MetricCalculationError, calculate::ProblemMetrics};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

fn count(value: u64) -> Result<i32, MetricCalculationError> {
    i32::try_from(value).map_err(|_| MetricCalculationError::CountOverflow)
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
    let previous: Option<(String, i64)> = sqlx::query_as(
        "SELECT min(input_fingerprint), count(*) FROM linggan_opportunity_problem_metric_period
         WHERE problem_ref=$1 AND taxonomy_version=$2 AND metric_def_version=$3
         HAVING min(input_fingerprint)=max(input_fingerprint)",
    )
    .bind(problem)
    .bind(taxonomy_version)
    .bind(metric_version)
    .fetch_optional(&mut **tx)
    .await?;
    if previous.as_ref().is_some_and(|(previous, rows)| {
        previous == fingerprint && usize::try_from(*rows).ok() == Some(expected_rows)
    }) {
        sqlx::query(
            "UPDATE linggan_opportunity_problem_metric_period SET computed_as_of=$4::timestamptz
                     WHERE problem_ref=$1 AND taxonomy_version=$2 AND metric_def_version=$3",
        )
        .bind(problem)
        .bind(taxonomy_version)
        .bind(metric_version)
        .bind(as_of)
        .execute(&mut **tx)
        .await?;
        return Ok(true);
    }
    Ok(false)
}

pub(super) async fn save(
    tx: &mut Transaction<'_, Postgres>,
    metrics: &ProblemMetrics,
    taxonomy_version: i64,
    metric_version: i32,
    as_of: &str,
) -> Result<(), MetricCalculationError> {
    sqlx::query(
        "DELETE FROM linggan_opportunity_problem_metric_period
                 WHERE problem_ref=$1 AND taxonomy_version=$2 AND metric_def_version=$3",
    )
    .bind(metrics.problem)
    .bind(taxonomy_version)
    .bind(metric_version)
    .execute(&mut **tx)
    .await?;
    for row in &metrics.rows {
        sqlx::query(
            "INSERT INTO linggan_opportunity_problem_metric_period(
             problem_ref, period_start, taxonomy_version, metric_def_version, protocol_version,
             k, n, ci_low, ci_high, empty_reason, baseline_k, baseline_n, no_baseline, mentioners_total,
             evidence_tier, input_fingerprint, computed_as_of)
             VALUES ($1,$2::date,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17::timestamptz)",
        )
        .bind(metrics.problem)
        .bind(&row.period_start)
        .bind(taxonomy_version)
        .bind(metric_version)
        .bind(row.protocol_version)
        .bind(count(row.k)?)
        .bind(count(row.n)?)
        .bind(row.ci_low)
        .bind(row.ci_high)
        .bind(row.empty_reason)
        .bind(count(row.baseline_k)?)
        .bind(count(row.baseline_n)?)
        .bind(row.no_baseline)
        .bind(count(row.mentioners_total)?)
        .bind(row.tier)
        .bind(&metrics.fingerprint)
        .bind(as_of)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
