//! Project the existing comment-study memberships into stable, pseudonymous mentions.
//! No model is invoked and no commenter identifier is returned in a report or error.

use super::{
    CommenterDigest, EvidenceTier, OpportunityMetricError, OpportunityMetricParams, PeriodBasis,
    TierInput, TierParams, evidence_tier, load_current_metric_definition,
};
use crate::ai_foundation::{comment_problem, store::FoundationError};
use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum MentionSyncError {
    #[error("opportunity_mention_database_unavailable")]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Metric(#[from] OpportunityMetricError),
    #[error("opportunity_metric_definition_changed")]
    MetricChanged,
    #[error("opportunity_mention_source_fact_missing: {rows} rows")]
    SourceFactMissing { rows: usize },
    #[error("opportunity_mention_source_provenance_missing: {rows} rows")]
    SourceProvenanceMissing { rows: usize },
    #[error("opportunity_mention_digest_version_mismatch: {rows} rows")]
    DigestVersionMismatch { rows: usize },
    #[error("opportunity_mention_digest_key_or_source_mismatch: {rows} rows")]
    DigestKeyOrSourceMismatch { rows: usize },
    #[error("opportunity_mention_migration_ledger_missing")]
    MigrationLedgerMissing,
    #[error("opportunity_mention_sync_busy")]
    SyncBusy,
    #[error("opportunity_mention_source_scope_mismatch: {rows} rows")]
    SourceScopeMismatch { rows: usize },
    #[error("opportunity_mention_publication_check_failed")]
    Publication(#[from] FoundationError),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MentionProblemReconciliation {
    pub problem_ref: Uuid,
    pub taxonomy_version: i64,
    pub source_mentioners: usize,
    pub projected_mentioners: usize,
    pub source_only_mentioners: usize,
    pub projected_only_mentioners: usize,
    pub identity_conflicts: usize,
    pub reasons: Vec<&'static str>,
    pub currently_published: bool,
    pub eligible_mentioners: u64,
    pub published_below_two_eligible: bool,
    pub eligibility: Vec<MentionEligibilityCount>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MentionEligibilityCount {
    pub reason: String,
    pub comments: i64,
    pub mentioners: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MentionSyncReport {
    pub source_memberships: usize,
    pub unique_comment_problem_pairs: usize,
    pub inserted: u64,
    pub updated: u64,
    pub already_present: usize,
    pub unknown_commenters: usize,
    pub problems_synced: i64,
    pub reconciliations: Vec<MentionProblemReconciliation>,
}

impl MentionSyncReport {
    /// Count conflicting stable-comment/problem pairs even when the two source sets agree.
    pub fn identity_conflicts(&self) -> usize {
        self.reconciliations
            .iter()
            .map(|item| item.identity_conflicts)
            .sum()
    }
}

struct SourceMention {
    content_ref: Uuid,
    comment_external_id: String,
    problem_ref: Uuid,
    taxonomy_version: i64,
    source_material_ref: Uuid,
    request_ref: Uuid,
    signal_ordinal: i32,
    observed_author: Option<String>,
    canonical_author: Option<String>,
    identity_conflict: bool,
    missing_fact: bool,
    scope_mismatch: bool,
    prompt_version: Option<String>,
    model: Option<String>,
    content_hash: String,
    historical: bool,
}

async fn read_sources(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Vec<SourceMention>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT comment.content_public_ref, comment.comment_external_id, problem.problem_ref,
                version.version AS taxonomy_version, request.source_ref, membership.request_ref,
                membership.signal_ordinal, comment.author_external_id AS observed_author,
                fact.commenter_external_id AS canonical_author,
                fact.commenter_external_id_conflict AS identity_conflict,
                fact.content_ref IS NULL AS missing_fact,
                (request.domain_ref <> definition.domain_ref OR request.work_ref <> comment.content_public_ref)
                  AS scope_mismatch,
                config.prompt_version_ref::text AS prompt_version, model.model_id AS model,
                request.input_fingerprint AS content_hash,
                membership.created_at <= ledger.applied_at AS historical
         FROM linggan_comment_problem_membership membership
         JOIN linggan_comment_problem_definition definition USING (definition_ref)
         JOIN linggan_comment_research_request request USING (request_ref)
         JOIN linggan_material_comment comment ON comment.material_ref = request.source_ref
         JOIN linggan_opportunity_problem problem ON problem.source_ref = definition.problem_ref
         JOIN linggan_opportunity_taxonomy_version version ON version.domain_ref = problem.domain_ref
         LEFT JOIN linggan_opportunity_comment_fact fact
           ON fact.content_ref = comment.content_public_ref
          AND fact.comment_external_id = comment.comment_external_id
         LEFT JOIN linggan_model_invocation invocation
           ON invocation.invocation_ref = membership.review_invocation_ref
         LEFT JOIN linggan_ai_task_config config ON config.task_config_ref = invocation.task_config_ref
         LEFT JOIN linggan_model_entry model ON model.model_ref = invocation.model_ref
         CROSS JOIN linggan_local_schema_migration ledger
         WHERE ledger.migration_id = '0211_opportunity_comment_mentions'
         ORDER BY comment.content_public_ref, comment.comment_external_id, problem.problem_ref,
                  membership.created_at, membership.request_ref, membership.signal_ordinal",
    )
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(SourceMention {
                content_ref: row.try_get("content_public_ref")?,
                comment_external_id: row.try_get("comment_external_id")?,
                problem_ref: row.try_get("problem_ref")?,
                taxonomy_version: row.try_get("taxonomy_version")?,
                source_material_ref: row.try_get("source_ref")?,
                request_ref: row.try_get("request_ref")?,
                signal_ordinal: row.try_get("signal_ordinal")?,
                observed_author: row.try_get("observed_author")?,
                canonical_author: row.try_get("canonical_author")?,
                identity_conflict: row
                    .try_get::<Option<bool>, _>("identity_conflict")?
                    .unwrap_or(false),
                missing_fact: row.try_get("missing_fact")?,
                scope_mismatch: row.try_get("scope_mismatch")?,
                prompt_version: row.try_get("prompt_version")?,
                model: row.try_get("model")?,
                content_hash: row.try_get("content_hash")?,
                historical: row.try_get("historical")?,
            })
        })
        .collect()
}

#[derive(Default)]
struct ProblemCounts {
    source: BTreeSet<String>,
    projected: BTreeSet<String>,
    identity_conflicts: usize,
    currently_published: bool,
    eligibility: Vec<MentionEligibilityCount>,
}

type MentionKey = (Uuid, String, Uuid, i64);

struct ExistingDigest {
    hash: Option<String>,
    version: String,
    verified: bool,
}

/// An existing digest must still match at least one immutable observation of that comment.
/// This catches a changed key under the same version without storing another key fingerprint.
async fn verify_existing_digest(
    tx: &mut Transaction<'_, Postgres>,
    digest: &CommenterDigest,
) -> Result<HashSet<MentionKey>, MentionSyncError> {
    let rows = sqlx::query(
        "SELECT mention.content_ref, mention.comment_external_id, mention.problem_ref,
                mention.taxonomy_version, mention.commenter_hash, mention.commenter_digest_version,
                observation.author_external_id
         FROM linggan_opportunity_comment_mention mention
         LEFT JOIN linggan_material_comment observation
           ON observation.content_public_ref = mention.content_ref
          AND observation.comment_external_id = mention.comment_external_id",
    )
    .fetch_all(&mut **tx)
    .await?;
    let mut existing = BTreeMap::<MentionKey, ExistingDigest>::new();
    for row in rows {
        let key = (
            row.try_get("content_ref")?,
            row.try_get("comment_external_id")?,
            row.try_get("problem_ref")?,
            row.try_get("taxonomy_version")?,
        );
        let hash: Option<String> = row.try_get("commenter_hash")?;
        let candidate: Option<String> = row.try_get("author_external_id")?;
        let entry = existing.entry(key).or_insert(ExistingDigest {
            verified: hash.is_none(),
            hash,
            version: row.try_get("commenter_digest_version")?,
        });
        if entry.hash.is_some() && digest.hash(candidate.as_deref()) == entry.hash {
            entry.verified = true;
        }
    }
    let wrong_version = existing
        .values()
        .filter(|row| row.version != digest.digest_version())
        .count();
    if wrong_version != 0 {
        return Err(MentionSyncError::DigestVersionMismatch {
            rows: wrong_version,
        });
    }
    let unverifiable = existing.values().filter(|row| !row.verified).count();
    if unverifiable != 0 {
        return Err(MentionSyncError::DigestKeyOrSourceMismatch { rows: unverifiable });
    }
    Ok(existing.into_keys().collect())
}

async fn collect_eligibility(
    tx: &mut Transaction<'_, Postgres>,
    params: &OpportunityMetricParams,
    counts: &mut BTreeMap<(Uuid, i64), ProblemCounts>,
) -> Result<(), MentionSyncError> {
    let basis = match params.period_basis {
        PeriodBasis::WorkPublishedCohort => "work_published_cohort",
        PeriodBasis::CommentPublished => "comment_published",
    };
    let domains: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT domain_ref FROM linggan_opportunity_problem ORDER BY domain_ref",
    )
    .fetch_all(&mut **tx)
    .await?;
    let mut published = HashSet::new();
    for domain in domains {
        published.extend(comment_problem::qualified_problem_refs_in(tx, domain).await?);
    }
    let rows = sqlx::query(
        "WITH source AS MATERIALIZED (
           SELECT mention.*, problem.domain_ref, fact.first_observed_at
           FROM linggan_opportunity_comment_mention mention
           JOIN linggan_opportunity_problem problem USING (problem_ref)
           JOIN linggan_opportunity_taxonomy_version version ON version.domain_ref = problem.domain_ref
            AND version.version = mention.taxonomy_version
           JOIN linggan_opportunity_comment_fact fact
             ON fact.content_ref = mention.content_ref AND fact.comment_external_id = mention.comment_external_id
         ), eligible AS MATERIALIZED (
           SELECT domain.domain_ref, candidate.* FROM (SELECT DISTINCT domain_ref FROM source) domain
           CROSS JOIN LATERAL linggan_opportunity_comment_eligibility(
             domain.domain_ref, now(), $1, $2
           ) candidate
         ), classified AS (
           SELECT source.problem_ref, source.taxonomy_version, source.content_ref,
                  source.comment_external_id, source.commenter_hash,
                  CASE WHEN eligible.content_ref IS NULL AND source.first_observed_at > now()
                         THEN 'not_observed_as_of'
                       WHEN eligible.content_ref IS NULL THEN 'not_in_domain_primary'
                       WHEN eligible.exclusion_reason IS NOT NULL THEN eligible.exclusion_reason
                       WHEN source.excluded_reason IS NOT NULL THEN source.excluded_reason
                       ELSE 'eligible' END AS reason
           FROM source LEFT JOIN eligible ON eligible.domain_ref = source.domain_ref
             AND eligible.content_ref = source.content_ref
             AND eligible.comment_external_id = source.comment_external_id
         )
         SELECT problem_ref, taxonomy_version, reason, count(*)::bigint AS comments,
                count(DISTINCT commenter_hash)::bigint AS mentioners
         FROM classified GROUP BY problem_ref, taxonomy_version, reason
         ORDER BY problem_ref, taxonomy_version, reason",
    )
    .bind(&params.period_tz)
    .bind(basis)
    .fetch_all(&mut **tx)
    .await?;
    for row in rows {
        let problem_ref: Uuid = row.try_get("problem_ref")?;
        let key = (problem_ref, row.try_get("taxonomy_version")?);
        let entry = counts.entry(key).or_default();
        entry.currently_published = published.contains(&problem_ref);
        entry.eligibility.push(MentionEligibilityCount {
            reason: row.try_get("reason")?,
            comments: row.try_get("comments")?,
            mentioners: row.try_get("mentioners")?,
        });
    }
    Ok(())
}

async fn insert_mention(
    tx: &mut Transaction<'_, Postgres>,
    row: &SourceMention,
    digest: &CommenterDigest,
    metric_def_version: i32,
) -> Result<u64, sqlx::Error> {
    let hash = digest.hash(row.canonical_author.as_deref());
    let reason = hash.is_none().then_some("commenter_unknown");
    let result = sqlx::query(
        "INSERT INTO linggan_opportunity_comment_mention(
          content_ref, comment_external_id, problem_ref, taxonomy_version, metric_def_version,
          commenter_hash, commenter_digest_version, source_material_ref, source_request_ref,
          source_signal_ordinal, assigned_by, prompt_version, model, content_hash, excluded_reason
         ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
         ON CONFLICT (content_ref, comment_external_id, problem_ref, taxonomy_version) DO UPDATE
         SET metric_def_version = EXCLUDED.metric_def_version,
             commenter_hash = EXCLUDED.commenter_hash,
             commenter_digest_version = EXCLUDED.commenter_digest_version,
             source_material_ref = EXCLUDED.source_material_ref,
             source_request_ref = EXCLUDED.source_request_ref,
             source_signal_ordinal = EXCLUDED.source_signal_ordinal,
             assigned_by = EXCLUDED.assigned_by,
             prompt_version = EXCLUDED.prompt_version,
             model = EXCLUDED.model,
             content_hash = EXCLUDED.content_hash,
             excluded_reason = EXCLUDED.excluded_reason,
             updated_at = clock_timestamp(),
             hash_changed_at = CASE WHEN linggan_opportunity_comment_mention.commenter_hash
                 IS DISTINCT FROM EXCLUDED.commenter_hash THEN clock_timestamp()
                 ELSE linggan_opportunity_comment_mention.hash_changed_at END
         WHERE ROW(linggan_opportunity_comment_mention.metric_def_version,
                   linggan_opportunity_comment_mention.commenter_hash,
                   linggan_opportunity_comment_mention.commenter_digest_version,
                   linggan_opportunity_comment_mention.source_material_ref,
                   linggan_opportunity_comment_mention.source_request_ref,
                   linggan_opportunity_comment_mention.source_signal_ordinal,
                   linggan_opportunity_comment_mention.assigned_by,
                   linggan_opportunity_comment_mention.prompt_version,
                   linggan_opportunity_comment_mention.model,
                   linggan_opportunity_comment_mention.content_hash,
                   linggan_opportunity_comment_mention.excluded_reason)
          IS DISTINCT FROM ROW(EXCLUDED.metric_def_version, EXCLUDED.commenter_hash,
                   EXCLUDED.commenter_digest_version, EXCLUDED.source_material_ref,
                   EXCLUDED.source_request_ref, EXCLUDED.source_signal_ordinal,
                   EXCLUDED.assigned_by, EXCLUDED.prompt_version, EXCLUDED.model,
                   EXCLUDED.content_hash, EXCLUDED.excluded_reason)",
    )
    .bind(row.content_ref)
    .bind(&row.comment_external_id)
    .bind(row.problem_ref)
    .bind(row.taxonomy_version)
    .bind(metric_def_version)
    .bind(hash)
    .bind(digest.digest_version())
    .bind(row.source_material_ref)
    .bind(row.request_ref)
    .bind(row.signal_ordinal)
    .bind(if row.historical { "migrated" } else { "ai" })
    .bind(&row.prompt_version)
    .bind(&row.model)
    .bind(&row.content_hash)
    .bind(reason)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

async fn reconcile_projected(
    tx: &mut Transaction<'_, Postgres>,
    digest: &CommenterDigest,
    counts: &mut BTreeMap<(Uuid, i64), ProblemCounts>,
) -> Result<(), MentionSyncError> {
    let rows = sqlx::query(
        "SELECT mention.problem_ref, mention.taxonomy_version, mention.commenter_hash,
                mention.commenter_digest_version
         FROM linggan_opportunity_comment_mention mention
         JOIN linggan_opportunity_problem problem USING (problem_ref)
         JOIN linggan_opportunity_taxonomy_version version ON version.domain_ref = problem.domain_ref
          AND version.version = mention.taxonomy_version",
    )
    .fetch_all(&mut **tx)
    .await?;
    let mut mismatched = 0;
    for row in rows {
        let version: String = row.try_get("commenter_digest_version")?;
        if version != digest.digest_version() {
            mismatched += 1;
            continue;
        }
        let key = (
            row.try_get("problem_ref")?,
            row.try_get("taxonomy_version")?,
        );
        let entry = counts.entry(key).or_default();
        if let Some(hash) = row.try_get::<Option<String>, _>("commenter_hash")? {
            entry.projected.insert(hash);
        }
    }
    if mismatched != 0 {
        return Err(MentionSyncError::DigestVersionMismatch { rows: mismatched });
    }
    Ok(())
}

fn validate_sources(sources: &[SourceMention]) -> Result<(), MentionSyncError> {
    let missing_facts = sources.iter().filter(|row| row.missing_fact).count();
    if missing_facts != 0 {
        return Err(MentionSyncError::SourceFactMissing {
            rows: missing_facts,
        });
    }
    let scope_mismatches = sources.iter().filter(|row| row.scope_mismatch).count();
    if scope_mismatches != 0 {
        return Err(MentionSyncError::SourceScopeMismatch {
            rows: scope_mismatches,
        });
    }
    let missing_provenance = sources
        .iter()
        .filter(|row| {
            row.prompt_version.as_deref().is_none_or(str::is_empty)
                || row.model.as_deref().is_none_or(str::is_empty)
        })
        .count();
    if missing_provenance != 0 {
        return Err(MentionSyncError::SourceProvenanceMissing {
            rows: missing_provenance,
        });
    }
    Ok(())
}

#[derive(Default)]
struct Projection {
    counts: BTreeMap<(Uuid, i64), ProblemCounts>,
    unique_pairs: usize,
    inserted: u64,
    updated: u64,
    unknown_commenters: usize,
}

async fn project_sources(
    tx: &mut Transaction<'_, Postgres>,
    sources: &[SourceMention],
    digest: &CommenterDigest,
    metric_version: i32,
    existing: &HashSet<MentionKey>,
) -> Result<Projection, sqlx::Error> {
    let mut projection = Projection::default();
    let mut seen = HashSet::new();
    for row in sources {
        let counts_for_problem = projection
            .counts
            .entry((row.problem_ref, row.taxonomy_version))
            .or_default();
        if let Some(hash) = digest.hash(row.observed_author.as_deref()) {
            counts_for_problem.source.insert(hash);
        }
        let identity = (
            row.content_ref,
            row.comment_external_id.clone(),
            row.problem_ref,
            row.taxonomy_version,
        );
        let was_present = existing.contains(&identity);
        if !seen.insert(identity) {
            continue;
        }
        counts_for_problem.identity_conflicts += usize::from(row.identity_conflict);
        projection.unknown_commenters += usize::from(row.canonical_author.is_none());
        let changed = insert_mention(tx, row, digest, metric_version).await?;
        if was_present {
            projection.updated += changed;
        } else {
            projection.inserted += changed;
        }
    }
    projection.unique_pairs = seen.len();
    Ok(projection)
}

fn reconciliations(
    counts: BTreeMap<(Uuid, i64), ProblemCounts>,
    params: &OpportunityMetricParams,
) -> Vec<MentionProblemReconciliation> {
    counts
        .into_iter()
        .map(|((problem_ref, taxonomy_version), item)| {
            let source_only = item.source.difference(&item.projected).count();
            let projected_only = item.projected.difference(&item.source).count();
            let reasons = if source_only + projected_only == 0 {
                Vec::new()
            } else if item.identity_conflicts > 0 {
                vec!["commenter_identity_conflict"]
            } else {
                vec!["source_projection_mismatch"]
            };
            let eligible_mentioners = item
                .eligibility
                .iter()
                .find(|bucket| bucket.reason == "eligible")
                .map_or(0, |bucket| bucket.mentioners as u64);
            let tier = evidence_tier(
                &TierInput {
                    mentioners_total: eligible_mentioners,
                    k_cur: 0,
                    n_cur: 0,
                    k_base: 0,
                    n_base: 0,
                },
                &TierParams {
                    z: params.z,
                    min_k_significant: params.min_k_significant.into(),
                    min_n_period: params.min_n_period.into(),
                    min_total_for_significant: params.min_total_for_significant.into(),
                },
            );
            MentionProblemReconciliation {
                problem_ref,
                taxonomy_version,
                source_mentioners: item.source.len(),
                projected_mentioners: item.projected.len(),
                source_only_mentioners: source_only,
                projected_only_mentioners: projected_only,
                identity_conflicts: item.identity_conflicts,
                reasons,
                currently_published: item.currently_published,
                eligible_mentioners,
                published_below_two_eligible: item.currently_published
                    && tier == EvidenceTier::Candidate,
                eligibility: item.eligibility,
            }
        })
        .collect()
}

/// Replays existing 0177 memberships, or incrementally attaches newer memberships. A unique
/// database key makes concurrent/repeated runs idempotent. The caller must enforce D-006 and load
/// the production digest key before calling this function.
pub async fn sync_comment_mentions(
    database: &Database,
    digest: &CommenterDigest,
) -> Result<MentionSyncReport, MentionSyncError> {
    let metric = load_current_metric_definition(database).await?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    let locked: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended('opportunity-mention-sync', 0))",
    )
    .fetch_one(&mut *tx)
    .await?;
    if !locked {
        return Err(MentionSyncError::SyncBusy);
    }
    let actual_version: Option<i32> = sqlx::query_scalar(
        "SELECT version FROM linggan_opportunity_metric_definition_current WHERE singleton",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if actual_version != Some(metric.version) {
        return Err(MentionSyncError::MetricChanged);
    }
    let ledger_present: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_local_schema_migration WHERE migration_id = '0211_opportunity_comment_mentions')",
    )
    .fetch_one(&mut *tx)
    .await?;
    if !ledger_present {
        return Err(MentionSyncError::MigrationLedgerMissing);
    }
    let problems_synced: i64 =
        sqlx::query_scalar("SELECT synced FROM linggan_opportunity_sync_problems()")
            .fetch_one(&mut *tx)
            .await?;
    let sources = read_sources(&mut tx).await?;
    validate_sources(&sources)?;
    let existing = verify_existing_digest(&mut tx, digest).await?;
    let mut projection =
        project_sources(&mut tx, &sources, digest, metric.version, &existing).await?;
    reconcile_projected(&mut tx, digest, &mut projection.counts).await?;
    collect_eligibility(&mut tx, &metric.params, &mut projection.counts).await?;
    let report = MentionSyncReport {
        source_memberships: sources.len(),
        unique_comment_problem_pairs: projection.unique_pairs,
        inserted: projection.inserted,
        updated: projection.updated,
        already_present: projection
            .unique_pairs
            .saturating_sub((projection.inserted + projection.updated) as usize),
        unknown_commenters: projection.unknown_commenters,
        problems_synced,
        reconciliations: reconciliations(projection.counts, &metric.params),
    };
    tx.commit().await?;
    Ok(report)
}
