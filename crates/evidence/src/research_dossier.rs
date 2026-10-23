//! The sole full-note contract. Content is stored once in immutable reading blocks.
use crate::creator_discovery::{Fragment, hash};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

pub mod content;
mod coverage;
mod duplicates;
mod inventory;
pub mod native;
mod projection;
mod relayer;
mod render;
mod selection;
pub use relayer::relayer_work;
pub use render::{context_sources, markdown, render_materials};
pub use selection::{Selection, select};

pub const VERSION: &str = "research-dossier.v2";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchDossier {
    pub contract: &'static str,
    pub work_ref: Uuid,
    pub revision: String,
    pub material_revision: String,
    pub as_of: String,
    pub note: Value,
    pub materials: Vec<Value>,
    pub coverage: Value,
    pub attachments: Value,
    pub selection: Value,
    #[serde(skip_serializing)]
    pub fragments: Vec<Fragment>,
    #[serde(skip_serializing)]
    pub media: Vec<Value>,
    #[serde(skip_serializing)]
    pub media_inventory: Vec<Value>,
    #[serde(skip_serializing)]
    pub candidate_statuses: Vec<Value>,
    #[serde(skip_serializing)]
    pub metadata: Value,
}

/// A query batch is not a material limit: every requested work and every block is returned.
pub async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, ResearchDossier>, sqlx::Error> {
    let mut output = BTreeMap::new();
    for batch in works.chunks(100) {
        let currents =
            crate::work_resource_current::read_work_resource_currents(tx, batch, as_of).await?;
        let mut readings =
            crate::material_reading::read_current_in(&mut **tx, batch, as_of).await?;
        let mut inventories = inventory::read_in(tx, batch, as_of).await?;
        let mut layouts = relayer::source_layouts_in(&mut **tx, batch, as_of).await?;
        let mut candidates =
            crate::material_reading::candidate_statuses_in(&mut **tx, batch, as_of).await?;
        for work in currents {
            let reference = work.public_ref;
            let mut dossier = projection::assemble(
                &work,
                readings.remove(&reference).unwrap_or_default(),
                inventories.remove(&reference).unwrap_or_default(),
                layouts.remove(&reference).unwrap_or_default(),
                as_of,
            );
            dossier.candidate_statuses = candidates.remove(&reference).unwrap_or_default();
            refresh_revision(&mut dossier);
            dossier.selection["completeRevision"] = json!(dossier.revision);
            output.insert(reference, dossier);
        }
    }
    Ok(output)
}

pub async fn read(
    database: &linggan_storage_postgres::Database,
    work: Uuid,
) -> Result<Option<ResearchDossier>, sqlx::Error> {
    read_at(database, work, None).await
}

pub async fn read_at(
    database: &linggan_storage_postgres::Database,
    work: Uuid,
    as_of: Option<&str>,
) -> Result<Option<ResearchDossier>, sqlx::Error> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let cutoff = cutoff_in(&mut tx, as_of).await?;
    let dossier = read_in(&mut tx, &[work], &cutoff).await?.remove(&work);
    tx.commit().await?;
    Ok(dossier)
}

pub async fn cutoff_in(
    tx: &mut Transaction<'_, Postgres>,
    value: Option<&str>,
) -> Result<String, sqlx::Error> {
    let valid: bool =
        sqlx::query_scalar("SELECT COALESCE($1::timestamptz,scope_001_now())<=scope_001_now()")
            .bind(value)
            .fetch_one(&mut **tx)
            .await?;
    if !valid {
        return Err(sqlx::Error::Protocol("future_cutoff".into()));
    }
    sqlx::query_scalar("SELECT to_char(COALESCE($1::timestamptz,scope_001_now()) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')")
        .bind(value).fetch_one(&mut **tx).await
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingVersion {
    pub work_ref: Uuid,
    pub material: Value,
    /// The unit's real current pointer in the same snapshot, independent of page deduplication.
    pub current_revision_ref: Option<Uuid>,
}

pub async fn read_version(
    database: &linggan_storage_postgres::Database,
    work: Uuid,
    reading: Uuid,
) -> Result<Option<ReadingVersion>, sqlx::Error> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let cutoff = cutoff_in(&mut tx, None).await?;
    let material =
        crate::material_reading::read_versions_in(&mut *tx, &[work], &cutoff, &[reading])
            .await?
            .remove(&work)
            .and_then(|v| v.into_iter().next());
    let result = if let Some(material) = material {
        let unit: Uuid = serde_json::from_value(material["ref"].clone())
            .map_err(|_| sqlx::Error::Protocol("reading_unit_invalid".into()))?;
        let current_revision_ref =
            crate::material_reading::current_reference_in(&mut tx, work, unit, &cutoff).await?;
        Some(ReadingVersion {
            work_ref: work,
            material: projection::public_material(material),
            current_revision_ref,
        })
    } else {
        None
    };
    tx.commit().await?;
    Ok(result)
}

pub async fn plan(
    database: &linggan_storage_postgres::Database,
    work: Uuid,
) -> Result<Option<Value>, sqlx::Error> {
    Ok(read(database, work).await?.map(|d| relayer_plan(&d)))
}

pub fn relayer_plan(dossier: &ResearchDossier) -> Value {
    let layouts: Vec<_> = dossier
        .media
        .iter()
        .filter(|v| v["layoutRef"].is_string())
        .collect();
    let current = layouts
        .iter()
        .filter(|v| v["ruleVersion"] == relayer::rule_version())
        .count();
    let protected = layouts
        .iter()
        .filter(|v| v["humanProtected"] == true)
        .count();
    let pending = layouts
        .iter()
        .filter(|v| v["humanProtected"] != true && v["ruleVersion"] != relayer::rule_version())
        .count();
    let image_slots: Vec<_> = dossier
        .media_inventory
        .iter()
        .filter(|v| v["component"] == "image")
        .collect();
    let pending_review = image_slots
        .iter()
        .filter(|s| {
            dossier
                .candidate_statuses
                .iter()
                .any(|c| c["kind"] == "image" && same_image_source(s, c))
        })
        .count();
    let needs_ocr = image_slots
        .iter()
        .filter(|s| {
            s["sourceState"] != "restricted"
                && !dossier
                    .materials
                    .iter()
                    .any(|m| same_image_source(s, m) && m["currentRevisionRef"].is_string())
                && !layouts.iter().any(|m| same_image_source(s, m))
                && !dossier
                    .candidate_statuses
                    .iter()
                    .any(|c| c["kind"] == "image" && same_image_source(s, c))
        })
        .count();
    json!({"workCount":1,"imageSlots":image_slots.len(),"imageJobs":layouts.len(),"layoutCount":layouts.len(),
        "pendingLayouts":pending,"pendingReviewCount":pending_review,"alreadyCurrent":current,"humanProtected":protected,"needsOcr":needs_ocr,
        "expectedRevision":dossier.revision})
}

fn same_image_source(slot: &Value, reading: &Value) -> bool {
    let matches = |source: &Value| {
        source["slotKey"] == reading["slotKey"]
            && source["sourceVersion"] == reading["sourceVersion"]
    };
    matches(slot)
        || slot["sameBytesSources"]
            .as_array()
            .is_some_and(|sources| sources.iter().any(matches))
}

pub(super) fn refresh_revision(dossier: &mut ResearchDossier) {
    dossier.material_revision =
        hash(&json!([VERSION, dossier.note, dossier.materials]).to_string());
    dossier.revision = hash(
        &json!([
            dossier.material_revision,
            dossier.coverage,
            dossier.metadata,
            dossier.candidate_statuses
        ])
        .to_string(),
    );
}

pub async fn creation_work_directory_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    query: Option<&str>,
    offset: i64,
) -> Result<(Vec<Uuid>, i64), sqlx::Error> {
    let filter = "FROM linggan_material_content material WHERE EXISTS(SELECT 1 FROM linggan_material_domain_usage usage WHERE usage.content_public_ref=material.public_ref AND usage.domain_ref=$1) AND ($2::text IS NULL OR EXISTS(SELECT 1 FROM linggan_material_content_detail detail JOIN linggan_runtime_capture_package package USING(package_ref) WHERE detail.content_public_ref=material.public_ref AND package.accepted_at<=scope_001_now() AND (lower(COALESCE(detail.title,'')) LIKE '%'||lower($2)||'%' OR lower(COALESCE(detail.body_text,'')) LIKE '%'||lower($2)||'%')))";
    let total = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) {filter}")))
        .bind(domain)
        .bind(query)
        .fetch_one(&mut **tx)
        .await?;
    let refs = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT material.public_ref {filter} ORDER BY material.public_ref LIMIT 31 OFFSET $3"
    )))
    .bind(domain)
    .bind(query)
    .bind(offset)
    .fetch_all(&mut **tx)
    .await?;
    Ok((refs, total))
}

pub async fn creation_work_in_domain(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    domain: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE content_public_ref=$1 AND domain_ref=$2)")
        .bind(work).bind(domain).fetch_one(&mut **tx).await
}
