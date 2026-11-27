//! Debounced, source-safe opportunity cards built from the shared metric calculation.
mod context;
mod groups;
mod model;
mod read;
mod read_model;
mod snapshot;
mod voices;

use super::{
    CommenterDigest, MentionSyncError, MetricCalculationError, MetricExclusion,
    OpportunityMetricDefinition, OpportunityMetricError, OpportunityMetricParams, RebuildMode,
};
use crate::comment_study_source::StudySourceError;
use linggan_storage_postgres::Database;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub use read::{
    EvidenceKind, EvidenceQuery, OpportunityListQuery, OpportunityReadError, read_card,
    read_card_in_domain, read_evidence, read_evidence_in_domain, read_opportunities,
};

#[derive(Debug, thiserror::Error)]
pub enum CardError {
    #[error("opportunity_card_database_unavailable")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Metrics(#[from] MetricCalculationError),
    #[error("{0}")]
    Definition(#[from] OpportunityMetricError),
    #[error("{0}")]
    MentionSync(#[from] MentionSyncError),
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
    #[error("opportunity_card_sources_busy")]
    SourcesBusy,
    #[error("opportunity_card_source_lock_permission_missing")]
    SourceLockPermission,
    #[error("opportunity_card_restriction_visibility_unavailable")]
    RestrictionVisibility,
    #[error("opportunity_card_source_coverage_missing: {0:?}")]
    SourceCoverage(Vec<String>),
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
    pub flag_counts: BTreeMap<String, usize>,
    pub voice_exclusion_reasons: BTreeMap<String, usize>,
    /// Counts observation rows in this domain snapshot, not distinct comments or creators.
    /// None means the rebuild had no due cards and did not evaluate these diagnostics.
    pub unparseable_observations: Option<UnparseableObservations>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnparseableObservations {
    pub comments: i64,
    pub creator_profiles: i64,
    pub work_details: i64,
    pub work_discoveries: i64,
    pub work_lanes: i64,
}

/// Recalculate and publish cards for one domain using a consistent current snapshot.
/// The caller enforces the feature switch and supplies the validated digest key.
pub async fn rebuild_opportunity_cards(
    database: &Database,
    domain: Uuid,
    digest: &CommenterDigest,
    mode: CardRebuildMode,
) -> Result<CardRebuildReport, CardError> {
    if mode == CardRebuildMode::Due {
        enqueue_source_events(database).await?;
    }
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    let (as_of, current_observation_proven) = snapshot::capture(&mut tx).await?;
    snapshot::require_visible_restrictions(&mut tx).await?;
    let (version, params) = current_definition(&mut tx, &as_of).await?;
    let definition = OpportunityMetricDefinition { version, params };
    // The membership projection and card calculation must observe the same snapshot.
    super::mentions::sync_comment_mentions_in(&mut tx, digest, Some(domain), &definition).await?;
    let params = &definition.params;
    let (pending, changes_deferred) =
        pending_changes(&mut tx, domain, &as_of, mode, params).await?;
    let mut report = CardRebuildReport {
        as_of: as_of.clone(),
        cards_built: 0,
        changes_deferred,
        voices_excluded: 0,
        flag_counts: BTreeMap::new(),
        voice_exclusion_reasons: BTreeMap::new(),
        unparseable_observations: None,
    };
    if pending.is_empty() {
        if mode == CardRebuildMode::Full {
            super::problem_metrics::problem_scope(&mut tx, domain, &as_of).await?;
        }
        snapshot::verify(&mut tx).await?;
        tx.commit().await?;
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
    report.unparseable_observations =
        Some(unparseable_observations(&mut tx, domain, &as_of, version).await?);
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
        current_observation_proven,
        params,
        metric_exclusions: &metric_report.exclusions,
    };
    for pending_row in pending {
        store_pending_card(
            &mut tx,
            pending_row.try_get("problem_ref")?,
            &inputs,
            &mut report,
        )
        .await?;
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
    snapshot::verify(&mut tx).await?;
    tx.commit().await?;
    Ok(report)
}

async fn store_pending_card(
    tx: &mut Transaction<'_, Postgres>,
    problem: Uuid,
    inputs: &CardInputs<'_>,
    report: &mut CardRebuildReport,
) -> Result<(), CardError> {
    let current_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_opportunity_card_change WHERE problem_ref=$1",
    )
    .bind(problem)
    .fetch_one(&mut **tx)
    .await?;
    let (card, excluded) = assemble_card(tx, problem, inputs).await?;
    let flags: std::collections::BTreeSet<&str> = card
        .flags
        .iter()
        .chain(
            card.demand
                .groups
                .iter()
                .flat_map(|group| group.flags.iter()),
        )
        .copied()
        .collect();
    for flag in flags {
        *report.flag_counts.entry(flag.to_owned()).or_default() += 1;
    }
    touch_sources(tx, problem, &card).await?;
    save(
        tx,
        problem,
        inputs.taxonomy,
        inputs.version,
        inputs.as_of,
        &card,
    )
    .await?;
    sqlx::query("DELETE FROM linggan_opportunity_card_change WHERE problem_ref=$1 AND revision=$2")
        .bind(problem)
        .bind(current_revision)
        .execute(&mut **tx)
        .await?;
    report.cards_built += 1;
    for (reason, count) in excluded {
        report.voices_excluded += count;
        *report.voice_exclusion_reasons.entry(reason).or_default() += count;
    }
    Ok(())
}

async fn unparseable_observations(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    version: i32,
) -> Result<UnparseableObservations, sqlx::Error> {
    let (comments, creator_profiles, work_details, work_discoveries, work_lanes): (
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        "WITH domain_works AS MATERIALIZED (SELECT DISTINCT usage.content_public_ref \
           FROM linggan_material_domain_usage usage WHERE usage.domain_ref=$1 \
             AND (usage.basis_kind='legacy_domain_migration' OR usage.created_at<=$2::timestamptz)) \
         SELECT \
           (SELECT COALESCE(sum(fact.unparseable_observations),0)::bigint \
            FROM linggan_opportunity_comment_fact_at($2::timestamptz) fact \
            JOIN domain_works work ON work.content_public_ref=fact.content_ref), \
           (SELECT count(*) FROM linggan_material_author_profile profile \
            JOIN linggan_runtime_capture_package package USING(package_ref) \
            WHERE profile.platform='xhs' AND profile.created_at<=$2::timestamptz \
              AND package.accepted_at<=$2::timestamptz \
              AND linggan_opportunity_observed_at(profile.observed_at) IS NULL \
              AND EXISTS (SELECT 1 FROM linggan_opportunity_creator_problem_density density \
                JOIN linggan_opportunity_problem problem USING(problem_ref) \
                WHERE problem.domain_ref=$1 AND density.creator_id=profile.author_external_id \
                  AND density.metric_def_version=$3 AND density.computed_as_of=$2::timestamptz)), \
           (SELECT count(*) FROM linggan_material_content_detail detail \
            JOIN domain_works work USING(content_public_ref) JOIN linggan_runtime_capture_package package USING(package_ref) \
            WHERE detail.created_at<=$2::timestamptz AND package.accepted_at<=$2::timestamptz \
              AND linggan_opportunity_observed_at(detail.observed_at) IS NULL), \
           (SELECT count(*) FROM linggan_material_discovery_finding finding \
            JOIN domain_works work USING(content_public_ref) JOIN linggan_runtime_capture_package package USING(package_ref) \
            WHERE finding.created_at<=$2::timestamptz AND package.accepted_at<=$2::timestamptz \
              AND linggan_opportunity_observed_at(finding.observed_at) IS NULL), \
           (SELECT count(*) FROM linggan_material_lane_observation lane \
            JOIN domain_works work USING(content_public_ref) JOIN linggan_runtime_capture_package package USING(package_ref) \
            WHERE lane.created_at<=$2::timestamptz AND package.accepted_at<=$2::timestamptz \
              AND linggan_opportunity_observed_at(lane.observed_at) IS NULL)",
    )
    .bind(domain)
    .bind(as_of)
    .bind(version)
    .fetch_one(&mut **tx)
    .await?;
    Ok(UnparseableObservations {
        comments,
        creator_profiles,
        work_details,
        work_discoveries,
        work_lanes,
    })
}

async fn touch_sources(
    tx: &mut Transaction<'_, Postgres>,
    problem: Uuid,
    card: &model::Card,
) -> Result<(), sqlx::Error> {
    // Lock both old and replacement voices before changing a card or its reverse index.
    let old: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT content_ref,comment_external_id FROM linggan_opportunity_card_voice_ref WHERE problem_ref=$1",
    )
    .bind(problem)
    .fetch_all(&mut **tx)
    .await?;
    let mut sources: BTreeSet<_> = old.into_iter().collect();
    sources.extend(card.demand.sample_voices.iter().map(|voice| {
        (
            voice.comment_ref.content_ref,
            voice.comment_ref.comment_external_id.clone(),
        )
    }));
    for (content, comment) in sources {
        sqlx::query("SELECT linggan_opportunity_card_touch_source($1,$2)")
            .bind(content)
            .bind(&comment)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
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

async fn enqueue_period_index_repairs_in(
    connection: &mut PgConnection,
    domain: Option<Uuid>,
    as_of: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO linggan_opportunity_card_change(problem_ref) \
         SELECT c.problem_ref FROM linggan_opportunity_opportunity_card c \
         JOIN linggan_opportunity_problem p USING(problem_ref) \
         LEFT JOIN linggan_opportunity_card_period_index i ON i.problem_ref=c.problem_ref \
           AND i.card_fingerprint=c.input_fingerprint AND i.taxonomy_version=c.taxonomy_version \
           AND i.metric_def_version=c.metric_def_version \
         WHERE ($1::uuid IS NULL OR p.domain_ref=$1) \
           AND p.created_at<=COALESCE($2::timestamptz,scope_001_now()) AND i.problem_ref IS NULL \
         ON CONFLICT(problem_ref) DO NOTHING",
    )
    .bind(domain)
    .bind(as_of)
    .execute(connection)
    .await?;
    Ok(())
}

/// Rediscover indexes invalidated by an older executor after its startup catch-up.
pub(crate) async fn enqueue_period_index_repairs(database: &Database) -> Result<(), sqlx::Error> {
    let mut connection = database.pool().acquire().await?;
    enqueue_period_index_repairs_in(&mut connection, None, None).await
}

async fn pending_changes(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    mode: CardRebuildMode,
    params: &OpportunityMetricParams,
) -> Result<(Vec<sqlx::postgres::PgRow>, usize), CardError> {
    // The executor and explicit rebuilds use the same missing-index detector.
    enqueue_period_index_repairs_in(tx, Some(domain), Some(as_of)).await?;
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
         AND ($3 OR LEAST(change.queued_at,change.changed_at)+make_interval(secs=>$4)<=clock_timestamp()) \
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
              AND LEAST(change.queued_at,change.changed_at)+make_interval(secs=>$2)>clock_timestamp()")
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
    current_observation_proven: bool,
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
        current_observation_proven: _,
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
    let creator_groups = groups::creators(tx, problem, inputs).await?;
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
    let period_index: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT jsonb_agg(jsonb_build_object('periodStart',period_start::text, \
         'protocolVersion',protocol_version,'evidenceTier',evidence_tier) \
         ORDER BY period_start,protocol_version NULLS LAST) \
         FROM linggan_opportunity_problem_metric_period WHERE problem_ref=$1 \
         AND taxonomy_version=$2 AND metric_def_version=$3 AND computed_as_of=$4::timestamptz",
    )
    .bind(problem)
    .bind(taxonomy)
    .bind(version)
    .bind(as_of)
    .fetch_one(&mut **tx)
    .await?;
    let period_index = period_index.ok_or(CardError::MissingMetrics)?;
    let fingerprint: String = Sha256::digest(serde_json::to_vec(&(&payload, &period_index))?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let result = sqlx::query("INSERT INTO linggan_opportunity_opportunity_card(problem_ref,payload,as_of,taxonomy_version,metric_def_version,input_fingerprint) \
        VALUES($1,$2,$3::timestamptz,$4,$5,$6) ON CONFLICT(problem_ref) DO UPDATE SET \
        payload=EXCLUDED.payload,as_of=EXCLUDED.as_of,built_at=clock_timestamp(),taxonomy_version=EXCLUDED.taxonomy_version,\
        metric_def_version=EXCLUDED.metric_def_version,input_fingerprint=EXCLUDED.input_fingerprint \
        WHERE linggan_opportunity_opportunity_card.as_of<=EXCLUDED.as_of")
        .bind(problem).bind(payload).bind(as_of).bind(taxonomy).bind(version).bind(&fingerprint)
        .execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(CardError::NewerResultExists);
    }
    sqlx::query("INSERT INTO linggan_opportunity_card_period_index(problem_ref,taxonomy_version,metric_def_version,card_fingerprint,period_index) \
        VALUES($1,$2,$3,$4,$5) ON CONFLICT(problem_ref) DO UPDATE SET \
        taxonomy_version=EXCLUDED.taxonomy_version,metric_def_version=EXCLUDED.metric_def_version,\
        card_fingerprint=EXCLUDED.card_fingerprint,period_index=EXCLUDED.period_index")
        .bind(problem).bind(taxonomy).bind(version).bind(fingerprint).bind(period_index)
        .execute(&mut **tx).await?;
    Ok(())
}
