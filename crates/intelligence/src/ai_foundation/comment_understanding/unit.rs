//! A frozen research unit as the executor reads it back, and the facts recorded at dispatch.
use crate::{ai_foundation::store::FoundationError, comment_study_source::StudySourceRole};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct FrozenTarget {
    pub request_ref: Uuid,
    pub source_ref: Uuid,
    pub role: StudySourceRole,
    /// 1-based; the target's alias in the call is `t<position>`.
    pub position: i32,
    pub knowledge_cutoff: String,
    /// A result row exists: the target was settled before the call, e.g. withheld.
    pub settled: bool,
}

#[derive(Debug, Clone)]
pub struct Unit {
    pub unit_ref: Uuid,
    pub item_ref: Uuid,
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub l1_request_ref: Uuid,
    pub targets: Vec<FrozenTarget>,
}

pub async fn load_in(
    tx: &mut Transaction<'_, Postgres>,
    unit_ref: Uuid,
) -> Result<Unit, FoundationError> {
    let unit = sqlx::query("SELECT item_ref,domain_ref,work_ref,l1_request_ref FROM linggan_comment_research_unit WHERE unit_ref=$1")
        .bind(unit_ref)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(FoundationError::NotFound)?;
    let rows = sqlx::query("SELECT ut.request_ref,ut.position,r.source_ref,r.observation_role,to_char(r.knowledge_cutoff AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS cutoff,EXISTS(SELECT 1 FROM linggan_comment_research_result x WHERE x.request_ref=r.request_ref) AS settled FROM linggan_comment_research_unit_target ut JOIN linggan_comment_research_request r ON r.request_ref=ut.request_ref WHERE ut.unit_ref=$1 ORDER BY ut.position")
        .bind(unit_ref)
        .fetch_all(&mut **tx)
        .await?;
    let targets = rows
        .into_iter()
        .map(|row| {
            let role = match row.try_get::<String, _>("observation_role")?.as_str() {
                "primary" => StudySourceRole::Primary,
                "reference" => StudySourceRole::Reference,
                _ => return Err(FoundationError::Invalid),
            };
            Ok(FrozenTarget {
                request_ref: row.try_get("request_ref")?,
                source_ref: row.try_get("source_ref")?,
                role,
                position: row.try_get("position")?,
                knowledge_cutoff: row.try_get("cutoff")?,
                settled: row.try_get("settled")?,
            })
        })
        .collect::<Result<Vec<_>, FoundationError>>()?;
    Ok(Unit {
        unit_ref,
        item_ref: unit.try_get("item_ref")?,
        domain_ref: unit.try_get("domain_ref")?,
        work_ref: unit.try_get("work_ref")?,
        l1_request_ref: unit.try_get("l1_request_ref")?,
        targets,
    })
}

/// The frozen L1 Core card the unit was planned against.
pub async fn l1_card_in(
    tx: &mut Transaction<'_, Postgres>,
    l1_request_ref: Uuid,
) -> Result<Option<Value>, FoundationError> {
    Ok(sqlx::query_scalar("SELECT result->'card' FROM linggan_content_understanding_saved_core WHERE request_ref=$1 AND result IS NOT NULL")
        .bind(l1_request_ref)
        .fetch_optional(&mut **tx)
        .await?)
}

/// Written in the transaction that creates the invocation, before the call: it is the only way
/// to turn the model's `p<n>` aliases back into Problems.
pub async fn record_dispatch_in(
    tx: &mut Transaction<'_, Postgres>,
    unit_ref: Uuid,
    invocation: Uuid,
    catalog: &[Uuid],
) -> Result<(), FoundationError> {
    sqlx::query("INSERT INTO linggan_comment_research_unit_dispatch(unit_ref,invocation_ref,catalog_problem_refs) VALUES($1,$2,$3)")
        .bind(unit_ref)
        .bind(invocation)
        .bind(catalog)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn dispatch_in(
    tx: &mut Transaction<'_, Postgres>,
    unit_ref: Uuid,
    invocation: Uuid,
) -> Result<Option<Vec<Uuid>>, FoundationError> {
    Ok(sqlx::query_scalar("SELECT catalog_problem_refs FROM linggan_comment_research_unit_dispatch WHERE unit_ref=$1 AND invocation_ref=$2")
        .bind(unit_ref)
        .bind(invocation)
        .fetch_optional(&mut **tx)
        .await?)
}
