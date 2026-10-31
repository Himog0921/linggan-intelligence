//! Explicit operator preview/apply, never a fallback in ordinary document reads.
//! Preview is repeatable-read/read-only. Apply rechecks the complete approved proposal.
use super::*;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;
mod bindings;
mod legacy;

fn source_error(error: crate::content_workbench_sources::SourceError) -> WorkbenchError {
    match error {
        crate::content_workbench_sources::SourceError::Database(e) => WorkbenchError::Database(e),
        _ => WorkbenchError::Source,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BindingChange {
    pub excerpt_id: Uuid,
    pub old: Value,
    pub new: Citation,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialChange {
    pub document_id: String,
    pub old_version: String,
    pub new_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetCheck {
    pub id: Uuid,
    pub sha256: String,
    pub byte_size: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryPreview {
    pub contract: String,
    pub database_identity: Value,
    pub work_id: Uuid,
    pub domain_ref: Uuid,
    pub base_revision: Uuid,
    pub base_number: i64,
    pub base_document_sha256: String,
    pub title: String,
    pub qualified_at: String,
    pub authored_sha256: String,
    pub candidate_sha256: String,
    pub bindings: Vec<BindingChange>,
    pub materials: Vec<MaterialChange>,
    pub assets: Vec<AssetCheck>,
}
impl RecoveryPreview {
    pub fn sha256(&self) -> Result<String, WorkbenchError> {
        Ok(hash(serde_json::to_vec(self)?))
    }
}
/// Historical digest for offline audit/fixtures; this does not change the normal decoder.
pub fn legacy_document_sha256(value: Value) -> Result<String, WorkbenchError> {
    Ok(hash(serde_json::to_vec(&legacy::decode(value)?)?))
}
async fn identity(tx: &mut Transaction<'_, Postgres>) -> Result<Value, WorkbenchError> {
    Ok(sqlx::query_scalar("SELECT jsonb_build_object('database',current_database(),'schema',current_schema(),'databaseOid',(SELECT oid::text FROM pg_database WHERE datname=current_database()),'cluster',(SELECT system_identifier::text FROM pg_control_system()))").fetch_one(&mut **tx).await?)
}
async fn build(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    cutoff: Option<&str>,
) -> Result<(RecoveryPreview, WorkDocument), WorkbenchError> {
    let row=sqlx::query("SELECT w.domain_ref,r.revision_ref,r.revision_number,r.document,r.document_sha256 FROM linggan_creation_work w JOIN linggan_creation_revision r ON r.revision_ref=w.current_revision_ref AND r.work_ref=w.work_ref WHERE w.work_ref=$1").bind(work).fetch_optional(&mut **tx).await?.ok_or(WorkbenchError::NotFound)?;
    let raw: Value = row.try_get("document")?;
    let old = legacy::decode(raw.clone())?;
    let digest: String = row.try_get("document_sha256")?;
    if hash(serde_json::to_vec(&old)?) != digest {
        return Err(WorkbenchError::Invalid("legacy_document_digest"));
    }
    let domain: Uuid = row.try_get("domain_ref")?;
    let as_of = match cutoff {
        Some(s) => s.to_owned(),
        None => {
            sqlx::query_scalar("SELECT scope_001_now()::text")
                .fetch_one(&mut **tx)
                .await?
        }
    };
    let mut candidate = serde_json::to_value(&old)?;
    let mut bindings = Vec::new();
    for (i, e) in old.excerpts.iter().enumerate() {
        let new = bindings::bind(tx, domain, e, &as_of).await?;
        candidate["excerpts"][i]["citation"] = serde_json::to_value(&new)?;
        bindings.push(BindingChange {
            excerpt_id: e.id,
            old: serde_json::to_value(&e.citation)?,
            new,
        });
    }
    let mut materials = Vec::new();
    for (i, s) in old.materials.iter().enumerate() {
        let current =
            crate::content_workbench_sources::materials::current_selection_in(tx, domain, s)
                .await
                .map_err(source_error)?;
        candidate["materials"][i]["sourceVersion"] = json!(current.source_version);
        materials.push(MaterialChange {
            document_id: s.document_id.clone(),
            old_version: s.source_version.clone(),
            new_version: current.source_version,
        });
    }
    let authored = legacy::authored_digest(raw)?;
    let doc: WorkDocument = serde_json::from_value(candidate)?;
    validation::document(&doc)?;
    if legacy::authored_digest(serde_json::to_value(&doc)?)? != authored {
        return Err(WorkbenchError::Invalid("recovery_authorship_changed"));
    }
    let mut assets = Vec::new();
    for id in doc
        .pages
        .iter()
        .filter_map(|p| p.asset_id)
        .collect::<std::collections::BTreeSet<_>>()
    {
        let a=sqlx::query("SELECT storage_key,sha256,byte_size FROM linggan_creation_asset WHERE asset_ref=$1 AND work_ref=$2").bind(id).bind(work).fetch_optional(&mut **tx).await?.ok_or(WorkbenchError::Source)?;
        let sha256: String = a.try_get("sha256")?;
        let byte_size: i64 = a.try_get("byte_size")?;
        assets::verified_file(&a.try_get::<String, _>("storage_key")?, &sha256, byte_size)?;
        assets.push(AssetCheck {
            id,
            sha256,
            byte_size,
        });
    }
    let preview = RecoveryPreview {
        contract: "creation-legacy-recovery.v1".into(),
        database_identity: identity(tx).await?,
        work_id: work,
        domain_ref: domain,
        base_revision: row.try_get("revision_ref")?,
        base_number: row.try_get("revision_number")?,
        base_document_sha256: digest,
        title: doc.title.clone(),
        qualified_at: as_of,
        authored_sha256: authored,
        candidate_sha256: hash(serde_json::to_vec(&doc)?),
        bindings,
        materials,
        assets,
    };
    Ok((preview, doc))
}
pub async fn preview(db: &Database, work: Uuid) -> Result<RecoveryPreview, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let (preview, _) = build(&mut tx, work, None).await?;
    tx.commit().await?;
    Ok(preview)
}
/// Requires the exact reviewed preview hash. No authored document is accepted from the caller.
pub async fn apply(
    db: &Database,
    p: &RecoveryPreview,
    expected_hash: &str,
    request: Uuid,
) -> Result<SaveReceipt, WorkbenchError> {
    if request.is_nil()
        || p.contract != "creation-legacy-recovery.v1"
        || p.sha256()? != expected_hash
    {
        return Err(WorkbenchError::Invalid("recovery_preview_hash"));
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    if identity(&mut tx).await? != p.database_identity {
        return Err(WorkbenchError::Invalid("recovery_database_identity"));
    }
    requests::ensure_request_owner_in(&mut tx, request, requests::RequestOwner::Work).await?;
    requests::lock(&mut tx, p.work_id.to_string()).await?;
    let command = hash(serde_json::to_vec(&json!([
        "legacy-recovery.v1",
        p.work_id,
        expected_hash
    ]))?);
    if let Some(mut r) = requests::replay(&mut tx, request, &command).await? {
        source_usage::redact(&mut tx, p.domain_ref, &mut r.revision.document).await?;
        tx.commit().await?;
        return Ok(r);
    }
    let head: Uuid = sqlx::query_scalar(
        "SELECT current_revision_ref FROM linggan_creation_work WHERE work_ref=$1",
    )
    .bind(p.work_id)
    .fetch_one(&mut *tx)
    .await?;
    if head != p.base_revision {
        return Err(WorkbenchError::Invalid("recovery_preview_stale"));
    }
    let (current, doc) = build(&mut tx, p.work_id, Some(&p.qualified_at)).await?;
    if &current != p {
        return Err(WorkbenchError::Invalid("recovery_preview_stale"));
    }
    let previous = Revision {
        id: p.base_revision,
        number: p.base_number,
        saved_at: String::new(),
        document: doc.clone(),
    };
    let revision = store::append(&mut tx, p.work_id, Some(&previous), doc).await?;
    let mut r = requests::receipt(
        &mut tx,
        p.work_id,
        request,
        &command,
        SaveReceipt {
            request_id: request,
            work_id: p.work_id,
            outcome: "legacy_recovered".into(),
            replayed: false,
            revision,
        },
    )
    .await?;
    source_usage::redact(&mut tx, p.domain_ref, &mut r.revision.document).await?;
    tx.commit().await?;
    Ok(r)
}
