//! The replacement Study source gate. Domain qualification and media readability are database
//! predicates, never prompt instructions. This module depends only on retained material
//! evidence and clean comment-study relations.

use crate::comment_cleaning::clean;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;

#[path = "comment_study_source/context.rs"]
pub(crate) mod context;
#[path = "comment_study_source/gate.rs"]
pub(crate) mod gate;
use uuid::Uuid;

/// Stable identifier used by synthetic fixtures and legacy acceptance examples only.
/// Production pages and commands always carry the selected observation domain.
pub const ADHD_DOMAIN_REF: &str = "00000000-0000-4000-8000-000000000001";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StudySourceRole {
    Primary,
    Reference,
}

impl StudySourceRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Reference => "reference",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StudySource {
    pub source_ref: Uuid,
    pub content_public_ref: Uuid,
    pub work_title: String,
    pub research_text: String,
    pub clean_state: String,
    pub context_manifest: Value,
    pub parent_source_ref: Option<Uuid>,
    pub parent_research_text: Option<String>,
    pub observation_role: StudySourceRole,
}

/// A read-only explanation of the exact source gate used by StudyRun freezing.  It deliberately
/// reports source facts and deterministic cleaning outcomes only; it is not a second eligibility
/// engine for the setup page.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySourcePreview {
    pub as_of: String,
    pub observation_role: StudySourceRole,
    pub total_comment_count: usize,
    pub eligible_comment_count: usize,
    pub excluded_counts: StudySourceExcludedCounts,
    pub works: Vec<StudySourcePreviewWork>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySourceExcludedCounts {
    pub comment_author_unknown: usize,
    pub work_author_unknown: usize,
    pub creator_voice: usize,
    pub body_unavailable: usize,
    pub source_restricted: usize,
    pub text_not_researchable: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySourcePreviewWork {
    pub work_ref: Uuid,
    pub title: String,
    pub eligible_comment_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum StudySourceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the configured Study domain is invalid")]
    InvalidDomain,
}

// Reads below preserve the existing run contract; P2 owns new-only selection and idempotency.
// Candidates are fetched in bounded windows, and work/parent context only after selection.
const SOURCE_PAGE_SIZE: i64 = 128;

pub async fn eligible_sources(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    load_sources(database, domain_ref, as_of, None, limit).await
}

pub async fn eligible_sources_for_works(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
    content_public_refs: &[Uuid],
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    let selections = content_public_refs
        .iter()
        .copied()
        .map(|work_ref| (work_ref, StudySourceRole::Primary))
        .collect::<Vec<_>>();
    if selections.is_empty() {
        return Ok(Vec::new());
    }
    load_sources(database, domain_ref, as_of, Some(&selections), limit).await
}

async fn load_sources(
    database: &Database,
    domain: Uuid,
    as_of: &str,
    selections: Option<&[(Uuid, StudySourceRole)]>,
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='15s'")
        .execute(&mut *tx)
        .await?;
    let result = fetch_eligible_sources(
        &mut tx,
        domain,
        as_of,
        selections,
        Some(StudySourceRole::Primary),
        limit,
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

pub(crate) async fn eligible_sources_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    works: &[Uuid],
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    let selections = works
        .iter()
        .copied()
        .map(|work_ref| (work_ref, StudySourceRole::Primary))
        .collect::<Vec<_>>();
    eligible_sources_in_transaction_for_selections(tx, domain, as_of, &selections, limit).await
}

pub(crate) async fn eligible_sources_in_transaction_for_selections(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    selections: &[(Uuid, StudySourceRole)],
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    fetch_eligible_sources(tx, domain, as_of, Some(selections), None, limit).await
}

struct Candidate {
    source: StudySource,
    parent_id: Option<String>,
}

async fn candidate_page(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    selections: Option<&[(Uuid, StudySourceRole)]>,
    preview_role: Option<StudySourceRole>,
    after: (i64, Uuid),
) -> Result<Vec<sqlx::postgres::PgRow>, StudySourceError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(domain)
            .fetch_one(&mut **tx)
            .await?;
    if !exists {
        return Err(StudySourceError::InvalidDomain);
    }
    let work_refs = selections.map(|items| items.iter().map(|(work, _)| *work).collect::<Vec<_>>());
    let roles = selections.map(|items| {
        items
            .iter()
            .map(|(_, role)| role.as_str())
            .collect::<Vec<_>>()
    });
    let rows = sqlx::query(include_str!("comment_study_source/candidates.sql"))
        .bind(domain)
        .bind(as_of)
        .bind(work_refs)
        .bind(roles)
        .bind(preview_role.map(StudySourceRole::as_str))
        .bind(after.0)
        .bind(after.1)
        .bind(SOURCE_PAGE_SIZE)
        .fetch_all(&mut **tx)
        .await?;
    Ok(rows)
}

fn classify(row: &sqlx::postgres::PgRow) -> Result<Result<Candidate, &'static str>, sqlx::Error> {
    let raw: Option<String> = row.try_get("body_text")?;
    let state: String = row.try_get("body_state")?;
    let author: Option<String> = row.try_get("author_external_id")?;
    let work_author: Option<String> = row.try_get("work_author_external_id")?;
    let known = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let author = known(&author);
    let work_author = known(&work_author);
    let cleaned = clean(raw.as_deref().unwrap_or(""));
    let reason = gate::exclusion([
        row.try_get("source_restricted")?,
        state != "KNOWN" || raw.is_none(),
        false,
        !matches!(cleaned.state.as_str(), "direct" | "context"),
        work_author.is_none(),
        author.is_none(),
        author.is_some() && author == work_author,
    ]);
    if let Some(reason) = reason {
        return Ok(Err(reason));
    }
    let observation_role = match row.try_get::<String, _>("observation_role")?.as_str() {
        "primary" => StudySourceRole::Primary,
        "reference" => StudySourceRole::Reference,
        _ => return Err(sqlx::Error::Protocol("invalid observation role".into())),
    };
    Ok(Ok(Candidate {
        source: StudySource {
            source_ref: row.try_get("material_ref")?,
            content_public_ref: row.try_get("content_public_ref")?,
            work_title: String::new(),
            research_text: cleaned.text,
            clean_state: cleaned.state,
            context_manifest: Value::Null,
            parent_source_ref: None,
            parent_research_text: None,
            observation_role,
        },
        parent_id: row.try_get("parent_comment_external_id")?,
    }))
}

async fn fetch_eligible_sources(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    selections: Option<&[(Uuid, StudySourceRole)]>,
    preview_role: Option<StudySourceRole>,
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    let limit = limit.clamp(1, 3000) as usize;
    let mut after = (0, Uuid::nil());
    let mut selected = Vec::new();
    while selected.len() < limit {
        let page = candidate_page(tx, domain, as_of, selections, preview_role, after).await?;
        if page.is_empty() {
            break;
        }
        for row in &page {
            after = (row.try_get("rank")?, row.try_get("content_public_ref")?);
            if let Ok(candidate) = classify(row)? {
                selected.push(candidate);
            }
            if selected.len() == limit {
                break;
            }
        }
        if page.len() < SOURCE_PAGE_SIZE as usize {
            break;
        }
    }
    attach_context(tx, domain, as_of, selected).await
}

async fn attach_context(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    candidates: Vec<Candidate>,
) -> Result<Vec<StudySource>, StudySourceError> {
    let works: Vec<_> = candidates
        .iter()
        .map(|c| c.source.content_public_ref)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut contexts = BTreeMap::new();
    for chunk in works.chunks(100) {
        contexts.extend(context::read_work_contexts(tx, domain, chunk, as_of).await?);
    }
    let keys: Vec<_> = candidates
        .iter()
        .filter_map(|c| {
            c.parent_id
                .clone()
                .map(|id| (c.source.content_public_ref, id))
        })
        .collect();
    let mut parents = BTreeMap::new();
    for chunk in keys.chunks(128) {
        parents.extend(context::read_parents(tx, chunk, as_of).await?);
    }
    candidates
        .into_iter()
        .map(|mut c| {
            let work = contexts
                .get(&c.source.content_public_ref)
                .ok_or_else(|| sqlx::Error::Protocol("qualified work context is missing".into()))?;
            c.source.work_title = work["displayTitle"]
                .as_str()
                .unwrap_or("未命名作品")
                .to_owned();
            c.source.context_manifest = work["contextManifest"].clone();
            if let Some(id) = c.parent_id
                && let Some(parent) = parents.get(&(c.source.content_public_ref, id))
            {
                c.source.parent_source_ref = parent["sourceRef"]
                    .as_str()
                    .and_then(|v| Uuid::parse_str(v).ok());
                c.source.parent_research_text = parent["researchText"].as_str().map(str::to_owned);
            }
            Ok(c.source)
        })
        .collect()
}

/// Legacy overview summary remains bounded in transfer, with exact counts across all pages.
/// The paginated /works catalog is the user-facing picker; this 100-item preview is not a catalog.
pub async fn preview_sources(
    database: &Database,
    domain: Uuid,
    as_of: &str,
) -> Result<StudySourcePreview, StudySourceError> {
    preview_sources_for_role(database, domain, as_of, StudySourceRole::Primary).await
}

pub async fn preview_sources_for_role(
    database: &Database,
    domain: Uuid,
    as_of: &str,
    observation_role: StudySourceRole,
) -> Result<StudySourcePreview, StudySourceError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='15s'")
        .execute(&mut *tx)
        .await?;
    let mut after = (0, Uuid::nil());
    let mut total = 0;
    let mut counts = BTreeMap::<Uuid, usize>::new();
    let mut excluded = StudySourceExcludedCounts::default();
    loop {
        let page =
            candidate_page(&mut tx, domain, as_of, None, Some(observation_role), after).await?;
        for row in &page {
            total += 1;
            after = (row.try_get("rank")?, row.try_get("content_public_ref")?);
            match classify(row)? {
                Ok(candidate) => {
                    *counts
                        .entry(candidate.source.content_public_ref)
                        .or_default() += 1
                }
                Err(reason) => increment_exclusion(&mut excluded, reason),
            }
        }
        if page.len() < SOURCE_PAGE_SIZE as usize {
            break;
        }
    }
    let eligible = counts.values().sum();
    let mut ordered: Vec<_> = counts.into_iter().collect();
    ordered.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ordered.truncate(100);
    let titles = context::read_titles(
        &mut tx,
        &ordered.iter().map(|value| value.0).collect::<Vec<_>>(),
        as_of,
    )
    .await?;
    let works = ordered
        .into_iter()
        .map(
            |(work_ref, eligible_comment_count)| StudySourcePreviewWork {
                work_ref,
                eligible_comment_count,
                title: titles
                    .get(&work_ref)
                    .and_then(|value| value["displayTitle"].as_str())
                    .unwrap_or("未命名作品")
                    .into(),
            },
        )
        .collect();
    tx.commit().await?;
    Ok(StudySourcePreview {
        as_of: as_of.into(),
        observation_role,
        total_comment_count: total,
        eligible_comment_count: eligible,
        excluded_counts: excluded,
        works,
    })
}

pub async fn preview_sources_for_roles(
    database: &Database,
    domain: Uuid,
    as_of: &str,
    observation_roles: &[StudySourceRole],
) -> Result<BTreeMap<StudySourceRole, StudySourcePreview>, StudySourceError> {
    let mut previews = BTreeMap::new();
    for role in observation_roles.iter().copied() {
        previews.insert(
            role,
            preview_sources_for_role(database, domain, as_of, role).await?,
        );
    }
    Ok(previews)
}

fn increment_exclusion(counts: &mut StudySourceExcludedCounts, reason: &str) {
    match reason {
        "sourceRestricted" => counts.source_restricted += 1,
        "bodyUnavailable" => counts.body_unavailable += 1,
        "workAuthorUnknown" => counts.work_author_unknown += 1,
        "commentAuthorUnknown" => counts.comment_author_unknown += 1,
        "creatorVoice" => counts.creator_voice += 1,
        _ => counts.text_not_researchable += 1,
    }
}
