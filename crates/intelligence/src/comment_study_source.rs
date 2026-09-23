//! The replacement Study source gate. Domain qualification and media readability are database
//! predicates, never prompt instructions. This module depends only on retained material
//! evidence and clean comment-study relations.

use crate::comment_cleaning::clean;
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;

#[path = "comment_study_source/gate.rs"]
pub(crate) mod gate;
#[path = "comment_study_source/context.rs"]
pub(crate) mod context;
use uuid::Uuid;

pub const ADHD_DOMAIN_REF: &str = "00000000-0000-4000-8000-000000000001";

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
}

/// A read-only explanation of the exact source gate used by StudyRun freezing.  It deliberately
/// reports source facts and deterministic cleaning outcomes only; it is not a second eligibility
/// engine for the setup page.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySourcePreview {
    pub as_of: String,
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
    database: &Database, domain_ref: Uuid, as_of: &str, limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    load_sources(database,domain_ref,as_of,None,limit).await
}

pub async fn eligible_sources_for_works(
    database: &Database, domain_ref: Uuid, as_of: &str,
    content_public_refs: &[Uuid], limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    if content_public_refs.is_empty() { return Ok(Vec::new()); }
    load_sources(database,domain_ref,as_of,Some(content_public_refs),limit).await
}

async fn load_sources(
    database: &Database, domain: Uuid, as_of: &str, works: Option<&[Uuid]>, limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    let mut tx=database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout='15s'").execute(&mut *tx).await?;
    let result=fetch_eligible_sources(&mut tx,domain,as_of,works,limit).await?;
    tx.commit().await?;
    Ok(result)
}

pub(crate) async fn eligible_sources_in_transaction(
    tx: &mut Transaction<'_,Postgres>, domain: Uuid, as_of: &str,
    works: &[Uuid], limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    fetch_eligible_sources(tx,domain,as_of,Some(works),limit).await
}

struct Candidate {
    source: StudySource,
    parent_id: Option<String>,
}

async fn candidate_page(
    tx: &mut Transaction<'_,Postgres>, domain: Uuid, as_of: &str,
    works: Option<&[Uuid]>, after: (i64,Uuid),
) -> Result<Vec<sqlx::postgres::PgRow>,StudySourceError> {
    if domain.to_string()!=ADHD_DOMAIN_REF { return Err(StudySourceError::InvalidDomain); }
    let rows=sqlx::query(include_str!("comment_study_source/candidates.sql"))
        .bind(domain).bind(as_of).bind(works.map(<[Uuid]>::to_vec))
        .bind(after.0).bind(after.1).bind(SOURCE_PAGE_SIZE).fetch_all(&mut **tx).await?;
    Ok(rows)
}

fn classify(row: &sqlx::postgres::PgRow) -> Result<Result<Candidate,&'static str>,sqlx::Error> {
    let raw: Option<String>=row.try_get("body_text")?;
    let state: String=row.try_get("body_state")?;
    let author: Option<String>=row.try_get("author_external_id")?;
    let work_author: Option<String>=row.try_get("work_author_external_id")?;
    let known=|v: &Option<String>|v.as_deref().map(str::trim).filter(|s|!s.is_empty()).map(str::to_owned);
    let author=known(&author);
    let work_author=known(&work_author);
    let cleaned=clean(raw.as_deref().unwrap_or(""));
    let reason=gate::exclusion([
        row.try_get("source_restricted")?, state!="KNOWN"||raw.is_none(), false,
        !matches!(cleaned.state.as_str(),"direct"|"context"),work_author.is_none(),author.is_none(),
        author.is_some()&&author==work_author,
    ]);
    if let Some(reason)=reason { return Ok(Err(reason)); }
    Ok(Ok(Candidate{
        source: StudySource{
            source_ref:row.try_get("material_ref")?,content_public_ref:row.try_get("content_public_ref")?,
            work_title:String::new(),research_text:cleaned.text,clean_state:cleaned.state,
            context_manifest:Value::Null,parent_source_ref:None,parent_research_text:None,
        },parent_id:row.try_get("parent_comment_external_id")?,
    }))
}

async fn fetch_eligible_sources(
    tx: &mut Transaction<'_,Postgres>, domain: Uuid, as_of: &str,
    works: Option<&[Uuid]>, limit: i64,
) -> Result<Vec<StudySource>,StudySourceError> {
    let limit=limit.clamp(1,3000) as usize;
    let mut after=(0,Uuid::nil());
    let mut selected=Vec::new();
    while selected.len()<limit {
        let page=candidate_page(tx,domain,as_of,works,after).await?;
        if page.is_empty() { break; }
        for row in &page {
            after=(row.try_get("rank")?,row.try_get("content_public_ref")?);
            if let Ok(candidate)=classify(row)? { selected.push(candidate); }
            if selected.len()==limit { break; }
        }
        if page.len()<SOURCE_PAGE_SIZE as usize { break; }
    }
    attach_context(tx,domain,as_of,selected).await
}

async fn attach_context(
    tx: &mut Transaction<'_,Postgres>,domain: Uuid,as_of: &str,candidates: Vec<Candidate>,
) -> Result<Vec<StudySource>,StudySourceError> {
    let works: Vec<_>=candidates.iter().map(|c|c.source.content_public_ref)
        .collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let mut contexts=BTreeMap::new();
    for chunk in works.chunks(100) {
        contexts.extend(context::read_work_contexts(tx,domain,chunk,as_of).await?);
    }
    let keys: Vec<_>=candidates.iter().filter_map(|c|c.parent_id.clone().map(|id|(c.source.content_public_ref,id))).collect();
    let mut parents=BTreeMap::new();
    for chunk in keys.chunks(128) { parents.extend(context::read_parents(tx,chunk,as_of).await?); }
    candidates.into_iter().map(|mut c| {
        let work=contexts.get(&c.source.content_public_ref).ok_or_else(||
            sqlx::Error::Protocol("qualified work context is missing".into()))?;
        c.source.work_title=work["displayTitle"].as_str().unwrap_or("未命名作品").to_owned();
        c.source.context_manifest=work["contextManifest"].clone();
        if let Some(id)=c.parent_id
            && let Some(parent)=parents.get(&(c.source.content_public_ref,id)) {
                c.source.parent_source_ref=parent["sourceRef"].as_str().and_then(|v|Uuid::parse_str(v).ok());
                c.source.parent_research_text=parent["researchText"].as_str().map(str::to_owned);
        }
        Ok(c.source)
    }).collect()
}

/// Legacy overview summary remains bounded in transfer, with exact counts across all pages.
/// The paginated /works catalog is the user-facing picker; this 100-item preview is not a catalog.
pub async fn preview_sources(
    database: &Database, domain: Uuid, as_of: &str,
) -> Result<StudySourcePreview,StudySourceError> {
    let mut tx=database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout='15s'").execute(&mut *tx).await?;
    let mut after=(0,Uuid::nil());
    let mut total=0;
    let mut counts=BTreeMap::<Uuid,usize>::new();
    let mut excluded=StudySourceExcludedCounts::default();
    loop {
        let page=candidate_page(&mut tx,domain,as_of,None,after).await?;
        for row in &page {
            total+=1;
            after=(row.try_get("rank")?,row.try_get("content_public_ref")?);
            match classify(row)? {
                Ok(c)=>*counts.entry(c.source.content_public_ref).or_default()+=1,
                Err(reason)=>increment_exclusion(&mut excluded,reason),
            }
        }
        if page.len()<SOURCE_PAGE_SIZE as usize { break; }
    }
    let eligible=counts.values().sum();
    let mut ordered: Vec<_>=counts.into_iter().collect();
    ordered.sort_by(|a,b|b.1.cmp(&a.1).then_with(||a.0.cmp(&b.0)));
    ordered.truncate(100);
    let titles=context::read_titles(&mut tx,&ordered.iter().map(|v|v.0).collect::<Vec<_>>(),as_of).await?;
    let works=ordered.into_iter().map(|(work_ref,eligible_comment_count)|StudySourcePreviewWork{
        work_ref,eligible_comment_count,
        title:titles.get(&work_ref).and_then(|w|w["displayTitle"].as_str()).unwrap_or("未命名作品").into(),
    }).collect();
    tx.commit().await?;
    Ok(StudySourcePreview{as_of:as_of.into(),total_comment_count:total,
        eligible_comment_count:eligible,excluded_counts:excluded,works})
}

fn increment_exclusion(counts: &mut StudySourceExcludedCounts,reason: &str) {
    match reason {
        "sourceRestricted"=>counts.source_restricted+=1,
        "bodyUnavailable"=>counts.body_unavailable+=1,
        "workAuthorUnknown"=>counts.work_author_unknown+=1,
        "commentAuthorUnknown"=>counts.comment_author_unknown+=1,
        "creatorVoice"=>counts.creator_voice+=1,
        _=>counts.text_not_researchable+=1,
    }
}
