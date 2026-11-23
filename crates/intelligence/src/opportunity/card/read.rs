//! Read current cards and their qualified evidence without changing source facts.
use super::{CardError, context, read_model, snapshot, voices};
use crate::comment_study_catalog::{StudyCatalogError, cursor};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use uuid::Uuid;

// Each reader signs only the relations it actually reads (D-061).
const CARD_CHANGE_TABLE: &str = "linggan_opportunity_card_change";
const CARD_READ_TABLES: &[&str] = &[
    "linggan_comment_problem_definition",
    "linggan_comment_problem_redirect",
    "linggan_comment_problem_redirect_revocation",
    "linggan_comment_research_request",
    "linggan_material_comment",
    "linggan_material_comment_restriction",
    CARD_CHANGE_TABLE,
    "linggan_opportunity_opportunity_card",
    "linggan_opportunity_problem",
    "linggan_opportunity_problem_lineage",
];
const LIST_READ_TABLES: &[&str] = &[
    "linggan_comment_problem_definition",
    "linggan_comment_problem_redirect",
    "linggan_comment_problem_redirect_revocation",
    "linggan_comment_research_request",
    "linggan_material_comment",
    "linggan_material_comment_restriction",
    CARD_CHANGE_TABLE,
    "linggan_opportunity_card_period_index",
    "linggan_opportunity_opportunity_card",
    "linggan_opportunity_problem",
    "linggan_opportunity_problem_lineage",
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
    #[error("opportunity_period_snapshot_unavailable")]
    PeriodSnapshotUnavailable,
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
    #[serde(default)]
    pub include_weekly_comparison: bool,
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
    let resolved: Uuid = rows[0].try_get("problem_ref")?;
    let redirected = sqlx::query_scalar(
        "SELECT live.problem_ref FROM linggan_opportunity_problem original
         JOIN linggan_comment_problem_resolution resolution ON resolution.problem_ref=original.source_ref
         JOIN linggan_opportunity_problem live ON live.source_ref=resolution.current_ref
         WHERE original.problem_ref=$1 AND resolution.problem_ref<>resolution.current_ref",
    )
    .bind(resolved)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(redirected.unwrap_or(resolved))
}

struct StoredCard {
    domain: Uuid,
    fingerprint: String,
    card: read_model::Card,
}

async fn mark_stale_cards(
    tx: &mut Transaction<'_, Postgres>,
    cards: &mut [read_model::Card],
) -> Result<(), OpportunityReadError> {
    let ids: Vec<Uuid> = cards.iter().map(|card| card.problem.id).collect();
    let pending: BTreeSet<Uuid> = sqlx::query_scalar(
        "SELECT problem_ref FROM linggan_opportunity_card_change WHERE problem_ref=ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect();
    for card in cards {
        if pending.contains(&card.problem.id)
            && !card
                .flags
                .iter()
                .any(|flag| matches!(flag, read_model::QualityFlag::StaleReadModel))
        {
            card.flags.push(read_model::QualityFlag::StaleReadModel);
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PeriodMatchGroup {
    protocol_version: Option<i32>,
    evidence_tier: read_model::EvidenceTier,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PeriodIndexEntry {
    period_start: String,
    protocol_version: Option<i32>,
    evidence_tier: read_model::EvidenceTier,
}

fn tier_matches(tier: read_model::EvidenceTier, requested: Option<&str>) -> bool {
    requested.is_none_or(|requested| {
        matches!(
            (tier, requested),
            (read_model::EvidenceTier::Candidate, "candidate")
                | (read_model::EvidenceTier::Established, "established")
                | (read_model::EvidenceTier::Significant, "significant")
        )
    })
}

fn valid_period_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return false;
    }
    let Ok(year) = value[0..4].parse::<u32>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u32>() else {
        return false;
    };
    let Ok(day) = value[8..10].parse::<u32>() else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year != 0 && (1..=max_day).contains(&day)
}

async fn load_card(
    tx: &mut Transaction<'_, Postgres>,
    problem: Uuid,
) -> Result<StoredCard, OpportunityReadError> {
    let row = sqlx::query("SELECT p.domain_ref,c.payload,c.input_fingerprint,c.taxonomy_version,c.metric_def_version, \
        EXISTS(SELECT 1 FROM linggan_comment_problem_withdrawn_source x WHERE x.problem_ref=p.source_ref) AS withdrawn_definition, \
        to_char(c.as_of AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS as_of \
        FROM linggan_opportunity_opportunity_card c JOIN linggan_opportunity_problem p USING(problem_ref) WHERE c.problem_ref=$1")
        .bind(problem).fetch_optional(&mut **tx).await?.ok_or(OpportunityReadError::NotFound)?;
    let raw: Value = row.try_get("payload")?;
    let mut card: read_model::Card =
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
    if row.try_get::<bool, _>("withdrawn_definition")? {
        card.problem.statement = "需求来源已撤回".into();
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
    read_card_scoped(database, requested, None).await
}

/// Read a card only when its current problem belongs to the requested domain.
pub async fn read_card_in_domain(
    database: &Database,
    domain: Uuid,
    requested: Uuid,
) -> Result<Value, OpportunityReadError> {
    read_card_scoped(database, requested, Some(domain)).await
}

async fn read_card_scoped(
    database: &Database,
    requested: Uuid,
    domain: Option<Uuid>,
) -> Result<Value, OpportunityReadError> {
    let mut tx = begin(database).await?;
    card_snapshot(&mut tx).await?;
    let problem = resolve(&mut tx, requested).await?;
    let mut stored = load_card(&mut tx, problem).await?;
    if domain.is_some_and(|domain| domain != stored.domain) {
        return Err(OpportunityReadError::NotFound);
    }
    stored.card.problem.redirected_from = (requested != problem).then_some(requested);
    mark_stale_cards(&mut tx, std::slice::from_mut(&mut stored.card)).await?;
    snapshot::verify_for(&mut tx, CARD_READ_TABLES).await?;
    tx.commit().await?;
    serde_json::to_value(stored.card).map_err(|_| OpportunityReadError::InvalidModel)
}

const LIST_SQL: &str = "SELECT p.problem_ref,c.input_fingerprint,(pending.problem_ref IS NOT NULL) AS has_pending_change, \
 CASE WHEN $2::text IS NULL THEN NULL ELSE i.period_index END AS period_index, \
 CASE WHEN $2::text IS NULL THEN NULL ELSE c.payload END AS period_payload \
 FROM linggan_opportunity_opportunity_card c \
 JOIN linggan_opportunity_problem p USING(problem_ref) \
 LEFT JOIN linggan_opportunity_card_change pending ON pending.problem_ref=c.problem_ref \
 LEFT JOIN linggan_opportunity_card_period_index i ON i.problem_ref=c.problem_ref \
 AND i.card_fingerprint=c.input_fingerprint AND i.taxonomy_version=c.taxonomy_version \
 AND i.metric_def_version=c.metric_def_version WHERE p.domain_ref=$1 \
 AND NOT EXISTS(SELECT 1 FROM linggan_comment_problem_active_redirect redirect WHERE redirect.from_problem_ref=p.source_ref) \
 AND NOT EXISTS(SELECT 1 FROM linggan_opportunity_problem_lineage l WHERE l.from_problem_ref=p.problem_ref AND l.relation IN ('merge','rename','split')) \
 AND ($4::text IS NULL OR c.payload#>>'{supply,state}'=$4) \
 AND ($2::text IS NOT NULL OR $3::text IS NULL OR EXISTS(SELECT 1 FROM jsonb_array_elements(c.payload#>'{demand,groups}') g WHERE g->>'evidenceTier'=$3)) \
 ORDER BY p.problem_ref";

fn validate_list_query(
    domain: Uuid,
    query: &OpportunityListQuery,
) -> Result<(), OpportunityReadError> {
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
    Ok(())
}

async fn validate_list_period(
    tx: &mut Transaction<'_, Postgres>,
    period: Option<&str>,
) -> Result<(), OpportunityReadError> {
    if let Some(period) = period {
        let valid = sqlx::query_scalar::<_, bool>("SELECT ($1::date)::text=$1")
            .bind(period)
            .fetch_one(&mut **tx)
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
    Ok(())
}

fn matching_period_groups(
    row: &sqlx::postgres::PgRow,
    period: &str,
    tier: Option<&str>,
    digest: &str,
) -> Result<Vec<PeriodMatchGroup>, OpportunityReadError> {
    let raw: Option<Value> = row.try_get("period_index")?;
    let raw = raw.ok_or(OpportunityReadError::PeriodSnapshotUnavailable)?;
    let index: Vec<PeriodIndexEntry> =
        serde_json::from_value(raw.clone()).map_err(|_| OpportunityReadError::InvalidModel)?;
    if index.is_empty() {
        return Err(OpportunityReadError::InvalidModel);
    }
    let mut keys = BTreeSet::new();
    if index.iter().any(|entry| {
        !valid_period_date(&entry.period_start)
            || entry.protocol_version.is_some_and(|version| version <= 0)
            || !keys.insert((&entry.period_start, entry.protocol_version))
    }) {
        return Err(OpportunityReadError::InvalidModel);
    }
    let payload: Value = row.try_get("period_payload")?;
    let encoded =
        serde_json::to_vec(&(&payload, &raw)).map_err(|_| OpportunityReadError::InvalidModel)?;
    if fingerprint(&encoded) != digest {
        return Err(OpportunityReadError::PeriodSnapshotUnavailable);
    }
    Ok(index
        .into_iter()
        .filter(|entry| entry.period_start == period && tier_matches(entry.evidence_tier, tier))
        .map(|entry| PeriodMatchGroup {
            protocol_version: entry.protocol_version,
            evidence_tier: entry.evidence_tier,
        })
        .collect())
}

type ListEntry = (Uuid, Option<Vec<PeriodMatchGroup>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeeklyGroupComparison {
    protocol_version: Option<i32>,
    current_period_start: String,
    previous_period_start: String,
    current_rate_per_1k: Option<f64>,
    previous_rate_per_1k: Option<f64>,
    status: &'static str,
    reason: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeeklyComparison {
    problem_id: Uuid,
    groups: Vec<WeeklyGroupComparison>,
}

fn compare_weekly_group(
    group: &read_model::DemandGroup,
    current: &str,
    previous: &str,
) -> WeeklyGroupComparison {
    let previous_rate = group
        .series
        .iter()
        .find(|point| point.period_start == previous)
        .and_then(|point| point.rate_per_1k);
    // A previous card's current point must not be labelled as this week's value.
    let current_rate = if group.current.period_start == current {
        group.current.rate_per_1k
    } else {
        None
    };
    let reason = if group.protocol_version.is_none() {
        Some("protocol_unknown")
    } else if group.current.period_start != current {
        Some("card_not_current_week")
    } else if current_rate.is_none() {
        Some("current_unavailable")
    } else if previous_rate.is_none() {
        Some("previous_unavailable")
    } else {
        None
    };
    let status = match (reason, current_rate, previous_rate) {
        (None, Some(now), Some(before)) if now > before => "rising",
        (None, Some(_), Some(_)) => "not_rising",
        _ => "incomparable",
    };
    WeeklyGroupComparison {
        protocol_version: group.protocol_version,
        current_period_start: current.to_owned(),
        previous_period_start: previous.to_owned(),
        current_rate_per_1k: current_rate,
        previous_rate_per_1k: previous_rate,
        status,
        reason,
    }
}

async fn weekly_comparisons(
    tx: &mut Transaction<'_, Postgres>,
    as_of: &str,
    cards: &[read_model::Card],
) -> Result<Vec<WeeklyComparison>, OpportunityReadError> {
    let versions: Vec<i32> = cards.iter().map(|card| card.versions.metric_def).collect();
    let rows = sqlx::query(
        "SELECT definition.version, periods.current::text AS current, \
         (periods.current - 7)::text AS previous FROM linggan_opportunity_metric_definition definition \
         CROSS JOIN LATERAL (SELECT linggan_opportunity_period_start($1::timestamptz, \
           definition.params->>'period_tz') AS current) periods WHERE definition.version=ANY($2)",
    ).bind(as_of).bind(versions).fetch_all(&mut **tx).await?;
    let mut periods = BTreeMap::new();
    for row in rows {
        periods.insert(
            row.try_get::<i32, _>("version")?,
            (
                row.try_get::<String, _>("current")?,
                row.try_get::<String, _>("previous")?,
            ),
        );
    }
    cards
        .iter()
        .map(|card| {
            let (current, previous) = periods
                .get(&card.versions.metric_def)
                .ok_or(OpportunityReadError::InvalidModel)?;
            Ok(WeeklyComparison {
                problem_id: card.problem.id,
                groups: card
                    .demand
                    .groups
                    .iter()
                    .map(|group| compare_weekly_group(group, current, previous))
                    .collect(),
            })
        })
        .collect()
}

fn list_entries(
    rows: Vec<sqlx::postgres::PgRow>,
    query: &OpportunityListQuery,
) -> Result<(Vec<ListEntry>, String), OpportunityReadError> {
    let mut key_data = Vec::new();
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let id: Uuid = row.try_get("problem_ref")?;
        let digest: String = row.try_get("input_fingerprint")?;
        let pending: bool = row.try_get("has_pending_change")?;
        let groups = if let Some(period) = &query.period {
            let groups = matching_period_groups(&row, period, query.tier.as_deref(), &digest)?;
            if groups.is_empty() {
                continue;
            }
            Some(groups)
        } else {
            None
        };
        key_data.extend_from_slice(id.as_bytes());
        key_data.extend_from_slice(digest.as_bytes());
        key_data.push(u8::from(pending));
        let encoded =
            serde_json::to_vec(&groups).map_err(|_| OpportunityReadError::InvalidModel)?;
        let len = u64::try_from(encoded.len()).map_err(|_| OpportunityReadError::InvalidModel)?;
        key_data.extend_from_slice(&len.to_be_bytes());
        key_data.extend_from_slice(&encoded);
        entries.push((id, groups));
    }
    Ok((entries, fingerprint(&key_data)))
}

/// List current cards using a stable, query-bound cursor.
pub async fn read_opportunities(
    database: &Database,
    domain: Uuid,
    query: &OpportunityListQuery,
) -> Result<Value, OpportunityReadError> {
    let limit = page_limit(query.limit)?;
    validate_list_query(domain, query)?;
    let mut scope_value = json!({"domain":domain,"period":query.period,"tier":query.tier,"supplyState":query.supply_state});
    if query.include_weekly_comparison {
        scope_value["includeWeeklyComparison"] = json!(true);
    }
    let scope = scope(scope_value)?;
    let prior = previous("opportunity-list", query.cursor.as_deref(), &scope)?;
    let mut tx = begin(database).await?;
    let mut tables = LIST_READ_TABLES.to_vec();
    if query.include_weekly_comparison {
        tables.push("linggan_opportunity_metric_definition");
        tables.sort_unstable();
    }
    let (captured, _) = snapshot::capture_for(&mut tx, &tables).await?;
    snapshot::require_visible_restrictions(&mut tx).await?;
    let as_of = accepted_as_of(prior.as_ref(), captured)?;
    let domain_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(domain)
            .fetch_one(&mut *tx)
            .await?;
    if !domain_exists {
        return Err(OpportunityReadError::NotFound);
    }
    validate_list_period(&mut tx, query.period.as_deref()).await?;
    let rows = sqlx::query(LIST_SQL)
        .bind(domain)
        .bind(&query.period)
        .bind(&query.tier)
        .bind(&query.supply_state)
        .fetch_all(&mut *tx)
        .await?;
    let (entries, version) = list_entries(rows, query)?;
    if prior
        .as_ref()
        .is_some_and(|prior| prior.last.fingerprint != version)
    {
        return Err(OpportunityReadError::PageChanged);
    }
    let start = prior.as_ref().map_or(0, |prior| {
        entries.partition_point(|(id, _)| *id <= prior.last.reference)
    });
    let end = start.saturating_add(limit).min(entries.len());
    let mut items = Vec::with_capacity(end - start);
    let mut matches = Vec::new();
    for (id, groups) in &entries[start..end] {
        items.push(load_card(&mut tx, *id).await?.card);
        if let Some(period) = &query.period {
            let groups = groups.as_ref().ok_or(OpportunityReadError::InvalidModel)?;
            matches.push(json!({"problemId":id,"periodStart":period,"groups":groups}));
        }
    }
    mark_stale_cards(&mut tx, &mut items).await?;
    let next = if end < entries.len() {
        Some(page_cursor(
            "opportunity-list",
            &scope,
            &as_of,
            entries[end - 1].0,
            end,
            version,
        )?)
    } else {
        None
    };
    let comparisons = if query.include_weekly_comparison {
        Some(weekly_comparisons(&mut tx, &as_of, &items).await?)
    } else {
        None
    };
    snapshot::verify_for(&mut tx, &tables).await?;
    tx.commit().await?;
    let mut result = json!({"domainRef":domain,"items":items,"page":{"limit":limit,"hasMore":next.is_some(),"nextCursor":next,"asOf":as_of}});
    if query.period.is_some() {
        result["periodMatches"] = json!(matches);
    }
    if let Some(comparisons) = comparisons {
        result["weeklyComparisons"] = json!(comparisons);
    }
    Ok(result)
}

type EvidencePage = (Vec<Value>, Value, String, usize);

async fn source_evidence_page(
    tx: &mut Transaction<'_, Postgres>,
    stored: &StoredCard,
    problem: Uuid,
    as_of: &str,
    kind: EvidenceKind,
    start: usize,
    limit: usize,
) -> Result<EvidencePage, OpportunityReadError> {
    let raw: Value = sqlx::query_scalar(
        "SELECT params FROM linggan_opportunity_metric_definition WHERE version=$1",
    )
    .bind(stored.card.versions.metric_def)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(OpportunityReadError::InvalidModel)?;
    let params = crate::opportunity::metric_definition::parse_params(raw)
        .map_err(|_| OpportunityReadError::InvalidModel)?;
    let source_kind = if matches!(kind, EvidenceKind::Comments) {
        voices::PageKind::Comments
    } else {
        voices::PageKind::Works
    };
    let page = voices::select_page(
        tx,
        &voices::SourceScope {
            domain: stored.domain,
            problem,
            taxonomy: stored.card.versions.taxonomy,
            as_of,
            tz: &params.period_tz,
            basis: context::basis(&params),
        },
        start,
        limit,
        source_kind,
    )
    .await
    .map_err(|error| match error {
        voices::SourcePageError::Card(error) => OpportunityReadError::Snapshot(error),
        voices::SourcePageError::SourceSelectionChanged => OpportunityReadError::SourceUnavailable,
    })?;
    let version = fingerprint(
        &serde_json::to_vec(&json!({
            "card":stored.fingerprint,"source":page.fingerprint,"kind":kind
        }))
        .map_err(|_| OpportunityReadError::InvalidModel)?,
    );
    Ok((
        page.items,
        serde_json::to_value(page.excluded).map_err(|_| OpportunityReadError::InvalidModel)?,
        version,
        page.total,
    ))
}

fn creator_evidence_page(
    stored: &StoredCard,
    start: usize,
    limit: usize,
) -> Result<EvidencePage, OpportunityReadError> {
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
    let exclusions = json!({});
    let version = fingerprint(
        &serde_json::to_vec(&json!({
            "card":stored.fingerprint,"items":items,"exclusions":exclusions
        }))
        .map_err(|_| OpportunityReadError::InvalidModel)?,
    );
    let total = items.len();
    Ok((
        items.into_iter().skip(start).take(limit).collect(),
        exclusions,
        version,
        total,
    ))
}

/// Return qualified comment sources, their works, or protocol-separated creator evidence.
pub async fn read_evidence(
    database: &Database,
    requested: Uuid,
    query: &EvidenceQuery,
) -> Result<Value, OpportunityReadError> {
    read_evidence_scoped(database, requested, query, None).await
}

/// Read evidence only within the card's domain.
pub async fn read_evidence_in_domain(
    database: &Database,
    domain: Uuid,
    requested: Uuid,
    query: &EvidenceQuery,
) -> Result<Value, OpportunityReadError> {
    read_evidence_scoped(database, requested, query, Some(domain)).await
}

/// A concurrent writer can outlast NOWAIT without making the source unavailable.
async fn begin_evidence_snapshot<'a>(
    database: &'a Database,
    tables: &[&str],
) -> Result<(Transaction<'a, Postgres>, String), OpportunityReadError> {
    let mut delays = [50, 150, 350].into_iter();
    loop {
        let mut tx = begin(database).await?;
        match snapshot::capture_for(&mut tx, tables).await {
            Ok((captured, _)) => return Ok((tx, captured)),
            Err(CardError::SourcesBusy) => {
                tx.rollback().await?;
                let Some(ms) = delays.next() else {
                    return Err(CardError::SourcesBusy.into());
                };
                tokio::time::sleep(Duration::from_millis(ms)).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

async fn read_evidence_scoped(
    database: &Database,
    requested: Uuid,
    query: &EvidenceQuery,
    domain: Option<Uuid>,
) -> Result<Value, OpportunityReadError> {
    let limit = page_limit(query.limit)?;
    let mut scope_value = json!({"problem":requested,"kind":query.kind});
    if let Some(domain) = domain {
        scope_value["domain"] = json!(domain);
    }
    let scope = scope(scope_value)?;
    let prior = previous("opportunity-evidence", query.cursor.as_deref(), &scope)?;
    let evidence_tables = snapshot::evidence_read_tables();
    // Creator evidence reads the stored card, but does not inspect pending changes.
    let creator_tables: Vec<_> = CARD_READ_TABLES
        .iter()
        .copied()
        .filter(|table| *table != CARD_CHANGE_TABLE)
        .collect();
    let tables = if matches!(query.kind, EvidenceKind::Creators) {
        creator_tables.as_slice()
    } else {
        &evidence_tables
    };
    let (mut tx, captured) = begin_evidence_snapshot(database, tables).await?;
    snapshot::require_visible_restrictions(&mut tx).await?;
    let as_of = accepted_as_of(prior.as_ref(), captured)?;
    let problem = resolve(&mut tx, requested).await?;
    let stored = load_card(&mut tx, problem).await?;
    if domain.is_some_and(|domain| domain != stored.domain) {
        return Err(OpportunityReadError::NotFound);
    }
    let start = prior.as_ref().map_or(Ok(0), |prior| {
        usize::try_from(prior.last.offset).map_err(|_| OpportunityReadError::InvalidCursor)
    })?;
    let (items, exclusions, version, total) = match query.kind {
        EvidenceKind::Comments | EvidenceKind::Works => {
            source_evidence_page(&mut tx, &stored, problem, &as_of, query.kind, start, limit)
                .await?
        }
        EvidenceKind::Creators => creator_evidence_page(&stored, start, limit)?,
    };
    if prior
        .as_ref()
        .is_some_and(|prior| prior.last.reference != problem || prior.last.fingerprint != version)
    {
        return Err(OpportunityReadError::PageChanged);
    }
    if start > total {
        return Err(OpportunityReadError::InvalidCursor);
    }
    let end = start.saturating_add(limit).min(total);
    let next = if end < total {
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
    let result = json!({"problem":{"id":problem,"redirectedFrom":(requested!=problem).then_some(requested)},
        "kind":query.kind,"cardAsOf":stored.card.as_of,"items":items,"excluded":exclusions,
        "page":{"limit":limit,"hasMore":next.is_some(),"nextCursor":next,"asOf":as_of}});
    snapshot::verify_for(&mut tx, tables).await?;
    tx.commit().await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{compare_weekly_group, read_model};

    #[test]
    fn weekly_comparison_requires_same_known_protocol_and_current_week() {
        let mut card: read_model::Card = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/opportunity_metrics_postgres/p1-card.snapshot.json"
        )))
        .unwrap();
        let group = &mut card.demand.groups[0];
        group.protocol_version = Some(1);
        group.current.rate_per_1k = Some(12.0);
        let rising = compare_weekly_group(group, "2026-11-09", "2026-11-02");
        assert_eq!(rising.status, "rising");
        assert_eq!(rising.previous_rate_per_1k, Some(10.0));
        group.current.rate_per_1k = Some(9.0);
        assert_eq!(
            compare_weekly_group(group, "2026-11-09", "2026-11-02").status,
            "not_rising"
        );
        group.protocol_version = None;
        assert_eq!(
            compare_weekly_group(group, "2026-11-09", "2026-11-02").reason,
            Some("protocol_unknown")
        );
        group.protocol_version = Some(1);
        let stale = compare_weekly_group(group, "2026-11-16", "2026-11-09");
        assert_eq!(stale.reason, Some("card_not_current_week"));
        assert_eq!(stale.current_period_start, "2026-11-16");
        assert_eq!(stale.current_rate_per_1k, None);
        group.current.rate_per_1k = None;
        assert_eq!(
            compare_weekly_group(group, "2026-11-09", "2026-11-02").reason,
            Some("current_unavailable")
        );
        group.current.rate_per_1k = Some(12.0);
        assert_eq!(
            compare_weekly_group(group, "2026-11-09", "2026-09-01").reason,
            Some("previous_unavailable")
        );
    }
}
