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
pub mod historical;
mod inventory;
pub mod native;
mod projection;
mod reading;
mod render;
mod selection;
pub use reading::ordered_refs;
pub use render::{context_sources, markdown, render_materials};
pub use selection::{Selection, segment, select};

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
    pub reading_view: Value,
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

/// Display metadata for internal adapters; source text remains in the canonical blocks.
pub fn reading_title(dossier: &ResearchDossier) -> Value {
    json!({"text":dossier.note["displayTitle"],"basis":dossier.note["displayTitleBasis"],"blockRefs":dossier.note["titleBlockRefs"],"platformSource":dossier.note["titleSource"]})
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
        output
            .extend(read_from_currents_in(tx, &currents.iter().collect::<Vec<_>>(), as_of).await?);
    }
    Ok(output)
}

/// Internal reuse of Current already read by a caller in this transaction/as_of.
/// Reading adoption, source eligibility, inventory and assembly still have one owner.
pub(crate) async fn read_from_currents_in(
    tx: &mut Transaction<'_, Postgres>,
    currents: &[&crate::work_resource_current::WorkResourceCurrent],
    as_of: &str,
) -> Result<BTreeMap<Uuid, ResearchDossier>, sqlx::Error> {
    let mut output = BTreeMap::new();
    for batch in currents.chunks(100) {
        let references: Vec<_> = batch.iter().map(|work| work.public_ref).collect();
        let mut readings =
            crate::material_reading::read_current_from_currents_in(&mut **tx, batch, as_of).await?;
        let mut inventories = inventory::read_in(tx, &references, as_of).await?;
        for work in batch {
            let reference = work.public_ref;
            let mut dossier = projection::assemble(
                work,
                readings.remove(&reference).unwrap_or_default(),
                inventories.remove(&reference).unwrap_or_default(),
                vec![],
                as_of,
            );
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

pub(super) fn refresh_revision(dossier: &mut ResearchDossier) {
    dossier.material_revision = hash(
        &json!([
            VERSION,
            dossier.note,
            dossier.materials,
            dossier.reading_view
        ])
        .to_string(),
    );
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
    let filter = "FROM linggan_material_content material WHERE EXISTS(SELECT 1 FROM linggan_material_domain_usage usage WHERE usage.content_public_ref=material.public_ref AND usage.domain_ref=$1 AND usage.use_kind='domain_research') AND ($2::text IS NULL OR EXISTS(SELECT 1 FROM linggan_material_content_detail detail JOIN linggan_runtime_capture_package package USING(package_ref) WHERE detail.content_public_ref=material.public_ref AND package.accepted_at<=scope_001_now() AND (lower(COALESCE(detail.title,'')) LIKE '%'||lower($2)||'%' OR lower(COALESCE(detail.body_text,'')) LIKE '%'||lower($2)||'%')))";
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
