//! Read current cards and their qualified evidence without changing source facts.
use super::{CardError, context, read_model, snapshot, voices};
use crate::comment_study_catalog::{StudyCatalogError, cursor};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

// Only these relations are read when listing or returning an existing card.
const CARD_READ_TABLES: &[&str] = &[
    "linggan_material_comment_restriction",
    "linggan_opportunity_opportunity_card",
    "linggan_opportunity_problem",
    "linggan_opportunity_problem_lineage",
    "linggan_opportunity_problem_metric_period",
    "observation_domain",
];

#[derive(Debug, thiserror::Error)]
pub enum OpportunityReadError {
    #[error("opportunity_read_database_unavailable")]
    Database(#[from] sqlx::Error),
    #[error("opportunity_read_snapshot_unavailable")]
    Snapshot(#[from] CardError),
    #[error("opportunity_invalid_request")]
    InvalidQuery,
    #[error("opportunity_invalid_limit")]
    InvalidLimit,
    #[error("opportunity_invalid_cursor")]
    InvalidCursor,
    #[error("opportunity_cursor_scope_mismatch")]
    CursorScopeMismatch,
    #[error("opportunity_page_changed")]
    PageChanged,
    #[error("opportunity_not_found")]
    NotFound,
    #[error("opportunity_problem_lineage_ambiguous")]
    LineageConflict,
    #[error("opportunity_card_source_unavailable")]
    SourceUnavailable,
    #[error("opportunity_read_model_invalid")]
    InvalidModel,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpportunityListQuery {
    pub period: Option<String>,
    pub tier: Option<String>,
    pub supply_state: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Comments,
    Works,
    Creators,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceQuery {
    pub kind: EvidenceKind,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Position {
    timestamp: String,
    reference: Uuid,
    offset: u64,
    fingerprint: String,
}

impl cursor::CursorPosition for Position {
    fn timestamp(&self) -> &str {
        &self.timestamp
    }
    fn valid_key(&self) -> bool {
        !self.reference.is_nil()
            && self.fingerprint.len() == 64
            && self
                .fingerprint
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

fn cursor_error(error: StudyCatalogError) -> OpportunityReadError {
    match error {
        StudyCatalogError::CursorScopeMismatch => OpportunityReadError::CursorScopeMismatch,
        _ => OpportunityReadError::InvalidCursor,
    }
}

fn page_limit(limit: Option<i64>) -> Result<usize, OpportunityReadError> {
    let limit = limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(OpportunityReadError::InvalidLimit);
    }
    usize::try_from(limit).map_err(|_| OpportunityReadError::InvalidLimit)
}

fn fingerprint(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn scope(value: Value) -> Result<String, OpportunityReadError> {
    cursor::scope_hash(&value).map_err(cursor_error)
}

fn previous(
    resource: &str,
    token: Option<&str>,
    scope: &str,
) -> Result<Option<cursor::Cursor<Position>>, OpportunityReadError> {
    token
        .map(|value| cursor::decode_for(resource, value, scope).map_err(cursor_error))
        .transpose()
}

fn page_cursor(
    resource: &str,
    scope: &str,
    as_of: &str,
    reference: Uuid,
    offset: usize,
    fingerprint: String,
) -> Result<String, OpportunityReadError> {
    let position = Position {
        timestamp: as_of.to_owned(),
        reference,
        offset: u64::try_from(offset).map_err(|_| OpportunityReadError::InvalidCursor)?,
        fingerprint,
    };
    cursor::encode_for(resource, scope, as_of, position).map_err(cursor_error)
}

async fn begin(database: &Database) -> Result<Transaction<'_, Postgres>, OpportunityReadError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

async fn card_snapshot(tx: &mut Transaction<'_, Postgres>) -> Result<String, OpportunityReadError> {
    let (as_of, _) = snapshot::capture_for(tx, CARD_READ_TABLES).await?;
    snapshot::require_visible_restrictions(tx).await?;
    Ok(as_of)
}

fn accepted_as_of(
    prior: Option<&cursor::Cursor<Position>>,
    captured: String,
) -> Result<String, OpportunityReadError> {
    match prior {
        Some(prior) if prior.as_of <= captured && prior.last.timestamp == prior.as_of => {
            Ok(prior.as_of.clone())
        }
        Some(_) => Err(OpportunityReadError::InvalidCursor),
        None => Ok(captured),
    }
}

async fn resolve(
    tx: &mut Transaction<'_, Postgres>,
    requested: Uuid,
) -> Result<Uuid, OpportunityReadError> {
    if requested.is_nil() {
        return Err(OpportunityReadError::InvalidQuery);
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_opportunity_problem WHERE problem_ref=$1)",
    )
    .bind(requested)
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(OpportunityReadError::NotFound);
    }
    let rows =
        sqlx::query("SELECT problem_ref,path FROM linggan_opportunity_resolve_problem_id($1)")
            .bind(requested)
            .fetch_all(&mut **tx)
            .await
            .map_err(|error| {
                if error
                    .as_database_error()
                    .and_then(|db| db.code())
                    .as_deref()
                    == Some("22023")
                {
                    OpportunityReadError::LineageConflict
                } else {
                    OpportunityReadError::Database(error)
                }
            })?;
    if rows.len() != 1 {
        return Err(OpportunityReadError::LineageConflict);
    }
    let path: Vec<Uuid> = rows[0].try_get("path")?;
    let split: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_opportunity_problem_lineage WHERE from_problem_ref=ANY($1) AND relation='split')")
        .bind(path).fetch_one(&mut **tx).await?;
    if split {
        return Err(OpportunityReadError::LineageConflict);
    }
    Ok(rows[0].try_get("problem_ref")?)
}

struct StoredCard {
    domain: Uuid,
    fingerprint: String,
    card: read_model::Card,
}

async fn load_card(
    tx: &mut Transaction<'_, Postgres>,
    problem: Uuid,
) -> Result<StoredCard, OpportunityReadError> {
    let row = sqlx::query("SELECT p.domain_ref,c.payload,c.input_fingerprint,c.taxonomy_version,c.metric_def_version, \
        to_char(c.as_of AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS as_of \
        FROM linggan_opportunity_opportunity_card c JOIN linggan_opportunity_problem p USING(problem_ref) WHERE c.problem_ref=$1")
        .bind(problem).fetch_optional(&mut **tx).await?.ok_or(OpportunityReadError::NotFound)?;
    let raw: Value = row.try_get("payload")?;
    let card: read_model::Card =
        serde_json::from_value(raw).map_err(|_| OpportunityReadError::InvalidModel)?;
    let db_as_of: String = row.try_get("as_of")?;
    if card.problem.id != problem
        || card.problem.redirected_from.is_some()
        || !(1..=3).contains(&card.problem.level)
        || card.versions.taxonomy != row.try_get::<i64, _>("taxonomy_version")?
        || card.versions.metric_def != row.try_get::<i32, _>("metric_def_version")?
        || card.as_of != db_as_of
        || card
            .demand
            .sample_voices
            .iter()
            .any(|voice| voice.tombstone && voice.text.is_some())
        || card
            .supply
            .works
            .iter()
            .any(|work| work.tombstone && work.title.is_some())
    {
        return Err(OpportunityReadError::InvalidModel);
    }
    let restricted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r \
        JOIN jsonb_array_elements($1::jsonb#>'{demand,sampleVoices}') voice \
          ON r.content_public_ref::text=voice#>>'{commentRef,contentRef}' \
         AND r.comment_external_id=voice#>>'{commentRef,commentExternalId}')",
    )
    .bind(serde_json::to_value(&card).map_err(|_| OpportunityReadError::InvalidModel)?)
    .fetch_one(&mut **tx)
    .await?;
    if restricted {
        return Err(OpportunityReadError::SourceUnavailable);
    }
    Ok(StoredCard {
        domain: row.try_get("domain_ref")?,
        fingerprint: row.try_get("input_fingerprint")?,
        card,
    })
}

/// Return the persisted card; historical recalculation is intentionally not offered here.
pub async fn read_card(
    database: &Database,
    requested: Uuid,
) -> Result<Value, OpportunityReadError> {
    let mut tx = begin(database).await?;
    card_snapshot(&mut tx).await?;
    let problem = resolve(&mut tx, requested).await?;
    let mut stored = load_card(&mut tx, problem).await?;
    stored.card.problem.redirected_from = (requested != problem).then_some(requested);
    snapshot::verify_for(&mut tx, CARD_READ_TABLES).await?;
    tx.commit().await?;
    serde_json::to_value(stored.card).map_err(|_| OpportunityReadError::InvalidModel)
}

const LIST_SQL: &str = "SELECT p.problem_ref,c.input_fingerprint FROM linggan_opportunity_opportunity_card c \
 JOIN linggan_opportunity_problem p USING(problem_ref) WHERE p.domain_ref=$1 \
 AND NOT EXISTS(SELECT 1 FROM linggan_opportunity_problem_lineage l WHERE l.from_problem_ref=p.problem_ref AND l.relation IN ('merge','rename','split')) \
 AND ($4::text IS NULL OR c.payload#>>'{supply,state}'=$4) \
 AND (($2::text IS NULL AND ($3::text IS NULL OR EXISTS(SELECT 1 FROM jsonb_array_elements(c.payload#>'{demand,groups}') g WHERE g->>'evidenceTier'=$3))) \
   OR ($2::text IS NOT NULL AND EXISTS(SELECT 1 FROM linggan_opportunity_problem_metric_period m \
       WHERE m.problem_ref=p.problem_ref AND m.taxonomy_version=c.taxonomy_version AND m.metric_def_version=c.metric_def_version \
       AND m.computed_as_of=c.as_of AND m.period_start::text=$2 AND ($3::text IS NULL OR m.evidence_tier=$3)))) \
 ORDER BY p.problem_ref";

/// List current cards using a stable, query-bound cursor.
pub async fn read_opportunities(
    database: &Database,
    domain: Uuid,
    query: &OpportunityListQuery,
) -> Result<Value, OpportunityReadError> {
    let limit = page_limit(query.limit)?;
    if domain.is_nil()
        || query
            .tier
            .as_deref()
            .is_some_and(|x| !matches!(x, "candidate" | "established" | "significant"))
        || query.supply_state.as_deref().is_some_and(|x| {
            !matches!(
                x,
                "not_evaluated"
                    | "awaiting_understanding"
                    | "insufficient_coverage"
                    | "covered"
                    | "suspected_gap"
            )
        })
    {
        return Err(OpportunityReadError::InvalidQuery);
    }
    let scope = scope(
        json!({"domain":domain,"period":query.period,"tier":query.tier,"supplyState":query.supply_state}),
    )?;
    let prior = previous("opportunity-list", query.cursor.as_deref(), &scope)?;
    let mut tx = begin(database).await?;
    let captured = card_snapshot(&mut tx).await?;
    let as_of = accepted_as_of(prior.as_ref(), captured)?;
    let domain_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(domain)
            .fetch_one(&mut *tx)
            .await?;
    if !domain_exists {
        return Err(OpportunityReadError::NotFound);
    }
    if let Some(period) = &query.period {
        let valid = sqlx::query_scalar::<_, bool>("SELECT ($1::date)::text=$1")
            .bind(period)
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| {
                match error
                    .as_database_error()
                    .and_then(|db| db.code())
                    .as_deref()
                {
                    Some("22007" | "22008") => OpportunityReadError::InvalidQuery,
                    _ => OpportunityReadError::Database(error),
                }
            })?;
        if !valid {
            return Err(OpportunityReadError::InvalidQuery);
        }
    }
    let rows = sqlx::query(LIST_SQL)
        .bind(domain)
        .bind(&query.period)
        .bind(&query.tier)
        .bind(&query.supply_state)
        .fetch_all(&mut *tx)
        .await?;
    let mut key_data = Vec::new();
    let mut ids = Vec::with_capacity(rows.len());
    for row in rows {
        let id: Uuid = row.try_get("problem_ref")?;
        let digest: String = row.try_get("input_fingerprint")?;
        key_data.extend_from_slice(id.as_bytes());
        key_data.extend_from_slice(digest.as_bytes());
        ids.push(id);
    }
    let version = fingerprint(&key_data);
    if prior
        .as_ref()
        .is_some_and(|prior| prior.last.fingerprint != version)
    {
        return Err(OpportunityReadError::PageChanged);
    }
    let start = prior.as_ref().map_or(0, |prior| {
        ids.partition_point(|id| *id <= prior.last.reference)
    });
    let end = start.saturating_add(limit).min(ids.len());
    let mut items = Vec::with_capacity(end - start);
    for id in &ids[start..end] {
        items.push(load_card(&mut tx, *id).await?.card);
    }
    let next = if end < ids.len() {
        Some(page_cursor(
            "opportunity-list",
            &scope,
            &as_of,
            ids[end - 1],
            end,
            version,
        )?)
    } else {
        None
    };
    snapshot::verify_for(&mut tx, CARD_READ_TABLES).await?;
    tx.commit().await?;
    Ok(
        json!({"domainRef":domain,"items":items,"page":{"limit":limit,"hasMore":next.is_some(),"nextCursor":next,"asOf":as_of}}),
    )
}

/// Return qualified comment sources, their works, or protocol-separated creator evidence.
pub async fn read_evidence(
    database: &Database,
    requested: Uuid,
    query: &EvidenceQuery,
) -> Result<Value, OpportunityReadError> {
    let limit = page_limit(query.limit)?;
    let scope = scope(json!({"problem":requested,"kind":query.kind}))?;
    let prior = previous("opportunity-evidence", query.cursor.as_deref(), &scope)?;
    let mut tx = begin(database).await?;
    let (captured, _) = snapshot::capture(&mut tx).await?;
    snapshot::require_visible_restrictions(&mut tx).await?;
    let as_of = accepted_as_of(prior.as_ref(), captured)?;
    let problem = resolve(&mut tx, requested).await?;
    let stored = load_card(&mut tx, problem).await?;
    let (items, exclusions) = match query.kind {
        EvidenceKind::Comments | EvidenceKind::Works => {
            let raw: Value = sqlx::query_scalar(
                "SELECT params FROM linggan_opportunity_metric_definition WHERE version=$1",
            )
            .bind(stored.card.versions.metric_def)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(OpportunityReadError::InvalidModel)?;
            let params = crate::opportunity::metric_definition::parse_params(raw)
                .map_err(|_| OpportunityReadError::InvalidModel)?;
            let (voices, exclusions) = voices::select_all(
                &mut tx,
                stored.domain,
                problem,
                stored.card.versions.taxonomy,
                &as_of,
                &params.period_tz,
                context::basis(&params),
            )
            .await?;
            let items = if matches!(query.kind, EvidenceKind::Comments) {
                voices
                    .into_iter()
                    .map(|voice| {
                        serde_json::to_value(voice).map_err(|_| OpportunityReadError::InvalidModel)
                    })
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                voices
                    .into_iter()
                    .map(|voice| voice.work_id)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .map(|work| json!({"workId":work,"basis":"comment_mention"}))
                    .collect()
            };
            (
                items,
                serde_json::to_value(exclusions).map_err(|_| OpportunityReadError::InvalidModel)?,
            )
        }
        EvidenceKind::Creators => {
            let mut items = Vec::new();
            for group in &stored.card.channel.groups {
                for (section, creators) in [
                    ("ranked", &group.ranked),
                    ("insufficient", &group.insufficient),
                ] {
                    for creator in creators {
                        items.push(json!({"protocolVersion":group.protocol_version,"section":section,"creator":creator}));
                    }
                }
            }
            (items, json!({}))
        }
    };
    let bytes = serde_json::to_vec(
        &json!({"card":stored.fingerprint,"items":items,"exclusions":exclusions}),
    )
    .map_err(|_| OpportunityReadError::InvalidModel)?;
    let version = fingerprint(&bytes);
    if prior
        .as_ref()
        .is_some_and(|prior| prior.last.reference != problem || prior.last.fingerprint != version)
    {
        return Err(OpportunityReadError::PageChanged);
    }
    let start = prior.as_ref().map_or(Ok(0), |prior| {
        usize::try_from(prior.last.offset).map_err(|_| OpportunityReadError::InvalidCursor)
    })?;
    if start > items.len() {
        return Err(OpportunityReadError::InvalidCursor);
    }
    let end = start.saturating_add(limit).min(items.len());
    let next = if end < items.len() {
        Some(page_cursor(
            "opportunity-evidence",
            &scope,
            &as_of,
            problem,
            end,
            version,
        )?)
    } else {
        None
    };
    let page_items = &items[start..end];
    let result = json!({"problem":{"id":problem,"redirectedFrom":(requested!=problem).then_some(requested)},
        "kind":query.kind,"cardAsOf":stored.card.as_of,"items":page_items,"excluded":exclusions,
        "page":{"limit":limit,"hasMore":next.is_some(),"nextCursor":next,"asOf":as_of}});
    snapshot::verify(&mut tx).await?;
    tx.commit().await?;
    Ok(result)
}
