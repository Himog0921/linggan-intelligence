//! Debounced, source-safe opportunity cards built from the shared metric calculation.
mod context;
mod groups;
mod model;
mod voices;

use super::{
    CommenterDigest, MetricCalculationError, MetricExclusion, OpportunityMetricError,
    OpportunityMetricParams, RebuildMode,
};
use crate::comment_study_source::StudySourceError;
use linggan_storage_postgres::Database;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Connection, Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum CardError {
    #[error("opportunity_card_database_unavailable")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Metrics(#[from] MetricCalculationError),
    #[error("{0}")]
    Definition(#[from] OpportunityMetricError),
    #[error("{0}")]
    Source(#[from] StudySourceError),
    #[error("opportunity_card_serialization_failed")]
    Serialization(#[from] serde_json::Error),
    #[error("opportunity_card_metrics_missing")]
    MissingMetrics,
    #[error("opportunity_card_metrics_invalid")]
    InvalidMetrics,
    #[error("opportunity_card_engagement_reference_invalid")]
    InvalidEngagementReference,
    #[error("opportunity_card_newer_result_exists")]
    NewerResultExists,
    #[error("opportunity_card_source_events_pending")]
    SourceEventsPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardRebuildMode {
    /// Process only changes whose debounce interval has elapsed.
    Due,
    /// Explicit operator rebuild of all problems in this domain.
    Full,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardRebuildReport {
    pub as_of: String,
    pub cards_built: usize,
    pub changes_deferred: usize,
    pub voices_excluded: usize,
    pub voice_exclusion_reasons: BTreeMap<String, usize>,
}

/// Recalculate and publish cards for one domain using a consistent current snapshot.
/// The caller enforces the feature switch and supplies the validated digest key.
pub async fn rebuild_opportunity_cards(
    database: &Database,
    domain: Uuid,
    digest: &CommenterDigest,
    mode: CardRebuildMode,
) -> Result<CardRebuildReport, CardError> {
    if mode == CardRebuildMode::Due && !enqueue_source_events(database).await? {
        return Err(CardError::SourceEventsPending);
    }
    // A session lock must precede the repeatable-read snapshot. Closing this checked-out
    // connection on every exit prevents lock leakage without exceeding the pool limit.
    let mut connection = database.pool().acquire().await?;
    connection.close_on_drop();
    sqlx::query(
        "SELECT pg_advisory_lock_shared(linggan_opportunity_card_restriction_lock_key($1))",
    )
    .bind(domain)
    .execute(&mut *connection)
    .await?;
    let mut tx = connection.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    let as_of: String = sqlx::query_scalar(
        "SELECT to_char(scope_001_now() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(&mut *tx)
    .await?;
    let (version, params) = current_definition(&mut tx, &as_of).await?;

    let (pending, changes_deferred) =
        pending_changes(&mut tx, domain, &as_of, mode, &params).await?;
    let mut report = CardRebuildReport {
        as_of: as_of.clone(),
        cards_built: 0,
        changes_deferred,
        voices_excluded: 0,
        voice_exclusion_reasons: BTreeMap::new(),
    };
    if pending.is_empty() {
        tx.commit().await?;
        connection.close().await?;
        return Ok(report);
    }
    let metric_report = super::problem_metrics::rebuild_problem_metrics_in(
        &mut tx,
        domain,
        &as_of,
        digest,
        RebuildMode::Incremental,
    )
    .await?;
    let taxonomy: i64 =
        sqlx::query_scalar("SELECT linggan_opportunity_taxonomy_version_at($1,$2::timestamptz)")
            .bind(domain)
            .bind(&as_of)
            .fetch_one(&mut *tx)
            .await?;
    let inputs = CardInputs {
        domain,
        taxonomy,
        version,
        as_of: &as_of,
        params: &params,
        metric_exclusions: &metric_report.exclusions,
    };
    for pending_row in pending {
        let problem: Uuid = pending_row.try_get("problem_ref")?;
        let current_revision: i64 = sqlx::query_scalar(
            "SELECT revision FROM linggan_opportunity_card_change WHERE problem_ref=$1",
        )
        .bind(problem)
        .fetch_one(&mut *tx)
        .await?;
        let (card, excluded) = assemble_card(&mut tx, problem, &inputs).await?;
        save(&mut tx, problem, taxonomy, version, &as_of, &card).await?;
        sqlx::query(
            "DELETE FROM linggan_opportunity_card_change WHERE problem_ref=$1 AND revision=$2",
        )
        .bind(problem)
        .bind(current_revision)
        .execute(&mut *tx)
        .await?;
        report.cards_built += 1;
        for (reason, count) in excluded {
            report.voices_excluded += count;
            *report.voice_exclusion_reasons.entry(reason).or_default() += count;
        }
    }
    let invariants = super::invariants::check_invariants_in(&mut tx, &as_of, domain).await?;
    if invariants.failed() {
        return Err(MetricCalculationError::InvariantViolations {
            checks: invariants
                .checks
                .iter()
                .filter(|check| check.available && check.violations > 0)
                .map(|check| check.id)
                .collect(),
        }
        .into());
    }
    tx.commit().await?;
    connection.close().await?;
    Ok(report)
}

async fn current_definition(
    tx: &mut Transaction<'_, Postgres>,
    as_of: &str,
) -> Result<(i32, OpportunityMetricParams), CardError> {
    let definition = sqlx::query(
        "SELECT d.version,d.params FROM linggan_opportunity_metric_definition_current c \
         JOIN linggan_opportunity_metric_definition d ON d.version=c.version \
         WHERE c.singleton AND c.switched_at<=$1::timestamptz AND d.created_at<=$1::timestamptz",
    )
    .bind(as_of)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(OpportunityMetricError::Missing)?;
    Ok((
        definition.try_get("version")?,
        super::metric_definition::parse_params(definition.try_get("params")?)?,
    ))
}

async fn pending_changes(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    mode: CardRebuildMode,
    params: &OpportunityMetricParams,
) -> Result<(Vec<sqlx::postgres::PgRow>, usize), CardError> {
    if mode == CardRebuildMode::Full {
        sqlx::query(
            "INSERT INTO linggan_opportunity_card_change(problem_ref) \
             SELECT problem_ref FROM linggan_opportunity_problem WHERE domain_ref=$1 AND created_at<=$2::timestamptz \
             ON CONFLICT(problem_ref) DO NOTHING",
        ).bind(domain).bind(as_of).execute(&mut **tx).await?;
    }
    let pending = sqlx::query(
        "SELECT change.problem_ref,change.revision FROM linggan_opportunity_card_change change \
         JOIN linggan_opportunity_problem problem USING(problem_ref) WHERE problem.domain_ref=$1 \
         AND problem.created_at<=$2::timestamptz \
         AND ($3 OR change.changed_at+make_interval(secs=>$4)<=clock_timestamp()) \
         ORDER BY change.problem_ref FOR UPDATE OF change",
    )
    .bind(domain)
    .bind(as_of)
    .bind(mode == CardRebuildMode::Full)
    .bind(f64::from(params.card_rebuild_debounce_seconds))
    .fetch_all(&mut **tx)
    .await?;
    let deferred: i64 = if mode == CardRebuildMode::Due {
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_change change \
            JOIN linggan_opportunity_problem problem USING(problem_ref) WHERE problem.domain_ref=$1 \
              AND change.changed_at+make_interval(secs=>$2)>clock_timestamp()")
            .bind(domain).bind(f64::from(params.card_rebuild_debounce_seconds))
            .fetch_one(&mut **tx).await?
    } else {
        0
    };
    Ok((
        pending,
        usize::try_from(deferred).map_err(|_| CardError::InvalidMetrics)?,
    ))
}

struct CardInputs<'a> {
    domain: Uuid,
    taxonomy: i64,
    version: i32,
    as_of: &'a str,
    params: &'a OpportunityMetricParams,
    metric_exclusions: &'a [MetricExclusion],
}

async fn assemble_card(
    tx: &mut Transaction<'_, Postgres>,
    problem: Uuid,
    inputs: &CardInputs<'_>,
) -> Result<(model::Card, BTreeMap<String, usize>), CardError> {
    let CardInputs {
        domain,
        taxonomy,
        version,
        as_of,
        params,
        metric_exclusions,
    } = *inputs;
    let row = sqlx::query(
        "SELECT label,level,parent_id FROM linggan_opportunity_problem WHERE problem_ref=$1",
    )
    .bind(problem)
    .fetch_one(&mut **tx)
    .await?;
    let demand_groups = groups::demand(tx, problem, taxonomy, version, as_of, params).await?;
    let creator_groups =
        groups::creators(tx, domain, problem, taxonomy, version, as_of, params).await?;
    let (sample_voices, excluded) = voices::select(
        tx,
        domain,
        problem,
        taxonomy,
        as_of,
        &params.period_tz,
        context::basis(params),
    )
    .await?;
    let (exclusions, flags) = context::exclusions_and_flags(
        tx,
        domain,
        as_of,
        version,
        params,
        metric_exclusions,
        &demand_groups,
    )
    .await?;
    let limitations = context::limitations(tx, domain, as_of, params).await?;
    let card = model::Card {
        problem: model::Problem {
            id: problem,
            statement: row.try_get("label")?,
            level: row.try_get("level")?,
            parent_id: row.try_get("parent_id")?,
            redirected_from: None,
        },
        as_of: as_of.to_owned(),
        versions: model::Versions {
            taxonomy,
            metric_def: version,
            judgment_prompt: None,
            model: None,
        },
        demand: model::Demand {
            groups: demand_groups,
            sample_voices,
            user_methods: model::UnavailableMethods {
                status: "unavailable",
                reason: "problem_method_association_unavailable",
            },
            importance: model::NotCalibrated {
                status: "not_calibrated",
            },
        },
        supply: model::Supply {
            state: "not_evaluated",
            judgment_status: None,
            coverage: None,
            counts: None,
            works: Vec::new(),
        },
        channel: model::Channel {
            window_days: params.creator_window_days,
            min_n: params.creator_min_n,
            groups: creator_groups,
        },
        flags,
        exclusions,
        limitations,
    };
    Ok((card, excluded))
}

/// Transfer durable source events to per-problem debounce markers outside source transactions.
// Bound each transaction independently of domain metric thresholds.
const SOURCE_EVENT_BATCH_SIZE: i64 = 1_024;

pub(crate) async fn enqueue_source_events(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query(include_str!("card/source_events.sql"))
        .bind(SOURCE_EVENT_BATCH_SIZE)
        .execute(database.pool())
        .await?;
    let remaining: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_opportunity_card_source_event)")
            .fetch_one(database.pool())
            .await?;
    Ok(!remaining)
}

async fn save(
    tx: &mut Transaction<'_, Postgres>,
    problem: Uuid,
    taxonomy: i64,
    version: i32,
    as_of: &str,
    card: &model::Card,
) -> Result<(), CardError> {
    let payload = serde_json::to_value(card)?;
    let fingerprint: String = Sha256::digest(serde_json::to_vec(&payload)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let result = sqlx::query("INSERT INTO linggan_opportunity_opportunity_card(problem_ref,payload,as_of,taxonomy_version,metric_def_version,input_fingerprint) \
        VALUES($1,$2,$3::timestamptz,$4,$5,$6) ON CONFLICT(problem_ref) DO UPDATE SET \
        payload=EXCLUDED.payload,as_of=EXCLUDED.as_of,built_at=clock_timestamp(),taxonomy_version=EXCLUDED.taxonomy_version,\
        metric_def_version=EXCLUDED.metric_def_version,input_fingerprint=EXCLUDED.input_fingerprint \
        WHERE linggan_opportunity_opportunity_card.as_of<=EXCLUDED.as_of")
        .bind(problem).bind(payload).bind(as_of).bind(taxonomy).bind(version).bind(fingerprint)
        .execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(CardError::NewerResultExists);
    }
    Ok(())
}
