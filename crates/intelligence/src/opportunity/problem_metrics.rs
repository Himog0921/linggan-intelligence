//! Compute versioned, protocol-separated problem periods from one as-of fact set.
mod calculate;
mod inputs;
mod store;
pub use calculate::MetricPeriod;

use super::{
    CommenterDigest, InvariantReport, MentionSyncError, OpportunityMetricError,
    OpportunityMetricParams, PeriodBasis,
};
use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

type ProtocolVersion = Option<i32>;

#[derive(Debug, thiserror::Error)]
pub enum MetricCalculationError {
    #[error("opportunity_metric_database_unavailable")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Definition(#[from] OpportunityMetricError),
    #[error("{0}")]
    MentionIntegrity(#[from] MentionSyncError),
    #[error("opportunity_metric_definition_unavailable_as_of")]
    DefinitionUnavailable,
    #[error("opportunity_metric_shadow_unavailable_as_of")]
    ShadowUnavailable,
    #[error("opportunity_taxonomy_version_unavailable_as_of")]
    TaxonomyUnavailable,
    #[error("opportunity_problem_registration_unavailable_as_of")]
    ProblemRegistrationUnavailable,
    #[error("opportunity_metric_future_as_of")]
    FutureAsOf,
    #[error("opportunity_metric_invalid_protocol")]
    InvalidProtocol,
    #[error("opportunity_metric_digest_version_mismatch")]
    DigestVersionMismatch,
    #[error("opportunity_metric_count_overflow")]
    CountOverflow,
    #[error("opportunity_metric_invalid_counts")]
    InvalidCounts,
    #[error("opportunity_metric_fingerprint_failed")]
    Fingerprint,
    #[error("opportunity_metric_rebuild_busy")]
    Busy,
    #[error("opportunity_metric_newer_result_exists")]
    NewerResultExists,
    #[error("opportunity_metric_invariant_violations: {checks:?}")]
    InvariantViolations { checks: Vec<&'static str> },
}

/// Incremental mode checks each problem's input fingerprint; full mode rebuilds every problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildMode {
    Incremental,
    Full,
    Preview,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricRebuildReport {
    pub definitions: usize,
    pub problems_rebuilt: usize,
    pub problems_unchanged: usize,
    pub period_rows_written: usize,
    pub problems_previewed: usize,
    pub preview: Vec<MetricPeriod>,
    pub exclusions: Vec<MetricExclusion>,
    pub identity_conflicts: Vec<MetricIdentityConflicts>,
    pub invariants: Option<InvariantReport>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricExclusion {
    pub metric_def_version: i32,
    pub reason: String,
    pub comments: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricIdentityConflicts {
    pub metric_def_version: i32,
    pub comments: i64,
}

struct MetricDefinition {
    version: i32,
    params: OpportunityMetricParams,
    params_source: serde_json::Value,
}

async fn definitions(
    tx: &mut Transaction<'_, Postgres>,
    as_of: &str,
) -> Result<Vec<MetricDefinition>, MetricCalculationError> {
    let row = sqlx::query(
        "SELECT definition.version, definition.params
         FROM linggan_opportunity_metric_definition_current current
         JOIN linggan_opportunity_metric_definition definition ON definition.version=current.version
         WHERE current.singleton AND current.switched_at <= $1::timestamptz
           AND definition.created_at <= $1::timestamptz",
    )
    .bind(as_of)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MetricCalculationError::DefinitionUnavailable)?;
    let params_source: serde_json::Value = row.try_get("params")?;
    let current = MetricDefinition {
        version: row.try_get("version")?,
        params: super::metric_definition::parse_params(params_source.clone())?,
        params_source,
    };
    let mut definitions = Vec::new();
    if current.params.period_basis == PeriodBasis::WorkPublishedCohort {
        let shadow = sqlx::query(
            "SELECT definition.version, definition.params
             FROM linggan_opportunity_metric_shadow shadow
             JOIN linggan_opportunity_metric_definition definition ON definition.version=shadow.shadow_version
             WHERE shadow.current_version=$1 AND definition.created_at <= $2::timestamptz",
        )
        .bind(current.version)
        .bind(as_of)
        .fetch_optional(&mut **tx)
        .await?;
        let row = shadow.ok_or(MetricCalculationError::ShadowUnavailable)?;
        let params_source: serde_json::Value = row.try_get("params")?;
        let definition = MetricDefinition {
            version: row.try_get("version")?,
            params: super::metric_definition::parse_params(params_source.clone())?,
            params_source,
        };
        if definition.params.period_basis != PeriodBasis::CommentPublished {
            return Err(MetricCalculationError::ShadowUnavailable);
        }
        definitions.push(definition);
    }
    definitions.insert(0, current);
    Ok(definitions)
}

async fn periods(
    tx: &mut Transaction<'_, Postgres>,
    as_of: &str,
    definition: &MetricDefinition,
) -> Result<Vec<String>, MetricCalculationError> {
    let count = definition
        .params
        .series_periods
        .checked_add(definition.params.baseline_periods)
        .and_then(|n| n.checked_add(definition.params.baseline_periods))
        .and_then(|n| i32::try_from(n).ok())
        .ok_or(MetricCalculationError::CountOverflow)?;
    Ok(sqlx::query_scalar(
        "SELECT (linggan_opportunity_period_start($1::timestamptz,$2)::timestamp
                 - make_interval(weeks => step))::date::text
         FROM generate_series(0,$3 - 1) step ORDER BY step",
    )
    .bind(as_of)
    .bind(&definition.params.period_tz)
    .bind(count)
    .fetch_all(&mut **tx)
    .await?)
}

async fn exclusions(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    definition: &MetricDefinition,
) -> Result<Vec<MetricExclusion>, sqlx::Error> {
    let basis = match definition.params.period_basis {
        PeriodBasis::WorkPublishedCohort => "work_published_cohort",
        PeriodBasis::CommentPublished => "comment_published",
    };
    let rows = sqlx::query(
        "SELECT reason, comments FROM linggan_opportunity_eligibility_exclusions( \
         $1,$2::timestamptz,$3,$4) ORDER BY reason",
    )
    .bind(domain)
    .bind(as_of)
    .bind(&definition.params.period_tz)
    .bind(basis)
    .fetch_all(&mut **tx)
    .await?;
    let mut counts = std::collections::BTreeMap::new();
    for row in rows {
        counts.insert(
            row.try_get::<String, _>("reason")?,
            row.try_get::<i64, _>("comments")?,
        );
    }
    // This is a closed, fully evaluated set of exclusion reasons, so an absent group is zero.
    for reason in [
        "not_in_domain_primary",
        "work_author_unknown",
        "period_anchor_missing",
        "comment_time_unknown",
        "observed_at_unparseable",
        "restricted",
        "creator_self",
    ] {
        counts.entry(reason.to_owned()).or_insert(0);
    }
    Ok(counts
        .into_iter()
        .map(|(reason, comments)| MetricExclusion {
            metric_def_version: definition.version,
            reason,
            comments,
        })
        .collect())
}

async fn identity_conflicts(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    definition: &MetricDefinition,
) -> Result<MetricIdentityConflicts, sqlx::Error> {
    let basis = match definition.params.period_basis {
        PeriodBasis::WorkPublishedCohort => "work_published_cohort",
        PeriodBasis::CommentPublished => "comment_published",
    };
    let comments = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_comment_eligibility($1,$2::timestamptz,$3,$4) candidate
         JOIN linggan_opportunity_comment_fact_at($2::timestamptz) fact
          USING (content_ref, comment_external_id)
         WHERE fact.commenter_external_id_conflict",
    ).bind(domain).bind(as_of).bind(&definition.params.period_tz).bind(basis)
     .fetch_one(&mut **tx).await?;
    Ok(MetricIdentityConflicts {
        metric_def_version: definition.version,
        comments,
    })
}

async fn validate_request(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    mode: RebuildMode,
) -> Result<(), MetricCalculationError> {
    let in_future: bool = sqlx::query_scalar("SELECT $1::timestamptz > statement_timestamp()")
        .bind(as_of)
        .fetch_one(&mut **tx)
        .await?;
    if in_future {
        return Err(MetricCalculationError::FutureAsOf);
    }
    let locked: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended('opportunity-metrics:' || $1::text, 0))",
    )
    .bind(domain)
    .fetch_one(&mut **tx)
    .await?;
    if !locked {
        return Err(MetricCalculationError::Busy);
    }
    let newer_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_opportunity_problem_metric_period metric
         JOIN linggan_opportunity_problem problem USING(problem_ref)
         WHERE problem.domain_ref=$1 AND metric.computed_as_of > $2::timestamptz)",
    )
    .bind(domain)
    .bind(as_of)
    .fetch_one(&mut **tx)
    .await?;
    if newer_exists && mode != RebuildMode::Preview {
        return Err(MetricCalculationError::NewerResultExists);
    }
    Ok(())
}

async fn problem_scope(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
) -> Result<Option<(Vec<Uuid>, i64)>, MetricCalculationError> {
    let problems: Vec<Uuid> = sqlx::query_scalar(
        "SELECT problem_ref FROM linggan_opportunity_problem WHERE domain_ref=$1
         AND created_at <= $2::timestamptz ORDER BY problem_ref",
    )
    .bind(domain)
    .bind(as_of)
    .fetch_all(&mut **tx)
    .await?;
    let source_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_problem_definition
         WHERE domain_ref=$1 AND created_at <= $2::timestamptz)",
    )
    .bind(domain)
    .bind(as_of)
    .fetch_one(&mut **tx)
    .await?;
    if problems.is_empty() && !source_exists {
        return Ok(None);
    }
    let taxonomy_version: i64 = sqlx::query_scalar(
        "SELECT version FROM linggan_opportunity_taxonomy_version_history
         WHERE domain_ref=$1 AND created_at <= $2::timestamptz
         ORDER BY created_at DESC,version DESC LIMIT 1",
    )
    .bind(domain)
    .bind(as_of)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MetricCalculationError::TaxonomyUnavailable)?;
    let missing_problem: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_problem_definition source
         WHERE source.domain_ref=$1 AND source.created_at <= $2::timestamptz
           AND NOT EXISTS (SELECT 1 FROM linggan_opportunity_problem problem
             WHERE problem.source_ref=source.problem_ref AND problem.created_at <= $2::timestamptz))",
    ).bind(domain).bind(as_of).fetch_one(&mut **tx).await?;
    if missing_problem {
        return Err(MetricCalculationError::ProblemRegistrationUnavailable);
    }
    Ok(Some((problems, taxonomy_version)))
}

/// Rebuild one domain in a consistent transaction. The caller enforces the feature switch and key.
pub async fn rebuild_problem_metrics(
    database: &Database,
    domain: Uuid,
    as_of: &str,
    digest: &CommenterDigest,
    mode: RebuildMode,
) -> Result<MetricRebuildReport, MetricCalculationError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    validate_request(&mut tx, domain, as_of, mode).await?;
    let definitions = definitions(&mut tx, as_of).await?;
    super::mentions::verify_digest_key(&mut tx, digest, mode != RebuildMode::Preview).await?;
    super::mentions::verify_existing_digest(&mut tx, digest, Some(domain)).await?;
    let mut report = MetricRebuildReport {
        definitions: definitions.len(),
        problems_rebuilt: 0,
        problems_unchanged: 0,
        period_rows_written: 0,
        problems_previewed: 0,
        preview: Vec::new(),
        exclusions: Vec::new(),
        identity_conflicts: Vec::new(),
        invariants: None,
    };
    let Some((problems, taxonomy_version)) = problem_scope(&mut tx, domain, as_of).await? else {
        return finish(tx, report, as_of, domain).await;
    };
    for definition in &definitions {
        report
            .exclusions
            .extend(exclusions(&mut tx, domain, as_of, definition).await?);
        report
            .identity_conflicts
            .push(identity_conflicts(&mut tx, domain, as_of, definition).await?);
        let periods = periods(&mut tx, as_of, definition).await?;
        let inputs =
            inputs::read_inputs(&mut tx, domain, as_of, definition, taxonomy_version, digest)
                .await?;
        let output_periods = definition
            .params
            .series_periods
            .checked_add(definition.params.baseline_periods)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(MetricCalculationError::CountOverflow)?;
        let expected_rows = inputs
            .denominators
            .len()
            .checked_mul(output_periods)
            .ok_or(MetricCalculationError::CountOverflow)?;
        for problem in &problems {
            let fingerprint =
                calculate::fingerprint(*problem, taxonomy_version, definition, &periods, &inputs)?;
            if mode == RebuildMode::Incremental
                && store::unchanged(
                    &mut tx,
                    *problem,
                    taxonomy_version,
                    definition.version,
                    as_of,
                    &fingerprint,
                    expected_rows,
                )
                .await?
            {
                report.problems_unchanged += 1;
                continue;
            }
            let calculated = calculate::calculate(
                *problem,
                taxonomy_version,
                fingerprint,
                definition,
                &periods,
                &inputs,
            )?;
            if mode == RebuildMode::Preview {
                report.problems_previewed += 1;
                report.preview.extend(calculated.rows);
                continue;
            }
            store::save(
                &mut tx,
                &calculated,
                taxonomy_version,
                definition.version,
                as_of,
            )
            .await?;
            report.problems_rebuilt += 1;
            report.period_rows_written += calculated.rows.len();
        }
    }
    finish(tx, report, as_of, domain).await
}

async fn finish(
    mut tx: Transaction<'_, Postgres>,
    mut report: MetricRebuildReport,
    as_of: &str,
    domain: Uuid,
) -> Result<MetricRebuildReport, MetricCalculationError> {
    let invariants = super::invariants::check_invariants_in(&mut tx, as_of, domain).await?;
    if invariants.failed() {
        let checks = invariants
            .checks
            .iter()
            .filter(|check| check.available && check.violations > 0)
            .map(|check| check.id)
            .collect();
        return Err(MetricCalculationError::InvariantViolations { checks });
    }
    report.invariants = Some(invariants);
    tx.commit().await?;
    Ok(report)
}
